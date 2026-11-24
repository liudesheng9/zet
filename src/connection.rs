//! Connection trees (`~n`, 关联树), Topic-side mounts (`<loc>+k`) and tree connections: the
//! operations and reads of docs/multi-tree-coordinates-proposal.md §7 and §9.3, shared by the MCP
//! server, the GUI and the Session.
//!
//! Every write runs under the archive edit lock in one transaction. It checks the **host**
//! authority (the Workspace and Lock of the tree that owns the placements and connections) apart
//! from the **home** authority of any Card it reads: mounting or connecting a Card never writes the
//! Card. A write that changes what a reader saw before (a move, an unplace, a rebind, a deleted
//! connection, an extraction, a promotion, a seeding) answers a plan first and applies on its
//! confirmation word; `expected_revision` and `plan_hash` from the plan, when passed back, must
//! still match. `op_key` makes a write idempotent: the same key and request replay the stored
//! answer, a different request under the same key is refused.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::coord::{self, ConnTree, EdgeKind, Handle, Placement, TreeRole, TreeShape};
use crate::{Access, CardType, Workspace};

/// The MCP and batch-free write tools of this module.
pub(crate) const WRITE_TOOLS: [&str; 15] = [
    "create_connection_tree",
    "place_card",
    "create_connection_card",
    "edit_placement",
    "move_placement",
    "unplace",
    "rebind_placement",
    "connect_placements",
    "edit_connection",
    "delete_connection",
    "extract_branch",
    "promote_card",
    "seed_connection_tree",
    "edit_connection_tree",
    "review_source",
];

/// The read tools of this module.
pub(crate) const READ_TOOLS: [&str; 5] = [
    "list_connection_trees",
    "get_connection_tree",
    "list_placements",
    "list_connections",
    "get_dependencies",
];

pub(crate) fn is_write_tool(name: &str) -> bool {
    WRITE_TOOLS.contains(&name)
}

pub(crate) fn is_tool(name: &str) -> bool {
    is_write_tool(name) || READ_TOOLS.contains(&name)
}

fn author(access: Access) -> &'static str {
    match access {
        Access::Owner => "owner",
        Access::Agent => "agent",
    }
}

// ---------------------------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------------------------

fn arg_str<'a>(args: &'a Value, name: &str) -> Result<&'a str> {
    args.get(name)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .with_context(|| format!("missing string argument `{name}`"))
}

fn opt_str<'a>(args: &'a Value, name: &str) -> Result<Option<&'a str>> {
    match args.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => bail!("argument `{name}` must be a string"),
    }
}

/// A tri-state patch field: absent (unchanged), null (clear) or a value.
enum Patch<'a> {
    Keep,
    Clear,
    Set(&'a str),
}

fn patch<'a>(args: &'a Value, name: &str) -> Result<Patch<'a>> {
    match args.get(name) {
        None => Ok(Patch::Keep),
        Some(Value::Null) => Ok(Patch::Clear),
        Some(Value::String(s)) if s.is_empty() => Ok(Patch::Clear),
        Some(Value::String(s)) => Ok(Patch::Set(s.as_str())),
        Some(_) => bail!("argument `{name}` must be a string or null"),
    }
}

/// One single-line plain-text field: at most `max` Unicode scalar values, no NUL, no line break,
/// no `[[…]]` Link macro (a gloss or a note never creates a Link).
fn plain_line(value: &str, name: &str, max: usize) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("`{name}` is empty; pass null to clear it");
    }
    if value.chars().count() > max {
        bail!("`{name}` has {} characters; at most {max} are allowed", value.chars().count());
    }
    if value.contains(['\n', '\r', '\0']) {
        bail!("`{name}` must be one line of plain text");
    }
    if value.contains("[[") {
        bail!("`{name}` is plain text and creates no Link; cite Cards in a Card body instead");
    }
    Ok(value.to_string())
}

fn plain_note(value: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("`note` is empty; pass null to clear it");
    }
    if value.chars().count() > coord::NOTE_MAX {
        bail!(
            "`note` has {} characters; at most {} are allowed",
            value.chars().count(),
            coord::NOTE_MAX
        );
    }
    if value.contains('\0') {
        bail!("`note` must be plain text");
    }
    if value.contains("[[") {
        bail!("`note` is plain text and creates no Link; develop the assertion into a Card to cite Cards");
    }
    Ok(value.to_string())
}

fn hash_value(value: &Value) -> String {
    let digest = Sha256::digest(value.to_string().as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

/// The request as the plan hash and the receipt see it: without the confirmation fields.
fn request_core(tool: &str, args: &Value) -> Value {
    let mut core = args.clone();
    if let Some(map) = core.as_object_mut() {
        for key in ["confirm", "plan_hash", "expected_revision", "op_key"] {
            map.remove(key);
        }
    }
    json!({"tool": tool, "args": core})
}

// ---------------------------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------------------------

/// Run one tool of this module under `access`. Reads open their own read; writes take the edit
/// lock and one transaction.
pub(crate) fn run(root: &Path, access: Access, tool: &str, args: &Value) -> Result<Value> {
    if !is_write_tool(tool) {
        let conn = crate::open_db(root)?;
        return read(&conn, access, tool, args);
    }
    let _lock = crate::acquire_edit_lock_within(root, crate::ops::EDIT_PATIENCE, "connection tree")?;
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let op_key = opt_str(args, "op_key")?.map(str::to_string);
    let request_hash = hash_value(&request_core(tool, args));
    if let Some(key) = &op_key {
        let stored: Option<(String, String)> = tx
            .query_row(
                "SELECT request_hash, result_json FROM applied_ops WHERE op_key = ?1",
                [key],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        if let Some((hash, result)) = stored {
            if hash != request_hash {
                bail!("op_key `{key}` was already used for a different request");
            }
            let mut value: Value = serde_json::from_str(&result)?;
            value["replayed"] = json!(true);
            return Ok(value);
        }
    }
    let revision = crate::link_index::revision(&tx)?;
    let mut ctx = Ctx {
        conn: &tx,
        access,
        tool,
        args,
        revision,
        changed: false,
    };
    let value = write(&mut ctx)?;
    let applied = value.get("applied").and_then(Value::as_bool).unwrap_or(true);
    if !applied {
        // A plan writes nothing; the transaction rolls back.
        drop(tx);
        return Ok(value);
    }
    let changed = ctx.changed;
    if changed {
        crate::link_index::bump_revision(&tx)?;
    }
    let mut value = value;
    value["archive_revision"] = json!(crate::link_index::revision(&tx)?);
    if let Some(key) = &op_key
        && changed
    {
        tx.execute(
            "INSERT INTO applied_ops(op_key, request_hash, result_json, committed_at)
             VALUES(?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![key, request_hash, value.to_string()],
        )?;
    }
    tx.commit()?;
    Ok(value)
}

struct Ctx<'a> {
    conn: &'a Connection,
    access: Access,
    tool: &'a str,
    args: &'a Value,
    revision: u64,
    changed: bool,
}

impl Ctx<'_> {
    /// Plan-or-apply: when the confirmation word is absent (or a stale plan is passed back), answer
    /// the plan and write nothing.
    fn confirmed(&self, word: &str, lines: &[String], effects: &Value) -> Result<Option<Value>> {
        let plan_hash = hash_value(&json!({
            "request": request_core(self.tool, self.args),
            "revision": self.revision,
            "effects": effects,
        }));
        let confirm = self.args.get("confirm").and_then(Value::as_str);
        if confirm != Some(word) {
            return Ok(Some(json!({
                "applied": false,
                "confirm": word,
                "lines": lines,
                "effects": effects,
                "plan_hash": plan_hash,
                "expected_revision": self.revision,
            })));
        }
        if let Some(expected) = self.args.get("expected_revision").and_then(Value::as_u64)
            && expected != self.revision
        {
            bail!(
                "stale plan: the archive is at revision {} but the plan was made at revision {expected}; ask for a new plan",
                self.revision
            );
        }
        if let Some(given) = self.args.get("plan_hash").and_then(Value::as_str)
            && given != plan_hash
        {
            bail!("stale plan: the plan changed since it was shown; ask for a new plan");
        }
        Ok(None)
    }
}

fn write(ctx: &mut Ctx) -> Result<Value> {
    match ctx.tool {
        "create_connection_tree" => create_tree(ctx),
        "edit_connection_tree" => edit_tree(ctx),
        "place_card" => place(ctx),
        "create_connection_card" => create_native(ctx),
        "edit_placement" => edit_gloss(ctx),
        "move_placement" => move_station(ctx),
        "unplace" => unplace(ctx),
        "rebind_placement" => rebind(ctx),
        "connect_placements" => connect(ctx),
        "edit_connection" => edit_connection(ctx),
        "delete_connection" => delete_connection(ctx),
        "extract_branch" => extract(ctx),
        "promote_card" => promote(ctx),
        "seed_connection_tree" => crate::dependencies::seed(ctx.conn, ctx.access, ctx.args, &mut |word, lines, effects| ctx_confirm(ctx.tool, ctx.args, ctx.revision, word, lines, effects)).map(|value| {
            if value.get("applied").and_then(Value::as_bool).unwrap_or(true) {
                ctx.changed = true;
            }
            value
        }),
        "review_source" => crate::coord_lock::review_source(ctx.conn, ctx.access, ctx.args).map(|value| {
            ctx.changed = true;
            value
        }),
        other => bail!("unknown Connection tree tool `{other}`"),
    }
}

/// The confirmation step for callers outside `Ctx` (seeding lives in `dependencies`).
fn ctx_confirm(
    tool: &str,
    args: &Value,
    revision: u64,
    word: &str,
    lines: &[String],
    effects: &Value,
) -> Result<Option<Value>> {
    let ctx = Ctx {
        conn: &Connection::open_in_memory()?,
        access: Access::Owner,
        tool,
        args,
        revision,
        changed: false,
    };
    ctx.confirmed(word, lines, effects)
}

// ---------------------------------------------------------------------------------------------
// Host authority
// ---------------------------------------------------------------------------------------------

/// The host of a write: a Connection tree or a Topic tree (for Topic-side mounts).
#[derive(Clone, Debug)]
pub(crate) enum Host {
    Connection(ConnTree),
    Topic { topic: String, workspace: Workspace, root: String },
}

impl Host {
    pub(crate) fn key(&self) -> String {
        match self {
            Host::Connection(tree) => tree.tree_id.clone(),
            Host::Topic { topic, .. } => topic.clone(),
        }
    }

    fn workspace(&self) -> Workspace {
        match self {
            Host::Connection(tree) => tree.workspace,
            Host::Topic { workspace, .. } => *workspace,
        }
    }

    fn label(&self) -> String {
        match self {
            Host::Connection(tree) => tree.handle(),
            Host::Topic { root, .. } => root.clone(),
        }
    }
}

/// The host of an existing placement.
pub(crate) fn host_of(conn: &Connection, placement: &Placement) -> Result<Host> {
    if placement.host_kind == "topic" {
        let root = format!("{}/0", placement.host_tree_key);
        let workspace = crate::load_card(conn, &root)
            .map(|card| card.workspace)
            .with_context(|| format!("Topic `{root}` hosting this mount does not exist"))?;
        Ok(Host::Topic {
            topic: placement.host_tree_key.clone(),
            workspace,
            root,
        })
    } else {
        let tree = coord::tree_by_id(conn, &placement.host_tree_key)?
            .context("the Connection tree of this station was deleted")?;
        Ok(Host::Connection(tree))
    }
}

/// Whether `access` may change the placements and connections of `host` now: its Workspace, and no
/// active Lock on the host tree.
pub(crate) fn can_edit_host(conn: &Connection, access: Access, host: &Host) -> Result<bool> {
    Ok(access.can_write(host.workspace()) && !crate::coord_lock::host_locked(conn, &host.key())?)
}

fn ensure_host_writable(conn: &Connection, access: Access, host: &Host) -> Result<()> {
    access.ensure_writable(host.workspace())?;
    crate::coord_lock::ensure_host_unlocked(conn, &host.key(), &host.label())
}

/// Whether `access` may edit the Card's own text now: its home Workspace and home Lock.
pub(crate) fn can_edit_card(conn: &Connection, access: Access, card: &crate::Card) -> bool {
    access.can_write(card.workspace)
        && crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id)).is_ok()
}

// ---------------------------------------------------------------------------------------------
// Station views
// ---------------------------------------------------------------------------------------------

/// How a station's Card stands: a live Card or a Missing mount (its home deleted the Card).
fn station_value(
    conn: &Connection,
    access: Access,
    host: &Host,
    host_editable: bool,
    placement: &Placement,
    coord_text: &str,
) -> Result<Value> {
    let identity = coord::identity(conn, &placement.card_id)?.context("station names an unknown Card")?;
    let mut value = json!({
        "placement_id": placement.placement_id,
        "coord": coord_text,
        "card_id": placement.card_id,
        "edge": placement.edge.id(),
        "parent_id": placement.parent_id,
        "ordinal": placement.ordinal,
        "gloss": placement.gloss,
        "host": host.label(),
        "host_workspace": host.workspace().id(),
        "can_edit_placement": host_editable,
        "bookmark": format!("place:{}", placement.placement_id),
    });
    if let Some(row) = identity.card_row.filter(|_| identity.live) {
        let card = card_by_row(conn, row)?;
        let role = TreeRole::of(&card);
        let native = matches!(host, Host::Connection(tree) if Handle::parse(&card.address).is_some_and(|h| h.tree == tree.display_num));
        value["status"] = json!("live");
        value["home_address"] = json!(card.address);
        value["home_role"] = json!(role.id());
        value["home_workspace"] = json!(card.workspace.id());
        value["title"] = json!(crate::parse_card_text(&card.text).map(|p| p.title).unwrap_or_else(|_| identity.last_title.clone()));
        value["type"] = json!(card.card_type.id());
        value["origin"] = json!(if native { "native" } else { "mount" });
        value["can_edit_card"] = json!(can_edit_card(conn, access, &card));
        value["citation"] = json!(format!("[[{}]]", citation_address(conn, &placement.card_id, &card.address)?));
        if card.is_lit || card.is_topic || (role == TreeRole::Connection && Handle::parse(&card.address).is_some_and(|h| h.local.is_none())) {
            // A mounted tree root is one portal node: it never expands its home tree.
            value["portal"] = json!({
                "tree": crate::tree_id(&card.address),
                "role": role.id(),
                "cards": home_tree_size(conn, &card)?,
            });
        }
    } else {
        value["status"] = json!("missing");
        value["home_address"] = json!(identity.last_address);
        value["title"] = json!(identity.last_title);
        value["origin"] = json!("mount");
        value["can_edit_card"] = json!(false);
    }
    Ok(value)
}

/// The citable address of a Card: its permanent handle when it has one (a native Card keeps
/// `~7#4` across promotion), else its canonical home address.
pub(crate) fn citation_address(conn: &Connection, card_id: &str, address: &str) -> Result<String> {
    let handle: Option<String> = conn
        .query_row("SELECT handle FROM card_handles WHERE card_id = ?1", [card_id], |r| r.get(0))
        .optional()?;
    Ok(handle.unwrap_or_else(|| address.to_string()))
}

fn home_tree_size(conn: &Connection, card: &crate::Card) -> Result<i64> {
    match TreeRole::of(card) {
        TreeRole::Connection => {
            let Some(tree) = coord::home_tree_of_address(conn, &card.address)? else {
                return Ok(1);
            };
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM placements WHERE host_tree_key = ?1",
                [&tree.tree_id],
                |r| r.get(0),
            )?)
        }
        _ => {
            let tree = crate::tree_id(&card.address);
            Ok(conn.query_row(
                "SELECT COUNT(*) FROM cards WHERE citation_key = ?1 OR location = ?2 OR location LIKE ?3",
                params![tree, format!("{tree}/0"), format!("{tree}/%")],
                |r| r.get(0),
            )?)
        }
    }
}

pub(crate) fn card_by_row(conn: &Connection, row: i64) -> Result<crate::Card> {
    let address: String = conn.query_row(
        "SELECT COALESCE(location, citation_key) FROM cards WHERE row_id = ?1",
        [row],
        |r| r.get(0),
    )?;
    crate::load_card(conn, &address)
}

/// A live Card named by a Card address (`at`/`target_at`) or by `card_id`.
fn target_card(conn: &Connection, args: &Value, at_field: &str) -> Result<(crate::Card, String)> {
    if let Some(card_id) = opt_str(args, "card_id")? {
        let identity = coord::identity(conn, card_id)?.with_context(|| format!("Card `{card_id}` does not exist"))?;
        let row = identity
            .card_row
            .filter(|_| identity.live)
            .with_context(|| format!("Card `{card_id}` was deleted (last at `{}`)", identity.last_address))?;
        return Ok((card_by_row(conn, row)?, card_id.to_string()));
    }
    let at = arg_str(args, at_field)?;
    if coord::is_coordinate(at) || coord::place_bookmark(at).is_some() {
        let station = coord::resolve_station(conn, at)?;
        let address = coord::address_of_card_id(conn, &station.card_id)?
            .with_context(|| format!("the Card at `{at}` was deleted"))?;
        let card = crate::load_card(conn, &address)?;
        return Ok((card, station.card_id));
    }
    let card = crate::load_card(conn, at)?;
    let card_id = coord::card_id_of_row(conn, card.row_id)?;
    Ok((card, card_id))
}

/// A station of `tree` named by a coordinate, a `place:` bookmark or a placement ID.
fn station_in(conn: &Connection, shape: &TreeShape, text: &str) -> Result<Placement> {
    let placement = coord::resolve_station(conn, text)?;
    if placement.host_tree_key != shape.tree.tree_id {
        bail!("station `{text}` is not in Connection tree {}", shape.tree.handle());
    }
    Ok(placement)
}

fn shape_of(conn: &Connection, tree: &ConnTree) -> Result<TreeShape> {
    coord::load_tree_shape(conn, tree)
}

/// The tree that holds a station named by `field` (a placement ID, bookmark or coordinate).
fn station_arg(conn: &Connection, args: &Value, field: &str) -> Result<(Placement, Host)> {
    let text = arg_str(args, field)?;
    let placement = coord::resolve_station(conn, text)?;
    let host = host_of(conn, &placement)?;
    Ok((placement, host))
}

// ---------------------------------------------------------------------------------------------
// Low-level writes
// ---------------------------------------------------------------------------------------------

fn insert_placement(conn: &Connection, p: &Placement) -> Result<()> {
    conn.execute(
        "INSERT INTO placements(placement_id, host_kind, host_tree_key, card_id, parent_id, anchor_card_id, edge, ordinal, gloss)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            p.placement_id,
            p.host_kind,
            p.host_tree_key,
            p.card_id,
            p.parent_id,
            p.anchor_card_id,
            p.edge.id(),
            p.ordinal.map(|v| v as i64),
            p.gloss,
        ],
    )?;
    Ok(())
}

/// Rewrite the structural slot of one station. Side ordinals are first parked above every used
/// ordinal, because the partial unique indexes are checked per statement.
fn set_slot(conn: &Connection, id: &str, parent: Option<&str>, edge: EdgeKind, ordinal: Option<u64>) -> Result<()> {
    conn.execute(
        "UPDATE placements SET parent_id = ?2, edge = ?3, ordinal = ?4 WHERE placement_id = ?1",
        params![id, parent, edge.id(), ordinal.map(|v| v as i64)],
    )?;
    Ok(())
}

/// Park a set of stations out of the way: each becomes a Side station of the root with a unique
/// high ordinal. Used inside one transaction while a plan rearranges stations; the final shape is
/// validated before commit (a Side of the root is never left behind).
fn park(conn: &Connection, shape: &TreeShape, ids: &[String]) -> Result<()> {
    let base: i64 = conn.query_row(
        "SELECT COALESCE(MAX(ordinal), 0) FROM placements WHERE host_tree_key = ?1",
        [&shape.tree.tree_id],
        |r| r.get(0),
    )?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE placements SET parent_id = ?2, edge = 'side', ordinal = ?3 WHERE placement_id = ?1",
            params![id, shape.root, base + 1_000_000 + i as i64],
        )?;
    }
    Ok(())
}

/// Renumber the Side stations of `parent` densely 1..N in their current order.
fn compact_sides(conn: &Connection, host_key: &str, parent: &str) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT placement_id FROM placements WHERE host_tree_key = ?1 AND parent_id = ?2 AND edge = 'side' ORDER BY ordinal",
    )?;
    let ids = stmt
        .query_map(params![host_key, parent], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(stmt);
    let base: i64 = conn.query_row(
        "SELECT COALESCE(MAX(ordinal), 0) FROM placements WHERE host_tree_key = ?1",
        [host_key],
        |r| r.get(0),
    )?;
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1",
            params![id, base + 2_000_000 + i as i64],
        )?;
    }
    for (i, id) in ids.iter().enumerate() {
        conn.execute(
            "UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1",
            params![id, i as i64 + 1],
        )?;
    }
    Ok(())
}

/// Validate the final shape of a Connection tree after a write: the structural invariants, and
/// every live native Card of the tree still has its home station.
fn validate_tree(conn: &Connection, tree: &ConnTree) -> Result<TreeShape> {
    let shape = shape_of(conn, tree).with_context(|| format!("Connection tree {} would break", tree.handle()))?;
    let native_missing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM card_identity i JOIN cards c ON c.row_id = i.card_row
         WHERE i.status = 'live' AND (c.location = ?1 OR c.location LIKE ?2)
           AND NOT EXISTS (SELECT 1 FROM placements p WHERE p.host_tree_key = ?3 AND p.card_id = i.card_id)",
        params![tree.handle(), format!("~{}#%", tree.display_num), tree.tree_id],
        |r| r.get(0),
    )?;
    if native_missing > 0 {
        bail!("a native Card of {} would lose its home station; delete or promote it instead", tree.handle());
    }
    Ok(shape)
}

fn retire_placements(conn: &Connection, ids: &[String], revision: i64) -> Result<()> {
    for id in ids {
        let (host, card): (String, String) = conn.query_row(
            "SELECT host_tree_key, card_id FROM placements WHERE placement_id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO retired_placements(placement_id, host_tree_key, card_id, retired_at_rev) VALUES(?1, ?2, ?3, ?4)",
            params![id, host, card, revision],
        )?;
    }
    Ok(())
}

/// The connections that touch any of `ids`.
fn incident_edges(conn: &Connection, tree_id: &str, ids: &BTreeSet<String>) -> Result<Vec<Edge>> {
    let mut out = Vec::new();
    for edge in edges_of_tree(conn, tree_id)? {
        if ids.contains(&edge.from) || ids.contains(&edge.to) {
            out.push(edge);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------------
// Tree connections (placement_edges)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct Edge {
    pub(crate) connection_id: String,
    pub(crate) tree_id: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) directed: bool,
    pub(crate) label: Option<String>,
    pub(crate) kind: Option<String>,
    pub(crate) note: Option<String>,
    pub(crate) created_by: String,
    pub(crate) created_at_rev: i64,
    pub(crate) updated_by: String,
    pub(crate) updated_at_rev: i64,
}

const EDGE_COLUMNS: &str = "connection_id, tree_id, from_placement_id, to_placement_id, direction, relation_label, relation_kind, note, created_by, created_at_rev, updated_by, updated_at_rev";

fn edge_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Edge> {
    Ok(Edge {
        connection_id: r.get(0)?,
        tree_id: r.get(1)?,
        from: r.get(2)?,
        to: r.get(3)?,
        directed: r.get::<_, String>(4)? == "directed",
        label: r.get(5)?,
        kind: r.get(6)?,
        note: r.get(7)?,
        created_by: r.get(8)?,
        created_at_rev: r.get(9)?,
        updated_by: r.get(10)?,
        updated_at_rev: r.get(11)?,
    })
}

pub(crate) fn edges_of_tree(conn: &Connection, tree_id: &str) -> Result<Vec<Edge>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {EDGE_COLUMNS} FROM placement_edges WHERE tree_id = ?1 ORDER BY created_at_rev, connection_id"
    ))?;
    let rows = stmt
        .query_map([tree_id], edge_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn edge_by_id(conn: &Connection, connection_id: &str) -> Result<Option<Edge>> {
    Ok(conn
        .query_row(
            &format!("SELECT {EDGE_COLUMNS} FROM placement_edges WHERE connection_id = ?1"),
            [connection_id],
            edge_from_row,
        )
        .optional()?)
}

fn insert_edge(conn: &Connection, e: &Edge) -> Result<()> {
    conn.execute(
        &format!("INSERT INTO placement_edges({EDGE_COLUMNS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
        params![
            e.connection_id,
            e.tree_id,
            e.from,
            e.to,
            if e.directed { "directed" } else { "neutral" },
            e.label,
            e.kind,
            e.note,
            e.created_by,
            e.created_at_rev,
            e.updated_by,
            e.updated_at_rev,
        ],
    )?;
    Ok(())
}

fn is_blank(e: &Edge) -> bool {
    !e.directed && e.label.is_none() && e.kind.is_none() && e.note.is_none()
}

/// Check one connection's final tuple and normalize a neutral pair's endpoint order.
fn normalize_edge(e: &mut Edge) -> Result<()> {
    if e.from == e.to {
        bail!("a connection joins two different stations");
    }
    if let Some(kind) = &e.kind {
        if !coord::RELATION_KINDS.contains(&kind.as_str()) {
            bail!(
                "unknown relation_kind `{kind}`; expected one of {} (`term` is reserved for Term links)",
                coord::RELATION_KINDS.join(", ")
            );
        }
        if !e.directed && !["contrasts", "corresponds", "refers"].contains(&kind.as_str()) {
            bail!("relation_kind `{kind}` has a direction; set direction to `directed` and order the endpoints");
        }
    }
    if !e.directed && e.from > e.to {
        std::mem::swap(&mut e.from, &mut e.to);
    }
    Ok(())
}

/// A connection as readers see it, with both endpoints' coordinates and titles and a sentence for
/// a directed assertion.
pub(crate) fn edge_value(conn: &Connection, shape: &TreeShape, e: &Edge) -> Result<Value> {
    let end = |id: &str| -> Result<Value> {
        let p = shape.by_id.get(id).context("connection endpoint is not a station of this tree")?;
        let identity = coord::identity(conn, &p.card_id)?.context("unknown Card")?;
        let title = match identity.card_row.filter(|_| identity.live) {
            Some(row) => {
                let card = card_by_row(conn, row)?;
                crate::parse_card_text(&card.text).map(|p| p.title).unwrap_or(identity.last_title.clone())
            }
            None => identity.last_title.clone(),
        };
        Ok(json!({
            "placement_id": id,
            "coord": shape.coord.get(id),
            "card_id": p.card_id,
            "title": title,
            "status": if identity.live { "live" } else { "missing" },
        }))
    };
    let from = end(&e.from)?;
    let to = end(&e.to)?;
    let predicate = e
        .label
        .clone()
        .or_else(|| e.kind.clone())
        .unwrap_or_else(|| "connects to".to_string());
    let sentence = if e.directed {
        format!(
            "{} {} {}",
            from["title"].as_str().unwrap_or_default(),
            predicate,
            to["title"].as_str().unwrap_or_default()
        )
    } else {
        format!(
            "{} — {} — {}",
            from["title"].as_str().unwrap_or_default(),
            predicate,
            to["title"].as_str().unwrap_or_default()
        )
    };
    Ok(json!({
        "connection_id": e.connection_id,
        "direction": if e.directed { "directed" } else { "neutral" },
        "from": from,
        "to": to,
        "relation_label": e.label,
        "relation_kind": e.kind,
        "note": e.note,
        "provenance": format!("In this tree · {}", shape.tree.handle()),
        "sentence": sentence,
        "created_by": e.created_by,
        "created_at_rev": e.created_at_rev,
        "updated_by": e.updated_by,
        "updated_at_rev": e.updated_at_rev,
    }))
}

// ---------------------------------------------------------------------------------------------
// Create and edit a Connection tree
// ---------------------------------------------------------------------------------------------

fn validate_purpose(value: Option<&str>) -> Result<Option<String>> {
    value
        .map(|p| plain_line(p, "purpose", coord::PURPOSE_MAX))
        .transpose()
}

fn next_connection_number(conn: &Connection) -> Result<u64> {
    let next: Option<String> = conn
        .query_row("SELECT value FROM metadata WHERE key = 'next_connection_id'", [], |r| r.get(0))
        .optional()?;
    let next = next.and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1);
    // Never below a number already issued: a handle survives its tree.
    let issued: Option<i64> = conn.query_row(
        "SELECT MAX(CAST(substr(handle, 2) AS INTEGER)) FROM card_handles WHERE instr(handle, '#') = 0",
        [],
        |r| r.get(0),
    )?;
    let next = next.max(issued.map(|v| v as u64 + 1).unwrap_or(1));
    let after = next.checked_add(1).context("Connection tree numbers exhausted")?;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('next_connection_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [after.to_string()],
    )?;
    Ok(next)
}

/// Create a Connection tree with just a title (and optional framing body and shelf `purpose`).
pub(crate) fn create_tree_tx(
    conn: &Connection,
    access: Access,
    workspace: Workspace,
    title: &str,
    body: &str,
    purpose: Option<&str>,
) -> Result<Value> {
    access.ensure_writable(workspace)?;
    let title = title.trim();
    if title.is_empty() {
        bail!("a Connection tree needs a title");
    }
    let purpose = validate_purpose(purpose)?;
    let num = next_connection_number(conn)?;
    let handle = Handle::root(num);
    let text = crate::compose_card_text(title, body, "");
    let accepted = crate::gate::check_proposed(
        conn,
        &crate::gate::Candidate::create(&handle, crate::gate::Kind::Connection, workspace, &text, CardType::Idea, None),
    )?;
    let parsed = crate::parse_card_text(&text)?;
    let text = crate::compose_card_text(&parsed.title, &parsed.body, &parsed.reverse);
    conn.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
         VALUES(?1, NULL, 0, 0, NULL, ?2, ?3, 'idea')",
        params![handle, text, workspace.id()],
    )?;
    let row = conn.last_insert_rowid();
    let card_id = coord::card_id_of_row(conn, row)?;
    let tree_id = coord::new_uuid()?;
    conn.execute(
        "INSERT INTO connection_trees(tree_id, display_num, workspace, root_card_id, purpose, next_local_seq, created_at)
         VALUES(?1, ?2, ?3, ?4, ?5, 1, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
        params![tree_id, num as i64, workspace.id(), card_id, purpose],
    )?;
    conn.execute(
        "INSERT INTO card_handles(handle, card_id, origin_tree_id, local_seq) VALUES(?1, ?2, ?3, 0)",
        params![handle, card_id, tree_id],
    )?;
    let root_placement = coord::new_uuid()?;
    insert_placement(
        conn,
        &Placement {
            placement_id: root_placement.clone(),
            host_kind: "connection".into(),
            host_tree_key: tree_id.clone(),
            card_id: card_id.clone(),
            parent_id: None,
            anchor_card_id: None,
            edge: EdgeKind::Root,
            ordinal: None,
            gloss: None,
        },
    )?;
    crate::link_index::source_changed(conn, &handle)?;
    let mut value = json!({
        "tree": handle,
        "tree_id": tree_id,
        "address": handle,
        "card_id": card_id,
        "root_placement_id": root_placement,
        "coord": format!("{handle}/0"),
        "workspace": workspace.id(),
        "purpose": purpose,
    });
    accepted.attach(&mut value);
    Ok(value)
}

fn create_tree(ctx: &mut Ctx) -> Result<Value> {
    let workspace = match opt_str(ctx.args, "workspace")? {
        Some(w) => Workspace::parse(w)?,
        None => match ctx.access {
            Access::Agent => Workspace::Agent,
            Access::Owner => Workspace::Human,
        },
    };
    let title = arg_str(ctx.args, "title")?;
    let body = opt_str(ctx.args, "body")?.unwrap_or_default();
    let value = create_tree_tx(ctx.conn, ctx.access, workspace, title, body, opt_str(ctx.args, "purpose")?)?;
    ctx.changed = true;
    Ok(value)
}

/// Change a Connection tree's shelf tag `purpose` (metadata only; the title and body are the root
/// Card's text, edited like any Card).
fn edit_tree(ctx: &mut Ctx) -> Result<Value> {
    let tree = coord::resolve_tree(ctx.conn, arg_str(ctx.args, "tree")?)?;
    let host = Host::Connection(tree.clone());
    ensure_host_writable(ctx.conn, ctx.access, &host)?;
    let purpose = match patch(ctx.args, "purpose")? {
        Patch::Keep => return Ok(json!({"tree": tree.handle(), "purpose": tree.purpose, "changed": false})),
        Patch::Clear => None,
        Patch::Set(p) => Some(plain_line(p, "purpose", coord::PURPOSE_MAX)?),
    };
    if purpose == tree.purpose {
        return Ok(json!({"tree": tree.handle(), "purpose": purpose, "changed": false}));
    }
    ctx.conn.execute(
        "UPDATE connection_trees SET purpose = ?2 WHERE tree_id = ?1",
        params![tree.tree_id, purpose],
    )?;
    ctx.changed = true;
    Ok(json!({"tree": tree.handle(), "purpose": purpose, "changed": true}))
}

// ---------------------------------------------------------------------------------------------
// Place (mount) a Card
// ---------------------------------------------------------------------------------------------

/// Where a new station goes in a Connection tree: under `parent` as its Direct successor (spliced
/// before an existing Direct successor) or as a Side successor (appended, or before a sibling).
struct Slot {
    parent: String,
    edge: EdgeKind,
    before: Option<String>,
}

fn slot_arg(conn: &Connection, shape: &TreeShape, args: &Value) -> Result<Slot> {
    let parent = match (opt_str(args, "parent_placement_id")?, opt_str(args, "parent_coord")?, opt_str(args, "parent")?) {
        (Some(p), None, None) | (None, Some(p), None) | (None, None, Some(p)) => station_in(conn, shape, p)?,
        (None, None, None) => bail!("name the parent station with `parent_coord` or `parent_placement_id`"),
        _ => bail!("give one of `parent_coord`, `parent_placement_id` or `parent`"),
    };
    let edge = match opt_str(args, "edge")?.unwrap_or("direct") {
        "direct" => EdgeKind::Direct,
        "side" => EdgeKind::Side,
        other => bail!("unknown edge `{other}`; expected direct or side"),
    };
    if edge == EdgeKind::Side && parent.edge == EdgeKind::Root {
        bail!("the root takes no Side successor: its Direct chain (`{}/1`, `{}/2`, …) holds the top-level stations", shape.tree.handle(), shape.tree.handle());
    }
    let before = match opt_str(args, "before")? {
        Some(b) => {
            let sibling = station_in(conn, shape, b)?;
            if edge != EdgeKind::Side || sibling.parent_id.as_deref() != Some(parent.placement_id.as_str()) || sibling.edge != EdgeKind::Side {
                bail!("`before` names a Side successor of the same parent");
            }
            Some(sibling.placement_id)
        }
        None => None,
    };
    Ok(Slot {
        parent: parent.placement_id,
        edge,
        before,
    })
}

/// Insert one new station at `slot`. A Direct station is spliced in: the parent's old Direct
/// successor continues after the new station. Returns the stations whose structural predecessor
/// changed (for gloss review).
fn insert_at_slot(conn: &Connection, shape: &TreeShape, slot: &Slot, mut p: Placement) -> Result<Vec<String>> {
    let mut changed = Vec::new();
    match slot.edge {
        EdgeKind::Direct => {
            let displaced = shape.direct.get(&slot.parent).cloned();
            if let Some(d) = &displaced {
                park(conn, shape, std::slice::from_ref(d))?;
            }
            p.parent_id = Some(slot.parent.clone());
            p.edge = EdgeKind::Direct;
            p.ordinal = None;
            insert_placement(conn, &p)?;
            if let Some(d) = displaced {
                set_slot(conn, &d, Some(&p.placement_id), EdgeKind::Direct, None)?;
                changed.push(d);
            }
        }
        EdgeKind::Side => {
            let siblings = shape.side.get(&slot.parent).cloned().unwrap_or_default();
            let position = match &slot.before {
                Some(b) => siblings.iter().position(|s| s == b).context("`before` is not a sibling")?,
                None => siblings.len(),
            };
            // Park the later siblings, insert, then renumber densely.
            let later: Vec<String> = siblings[position..].to_vec();
            park(conn, shape, &later)?;
            p.parent_id = Some(slot.parent.clone());
            p.edge = EdgeKind::Side;
            p.ordinal = Some(position as u64 + 1);
            insert_placement(conn, &p)?;
            for (i, id) in later.iter().enumerate() {
                set_slot(conn, id, Some(&slot.parent), EdgeKind::Side, Some(position as u64 + 2 + i as u64))?;
            }
        }
        _ => bail!("unsupported slot"),
    }
    Ok(changed)
}

/// The gloss decisions a write needs: every station in `affected` whose gloss is nonempty must be
/// named in `gloss_updates` (a string keeps or replaces it, null clears it).
fn gloss_review(shape: &TreeShape, affected: &[String], args: &Value) -> Result<(Vec<Value>, BTreeMap<String, Option<String>>, Vec<String>)> {
    let updates = args.get("gloss_updates").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut review = Vec::new();
    let mut decided = BTreeMap::new();
    let mut missing = Vec::new();
    for id in affected {
        let Some(p) = shape.by_id.get(id) else { continue };
        if let Some(gloss) = &p.gloss {
            review.push(json!({"placement_id": id, "coord": shape.coord.get(id), "gloss": gloss}));
            match updates.get(id) {
                Some(Value::Null) => {
                    decided.insert(id.clone(), None);
                }
                Some(Value::String(s)) if s.trim().is_empty() => {
                    decided.insert(id.clone(), None);
                }
                Some(Value::String(s)) => {
                    decided.insert(id.clone(), Some(plain_line(s, "gloss", coord::GLOSS_MAX)?));
                }
                Some(_) => bail!("gloss_updates values are strings (keep or replace) or null (clear)"),
                None => missing.push(id.clone()),
            }
        }
    }
    for (id, value) in &updates {
        if !affected.contains(id) && shape.by_id.contains_key(id) {
            // An explicit edit of an unaffected gloss is allowed too.
            decided.insert(id.clone(), match value {
                Value::Null => None,
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => Some(plain_line(s, "gloss", coord::GLOSS_MAX)?),
                _ => bail!("gloss_updates values are strings (keep or replace) or null (clear)"),
            });
        }
    }
    Ok((review, decided, missing))
}

fn apply_glosses(conn: &Connection, decided: &BTreeMap<String, Option<String>>) -> Result<()> {
    for (id, gloss) in decided {
        conn.execute("UPDATE placements SET gloss = ?2 WHERE placement_id = ?1", params![id, gloss])?;
    }
    Ok(())
}

fn place(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let edge = opt_str(ctx.args, "edge")?.unwrap_or("direct");
    if edge == "attach" || opt_str(ctx.args, "anchor_at")?.is_some() {
        return attach(ctx);
    }
    let tree = coord::resolve_tree(conn, arg_str(ctx.args, "host")?)?;
    let host = Host::Connection(tree.clone());
    ensure_host_writable(conn, ctx.access, &host)?;
    let (card, card_id) = target_card(conn, ctx.args, "target_at")?;
    let shape = shape_of(conn, &tree)?;
    if let Some(existing) = shape.placement_of_card(&card_id) {
        // One coordinate per Card per tree: placing it again answers its station.
        return Ok(json!({
            "already_placed": true,
            "placement_id": existing.placement_id,
            "coord": shape.coord.get(&existing.placement_id),
            "card_id": card_id,
            "tree": tree.handle(),
            "changed": false,
        }));
    }
    let gloss = opt_str(ctx.args, "gloss")?
        .map(|g| plain_line(g, "gloss", coord::GLOSS_MAX))
        .transpose()?;
    let slot = slot_arg(conn, &shape, ctx.args)?;
    let affected: Vec<String> = match slot.edge {
        EdgeKind::Direct => shape.direct.get(&slot.parent).cloned().into_iter().collect(),
        _ => Vec::new(),
    };
    let (review, decided, missing) = gloss_review(&shape, &affected, ctx.args)?;
    if !review.is_empty() {
        let lines: Vec<String> = review
            .iter()
            .map(|r| format!("review gloss of {} (its predecessor changes): {}", r["coord"].as_str().unwrap_or_default(), r["gloss"].as_str().unwrap_or_default()))
            .collect();
        if !missing.is_empty() && ctx.args.get("confirm").and_then(Value::as_str) == Some("place") {
            bail!("gloss review required: pass gloss_updates for {}", missing.join(", "));
        }
        if let Some(plan) = ctx.confirmed("place", &lines, &json!({"gloss_review": review}))? {
            return Ok(plan);
        }
    }
    let placement_id = coord::new_uuid()?;
    insert_at_slot(
        conn,
        &shape,
        &slot,
        Placement {
            placement_id: placement_id.clone(),
            host_kind: "connection".into(),
            host_tree_key: tree.tree_id.clone(),
            card_id: card_id.clone(),
            parent_id: None,
            anchor_card_id: None,
            edge: slot.edge,
            ordinal: None,
            gloss,
        },
    )?;
    apply_glosses(conn, &decided)?;
    let shape = validate_tree(conn, &tree)?;
    ctx.changed = true;
    Ok(json!({
        "applied": true,
        "placement_id": placement_id,
        "coord": shape.coord.get(&placement_id),
        "card_id": card_id,
        "home_address": card.address,
        "tree": tree.handle(),
        "changed": true,
    }))
}

// ---------------------------------------------------------------------------------------------
// Topic-side mounts (`<loc>+k`)
// ---------------------------------------------------------------------------------------------

fn attach(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let anchor_at = opt_str(ctx.args, "anchor_at")?
        .or(opt_str(ctx.args, "host")?)
        .context("name the Topic home Card with `anchor_at`")?;
    let anchor = crate::load_card(conn, anchor_at)?;
    if TreeRole::of(&anchor) != TreeRole::Topic {
        bail!("a Topic-side mount stands beside a Card of a Topic tree; `{}` is not one", anchor.address);
    }
    let topic = crate::tree_id(&anchor.address).to_string();
    let root = format!("{topic}/0");
    let host = Host::Topic {
        topic: topic.clone(),
        workspace: crate::load_card(conn, &root)?.workspace,
        root,
    };
    ensure_host_writable(conn, ctx.access, &host)?;
    let anchor_id = coord::card_id_of_row(conn, anchor.row_id)?;
    let (card, card_id) = target_card(conn, ctx.args, "target_at")?;
    if TreeRole::of(&card) == TreeRole::Topic && crate::tree_id(&card.address) == topic {
        bail!("`{}` already has its home in Topic {topic}; a Topic-side mount brings a Card from outside", card.address);
    }
    if let Some(existing) = coord::placements_of_host(conn, &topic)?.into_iter().find(|p| p.card_id == card_id) {
        return Ok(json!({
            "already_placed": true,
            "placement_id": existing.placement_id,
            "coord": coord::coordinate_of(conn, &existing)?,
            "card_id": card_id,
            "changed": false,
        }));
    }
    let gloss = opt_str(ctx.args, "gloss")?
        .map(|g| plain_line(g, "gloss", coord::GLOSS_MAX))
        .transpose()?;
    let siblings = coord::attachments_of_anchor(conn, &anchor_id)?;
    let position = match opt_str(ctx.args, "before")? {
        Some(b) => {
            let sibling = coord::resolve_station(conn, b)?;
            siblings
                .iter()
                .position(|s| s.placement_id == sibling.placement_id)
                .context("`before` names a mount beside the same Card")?
        }
        None => siblings.len(),
    };
    let base = siblings.iter().filter_map(|s| s.ordinal).max().unwrap_or(0) + 1_000_000;
    for (i, s) in siblings[position..].iter().enumerate() {
        conn.execute("UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1", params![s.placement_id, (base + i as u64) as i64])?;
    }
    let placement_id = coord::new_uuid()?;
    insert_placement(
        conn,
        &Placement {
            placement_id: placement_id.clone(),
            host_kind: "topic".into(),
            host_tree_key: topic.clone(),
            card_id: card_id.clone(),
            parent_id: None,
            anchor_card_id: Some(anchor_id.clone()),
            edge: EdgeKind::Attach,
            ordinal: Some(position as u64 + 1),
            gloss,
        },
    )?;
    for (i, s) in siblings[position..].iter().enumerate() {
        conn.execute("UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1", params![s.placement_id, (position + 2 + i) as i64])?;
    }
    ctx.changed = true;
    Ok(json!({
        "applied": true,
        "placement_id": placement_id,
        "coord": format!("{}+{}", anchor.address, position + 1),
        "card_id": card_id,
        "home_address": card.address,
        "anchor": anchor.address,
        "changed": true,
    }))
}

/// Renumber the mounts beside one anchor densely after a removal.
fn compact_attachments(conn: &Connection, anchor_id: &str) -> Result<()> {
    let siblings = coord::attachments_of_anchor(conn, anchor_id)?;
    let base = siblings.iter().filter_map(|s| s.ordinal).max().unwrap_or(0) + 2_000_000;
    for (i, s) in siblings.iter().enumerate() {
        conn.execute("UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1", params![s.placement_id, (base + i as u64) as i64])?;
    }
    for (i, s) in siblings.iter().enumerate() {
        conn.execute("UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1", params![s.placement_id, i as i64 + 1])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Native Cards (`~n#k`)
// ---------------------------------------------------------------------------------------------

/// The Card types a Connection tree may author natively. Structured families need their own home
/// structure (Phase 5) or keep their book-bound home.
pub(crate) fn ensure_native_type(card_type: CardType) -> Result<()> {
    match card_type {
        CardType::Idea | CardType::Event | CardType::Finding => Ok(()),
        CardType::Model | CardType::Hub | CardType::Term | CardType::Cited => bail!(
            "native_structure_required: a Connection tree authors Idea, Event and Finding Cards natively; a {} Card keeps its structured home (a Model or Math unit needs its filing structure, a Hub its level's home, a Term or Cited Card its book). Mount an existing one, or write a native Idea Card",
            card_type.id()
        ),
    }
}

fn create_native(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let tree = coord::resolve_tree(conn, arg_str(ctx.args, "tree")?)?;
    let host = Host::Connection(tree.clone());
    ensure_host_writable(conn, ctx.access, &host)?;
    let card_type = match opt_str(ctx.args, "type")? {
        Some(t) => CardType::parse(t)?,
        None => CardType::Idea,
    };
    ensure_native_type(card_type)?;
    let title = arg_str(ctx.args, "title")?;
    let body = opt_str(ctx.args, "body")?.unwrap_or_default();
    let shape = shape_of(conn, &tree)?;
    let slot = slot_arg(conn, &shape, ctx.args)?;
    let affected: Vec<String> = match slot.edge {
        EdgeKind::Direct => shape.direct.get(&slot.parent).cloned().into_iter().collect(),
        _ => Vec::new(),
    };
    let (review, decided, missing) = gloss_review(&shape, &affected, ctx.args)?;
    if !review.is_empty() && !missing.is_empty() {
        bail!(
            "gloss review required: the new Card displaces {}; pass gloss_updates for {}",
            review.iter().filter_map(|r| r["coord"].as_str()).collect::<Vec<_>>().join(", "),
            missing.join(", ")
        );
    }
    let gloss = opt_str(ctx.args, "gloss")?
        .map(|g| plain_line(g, "gloss", coord::GLOSS_MAX))
        .transpose()?;
    let local = tree.next_local_seq;
    let handle = Handle::native(tree.display_num, local);
    let text = crate::compose_card_text(title, body, "");
    let accepted = crate::gate::check_proposed(
        conn,
        &crate::gate::Candidate::create(&handle, crate::gate::Kind::Regular, tree.workspace, &text, card_type, None),
    )?;
    let parsed = crate::parse_card_text(&text)?;
    let text = crate::compose_card_text(&parsed.title, &parsed.body, &parsed.reverse);
    conn.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
         VALUES(?1, NULL, 0, 0, NULL, ?2, ?3, ?4)",
        params![handle, text, tree.workspace.id(), card_type.id()],
    )?;
    let card_id = coord::card_id_of_row(conn, conn.last_insert_rowid())?;
    conn.execute(
        "INSERT INTO card_handles(handle, card_id, origin_tree_id, local_seq) VALUES(?1, ?2, ?3, ?4)",
        params![handle, card_id, tree.tree_id, local as i64],
    )?;
    conn.execute(
        "UPDATE connection_trees SET next_local_seq = ?2 WHERE tree_id = ?1",
        params![tree.tree_id, (local + 1) as i64],
    )?;
    let placement_id = coord::new_uuid()?;
    insert_at_slot(
        conn,
        &shape,
        &slot,
        Placement {
            placement_id: placement_id.clone(),
            host_kind: "connection".into(),
            host_tree_key: tree.tree_id.clone(),
            card_id: card_id.clone(),
            parent_id: None,
            anchor_card_id: None,
            edge: slot.edge,
            ordinal: None,
            gloss,
        },
    )?;
    apply_glosses(conn, &decided)?;
    crate::link_index::source_changed(conn, &handle)?;
    let shape = validate_tree(conn, &tree)?;
    ctx.changed = true;
    let mut value = json!({
        "applied": true,
        "address": handle,
        "handle": handle,
        "card_id": card_id,
        "type": card_type.id(),
        "placement_id": placement_id,
        "coord": shape.coord.get(&placement_id),
        "tree": tree.handle(),
    });
    accepted.attach(&mut value);
    Ok(value)
}

// ---------------------------------------------------------------------------------------------
// Gloss
// ---------------------------------------------------------------------------------------------

fn edit_gloss(ctx: &mut Ctx) -> Result<Value> {
    let (placement, host) = station_arg(ctx.conn, ctx.args, "placement_id")?;
    ensure_host_writable(ctx.conn, ctx.access, &host)?;
    if placement.edge == EdgeKind::Root {
        bail!("the root station has no positional gloss; its framing text is the root Card's body");
    }
    let gloss = match patch(ctx.args, "gloss")? {
        Patch::Keep => bail!("pass `gloss` (a string) or null to clear it"),
        Patch::Clear => None,
        Patch::Set(g) => Some(plain_line(g, "gloss", coord::GLOSS_MAX)?),
    };
    if let Some(base) = ctx.args.get("base_gloss") {
        let base = base.as_str().map(str::to_string);
        if base != placement.gloss {
            bail!("stale gloss: the station's gloss changed since it was read");
        }
    }
    let changed = gloss != placement.gloss;
    if changed {
        ctx.conn.execute(
            "UPDATE placements SET gloss = ?2 WHERE placement_id = ?1",
            params![placement.placement_id, gloss],
        )?;
        ctx.changed = true;
    }
    Ok(json!({"placement_id": placement.placement_id, "gloss": gloss, "changed": changed}))
}

// ---------------------------------------------------------------------------------------------
// Move a station
// ---------------------------------------------------------------------------------------------

/// Move one station inside its Connection tree. The station carries its Side branches; its old
/// Direct continuation closes the gap (takes its old slot); at the destination a displaced Direct
/// successor continues after it. Every station whose structural predecessor changes and that has a
/// nonempty gloss must be reviewed (`gloss_updates`). Connections follow placement IDs and never
/// change; no Card and no Link is written.
fn move_station(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (moving, host) = station_arg(conn, ctx.args, "placement_id")?;
    let Host::Connection(tree) = &host else {
        return move_attachment(ctx, moving, host);
    };
    ensure_host_writable(conn, ctx.access, &host)?;
    if moving.edge == EdgeKind::Root {
        bail!("the root station does not move");
    }
    let shape = shape_of(conn, tree)?;
    let destination = ctx.args.get("destination").cloned().unwrap_or_else(|| ctx.args.clone());
    let slot = slot_arg(conn, &shape, &destination)?;
    // The moving branch: the station and its Side branches (not its Direct continuation).
    let mut branch: BTreeSet<String> = BTreeSet::from([moving.placement_id.clone()]);
    for side in shape.side.get(&moving.placement_id).cloned().unwrap_or_default() {
        branch.extend(shape.subtree(&side));
    }
    if branch.contains(&slot.parent) {
        bail!("a station cannot move under itself or its own Side branches");
    }
    let old_parent = moving.parent_id.clone().context("non-root station without parent")?;
    let continuation = shape.direct.get(&moving.placement_id).cloned();
    if slot.parent == moving.placement_id {
        bail!("a station cannot move under itself");
    }
    // The stations whose structural predecessor changes.
    let mut affected = Vec::new();
    let same_slot = slot.parent == old_parent
        && slot.edge == moving.edge
        && (slot.edge == EdgeKind::Direct || slot.before.is_none() && shape.side.get(&old_parent).and_then(|s| s.last()) == Some(&moving.placement_id));
    if same_slot {
        return Ok(json!({"applied": true, "placement_id": moving.placement_id, "coord": shape.coord.get(&moving.placement_id), "changed": false}));
    }
    if slot.parent != old_parent {
        affected.push(moving.placement_id.clone());
    }
    if let Some(c) = &continuation {
        affected.push(c.clone());
    }
    let displaced = match slot.edge {
        EdgeKind::Direct => shape.direct.get(&slot.parent).cloned().filter(|d| d != &moving.placement_id),
        _ => None,
    };
    // Moving to the Direct slot right after its own continuation's new place is fine: the
    // continuation is counted once.
    if let Some(d) = &displaced
        && !affected.contains(d)
    {
        affected.push(d.clone());
    }
    let (review, decided, missing) = gloss_review(&shape, &affected, ctx.args)?;
    let mut lines = vec![format!(
        "move {} ({}) to {} of {}",
        shape.coord[&moving.placement_id],
        moving.placement_id,
        slot.edge.id(),
        shape.coord[&slot.parent]
    )];
    for r in &review {
        lines.push(format!(
            "review gloss of {}: {}",
            r["coord"].as_str().unwrap_or_default(),
            r["gloss"].as_str().unwrap_or_default()
        ));
    }
    let incident = incident_edges(conn, &tree.tree_id, &branch)?;
    lines.push(format!("{} tree connection(s) follow their stations unchanged", incident.len()));
    let effects = json!({"moved": branch, "gloss_review": review, "connections_unchanged": incident.iter().map(|e| &e.connection_id).collect::<Vec<_>>()});
    if let Some(plan) = ctx.confirmed("move", &lines, &effects)? {
        return Ok(plan);
    }
    if !missing.is_empty() {
        bail!("gloss review required: pass gloss_updates (a string keeps or replaces, null clears) for {}", missing.join(", "));
    }
    // 1. Detach the moving station: its continuation takes its old slot.
    let mut park_ids = vec![moving.placement_id.clone()];
    if let Some(c) = &continuation {
        park_ids.push(c.clone());
    }
    if let Some(d) = &displaced
        && !park_ids.contains(d)
    {
        park_ids.push(d.clone());
    }
    park(conn, &shape, &park_ids)?;
    if let Some(c) = &continuation {
        if moving.edge == EdgeKind::Side {
            set_slot(conn, c, Some(&old_parent), EdgeKind::Side, moving.ordinal)?;
        } else {
            set_slot(conn, c, Some(&old_parent), EdgeKind::Direct, None)?;
        }
    }
    // The moving station's own Direct slot is now free (its continuation left).
    let shape_now = shape_of_partial(conn, tree)?;
    // 2. Put it at the destination.
    match slot.edge {
        EdgeKind::Direct => {
            let current = shape_now.direct.get(&slot.parent).cloned();
            set_slot(conn, &moving.placement_id, Some(&slot.parent), EdgeKind::Direct, None)?;
            if let Some(d) = displaced.or(current).filter(|d| d != &moving.placement_id) {
                set_slot(conn, &d, Some(&moving.placement_id), EdgeKind::Direct, None)?;
            }
        }
        EdgeKind::Side => {
            let mut siblings: Vec<String> = shape_now
                .side
                .get(&slot.parent)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|s| s != &moving.placement_id)
                .collect();
            let position = match &slot.before {
                Some(b) => siblings.iter().position(|s| s == b).context("`before` is not a sibling")?,
                None => siblings.len(),
            };
            siblings.insert(position, moving.placement_id.clone());
            park(conn, &shape_now, &siblings)?;
            for (i, id) in siblings.iter().enumerate() {
                set_slot(conn, id, Some(&slot.parent), EdgeKind::Side, Some(i as u64 + 1))?;
            }
        }
        _ => bail!("unsupported destination"),
    }
    if moving.edge == EdgeKind::Side && continuation.is_none() {
        compact_sides(conn, &tree.tree_id, &old_parent)?;
    }
    apply_glosses(conn, &decided)?;
    let shape = validate_tree(conn, tree)?;
    ctx.changed = true;
    Ok(json!({
        "applied": true,
        "placement_id": moving.placement_id,
        "coord": shape.coord.get(&moving.placement_id),
        "coordinates": shape.order.iter().map(|id| json!({"placement_id": id, "coord": shape.coord[id]})).collect::<Vec<_>>(),
        "gloss_reviewed": decided.keys().collect::<Vec<_>>(),
        "connections_unchanged": incident.len(),
        "changed": true,
    }))
}

/// A tree shape read in the middle of a rearrangement, without the reachability check (parked
/// stations hang off the root until they are placed).
fn shape_of_partial(conn: &Connection, tree: &ConnTree) -> Result<TreeShape> {
    let placements = coord::placements_of_host(conn, &tree.tree_id)?;
    let mut by_id = BTreeMap::new();
    let mut direct = BTreeMap::new();
    let mut side: BTreeMap<String, Vec<(u64, String)>> = BTreeMap::new();
    let mut root = String::new();
    for p in placements {
        match p.edge {
            EdgeKind::Root => root = p.placement_id.clone(),
            EdgeKind::Direct => {
                direct.insert(p.parent_id.clone().unwrap_or_default(), p.placement_id.clone());
            }
            EdgeKind::Side => side
                .entry(p.parent_id.clone().unwrap_or_default())
                .or_default()
                .push((p.ordinal.unwrap_or(0), p.placement_id.clone())),
            EdgeKind::Attach => {}
        }
        by_id.insert(p.placement_id.clone(), p);
    }
    let side = side
        .into_iter()
        .map(|(k, mut v)| {
            v.sort();
            (k, v.into_iter().map(|(_, id)| id).collect())
        })
        .collect();
    Ok(TreeShape {
        tree: tree.clone(),
        by_id,
        root,
        direct,
        side,
        coord: BTreeMap::new(),
        by_coord: BTreeMap::new(),
        order: Vec::new(),
    })
}

/// Reorder or re-anchor one Topic-side mount (within its Topic).
fn move_attachment(ctx: &mut Ctx, moving: Placement, host: Host) -> Result<Value> {
    let conn = ctx.conn;
    ensure_host_writable(conn, ctx.access, &host)?;
    let destination = ctx.args.get("destination").cloned().unwrap_or_else(|| ctx.args.clone());
    let anchor_at = opt_str(&destination, "anchor_at")?.context("a Topic-side mount moves to `anchor_at` (a Card of the same Topic)")?;
    let anchor = crate::load_card(conn, anchor_at)?;
    if TreeRole::of(&anchor) != TreeRole::Topic || crate::tree_id(&anchor.address) != host.key() {
        bail!("a Topic-side mount moves only beside Cards of its own Topic {}", host.key());
    }
    let anchor_id = coord::card_id_of_row(conn, anchor.row_id)?;
    let old_anchor = moving.anchor_card_id.clone().unwrap_or_default();
    let lines = vec![format!("move mount {} beside {}", coord::coordinate_of(conn, &moving)?, anchor.address)];
    if let Some(plan) = ctx.confirmed("move", &lines, &json!({"moved": [moving.placement_id]}))? {
        return Ok(plan);
    }
    let mut siblings: Vec<Placement> = coord::attachments_of_anchor(conn, &anchor_id)?
        .into_iter()
        .filter(|s| s.placement_id != moving.placement_id)
        .collect();
    let position = match opt_str(&destination, "before")? {
        Some(b) => {
            let sibling = coord::resolve_station(conn, b)?;
            siblings.iter().position(|s| s.placement_id == sibling.placement_id).context("`before` names a mount beside the destination Card")?
        }
        None => siblings.len(),
    };
    conn.execute(
        "UPDATE placements SET anchor_card_id = ?2, ordinal = ?3 WHERE placement_id = ?1",
        params![moving.placement_id, anchor_id, 9_000_000_i64],
    )?;
    siblings.insert(position, moving.clone());
    for (i, s) in siblings.iter().enumerate() {
        conn.execute("UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1", params![s.placement_id, 8_000_000 + i as i64])?;
    }
    for (i, s) in siblings.iter().enumerate() {
        conn.execute("UPDATE placements SET ordinal = ?2 WHERE placement_id = ?1", params![s.placement_id, i as i64 + 1])?;
    }
    if old_anchor != anchor_id {
        compact_attachments(conn, &old_anchor)?;
    }
    ctx.changed = true;
    let moved = coord::placement(conn, &moving.placement_id)?.context("moved mount")?;
    Ok(json!({"applied": true, "placement_id": moving.placement_id, "coord": coord::coordinate_of(conn, &moved)?, "changed": true}))
}

// ---------------------------------------------------------------------------------------------
// Unplace (detach) a station
// ---------------------------------------------------------------------------------------------

fn unplace(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (station, host) = station_arg(conn, ctx.args, "placement_id")?;
    ensure_host_writable(conn, ctx.access, &host)?;
    let revision = coord::next_revision(conn)?;
    let Host::Connection(tree) = &host else {
        // A Topic-side mount is a leaf: removing it never deletes the mounted Card.
        let lines = vec![format!(
            "detach mount {} ({}) from Topic {}; the Card stays at its home",
            coord::coordinate_of(conn, &station).unwrap_or_default(),
            station.placement_id,
            host.key()
        )];
        if let Some(plan) = ctx.confirmed("unplace", &lines, &json!({"removed": [station.placement_id]}))? {
            return Ok(plan);
        }
        retire_placements(conn, std::slice::from_ref(&station.placement_id), revision)?;
        conn.execute("DELETE FROM placements WHERE placement_id = ?1", [&station.placement_id])?;
        if let Some(anchor) = &station.anchor_card_id {
            compact_attachments(conn, anchor)?;
        }
        ctx.changed = true;
        return Ok(json!({"applied": true, "removed": [station.placement_id], "changed": true}));
    };
    if station.edge == EdgeKind::Root {
        bail!("the root station is the tree itself; delete the tree's root Card to remove the tree");
    }
    let shape = shape_of(conn, tree)?;
    let home_here = |p: &Placement| -> Result<bool> {
        Ok(match coord::address_of_card_id(conn, &p.card_id)? {
            Some(address) => Handle::parse(&address).is_some_and(|h| h.tree == tree.display_num),
            None => false,
        })
    };
    if home_here(&station)? {
        bail!(
            "station {} is the home of native Card `{}`; delete the Card or promote it instead of unplacing its home station",
            shape.coord[&station.placement_id],
            coord::address_of_card_id(conn, &station.card_id)?.unwrap_or_default()
        );
    }
    let policy = opt_str(ctx.args, "children")?;
    let continuation = shape.direct.get(&station.placement_id).cloned();
    let sides = shape.side.get(&station.placement_id).cloned().unwrap_or_default();
    let has_children = continuation.is_some() || !sides.is_empty();
    let mut removed: Vec<String> = vec![station.placement_id.clone()];
    let mut reattach_to: Option<String> = None;
    let mut affected = Vec::new();
    match (has_children, policy) {
        (false, _) => {}
        (true, Some("detach")) => {
            removed = shape.subtree(&station.placement_id);
            for id in &removed {
                if home_here(&shape.by_id[id])? {
                    bail!(
                        "detaching this branch would remove the home station of native Card `{}`; reattach the children or move that Card first",
                        coord::address_of_card_id(conn, &shape.by_id[id].card_id)?.unwrap_or_default()
                    );
                }
            }
        }
        (true, Some("reattach")) => {
            if let Some(target) = opt_str(ctx.args, "reattach_to")? {
                let target = station_in(conn, &shape, target)?;
                if shape.subtree(&station.placement_id).contains(&target.placement_id) {
                    bail!("`reattach_to` must be outside the detached station's branch");
                }
                if target.edge == EdgeKind::Root {
                    bail!("the root takes no Side successor; reattach to a top-level station");
                }
                reattach_to = Some(target.placement_id);
            }
            affected.extend(continuation.iter().cloned());
            affected.extend(sides.iter().cloned());
        }
        (true, None) => bail!("station {} has successors: pass children: \"reattach\" (they take its place) or \"detach\" (the whole branch is removed)", shape.coord[&station.placement_id]),
        (true, Some(other)) => bail!("unknown children policy `{other}`; expected reattach or detach"),
    }
    let removed_set: BTreeSet<String> = removed.iter().cloned().collect();
    let incident = incident_edges(conn, &tree.tree_id, &removed_set)?;
    let (review, decided, missing) = gloss_review(&shape, &affected, ctx.args)?;
    let mut lines = vec![format!(
        "detach {} station(s) starting at {}; the mounted Cards stay at their homes",
        removed.len(),
        shape.coord[&station.placement_id]
    )];
    for id in &removed {
        if let Some(g) = &shape.by_id[id].gloss {
            lines.push(format!("gloss lost with {}: {g}", shape.coord[id]));
        }
    }
    for e in &incident {
        lines.push(format!("tree connection {} is deleted with its endpoint", e.connection_id));
    }
    for r in &review {
        lines.push(format!("review gloss of {}: {}", r["coord"].as_str().unwrap_or_default(), r["gloss"].as_str().unwrap_or_default()));
    }
    let effects = json!({
        "removed": removed,
        "connections_deleted": incident.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "gloss_review": review,
    });
    if let Some(plan) = ctx.confirmed("unplace", &lines, &effects)? {
        return Ok(plan);
    }
    if !missing.is_empty() {
        bail!("gloss review required: pass gloss_updates for {}", missing.join(", "));
    }
    for e in &incident {
        conn.execute("DELETE FROM placement_edges WHERE connection_id = ?1", [&e.connection_id])?;
    }
    if has_children && policy == Some("reattach") {
        let old_parent = station.parent_id.clone().context("non-root station without parent")?;
        let mut kids: Vec<String> = Vec::new();
        kids.extend(continuation.iter().cloned());
        kids.extend(sides.iter().cloned());
        park(conn, &shape, &kids)?;
        conn.execute("DELETE FROM placements WHERE placement_id = ?1", [&station.placement_id])
            .ok();
        // Re-insert the station temporarily is unnecessary: it is removed below.
        match &reattach_to {
            Some(target) => {
                let existing = shape.side.get(target).map(|s| s.len()).unwrap_or(0);
                for (i, kid) in kids.iter().enumerate() {
                    set_slot(conn, kid, Some(target), EdgeKind::Side, Some((existing + i + 1) as u64))?;
                }
            }
            None => {
                // The Direct continuation takes the station's slot; the Side branches follow it.
                let (first, rest): (String, Vec<String>) = match &continuation {
                    Some(c) => (c.clone(), sides.clone()),
                    None => {
                        if station.edge == EdgeKind::Direct && shape.by_id[&old_parent].edge == EdgeKind::Root {
                            bail!("the root takes no Side successor: pass reattach_to (a station that should hold these Side branches) or children: \"detach\"");
                        }
                        (String::new(), sides.clone())
                    }
                };
                if !first.is_empty() {
                    set_slot(conn, &first, Some(&old_parent), station.edge, station.ordinal)?;
                    let existing = shape.side.get(&first).map(|s| s.len()).unwrap_or(0);
                    for (i, kid) in rest.iter().enumerate() {
                        set_slot(conn, kid, Some(&first), EdgeKind::Side, Some((existing + i + 1) as u64))?;
                    }
                } else {
                    // No continuation: the Side branches take the station's place among its parent's
                    // Side successors (or follow the parent's Side list for a Direct station).
                    let existing = shape.side.get(&old_parent).cloned().unwrap_or_default();
                    let mut order: Vec<String> = Vec::new();
                    for s in existing {
                        if s == station.placement_id {
                            order.extend(rest.iter().cloned());
                        } else {
                            order.push(s);
                        }
                    }
                    if station.edge == EdgeKind::Direct {
                        order.extend(rest.iter().cloned());
                    }
                    park(conn, &shape, &order)?;
                    for (i, kid) in order.iter().enumerate() {
                        set_slot(conn, kid, Some(&old_parent), EdgeKind::Side, Some(i as u64 + 1))?;
                    }
                }
            }
        }
        removed = vec![station.placement_id.clone()];
        retire_placements_known(conn, &station, revision)?;
        if station.edge == EdgeKind::Side {
            compact_sides(conn, &tree.tree_id, &old_parent)?;
        }
    } else {
        retire_placements(conn, &removed, revision)?;
        // Delete leaves first so no remaining station points at a removed parent mid-statement.
        for id in removed.iter().rev() {
            conn.execute("DELETE FROM placements WHERE placement_id = ?1", [id])?;
        }
        if let Some(parent) = &station.parent_id
            && station.edge == EdgeKind::Side
        {
            compact_sides(conn, &tree.tree_id, parent)?;
        }
    }
    apply_glosses(conn, &decided)?;
    validate_tree(conn, tree)?;
    ctx.changed = true;
    Ok(json!({
        "applied": true,
        "removed": removed,
        "connections_deleted": incident.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "changed": true,
    }))
}

fn retire_placements_known(conn: &Connection, station: &Placement, revision: i64) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO retired_placements(placement_id, host_tree_key, card_id, retired_at_rev) VALUES(?1, ?2, ?3, ?4)",
        params![station.placement_id, station.host_tree_key, station.card_id, revision],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Rebind a Missing mount
// ---------------------------------------------------------------------------------------------

/// Replace the Card of a Missing mount with a live Card. The station gets a fresh placement ID (the
/// old bookmark reports removed); every incident connection needs an explicit disposition
/// (`keep` recreates it on the new station with a fresh ID, `delete` drops it), because an old
/// assertion never transfers silently to replacement evidence.
fn rebind(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (station, host) = station_arg(conn, ctx.args, "placement_id")?;
    ensure_host_writable(conn, ctx.access, &host)?;
    let identity = coord::identity(conn, &station.card_id)?.context("unknown Card")?;
    if identity.live {
        bail!("station {} holds a live Card; only a Missing mount is rebound", station.placement_id);
    }
    let (card, card_id) = target_card(conn, ctx.args, "target_at")?;
    let revision = coord::next_revision(conn)?;
    let mut lines = vec![format!(
        "rebind missing station (was `{}`: {}) to `{}`; it gets a new placement ID",
        identity.last_address, identity.last_title, card.address
    )];
    match &host {
        Host::Connection(tree) => {
            let shape = shape_of(conn, tree)?;
            if shape.placement_of_card(&card_id).is_some() {
                bail!("`{}` already has a station in {}", card.address, tree.handle());
            }
            let ids = BTreeSet::from([station.placement_id.clone()]);
            let incident = incident_edges(conn, &tree.tree_id, &ids)?;
            let dispositions = ctx.args.get("connection_dispositions").and_then(Value::as_object).cloned().unwrap_or_default();
            let mut undecided = Vec::new();
            for e in &incident {
                match dispositions.get(&e.connection_id).and_then(Value::as_str) {
                    Some("keep") | Some("delete") => {}
                    _ => undecided.push(e.connection_id.clone()),
                }
                lines.push(format!("decide connection {}: keep (moves to the new Card) or delete", e.connection_id));
            }
            let mut affected = vec![station.placement_id.clone()];
            affected.extend(shape.direct.get(&station.placement_id).cloned());
            affected.extend(shape.side.get(&station.placement_id).cloned().unwrap_or_default());
            let (review, decided, missing) = gloss_review(&shape, &affected, ctx.args)?;
            for r in &review {
                lines.push(format!("review gloss of {}: {}", r["coord"].as_str().unwrap_or_default(), r["gloss"].as_str().unwrap_or_default()));
            }
            let effects = json!({"connections": incident.iter().map(|e| &e.connection_id).collect::<Vec<_>>(), "gloss_review": review, "card_id": card_id});
            if let Some(plan) = ctx.confirmed("rebind", &lines, &effects)? {
                return Ok(plan);
            }
            if !undecided.is_empty() {
                bail!("every incident connection needs a disposition: pass connection_dispositions {{\"<connection_id>\": \"keep\" | \"delete\"}} for {}", undecided.join(", "));
            }
            if !missing.is_empty() {
                bail!("gloss review required: pass gloss_updates for {}", missing.join(", "));
            }
            let new_id = coord::new_uuid()?;
            // Insert the new station in a parked slot, move children over, then take the old slot.
            let kids: Vec<String> = shape
                .direct
                .get(&station.placement_id)
                .cloned()
                .into_iter()
                .chain(shape.side.get(&station.placement_id).cloned().unwrap_or_default())
                .collect();
            insert_placement(
                conn,
                &Placement {
                    placement_id: new_id.clone(),
                    host_kind: "connection".into(),
                    host_tree_key: tree.tree_id.clone(),
                    card_id: card_id.clone(),
                    parent_id: Some(shape.root.clone()),
                    anchor_card_id: None,
                    edge: EdgeKind::Side,
                    ordinal: Some(9_000_000),
                    gloss: station.gloss.clone(),
                },
            )?;
            for kid in &kids {
                let p = &shape.by_id[kid];
                conn.execute(
                    "UPDATE placements SET parent_id = ?2 WHERE placement_id = ?1",
                    params![kid, new_id],
                )?;
                let _ = p;
            }
            for e in &incident {
                let keep = dispositions.get(&e.connection_id).and_then(Value::as_str) == Some("keep");
                conn.execute("DELETE FROM placement_edges WHERE connection_id = ?1", [&e.connection_id])?;
                if keep {
                    let mut moved = e.clone();
                    moved.connection_id = coord::new_uuid()?;
                    if moved.from == station.placement_id {
                        moved.from = new_id.clone();
                    }
                    if moved.to == station.placement_id {
                        moved.to = new_id.clone();
                    }
                    moved.updated_by = author(ctx.access).to_string();
                    moved.updated_at_rev = revision;
                    normalize_edge(&mut moved)?;
                    insert_edge(conn, &moved)?;
                }
            }
            retire_placements_known(conn, &station, revision)?;
            conn.execute("DELETE FROM placements WHERE placement_id = ?1", [&station.placement_id])?;
            set_slot(conn, &new_id, station.parent_id.as_deref(), station.edge, station.ordinal)?;
            let mut decided = decided;
            if let Some(g) = decided.remove(&station.placement_id) {
                decided.insert(new_id.clone(), g);
            }
            apply_glosses(conn, &decided)?;
            let shape = validate_tree(conn, tree)?;
            ctx.changed = true;
            Ok(json!({"applied": true, "placement_id": new_id, "replaces": station.placement_id, "coord": shape.coord.get(&new_id), "card_id": card_id, "changed": true}))
        }
        Host::Topic { topic, .. } => {
            if coord::placements_of_host(conn, topic)?.iter().any(|p| p.card_id == card_id) {
                bail!("`{}` is already mounted in Topic {topic}", card.address);
            }
            if let Some(plan) = ctx.confirmed("rebind", &lines, &json!({"card_id": card_id}))? {
                return Ok(plan);
            }
            let new_id = coord::new_uuid()?;
            retire_placements_known(conn, &station, revision)?;
            conn.execute("DELETE FROM placements WHERE placement_id = ?1", [&station.placement_id])?;
            let mut p = station.clone();
            p.placement_id = new_id.clone();
            p.card_id = card_id.clone();
            insert_placement(conn, &p)?;
            ctx.changed = true;
            Ok(json!({"applied": true, "placement_id": new_id, "replaces": station.placement_id, "card_id": card_id, "changed": true}))
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Connections
// ---------------------------------------------------------------------------------------------

fn connect(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let tree = coord::resolve_tree(conn, arg_str(ctx.args, "tree")?)?;
    let host = Host::Connection(tree.clone());
    ensure_host_writable(conn, ctx.access, &host)?;
    let shape = shape_of(conn, &tree)?;
    let from = station_in(conn, &shape, arg_str(ctx.args, "from_placement_id").or_else(|_| arg_str(ctx.args, "from"))?)?;
    let to = station_in(conn, &shape, arg_str(ctx.args, "to_placement_id").or_else(|_| arg_str(ctx.args, "to"))?)?;
    let directed = match opt_str(ctx.args, "direction")?.unwrap_or("neutral") {
        "neutral" => false,
        "directed" => true,
        other => bail!("unknown direction `{other}`; expected neutral or directed"),
    };
    let revision = coord::next_revision(conn)?;
    let mut edge = Edge {
        connection_id: coord::new_uuid()?,
        tree_id: tree.tree_id.clone(),
        from: from.placement_id,
        to: to.placement_id,
        directed,
        label: opt_str(ctx.args, "relation_label")?.map(|l| plain_line(l, "relation_label", coord::LABEL_MAX)).transpose()?,
        kind: opt_str(ctx.args, "relation_kind")?.map(str::to_string),
        note: opt_str(ctx.args, "note")?.map(plain_note).transpose()?,
        created_by: author(ctx.access).to_string(),
        created_at_rev: revision,
        updated_by: author(ctx.access).to_string(),
        updated_at_rev: revision,
    };
    normalize_edge(&mut edge)?;
    if is_blank(&edge)
        && let Some(existing) = edges_of_tree(conn, &tree.tree_id)?
            .into_iter()
            .find(|e| is_blank(e) && e.from == edge.from && e.to == edge.to)
    {
        // One blank quick-connect per pair: connecting again answers the existing one.
        let mut value = edge_value(conn, &shape, &existing)?;
        value["already_connected"] = json!(true);
        value["changed"] = json!(false);
        return Ok(value);
    }
    insert_edge(conn, &edge)?;
    ctx.changed = true;
    let mut value = edge_value(conn, &shape, &edge)?;
    value["changed"] = json!(true);
    Ok(value)
}

fn edit_connection(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let id = arg_str(ctx.args, "connection_id")?;
    let mut edge = edge_by_id(conn, id)?.with_context(|| format!("connection `{id}` does not exist"))?;
    let tree = coord::tree_by_id(conn, &edge.tree_id)?.context("connection of a deleted tree")?;
    ensure_host_writable(conn, ctx.access, &Host::Connection(tree.clone()))?;
    if let Some(base) = ctx.args.get("base_updated_at_rev").and_then(Value::as_i64)
        && base != edge.updated_at_rev
    {
        bail!("stale connection: it changed at revision {} after it was read", edge.updated_at_rev);
    }
    let before = edge.clone();
    match opt_str(ctx.args, "direction")? {
        Some("neutral") => edge.directed = false,
        Some("directed") => edge.directed = true,
        Some(other) => bail!("unknown direction `{other}`; expected neutral or directed"),
        None => {}
    }
    match patch(ctx.args, "relation_label")? {
        Patch::Keep => {}
        Patch::Clear => edge.label = None,
        Patch::Set(l) => edge.label = Some(plain_line(l, "relation_label", coord::LABEL_MAX)?),
    }
    match patch(ctx.args, "relation_kind")? {
        Patch::Keep => {}
        Patch::Clear => edge.kind = None,
        Patch::Set(k) => edge.kind = Some(k.to_string()),
    }
    match patch(ctx.args, "note")? {
        Patch::Keep => {}
        Patch::Clear => edge.note = None,
        Patch::Set(n) => edge.note = Some(plain_note(n)?),
    }
    if ctx.args.get("reverse_endpoints").and_then(Value::as_bool) == Some(true) {
        if !edge.directed {
            bail!("a neutral connection has no direction to reverse");
        }
        std::mem::swap(&mut edge.from, &mut edge.to);
    }
    normalize_edge(&mut edge)?;
    let shape = shape_of(conn, &tree)?;
    let same = edge.directed == before.directed && edge.from == before.from && edge.to == before.to && edge.label == before.label && edge.kind == before.kind && edge.note == before.note;
    if same {
        let mut value = edge_value(conn, &shape, &edge)?;
        value["changed"] = json!(false);
        return Ok(value);
    }
    if is_blank(&edge)
        && edges_of_tree(conn, &tree.tree_id)?
            .iter()
            .any(|e| e.connection_id != edge.connection_id && is_blank(e) && e.from == edge.from && e.to == edge.to)
    {
        bail!("conflict: this pair already has a blank connection; delete this one instead of clearing it");
    }
    edge.updated_by = author(ctx.access).to_string();
    edge.updated_at_rev = coord::next_revision(conn)?;
    conn.execute(
        "UPDATE placement_edges SET from_placement_id = ?2, to_placement_id = ?3, direction = ?4, relation_label = ?5,
             relation_kind = ?6, note = ?7, updated_by = ?8, updated_at_rev = ?9 WHERE connection_id = ?1",
        params![
            edge.connection_id,
            edge.from,
            edge.to,
            if edge.directed { "directed" } else { "neutral" },
            edge.label,
            edge.kind,
            edge.note,
            edge.updated_by,
            edge.updated_at_rev
        ],
    )?;
    ctx.changed = true;
    let mut value = edge_value(conn, &shape, &edge)?;
    value["changed"] = json!(true);
    Ok(value)
}

fn delete_connection(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let id = arg_str(ctx.args, "connection_id")?;
    let edge = edge_by_id(conn, id)?.with_context(|| format!("connection `{id}` does not exist"))?;
    let tree = coord::tree_by_id(conn, &edge.tree_id)?.context("connection of a deleted tree")?;
    ensure_host_writable(conn, ctx.access, &Host::Connection(tree.clone()))?;
    let shape = shape_of(conn, &tree)?;
    let value = edge_value(conn, &shape, &edge)?;
    let lines = vec![format!("delete tree connection: {}", value["sentence"].as_str().unwrap_or_default())];
    if let Some(plan) = ctx.confirmed("delete", &lines, &json!({"connection": value}))? {
        return Ok(plan);
    }
    conn.execute("DELETE FROM placement_edges WHERE connection_id = ?1", [id])?;
    ctx.changed = true;
    Ok(json!({"applied": true, "deleted": id, "changed": true}))
}

// ---------------------------------------------------------------------------------------------
// Extract a branch (copy into a new Connection tree)
// ---------------------------------------------------------------------------------------------

fn extract(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let source = coord::resolve_tree(conn, arg_str(ctx.args, "source_tree")?)?;
    let shape = shape_of(conn, &source)?;
    let top = station_in(conn, &shape, arg_str(ctx.args, "root_placement_id")?)?;
    if top.edge == EdgeKind::Root {
        bail!("extract a branch below the root; the root is the tree itself");
    }
    let selection = shape.subtree(&top.placement_id);
    let selected: BTreeSet<String> = selection.iter().cloned().collect();
    let edges = edges_of_tree(conn, &source.tree_id)?;
    let internal: Vec<&Edge> = edges.iter().filter(|e| selected.contains(&e.from) && selected.contains(&e.to)).collect();
    let crossing: Vec<&Edge> = edges.iter().filter(|e| selected.contains(&e.from) != selected.contains(&e.to)).collect();
    let workspace = match opt_str(ctx.args, "workspace")? {
        Some(w) => Workspace::parse(w)?,
        None => source.workspace,
    };
    ctx.access.ensure_writable(workspace)?;
    let title = arg_str(ctx.args, "title")?;
    let mut lines = vec![format!(
        "copy {} station(s) from {} (starting at {}) into a new Connection tree \"{title}\"; the original tree is unchanged",
        selection.len(),
        source.handle(),
        shape.coord[&top.placement_id]
    )];
    lines.push(format!("{} internal connection(s) are copied with fresh IDs", internal.len()));
    for e in &crossing {
        lines.push(format!("boundary connection {} stays in {} only", e.connection_id, source.handle()));
    }
    let effects = json!({
        "stations": selection,
        "internal": internal.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "boundary": crossing.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
    });
    if let Some(plan) = ctx.confirmed("extract", &lines, &effects)? {
        return Ok(plan);
    }
    let created = create_tree_tx(conn, ctx.access, workspace, title, "", opt_str(ctx.args, "purpose")?)?;
    let new_tree = coord::resolve_tree(conn, created["tree"].as_str().unwrap_or_default())?;
    let new_root = created["root_placement_id"].as_str().unwrap_or_default().to_string();
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for id in &selection {
        map.insert(id.clone(), coord::new_uuid()?);
    }
    // Insert in outline order so every parent exists first.
    for id in &selection {
        let p = &shape.by_id[id];
        let (parent, edge, ordinal) = if id == &top.placement_id {
            (Some(new_root.clone()), EdgeKind::Direct, None)
        } else {
            (p.parent_id.as_ref().map(|parent| map[parent].clone()), p.edge, p.ordinal)
        };
        insert_placement(
            conn,
            &Placement {
                placement_id: map[id].clone(),
                host_kind: "connection".into(),
                host_tree_key: new_tree.tree_id.clone(),
                card_id: p.card_id.clone(),
                parent_id: parent,
                anchor_card_id: None,
                edge,
                ordinal,
                gloss: if id == &top.placement_id { None } else { p.gloss.clone() },
            },
        )?;
    }
    let revision = coord::next_revision(conn)?;
    for e in internal {
        let mut copy = e.clone();
        copy.connection_id = coord::new_uuid()?;
        copy.tree_id = new_tree.tree_id.clone();
        copy.from = map[&e.from].clone();
        copy.to = map[&e.to].clone();
        copy.updated_by = author(ctx.access).to_string();
        copy.updated_at_rev = revision;
        normalize_edge(&mut copy)?;
        insert_edge(conn, &copy)?;
    }
    validate_tree(conn, &new_tree)?;
    ctx.changed = true;
    Ok(json!({
        "applied": true,
        "tree": new_tree.handle(),
        "tree_id": new_tree.tree_id,
        "stations": map,
        "boundary_connections": crossing.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "changed": true,
    }))
}

// ---------------------------------------------------------------------------------------------
// Promote a native Card into a Topic tree (move_home)
// ---------------------------------------------------------------------------------------------

fn promote(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (card, card_id) = target_card(conn, ctx.args, "at")?;
    let handle = Handle::parse(&card.address).filter(|h| h.local.is_some()).with_context(|| {
        format!("`{}` is not a native Card of a Connection tree; only `~n#k` Cards are promoted", card.address)
    })?;
    let tree = coord::tree_by_num(conn, handle.tree)?.context("the Card's Connection tree no longer exists")?;
    ctx.access.ensure_writable(card.workspace)?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
    crate::coord_lock::ensure_card_not_pinned(conn, &card_id, "pinned-move")?;
    let to = arg_str(ctx.args, "to").or_else(|_| arg_str(ctx.args, "target"))?;
    if !crate::is_valid_regular_location(to) || TreeRole::of_address(to) != TreeRole::Topic {
        bail!("promote to a free Regular Location of a Topic tree, such as `3/6` or `3/2|b`");
    }
    if let Some(topic) = opt_str(ctx.args, "destination_topic")?
        && crate::tree_id(to) != topic.trim_end_matches("/0")
    {
        bail!("`{to}` is not in Topic {topic}");
    }
    if crate::location_exists(conn, to)? {
        bail!("`{to}` is taken; promote to a free Location");
    }
    let parent = crate::parent_location(to).context("no parent Location")?;
    if !crate::location_exists(conn, &parent)? {
        bail!("`{to}` needs its predecessor `{parent}` to exist");
    }
    let topic_root = crate::tree_root_address(crate::tree_id(to));
    let destination_workspace = crate::workspace::tree_workspace(conn, &topic_root)?;
    if destination_workspace != card.workspace {
        bail!("promotion stays inside one Workspace: `{}` is {} and Topic {} is {}", card.address, card.workspace, crate::tree_id(to), destination_workspace);
    }
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::CreateAt(to))?;
    let topic = crate::tree_id(to).to_string();
    let attached = coord::placements_of_host(conn, &topic)?.into_iter().find(|p| p.card_id == card_id);
    let mut lines = vec![
        format!("move the home of `{}` from Connection tree {} to Topic Location `{to}`", card.address, tree.handle()),
        format!("`[[{}]]` keeps resolving to the same Card; its station in {} and its connections stay", card.address, tree.handle()),
    ];
    if let Some(a) = &attached {
        lines.push(format!("the Topic-side mount {} of this Card is removed (a Topic tree holds a Card once)", coord::coordinate_of(conn, a)?));
        if opt_str(ctx.args, "duplicate_resolution")? != Some("detach") {
            bail!("conflict: Topic {topic} already mounts this Card at {}; pass duplicate_resolution: \"detach\" to remove that mount in the same promotion", coord::coordinate_of(conn, a)?);
        }
    }
    let effects = json!({"card_id": card_id, "from": card.address, "to": to});
    if let Some(plan) = ctx.confirmed("promote", &lines, &effects)? {
        return Ok(plan);
    }
    // The gate checks the Card in its final Topic context.
    let candidate = crate::gate::Candidate {
        address: to.to_string(),
        ..crate::gate::Candidate::edit(&card, &card.text, card.card_type, card.facet.clone(), card.emphasis, card.level, card.hub_level, card.form)
    };
    let accepted = crate::gate::check_proposed(conn, &candidate)?;
    if let Some(a) = attached {
        retire_placements_known(conn, &a, coord::next_revision(conn)?)?;
        conn.execute("DELETE FROM placements WHERE placement_id = ?1", [&a.placement_id])?;
        if let Some(anchor) = &a.anchor_card_id {
            compact_attachments(conn, anchor)?;
        }
    }
    conn.execute("UPDATE cards SET location = ?2 WHERE row_id = ?1", params![card.row_id, to])?;
    // The Card is re-keyed as a Link source; authored targets that spell `~7#4` stay as written.
    crate::link_index::source_rekeyed(conn, &card.address, to)?;
    validate_tree(conn, &tree)?;
    ctx.changed = true;
    let mut value = json!({
        "applied": true,
        "card_id": card_id,
        "address": to,
        "handle": card.address,
        "tree": tree.handle(),
        "changed": true,
    });
    accepted.attach(&mut value);
    Ok(value)
}

// ---------------------------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------------------------

fn read(conn: &Connection, access: Access, tool: &str, args: &Value) -> Result<Value> {
    match tool {
        "list_connection_trees" => list_trees(conn, access, args),
        "get_connection_tree" => get_tree(conn, access, args),
        "list_placements" => list_placements(conn, access, args),
        "list_connections" => list_connections(conn, args),
        "get_dependencies" => crate::dependencies::get(conn, args),
        other => bail!("unknown Connection tree read `{other}`"),
    }
}

pub(crate) fn tree_summary(conn: &Connection, access: Access, tree: &ConnTree) -> Result<Value> {
    let title = coord::address_of_card_id(conn, &tree.root_card_id)?
        .and_then(|address| crate::load_card(conn, &address).ok())
        .and_then(|card| crate::parse_card_text(&card.text).ok())
        .map(|p| p.title)
        .unwrap_or_default();
    let (stations, native): (i64, i64) = conn.query_row(
        "SELECT COUNT(*), COALESCE(SUM(CASE WHEN c.location = ?2 OR c.location LIKE ?3 THEN 1 ELSE 0 END), 0)
         FROM placements p
         LEFT JOIN card_identity i ON i.card_id = p.card_id AND i.status = 'live'
         LEFT JOIN cards c ON c.row_id = i.card_row
         WHERE p.host_tree_key = ?1 AND p.edge <> 'root'",
        params![tree.tree_id, tree.handle(), format!("~{}#%", tree.display_num)],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let missing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM placements p JOIN card_identity i ON i.card_id = p.card_id
         WHERE p.host_tree_key = ?1 AND i.status = 'retired'",
        [&tree.tree_id],
        |r| r.get(0),
    )?;
    let connections: i64 = conn.query_row(
        "SELECT COUNT(*) FROM placement_edges WHERE tree_id = ?1",
        [&tree.tree_id],
        |r| r.get(0),
    )?;
    let host = Host::Connection(tree.clone());
    Ok(json!({
        "tree": tree.handle(),
        "tree_id": tree.tree_id,
        "title": title,
        "workspace": tree.workspace.id(),
        "purpose": tree.purpose,
        "root_card_id": tree.root_card_id,
        "mounted": stations - native,
        "native": native,
        "missing": missing,
        "connections": connections,
        "locked": crate::coord_lock::host_locked(conn, &tree.tree_id)?,
        "can_edit_tree": can_edit_host(conn, access, &host)?,
        "created_at": tree.created_at,
        "counts": format!("{} mounted · {} native · {} connections", stations - native, native, connections),
    }))
}

fn list_trees(conn: &Connection, access: Access, args: &Value) -> Result<Value> {
    let workspace = match opt_str(args, "workspace")? {
        None | Some("all") => None,
        Some(w) => Some(Workspace::parse(w)?),
    };
    let purpose = opt_str(args, "purpose")?;
    let filter = opt_str(args, "query")?.map(str::to_lowercase);
    let mut out = Vec::new();
    for tree in coord::trees(conn)? {
        if workspace.is_some_and(|w| w != tree.workspace) {
            continue;
        }
        if let Some(p) = purpose
            && tree.purpose.as_deref() != Some(p)
        {
            continue;
        }
        let summary = tree_summary(conn, access, &tree)?;
        if let Some(q) = &filter {
            let hay = format!("{} {}", summary["title"].as_str().unwrap_or_default(), summary["purpose"].as_str().unwrap_or_default()).to_lowercase();
            if !hay.contains(q.as_str()) {
                continue;
            }
        }
        out.push(summary);
    }
    Ok(json!({"trees": out, "archive_revision": crate::link_index::revision(conn)?}))
}

/// One Connection tree: its root, its stations in outline order (bounded by `limit`/`cursor`), and
/// its connections. A mounted tree root is one portal station; nothing of its home tree is read.
pub(crate) fn get_tree(conn: &Connection, access: Access, args: &Value) -> Result<Value> {
    let tree = coord::resolve_tree(conn, arg_str(args, "tree")?)?;
    let shape = shape_of(conn, &tree)?;
    let host = Host::Connection(tree.clone());
    let editable = can_edit_host(conn, access, &host)?;
    let revision = crate::link_index::revision(conn)?;
    let limit = args.get("limit").and_then(Value::as_u64).unwrap_or(500).clamp(1, 5000) as usize;
    let start = match opt_str(args, "cursor")? {
        Some(cursor) => {
            let (rev, offset) = cursor.split_once(':').context("invalid cursor")?;
            if rev.parse::<u64>().ok() != Some(revision) {
                bail!("stale cursor: the tree changed since the first page; read it again from the start");
            }
            offset.parse::<usize>().context("invalid cursor")?
        }
        None => 0,
    };
    let mut stations = Vec::new();
    for id in shape.order.iter().skip(start).take(limit) {
        stations.push(station_value(conn, access, &host, editable, &shape.by_id[id], &shape.coord[id])?);
    }
    let next = (start + limit < shape.order.len()).then(|| format!("{revision}:{}", start + limit));
    let mut value = tree_summary(conn, access, &tree)?;
    value["root_placement_id"] = json!(shape.root);
    value["placements"] = json!(stations);
    value["total_placements"] = json!(shape.order.len());
    value["next_cursor"] = json!(next);
    if args.get("include_connections").and_then(Value::as_bool).unwrap_or(true) {
        value["connections"] = json!(edges_of_tree(conn, &tree.tree_id)?
            .iter()
            .map(|e| edge_value(conn, &shape, e))
            .collect::<Result<Vec<_>>>()?);
    }
    value["archive_revision"] = json!(revision);
    Ok(value)
}

/// Every tree a Card stands in: its home and each station (Connection trees and Topic-side mounts).
pub(crate) fn list_placements(conn: &Connection, access: Access, args: &Value) -> Result<Value> {
    let card_id = match opt_str(args, "card_id")? {
        Some(id) => id.to_string(),
        None => {
            let at = arg_str(args, "at")?;
            if coord::is_coordinate(at) || coord::place_bookmark(at).is_some() {
                coord::resolve_station(conn, at)?.card_id
            } else {
                coord::card_id_of(conn, at)?
            }
        }
    };
    let identity = coord::identity(conn, &card_id)?.with_context(|| format!("Card `{card_id}` does not exist"))?;
    let mut stations = Vec::new();
    for p in coord::placements_of_card(conn, &card_id)? {
        let host = host_of(conn, &p)?;
        let editable = can_edit_host(conn, access, &host)?;
        let coord_text = coord::coordinate_of(conn, &p)?;
        stations.push(station_value(conn, access, &host, editable, &p, &coord_text)?);
    }
    let handle: Option<String> = conn
        .query_row("SELECT handle FROM card_handles WHERE card_id = ?1", [&card_id], |r| r.get(0))
        .optional()?;
    Ok(json!({
        "card_id": card_id,
        "status": if identity.live { "live" } else { "retired" },
        "home": if identity.live { coord::address_of_card_id(conn, &card_id)? } else { None },
        "home_role": if identity.live { coord::address_of_card_id(conn, &card_id)?.map(|a| TreeRole::of_address(&a).id()) } else { None },
        "last_address": identity.last_address,
        "title": identity.last_title,
        "handle": handle,
        "placements": stations,
        "archive_revision": crate::link_index::revision(conn)?,
    }))
}

/// The connections of one Connection tree, or those of one station (incoming, outgoing, neutral).
pub(crate) fn list_connections(conn: &Connection, args: &Value) -> Result<Value> {
    let tree = coord::resolve_tree(conn, arg_str(args, "tree")?)?;
    let shape = shape_of(conn, &tree)?;
    let focus = match opt_str(args, "placement_id")? {
        Some(p) => Some(station_in(conn, &shape, p)?.placement_id),
        None => None,
    };
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    let mut neutral = Vec::new();
    let mut all = Vec::new();
    for e in edges_of_tree(conn, &tree.tree_id)? {
        let value = edge_value(conn, &shape, &e)?;
        match &focus {
            None => all.push(value),
            Some(f) if &e.from != f && &e.to != f => {}
            Some(f) => {
                if !e.directed {
                    neutral.push(value);
                } else if &e.from == f {
                    outgoing.push(value);
                } else {
                    incoming.push(value);
                }
            }
        }
    }
    let mut value = json!({"tree": tree.handle(), "archive_revision": crate::link_index::revision(conn)?});
    match focus {
        Some(f) => {
            value["placement_id"] = json!(f);
            value["incoming"] = json!(incoming);
            value["outgoing"] = json!(outgoing);
            value["neutral"] = json!(neutral);
            // Labels already used in this tree, for suggestion.
        }
        None => value["connections"] = json!(all),
    }
    let labels: BTreeSet<String> = edges_of_tree(conn, &tree.tree_id)?.into_iter().filter_map(|e| e.label).collect();
    value["labels"] = json!(labels);
    Ok(value)
}

/// The station context of a Card read through a coordinate: added to `get_card` answers.
pub(crate) fn station_context(conn: &Connection, access: Access, station: &Placement) -> Result<Value> {
    let host = host_of(conn, station)?;
    let editable = can_edit_host(conn, access, &host)?;
    let coord_text = coord::coordinate_of(conn, station)?;
    let mut value = station_value(conn, access, &host, editable, station, &coord_text)?;
    if let Host::Connection(tree) = &host {
        let shape = shape_of(conn, tree)?;
        let parent = station.parent_id.as_ref().and_then(|p| shape.by_id.get(p));
        value["parent"] = json!(parent.map(|p| json!({"placement_id": p.placement_id, "coord": shape.coord.get(&p.placement_id)})));
        let mut incoming = Vec::new();
        let mut outgoing = Vec::new();
        let mut neutral = Vec::new();
        for e in edges_of_tree(conn, &tree.tree_id)? {
            if e.from != station.placement_id && e.to != station.placement_id {
                continue;
            }
            let v = edge_value(conn, &shape, &e)?;
            if !e.directed {
                neutral.push(v);
            } else if e.from == station.placement_id {
                outgoing.push(v);
            } else {
                incoming.push(v);
            }
        }
        value["connections"] = json!({"incoming": incoming, "outgoing": outgoing, "neutral": neutral});
    }
    Ok(value)
}

/// Add `Map` helper so callers can merge objects.
pub(crate) fn merge(target: &mut Value, extra: Value) {
    if let (Some(t), Value::Object(e)) = (target.as_object_mut(), extra) {
        let e: Map<String, Value> = e;
        for (k, v) in e {
            t.insert(k, v);
        }
    }
}
