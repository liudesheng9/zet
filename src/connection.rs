//! Coordinate operations, tree-local connections, and projections shared by MCP, GUI, and Session.
//! Writes run under the archive edit lock in one transaction. Host authority covers Coordinates,
//! while owner authority covers Cards. Destructive operations bind their plans to the revision.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::coord::{self, ConnTree, EdgeKind, Handle, Placement, TreeRecord, TreeRole, TreeShape};
use crate::{Access, CardType, Workspace};

/// The MCP and batch-free write tools of this module.
pub(crate) const WRITE_TOOLS: [&str; 22] = [
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
    "add_coordinate",
    "add_coordinates",
    "move_coordinate",
    "reorder_coordinates",
    "detach_coordinate",
    "refile_card",
    "rebind_coordinate",
];

/// The read tools of this module.
pub(crate) const READ_TOOLS: [&str; 8] = [
    "list_side_mounts",
    "list_connection_trees",
    "get_connection_tree",
    "list_placements",
    "list_connections",
    "get_dependencies",
    "list_coordinates",
    "get_tree_coordinates",
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
        bail!(
            "`{name}` has {} characters; at most {max} are allowed",
            value.chars().count()
        );
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
        bail!(
            "`note` is plain text and creates no Link; develop the assertion into a Card to cite Cards"
        );
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
        for key in [
            "confirm",
            "plan_hash",
            "expected_revision",
            "op_key",
            "preview",
        ] {
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
    if !is_write_tool(tool) && tool != "delete_card" {
        // One read transaction: every row of the answer belongs to one archive revision.
        let conn = crate::open_db(root)?;
        let view = conn.unchecked_transaction()?;
        let value = read(&view, access, tool, args)?;
        view.rollback()?;
        return Ok(value);
    }
    if tool == "delete_card" {
        arg_str(args, "card_id")?;
        if args.get("at").is_some_and(|at| !at.is_null()) {
            bail!(
                "give either `at` for an explicit subtree delete or `card_id` for a single Card delete"
            );
        }
        if args
            .get("confirm")
            .is_some_and(|confirm| !confirm.is_null())
        {
            arg_str(args, "confirm")?;
            if opt_str(args, "plan_hash")?.is_none() {
                bail!("confirming a Card delete requires the plan_hash; ask for the plan first");
            }
        }
    }
    let _lock =
        crate::acquire_edit_lock_within(root, crate::ops::EDIT_PATIENCE, "connection tree")?;
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
            if tool == "delete_card"
                && args["confirm"].as_str().is_some()
                && value["plan_hash"].as_str() != args["plan_hash"].as_str()
            {
                bail!("plan_hash does not match the recorded Card delete");
            }
            value["replayed"] = json!(true);
            return Ok(value);
        }
    }
    let revision = crate::link_index::revision(&tx)?;
    if let Some(expected) = args.get("expected_revision") {
        let expected = expected
            .as_u64()
            .context("expected_revision must be an archive revision")?;
        if expected != revision {
            bail!(
                "stale plan: the archive is at revision {revision}, expected {expected}; ask for a new plan"
            );
        }
    }
    let mut ctx = Ctx {
        conn: &tx,
        access,
        tool,
        args,
        revision,
        changed: false,
    };
    let value = write(&mut ctx)?;
    let applied = value
        .get("applied")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    // `preview: true` asks what would happen and never writes, also for a write that would
    // otherwise apply at once.
    if args.get("preview").and_then(Value::as_bool) == Some(true) && applied {
        drop(tx);
        let mut value = value;
        value["applied"] = json!(false);
        value["preview"] = json!(true);
        return Ok(value);
    }
    if !applied {
        // A plan writes nothing; the transaction rolls back.
        drop(tx);
        return Ok(value);
    }
    let changed = ctx.changed;
    if changed && crate::link_index::revision(&tx)? == revision {
        crate::link_index::bump_revision(&tx)?;
    }
    let mut value = value;
    value["archive_revision"] = json!(crate::link_index::revision(&tx)?);
    // Every applied keyed request keeps its receipt, also a no-op (an already-placed Card, an
    // existing blank pair): a retry after later changes replays this answer instead of acting again.
    if let Some(key) = &op_key {
        tx.execute(
            "INSERT INTO applied_ops(op_key, request_hash, result_json, committed_at)
             VALUES(?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ','now'))",
            params![key, request_hash, value.to_string()],
        )?;
    }
    tx.commit()?;
    Ok(value)
}

/// Delete the named Card through the shared Coordinate transaction and receipt path.
pub(crate) fn delete_card_identity(
    root: &Path,
    access: crate::ops::Access,
    args: &Value,
) -> Result<Value> {
    run(root, access, "delete_card", args)
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
        confirm_plan(
            self.conn,
            self.tool,
            self.args,
            self.revision,
            word,
            lines,
            effects,
        )
    }
}

fn write(ctx: &mut Ctx) -> Result<Value> {
    match ctx.tool {
        "create_connection_tree" => create_tree(ctx),
        "edit_connection_tree" => edit_tree(ctx),
        "place_card" => place(ctx),
        "create_connection_card" => create_native(ctx),
        "edit_placement" => edit_gloss(ctx),
        "move_placement" | "move_coordinate" => move_station(ctx),
        "unplace" | "detach_coordinate" => unplace(ctx),
        "rebind_placement" | "rebind_coordinate" => rebind(ctx),
        "add_coordinate" | "add_coordinates" => place(ctx),
        "reorder_coordinates" => reorder_coordinates(ctx),
        "refile_card" => refile(ctx),
        "delete_card" => delete_card(ctx),
        "connect_placements" => connect(ctx),
        "edit_connection" => edit_connection(ctx),
        "delete_connection" => delete_connection(ctx),
        "extract_branch" => extract(ctx),
        "promote_card" => promote(ctx),
        "seed_connection_tree" => crate::dependencies::seed(
            ctx.conn,
            ctx.access,
            ctx.args,
            &mut |word, lines, effects| {
                confirm_plan(
                    ctx.conn,
                    ctx.tool,
                    ctx.args,
                    ctx.revision,
                    word,
                    lines,
                    effects,
                )
            },
        )
        .map(|value| {
            if value
                .get("applied")
                .and_then(Value::as_bool)
                .unwrap_or(true)
            {
                ctx.changed = true;
            }
            value
        }),
        "review_source" => {
            crate::coord_lock::review_source(ctx.conn, ctx.access, ctx.args).map(|value| {
                ctx.changed = true;
                value
            })
        }
        other => bail!("unknown Connection tree tool `{other}`"),
    }
}

/// Plan-or-apply for every destructive write (also seeding, in `dependencies`): without the
/// confirmation word the answer is the plan and nothing is written. A confirmation must carry the
/// plan's word, its `expected_revision` and its `plan_hash`; the hash binds the archive identity,
/// the revision, the whole request (with every decision such as `gloss_updates`) and the resolved
/// effects, so a changed archive or a changed request is refused as stale.
pub(crate) fn confirm_plan(
    conn: &Connection,
    tool: &str,
    args: &Value,
    revision: u64,
    word: &str,
    lines: &[String],
    effects: &Value,
) -> Result<Option<Value>> {
    let plan_hash = hash_value(&json!({
        "archive": crate::tree_lock::archive_identity(conn)?,
        "request": request_core(tool, args),
        "revision": revision,
        "effects": effects,
    }));
    let confirm = args.get("confirm").and_then(Value::as_str);
    if confirm != Some(word) {
        if let Some(other) = confirm {
            bail!(
                "the confirmation word of this plan is `{word}`, not `{other}`; ask for the plan again"
            );
        }
        return Ok(Some(json!({
            "applied": false,
            "confirm": word,
            "lines": lines,
            "effects": effects,
            "plan_hash": plan_hash,
            "expected_revision": revision,
        })));
    }
    let expected = args.get("expected_revision").and_then(Value::as_u64);
    if expected.is_none() && tool != "delete_card" {
        bail!(
            "a confirmation carries the plan's expected_revision and plan_hash; ask for the plan first"
        );
    }
    if let Some(expected) = expected
        && expected != revision
    {
        bail!(
            "stale plan: the archive is at revision {revision} but the plan was made at revision {expected}; ask for a new plan"
        );
    }
    match args.get("plan_hash").and_then(Value::as_str) {
        Some(given) if given == plan_hash => Ok(None),
        Some(_) => bail!(
            "stale plan: the plan changed since it was shown (or the request changed); ask for a new plan"
        ),
        None => bail!(
            "a confirmation carries the plan's expected_revision and plan_hash; ask for the plan first"
        ),
    }
}

// ---------------------------------------------------------------------------------------------
// Host authority
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub(crate) struct Host(pub(crate) TreeRecord);

impl Host {
    pub(crate) fn key(&self) -> &str {
        &self.0.tree_id
    }

    pub(crate) fn workspace(&self) -> Workspace {
        self.0.workspace
    }

    pub(crate) fn label(&self) -> String {
        self.0.root_address()
    }
}

pub(crate) fn host_of(conn: &Connection, coordinate: &Placement) -> Result<Host> {
    Ok(Host(
        coord::tree_by_id(conn, &coordinate.tree_id)?
            .context("the coordinate's tree was deleted")?,
    ))
}

/// Whether `access` may change the placements and connections of `host` now: its Workspace, and no
/// active Lock on the host tree.
pub(crate) fn can_edit_host(conn: &Connection, access: Access, host: &Host) -> Result<bool> {
    Ok(access.can_write(host.workspace()) && !crate::coord_lock::host_locked(conn, host.key())?)
}

fn ensure_host_writable(conn: &Connection, access: Access, host: &Host) -> Result<()> {
    access.ensure_writable(host.workspace())?;
    crate::coord_lock::ensure_host_unlocked(conn, host.key(), &host.label())
}

/// Whether `access` may edit the Card's own text now: its home Workspace and home Lock.
pub(crate) fn can_edit_card(conn: &Connection, access: Access, card: &crate::Card) -> bool {
    access.can_write(card.workspace)
        && crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))
            .is_ok()
}

// ---------------------------------------------------------------------------------------------
// Station views
// ---------------------------------------------------------------------------------------------

/// The citable address of a Card: its permanent handle when it has one (a native Card keeps
/// `~7#4` across promotion), else its canonical home address.
pub(crate) fn citation_address(conn: &Connection, card_id: &str, address: &str) -> Result<String> {
    let handle: Option<String> = conn
        .query_row(
            "SELECT handle FROM card_handles WHERE card_id = ?1",
            [card_id],
            |r| r.get(0),
        )
        .optional()?;
    Ok(handle.unwrap_or_else(|| address.to_string()))
}

pub(crate) fn card_by_row(conn: &Connection, row: i64) -> Result<crate::Card> {
    let address: String = conn.query_row(
        "SELECT COALESCE(location, citation_key) FROM cards WHERE row_id = ?1",
        [row],
        |r| r.get(0),
    )?;
    crate::load_card(conn, &address)
}

fn alias<'a>(args: &'a Value, names: &[&str]) -> Result<Option<&'a str>> {
    let mut value = None;
    for name in names {
        if let Some(next) = opt_str(args, name)? {
            if value.is_some_and(|previous| previous != next) {
                bail!("give one of {}", names.join(", "));
            }
            value = Some(next);
        }
    }
    Ok(value)
}

fn required_alias<'a>(args: &'a Value, names: &[&str]) -> Result<&'a str> {
    alias(args, names)?.with_context(|| format!("give one of {}", names.join(", ")))
}

fn named_card_id(conn: &Connection, text: &str) -> Result<String> {
    if coord::is_uuid(text) && coord::identity(conn, text)?.is_some() {
        return Ok(text.to_string());
    }
    if !coord::is_coordinate(text)
        && coord::place_bookmark(text).is_none()
        && let Some(address) = coord::canonical(conn, text)?
    {
        return coord::card_id_of(conn, &address);
    }
    Ok(coord::resolve_coord(conn, text)?.card_id)
}

pub(crate) fn named_target_id(conn: &Connection, args: &Value, at_field: &str) -> Result<String> {
    if let Some(id) = opt_str(args, "card_id")? {
        Ok(id.to_string())
    } else if let Some(at) = opt_str(args, "target_coord_id")? {
        Ok(coord::resolve_coord(conn, at)?.card_id)
    } else {
        named_card_id(conn, arg_str(args, at_field)?)
    }
}

pub(crate) fn live_card(
    conn: &Connection,
    card_id: &str,
) -> Result<(crate::Card, coord::Identity)> {
    let identity = coord::identity(conn, card_id)?
        .with_context(|| format!("Card `{card_id}` does not exist"))?;
    let row = identity
        .card_row
        .filter(|_| identity.live)
        .with_context(|| {
            format!(
                "Card `{card_id}` was deleted (last at `{}`)",
                identity.last_address
            )
        })?;
    Ok((card_by_row(conn, row)?, identity))
}

fn station_in(conn: &Connection, shape: &TreeShape, text: &str) -> Result<Placement> {
    let row = match shape
        .by_coord
        .get(text)
        .or_else(|| shape.by_id.get(text).map(|p| &p.coord_id))
    {
        Some(id) => shape.by_id[id].clone(),
        None => coord::resolve_coord(conn, text)?,
    };
    if row.tree_id != shape.tree.tree_id {
        bail!("coordinate `{text}` is not in tree {}", shape.tree.handle());
    }
    Ok(row)
}

fn shape_of(conn: &Connection, tree: &TreeRecord) -> Result<TreeShape> {
    coord::load_tree_shape(conn, tree)
}

fn station_arg(conn: &Connection, args: &Value) -> Result<(Placement, Host)> {
    let row = coord::resolve_coord(
        conn,
        required_alias(args, &["coord_id", "placement_id", "coord"])?,
    )?;
    let host = host_of(conn, &row)?;
    Ok((row, host))
}

pub(crate) fn filing_row(conn: &Connection, identity: &coord::Identity) -> Result<Placement> {
    let id = identity
        .filing_coord_id
        .as_deref()
        .context("Card has no Filing Coordinate")?;
    let row = coord::coordinate(conn, id)?.context("Card's Filing Coordinate is missing")?;
    if row.card_id != identity.card_id || row.host_workspace != identity.live_owner_workspace()? {
        bail!("Card's Filing Coordinate disagrees with its identity or Workspace");
    }
    Ok(row)
}

fn check_admission(conn: &Connection, tree: &TreeRecord, identity: &coord::Identity) -> Result<()> {
    if !identity.live {
        bail!("Card `{}` was deleted", identity.last_address);
    }
    if tree.role == TreeRole::Literature && filing_row(conn, identity)?.tree_id != tree.tree_id {
        bail!(
            "MC3: Literature trees are closed to foreign Cards in v1; only repeated Coordinates of this book's own filed Cards are allowed"
        );
    }
    Ok(())
}

fn insert_placement(conn: &Connection, p: &Placement) -> Result<()> {
    conn.execute(
        "INSERT INTO coordinates(coord_id, tree_id, host_workspace, card_id, parent_coord_id, edge, slot, gloss)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![p.coord_id, p.tree_id, p.host_workspace.id(), p.card_id, p.parent_coord_id,
                p.edge.id(), p.slot, p.gloss],
    )?;
    Ok(())
}

fn set_position(
    conn: &Connection,
    id: &str,
    tree: &TreeRecord,
    parent: &str,
    edge: EdgeKind,
    slot: Option<u64>,
) -> Result<()> {
    conn.execute(
        "UPDATE coordinates SET tree_id = ?2, host_workspace = ?3, parent_coord_id = ?4,
                edge = ?5, slot = ?6 WHERE coord_id = ?1",
        params![
            id,
            tree.tree_id,
            tree.workspace.id(),
            parent,
            edge.id(),
            slot
        ],
    )?;
    Ok(())
}

fn set_parent(conn: &Connection, id: &str, parent: &str) -> Result<()> {
    conn.execute(
        "UPDATE coordinates SET parent_coord_id = ?2 WHERE coord_id = ?1",
        params![id, parent],
    )?;
    Ok(())
}

/// Free unique slots before a splice. Root Side slots are used only until the transaction
/// has assigned each parked Coordinate its final position.
fn park(conn: &Connection, shape: &TreeShape, ids: &[String]) -> Result<()> {
    let occupied: BTreeSet<_> = side_slots(conn, &shape.root)?
        .into_iter()
        .map(|(_, slot)| slot)
        .collect();
    let mut slot = 0;
    for id in ids {
        slot = add_slots(slot, 1)?;
        while occupied.contains(&slot) {
            slot = add_slots(slot, 1)?;
        }
        set_position(
            conn,
            id,
            &shape.tree,
            &shape.root,
            EdgeKind::Side,
            Some(slot),
        )?;
    }
    Ok(())
}

fn side_slots(conn: &Connection, parent: &str) -> Result<Vec<(String, u64)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT coord_id, slot FROM coordinates WHERE parent_coord_id = ?1 AND edge = 'side' ORDER BY slot",
    )?;
    let rows = stmt
        .query_map([parent], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn direct_child(conn: &Connection, parent: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT coord_id FROM coordinates WHERE parent_coord_id = ?1 AND edge = 'direct'",
            [parent],
            |r| r.get(0),
        )
        .optional()?)
}

fn add_slots(slot: u64, count: u64) -> Result<u64> {
    slot.checked_add(count)
        .filter(|&slot| slot <= i64::MAX as u64)
        .context("Side slots exhausted")
}

fn compact_sides(conn: &Connection, tree: &TreeRecord, parent: &str) -> Result<()> {
    if tree.role == TreeRole::Connection {
        for (i, (id, slot)) in side_slots(conn, parent)?.into_iter().enumerate() {
            let next = i as u64 + 1;
            if next != slot {
                set_position(conn, &id, tree, parent, EdgeKind::Side, Some(next))?;
            }
        }
    }
    Ok(())
}

struct Slot {
    parent: String,
    edge: EdgeKind,
    before: Option<String>,
    explicit: Option<u64>,
}

fn slot_arg(conn: &Connection, shape: &TreeShape, args: &Value) -> Result<Slot> {
    let parent = station_in(
        conn,
        shape,
        required_alias(
            args,
            &[
                "parent_coord_id",
                "parent_placement_id",
                "parent_coord",
                "parent",
            ],
        )?,
    )?;
    let edge = match opt_str(args, "edge")?.unwrap_or("direct") {
        "direct" => EdgeKind::Direct,
        "side" => EdgeKind::Side,
        other => bail!("unknown edge `{other}`; expected direct or side"),
    };
    if edge == EdgeKind::Side && parent.edge == EdgeKind::Root {
        bail!("the root takes no Side successor; use its Direct successor");
    }
    let before = alias(args, &["before_coord_id", "before"])?
        .map(|text| station_in(conn, shape, text))
        .transpose()?;
    if let Some(sibling) = &before
        && (edge != EdgeKind::Side
            || sibling.edge != EdgeKind::Side
            || sibling.parent_coord_id.as_deref() != Some(&parent.coord_id))
    {
        bail!("`before` names a Side successor of the same parent");
    }
    let explicit = match args.get("slot") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .filter(|&n| n > 0 && n <= i64::MAX as u64)
                .context("slot must be a positive integer")?,
        ),
    };
    if explicit.is_some() && (before.is_some() || edge != EdgeKind::Side) {
        bail!("an explicit slot is for a Side insertion without `before`");
    }
    Ok(Slot {
        parent: parent.coord_id,
        edge,
        before: before.map(|p| p.coord_id),
        explicit,
    })
}

/// Reserve consecutive Side slots, shifting only the siblings that conflict.
fn reserve_side_slots(
    conn: &Connection,
    tree: &TreeRecord,
    slot: &Slot,
    count: usize,
) -> Result<u64> {
    let siblings = side_slots(conn, &slot.parent)?;
    let count = count as u64;
    if let Some(start) = slot.explicit {
        let end = add_slots(start, count - 1)?;
        if siblings
            .iter()
            .any(|(_, current)| (start..=end).contains(current))
        {
            bail!("Side slot {start} or a later slot in this block is occupied");
        }
        return Ok(start);
    }
    let Some(before) = &slot.before else {
        let start = add_slots(siblings.last().map_or(0, |(_, n)| *n), 1)?;
        add_slots(start, count - 1)?;
        return Ok(start);
    };
    let position = siblings
        .iter()
        .position(|(id, _)| id == before)
        .context("`before` is not a Side sibling")?;
    let start = siblings[position].1;
    let mut next = add_slots(start, count)?;
    let mut shifts = Vec::new();
    for (id, old) in &siblings[position..] {
        if *old >= next {
            break;
        }
        shifts.push((id, next));
        next = next.saturating_add(1);
    }
    for (id, rank) in shifts.into_iter().rev() {
        set_position(
            conn,
            id,
            tree,
            &slot.parent,
            EdgeKind::Side,
            Some(add_slots(rank, 0)?),
        )?;
    }
    Ok(start)
}

fn gloss_review(
    shapes: &[&TreeShape],
    affected: &BTreeSet<String>,
    args: &Value,
) -> Result<(Vec<Value>, BTreeMap<String, Option<String>>, Vec<String>)> {
    let updates = match args.get("gloss_updates") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_object()
                .context("gloss_updates must be an object keyed by coord_id")?,
        ),
    };
    let mut review = Vec::new();
    let mut decisions = BTreeMap::new();
    let mut missing = Vec::new();
    for shape in shapes {
        for id in affected {
            if let Some(row) = shape.by_id.get(id)
                && let Some(gloss) = &row.gloss
            {
                review.push(json!({"coord_id": id, "placement_id": id, "coord": shape.coord[id], "gloss": gloss}));
                if updates.is_none_or(|updates| !updates.contains_key(id)) {
                    missing.push(id.clone());
                }
            }
        }
    }
    if let Some(updates) = updates {
        for (id, value) in updates {
            if !shapes.iter().any(|shape| shape.by_id.contains_key(id)) {
                bail!("gloss_updates names a Coordinate outside the affected trees: {id}");
            }
            let gloss = match value {
                Value::Null => None,
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => Some(plain_line(s, "gloss", coord::GLOSS_MAX)?),
                _ => bail!("gloss_updates values are strings (keep or replace) or null (clear)"),
            };
            decisions.insert(id.clone(), gloss);
        }
    }
    Ok((review, decisions, missing))
}

pub(crate) fn append_gloss_review(lines: &mut Vec<String>, review: &[Value]) {
    for item in review {
        lines.push(format!(
            "review gloss of {}: {}",
            item["coord"].as_str().unwrap_or_default(),
            item["gloss"].as_str().unwrap_or_default(),
        ));
    }
}

pub(crate) fn require_gloss_decisions(missing: &[String]) -> Result<()> {
    if !missing.is_empty() {
        bail!(
            "gloss review required: pass gloss_updates for {}",
            missing.join(", ")
        );
    }
    Ok(())
}

fn apply_glosses(conn: &Connection, decisions: &BTreeMap<String, Option<String>>) -> Result<()> {
    for (id, gloss) in decisions {
        conn.execute(
            "UPDATE coordinates SET gloss = ?2 WHERE coord_id = ?1 AND gloss IS NOT ?2",
            params![id, gloss],
        )?;
    }
    Ok(())
}

fn coordinate_result(p: &Placement, shape: &TreeShape, address: &str, is_filing: bool) -> Value {
    json!({
        "coord_id": p.coord_id, "placement_id": p.coord_id, "coord": shape.coord[&p.coord_id],
        "card_id": p.card_id, "home_address": address, "tree": shape.tree.handle(),
        "edge": p.edge.id(), "parent_coord_id": p.parent_coord_id, "slot": p.slot, "ordinal": p.slot,
        "is_filing": is_filing,
    })
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
// Tree connections (coordinate_edges)
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

const EDGE_COLUMNS: &str = "connection_id, tree_id, from_coord_id, to_coord_id, direction, relation_label, relation_kind, note, created_by, created_at_rev, updated_by, updated_at_rev";

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
        "SELECT {EDGE_COLUMNS} FROM coordinate_edges WHERE tree_id = ?1 ORDER BY created_at_rev, connection_id"
    ))?;
    let rows = stmt
        .query_map([tree_id], edge_from_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

pub(crate) fn edge_by_id(conn: &Connection, connection_id: &str) -> Result<Option<Edge>> {
    Ok(conn
        .query_row(
            &format!("SELECT {EDGE_COLUMNS} FROM coordinate_edges WHERE connection_id = ?1"),
            [connection_id],
            edge_from_row,
        )
        .optional()?)
}

fn insert_edge(conn: &Connection, e: &Edge) -> Result<()> {
    conn.execute(
        &format!("INSERT INTO coordinate_edges({EDGE_COLUMNS}) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)"),
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
            bail!(
                "relation_kind `{kind}` has a direction; set direction to `directed` and order the endpoints"
            );
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
        let p = shape
            .by_id
            .get(id)
            .context("connection endpoint is not a station of this tree")?;
        let identity = coord::identity(conn, &p.card_id)?.context("unknown Card")?;
        let title = match identity.card_row.filter(|_| identity.live) {
            Some(row) => {
                let card = card_by_row(conn, row)?;
                crate::parse_card_text(&card.text)
                    .map(|p| p.title)
                    .unwrap_or(identity.last_title.clone())
            }
            None => identity.last_title.clone(),
        };
        Ok(json!({
            "coord_id": id,
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
        "from_coord_id": e.from,
        "to_coord_id": e.to,
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
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_connection_id'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let next = next.and_then(|v| v.parse::<u64>().ok()).unwrap_or(1).max(1);
    // Never below a number already issued: a handle survives its tree.
    let issued: Option<i64> = conn.query_row(
        "SELECT MAX(CAST(substr(handle, 2) AS INTEGER)) FROM card_handles WHERE instr(handle, '#') = 0",
        [],
        |r| r.get(0),
    )?;
    let next = next.max(issued.map(|v| v as u64 + 1).unwrap_or(1));
    let after = next
        .checked_add(1)
        .context("Connection tree numbers exhausted")?;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('next_connection_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [after.to_string()],
    )?;
    Ok(next)
}

/// Create a Connection tree. The root is an ordinary Card with no special meaning
/// (docs/connection-trees-free-proposal.md D2): its title and body are optional. A root written
/// without a title is stored with its own handle as its title, because a Card title is never empty;
/// `root_title` reads such a root as untitled.
pub(crate) fn create_tree_tx(
    conn: &Connection,
    access: Access,
    workspace: Workspace,
    title: &str,
    body: &str,
    purpose: Option<&str>,
) -> Result<Value> {
    access.ensure_writable(workspace)?;
    let purpose = validate_purpose(purpose)?;
    let num = next_connection_number(conn)?;
    let handle = Handle::root(num);
    let title = match title.trim() {
        "" => handle.as_str(),
        given => given,
    };
    let text = crate::compose_card_text(title, body, "");
    let accepted = crate::gate::check_proposed(
        conn,
        &crate::gate::Candidate::create(
            &handle,
            crate::gate::Kind::Connection,
            workspace,
            &text,
            CardType::Idea,
            None,
        ),
    )?;
    let parsed = crate::parse_card_text(&text)?;
    let text = crate::compose_card_text(&parsed.title, &parsed.body, &parsed.reverse);
    conn.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
         VALUES(?1, NULL, 0, 0, NULL, ?2, ?3, 'idea')",
        params![handle, text, workspace.id()],
    )?;
    let row = conn.last_insert_rowid();
    let (card_id, tree_id, root_placement) = coord::register_root_card_tx(
        conn,
        row,
        TreeRole::Connection,
        &handle,
        Some(num),
        workspace,
        &parsed.title,
        purpose.as_deref(),
    )?;
    crate::link_index::sync_source(conn, &handle)?;
    let mut value = json!({
        "tree": handle,
        "tree_id": tree_id,
        "address": handle,
        "card_id": card_id,
        "root_coord_id": root_placement,
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
    let title = opt_str(ctx.args, "title")?.unwrap_or_default();
    let body = opt_str(ctx.args, "body")?.unwrap_or_default();
    let value = create_tree_tx(
        ctx.conn,
        ctx.access,
        workspace,
        title,
        body,
        opt_str(ctx.args, "purpose")?,
    )?;
    ctx.changed = true;
    Ok(value)
}

/// Change a Connection tree's shelf tag `purpose` (metadata only; the title and body are the root
/// Card's text, edited like any Card).
fn edit_tree(ctx: &mut Ctx) -> Result<Value> {
    let tree = coord::resolve_tree(ctx.conn, arg_str(ctx.args, "tree")?)?;
    let host = Host(tree.clone());
    ensure_host_writable(ctx.conn, ctx.access, &host)?;
    let purpose = match patch(ctx.args, "purpose")? {
        Patch::Keep => {
            return Ok(json!({"tree": tree.handle(), "purpose": tree.purpose, "changed": false}));
        }
        Patch::Clear => None,
        Patch::Set(p) => Some(plain_line(p, "purpose", coord::PURPOSE_MAX)?),
    };
    if purpose == tree.purpose {
        return Ok(json!({"tree": tree.handle(), "purpose": purpose, "changed": false}));
    }
    ctx.conn.execute(
        "UPDATE trees SET purpose = ?2 WHERE tree_id = ?1",
        params![tree.tree_id, purpose],
    )?;
    ctx.changed = true;
    Ok(json!({"tree": tree.handle(), "purpose": purpose, "changed": true}))
}

// ---------------------------------------------------------------------------------------------
// Place (mount) a Card
// ---------------------------------------------------------------------------------------------

fn refuse_legacy_attachment(args: &Value) -> Result<()> {
    if opt_str(args, "edge")? == Some("attach")
        || args.get("anchor_at").is_some_and(|at| !at.is_null())
    {
        bail!(
            "Topic-side mounts (+k / edge: attach) are retired under MC1; use add_coordinate to place the Card directly onto a Standard Direct or Side slot in the Topic tree"
        );
    }
    Ok(())
}

fn place(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let legacy = ctx.tool == "place_card";
    if legacy {
        refuse_legacy_attachment(ctx.args)?;
    }
    let host_name = required_alias(ctx.args, &["tree", "host"])?;
    let tree = if legacy {
        coord::resolve_tree(conn, host_name)?
    } else {
        coord::resolve_any_tree(conn, host_name)?
    };
    ensure_host_writable(conn, ctx.access, &Host(tree.clone()))?;
    let shape = shape_of(conn, &tree)?;
    let multiple = ctx.tool == "add_coordinates" || ctx.args.get("targets").is_some();
    if multiple && ctx.tool == "add_coordinate" {
        bail!("use add_coordinates to place a list of targets");
    }
    if legacy && multiple && ctx.args.get("gloss").is_some() {
        bail!(
            "a gloss belongs to one station: mount the Cards, then gloss each station with edit_placement"
        );
    }
    let gloss = opt_str(ctx.args, "gloss")?
        .map(|g| plain_line(g, "gloss", coord::GLOSS_MAX))
        .transpose()?;
    if multiple && gloss.is_some() {
        bail!("pass gloss only for a single Coordinate");
    }
    let targets = if multiple {
        if ["target_at", "target_coord_id", "card_id"]
            .iter()
            .any(|name| ctx.args.get(*name).is_some())
        {
            bail!("give either targets or one Card, not both");
        }
        let targets = ctx.args["targets"]
            .as_array()
            .context("targets must be an array of Card addresses")?;
        if targets.is_empty() || targets.len() > 500 {
            bail!("targets must contain 1 to 500 Cards");
        }
        targets
            .iter()
            .map(|target| {
                let target = target.as_str().context("each target must be a string")?;
                Ok((named_card_id(conn, target)?, Some(target)))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        vec![(named_target_id(conn, ctx.args, "target_at")?, None)]
    };
    let mut seen = BTreeSet::new();
    let mut already = Vec::new();
    let mut additions = Vec::new();
    for (id, target) in targets {
        let (card, identity) = live_card(conn, &id)?;
        check_admission(conn, &tree, &identity)?;
        if legacy && !seen.insert(id.clone()) {
            continue;
        }
        if legacy && let Some(row) = shape.coords_of_card(&id).first() {
            let mut result = coordinate_result(
                row,
                &shape,
                &card.address,
                identity.filing_coord_id.as_deref() == Some(&row.coord_id),
            );
            result["already_placed"] = json!(true);
            result["changed"] = json!(false);
            result["applied"] = json!(true);
            if !multiple {
                if opt_str(ctx.args, "confirm")?.is_some() {
                    bail!("stale plan: this Card is already placed; ask for a new plan");
                }
                return Ok(result);
            }
            result["target"] = json!(target);
            already.push(result);
        } else {
            additions.push((id, card.address));
        }
    }
    if additions.is_empty() {
        if opt_str(ctx.args, "confirm")?.is_some() {
            bail!("stale plan: these Cards are already placed; ask for a new plan");
        }
        return Ok(
            json!({"applied": true, "tree": tree.handle(), "changed": false,
            "added": [], "placed": [], "already_placed": already}),
        );
    }
    let slot = slot_arg(conn, &shape, ctx.args)?;
    coord::check_model_term_splice_guard(conn, &tree, &slot.parent, slot.edge)?;
    let displaced = (slot.edge == EdgeKind::Direct)
        .then(|| shape.direct.get(&slot.parent))
        .flatten();
    let affected = displaced.into_iter().cloned().collect();
    let (review, decisions, missing) = gloss_review(&[&shape], &affected, ctx.args)?;
    let ids = additions
        .iter()
        .map(|_| coord::new_uuid())
        .collect::<Result<Vec<_>>>()?;
    let mut parent = slot.parent.clone();
    let side_start = if slot.edge == EdgeKind::Side {
        Some(reserve_side_slots(conn, &tree, &slot, additions.len())?)
    } else {
        if let Some(displaced) = displaced {
            set_parent(conn, displaced, ids.last().context("empty splice")?)?;
        }
        None
    };
    for (i, (id, (card_id, _))) in ids.iter().zip(&additions).enumerate() {
        insert_placement(
            conn,
            &Placement {
                coord_id: id.clone(),
                tree_id: tree.tree_id.clone(),
                host_workspace: tree.workspace,
                card_id: card_id.clone(),
                parent_coord_id: Some(parent.clone()),
                edge: slot.edge,
                slot: side_start.map(|start| start + i as u64),
                gloss: gloss.clone(),
            },
        )?;
        if slot.edge == EdgeKind::Direct {
            parent.clone_from(id);
        }
    }
    let after = shape_of(conn, &tree)?;
    let mapping = filing_path_changes(conn, &[&shape], &[&after], &BTreeSet::new())?;
    check_filing_mapping(conn, ctx.access, &mapping)?;
    if !review.is_empty() || !mapping.is_empty() || opt_str(ctx.args, "confirm")?.is_some() {
        let mut lines = vec![format!(
            "place {} Coordinate(s) in {}",
            additions.len(),
            tree.handle()
        )];
        lines.extend(
            additions
                .iter()
                .map(|(_, address)| format!("place Card {address}")),
        );
        append_gloss_review(&mut lines, &review);
        append_filing_changes(&mut lines, &mapping);
        let effects = json!({"order": additions.iter().map(|(id, address)| json!([address, id])).collect::<Vec<_>>(),
            "displaced": displaced, "gloss_review": review, "filing_moves": mapping, "already_placed": already});
        if let Some(plan) = ctx.confirmed(if legacy { "place" } else { "add" }, &lines, &effects)? {
            return Ok(plan);
        }
    }
    require_gloss_decisions(&missing)?;
    apply_glosses(conn, &decisions)?;
    apply_filing_mapping(conn, &mapping)?;
    let added: Vec<_> = ids
        .iter()
        .zip(additions)
        .map(|(id, (_, address))| {
            coordinate_result(
                &after.by_id[id],
                &after,
                mapping.get(&address).unwrap_or(&address),
                false,
            )
        })
        .collect();
    ctx.changed = true;
    if multiple {
        return Ok(
            json!({"applied": true, "tree": tree.handle(), "changed": true,
            "added": added, "placed": added, "already_placed": already}),
        );
    }
    let mut value = added.into_iter().next().context("empty splice")?;
    value["applied"] = json!(true);
    value["changed"] = json!(true);
    value["already_placed"] = json!(false);
    Ok(value)
}

pub(crate) fn ensure_native_type(card_type: CardType) -> Result<()> {
    if !matches!(
        card_type,
        CardType::Idea | CardType::Event | CardType::Finding
    ) {
        bail!(
            "native_structure_required: Connection trees can author Idea, Event, and Finding Cards"
        );
    }
    Ok(())
}

fn allocate_native_handle(conn: &Connection, tree: &TreeRecord) -> Result<(u64, String)> {
    let num = tree
        .display_num
        .context("Connection tree is missing its display number")?;
    let local = tree.next_local_seq;
    let next = local
        .checked_add(1)
        .filter(|&n| n <= i64::MAX as u64)
        .context("Connection handles exhausted")?;
    conn.execute(
        "UPDATE trees SET next_local_seq = ?2 WHERE tree_id = ?1",
        params![tree.tree_id, next],
    )?;
    Ok((local, Handle::native(num, local)))
}

fn create_native(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let tree = coord::resolve_tree(conn, arg_str(ctx.args, "tree")?)?;
    ensure_host_writable(conn, ctx.access, &Host(tree.clone()))?;
    let card_type = opt_str(ctx.args, "type")?
        .map(CardType::parse)
        .transpose()?
        .unwrap_or(CardType::Idea);
    ensure_native_type(card_type)?;
    let title = opt_str(ctx.args, "title")?.unwrap_or_default();
    let body = opt_str(ctx.args, "body")?.unwrap_or_default();
    let shape = shape_of(conn, &tree)?;
    let slot = slot_arg(conn, &shape, ctx.args)?;
    let displaced = (slot.edge == EdgeKind::Direct)
        .then(|| shape.direct.get(&slot.parent))
        .flatten();
    let affected = displaced.into_iter().cloned().collect();
    let (_, decisions, missing) = gloss_review(&[&shape], &affected, ctx.args)?;
    require_gloss_decisions(&missing)?;
    let gloss = opt_str(ctx.args, "gloss")?
        .map(|g| plain_line(g, "gloss", coord::GLOSS_MAX))
        .transpose()?;
    let (local, handle) = allocate_native_handle(conn, &tree)?;
    let text = crate::compose_card_text(title, body, "");
    let accepted = crate::gate::check_proposed(
        conn,
        &crate::gate::Candidate::create(
            &handle,
            crate::gate::Kind::Regular,
            tree.workspace,
            &text,
            card_type,
            None,
        ),
    )?;
    let parsed = crate::parse_card_text(&text)?;
    let text = crate::compose_card_text(&parsed.title, &parsed.body, &parsed.reverse);
    conn.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
         VALUES(?1, NULL, 0, 0, NULL, ?2, ?3, ?4)",
        params![handle, text, tree.workspace.id(), card_type.id()],
    )?;
    let row = conn.last_insert_rowid();
    let coord_id = coord::new_uuid()?;
    let side = if slot.edge == EdgeKind::Side {
        Some(reserve_side_slots(conn, &tree, &slot, 1)?)
    } else {
        if let Some(displaced) = displaced {
            set_parent(conn, displaced, &coord_id)?;
        }
        None
    };
    let card_id = coord::register_connection_native_tx(
        conn,
        row,
        &tree,
        &handle,
        local,
        &parsed.title,
        &coord_id,
        &slot.parent,
        slot.edge,
        side,
        gloss.as_deref(),
    )?;
    apply_glosses(conn, &decisions)?;
    crate::link_index::sync_source(conn, &handle)?;
    let shape = shape_of(conn, &tree)?;
    ctx.changed = true;
    let mut value = coordinate_result(&shape.by_id[&coord_id], &shape, &handle, true);
    value["applied"] = json!(true);
    value["address"] = json!(handle);
    value["handle"] = json!(handle);
    value["type"] = json!(card_type.id());
    value["card_id"] = json!(card_id);
    value["changed"] = json!(true);
    accepted.attach(&mut value);
    Ok(value)
}

fn edit_gloss(ctx: &mut Ctx) -> Result<Value> {
    let (row, host) = station_arg(ctx.conn, ctx.args)?;
    ensure_host_writable(ctx.conn, ctx.access, &host)?;
    if row.edge == EdgeKind::Root {
        bail!(
            "the root Coordinate has no positional gloss; its framing text is the root Card's body"
        );
    }
    let gloss = match patch(ctx.args, "gloss")? {
        Patch::Keep => bail!("pass gloss (a string) or null to clear it"),
        Patch::Clear => None,
        Patch::Set(gloss) => Some(plain_line(gloss, "gloss", coord::GLOSS_MAX)?),
    };
    if let Some(base) = ctx.args.get("base_gloss")
        && base.as_str() != row.gloss.as_deref()
    {
        bail!("stale gloss: the Coordinate's gloss changed since it was read");
    }
    ctx.changed = gloss != row.gloss;
    if ctx.changed {
        ctx.conn.execute(
            "UPDATE coordinates SET gloss = ?2 WHERE coord_id = ?1",
            params![row.coord_id, gloss],
        )?;
    }
    Ok(
        json!({"coord_id": row.coord_id, "placement_id": row.coord_id, "gloss": gloss, "changed": ctx.changed}),
    )
}

fn children_of(shape: &TreeShape, id: &str) -> Vec<String> {
    shape
        .direct
        .get(id)
        .into_iter()
        .chain(shape.side.get(id).into_iter().flatten())
        .cloned()
        .collect()
}

fn append_side_children(
    conn: &Connection,
    tree: &TreeRecord,
    parent: &str,
    children: &[String],
) -> Result<()> {
    if children.is_empty() {
        return Ok(());
    }
    let mut next = side_slots(conn, parent)?
        .last()
        .map_or(0, |(_, slot)| *slot);
    for id in children {
        next = add_slots(next, 1)?;
        set_position(conn, id, tree, parent, EdgeKind::Side, Some(next))?;
    }
    Ok(())
}

/// A vacated slot's Direct continuation takes its place. Side children follow that
/// continuation, or join the old parent's Side branches when there is no continuation.
fn reattach_children(
    conn: &Connection,
    shape: &TreeShape,
    row: &Placement,
    target: Option<&str>,
    move_sides: bool,
) -> Result<()> {
    if let Some(target) = target {
        if shape.by_id[target].edge == EdgeKind::Root
            && !children_of(shape, &row.coord_id).is_empty()
        {
            bail!("the root takes no Side successor; pass a non-root reattach_to");
        }
        return append_side_children(
            conn,
            &shape.tree,
            target,
            &children_of(shape, &row.coord_id),
        );
    }
    let old_parent = row
        .parent_coord_id
        .as_deref()
        .context("non-root Coordinate has no parent")?;
    let next = shape.direct.get(&row.coord_id);
    if let Some(next) = next {
        set_position(conn, next, &shape.tree, old_parent, row.edge, row.slot)?;
    }
    if move_sides && let Some(children) = shape.side.get(&row.coord_id) {
        let parent = next.map(String::as_str).unwrap_or(old_parent);
        if shape.by_id[parent].edge == EdgeKind::Root && !children.is_empty() {
            bail!("the root takes no Side successor; pass reattach_to or choose the whole subtree");
        }
        if next.is_none() && row.edge == EdgeKind::Side && shape.tree.role != TreeRole::Connection {
            if let Some((first, rest)) = children.split_first() {
                set_position(conn, first, &shape.tree, parent, EdgeKind::Side, row.slot)?;
                append_side_children(conn, &shape.tree, parent, rest)?;
            }
        } else if next.is_none() && row.edge == EdgeKind::Side {
            let slot = Slot {
                parent: parent.to_string(),
                edge: EdgeKind::Side,
                before: shape
                    .side
                    .get(parent)
                    .into_iter()
                    .flatten()
                    .find(|&id| shape.by_id[id].slot > row.slot)
                    .cloned(),
                explicit: None,
            };
            let start = reserve_side_slots(conn, &shape.tree, &slot, children.len())?;
            for (i, id) in children.iter().enumerate() {
                set_position(
                    conn,
                    id,
                    &shape.tree,
                    parent,
                    EdgeKind::Side,
                    Some(start + i as u64),
                )?;
            }
        } else {
            append_side_children(conn, &shape.tree, parent, children)?;
        }
    }
    Ok(())
}

fn changed_parents(before: &[&TreeShape], after: &[&TreeShape]) -> BTreeSet<String> {
    let parents = after
        .iter()
        .flat_map(|shape| shape.by_id.values().map(|row| (row.coord_id.as_str(), row)))
        .collect::<BTreeMap<_, _>>();
    before
        .iter()
        .flat_map(|shape| shape.by_id.values())
        .filter(|old| {
            parents
                .get(old.coord_id.as_str())
                .is_some_and(|new| new.parent_coord_id != old.parent_coord_id)
        })
        .map(|row| row.coord_id.clone())
        .collect()
}

fn move_station(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (moving, host) = station_arg(conn, ctx.args)?;
    let source = &host.0;
    if ctx.tool == "move_placement" && source.role != TreeRole::Connection {
        bail!("move_placement is for Connection trees; use move_coordinate for other trees");
    }
    ensure_host_writable(conn, ctx.access, &host)?;
    if moving.edge == EdgeKind::Root {
        bail!("the defining root Coordinate does not move");
    }
    let from = shape_of(conn, source)?;
    let destination = ctx.args.get("destination").unwrap_or(ctx.args);
    let target = opt_str(destination, "tree")?
        .map(|at| coord::resolve_any_tree(conn, at))
        .transpose()?
        .unwrap_or_else(|| source.clone());
    let cross_tree = target.tree_id != source.tree_id;
    if ctx.tool == "move_placement" && target.role != TreeRole::Connection {
        bail!("move_placement is for Connection trees; use move_coordinate for other trees");
    }
    ensure_host_writable(conn, ctx.access, &Host(target.clone()))?;
    let to = if cross_tree {
        shape_of(conn, &target)?
    } else {
        from.clone()
    };
    let slot = slot_arg(conn, &to, destination)?;
    let mode = opt_str(ctx.args, "mode")?;
    let span_end = if mode == Some("span") {
        let end = station_in(
            conn,
            &from,
            required_alias(ctx.args, &["through_coord_id", "span_end_coord_id"])?,
        )?;
        if end.edge == EdgeKind::Root {
            bail!("a move span cannot include the defining root Coordinate");
        }
        Some(end.coord_id)
    } else {
        None
    };
    let branch = match mode {
        Some("subtree" | "alone" | "span") => false,
        None => ctx.tool == "move_placement",
        Some(other) => bail!("unknown move mode `{other}`; expected subtree, alone or span"),
    };
    let alone = mode == Some("alone");
    let mut moved = BTreeSet::from([moving.coord_id.clone()]);
    if let Some(tail) = span_end.as_ref().or(branch.then_some(&moving.coord_id)) {
        let mut current = &moving.coord_id;
        loop {
            moved.insert(current.clone());
            for child in from.side.get(current).into_iter().flatten() {
                moved.extend(from.subtree(child));
            }
            if current == tail {
                break;
            }
            current = from
                .direct
                .get(current)
                .context("through_coord_id must be on the Direct chain starting at coord_id")?;
        }
    } else if !alone {
        moved.extend(from.subtree(&moving.coord_id));
    }
    if moved.contains(&slot.parent)
        || !cross_tree
            && (span_end.is_some() || ctx.tool == "move_coordinate" && alone)
            && from.subtree(&moving.coord_id).contains(&slot.parent)
    {
        bail!("coordinate cycle: cannot move a Coordinate under itself or its descendant");
    }
    let old_parent = moving
        .parent_coord_id
        .as_deref()
        .context("non-root Coordinate has no parent")?;
    let same_slot = !cross_tree
        && slot.parent == old_parent
        && slot.edge == moving.edge
        && (slot.edge == EdgeKind::Direct
            || slot.explicit.is_some() && slot.explicit == moving.slot
            || slot.before.as_deref() == Some(&moving.coord_id)
            || slot.before.is_none()
                && slot.explicit.is_none()
                && from.side.get(old_parent).and_then(|ids| ids.last()) == Some(&moving.coord_id));
    if same_slot {
        if opt_str(ctx.args, "confirm")?.is_some() {
            bail!("stale plan: the Coordinate is already at that slot");
        }
        return Ok(
            json!({"applied": true, "coord_id": moving.coord_id, "placement_id": moving.coord_id,
            "coord": from.coord[&moving.coord_id], "changed": false}),
        );
    }
    if slot.before.as_ref().is_some_and(|id| moved.contains(id)) {
        bail!("`before` cannot name a moving Coordinate");
    }
    let mut moved_filings = Vec::new();
    if cross_tree || target.role == TreeRole::Literature {
        for id in &moved {
            let row = &from.by_id[id];
            let identity = coord::identity(conn, &row.card_id)?.context("unknown Card identity")?;
            if cross_tree && identity.filing_coord_id.as_deref() == Some(id.as_str()) {
                let card = live_card(conn, &row.card_id)?.0;
                let (parent, edge) = if id == &moving.coord_id {
                    (slot.parent.as_str(), slot.edge)
                } else {
                    (
                        row.parent_coord_id
                            .as_deref()
                            .context("a moved Coordinate needs a parent")?,
                        row.edge,
                    )
                };
                check_filing_destination(conn, ctx.access, &card, &target, parent, edge)?;
                check_literature_survivors(conn, &row.card_id, &target, &moved)?;
                moved_filings.push(card);
            } else if target.role == TreeRole::Literature {
                check_admission(conn, &target, &identity)?;
            }
        }
    }
    coord::check_model_term_splice_guard(conn, &target, &slot.parent, slot.edge)?;
    let incident = incident_edges(conn, &source.tree_id, &moved)?;
    let boundary: Vec<&Edge> = incident
        .iter()
        .filter(|e| cross_tree && moved.contains(&e.from) != moved.contains(&e.to))
        .collect();
    let internal: Vec<&Edge> = incident
        .iter()
        .filter(|e| !cross_tree || moved.contains(&e.from) && moved.contains(&e.to))
        .collect();

    park(conn, &from, std::slice::from_ref(&moving.coord_id))?;
    if branch || alone {
        reattach_children(conn, &from, &moving, None, alone)?;
    } else if let Some(after_through) = span_end.as_ref().and_then(|end| from.direct.get(end)) {
        set_position(
            conn,
            after_through,
            source,
            old_parent,
            moving.edge,
            moving.slot,
        )?;
    }
    if cross_tree {
        for id in &moved {
            if id != &moving.coord_id {
                let row = &from.by_id[id];
                set_position(
                    conn,
                    id,
                    &target,
                    row.parent_coord_id
                        .as_deref()
                        .context("missing branch parent")?,
                    row.edge,
                    row.slot,
                )?;
            }
        }
        for edge in &boundary {
            conn.execute(
                "DELETE FROM coordinate_edges WHERE connection_id = ?1",
                [&edge.connection_id],
            )?;
        }
        for edge in &internal {
            conn.execute(
                "UPDATE coordinate_edges SET tree_id = ?2 WHERE connection_id = ?1",
                params![edge.connection_id, target.tree_id],
            )?;
        }
    }
    if slot.edge == EdgeKind::Direct {
        let displaced = direct_child(conn, &slot.parent)?;
        if let Some(displaced) = displaced {
            if let Some(tail) = &span_end {
                set_parent(conn, &displaced, tail)?;
            } else {
                let mut tail = moving.coord_id.clone();
                while let Some(child) = direct_child(conn, &tail)? {
                    tail = child;
                }
                set_parent(conn, &displaced, &tail)?;
            }
        }
        set_position(
            conn,
            &moving.coord_id,
            &target,
            &slot.parent,
            EdgeKind::Direct,
            None,
        )?;
    } else {
        let rank = reserve_side_slots(conn, &target, &slot, 1)?;
        set_position(
            conn,
            &moving.coord_id,
            &target,
            &slot.parent,
            EdgeKind::Side,
            Some(rank),
        )?;
    }
    if moving.edge == EdgeKind::Side
        && (cross_tree || slot.parent != old_parent || slot.explicit.is_none())
    {
        compact_sides(conn, source, old_parent)?;
    }
    let after = shape_of(conn, &target)?;
    let source_after = if cross_tree {
        Some(shape_of(conn, source)?)
    } else {
        None
    };
    let mut before_shapes = vec![&from];
    let mut after_shapes = vec![&after];
    if let Some(source_after) = &source_after {
        before_shapes.push(&to);
        after_shapes.push(source_after);
    }
    let affected = changed_parents(&before_shapes, &after_shapes);
    let (review, decisions, missing) = gloss_review(&before_shapes, &affected, ctx.args)?;
    let mapping = filing_path_changes(conn, &before_shapes, &after_shapes, &BTreeSet::new())?;
    check_filing_mapping(conn, ctx.access, &mapping)?;
    let mut changes = crate::link_index::Changes::default();
    write_filing_mapping(conn, &mapping, &mut changes)?;
    for card in &moved_filings {
        check_refile_gate(conn, card)?;
    }
    let mut lines = vec![format!(
        "move {} to {}",
        from.coord[&moving.coord_id], after.coord[&moving.coord_id]
    )];
    append_gloss_review(&mut lines, &review);
    append_filing_changes(&mut lines, &mapping);
    if !boundary.is_empty() {
        lines.push(format!(
            "{} boundary connection(s) must be deleted; pass boundary_edges: delete to approve",
            boundary.len()
        ));
    }
    let effects = json!({"moved": moved, "gloss_review": review, "filing_moves": mapping,
        "connections_unchanged": internal.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "connections_deleted": boundary.iter().map(|e| &e.connection_id).collect::<Vec<_>>()});
    if let Some(plan) = ctx.confirmed("move", &lines, &effects)? {
        return Ok(plan);
    }
    if !boundary.is_empty() && opt_str(ctx.args, "boundary_edges")? != Some("delete") {
        bail!("cross-tree move has boundary connections; pass boundary_edges: delete");
    }
    require_gloss_decisions(&missing)?;
    apply_glosses(conn, &decisions)?;
    if !mapping.is_empty() {
        changes.finish(conn)?;
    }
    let identity = coord::identity(conn, &moving.card_id)?.context("unknown Card identity")?;
    let address = identity.last_address;
    let is_filing = identity.filing_coord_id.as_deref() == Some(moving.coord_id.as_str());
    let mut result = coordinate_result(&after.by_id[&moving.coord_id], &after, &address, is_filing);
    result["applied"] = json!(true);
    result["changed"] = json!(true);
    result["connections_unchanged"] = json!(internal.len());
    result["connections_deleted"] = effects["connections_deleted"].clone();
    result["gloss_reviewed"] = json!(decisions.keys().collect::<Vec<_>>());
    if ctx.tool == "move_placement" {
        result["coordinates"] = json!(
            after
                .order
                .iter()
                .map(|id| json!({
                    "coord_id": id, "placement_id": id, "coord": after.coord[id],
                }))
                .collect::<Vec<_>>()
        );
    }
    ctx.changed = true;
    Ok(result)
}

fn reorder_coordinates(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let tree = coord::resolve_any_tree(conn, arg_str(ctx.args, "tree")?)?;
    ensure_host_writable(conn, ctx.access, &Host(tree.clone()))?;
    let shape = shape_of(conn, &tree)?;
    let parent = station_in(
        conn,
        &shape,
        required_alias(ctx.args, &["parent_coord_id", "parent_coord", "parent"])?,
    )?;
    let order = ctx.args["order"]
        .as_array()
        .context("order must list the Side children")?
        .iter()
        .map(|id| {
            Ok(station_in(
                conn,
                &shape,
                id.as_str().context("order entries must name Coordinates")?,
            )?
            .coord_id)
        })
        .collect::<Result<Vec<String>>>()?;
    let siblings = shape
        .side
        .get(&parent.coord_id)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if order.len() != siblings.len()
        || order.iter().collect::<BTreeSet<_>>() != siblings.iter().collect()
    {
        bail!("order must name every Side child of the parent exactly once");
    }
    if order.as_slice() == siblings {
        if opt_str(ctx.args, "confirm")?.is_some() {
            bail!("stale plan: the Side children already have this order");
        }
        return Ok(json!({"applied": true, "changed": false, "order": order}));
    }
    park(conn, &shape, &order)?;
    for (id, old) in order.iter().zip(siblings) {
        set_position(
            conn,
            id,
            &tree,
            &parent.coord_id,
            EdgeKind::Side,
            shape.by_id[old].slot,
        )?;
    }
    let after = shape_of(conn, &tree)?;
    let mapping = filing_path_changes(conn, &[&shape], &[&after], &BTreeSet::new())?;
    check_filing_mapping(conn, ctx.access, &mapping)?;
    let mut lines = vec![format!(
        "reorder {} Side Coordinates of {}",
        order.len(),
        shape.coord[&parent.coord_id]
    )];
    append_filing_changes(&mut lines, &mapping);
    if let Some(plan) = ctx.confirmed(
        "reorder",
        &lines,
        &json!({"order": order, "filing_moves": mapping}),
    )? {
        return Ok(plan);
    }
    apply_filing_mapping(conn, &mapping)?;
    ctx.changed = true;
    Ok(json!({"applied": true, "changed": true, "tree": tree.handle(), "order": order}))
}

fn unplace(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (row, host) = station_arg(conn, ctx.args)?;
    ensure_host_writable(conn, ctx.access, &host)?;
    if row.edge == EdgeKind::Root {
        bail!("the defining root cannot be detached; use delete_card to remove the tree");
    }
    let tree = &host.0;
    let shape = shape_of(conn, tree)?;
    let subtree = shape.subtree(&row.coord_id);
    let policy = opt_str(ctx.args, "children")?;
    let removed = match policy {
        Some("detach") => subtree.clone(),
        Some("reattach") => vec![row.coord_id.clone()],
        None if subtree.len() == 1 => vec![row.coord_id.clone()],
        None => bail!("the Coordinate has successors: pass children: reattach or detach"),
        Some(other) => bail!("unknown children policy `{other}`; expected reattach or detach"),
    };
    let reattach_to = opt_str(ctx.args, "reattach_to")?
        .map(|target| station_in(conn, &shape, target))
        .transpose()?;
    if let Some(target) = &reattach_to
        && (policy != Some("reattach") || subtree.contains(&target.coord_id))
    {
        bail!(
            "reattach_to requires children: reattach and a Coordinate outside the detached branch"
        );
    }
    let removed_set: BTreeSet<String> = removed.iter().cloned().collect();
    let replacement = opt_str(ctx.args, "replacement_filing_coord_id")?
        .map(|target| coord::resolve_coord(conn, target))
        .transpose()?;
    let mut refilings = Vec::new();
    for card_id in removed
        .iter()
        .map(|id| &shape.by_id[id].card_id)
        .collect::<BTreeSet<_>>()
    {
        let identity = coord::identity(conn, card_id)?.context("unknown Card identity")?;
        if !identity
            .filing_coord_id
            .as_ref()
            .is_some_and(|filing| removed_set.contains(filing))
        {
            continue;
        }
        let owner_workspace = identity.live_owner_workspace()?;
        let survivors: Vec<_> = coord::coordinates_of_card(conn, card_id)?
            .into_iter()
            .filter(|p| p.host_workspace == owner_workspace && !removed_set.contains(&p.coord_id))
            .map(|p| p.coord_id)
            .collect();
        if survivors.is_empty() {
            bail!(
                "cannot detach the last owner-Workspace Coordinate (home station) of Card `{}`; use delete_card to delete the Card",
                identity.last_address
            );
        }
        if let Some(replacement) = &replacement
            && survivors.contains(&replacement.coord_id)
        {
            let card = live_card(conn, card_id)?.0;
            check_refile(
                conn,
                ctx.access,
                &card,
                &identity,
                replacement,
                &removed_set,
            )?;
            refilings.push((card, replacement.clone()));
        } else {
            bail!(
                "detaching the Filing Coordinate of Card `{}` requires replacement_filing_coord_id from [{}] or refile_card first",
                identity.last_address,
                survivors.join(", ")
            );
        }
    }
    if replacement.is_some() && refilings.is_empty() {
        bail!("replacement_filing_coord_id is only for a Filing Coordinate being detached");
    }

    let incident = incident_edges(conn, &tree.tree_id, &removed_set)?;
    let revision = coord::next_revision(conn)?;
    if policy == Some("reattach") {
        park(conn, &shape, std::slice::from_ref(&row.coord_id))?;
        reattach_children(
            conn,
            &shape,
            &row,
            reattach_to.as_ref().map(|p| p.coord_id.as_str()),
            true,
        )?;
    }
    // Filing references are deferred until commit. Save the replacement filing pointers
    // before removing rows, including when a surviving child is the replacement.
    for (_, replacement) in &refilings {
        set_filing(conn, &replacement.card_id, &replacement.coord_id)?;
    }
    coord::remove_coordinates_tx(conn, &removed, revision)?;
    if row.edge == EdgeKind::Side {
        compact_sides(
            conn,
            tree,
            row.parent_coord_id.as_deref().context("missing parent")?,
        )?;
    }
    let after = shape_of(conn, tree)?;
    let affected = changed_parents(&[&shape], &[&after]);
    let (review, decisions, missing) = gloss_review(&[&shape], &affected, ctx.args)?;
    let excluded: BTreeSet<String> = refilings.iter().map(|(_, p)| p.card_id.clone()).collect();
    let mut mapping = filing_path_changes(conn, &[&shape], &[&after], &excluded)?;
    for (card, replacement) in &refilings {
        let target = host_of(conn, replacement)?.0;
        let target_address =
            address_for_filing(conn, &target, &replacement.coord_id, &replacement.card_id)?;
        if card.address != target_address {
            mapping.insert(card.address.clone(), target_address);
        }
    }
    check_filing_mapping(conn, ctx.access, &mapping)?;
    let mut changes = crate::link_index::Changes::default();
    write_filing_mapping(conn, &mapping, &mut changes)?;
    for (card, _) in &refilings {
        check_refile_gate(conn, card)?;
    }
    let mut lines = vec![format!(
        "detach {} Coordinate(s) starting at {}",
        removed.len(),
        shape.coord[&row.coord_id]
    )];
    for id in &removed {
        if let Some(gloss) = &shape.by_id[id].gloss {
            lines.push(format!("gloss lost with {}: {gloss}", shape.coord[id]));
        }
    }
    for e in &incident {
        lines.push(format!(
            "tree connection {} is deleted with its endpoint",
            e.connection_id
        ));
    }
    append_gloss_review(&mut lines, &review);
    append_filing_changes(&mut lines, &mapping);
    let effects = json!({"removed": removed, "gloss_review": review, "filing_moves": mapping,
        "connections_deleted": incident.iter().map(|e| &e.connection_id).collect::<Vec<_>>()});
    if let Some(plan) = ctx.confirmed(
        if ctx.tool == "unplace" {
            "unplace"
        } else {
            "detach"
        },
        &lines,
        &effects,
    )? {
        return Ok(plan);
    }
    require_gloss_decisions(&missing)?;
    apply_glosses(conn, &decisions)?;
    if !mapping.is_empty() {
        changes.finish(conn)?;
    }
    ctx.changed = true;
    Ok(json!({"applied": true, "changed": true, "removed": removed,
        "connections_deleted": effects["connections_deleted"], "filing_moves": mapping}))
}

fn rebind(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let (old, host) = station_arg(conn, ctx.args)?;
    ensure_host_writable(conn, ctx.access, &host)?;
    let previous = coord::identity(conn, &old.card_id)?.context("unknown Card identity")?;
    if previous.live || old.edge == EdgeKind::Root {
        bail!("only a Missing non-root Coordinate can be rebound");
    }
    let card_id = named_target_id(conn, ctx.args, "target_at")?;
    let (card, identity) = live_card(conn, &card_id)?;
    let tree = &host.0;
    check_admission(conn, tree, &identity)?;
    let shape = shape_of(conn, tree)?;
    let children = children_of(&shape, &old.coord_id);
    let mut affected = BTreeSet::from([old.coord_id.clone()]);
    affected.extend(children.iter().cloned());
    let (review, mut decisions, missing) = gloss_review(&[&shape], &affected, ctx.args)?;
    let edges = incident_edges(conn, &tree.tree_id, &BTreeSet::from([old.coord_id.clone()]))?;
    let dispositions = ctx
        .args
        .get("connection_dispositions")
        .and_then(Value::as_object);
    let mut lines = vec![format!(
        "rebind Missing Coordinate {} from `{}` to `{}` and issue a fresh coord_id",
        shape.coord[&old.coord_id], previous.last_address, card.address
    )];
    for edge in &edges {
        lines.push(format!(
            "decide connection {}: keep or delete",
            edge.connection_id
        ));
    }
    append_gloss_review(&mut lines, &review);
    let effects = json!({"coord_id": old.coord_id, "card_id": card_id, "gloss_review": review,
        "connections": edges.iter().map(|e| &e.connection_id).collect::<Vec<_>>()});
    if let Some(plan) = ctx.confirmed("rebind", &lines, &effects)? {
        return Ok(plan);
    }
    require_gloss_decisions(&missing)?;
    let kept = edges
        .into_iter()
        .map(|edge| {
            match dispositions
                .and_then(|d| d.get(&edge.connection_id))
                .and_then(Value::as_str)
            {
                Some("keep") => Ok(Some(edge)),
                Some("delete") => Ok(None),
                _ => bail!(
                    "pass connection_dispositions with keep or delete for {}",
                    edge.connection_id
                ),
            }
        })
        .collect::<Result<Vec<_>>>()?;
    let revision = coord::next_revision(conn)?;
    let coord_id = coord::new_uuid()?;
    for child in &children {
        set_parent(conn, child, &coord_id)?;
    }
    coord::remove_coordinates_tx(conn, std::slice::from_ref(&old.coord_id), revision)?;
    let rebind = Placement {
        coord_id: coord_id.clone(),
        card_id,
        gloss: decisions
            .remove(&old.coord_id)
            .unwrap_or_else(|| old.gloss.clone()),
        ..old.clone()
    };
    insert_placement(conn, &rebind)?;
    for mut edge in kept.into_iter().flatten() {
        edge.connection_id = coord::new_uuid()?;
        if edge.from == old.coord_id {
            edge.from.clone_from(&coord_id);
        } else {
            edge.to.clone_from(&coord_id);
        }
        edge.created_by = author(ctx.access).to_string();
        edge.updated_by = edge.created_by.clone();
        edge.created_at_rev = revision;
        edge.updated_at_rev = revision;
        normalize_edge(&mut edge)?;
        insert_edge(conn, &edge)?;
    }
    apply_glosses(conn, &decisions)?;
    let after = shape_of(conn, tree)?;
    let mut result = coordinate_result(&rebind, &after, &card.address, false);
    result["applied"] = json!(true);
    result["changed"] = json!(true);
    result["replaces"] = json!(old.coord_id);
    ctx.changed = true;
    Ok(result)
}

// ---------------------------------------------------------------------------------------------
// Connections
// ---------------------------------------------------------------------------------------------

fn connect(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let tree = coord::resolve_tree(conn, arg_str(ctx.args, "tree")?)?;
    let host = Host(tree.clone());
    ensure_host_writable(conn, ctx.access, &host)?;
    let shape = shape_of(conn, &tree)?;
    let from = station_in(
        conn,
        &shape,
        required_alias(ctx.args, &["from_coord_id", "from_placement_id", "from"])?,
    )?;
    let to = station_in(
        conn,
        &shape,
        required_alias(ctx.args, &["to_coord_id", "to_placement_id", "to"])?,
    )?;
    let directed = match opt_str(ctx.args, "direction")?.unwrap_or("neutral") {
        "neutral" => false,
        "directed" => true,
        other => bail!("unknown direction `{other}`; expected neutral or directed"),
    };
    let revision = coord::next_revision(conn)?;
    let mut edge = Edge {
        connection_id: coord::new_uuid()?,
        tree_id: tree.tree_id.clone(),
        from: from.coord_id,
        to: to.coord_id,
        directed,
        label: opt_str(ctx.args, "relation_label")?
            .map(|l| plain_line(l, "relation_label", coord::LABEL_MAX))
            .transpose()?,
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
    let mut edge =
        edge_by_id(conn, id)?.with_context(|| format!("connection `{id}` does not exist"))?;
    let tree = coord::tree_by_id(conn, &edge.tree_id)?.context("connection of a deleted tree")?;
    ensure_host_writable(conn, ctx.access, &Host(tree.clone()))?;
    if let Some(base) = ctx.args.get("base_updated_at_rev").and_then(Value::as_i64)
        && base != edge.updated_at_rev
    {
        bail!(
            "stale connection: it changed at revision {} after it was read",
            edge.updated_at_rev
        );
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
    let same = edge.directed == before.directed
        && edge.from == before.from
        && edge.to == before.to
        && edge.label == before.label
        && edge.kind == before.kind
        && edge.note == before.note;
    if same {
        let mut value = edge_value(conn, &shape, &edge)?;
        value["changed"] = json!(false);
        return Ok(value);
    }
    if is_blank(&edge)
        && edges_of_tree(conn, &tree.tree_id)?.iter().any(|e| {
            e.connection_id != edge.connection_id
                && is_blank(e)
                && e.from == edge.from
                && e.to == edge.to
        })
    {
        bail!(
            "conflict: this pair already has a blank connection; delete this one instead of clearing it"
        );
    }
    edge.updated_by = author(ctx.access).to_string();
    edge.updated_at_rev = coord::next_revision(conn)?;
    conn.execute(
        "UPDATE coordinate_edges SET from_coord_id = ?2, to_coord_id = ?3, direction = ?4, relation_label = ?5,
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
    let edge =
        edge_by_id(conn, id)?.with_context(|| format!("connection `{id}` does not exist"))?;
    let tree = coord::tree_by_id(conn, &edge.tree_id)?.context("connection of a deleted tree")?;
    ensure_host_writable(conn, ctx.access, &Host(tree.clone()))?;
    let shape = shape_of(conn, &tree)?;
    let value = edge_value(conn, &shape, &edge)?;
    let lines = vec![format!(
        "delete tree connection: {}",
        value["sentence"].as_str().unwrap_or_default()
    )];
    if let Some(plan) = ctx.confirmed("delete", &lines, &json!({"connection": value}))? {
        return Ok(plan);
    }
    conn.execute(
        "DELETE FROM coordinate_edges WHERE connection_id = ?1",
        [id],
    )?;
    ctx.changed = true;
    Ok(json!({"applied": true, "deleted": id, "changed": true}))
}

// ---------------------------------------------------------------------------------------------
// Extract a branch (copy into a new Connection tree)
// ---------------------------------------------------------------------------------------------

fn extract(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let source = coord::resolve_any_tree(conn, arg_str(ctx.args, "source_tree")?)?;
    let shape = shape_of(conn, &source)?;
    let top = station_in(
        conn,
        &shape,
        required_alias(ctx.args, &["root_coord_id", "root_placement_id"])?,
    )?;
    if top.edge == EdgeKind::Root {
        bail!("extract a branch below the root; the root is the tree itself");
    }
    let selection = shape.subtree(&top.coord_id);
    let selected: BTreeSet<String> = selection.iter().cloned().collect();
    let edges = edges_of_tree(conn, &source.tree_id)?;
    let internal: Vec<&Edge> = edges
        .iter()
        .filter(|e| selected.contains(&e.from) && selected.contains(&e.to))
        .collect();
    let crossing: Vec<&Edge> = edges
        .iter()
        .filter(|e| selected.contains(&e.from) != selected.contains(&e.to))
        .collect();
    let workspace = match opt_str(ctx.args, "workspace")? {
        Some(w) => Workspace::parse(w)?,
        None => source.workspace,
    };
    ctx.access.ensure_writable(workspace)?;
    // The new root has no meaning either (D2): its title is optional.
    let title = opt_str(ctx.args, "title")?.unwrap_or_default();
    let named = if title.trim().is_empty() {
        "an untitled Connection tree".to_string()
    } else {
        format!("a new Connection tree \"{}\"", title.trim())
    };
    let mut lines = vec![format!(
        "copy {} station(s) from {} (starting at {}) into {named}; the original tree is unchanged",
        selection.len(),
        source.handle(),
        shape.coord[&top.coord_id]
    )];
    lines.push(format!(
        "{} internal connection(s) are copied with fresh IDs",
        internal.len()
    ));
    for e in &crossing {
        lines.push(format!(
            "boundary connection {} stays in {} only",
            e.connection_id,
            source.handle()
        ));
    }
    if top.gloss.is_some() {
        lines.push("the copied top station starts without its source gloss".to_string());
    }
    let effects = json!({
        "stations": selection,
        "internal": internal.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "boundary": crossing.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
    });
    if let Some(plan) = ctx.confirmed("extract", &lines, &effects)? {
        return Ok(plan);
    }
    let created = create_tree_tx(
        conn,
        ctx.access,
        workspace,
        title,
        "",
        opt_str(ctx.args, "purpose")?,
    )?;
    let new_tree = coord::resolve_tree(conn, created["tree"].as_str().unwrap_or_default())?;
    let new_root = created["root_placement_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let mut map: BTreeMap<String, String> = BTreeMap::new();
    for id in &selection {
        map.insert(id.clone(), coord::new_uuid()?);
    }
    // Insert in outline order so every parent exists first.
    for id in &selection {
        let p = &shape.by_id[id];
        let (parent, edge, ordinal, gloss) = if id == &top.coord_id {
            (Some(new_root.clone()), EdgeKind::Direct, None, None)
        } else {
            (
                p.parent_coord_id.as_ref().map(|parent| map[parent].clone()),
                p.edge,
                p.slot,
                p.gloss.clone(),
            )
        };
        insert_placement(
            conn,
            &Placement {
                coord_id: map[id].clone(),
                tree_id: new_tree.tree_id.clone(),
                host_workspace: new_tree.workspace,
                card_id: p.card_id.clone(),
                parent_coord_id: parent,
                edge,
                slot: ordinal,
                gloss,
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
    shape_of(conn, &new_tree)?;
    ctx.changed = true;
    Ok(json!({
        "applied": true,
        "tree": new_tree.handle(),
        "tree_id": new_tree.tree_id,
        "stations": map,
        "coordinates": map,
        "boundary_connections": crossing.iter().map(|e| &e.connection_id).collect::<Vec<_>>(),
        "changed": true,
    }))
}

// ---------------------------------------------------------------------------------------------
// Refiling, deletion, and tree readers

fn address_for_filing(
    conn: &Connection,
    tree: &TreeRecord,
    id: &str,
    card_id: &str,
) -> Result<String> {
    if tree.role != TreeRole::Connection {
        let row = coord::coordinate(conn, id)?.context("destination Coordinate is missing")?;
        return coord::coordinate_path(conn, &row);
    }
    // The schema grants one permanent handle per Card. A previously issued handle continues to
    // identify the Card, even when its Filing Coordinate is in another Connection tree.
    if let Some(handle) = conn
        .query_row(
            "SELECT handle FROM card_handles WHERE card_id = ?1",
            [card_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
    {
        return Ok(handle);
    }
    let tree = coord::tree_by_id(conn, &tree.tree_id)?.context("destination tree is missing")?;
    let (local, handle) = allocate_native_handle(conn, &tree)?;
    conn.execute(
        "INSERT INTO card_handles(handle, card_id, origin_tree_id, local_seq) VALUES(?1, ?2, ?3, ?4)",
        params![handle, card_id, tree.tree_id, local],
    )?;
    Ok(handle)
}

/// Compute filing renames only for Coordinates that survive the edit.
fn filing_path_changes(
    conn: &Connection,
    before: &[&TreeShape],
    after: &[&TreeShape],
    excluded: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let origins: BTreeMap<_, _> = before
        .iter()
        .flat_map(|&shape| shape.by_id.keys().map(move |id| (id.as_str(), shape)))
        .collect();
    let mut mapping = BTreeMap::new();
    for shape in after {
        for (id, row) in &shape.by_id {
            let Some(old_shape) = origins.get(id.as_str()) else {
                continue;
            };
            if excluded.contains(&row.card_id)
                || (old_shape.tree.tree_id == shape.tree.tree_id
                    && (shape.tree.role == TreeRole::Connection
                        || old_shape.coord[id] == shape.coord[id]))
            {
                continue;
            }
            let identity = coord::identity(conn, &row.card_id)?.context("unknown Card identity")?;
            if identity.filing_coord_id.as_deref() != Some(id.as_str()) {
                continue;
            }
            let new = if shape.tree.role == TreeRole::Connection {
                address_for_filing(conn, &shape.tree, id, &row.card_id)?
            } else {
                shape.coord[id].clone()
            };
            if identity.last_address != new {
                mapping.insert(identity.last_address, new);
            }
        }
    }
    Ok(mapping)
}

fn literal_rewrites(mapping: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    mapping
        .iter()
        .filter(|(old, _)| !coord::is_handle(old))
        .map(|(old, new)| (old.clone(), new.clone()))
        .collect()
}

pub(crate) fn check_filing_mapping(
    conn: &Connection,
    access: Access,
    mapping: &BTreeMap<String, String>,
) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    let rewrite = literal_rewrites(mapping);
    access.ensure_link_rewrites_allowed(conn, &rewrite)?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Mapping(&rewrite))?;
    crate::coord_lock::ensure_moves_allowed(conn, mapping.keys())?;
    for (old, new) in mapping {
        let card = crate::load_card(conn, old)?;
        access.ensure_writable(card.workspace)?;
        if coord::is_handle(old) {
            crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
            if !coord::is_handle(new) {
                crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::CreateAt(new))?;
            }
        }
        if mapping.contains_key(new) {
            continue;
        }
        match coord::canonical(conn, new)? {
            Some(address) if &address == old => continue,
            Some(_) => bail!("the new filing address `{new}` is occupied"),
            None => {}
        }
        if crate::link_index::ready(conn)
            && !crate::link_index::rewrite_sources(conn, [new])?.is_empty()
        {
            bail!("the new filing address `{new}` would capture an existing Broken link");
        }
    }
    Ok(())
}

pub(crate) fn append_filing_changes(lines: &mut Vec<String>, mapping: &BTreeMap<String, String>) {
    lines.extend(
        mapping
            .iter()
            .map(|(old, new)| format!("filing address: {old} -> {new}")),
    );
}

/// Re-key the Card cache and Link index after the Coordinate topology has already been written.
/// In particular, native handles remain literal targets even when the old cache address changes.
pub(crate) fn write_filing_mapping(
    conn: &Connection,
    mapping: &BTreeMap<String, String>,
    changes: &mut crate::link_index::Changes,
) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    let rewrite = literal_rewrites(mapping);
    let indexed = crate::link_index::ready(conn);
    let handles: BTreeSet<_> = mapping
        .keys()
        .filter(|old| coord::is_handle(old))
        .cloned()
        .collect();
    let sources = if indexed {
        crate::link_index::rewrite_sources(conn, rewrite.keys())?
    } else {
        BTreeSet::new()
    };
    changes.drop_sources(conn, &handles)?;
    // Stage by immutable row ID so overlapping source and destination addresses cannot collide.
    let mut rows = Vec::new();
    for (old, new) in mapping {
        let card = crate::load_card(conn, old)?;
        conn.execute(
            "UPDATE cards SET location = ?2 WHERE row_id = ?1",
            params![card.row_id, format!("__moving__{old}")],
        )?;
        rows.push((card.row_id, new));
    }
    for (row, new) in rows {
        conn.execute(
            "UPDATE cards SET location = ?2 WHERE row_id = ?1",
            params![row, new],
        )?;
        conn.execute(
            "UPDATE card_identity SET last_address = ?2 WHERE card_row = ?1 AND last_address <> ?2",
            params![row, new],
        )?;
    }
    let affected = if indexed {
        sources
            .iter()
            .map(|source| crate::load_card(conn, mapping.get(source).unwrap_or(source)))
            .collect::<Result<Vec<_>>>()?
    } else {
        crate::load_cards(conn)?
    };
    changes.renamed(conn, &rewrite)?;
    for card in affected {
        let text = crate::replace_link_locations(&card.text, &rewrite)?;
        if text != card.text {
            conn.execute(
                "UPDATE cards SET text = ?2 WHERE row_id = ?1",
                params![card.row_id, text],
            )?;
            changes.rewrote(&card.address);
        }
    }
    for old in &handles {
        changes.resync(conn, &mapping[old])?;
    }
    crate::spread::follow(conn, mapping, &BTreeSet::new())
}

fn apply_filing_mapping(conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<()> {
    if !mapping.is_empty() {
        let mut changes = crate::link_index::Changes::default();
        write_filing_mapping(conn, mapping, &mut changes)?;
        changes.finish(conn)?;
    }
    Ok(())
}

fn set_filing(conn: &Connection, card_id: &str, coord_id: &str) -> Result<()> {
    conn.execute(
        "UPDATE card_identity SET filing_coord_id = ?2 WHERE card_id = ?1",
        params![card_id, coord_id],
    )?;
    Ok(())
}

/// Validate filing authority and structure before changing the outline. The Write Gate runs
/// against the final staged addresses once the entire filing mapping is in place.
fn check_filing_destination(
    conn: &Connection,
    access: Access,
    card: &crate::Card,
    dest_tree: &TreeRecord,
    dest_parent_coord_id: &str,
    dest_edge: EdgeKind,
) -> Result<()> {
    let card_id = coord::card_id_of_row(conn, card.row_id)?;
    let identity = coord::identity(conn, &card_id)?.context("unknown Card identity")?;
    if dest_tree.workspace != card.workspace
        || dest_tree.workspace != identity.live_owner_workspace()?
    {
        bail!("a Filing Coordinate must stay within the Card's owner Workspace");
    }
    let source = filing_row(conn, &identity)?;
    if source.edge == EdgeKind::Root || dest_edge == EdgeKind::Root {
        bail!("a defining Tree root cannot be refiled");
    }
    ensure_host_writable(conn, access, &host_of(conn, &source)?)?;
    ensure_host_writable(conn, access, &Host(dest_tree.clone()))?;
    if dest_tree.role == TreeRole::Connection {
        ensure_native_type(card.card_type)?;
    } else if dest_tree.role == TreeRole::Literature && source.tree_id != dest_tree.tree_id {
        bail!(
            "MC3: Literature tree `{}` accepts only its own filed Cards",
            dest_tree.handle()
        );
    }
    coord::check_model_term_splice_guard(conn, dest_tree, dest_parent_coord_id, dest_edge)?;
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
    crate::coord_lock::ensure_card_not_pinned(conn, &card_id, "pinned-move")
}

fn check_literature_survivors(
    conn: &Connection,
    card_id: &str,
    dest_tree: &TreeRecord,
    leaving: &BTreeSet<String>,
) -> Result<()> {
    for row in coord::coordinates_of_card(conn, card_id)? {
        if row.tree_id == dest_tree.tree_id || leaving.contains(&row.coord_id) {
            continue;
        }
        let tree = host_of(conn, &row)?.0;
        if tree.role == TreeRole::Literature {
            let hint = if row.edge == EdgeKind::Root {
                "the defining root cannot leave that tree"
            } else {
                "move the Filing Coordinate or detach the remaining occurrence first"
            };
            bail!(
                "MC3: this filing change would leave a foreign Card in Literature tree `{}`; {hint}",
                tree.handle()
            );
        }
    }
    Ok(())
}

fn check_refile(
    conn: &Connection,
    access: Access,
    card: &crate::Card,
    identity: &coord::Identity,
    target: &Placement,
    removed: &BTreeSet<String>,
) -> Result<()> {
    if target.card_id != identity.card_id {
        bail!("the replacement Filing Coordinate must hold the same Card");
    }
    let from = filing_row(conn, identity)?;
    if from.edge == EdgeKind::Root {
        bail!("a defining Tree root cannot be refiled");
    }
    let destination = host_of(conn, target)?;
    let parent = target
        .parent_coord_id
        .as_deref()
        .context("the Filing Coordinate needs a parent")?;
    check_filing_destination(conn, access, card, &destination.0, parent, target.edge)?;
    if from.tree_id != target.tree_id {
        check_literature_survivors(conn, &identity.card_id, &destination.0, removed)?;
    }
    Ok(())
}

fn check_refile_gate(conn: &Connection, before: &crate::Card) -> Result<crate::gate::Accepted> {
    let current = card_by_row(conn, before.row_id)?;
    let mut candidate = crate::gate::Candidate::edit(
        &current,
        &current.text,
        current.card_type,
        current.facet.clone(),
        current.emphasis,
        current.level,
        current.hub_level,
        current.form,
    );
    candidate.before = Some(before.clone());
    crate::gate::check_proposed(conn, &candidate)
}

fn finish_refile(
    ctx: &mut Ctx,
    card: &crate::Card,
    identity: &coord::Identity,
    target: &Placement,
    word: &str,
) -> Result<Value> {
    let conn = ctx.conn;
    let tree = host_of(conn, target)?.0;
    let address = address_for_filing(conn, &tree, &target.coord_id, &identity.card_id)?;
    let mapping = if address == card.address {
        BTreeMap::new()
    } else {
        BTreeMap::from([(card.address.clone(), address.clone())])
    };
    check_filing_mapping(conn, ctx.access, &mapping)?;
    set_filing(conn, &identity.card_id, &target.coord_id)?;
    let mut changes = crate::link_index::Changes::default();
    write_filing_mapping(conn, &mapping, &mut changes)?;
    let accepted = check_refile_gate(conn, card)?;
    let mut lines = vec![format!("refile `{}` at `{}`", card.address, address)];
    append_filing_changes(&mut lines, &mapping);
    // The destination path is stable across a promotion preview and apply, even if a fresh
    // Coordinate UUID must be allocated to fill it.
    let path = coord::coordinate_path(conn, target)?;
    let effects = json!({"card_id": identity.card_id, "from_coord_id": identity.filing_coord_id,
        "destination": path, "filing_moves": mapping});
    if let Some(plan) = ctx.confirmed(word, &lines, &effects)? {
        return Ok(plan);
    }
    if !mapping.is_empty() {
        changes.finish(conn)?;
    }
    ctx.changed = true;
    let mut result = json!({"applied": true, "changed": true, "card_id": identity.card_id,
        "coord_id": target.coord_id, "placement_id": target.coord_id, "filing_coord_id": target.coord_id,
        "coord": path, "home_address": address, "address": address, "from": card.address, "to": address,
        "tree": tree.handle(), "is_filing": true});
    accepted.attach(&mut result);
    Ok(result)
}

fn refile(ctx: &mut Ctx) -> Result<Value> {
    let card_id = named_target_id(ctx.conn, ctx.args, "at")?;
    let (card, identity) = live_card(ctx.conn, &card_id)?;
    let target = coord::resolve_coord(ctx.conn, arg_str(ctx.args, "to_coord_id")?)?;
    check_refile(
        ctx.conn,
        ctx.access,
        &card,
        &identity,
        &target,
        &BTreeSet::new(),
    )?;
    if identity.filing_coord_id.as_deref() == Some(&target.coord_id) {
        if opt_str(ctx.args, "confirm")?.is_some() {
            bail!("stale plan: the Card is already filed at that Coordinate");
        }
        return Ok(
            json!({"applied": true, "changed": false, "card_id": card_id,
            "filing_coord_id": target.coord_id, "coord_id": target.coord_id, "placement_id": target.coord_id,
            "address": card.address, "home_address": card.address, "is_filing": true}),
        );
    }
    finish_refile(ctx, &card, &identity, &target, "refile")
}

fn promote(ctx: &mut Ctx) -> Result<Value> {
    let conn = ctx.conn;
    let card_id = named_target_id(conn, ctx.args, "at")?;
    let (card, identity) = live_card(conn, &card_id)?;
    let source = filing_row(conn, &identity)?;
    if host_of(conn, &source)?.0.role != TreeRole::Connection || source.edge == EdgeKind::Root {
        bail!("only a native Connection Card can be promoted");
    }
    let to = required_alias(ctx.args, &["to", "target"])?;
    if !crate::is_valid_regular_location(to) || TreeRole::of_address(to) != TreeRole::Topic {
        bail!("promote to a Regular Coordinate of a Topic tree, such as `3/6` or `3/2|b`");
    }
    let tree = coord::resolve_any_tree(conn, crate::tree_id(to))?;
    if let Some(topic) = opt_str(ctx.args, "destination_topic")?
        && coord::resolve_any_tree(conn, topic)?.tree_id != tree.tree_id
    {
        bail!("`{to}` is not in Topic {topic}");
    }
    let shape = shape_of(conn, &tree)?;
    let target = if let Some(id) = shape.by_coord.get(to) {
        shape.by_id[id].clone()
    } else {
        let parent = crate::parent_location(to).context("no predecessor for the destination")?;
        let parent = station_in(conn, &shape, &parent)?;
        let last = crate::last_segment(to);
        let (edge, slot) = if last.bytes().all(|byte| byte.is_ascii_digit()) {
            (EdgeKind::Direct, None)
        } else {
            (EdgeKind::Side, Some(crate::side_label_to_number(last)))
        };
        coord::check_model_term_splice_guard(conn, &tree, &parent.coord_id, edge)?;
        let target = Placement {
            coord_id: coord::new_uuid()?,
            tree_id: tree.tree_id.clone(),
            host_workspace: tree.workspace,
            card_id: card_id.clone(),
            parent_coord_id: Some(parent.coord_id),
            edge,
            slot,
            gloss: None,
        };
        check_refile(
            conn,
            ctx.access,
            &card,
            &identity,
            &target,
            &BTreeSet::new(),
        )?;
        insert_placement(conn, &target)?;
        if coord::coordinate_path(conn, &target)? != to {
            bail!("the requested destination does not match its derived Coordinate path");
        }
        target
    };
    check_refile(
        conn,
        ctx.access,
        &card,
        &identity,
        &target,
        &BTreeSet::new(),
    )?;
    let mut value = finish_refile(ctx, &card, &identity, &target, "promote")?;
    if value["applied"] == true {
        value["handle"] = json!(card.address);
        value["destination_tree"] = json!(tree.handle());
        value["tree"] = json!(host_of(conn, &source)?.0.handle());
    }
    Ok(value)
}

fn delete_card(ctx: &mut Ctx) -> Result<Value> {
    crate::ops::delete_identity_tx(
        ctx.conn,
        ctx.access,
        ctx.args,
        &mut |word, lines, effects| {
            confirm_plan(
                ctx.conn,
                ctx.tool,
                ctx.args,
                ctx.revision,
                word,
                lines,
                effects,
            )
        },
    )
    .map(|mut value| {
        ctx.changed = value["applied"] == true;
        if ctx.changed {
            value["plan_hash"] = ctx.args["plan_hash"].clone();
        }
        value
    })
}

pub(crate) struct IdentityDeleteCoordinates {
    pub(crate) filing_moves: BTreeMap<String, String>,
    pub(crate) removed: BTreeSet<String>,
    pub(crate) missing: Vec<Placement>,
    pub(crate) gloss_review: Vec<Value>,
    pub(crate) missing_gloss: Vec<String>,
}

/// Stage the removal of every occurrence of the Card in its owner Workspace.
pub(crate) fn remove_coordinates_for_identity_delete(
    conn: &Connection,
    access: Access,
    identity: &coord::Identity,
    args: &Value,
) -> Result<IdentityDeleteCoordinates> {
    let detach = match opt_str(args, "children")? {
        Some("detach") => true,
        None | Some("reattach") => false,
        Some(other) => bail!("unknown children policy `{other}`; expected reattach or detach"),
    };
    let owner = identity.live_owner_workspace()?;
    let (owned, missing): (Vec<_>, Vec<_>) = coord::coordinates_of_card(conn, &identity.card_id)?
        .into_iter()
        .partition(|row| row.host_workspace == owner);
    let mut by_tree: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in owned {
        if row.edge == EdgeKind::Root {
            bail!(
                "cannot delete a defining root by Card identity; delete or empty the tree explicitly"
            );
        }
        by_tree.entry(row.tree_id).or_default().insert(row.coord_id);
    }
    let mut planned = Vec::new();
    let mut removed = BTreeSet::new();
    for (tree_id, mut ids) in by_tree {
        let host = Host(coord::tree_by_id(conn, &tree_id)?.context("Coordinate host is missing")?);
        crate::scope::ensure_literature_host_allowed(host.0.role, &host.0.public_key)?;
        ensure_host_writable(conn, access, &host)?;
        let before = shape_of(conn, &host.0)?;
        if detach {
            // Ancestors precede their children. Expand overlapping subtrees in one pass.
            for id in &before.order {
                if before.by_id[id]
                    .parent_coord_id
                    .as_ref()
                    .is_some_and(|parent| ids.contains(parent))
                {
                    ids.insert(id.clone());
                }
            }
        }
        removed.extend(ids.iter().cloned());
        planned.push((before, ids));
    }
    if detach {
        for (shape, ids) in &planned {
            for id in ids {
                let row = &shape.by_id[id];
                if row.card_id == identity.card_id {
                    continue;
                }
                let card = coord::identity(conn, &row.card_id)?.context("unknown Card identity")?;
                if card.filing_coord_id.as_deref() != Some(id.as_str()) {
                    continue;
                }
                let owner = card.live_owner_workspace()?;
                let survives =
                    coord::coordinates_of_card(conn, &row.card_id)?
                        .iter()
                        .any(|other| {
                            other.host_workspace == owner && !removed.contains(&other.coord_id)
                        });
                if !survives {
                    bail!(
                        "cannot detach the last owner-Workspace Coordinate of Card `{}`; use children: reattach",
                        card.last_address
                    );
                }
                bail!(
                    "detaching would remove the Filing Coordinate of Card `{}`; refile that Card first or use children: reattach",
                    card.last_address
                );
            }
        }
    }

    let revision = coord::next_revision(conn)?;
    let mut after = Vec::new();
    for (shape, ids) in &planned {
        if detach {
            coord::remove_coordinates_tx(conn, &ids.iter().cloned().collect::<Vec<_>>(), revision)?;
        } else {
            // Outline order visits ancestors before descendants. Process it backward so
            // another occurrence of this Card is never reattached as a surviving child.
            for id in shape.order.iter().rev().filter(|id| ids.contains(*id)) {
                reattach_identity_delete_children(conn, shape, &shape.by_id[id])?;
                coord::remove_coordinates_tx(conn, std::slice::from_ref(id), revision)?;
            }
        }
        let side_parents: BTreeSet<_> = ids
            .iter()
            .map(|id| &shape.by_id[id])
            .filter(|row| row.edge == EdgeKind::Side)
            .filter_map(|row| row.parent_coord_id.as_deref())
            .filter(|parent| !ids.contains(*parent))
            .collect();
        for parent in side_parents {
            compact_sides(conn, &shape.tree, parent)?;
        }
        after.push(shape_of(conn, &shape.tree)?);
    }
    let before: Vec<_> = planned.iter().map(|(shape, _)| shape).collect();
    let after: Vec<_> = after.iter().collect();
    let affected = changed_parents(&before, &after);
    let (gloss_review, decisions, missing_gloss) = gloss_review(&before, &affected, args)?;
    apply_glosses(conn, &decisions)?;
    let excluded = BTreeSet::from([identity.card_id.clone()]);
    let filing_moves = filing_path_changes(conn, &before, &after, &excluded)?;
    Ok(IdentityDeleteCoordinates {
        filing_moves,
        removed,
        missing,
        gloss_review,
        missing_gloss,
    })
}

fn reattach_identity_delete_children(
    conn: &Connection,
    shape: &TreeShape,
    row: &Placement,
) -> Result<()> {
    let tree = &shape.tree;
    let parent = row
        .parent_coord_id
        .as_deref()
        .context("cannot delete a defining root by Card identity")?;
    park(conn, shape, std::slice::from_ref(&row.coord_id))?;
    let successor = direct_child(conn, &row.coord_id)?;
    let sides: Vec<_> = side_slots(conn, &row.coord_id)?
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if let Some(successor) = &successor {
        set_position(conn, successor, tree, parent, row.edge, row.slot)?;
    }
    if parent == shape.root {
        // A defining root takes only a Direct child. Its surviving Direct child
        // hosts the Side branches, or the first Side branch becomes Direct.
        if let Some(successor) = successor {
            append_side_children(conn, tree, &successor, &sides)?;
        } else if let Some((first, rest)) = sides.split_first() {
            set_position(conn, first, tree, parent, EdgeKind::Direct, None)?;
            append_side_children(conn, tree, first, rest)?;
        }
    } else {
        append_side_children(conn, tree, parent, &sides)?;
    }
    Ok(())
}

fn read(conn: &Connection, access: Access, tool: &str, args: &Value) -> Result<Value> {
    match tool {
        "list_connection_trees" => list_trees(conn, access, args),
        "get_connection_tree" => get_tree(conn, access, args),
        "get_tree_coordinates" => {
            let tree = coord::resolve_any_tree(conn, arg_str(args, "tree")?)?;
            tree_page(conn, access, args, &tree)
        }
        "list_placements" | "list_coordinates" => {
            list_coordinates(conn, access, args, tool == "list_placements")
        }
        "list_connections" => list_connections(conn, args),
        "list_side_mounts" => list_side_mounts(conn, access, args),
        "get_dependencies" => crate::dependencies::get(conn, args),
        _ => bail!("unknown Coordinate tool `{tool}`"),
    }
}

pub(crate) fn root_title(conn: &Connection, tree: &ConnTree) -> Result<(String, bool)> {
    let address = coord::address_of_card_id(conn, &tree.root_card_id)?
        .with_context(|| format!("the root Card of {} is missing", tree.handle()))?;
    let card = crate::load_card(conn, &address)?;
    let title = crate::parse_card_text(&card.text)?.title;
    if tree.role == TreeRole::Connection && title == tree.handle() {
        return Ok((String::new(), true));
    }
    Ok((title, false))
}

pub(crate) fn tree_summary(conn: &Connection, access: Access, tree: &ConnTree) -> Result<Value> {
    let (title, untitled) = root_title(conn, tree)?;
    let (total, distinct, native, mounted, missing): (i64, i64, i64, i64, i64) = conn.query_row(
        "SELECT COUNT(*), COUNT(DISTINCT c.card_id),
            COUNT(CASE WHEN c.edge <> 'root' AND i.filing_coord_id = c.coord_id THEN 1 END),
            COUNT(CASE WHEN c.edge <> 'root' AND i.filing_coord_id IS NOT c.coord_id THEN 1 END),
            COUNT(CASE WHEN i.status = 'retired' THEN 1 END)
         FROM coordinates AS c JOIN card_identity AS i ON i.card_id = c.card_id
         WHERE c.tree_id = ?1",
        [&tree.tree_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    let connections: i64 = conn.query_row(
        "SELECT COUNT(*) FROM coordinate_edges WHERE tree_id = ?1",
        [&tree.tree_id],
        |r| r.get(0),
    )?;
    Ok(json!({
        "tree": tree.handle(), "tree_id": tree.tree_id, "title": title, "untitled": untitled,
        "role": tree.role.id(), "workspace": tree.workspace.id(), "purpose": tree.purpose,
        "root_card_id": tree.root_card_id, "mounted": mounted, "native": native, "missing": missing,
        "connections": connections, "distinct_cards": distinct, "total_coordinates": total,
        "locked": crate::coord_lock::host_locked(conn, &tree.tree_id)?,
        "can_edit_tree": can_edit_host(conn, access, &Host(tree.clone()))?, "created_at": tree.created_at,
        "counts": format!("{mounted} mounted · {native} native · {connections} connections"),
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
        if workspace.is_some_and(|w| w != tree.workspace)
            || purpose.is_some_and(|p| tree.purpose.as_deref() != Some(p))
        {
            continue;
        }
        if let Some(q) = &filter {
            let (title, _) = root_title(conn, &tree)?;
            let haystack = format!(
                "{} {} {}",
                tree.handle(),
                title,
                tree.purpose.as_deref().unwrap_or_default()
            )
            .to_lowercase();
            if !haystack.contains(q.as_str()) {
                continue;
            }
        }
        out.push(tree_summary(conn, access, &tree)?);
    }
    match opt_str(args, "sort")? {
        None | Some("number") => {}
        Some("recent") => out.reverse(),
        Some(other) => bail!("unknown sort `{other}`; expected number or recent"),
    }
    Ok(json!({"trees": out, "archive_revision": crate::link_index::revision(conn)?}))
}

pub(crate) fn get_tree(conn: &Connection, access: Access, args: &Value) -> Result<Value> {
    let tree = coord::resolve_tree(conn, arg_str(args, "tree")?)?;
    tree_page(conn, access, args, &tree)
}

fn tree_page(conn: &Connection, access: Access, args: &Value, tree: &TreeRecord) -> Result<Value> {
    let shape = shape_of(conn, tree)?;
    let host = Host(tree.clone());
    let editable = can_edit_host(conn, access, &host)?;
    let revision = crate::link_index::revision(conn)?;
    let limit = args
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(500)
        .clamp(1, 5000) as usize;
    let start = match opt_str(args, "cursor")? {
        Some(cursor) => {
            let (rev, offset) = cursor.split_once(':').context("invalid cursor")?;
            if rev.parse::<u64>().ok() != Some(revision) {
                bail!(
                    "stale cursor: the tree changed since the first page; read it again from the start"
                );
            }
            offset.parse::<usize>().context("invalid cursor")?
        }
        None => 0,
    };
    let mut repeats = BTreeMap::new();
    for row in shape.by_id.values() {
        *repeats.entry(row.card_id.as_str()).or_insert(0) += 1;
    }
    let mut stations = Vec::new();
    for id in shape.order.iter().skip(start).take(limit) {
        let row = &shape.by_id[id];
        stations.push(station_value(
            conn,
            access,
            &host,
            editable,
            row,
            &shape.coord[id],
            repeats[row.card_id.as_str()],
        )?);
    }
    let end = start.saturating_add(limit);
    let next = (end < shape.order.len()).then(|| format!("{revision}:{end}"));
    let mut value = tree_summary(conn, access, tree)?;
    value["root_coord_id"] = json!(shape.root);
    value["root_placement_id"] = json!(shape.root);
    value["coordinates"] = json!(stations);
    value["placements"] = json!(stations);
    value["total_coordinates"] = json!(shape.order.len());
    value["total_placements"] = json!(shape.order.len());
    value["next_cursor"] = json!(next);
    value["archive_revision"] = json!(revision);
    if args
        .get("include_connections")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        value["connections"] = json!(
            edges_of_tree(conn, &tree.tree_id)?
                .iter()
                .map(|e| edge_value(conn, &shape, e))
                .collect::<Result<Vec<_>>>()?
        );
    } else {
        value
            .as_object_mut()
            .expect("tree summary is an object")
            .remove("connections");
    }
    if tree.role != TreeRole::Connection {
        let (elsewhere, crossings) = projections(conn, &shape)?;
        value["elsewhere"] = elsewhere;
        value["crossings"] = crossings;
    }
    Ok(value)
}

fn list_coordinates(
    conn: &Connection,
    access: Access,
    args: &Value,
    legacy_placements: bool,
) -> Result<Value> {
    let card_id = named_target_id(conn, args, "at")?;
    let identity = coord::identity(conn, &card_id)?
        .with_context(|| format!("Card `{card_id}` does not exist"))?;
    let coordinates = coord::coordinates_of_card(conn, &card_id)?;
    let mut shapes = BTreeMap::new();
    let mut repeats = BTreeMap::new();
    for row in &coordinates {
        *repeats.entry(row.tree_id.as_str()).or_insert(0) += 1;
        if !shapes.contains_key(&row.tree_id) {
            let host = host_of(conn, row)?;
            let editable = can_edit_host(conn, access, &host)?;
            let shape = shape_of(conn, &host.0)?;
            shapes.insert(row.tree_id.clone(), (host, editable, shape));
        }
    }
    let mut stations = Vec::new();
    for row in &coordinates {
        let (host, editable, shape) = &shapes[&row.tree_id];
        stations.push(station_value(
            conn,
            access,
            host,
            *editable,
            row,
            &shape.coord[&row.coord_id],
            repeats[row.tree_id.as_str()],
        )?);
    }
    stations.sort_by(|a, b| a["coord"].as_str().cmp(&b["coord"].as_str()));
    let placements: Vec<_> = stations
        .iter()
        .filter(|station| {
            !legacy_placements
                || station["is_filing"] == false
                || station["tree_role"] == TreeRole::Connection.id()
        })
        .collect();
    let handle: Option<String> = conn
        .query_row(
            "SELECT handle FROM card_handles WHERE card_id = ?1",
            [&card_id],
            |r| r.get(0),
        )
        .optional()?;
    let home = identity.live.then_some(&identity.last_address);
    let role = if identity.live {
        Some(host_of(conn, &filing_row(conn, &identity)?)?.0.role.id())
    } else {
        None
    };
    Ok(json!({
        "card_id": card_id, "status": if identity.live { "live" } else { "retired" }, "home": home,
        "home_role": role, "last_address": identity.last_address, "title": identity.last_title,
        "handle": handle, "filing_coord_id": identity.filing_coord_id, "coordinates": stations,
        "placements": placements, "places": stations.len(), "archive_revision": crate::link_index::revision(conn)?,
    }))
}

fn station_value(
    conn: &Connection,
    access: Access,
    host: &Host,
    editable: bool,
    row: &Placement,
    path: &str,
    repeats: usize,
) -> Result<Value> {
    let identity =
        coord::identity(conn, &row.card_id)?.context("Coordinate names an unknown Card")?;
    let is_filing = identity.filing_coord_id.as_deref() == Some(row.coord_id.as_str());
    let origin = match (row.edge, is_filing, host.0.role) {
        (EdgeKind::Root, _, TreeRole::Connection) | (_, true, TreeRole::Connection) => "native",
        (EdgeKind::Root, _, _) => "root",
        (_, true, _) => "filing",
        (_, false, TreeRole::Connection) => "mount",
        _ => "secondary",
    };
    let places: i64 = conn.query_row(
        "SELECT COUNT(*) FROM coordinates WHERE card_id = ?1",
        [&row.card_id],
        |r| r.get(0),
    )?;
    let mut value = json!({
        "coord_id": row.coord_id, "placement_id": row.coord_id, "coord": path,
        "card_id": row.card_id, "edge": row.edge.id(), "parent_coord_id": row.parent_coord_id,
        "parent_id": row.parent_coord_id, "slot": row.slot, "ordinal": row.slot, "gloss": row.gloss,
        "host": host.label(), "tree": host.0.handle(), "tree_id": host.key(), "tree_role": host.0.role.id(),
        "host_workspace": host.workspace().id(), "can_edit_coordinate": editable, "can_edit_placement": editable,
        "bookmark": format!("place:{}", row.coord_id), "is_filing": is_filing, "origin": origin,
        "places": places, "repeat_count": repeats, "home_address": identity.last_address,
        "home_workspace": identity.owner_workspace.map(Workspace::id),
    });
    if let Some(card_row) = identity.card_row.filter(|_| identity.live) {
        let card = card_by_row(conn, card_row)?;
        let filing = filing_row(conn, &identity)?;
        let home = if filing.tree_id == row.tree_id {
            host.0.clone()
        } else {
            host_of(conn, &filing)?.0
        };
        let parsed = crate::parse_card_text(&card.text)?;
        value["status"] = json!("live");
        value["home_role"] = json!(home.role.id());
        value["title"] = json!(parsed.title);
        value["excerpt"] = json!(crate::ops::excerpt(&parsed.body));
        value["type"] = json!(card.card_type.id());
        value["can_edit_card"] = json!(
            can_edit_card(conn, access, &card)
                && !crate::coord_lock::host_locked(conn, &home.tree_id)?
        );
        value["citation"] = json!(format!(
            "[[{}]]",
            citation_address(conn, &row.card_id, &card.address)?
        ));
        if filing.edge == EdgeKind::Root && row.edge != EdgeKind::Root {
            let cards: i64 = conn.query_row(
                "SELECT COUNT(*) FROM coordinates AS p JOIN card_identity AS i ON i.filing_coord_id = p.coord_id
                 WHERE p.tree_id = ?1",
                [&home.tree_id], |r| r.get(0),
            )?;
            value["portal"] =
                json!({"tree": home.handle(), "role": home.role.id(), "cards": cards});
        }
    } else {
        value["status"] = json!("missing");
        value["title"] = json!(identity.last_title);
        value["home_role"] = json!(TreeRole::of_address(&identity.last_address).id());
        value["excerpt"] = json!("");
        value["type"] = Value::Null;
        value["citation"] = Value::Null;
        value["can_edit_card"] = json!(false);
    }
    if let Some(drift) = crate::coord_lock::drift(conn, &row.coord_id)? {
        value["drift"] = drift;
    }
    Ok(value)
}

pub(crate) fn list_connections(conn: &Connection, args: &Value) -> Result<Value> {
    let tree = coord::resolve_any_tree(conn, arg_str(args, "tree")?)?;
    let shape = shape_of(conn, &tree)?;
    let focus = alias(args, &["coord_id", "placement_id"])?
        .map(|id| station_in(conn, &shape, id))
        .transpose()?;
    let edges = edges_of_tree(conn, &tree.tree_id)?;
    let labels = edges
        .iter()
        .filter_map(|e| e.label.as_deref())
        .collect::<BTreeSet<_>>();
    let mut value = json!({"tree": tree.handle(), "archive_revision": crate::link_index::revision(conn)?, "labels": labels});
    if let Some(row) = focus {
        value["coord_id"] = json!(row.coord_id);
        value["placement_id"] = json!(row.coord_id);
        let by_direction = incident_values(conn, &shape, &edges, &row.coord_id)?;
        for key in ["incoming", "outgoing", "neutral"] {
            value[key] = by_direction[key].clone();
        }
    } else {
        value["connections"] = json!(
            edges
                .iter()
                .map(|edge| edge_value(conn, &shape, edge))
                .collect::<Result<Vec<_>>>()?
        );
    }
    Ok(value)
}

fn incident_values(
    conn: &Connection,
    shape: &TreeShape,
    edges: &[Edge],
    coord_id: &str,
) -> Result<Value> {
    let mut incoming = Vec::new();
    let mut outgoing = Vec::new();
    let mut neutral = Vec::new();
    for edge in edges
        .iter()
        .filter(|e| e.from == coord_id || e.to == coord_id)
    {
        let value = edge_value(conn, shape, edge)?;
        if !edge.directed {
            neutral.push(value);
        } else if edge.from == coord_id {
            outgoing.push(value);
        } else {
            incoming.push(value);
        }
    }
    Ok(json!({"incoming": incoming, "outgoing": outgoing, "neutral": neutral}))
}

pub(crate) fn station_context(conn: &Connection, access: Access, row: &Placement) -> Result<Value> {
    let host = host_of(conn, row)?;
    let shape = shape_of(conn, &host.0)?;
    let editable = can_edit_host(conn, access, &host)?;
    let repeats = shape.coords_of_card(&row.card_id).len();
    let mut value = station_value(
        conn,
        access,
        &host,
        editable,
        row,
        &shape.coord[&row.coord_id],
        repeats,
    )?;
    value["parent"] = json!(
        row.parent_coord_id
            .as_ref()
            .map(|id| json!({"coord_id": id, "placement_id": id, "coord": shape.coord[id]}))
    );
    let edges = edges_of_tree(conn, host.key())?;
    if host.0.role == TreeRole::Connection || !edges.is_empty() {
        value["connections"] = incident_values(conn, &shape, &edges, &row.coord_id)?;
    }
    Ok(value)
}

pub(crate) fn list_side_mounts(conn: &Connection, _access: Access, args: &Value) -> Result<Value> {
    Ok(
        json!({"at": arg_str(args, "at")?, "mounts": [], "retired": true,
        "archive_revision": crate::link_index::revision(conn)?}),
    )
}

struct PlaneOccurrence {
    coord_id: String,
    tree_id: String,
    card_id: String,
    address: String,
    is_filing: bool,
}

pub(crate) fn tree_coordinates(conn: &Connection, tree: &str) -> Result<(Value, Value)> {
    if !coord::installed(conn) {
        return Ok((json!({}), json!({})));
    }
    let tree = coord::resolve_any_tree(conn, tree)?;
    projections(conn, &shape_of(conn, &tree)?)
}

fn projections(conn: &Connection, plane: &TreeShape) -> Result<(Value, Value)> {
    let mut stmt = conn.prepare_cached(
        "WITH plane_cards AS (SELECT DISTINCT card_id FROM coordinates WHERE tree_id = ?1)
         SELECT c.coord_id, c.tree_id, c.card_id, i.last_address, i.filing_coord_id = c.coord_id
         FROM plane_cards AS p JOIN card_identity AS i ON i.card_id = p.card_id AND i.status = 'live'
         JOIN coordinates AS c ON c.card_id = p.card_id",
    )?;
    let occurrences = stmt
        .query_map([&plane.tree.tree_id], |r| {
            Ok(PlaneOccurrence {
                coord_id: r.get(0)?,
                tree_id: r.get(1)?,
                card_id: r.get(2)?,
                address: r.get(3)?,
                is_filing: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let visible_cards: BTreeSet<_> = occurrences.iter().map(|row| row.card_id.as_str()).collect();
    let mut targets: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for id in &plane.order {
        let card_id = plane.by_id[id].card_id.as_str();
        if visible_cards.contains(card_id) {
            targets.entry(card_id).or_default().push(id);
        }
    }
    let mut shapes = BTreeMap::new();
    for occurrence in &occurrences {
        if occurrence.tree_id != plane.tree.tree_id && !shapes.contains_key(&occurrence.tree_id) {
            let tree = coord::tree_by_id(conn, &occurrence.tree_id)?
                .context("Coordinate host is missing")?;
            shapes.insert(occurrence.tree_id.clone(), shape_of(conn, &tree)?);
        }
    }
    let elsewhere = elsewhere_of(plane, &occurrences, &targets, &shapes);
    let crossings = crossings_of(conn, &occurrences, &targets, &shapes)?;
    Ok((elsewhere, crossings))
}

fn elsewhere_of(
    plane: &TreeShape,
    occurrences: &[PlaneOccurrence],
    targets: &BTreeMap<&str, Vec<&str>>,
    shapes: &BTreeMap<String, TreeShape>,
) -> Value {
    let mut elsewhere: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for occurrence in occurrences {
        let shape = if occurrence.tree_id == plane.tree.tree_id {
            if occurrence.is_filing || targets[occurrence.card_id.as_str()].len() == 1 {
                continue;
            }
            plane
        } else {
            &shapes[&occurrence.tree_id]
        };
        let id = &occurrence.coord_id;
        elsewhere.entry(&occurrence.address).or_default().push(json!({
            "coord_id": id, "placement_id": id, "coord": shape.coord[id], "host": shape.tree.root_address(),
            "tree": shape.tree.handle(), "tree_id": shape.tree.tree_id, "tree_role": shape.tree.role.id(),
            "is_filing": occurrence.is_filing, "gloss": shape.by_id[id].gloss,
        }));
    }
    for rows in elsewhere.values_mut() {
        rows.sort_by(|a, b| a["coord"].as_str().cmp(&b["coord"].as_str()));
    }
    json!(elsewhere)
}

fn crossings_of(
    conn: &Connection,
    occurrences: &[PlaneOccurrence],
    targets: &BTreeMap<&str, Vec<&str>>,
    shapes: &BTreeMap<String, TreeShape>,
) -> Result<Value> {
    let mut hosts: BTreeMap<&str, BTreeMap<&str, &PlaneOccurrence>> = BTreeMap::new();
    for row in occurrences {
        if shapes
            .get(&row.tree_id)
            .is_some_and(|shape| shape.tree.role == TreeRole::Connection)
        {
            hosts
                .entry(row.tree_id.as_str())
                .or_default()
                .insert(row.coord_id.as_str(), row);
        }
    }
    let mut crossings = serde_json::Map::new();
    for (tree_id, in_plane) in hosts {
        let shape = &shapes[tree_id];
        let mut stations = Vec::new();
        let mut lines = Vec::new();
        let mut exits_by_coord: BTreeMap<&str, usize> = BTreeMap::new();
        let outside = |id: &str| id != shape.root && !in_plane.contains_key(id);
        // Parent paths are visited before their children. Carry the nearest in-plane
        // ancestor and first edge through outside Coordinates in one traversal.
        let mut ancestors: BTreeMap<&str, (&str, EdgeKind, usize)> = BTreeMap::new();
        for id in shape.order.iter().map(String::as_str) {
            let row = &shape.by_id[id];
            if let Some(parent) = row.parent_coord_id.as_deref() {
                let ancestor = if in_plane.contains_key(parent) {
                    Some((parent, row.edge, 0))
                } else {
                    ancestors
                        .get(parent)
                        .map(|&(ancestor, edge, hops)| (ancestor, edge, hops + 1))
                };
                if let Some(ancestor) = ancestor {
                    ancestors.insert(id, ancestor);
                }
            }
            let Some(station) = in_plane.get(id) else {
                continue;
            };
            stations.push(json!({
                "coord_id": id, "placement_id": id, "coord": shape.coord[id],
                "card_id": station.card_id, "address": station.address,
                "target_coord_ids": targets[station.card_id.as_str()],
            }));
            if let Some(&(from, edge, hops)) = ancestors.get(id) {
                lines.push(json!({
                    "kind": "structure", "from": in_plane[from].address, "to": station.address,
                    "from_coord_id": from, "to_coord_id": id, "from_placement_id": from, "to_placement_id": id,
                    "from_coord": shape.coord[from], "to_coord": shape.coord[id], "edge": edge.id(), "hops": hops,
                }));
            }
            let exits = row.parent_coord_id.as_deref().is_some_and(outside) as usize
                + shape.direct.get(id).is_some_and(|child| outside(child)) as usize
                + shape.side.get(id).map_or(0, |children| {
                    children.iter().filter(|child| outside(child)).count()
                });
            exits_by_coord.insert(id, exits);
        }
        for edge in edges_of_tree(conn, tree_id)? {
            let from = edge.from.as_str();
            let to = edge.to.as_str();
            match (in_plane.get(from), in_plane.get(to)) {
                (Some(a), Some(b)) => lines.push(json!({
                    "kind": "connection", "from": a.address, "to": b.address,
                    "from_coord_id": from, "to_coord_id": to, "from_placement_id": from, "to_placement_id": to,
                    "from_coord": shape.coord[from], "to_coord": shape.coord[to],
                    "direction": if edge.directed { "directed" } else { "neutral" },
                    "relation_label": edge.label, "relation_kind": edge.kind, "connection_id": edge.connection_id,
                })),
                (Some(a), None) if outside(to) => *exits_by_coord.entry(a.coord_id.as_str()).or_default() += 1,
                (None, Some(b)) if outside(from) => *exits_by_coord.entry(b.coord_id.as_str()).or_default() += 1,
                _ => {}
            }
        }
        let mut exits: BTreeMap<&str, usize> = BTreeMap::new();
        for (&id, &count) in &exits_by_coord {
            if count > 0 {
                *exits.entry(in_plane[id].address.as_str()).or_default() += count;
            }
        }
        let (title, untitled) = root_title(conn, &shape.tree)?;
        crossings.insert(shape.tree.handle(), json!({
            "tree": shape.tree.handle(), "tree_id": tree_id, "title": title, "untitled": untitled,
            "workspace": shape.tree.workspace.id(), "stations": stations, "lines": lines,
            "exits": exits, "exits_by_coord": exits_by_coord,
        }));
    }
    Ok(Value::Object(crossings))
}
