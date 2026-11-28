//! The Session at a Coordinate (docs/multi-coordinate-cards-proposal.md §8.9).
//! The Pointer retains the Coordinate ID while the path is derived from the current tree.
//! Coordinate writes use the shared owner operations and their confirmation plans.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;

use super::ViewLine;
use super::command::{CardSummary, CommandResult, Execution, Interaction, Listing, Pointer, SessionState};
use crate::coord::{self, CoordinateRow, EdgeKind, TreeRole};

/// Run one owner write through the shared operations: a plan asks for its confirmation word first.
fn write(root: &Path, interaction: &mut dyn Interaction, tool: &str, args: Value) -> Result<Value> {
    let mut args = args;
    let mut first = crate::connection::run(root, crate::Access::Owner, tool, &args)?;
    if first.get("applied").and_then(Value::as_bool) != Some(false) {
        return Ok(first);
    }
    // A gloss whose predecessor changes is shown in the plan; confirming the plan keeps it as
    // reviewed (change it afterwards with edit-gloss). The decision is part of the request, so the
    // plan is made again with it before it is confirmed.
    let review = first["effects"]["gloss_review"].as_array().cloned().unwrap_or_default();
    if !review.is_empty() {
        let mut updates = serde_json::Map::new();
        for item in &review {
            if let (Some(id), Some(gloss)) = (item["coord_id"].as_str(), item["gloss"].as_str()) {
                updates.insert(id.to_string(), json!(gloss));
            }
        }
        args["gloss_updates"] = Value::Object(updates);
        // Plan again with the decisions; a preview never writes. Once the owner has started a
        // review, nothing applies without the confirmation word: a plan that no longer asks for
        // one changed under the review, and the owner runs the command again.
        // The decisions answer the glosses the owner saw: a plan made at another revision (a gloss
        // edited meanwhile) is no longer the one reviewed.
        let reviewed = first["expected_revision"].clone();
        let mut preview = args.clone();
        preview["preview"] = json!(true);
        first = crate::connection::run(root, crate::Access::Owner, tool, &preview)?;
        if first.get("confirm").and_then(Value::as_str).is_none() || first["expected_revision"] != reviewed {
            bail!("the plan changed during the gloss review; nothing was written, run the command again");
        }
    }
    let word = first["confirm"].as_str().context("plan without a confirmation word")?.to_string();
    let lines: Vec<String> = first["lines"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    interaction.confirm(&word, &lines)?;
    let mut confirmed = args;
    confirmed["confirm"] = json!(word);
    confirmed["plan_hash"] = first["plan_hash"].clone();
    confirmed["expected_revision"] = first["expected_revision"].clone();
    crate::connection::run(root, crate::Access::Owner, tool, &confirmed)
}

fn tree_of(conn: &Connection, row: &CoordinateRow) -> Result<coord::TreeShape> {
    let crate::connection::Host(tree) = crate::connection::host_of(conn, row)?;
    coord::load_tree_shape(conn, &tree)
}

/// Run coordinate commands. At a Card, coordinate writes start from its Filing Coordinate.
pub(super) fn execute(
    root: &Path,
    state: &mut SessionState,
    parts: &[&str],
    line: &str,
    interaction: &mut dyn Interaction,
) -> Result<Option<Execution>> {
    let navigated = || Ok(Some(Execution::Continue(CommandResult::Navigated)));
    let changed = || Ok(Some(Execution::Continue(CommandResult::Changed)));
    // Global commands can run even after the current Coordinate has been detached.
    if matches!(
        parts.first().copied(),
        None | Some("go" | "root" | "ws" | "back" | "fwd" | "q" | "help" | "stats" | "status" | "lsbk" | "t" | "l")
    ) {
        return Ok(None);
    }
    let pointer = state.pointer().clone();
    match (&pointer, parts) {
        (Pointer::Card(address), ["places"]) => {
            return Ok(Some(places(root, json!({"at": address}))?));
        }
        (Pointer::Card(address), ["promote", to]) => {
            let promoted = write(root, interaction, "promote_card", json!({"at": address, "to": to}))?;
            let address = promoted["address"].as_str().context("promotion returned no filing address")?;
            state.set_pointer(Pointer::Card(address.to_string()));
            return changed();
        }
        (Pointer::Card(_), ["add-coord" | "detach-coord", ..]) | (Pointer::Station(_), _) => {}
        _ => return Ok(None),
    }
    let conn = crate::open_db(root)?;
    let here = match pointer {
        Pointer::Station(id) => coord::resolve_coord(&conn, &id)?,
        Pointer::Card(address) => {
            let card_id = coord::card_id_of(&conn, &address)?;
            let identity = coord::identity(&conn, &card_id)?.context("unknown Card identity")?;
            crate::connection::filing_row(&conn, &identity)?
        }
        Pointer::Root => return Ok(None),
    };
    let id = &here.coord_id;
    let open_home = || {
        coord::address_of_card_id(&conn, &here.card_id)?
            .context("missing mount: the Card of this station was deleted at its home")
    };
    match parts {
        ["n"] => {
            let shape = tree_of(&conn, &here)?;
            let next = shape.direct.get(id).context("this station has no next station")?;
            state.set_pointer(Pointer::Station(next.clone()));
            navigated()
        }
        ["p"] | ["up"] => {
            let parent = here.parent_coord_id.clone().context("the root station has no predecessor")?;
            state.set_pointer(Pointer::Station(parent));
            navigated()
        }
        ["b"] | ["b", _] => {
            let shape = tree_of(&conn, &here)?;
            let sides = shape.side.get(id).map(Vec::as_slice).unwrap_or_default();
            let chosen = match parts {
                ["b", k] => {
                    let k: usize = k.parse().context("usage: b [<k>]")?;
                    sides.get(k.wrapping_sub(1)).cloned().with_context(|| format!("this station has {} Side branch(es)", sides.len()))?
                }
                _ => match sides {
                    [] => bail!("this station has no Side branch"),
                    [one] => one.clone(),
                    many => bail!(
                        "this station has {} Side branches; choose one with b <k>: {}",
                        many.len(),
                        many.iter().enumerate().map(|(i, s)| format!("{} {}", i + 1, shape.coord[s])).collect::<Vec<_>>().join(", ")
                    ),
                },
            };
            state.set_pointer(Pointer::Station(chosen));
            navigated()
        }
        ["home"] => {
            state.set_pointer(Pointer::Card(open_home()?));
            navigated()
        }
        ["places"] => {
            Ok(Some(places(root, json!({"card_id": here.card_id}))?))
        }
        ["conns"] => {
            let listed = crate::connection::list_connections(&conn, &json!({"tree": here.tree_id, "coord_id": id}))?;
            let mut cards = Vec::new();
            for (group, label) in [("incoming", "in"), ("outgoing", "out"), ("neutral", "with")] {
                for c in listed[group].as_array().into_iter().flatten() {
                    cards.push(CardSummary {
                        address: c["connection_id"].as_str().unwrap_or_default().to_string(),
                        title: format!("{label}: {} ({})", c["sentence"].as_str().unwrap_or_default(), c["provenance"].as_str().unwrap_or_default()),
                        bookmark: None,
                    });
                }
            }
            Ok(Some(Execution::Continue(CommandResult::List(Listing {
                heading: format!("connections of {}: {}", coord::coordinate_path(&conn, &here)?, cards.len()),
                cards,
            }))))
        }
        ["e"] => {
            // Editing at a station edits the Card at its home; the Pointer stays on the station.
            state.set_pointer(Pointer::Card(open_home()?));
            let result = super::command::execute_edit(root, state, interaction);
            state.set_pointer(Pointer::Station(id.clone()));
            result.map(Some)
        }
        [verb @ ("place" | "add-coord"), target, rest @ ..] => {
            let crate::connection::Host(tree) = crate::connection::host_of(&conn, &here)?;
            let (edge, gloss) = match rest {
                ["side", gloss @ ..] => ("side", gloss.join(" ")),
                gloss => ("direct", gloss.join(" ")),
            };
            let mut args = json!({"tree": tree.tree_id, "parent_coord_id": id, "edge": edge, "target_at": target});
            if !gloss.trim().is_empty() {
                args["gloss"] = json!(gloss.trim());
            }
            let tool = if *verb == "place" && tree.role == TreeRole::Connection {
                "place_card"
            } else {
                "add_coordinate"
            };
            let placed = write(root, interaction, tool, args)?;
            let new = placed["coord_id"].as_str().context("addition returned no Coordinate ID")?;
            state.set_pointer(Pointer::Station(new.to_string()));
            changed()
        }
        [verb @ ("unplace" | "detach-coord")] | [verb @ ("unplace" | "detach-coord"), _] => {
            let mut args = json!({"coord_id": id});
            if let [_, policy] = parts {
                args["children"] = json!(policy);
            }
            let tool = if *verb == "unplace" { "unplace" } else { "detach_coordinate" };
            write(root, interaction, tool, args)?;
            let parent = here.parent_coord_id.context("detached Coordinate had no parent")?;
            state.set_pointer(Pointer::Station(parent));
            changed()
        }
        ["refile"] => {
            let identity = coord::identity(&conn, &here.card_id)?.context("unknown Card identity")?;
            if identity.filing_coord_id.as_deref() == Some(id.as_str()) {
                bail!("this is already the Card's Filing Coordinate");
            }
            if Some(here.host_workspace) != identity.owner_workspace {
                bail!("refiling stays inside the Card's owner Workspace");
            }
            write(root, interaction, "refile_card", json!({"card_id": here.card_id, "to_coord_id": id}))?;
            changed()
        }
        ["connect", other, rest @ ..] => {
            let mut args = json!({"tree": here.tree_id, "from_coord_id": id, "to_coord_id": other});
            let rest: Vec<&str> = match rest {
                ["directed", label @ ..] => {
                    args["direction"] = json!("directed");
                    label.to_vec()
                }
                label => label.to_vec(),
            };
            if !rest.is_empty() {
                args["relation_label"] = json!(rest.join(" "));
            }
            write(root, interaction, "connect_placements", args)?;
            changed()
        }
        ["edit-conn", connection, fields @ ..] if !fields.is_empty() => {
            let mut args = json!({"connection_id": connection});
            // `field=value` pairs; a value may hold spaces up to the next `field=`.
            let text = line.split_once(connection).map(|(_, rest)| rest.trim()).unwrap_or_default();
            for (field, value) in key_values(text) {
                let name = match field.as_str() {
                    "label" | "relation_label" => "relation_label",
                    "kind" | "relation_kind" => "relation_kind",
                    "note" => "note",
                    "direction" => "direction",
                    "reverse" => {
                        args["reverse_endpoints"] = json!(value == "yes" || value == "true");
                        continue;
                    }
                    other => bail!("unknown connection field `{other}`; use label, kind, note, direction or reverse"),
                };
                args[name] = if value == "none" { Value::Null } else { json!(value) };
            }
            write(root, interaction, "edit_connection", args)?;
            changed()
        }
        ["unconnect", connection] => {
            write(root, interaction, "delete_connection", json!({"connection_id": connection}))?;
            changed()
        }
        ["edit-gloss", ..] => {
            let text = line.trim_start().strip_prefix("edit-gloss").unwrap_or_default().trim();
            let gloss = if text == "none" || text.is_empty() { Value::Null } else { json!(text) };
            write(root, interaction, "edit_placement", json!({"coord_id": id, "gloss": gloss}))?;
            changed()
        }
        ["move-place", parent] | ["move-place", parent, _] => {
            let edge = if parts.len() == 3 && parts[2] == "side" { "side" } else { "direct" };
            let crate::connection::Host(tree) = crate::connection::host_of(&conn, &here)?;
            let tool = if tree.role == TreeRole::Connection { "move_placement" } else { "move_coordinate" };
            let args = json!({"coord_id": id, "destination": {"parent_coord_id": parent, "edge": edge}});
            write(root, interaction, tool, args)?;
            changed()
        }
        ["promote", to] => {
            write(root, interaction, "promote_card", json!({"card_id": here.card_id, "to": to}))?;
            changed()
        }
        ["del", ..] | ["ins", ..] | ["mv", ..] | ["in", ..] | ["out", ..] | ["ord", ..] | ["ty", ..] | ["lv", ..] | ["mf", ..] | ["lt", ..] => bail!(
            "this is a station (an occurrence of the Card); `{}` acts on a Card at its home. Use `home` first, or the station verbs: unplace, move-place, connect",
            parts[0]
        ),
        _ => Ok(None),
    }
}

fn key_values(text: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for word in text.split_whitespace() {
        match word.split_once('=') {
            Some((key, value)) if !key.is_empty() && key.chars().all(|c| c.is_ascii_alphabetic() || c == '_') => {
                out.push((key.to_string(), value.to_string()));
            }
            _ => {
                if let Some(last) = out.last_mut() {
                    last.1.push(' ');
                    last.1.push_str(word);
                }
            }
        }
    }
    out
}

fn places(root: &Path, args: Value) -> Result<Execution> {
    let listed = crate::connection::run(root, crate::Access::Owner, "list_coordinates", &args)?;
    let address = listed["home"].as_str().or_else(|| listed["last_address"].as_str()).unwrap_or_default();
    let cards = listed["coordinates"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| CardSummary {
            address: p["coord"].as_str().unwrap_or_default().to_string(),
            title: format!(
                "in {}{} {}",
                p["host"].as_str().unwrap_or_default(),
                p["gloss"].as_str().map(|g| format!(" · {g}")).unwrap_or_default(),
                if p["is_filing"] == true { "[filing]" } else { "[secondary]" },
            ),
            bookmark: p["bookmark"].as_str().map(str::to_string),
        })
        .collect::<Vec<_>>();
    Ok(Execution::Continue(CommandResult::List(Listing {
        heading: format!("Coordinates of {address}: {}", cards.len()),
        cards,
    })))
}

/// The header of a station view, and the home address of its Card (none for a Missing mount).
pub(super) fn header_lines(conn: &Connection, id: &str) -> Result<(Vec<ViewLine>, Option<String>)> {
    let here = coord::resolve_coord(conn, id)?;
    let shape = tree_of(conn, &here)?;
    let tree = &shape.tree;
    let coord_text = &shape.coord[id];
    let identity = coord::identity(conn, &here.card_id)?.context("unknown Card")?;
    let home = coord::address_of_card_id(conn, &here.card_id)?;
    let (title, _) = crate::connection::root_title(conn, tree)?;
    let lock = if crate::coord_lock::host_locked(conn, &tree.tree_id)? { " [locked]" } else { "" };
    let filing = if identity.filing_coord_id.as_deref() == Some(id) { "[filing]" } else { "[secondary]" };
    let role = tree_role_label(tree.role);
    let title = if title.is_empty() { String::new() } else { format!(" · {title}") };
    let mut lines = vec![ViewLine::Plain(format!(
        "station: {coord_text} in {}{title} · {role} tree {filing}{lock}",
        tree.root_address(),
    ))];
    let neighbor = |label: &str, id: &str| ViewLine::Entry {
        address: format!("{label}: {}", shape.coord[id]),
        rest: format!(" {}", title_of(conn, &shape.by_id[id])),
        bookmark: Some(format!("place:{id}")),
    };
    if let Some(parent) = &here.parent_coord_id {
        lines.push(neighbor("prev", parent));
    }
    if let Some(next) = shape.direct.get(id) {
        lines.push(neighbor("next", next));
    }
    for side in shape.side.get(id).into_iter().flatten() {
        lines.push(neighbor("side", side));
    }
    let edges = crate::connection::edges_of_tree(conn, &tree.tree_id)?;
    let mine = edges.iter().filter(|e| e.from == id || e.to == id).count();
    if mine > 0 {
        lines.push(ViewLine::Plain(format!("connections: {mine} (conns lists them)")));
    }
    if let Some(gloss) = &here.gloss {
        lines.push(ViewLine::Plain(format!("gloss: {gloss}")));
    }
    if here.edge == EdgeKind::Root {
        lines.push(ViewLine::Plain(format!("the root of this {role} tree")));
    }
    match &home {
        Some(home) => {
            let filing = crate::connection::filing_row(conn, &identity)?;
            let crate::connection::Host(home_tree) = crate::connection::host_of(conn, &filing)?;
            lines.push(ViewLine::Plain(format!(
                "home: {home} · {} · {}",
                home_tree.role.id(),
                identity.owner_workspace.map(crate::Workspace::id).unwrap_or("unknown")
            )));
            if let Some(drift) = crate::coord_lock::drift(conn, id)?
                && drift["differs"] == true
            {
                lines.push(ViewLine::Plain(format!(
                    "source differs from lock-time version{}",
                    if drift["reviewed"] == true { " (reviewed)" } else { "" }
                )));
            }
        }
        None => lines.push(ViewLine::Plain(format!(
            "missing mount: `{}` ({}) was deleted at its home; repair it with rebind or unplace",
            identity.last_address, identity.last_title
        ))),
    }
    lines.push(ViewLine::Hint("walk with n, p, b. Use home to open the Card and back to return here.".to_string()));
    Ok((lines, home))
}

fn tree_role_label(role: TreeRole) -> &'static str {
    match role {
        TreeRole::Connection => "Connection",
        TreeRole::Topic => "Topic",
        TreeRole::Literature => "Literature",
    }
}

fn title_of(conn: &Connection, p: &CoordinateRow) -> String {
    coord::identity(conn, &p.card_id)
        .ok()
        .flatten()
        .map(|i| match i.card_row.filter(|_| i.live) {
            Some(row) => crate::connection::card_by_row(conn, row)
                .ok()
                .and_then(|c| crate::parse_card_text(&c.text).ok())
                .map(|p| p.title)
                .unwrap_or(i.last_title),
            None => format!("{} (missing)", i.last_title),
        })
        .unwrap_or_default()
}

/// `ls` at a station: the live Cards of its tree's stations, in outline order.
pub(super) fn listing(conn: &Connection, id: &str) -> Result<(String, Vec<String>)> {
    let here = coord::resolve_coord(conn, id)?;
    let shape = tree_of(conn, &here)?;
    let mut addresses = Vec::new();
    for id in &shape.order {
        if let Some(address) = coord::address_of_card_id(conn, &shape.by_id[id].card_id)? {
            addresses.push(address);
        }
    }
    let role = tree_role_label(shape.tree.role);
    Ok((format!("{role} tree {}: {} stations", shape.tree.root_address(), shape.order.len()), addresses))
}
