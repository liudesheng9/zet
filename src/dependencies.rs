//! The cross-book Dependency Lens and one-time seeding (docs/multi-tree-coordinates-proposal.md
//! §8.3, Phase 3).
//!
//! `get_dependencies` is a read bound to one archive revision. It reads the Link index for the
//! authored reasoning Links (`rests-on` by default) of one Card (scope `card`) or of one Model or
//! Math unit's statement, premises and proof or derivation steps (scope `unit`), re-parses each
//! carrier Card's authored body to give the exact invoking sentence and macro (byte ranges into the
//! body, and the occurrence's `ord` for a renderer), and answers the prerequisites, the dependents,
//! the cycles, the reach-in diagnostics (a Link into another unit's proof part rather than its
//! statement) and a prerequisite-first seed order. Only authored Card-body evidence enters the
//! closure and the order; a Connection tree's working connections are an overlay that is compared,
//! never followed.
//!
//! Seeding creates a Connection tree once from a Hub Card's members, a saved Spread, a Dependency
//! Lens order or an ordered selection: one Direct chain of mounts, each Card once, and no tree
//! connection.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::coord;
use crate::{Access, CardType, Workspace};

const DEFAULT_MAX_NODES: usize = 200;
const MAX_OCCURRENCES: usize = 5000;

fn opt_str<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
    args.get(name).and_then(Value::as_str).filter(|s| !s.trim().is_empty())
}

/// A sentence ends at CJK or ASCII end punctuation, a line break, or a period followed by a space
/// (the Model board's rule).
fn ends_sentence(body: &str, index: usize, ch: char) -> bool {
    ['。', '！', '？', '!', '?', '；', ';', '\n'].contains(&ch)
        || (ch == '.' && body[index + 1..].starts_with(char::is_whitespace))
}

/// The byte range of the sentence holding `at` in `body`, without surrounding blanks.
pub(crate) fn sentence_range(body: &str, at: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let start = body[..at.start]
        .char_indices()
        .rev()
        .find(|&(index, ch)| ends_sentence(body, index, ch))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let end = body[at.end..]
        .char_indices()
        .find(|&(index, ch)| ends_sentence(body, at.end + index, ch))
        .map_or(body.len(), |(index, ch)| at.end + index + if ch == '\n' { 0 } else { ch.len_utf8() });
    let slice = &body[start..end];
    let lead = slice.len() - slice.trim_start().len();
    let trail = slice.len() - slice.trim_end().len();
    (start + lead)..(end - trail).max(start + lead)
}

/// The Model unit a Card belongs to: itself for a Model head, the nearest head it is filed under
/// for a part, none otherwise.
fn unit_head(conn: &Connection, card: &crate::Card) -> Option<String> {
    if card.card_type != CardType::Model {
        return None;
    }
    if card.facet.is_none() {
        return Some(card.address.clone());
    }
    crate::model::find_head(crate::parent_location(&card.address), |address| {
        crate::load_card(conn, address)
            .ok()
            .map(|c| (c.card_type, c.facet.is_some()))
    })
}

/// The carriers of a unit: its head and its parts, exactly as the Model report counts them (filed
/// under the head, Direct or Side, and named by the head; never a `**引入**` import).
fn unit_carriers(conn: &Connection, head: &str) -> Result<Vec<String>> {
    let head_card = crate::load_card(conn, head)?;
    let mut carriers = vec![head.to_string()];
    for part in crate::model::parts_in_db(conn, &head_card)? {
        if !carriers.contains(&part) {
            carriers.push(part);
        }
    }
    Ok(carriers)
}

/// Whether a Model part carries a proof or derivation step (`证明：…`, `推导…`), not a statement.
fn is_proof_part(facet: &crate::model::Facet) -> bool {
    use crate::math::Role;
    crate::model::is_derivation(facet)
        || matches!(crate::math::role(facet), Some(Role::Step | Role::Plan | Role::AltProof | Role::Derivation))
}

#[derive(Clone)]
struct Node {
    address: String,
    title: String,
    card_type: String,
    form: Option<String>,
    workspace: Workspace,
    card_id: String,
    unit: Option<String>,
}

fn node_of(conn: &Connection, address: &str, unit_scope: bool) -> Result<(Node, Option<String>)> {
    let card = crate::load_card(conn, address)?;
    let head = unit_head(conn, &card);
    // In unit scope a Link into another unit's part stands for that unit; a Link into a proof or
    // derivation part rather than the statement is reported as reach-in.
    let (node_card, reached) = match (&head, unit_scope) {
        (Some(head), true) if head != &card.address => (crate::load_card(conn, head)?, Some(card.address.clone())),
        _ => (card, None),
    };
    let title = crate::parse_card_text(&node_card.text).map(|p| p.title).unwrap_or_default();
    let card_id = coord::card_id_of_row(conn, node_card.row_id)?;
    Ok((
        Node {
            address: node_card.address.clone(),
            title,
            card_type: node_card.card_type.id().to_string(),
            form: node_card.form.map(|f| f.id().to_string()),
            workspace: node_card.workspace,
            card_id,
            unit: unit_head(conn, &node_card),
        },
        reached,
    ))
}

fn node_value(node: &Node) -> Value {
    json!({
        "address": node.address,
        "title": node.title,
        "type": node.card_type,
        "form": node.form,
        "workspace": node.workspace.id(),
        "card_id": node.card_id,
        "unit": node.unit,
        "tree": crate::tree_id(&node.address),
    })
}

/// One authored occurrence: the carrier Card's `ord`-th Link names `target`.
struct Occurrence {
    carrier: String,
    ord: usize,
    target: String,
    kind: Option<String>,
}

fn occurrences_from(conn: &Connection, carrier: &str) -> Result<Vec<Occurrence>> {
    let mut stmt = conn.prepare_cached("SELECT ord, target, kind FROM links WHERE source = ?1 ORDER BY ord")?;
    let rows = stmt
        .query_map([carrier], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, Option<String>>(2)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .map(|(ord, target, kind)| Occurrence { carrier: carrier.to_string(), ord: ord as usize, target, kind })
        .collect())
}

fn occurrences_into(conn: &Connection, target: &str) -> Result<Vec<Occurrence>> {
    let mut out = Vec::new();
    let mut stmt = conn.prepare_cached("SELECT source, ord, kind FROM links WHERE target = ?1 ORDER BY source, ord")?;
    for spelling in coord::spellings(conn, target)? {
        for row in stmt.query_map([&spelling], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<String>>(2)?)))? {
            let (source, ord, kind) = row?;
            out.push(Occurrence { carrier: source, ord: ord as usize, target: spelling.clone(), kind });
        }
    }
    Ok(out)
}

/// The evidence of one occurrence, re-read from the carrier's authored body at this revision.
fn evidence(conn: &Connection, bodies: &mut BTreeMap<String, (String, String)>, occ: &Occurrence) -> Result<Value> {
    if !bodies.contains_key(&occ.carrier) {
        let card = crate::load_card(conn, &occ.carrier)?;
        let parsed = crate::parse_card_text(&card.text)?;
        let fp = crate::card::authored_fingerprint(&card)?;
        bodies.insert(occ.carrier.clone(), (parsed.body, fp));
    }
    let (body, fp) = &bodies[&occ.carrier];
    let links = crate::link::LinkBody::parse(body)?;
    let found = links
        .occurrences()
        .get(occ.ord)
        .filter(|o| o.target == occ.target)
        .with_context(|| format!("index_health: the Link index row {}#{} does not match the carrier body; run `zt status --check-links`", occ.carrier, occ.ord))?;
    let range = sentence_range(body, found.macro_range.clone());
    let carrier = crate::load_card(conn, &occ.carrier)?;
    let title = crate::parse_card_text(&carrier.text).map(|p| p.title).unwrap_or_default();
    Ok(json!({
        "carrier": occ.carrier,
        "carrier_title": title,
        "carrier_fp": fp,
        "ord": occ.ord,
        "target": occ.target,
        "kind": occ.kind,
        "sentence": &body[range.clone()],
        "sentence_byte_range": [range.start, range.end],
        "macro_byte_range": [found.macro_range.start, found.macro_range.end],
        "provenance": format!("In the Card · {} ({})", occ.carrier, title),
    }))
}

/// Tarjan's strongly connected components over `edges` (from → to).
fn components(nodes: &BTreeSet<String>, edges: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    struct State<'a> {
        edges: &'a BTreeMap<String, BTreeSet<String>>,
        index: usize,
        indices: BTreeMap<String, usize>,
        low: BTreeMap<String, usize>,
        stack: Vec<String>,
        on: BTreeSet<String>,
        out: Vec<Vec<String>>,
    }
    fn visit(s: &mut State, v: &str) {
        s.indices.insert(v.to_string(), s.index);
        s.low.insert(v.to_string(), s.index);
        s.index += 1;
        s.stack.push(v.to_string());
        s.on.insert(v.to_string());
        let next: Vec<String> = s.edges.get(v).map(|e| e.iter().cloned().collect()).unwrap_or_default();
        for w in next {
            if !s.indices.contains_key(&w) {
                visit(s, &w);
                let lw = s.low[&w];
                let lv = s.low[v];
                s.low.insert(v.to_string(), lv.min(lw));
            } else if s.on.contains(&w) {
                let iw = s.indices[&w];
                let lv = s.low[v];
                s.low.insert(v.to_string(), lv.min(iw));
            }
        }
        if s.low[v] == s.indices[v] {
            let mut comp = Vec::new();
            while let Some(w) = s.stack.pop() {
                s.on.remove(&w);
                comp.push(w.clone());
                if w == v {
                    break;
                }
            }
            comp.sort();
            s.out.push(comp);
        }
    }
    let mut s = State { edges, index: 0, indices: BTreeMap::new(), low: BTreeMap::new(), stack: Vec::new(), on: BTreeSet::new(), out: Vec::new() };
    for v in nodes {
        if !s.indices.contains_key(v) {
            visit(&mut s, v);
        }
    }
    s.out
}

/// `get_dependencies`.
pub(crate) fn get(conn: &Connection, args: &Value) -> Result<Value> {
    let at = opt_str(args, "at").context("missing string argument `at`")?;
    let (address, _) = crate::read::read_address(conn, at)?;
    let scope = opt_str(args, "scope").unwrap_or("card");
    let unit_scope = match scope {
        "card" => false,
        "unit" | "model" => true,
        other => bail!("unknown scope `{other}`; expected card or unit"),
    };
    let depth = args.get("depth").and_then(Value::as_u64).unwrap_or(1);
    if !(1..=3).contains(&depth) {
        bail!("depth is 1, 2 or 3");
    }
    let max_nodes = args.get("max_nodes").and_then(Value::as_u64).map(|n| n as usize).unwrap_or(DEFAULT_MAX_NODES).clamp(1, 2000);
    let include_terms = args.get("include_terms").and_then(Value::as_bool).unwrap_or(false);
    let kinds: BTreeSet<String> = match args.get("kinds").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        None => BTreeSet::from(["rests-on".to_string()]),
    };
    let counts = |kind: &Option<String>| -> Option<&'static str> {
        match kind.as_deref() {
            Some("term") => include_terms.then_some("vocabulary"),
            Some(k) if kinds.contains(k) => Some("reasoning"),
            None if kinds.contains("refers") => Some("reasoning"),
            _ => None,
        }
    };
    let revision = crate::link_index::revision(conn)?;
    let (focus, _) = node_of(conn, &address, unit_scope)?;
    let focus_card = crate::load_card(conn, &address)?;
    let focus_unit = if unit_scope { unit_head(conn, &focus_card) } else { None };
    let carriers_of = |conn: &Connection, node: &Node| -> Result<Vec<String>> {
        match (&node.unit, unit_scope) {
            (Some(head), true) if head == &node.address => unit_carriers(conn, head),
            _ => Ok(vec![node.address.clone()]),
        }
    };

    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    nodes.insert(focus.address.clone(), focus.clone());
    let mut bodies = BTreeMap::new();
    let mut edges: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
    let mut vocabulary = Vec::new();
    let mut reach_in = Vec::new();
    let mut truncated = false;
    let mut occurrences_seen = 0usize;

    // Prerequisites: breadth first up to `depth`, through authored reasoning Links only.
    let mut queue = VecDeque::from([(focus.address.clone(), 0u64)]);
    let mut expanded = BTreeSet::new();
    let mut frontier = BTreeSet::new();
    while let Some((node_address, level)) = queue.pop_front() {
        if !expanded.insert(node_address.clone()) {
            continue;
        }
        if level >= depth {
            frontier.insert(node_address.clone());
            continue;
        }
        let node = nodes[&node_address].clone();
        for carrier in carriers_of(conn, &node)? {
            for occ in occurrences_from(conn, &carrier)? {
                let Some(class) = counts(&occ.kind) else { continue };
                occurrences_seen += 1;
                // A Term use is vocabulary, never a reasoning edge or a cycle.
                let vocabulary_only = class == "vocabulary";
                if occurrences_seen > MAX_OCCURRENCES {
                    truncated = true;
                    break;
                }
                let Some(target_address) = coord::canonical(conn, &occ.target)? else { continue };
                let (target, reached) = node_of(conn, &target_address, unit_scope)?;
                if target.address == node.address {
                    // A Link between two carriers of one unit is internal; a Card that cites itself
                    // with a reasoning Link is a genuine self-loop and reaches the cycle report.
                    if target_address == occ.carrier && !vocabulary_only {
                        let mut ev = evidence(conn, &mut bodies, &occ)?;
                        ev["via_model"] = json!(node.unit);
                        edges.entry((node.address.clone(), node.address.clone())).or_default().push(ev);
                    }
                    continue;
                }
                let mut ev = evidence(conn, &mut bodies, &occ)?;
                ev["via_model"] = json!(node.unit);
                if class == "vocabulary" {
                    ev["from"] = json!(node.address);
                    ev["to"] = json!(target.address);
                    vocabulary.push(ev);
                    continue;
                }
                if let Some(part) = &reached {
                    // A Link into another unit's own proof or derivation part reaches into it.
                    let part_card = crate::load_card(conn, part)?;
                    let is_statement = part_card.facet.as_ref().is_some_and(|f| !is_proof_part(f));
                    if !is_statement {
                        reach_in.push(json!({"carrier": occ.carrier, "ord": occ.ord, "target": part, "unit": target.address, "detail": "links a proof or derivation part of another unit rather than its statement"}));
                    }
                    ev["reached_part"] = json!(part);
                }
                if !nodes.contains_key(&target.address) {
                    if nodes.len() >= max_nodes {
                        truncated = true;
                        continue;
                    }
                    nodes.insert(target.address.clone(), target.clone());
                }
                edges.entry((node.address.clone(), target.address.clone())).or_default().push(ev);
                queue.push_back((target.address.clone(), level + 1));
            }
        }
    }

    // Dependents: who invokes the focus (one level), with the same evidence.
    let mut dependents: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let focus_carriers = carriers_of(conn, &focus)?;
    let focus_set: BTreeSet<&String> = focus_carriers.iter().collect();
    // The parts each Model head counts as its own (imports excluded), read once per head.
    let mut head_parts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for carrier in &focus_carriers {
        for occ in occurrences_into(conn, carrier)? {
            let Some(class) = counts(&occ.kind) else { continue };
            if focus_set.contains(&occ.carrier) {
                continue;
            }
            let (source, _) = node_of(conn, &occ.carrier, unit_scope)?;
            // A shared part belongs to every Model that names it: each of them is a user too.
            let mut users = vec![source];
            if unit_scope && let Ok(part) = crate::load_card(conn, &occ.carrier) && part.facet.is_some() {
                for (head_address,) in conn
                    .prepare_cached("SELECT DISTINCT l.source FROM links l JOIN cards c ON c.location = l.source WHERE l.target = ?1 AND c.card_type = 'model' AND c.model_facet IS NULL")?
                    .query_map([&occ.carrier], |r| Ok((r.get::<_, String>(0)?,)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?
                {
                    if users.iter().any(|u| u.address == head_address) {
                        continue;
                    }
                    // A head that only imports the part, or cites it, does not include it.
                    if !head_parts.contains_key(&head_address) {
                        let parts = match crate::load_card(conn, &head_address) {
                            Ok(head) => crate::model::parts_in_db(conn, &head)?,
                            Err(_) => Vec::new(),
                        };
                        head_parts.insert(head_address.clone(), parts);
                    }
                    if head_parts[&head_address].contains(&occ.carrier) {
                        users.push(node_of(conn, &head_address, unit_scope)?.0);
                    }
                }
            }
            let source = users.remove(0);
            for extra in users {
                if extra.address == focus.address {
                    continue;
                }
                let mut ev = evidence(conn, &mut bodies, &occ)?;
                ev["via_model"] = json!(extra.address);
                ev["class"] = json!(class);
                if class == "vocabulary" || (!nodes.contains_key(&extra.address) && nodes.len() >= max_nodes) {
                    continue;
                }
                dependents.entry(extra.address.clone()).or_default().push(ev);
                nodes.entry(extra.address.clone()).or_insert(extra);
            }
            if source.address == focus.address {
                continue;
            }
            let mut ev = evidence(conn, &mut bodies, &occ)?;
            ev["via_model"] = json!(source.unit);
            ev["class"] = json!(class);
            if class == "vocabulary" {
                continue;
            }
            if !nodes.contains_key(&source.address) && nodes.len() >= max_nodes {
                truncated = true;
                continue;
            }
            dependents.entry(source.address.clone()).or_default().push(ev);
            nodes.entry(source.address.clone()).or_insert(source);
        }
    }

    // Cycles and the prerequisite-first order over the authored prerequisite graph.
    let prereq_nodes: BTreeSet<String> = expanded.iter().chain(frontier.iter()).cloned().collect();
    let mut adjacency: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (from, to) in edges.keys() {
        adjacency.entry(from.clone()).or_default().insert(to.clone());
    }
    let comps = components(&prereq_nodes, &adjacency);
    let mut cycles = Vec::new();
    for comp in &comps {
        let self_loop = comp.len() == 1 && adjacency.get(&comp[0]).is_some_and(|t| t.contains(&comp[0]));
        if comp.len() > 1 || self_loop {
            cycles.push(json!({"nodes": comp}));
        }
    }
    // Tarjan emits components in reverse topological order of the condensation: a component comes
    // after every component it can reach, so prerequisites (reached) come first.
    let mut seed_order = Vec::new();
    for comp in &comps {
        for address in comp {
            seed_order.push(address.clone());
        }
    }

    // The overlay: a Connection tree's working connections compared with the authored edges.
    let overlay = match opt_str(args, "overlay_tree") {
        Some(tree) => Some(overlay_value(conn, tree, &nodes, &edges)?),
        None => None,
    };

    let edge_values: Vec<Value> = edges
        .iter()
        .map(|((from, to), evidence)| {
            json!({"from": from, "to": to, "relation": "depends on", "evidence": evidence, "origin": "authored"})
        })
        .collect();
    let prerequisites: Vec<Value> = nodes
        .values()
        .filter(|n| prereq_nodes.contains(&n.address) && n.address != focus.address)
        .map(node_value)
        .collect();
    let dependent_values: Vec<Value> = dependents
        .iter()
        .map(|(address, evidence)| json!({"node": node_value(&nodes[address]), "evidence": evidence}))
        .collect();
    Ok(json!({
        "focus": node_value(&focus),
        "scope": if unit_scope { "unit" } else { "card" },
        "unit": focus_unit,
        "carriers": focus_carriers,
        "depth": depth,
        "kinds": kinds,
        "nodes": nodes.values().map(node_value).collect::<Vec<_>>(),
        "prerequisites": prerequisites,
        "dependents": dependent_values,
        "edges": edge_values,
        "vocabulary": vocabulary,
        "cycles": cycles,
        "reach_in": reach_in,
        "seed_order": seed_order,
        "frontier": frontier,
        "truncated": truncated,
        "overlay": overlay,
        "archive_revision": revision,
    }))
}

/// Compare a Connection tree's connections with the authored edges: each authored edge is
/// `authored`, `both` (a directed tree connection between the same two Cards in the same direction
/// with a compatible predicate) or a tree connection is `tree_local`. The overlay never changes the
/// closure, the cycles or the order.
fn overlay_value(
    conn: &Connection,
    tree: &str,
    nodes: &BTreeMap<String, Node>,
    edges: &BTreeMap<(String, String), Vec<Value>>,
) -> Result<Value> {
    let tree = coord::resolve_tree(conn, tree)?;
    let shape = coord::load_tree_shape(conn, &tree)?;
    let by_card: BTreeMap<String, &String> = nodes.iter().map(|(address, n)| (n.card_id.clone(), address)).collect();
    let mut matched = BTreeSet::new();
    let mut local = Vec::new();
    for e in crate::connection::edges_of_tree(conn, &tree.tree_id)? {
        let (Some(from), Some(to)) = (shape.by_id.get(&e.from), shape.by_id.get(&e.to)) else { continue };
        let from_node = by_card.get(&from.card_id).map(|a| a.to_string());
        let to_node = by_card.get(&to.card_id).map(|a| a.to_string());
        if from_node.is_none() && to_node.is_none() {
            continue;
        }
        // Only a directed rests-on connection matches an authored rests-on edge; a label or another
        // kind is a different claim, and so is an authored edge of another kind.
        let compatible = e.directed && e.kind.as_deref() == Some("rests-on");
        let authored_rests_on = |f: &String, t: &String| {
            edges
                .get(&(f.clone(), t.clone()))
                .is_some_and(|evidence| evidence.iter().any(|ev| ev["kind"].as_str() == Some("rests-on")))
        };
        let both = compatible
            && matches!((&from_node, &to_node), (Some(f), Some(t)) if authored_rests_on(f, t));
        if both && let (Some(f), Some(t)) = (&from_node, &to_node) {
            matched.insert((f.clone(), t.clone()));
        }
        let value = crate::connection::edge_value(conn, &shape, &e)?;
        let from_address = match from_node {
            Some(address) => Some(address),
            None => coord::address_of_card_id(conn, &from.card_id)?,
        };
        let to_address = match to_node {
            Some(address) => Some(address),
            None => coord::address_of_card_id(conn, &to.card_id)?,
        };
        local.push(json!({
            "class": if both { "both" } else { "tree_local" },
            "from": from_address, "to": to_address,
            "from_coord_id": e.from, "to_coord_id": e.to,
            "from_coord": shape.coord[&e.from], "to_coord": shape.coord[&e.to],
            "connection": value,
        }));
    }
    let authored: Vec<Value> = edges
        .keys()
        .filter(|k| !matched.contains(*k))
        .map(|(f, t)| json!({"class": "authored", "from": f, "to": t}))
        .collect();
    Ok(json!({"tree": tree.handle(), "authored_only": authored, "connections": local}))
}

// ---------------------------------------------------------------------------------------------
// Seeding
// ---------------------------------------------------------------------------------------------

/// `seed_connection_tree`: preview the ordered Cards, then create the tree once on `seed`.
pub(crate) fn seed(
    conn: &Connection,
    access: Access,
    args: &Value,
    confirm: &mut dyn FnMut(&str, &[String], &Value) -> Result<Option<Value>>,
) -> Result<Value> {
    let from = opt_str(args, "from").context("missing `from` (hub, spread, dependencies, search or outline)")?;
    let mode = match args.get("mode") {
        None | Some(Value::Null) => "unique",
        Some(Value::String(mode)) if matches!(mode.as_str(), "unique" | "occurrences") => mode.as_str(),
        _ => bail!("seed mode must be unique or occurrences"),
    };
    let params = args.get("source_params").cloned().unwrap_or_else(|| json!({}));
    // The root has no meaning (docs/connection-trees-free-proposal.md D2): the title is optional.
    let title = opt_str(args, "title").unwrap_or_default();
    let workspace = match opt_str(args, "workspace") {
        Some(w) => Workspace::parse(w)?,
        None => match access {
            Access::Agent => Workspace::Agent,
            Access::Owner => Workspace::Human,
        },
    };
    access.ensure_writable(workspace)?;
    let mut source_state = json!(null);
    let candidates: Vec<String> = match from {
        "hub" => {
            let at = params.get("at").and_then(Value::as_str).context("hub seeding needs source_params.at")?;
            let hub = crate::load_card(conn, at)?;
            if hub.card_type != CardType::Hub {
                bail!("`{at}` is not a Hub Card");
            }
            let body = crate::parse_card_text(&hub.text)?.body;
            source_state = json!(crate::card::authored_fingerprint(&hub)?);
            crate::hub::parse_body(&body).lines.into_iter().map(|line| line.target).collect()
        }
        "spread" => {
            let spreads = crate::spread::load(conn)?;
            let wanted = params.get("id").or(params.get("name")).and_then(Value::as_str).context("spread seeding needs source_params.id or name")?;
            let spread = spreads["spreads"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|s| s["id"] == wanted || s["name"] == wanted)
                .with_context(|| format!("no saved Spread `{wanted}`"))?
                .clone();
            source_state = spread.clone();
            let mut cards: Vec<(f64, f64, String)> = spread["cards"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| Some((c["y"].as_f64()?, c["x"].as_f64()?, c["at"].as_str()?.to_string())))
                .collect();
            // Top to bottom, then left to right; ties keep the address order.
            cards.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
            cards.into_iter().map(|(_, _, at)| at).collect()
        }
        "dependencies" => {
            let lens = get(conn, &params)?;
            if let Some(order) = params.get("order").and_then(Value::as_array) {
                let allowed: BTreeSet<String> = lens["seed_order"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
                let order: Vec<String> = order.iter().filter_map(Value::as_str).map(str::to_string).collect();
                for at in &order {
                    if !allowed.contains(at) {
                        bail!("`{at}` is not in this Dependency Lens; reorder only its Cards");
                    }
                }
                let given: BTreeSet<&String> = order.iter().collect();
                if given.len() != order.len() || given.len() != allowed.len() {
                    bail!("a chosen order names every Card of the Dependency Lens once ({} Cards)", allowed.len());
                }
                // Outside a cycle a prerequisite stays before the Card that rests on it.
                let in_cycle = |a: &str, b: &str| {
                    lens["cycles"].as_array().into_iter().flatten().any(|c| {
                        let nodes: Vec<&str> = c["nodes"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
                        nodes.contains(&a) && nodes.contains(&b)
                    })
                };
                let position = |a: &str| order.iter().position(|x| x == a);
                for edge in lens["edges"].as_array().into_iter().flatten() {
                    let (Some(from), Some(to)) = (edge["from"].as_str(), edge["to"].as_str()) else { continue };
                    if from == to || in_cycle(from, to) {
                        continue;
                    }
                    if let (Some(f), Some(t)) = (position(from), position(to))
                        && t > f
                    {
                        bail!("`{to}` is a prerequisite of `{from}` and comes first outside a cycle");
                    }
                }
                order
            } else {
                if !lens["cycles"].as_array().is_none_or(Vec::is_empty) {
                    bail!("the Dependency Lens holds a cycle; pass source_params.order to choose an order within it");
                }
                lens["seed_order"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect()
            }
        }
        "search" | "outline" => params
            .get("cards")
            .or_else(|| params.get("order"))
            .and_then(Value::as_array)
            .context("seeding from search or outline needs source_params.cards in occurrence order")?
            .iter()
            .map(|value| value.as_str().map(str::to_string).context("each seed target must be a string"))
            .collect::<Result<Vec<_>>>()?,
        other => bail!("unknown seed source `{other}`; expected hub, spread, dependencies, search or outline"),
    };
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    let mut duplicates = Vec::new();
    let mut missing = Vec::new();
    for at in candidates {
        let card_id = if coord::is_uuid(&at) && coord::identity(conn, &at)?.is_some() {
            Some(at.clone())
        } else if let Some(address) = coord::canonical(conn, &at)? {
            Some(coord::card_id_of(conn, &address)?)
        } else {
            coord::resolve_coord(conn, &at).ok().map(|row| row.card_id)
        };
        let Some(card_id) = card_id else {
            missing.push(at);
            continue;
        };
        let Some(address) = coord::address_of_card_id(conn, &card_id)? else {
            missing.push(at);
            continue;
        };
        if mode == "unique" && !seen.insert(card_id.clone()) {
            duplicates.push(at);
            continue;
        }
        let card = crate::load_card(conn, &address)?;
        let title = crate::parse_card_text(&card.text)?.title;
        ordered.push((address, card_id, title));
    }
    if ordered.is_empty() {
        bail!("nothing to seed: the source names no live Card");
    }
    let named = if title.is_empty() { "an untitled Connection tree".to_string() } else { format!("Connection tree \"{title}\"") };
    let mut lines = vec![format!("create {named} with {} station(s) in this order; no tree connection is created", ordered.len())];
    for (i, (address, _, title)) in ordered.iter().enumerate() {
        lines.push(format!("{}. {address} {title}", i + 1));
    }
    for d in &duplicates {
        lines.push(format!("duplicate skipped (first occurrence wins): {d}"));
    }
    for m in &missing {
        lines.push(format!("missing, skipped: {m}"));
    }
    let effects = json!({
        "mode": mode,
        "order": ordered.iter().map(|(a, id, _)| json!([a, id])).collect::<Vec<_>>(),
        "duplicates": duplicates,
        "missing": missing,
        "source_state": source_state,
    });
    if let Some(plan) = confirm("seed", &lines, &effects)? {
        return Ok(plan);
    }
    let created = crate::connection::create_tree_tx(conn, access, workspace, title, "", opt_str(args, "purpose"))?;
    let tree = coord::resolve_tree(conn, created["tree"].as_str().unwrap_or_default())?;
    let mut parent = created["root_placement_id"].as_str().unwrap_or_default().to_string();
    let mut stations = Vec::new();
    for (address, card_id, _) in &ordered {
        let id = coord::new_uuid()?;
        conn.execute(
            "INSERT INTO coordinates(coord_id, tree_id, host_workspace, card_id, parent_coord_id, edge, slot)
             VALUES(?1, ?2, ?3, ?4, ?5, 'direct', NULL)",
            rusqlite::params![id, tree.tree_id, tree.workspace.id(), card_id, parent],
        )?;
        stations.push(json!({"coord_id": id, "placement_id": id, "card_id": card_id, "home_address": address}));
        parent = id;
    }
    let shape = coord::load_tree_shape(conn, &tree)?;
    let stations: Vec<Value> = stations
        .into_iter()
        .map(|mut s| {
            let id = s["placement_id"].as_str().unwrap_or_default().to_string();
            s["coord"] = json!(shape.coord.get(&id));
            s
        })
        .collect();
    Ok(json!({
        "applied": true,
        "tree": tree.handle(),
        "tree_id": tree.tree_id,
        "stations": stations,
        "mode": mode,
        "duplicates": duplicates,
        "missing": missing,
        "connections": 0,
        "changed": true,
    }))
}
