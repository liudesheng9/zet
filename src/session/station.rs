//! The Session at a station (docs/multi-tree-coordinates-proposal.md §9.2): the Pointer may stand on
//! one occurrence of a Card (a station of a Connection tree or a Topic-side mount), named by its
//! placement ID so that its coordinate is derived afresh at every view. `n`, `p` and `b` walk the
//! tree; `home` opens the Card at its home and `back` returns to the exact station; `places` and
//! `conns` list the occurrences of the Card and the connections of the station; the mutation verbs
//! go through the shared Connection-tree operations as the owner, with the same plans.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;

use super::command::{CardSummary, CommandResult, Execution, Interaction, Listing, Pointer, SessionState};
use crate::coord::{self, EdgeKind};

/// Run one owner write through the shared operations: a plan asks for its confirmation word first.
fn write(root: &Path, interaction: &mut dyn Interaction, tool: &str, args: Value) -> Result<Value> {
    let first = crate::connection::run(root, crate::Access::Owner, tool, &args)?;
    if first.get("applied").and_then(Value::as_bool) != Some(false) {
        return Ok(first);
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

fn station(conn: &Connection, id: &str) -> Result<coord::Placement> {
    coord::placement(conn, id)?.with_context(|| format!("placement removed: station `{id}` no longer exists"))
}

fn tree_of(conn: &Connection, p: &coord::Placement) -> Result<coord::TreeShape> {
    let tree = coord::tree_by_id(conn, &p.host_tree_key)?.context("this station is a Topic-side mount; it has no tree to walk")?;
    coord::load_tree_shape(conn, &tree)
}

/// The station verbs, and `places`/`promote` at a home Card. `None` lets the ordinary commands run.
pub(super) fn execute(
    root: &Path,
    state: &mut SessionState,
    parts: &[&str],
    line: &str,
    interaction: &mut dyn Interaction,
) -> Result<Option<Execution>> {
    let navigated = || Ok(Some(Execution::Continue(CommandResult::Navigated)));
    let changed = || Ok(Some(Execution::Continue(CommandResult::Changed)));
    let at_station = match state.pointer() {
        Pointer::Station(id) => Some(id.clone()),
        _ => None,
    };
    // At a home Card: the occurrence listing and promotion.
    if at_station.is_none() {
        if let Pointer::Card(address) = state.pointer().clone() {
            match parts {
                ["places"] => return Ok(Some(places(root, &address)?)),
                ["promote", to] => {
                    write(root, interaction, "promote_card", json!({"at": address, "to": to}))?;
                    let conn = crate::open_db(root)?;
                    state.set_pointer(Pointer::Card(coord::canonical(&conn, &address)?.unwrap_or_else(|| (*to).to_string())));
                    return changed();
                }
                _ => {}
            }
        }
        return Ok(None);
    }
    let id = at_station.expect("station");
    let conn = crate::open_db(root)?;
    let here = station(&conn, &id)?;
    let home = coord::address_of_card_id(&conn, &here.card_id)?;
    match parts {
        ["n"] => {
            let shape = tree_of(&conn, &here)?;
            let next = shape.direct.get(&id).context("this station has no next station")?;
            state.set_pointer(Pointer::Station(next.clone()));
            navigated()
        }
        ["p"] | ["up"] => {
            let parent = here.parent_id.clone().context("the root station has no predecessor")?;
            state.set_pointer(Pointer::Station(parent));
            navigated()
        }
        ["b"] | ["b", _] => {
            let shape = tree_of(&conn, &here)?;
            let sides = shape.side.get(&id).cloned().unwrap_or_default();
            let chosen = match parts {
                ["b", k] => {
                    let k: usize = k.parse().context("usage: b [<k>]")?;
                    sides.get(k.wrapping_sub(1)).cloned().with_context(|| format!("this station has {} Side branch(es)", sides.len()))?
                }
                _ => match sides.as_slice() {
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
            let home = home.context("missing mount: the Card of this station was deleted at its home")?;
            state.set_pointer(Pointer::Card(home));
            navigated()
        }
        ["places"] => {
            let home = home.context("missing mount: the Card of this station was deleted at its home")?;
            Ok(Some(places(root, &home)?))
        }
        ["conns"] => {
            let shape = tree_of(&conn, &here)?;
            let listed = crate::connection::list_connections(&conn, &json!({"tree": shape.tree.handle(), "placement_id": id}))?;
            let mut cards = Vec::new();
            for (group, label) in [("incoming", "in"), ("outgoing", "out"), ("neutral", "with")] {
                for c in listed[group].as_array().into_iter().flatten() {
                    cards.push(CardSummary {
                        address: c["connection_id"].as_str().unwrap_or_default().to_string(),
                        title: format!("{label}: {} ({})", c["sentence"].as_str().unwrap_or_default(), c["provenance"].as_str().unwrap_or_default()),
                    });
                }
            }
            Ok(Some(Execution::Continue(CommandResult::List(Listing {
                heading: format!("connections of {}: {}", shape.coord[&id], cards.len()),
                cards,
            }))))
        }
        ["e"] => {
            let home = home.context("missing mount: the Card of this station was deleted at its home")?;
            // Editing at a station edits the Card at its home; the Pointer stays on the station.
            state.set_pointer(Pointer::Card(home));
            let result = super::command::execute_edit(root, state, interaction);
            state.set_pointer(Pointer::Station(id));
            result.map(Some)
        }
        ["place", target, rest @ ..] => {
            let shape = tree_of(&conn, &here)?;
            let (edge, gloss) = match rest {
                ["side", gloss @ ..] => ("side", gloss.join(" ")),
                gloss => ("direct", gloss.join(" ")),
            };
            let mut args = json!({"host": shape.tree.handle(), "parent_placement_id": id, "edge": edge, "target_at": target});
            if !gloss.trim().is_empty() {
                args["gloss"] = json!(gloss.trim());
            }
            let placed = write(root, interaction, "place_card", args)?;
            if let Some(new) = placed["placement_id"].as_str() {
                state.set_pointer(Pointer::Station(new.to_string()));
            }
            changed()
        }
        ["unplace"] | ["unplace", _] => {
            let mut args = json!({"placement_id": id});
            if let ["unplace", policy] = parts {
                args["children"] = json!(policy);
            }
            let parent = here.parent_id.clone();
            write(root, interaction, "unplace", args)?;
            state.set_pointer(match (parent, home) {
                (Some(parent), _) => Pointer::Station(parent),
                (None, Some(home)) => Pointer::Card(home),
                (None, None) => Pointer::Root,
            });
            changed()
        }
        ["connect", other, rest @ ..] => {
            let shape = tree_of(&conn, &here)?;
            let mut args = json!({"tree": shape.tree.handle(), "from_placement_id": id, "to_placement_id": other});
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
            write(root, interaction, "edit_placement", json!({"placement_id": id, "gloss": gloss}))?;
            changed()
        }
        ["move-place", parent] | ["move-place", parent, _] => {
            let edge = if parts.len() == 3 && parts[2] == "side" { "side" } else { "direct" };
            let args = json!({"placement_id": id, "destination": {"parent": parent, "edge": edge}});
            match write(root, interaction, "move_placement", args.clone()) {
                Ok(_) => changed(),
                Err(error) if format!("{error:#}").contains("gloss review required") => {
                    bail!("{error:#}; review the glosses with edit-gloss after the move, or clear them first")
                }
                Err(error) => Err(error),
            }
        }
        ["promote", to] => {
            let home = home.context("missing mount: the Card of this station was deleted at its home")?;
            write(root, interaction, "promote_card", json!({"at": home, "to": to}))?;
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

fn places(root: &Path, address: &str) -> Result<Execution> {
    let conn = crate::open_db(root)?;
    let listed = crate::connection::list_placements(&conn, crate::Access::Owner, &json!({"at": address}))?;
    let cards = listed["placements"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| CardSummary {
            address: p["coord"].as_str().unwrap_or_default().to_string(),
            title: format!(
                "in {}{}",
                p["host"].as_str().unwrap_or_default(),
                p["gloss"].as_str().map(|g| format!(" · {g}")).unwrap_or_default()
            ),
        })
        .collect::<Vec<_>>();
    Ok(Execution::Continue(CommandResult::List(Listing {
        heading: format!("{address} stands in {} other place(s)", cards.len()),
        cards,
    })))
}

/// The header of a station view, and the home address of its Card (none for a Missing mount).
pub(super) fn header_lines(conn: &Connection, id: &str) -> Result<(Vec<String>, Option<String>)> {
    let here = station(conn, id)?;
    let host = crate::connection::host_of(conn, &here)?;
    let coord_text = coord::coordinate_of(conn, &here)?;
    let identity = coord::identity(conn, &here.card_id)?.context("unknown Card")?;
    let home = coord::address_of_card_id(conn, &here.card_id)?;
    let mut lines = Vec::new();
    match &host {
        crate::connection::Host::Connection(tree) => {
            let summary = crate::connection::tree_summary(conn, crate::Access::Owner, tree)?;
            lines.push(format!(
                "station: {coord_text} in {} · {}{}",
                tree.handle(),
                summary["title"].as_str().unwrap_or_default(),
                if summary["locked"] == true { " [locked]" } else { "" }
            ));
            let shape = coord::load_tree_shape(conn, tree)?;
            if let Some(parent) = &here.parent_id {
                lines.push(format!("prev: {} {}", shape.coord[parent], title_of(conn, &shape.by_id[parent])));
            }
            if let Some(next) = shape.direct.get(id) {
                lines.push(format!("next: {} {}", shape.coord[next], title_of(conn, &shape.by_id[next])));
            }
            for side in shape.side.get(id).into_iter().flatten() {
                lines.push(format!("side: {} {}", shape.coord[side], title_of(conn, &shape.by_id[side])));
            }
            let edges = crate::connection::edges_of_tree(conn, &tree.tree_id)?;
            let mine = edges.iter().filter(|e| e.from == id || e.to == id).count();
            if mine > 0 {
                lines.push(format!("connections: {mine} (conns lists them)"));
            }
        }
        crate::connection::Host::Topic { root, .. } => {
            lines.push(format!("station: {coord_text} · mounted beside a Card of Topic {root}"));
        }
    }
    if let Some(gloss) = &here.gloss {
        lines.push(format!("gloss: {gloss}"));
    }
    if here.edge == EdgeKind::Root {
        lines.push("the root of this Connection tree".to_string());
    }
    match &home {
        Some(home) => {
            let card = crate::load_card(conn, home)?;
            lines.push(format!(
                "home: {home} · {} · {}",
                crate::coord::TreeRole::of(&card).id(),
                card.workspace.id()
            ));
            if let Some(drift) = crate::coord_lock::drift(conn, id)?
                && drift["differs"] == true
            {
                lines.push(format!(
                    "source differs from lock-time version{}",
                    if drift["reviewed"] == true { " (reviewed)" } else { "" }
                ));
            }
        }
        None => lines.push(format!(
            "missing mount: `{}` ({}) was deleted at its home; repair it with rebind or unplace",
            identity.last_address, identity.last_title
        )),
    }
    lines.push("walk with n, p, b; home opens the Card; back returns here".to_string());
    Ok((lines, home))
}

fn title_of(conn: &Connection, p: &coord::Placement) -> String {
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
    let here = station(conn, id)?;
    let Ok(shape) = tree_of(conn, &here) else {
        let home = coord::address_of_card_id(conn, &here.card_id)?.into_iter().collect();
        return Ok(("Topic-side mount".to_string(), home));
    };
    let addresses: Vec<String> = shape
        .order
        .iter()
        .filter_map(|p| coord::address_of_card_id(conn, &shape.by_id[p].card_id).ok().flatten())
        .collect();
    Ok((format!("Connection tree {}: {} stations", shape.tree.handle(), shape.order.len()), addresses))
}
