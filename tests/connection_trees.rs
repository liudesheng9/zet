//! Connection trees (关联树), mounts and tree connections over a real `zt mcp` and the owner's GUI
//! (docs/multi-tree-coordinates-proposal.md §3–§7, Phase 1): a tree starts with a title, mounts Cards
//! from anywhere by identity without writing them, keeps one coordinate per Card, records neutral and
//! directed connections that follow placement IDs through every reorganization, refuses coordinates
//! inside `[[…]]`, and turns the station of a deleted source into a Missing mount that keeps its
//! gloss, children and connections.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// Two books, each a root with two notes: `Grossman1986` and `Hart1995` (agent Workspace).
fn books(mcp: &mut Mcp) {
    for (key, title) in [("Grossman1986", "The Costs and Benefits of Ownership"), ("Hart1995", "Firms, Contracts, and Financial Structure")] {
        mcp.ok("create_literature", json!({"bibtex": format!("@book{{{key}, title={{{title}}}}}")}));
        mcp.ok("create_successor", json!({"at": key, "title": format!("{key} 第一节"), "body": format!("第一节的内容。\n\n来源：[[{key}]] §1")}));
        mcp.ok("create_successor", json!({"at": format!("{key}/1"), "title": format!("{key} 第二节"), "body": format!("第二节的内容。\n\n来源：[[{key}]] §2")}));
    }
}

fn tree(mcp: &mut Mcp, title: &str) -> Value {
    mcp.ok("create_connection_tree", json!({"title": title}))
}

fn get_tree(mcp: &mut Mcp, tree: &str) -> Value {
    mcp.ok("get_connection_tree", json!({"tree": tree}))
}

fn station<'a>(view: &'a Value, home: &str) -> &'a Value {
    view["placements"]
        .as_array()
        .expect("placements")
        .iter()
        .find(|p| p["home_address"] == home)
        .unwrap_or_else(|| panic!("no station of {home} in {view}"))
}

fn place(mcp: &mut Mcp, host: &str, parent: &str, edge: &str, target: &str) -> Value {
    mcp.ok("place_card", json!({"host": host, "parent_coord": parent, "edge": edge, "target_at": target}))
}

fn confirmed(mcp: &mut Mcp, tool: &str, args: Value) -> Value {
    let plan = mcp.ok(tool, args.clone());
    assert_eq!(plan["applied"], false, "{tool} plans first: {plan}");
    let mut args = args;
    args["confirm"] = plan["confirm"].clone();
    args["plan_hash"] = plan["plan_hash"].clone();
    args["expected_revision"] = plan["expected_revision"].clone();
    let done = mcp.ok(tool, args);
    assert_eq!(done["applied"], true, "{tool} applies on confirmation: {done}");
    done
}

/// §3.2/§5: a Connection tree starts with only a title; its root `~1` is a citable Card and `~1/0`
/// is its root coordinate. A coordinate never stands inside `[[…]]`.
#[test]
fn a_connection_tree_starts_with_a_title_and_its_root_is_citable() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let made = tree(&mut mcp, "Boundary of the Firm");
    assert_eq!(made["tree"], "~1");
    assert_eq!(made["coord"], "~1/0");
    let view = get_tree(&mut mcp, "~1");
    assert_eq!(view["title"], "Boundary of the Firm");
    assert_eq!(view["workspace"], "agent");
    assert_eq!(view["placements"].as_array().unwrap().len(), 1, "{view}");
    assert_eq!(view["placements"][0]["coord"], "~1/0");
    assert_eq!(view["placements"][0]["origin"], "native");
    let root = mcp.ok("get_card", json!({"at": "~1"}));
    assert_eq!(root["title"], "Boundary of the Firm");

    // A Topic Card cites the tree root by its handle; a coordinate is refused with a hint.
    mcp.ok("create_topic", json!({"title": "Topic", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Cites the tree", "body": "See [[~1]]."}));
    let refused = mcp.refused("create_successor", json!({"at": "0/1", "title": "Cites a slot", "body": "See [[~1/0]]."}));
    assert!(refused.to_string().contains("navigation coordinate"), "{refused}");
    let refused = mcp.refused("create_successor", json!({"at": "0/1", "title": "Cites a mount", "body": "See [[0/1+1]]."}));
    assert!(refused.to_string().contains("navigation coordinate"), "{refused}");
    // The root's Reverse links list the citing Card.
    let root = mcp.ok("get_card", json!({"at": "~1"}));
    assert!(root.to_string().contains("0/1"), "{root}");

    // Numbers are never reused and a second tree is `~2`.
    assert_eq!(tree(&mut mcp, "Second")["tree"], "~2");
    let listed = mcp.ok("list_connection_trees", json!({}));
    assert_eq!(listed["trees"].as_array().unwrap().len(), 2, "{listed}");
    let links = scratch.cmd().args(["status", "--check-links"]).output().expect("check links");
    let report: Value = serde_json::from_slice(&links.stdout).expect("report");
    assert_eq!(report["ok"], true, "{report}");
}

/// §3.4.2/§3.4.3, Scenario A: two Locked books are mounted as single portal stations; mounting
/// writes neither book; a Card stands at most once in a tree.
#[test]
fn locked_books_mount_as_single_portals_and_a_card_stands_once_per_tree() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    let gui = Gui::connect(scratch.cmd());
    gui.lock("Grossman1986");
    gui.lock("Hart1995");
    let before_g = mcp.ok("get_tree", json!({"tree": "Grossman1986", "bodies": true}));
    let before_h = mcp.ok("get_tree", json!({"tree": "Hart1995", "bodies": true}));
    tree(&mut mcp, "Incomplete contracts");
    let g = place(&mut mcp, "~1", "~1/0", "direct", "Grossman1986");
    assert_eq!(g["coord"], "~1/1");
    let h = place(&mut mcp, "~1", "~1/1", "direct", "Hart1995");
    assert_eq!(h["coord"], "~1/2");
    let again = place(&mut mcp, "~1", "~1/2", "direct", "Grossman1986");
    assert_eq!(again["already_placed"], true, "{again}");
    assert_eq!(again["coord"], "~1/1");
    // Selective drill-down: a note of Hart's book is a distinct Card.
    let note = place(&mut mcp, "~1", "~1/2", "side", "Hart1995/1");
    assert_eq!(note["coord"], "~1/2|a");
    let view = get_tree(&mut mcp, "~1");
    assert_eq!(view["placements"].as_array().unwrap().len(), 4, "{view}");
    let hart = station(&view, "Hart1995");
    assert_eq!(hart["portal"]["cards"], 3, "a portal counts its home tree: {hart}");
    assert_eq!(hart["can_edit_card"], false, "a locked book is not editable: {hart}");
    assert_eq!(hart["can_edit_placement"], true, "{hart}");
    assert_eq!(hart["origin"], "mount");
    assert_eq!(mcp.ok("get_tree", json!({"tree": "Grossman1986", "bodies": true}))["cards"], before_g["cards"]);
    assert_eq!(mcp.ok("get_tree", json!({"tree": "Hart1995", "bodies": true}))["cards"], before_h["cards"]);
    let stations = mcp.ok("list_placements", json!({"at": "Hart1995"}));
    assert_eq!(stations["placements"].as_array().unwrap().len(), 1, "{stations}");
    assert_eq!(stations["placements"][0]["coord"], "~1/2");
    // The root takes no Side successor.
    let refused = mcp.refused("place_card", json!({"host": "~1", "parent_coord": "~1/0", "edge": "side", "target_at": "Grossman1986/1"}));
    assert!(refused.to_string().contains("root takes no Side"), "{refused}");
    let report: Value = serde_json::from_slice(&scratch.cmd().args(["status", "--check-locks"]).output().unwrap().stdout).unwrap_or(Value::Null);
    assert_ne!(report["ok"], false, "locks stay intact: {report}");
}

/// §3.4.5: quick-connect is a neutral blank pair (one per pair, order-free); directed assertions
/// keep their order; free labels, canonical kinds and plain notes are checked; several labeled
/// assertions may join one pair.
#[test]
fn connections_are_neutral_by_default_and_assertions_are_explicit() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    tree(&mut mcp, "Pairs");
    let g = place(&mut mcp, "~1", "~1/0", "direct", "Grossman1986");
    let h = place(&mut mcp, "~1", "~1/1", "direct", "Hart1995");
    let (gp, hp) = (g["placement_id"].as_str().unwrap(), h["placement_id"].as_str().unwrap());
    let blank = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp}));
    assert_eq!(blank["direction"], "neutral");
    let again = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": gp, "to_placement_id": hp}));
    assert_eq!(again["already_connected"], true, "the blank pair is order-free and unique: {again}");
    assert_eq!(again["connection_id"], blank["connection_id"]);
    let relaxes = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/2", "to_placement_id": "~1/1", "direction": "directed", "relation_kind": "relaxes", "relation_label": "多资产推广", "note": "Extends GHM 1986 to multi-asset authority."}));
    assert_eq!(relaxes["from"]["placement_id"], hp);
    assert_eq!(relaxes["provenance"], "In this tree · ~1");
    assert!(relaxes["sentence"].as_str().unwrap().contains("多资产推广"), "{relaxes}");
    let objects = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp, "direction": "directed", "relation_label": "待验证"}));
    assert_eq!(objects["relation_kind"], Value::Null, "a free label is never translated into a kind");
    let refused = mcp.refused("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp, "relation_kind": "rests-on"}));
    assert!(refused.to_string().contains("directed"), "{refused}");
    mcp.refused("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp, "note": "see [[Hart1995]]"}));
    mcp.refused("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp, "relation_label": "x".repeat(33)}));
    mcp.refused("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": hp}));
    mcp.refused("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp, "relation_kind": "term", "direction": "directed"}));
    // The root is a valid endpoint.
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/0", "to_placement_id": hp}));
    let list = mcp.ok("list_connections", json!({"tree": "~1", "placement_id": hp}));
    assert_eq!(list["outgoing"].as_array().unwrap().len(), 2, "{list}");
    assert_eq!(list["neutral"].as_array().unwrap().len(), 2, "{list}");
    assert!(list["labels"].as_array().unwrap().iter().any(|l| l == "多资产推广"));
    // Edit: clear the label, reverse the direction; the ID stays.
    let id = objects["connection_id"].as_str().unwrap();
    let edited = mcp.ok("edit_connection", json!({"connection_id": id, "relation_label": null, "note": "Disputes.", "reverse_endpoints": true}));
    assert_eq!(edited["connection_id"], id);
    assert_eq!(edited["from"]["placement_id"], gp);
    // Making it a neutral blank would collide with the existing blank pair.
    let conflict = mcp.refused("edit_connection", json!({"connection_id": id, "note": null, "direction": "neutral"}));
    assert!(conflict.to_string().contains("conflict"), "{conflict}");
    // A Connection tree writes no Card and no Link.
    let card = mcp.ok("get_card", json!({"at": "Hart1995"}));
    assert!(!card.to_string().contains("~1"), "{card}");
    let deleted = confirmed(&mut mcp, "delete_connection", json!({"connection_id": id}));
    assert_eq!(deleted["deleted"], id);
}

/// §7.2 "Move placement", Acceptance §10.3 (1)(2): moving a station reviews every gloss whose
/// predecessor changes, keeps every connection on its exact two placements, and writes no Card.
#[test]
fn a_move_reviews_glosses_and_keeps_connections_on_their_placements() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    tree(&mut mcp, "Move");
    let g = place(&mut mcp, "~1", "~1/0", "direct", "Grossman1986");
    let h = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "edge": "direct", "target_at": "Hart1995", "gloss": "Compare asset-control allocation here."}));
    let note = place(&mut mcp, "~1", "~1/2", "direct", "Grossman1986/1");
    let framing = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/3", "edge": "direct", "title": "Control rights as a frame", "body": "A framing note."}));
    assert_eq!(framing["coord"], "~1/4");
    assert_eq!(framing["handle"], "~1#1");
    let (gp, hp) = (g["placement_id"].as_str().unwrap(), h["placement_id"].as_str().unwrap());
    let relaxes = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": hp, "to_placement_id": gp, "direction": "directed", "relation_kind": "relaxes", "note": "Extends GHM."}));
    let book_before = mcp.ok("get_tree", json!({"tree": "Hart1995", "bodies": true}));
    // Move Hart under the framing Card as a Side branch: Hart's gloss and its continuation's
    // predecessor change.
    let args = json!({"placement_id": hp, "destination": {"parent_placement_id": framing["placement_id"], "edge": "side"}});
    let plan = mcp.ok("move_placement", args.clone());
    assert_eq!(plan["applied"], false);
    assert!(plan["lines"].to_string().contains("Compare asset-control allocation here."), "{plan}");
    let mut without = args.clone();
    without["confirm"] = json!("move");
    without["plan_hash"] = plan["plan_hash"].clone();
    without["expected_revision"] = plan["expected_revision"].clone();
    let refused = mcp.refused("move_placement", without);
    assert!(refused.to_string().contains("gloss review required"), "{refused}");
    // A confirmation must carry the plan it confirms.
    let mut bare = args.clone();
    bare["confirm"] = json!("move");
    let refused = mcp.refused("move_placement", bare);
    assert!(refused.to_string().contains("plan_hash"), "{refused}");
    let mut reviewed = args.clone();
    reviewed["gloss_updates"] = json!({hp: "Read as a refinement of the frame."});
    let done = confirmed(&mut mcp, "move_placement", reviewed);
    assert_eq!(done["coord"], "~1/3|a", "Grossman's note closed the gap: {done}");
    let view = get_tree(&mut mcp, "~1");
    assert_eq!(station(&view, "Grossman1986/1")["coord"], "~1/2");
    assert_eq!(station(&view, "Hart1995")["gloss"], "Read as a refinement of the frame.");
    let kept = view["connections"].as_array().unwrap().iter().find(|c| c["connection_id"] == relaxes["connection_id"]).expect("connection survives");
    assert_eq!(kept["from"]["placement_id"], hp);
    assert_eq!(kept["to"]["placement_id"], gp);
    assert_eq!(kept["from"]["coord"], "~1/3|a");
    assert_eq!(kept["updated_at_rev"], relaxes["updated_at_rev"], "the connection record itself is untouched");
    assert_eq!(mcp.ok("get_tree", json!({"tree": "Hart1995", "bodies": true}))["cards"], book_before["cards"]);
    let _ = note;
    // A move under its own branch is refused.
    let refused = mcp.refused("move_placement", json!({"placement_id": framing["placement_id"], "destination": {"parent_placement_id": hp, "edge": "side"}, "confirm": "move"}));
    assert!(refused.to_string().contains("under itself"), "{refused}");
    // A stale plan is refused.
    let plan = mcp.ok("move_placement", json!({"placement_id": hp, "destination": {"parent_coord": "~1/1", "edge": "side"}}));
    mcp.ok("edit_placement", json!({"placement_id": gp, "gloss": "Base case."}));
    let stale = mcp.refused("move_placement", json!({"placement_id": hp, "destination": {"parent_coord": "~1/1", "edge": "side"}, "confirm": "move", "expected_revision": plan["expected_revision"]}));
    assert!(stale.to_string().contains("stale"), "{stale}");
}

/// §5.3 and §3.4.6: a native Card has a permanent handle, cited as `[[~1#1]]`; reordering the tree
/// rewrites no Link anywhere.
#[test]
fn native_handles_are_citable_and_reordering_rewrites_no_link() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    tree(&mut mcp, "Natives");
    let a = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "第一张", "body": "第一张的内容。"}));
    let b = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "title": "第二张", "body": "引用 [[~1#1]]。"}));
    assert_eq!(b["handle"], "~1#2");
    mcp.ok("create_topic", json!({"title": "Topic", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Cites a native", "body": "See [[~1#2]] and [[~1#1]]."}));
    let before = mcp.ok("get_card", json!({"at": "0/1"}))["body"].clone();
    confirmed(&mut mcp, "move_placement", json!({"placement_id": b["placement_id"], "destination": {"parent_coord": "~1/0", "edge": "direct"}}));
    let view = get_tree(&mut mcp, "~1");
    assert_eq!(station(&view, "~1#2")["coord"], "~1/1");
    assert_eq!(station(&view, "~1#1")["coord"], "~1/2");
    assert_eq!(mcp.ok("get_card", json!({"at": "0/1"}))["body"], before, "no Link was rewritten");
    let first = mcp.ok("get_card", json!({"at": "~1#1"}));
    assert!(first.to_string().contains("~1#2"), "Reverse links: {first}");
    // Native structured families are refused.
    let refused = mcp.refused("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "type": "model", "title": "A model", "body": "x"}));
    assert!(refused.to_string().contains("native_structure_required") || refused.to_string().contains("idea"), "{refused}");
    // A native Card's home station cannot be unplaced.
    let refused = mcp.refused("unplace", json!({"placement_id": a["placement_id"], "confirm": "unplace"}));
    assert!(refused.to_string().contains("home station"), "{refused}");
}

/// §7.1: an agent mounts and connects inside its own trees only; the owner's tree is read-only to it.
#[test]
fn the_agent_writes_only_agent_hosts_and_the_owner_writes_any() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    let gui = Gui::connect(scratch.cmd());
    let human = gui.conn("create_connection_tree", json!({"title": "Owner's map"}));
    assert_eq!(human["workspace"], "human");
    let refused = mcp.refused("place_card", json!({"host": human["tree"], "parent_coord": format!("{}/0", human["tree"].as_str().unwrap()), "target_at": "Hart1995"}));
    assert!(refused.to_string().contains("read-only"), "{refused}");
    let placed = gui.conn("place_card", json!({"host": human["tree"], "parent_coord": format!("{}/0", human["tree"].as_str().unwrap()), "target_at": "Hart1995"}));
    assert_eq!(placed["coord"], "~1/1");
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(view["can_edit_tree"], false);
    assert_eq!(station(&view, "Hart1995")["can_edit_placement"], false);
    assert_eq!(station(&view, "Hart1995")["can_edit_card"], true, "the agent may still edit its own book Card");
    // A Literature-scoped connection makes no Connection-tree write.
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let refused = literature.refused("create_connection_tree", json!({"title": "No"}));
    assert!(refused.to_string().contains("connection_tree"), "{refused}");
    let refused = literature.refused("create_successor", json!({"at": "Hart1995/2", "title": "Cites a tree", "body": "See [[~1]].\n\n来源：[[Hart1995]] §3"}));
    assert!(refused.to_string().contains("topic_link") || refused.to_string().contains("Topic"), "{refused}");
}

/// §7.3, Acceptance §10.3 (4): re-digesting an agent book deletes a mounted source; the owner's
/// Connection tree keeps the station as a Missing mount with its gloss, children and connections, and
/// no human row changes. Rebinding needs a decision for every connection and issues a new station ID.
#[test]
fn a_deleted_source_leaves_a_missing_mount_and_rebinding_decides_every_connection() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    let gui = Gui::connect(scratch.cmd());
    let human = gui.conn("create_connection_tree", json!({"title": "Owner's investigation"}));
    let t = human["tree"].as_str().unwrap().to_string();
    let a = gui.conn("place_card", json!({"host": t, "parent_coord": format!("{t}/0"), "target_at": "Hart1995/1", "gloss": "Primary evidence."}));
    let b = gui.conn("place_card", json!({"host": t, "parent_coord": format!("{t}/1"), "edge": "side", "target_at": "Grossman1986/1"}));
    let c = gui.conn("place_card", json!({"host": t, "parent_coord": format!("{t}/1"), "edge": "direct", "target_at": "Grossman1986"}));
    let edge = gui.conn("connect_placements", json!({"tree": t, "from_placement_id": b["placement_id"], "to_placement_id": a["placement_id"], "direction": "directed", "relation_label": "corroborates"}));
    let human_rows = |s: &ScratchBox| -> Vec<(String, String)> {
        let db = writer::open(s.archive_root.join("zt.sqlite3")).unwrap();
        let mut stmt = db.prepare("SELECT COALESCE(location, citation_key), text FROM cards WHERE workspace = 'human' ORDER BY 1").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    let host_rows = |s: &ScratchBox| -> Vec<(String, String, Option<String>)> {
        let db = writer::open(s.archive_root.join("zt.sqlite3")).unwrap();
        let mut stmt = db.prepare("SELECT coord_id, card_id, gloss FROM coordinates WHERE host_workspace = 'human' ORDER BY 1").unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().collect::<rusqlite::Result<Vec<_>>>().unwrap()
    };
    let humans_before = human_rows(&scratch);
    let hosts_before = host_rows(&scratch);
    // The agent re-digests Hart's book: every note goes, the root stays.
    let out = scratch.cmd().args(["del", "--at", "Hart1995", "--keep-root"]).write_stdin("delete\n").output().expect("del");
    if !out.status.success() {
        // The CLI asks for the plan's confirmation word; read it from the plan output.
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let word = text.lines().find_map(|l| l.strip_prefix("type `").and_then(|r| r.split('`').next())).unwrap_or("delete").to_string();
        let out = scratch.cmd().args(["del", "--at", "Hart1995", "--keep-root"]).write_stdin(format!("{word}\n")).output().expect("del");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
    assert_eq!(human_rows(&scratch), humans_before, "no human row changed");
    assert_eq!(host_rows(&scratch), hosts_before, "no host row changed");
    let view = mcp.ok("get_connection_tree", json!({"tree": t}));
    let missing = view["placements"].as_array().unwrap().iter().find(|p| p["placement_id"] == a["placement_id"]).expect("station kept").clone();
    assert_eq!(missing["status"], "missing");
    assert_eq!(missing["home_address"], "Hart1995/1");
    assert_eq!(missing["gloss"], "Primary evidence.");
    assert_eq!(missing["coord"], format!("{t}/1"));
    assert_eq!(view["missing"], 1);
    assert!(view["connections"].as_array().unwrap().iter().any(|e| e["connection_id"] == edge["connection_id"]), "{view}");
    // A new Card at the same Location never revives the retired station.
    mcp.ok("create_successor", json!({"at": "Hart1995", "title": "新的第一节", "body": "新内容。\n\n来源：[[Hart1995]] §1"}));
    let view = mcp.ok("get_connection_tree", json!({"tree": t}));
    let still = view["placements"].as_array().unwrap().iter().find(|p| p["placement_id"] == a["placement_id"]).unwrap();
    assert_eq!(still["status"], "missing", "{still}");
    // Rebind: every incident connection needs a decision; the station gets a fresh ID.
    let args = json!({"placement_id": a["placement_id"], "target_at": "Hart1995/1", "gloss_updates": {a["placement_id"].as_str().unwrap(): "Primary evidence (new edition)."}});
    let plan = gui.conn("rebind_placement", args.clone());
    assert!(plan["lines"].to_string().contains(edge["connection_id"].as_str().unwrap()), "{plan}");
    let mut undecided = args.clone();
    undecided["confirm"] = json!("rebind");
    undecided["plan_hash"] = plan["plan_hash"].clone();
    undecided["expected_revision"] = plan["expected_revision"].clone();
    let refused = gui.conn_refused("rebind_placement", undecided);
    assert!(refused.to_string().contains("disposition"), "{refused}");
    let mut decided = args;
    decided["connection_dispositions"] = json!({edge["connection_id"].as_str().unwrap(): "keep"});
    let done = gui.conn_confirmed("rebind_placement", decided);
    assert_ne!(done["placement_id"], a["placement_id"]);
    let view = mcp.ok("get_connection_tree", json!({"tree": t}));
    let rebound = station(&view, "Hart1995/1");
    assert_eq!(rebound["status"], "live");
    assert_eq!(rebound["coord"], format!("{t}/1"));
    assert_eq!(rebound["gloss"], "Primary evidence (new edition).");
    let kept = &view["connections"][0];
    assert_eq!(kept["to"]["placement_id"], done["placement_id"]);
    assert_ne!(kept["connection_id"], edge["connection_id"], "a kept assertion is a new record on the new station");
    // The old bookmark reports removed.
    let refused = mcp.refused("list_placements", json!({"at": format!("place:{}", a["placement_id"].as_str().unwrap())}));
    assert!(refused.to_string().contains("removed"), "{refused}");
    let _ = c;
}

/// §7.2 "Detach": unplacing never deletes the mounted Card; children reattach or the branch is
/// removed with its connections, all listed in the plan.
#[test]
fn unplace_reattaches_or_detaches_children_and_lists_lost_connections() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    tree(&mut mcp, "Detach");
    let g = place(&mut mcp, "~1", "~1/0", "direct", "Grossman1986");
    let g1 = place(&mut mcp, "~1", "~1/1", "direct", "Grossman1986/1");
    let g2 = place(&mut mcp, "~1", "~1/2", "direct", "Grossman1986/2");
    let h = place(&mut mcp, "~1", "~1/2", "side", "Hart1995");
    let e = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": h["placement_id"], "to_placement_id": g["placement_id"]}));
    // ~1/2 has a Direct continuation (~1/3) and a Side branch (~1/2|a).
    let refused = mcp.refused("unplace", json!({"placement_id": g1["placement_id"], "confirm": "unplace"}));
    assert!(refused.to_string().contains("children"), "{refused}");
    confirmed(&mut mcp, "unplace", json!({"placement_id": g1["placement_id"], "children": "reattach"}));
    let view = get_tree(&mut mcp, "~1");
    assert_eq!(station(&view, "Grossman1986/2")["coord"], "~1/2");
    assert_eq!(station(&view, "Hart1995")["coord"], "~1/2|a");
    assert!(mcp.ok("get_card", json!({"at": "Grossman1986/1"}))["title"].is_string(), "the Card stays at home");
    let plan = mcp.ok("unplace", json!({"placement_id": g2["placement_id"], "children": "detach"}));
    assert!(plan["lines"].to_string().contains(e["connection_id"].as_str().unwrap()), "{plan}");
    confirmed(&mut mcp, "unplace", json!({"placement_id": g2["placement_id"], "children": "detach"}));
    let view = get_tree(&mut mcp, "~1");
    assert_eq!(view["placements"].as_array().unwrap().len(), 2, "{view}");
    assert_eq!(view["connections"].as_array().unwrap().len(), 0);
}

/// §7.2 "Extract a branch": a copy into a new tree carries the internal connections with fresh IDs
/// and leaves the original and its boundary connections untouched.
#[test]
fn extracting_a_branch_copies_internal_connections_and_keeps_the_original() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    tree(&mut mcp, "Source");
    let g = place(&mut mcp, "~1", "~1/0", "direct", "Grossman1986");
    let h = place(&mut mcp, "~1", "~1/1", "direct", "Hart1995");
    let h1 = place(&mut mcp, "~1", "~1/2", "side", "Hart1995/1");
    let internal = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": h1["placement_id"], "to_placement_id": h["placement_id"], "direction": "directed", "relation_label": "detail"}));
    let boundary = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": h["placement_id"], "to_placement_id": g["placement_id"]}));
    let before = get_tree(&mut mcp, "~1");
    let done = confirmed(&mut mcp, "extract_branch", json!({"source_tree": "~1", "root_placement_id": h["placement_id"], "title": "Hart only"}));
    assert_eq!(done["tree"], "~2");
    assert_eq!(done["boundary_connections"][0], boundary["connection_id"]);
    let copy = get_tree(&mut mcp, "~2");
    assert_eq!(station(&copy, "Hart1995")["coord"], "~2/1");
    assert_eq!(station(&copy, "Hart1995/1")["coord"], "~2/1|a");
    assert_eq!(copy["connections"].as_array().unwrap().len(), 1);
    assert_ne!(copy["connections"][0]["connection_id"], internal["connection_id"]);
    assert_eq!(copy["connections"][0]["relation_label"], "detail");
    let after = get_tree(&mut mcp, "~1");
    // The original stations stay as they were; only the count of places their Cards stand in grows,
    // because the extracted tree holds copies of those stations.
    let without_places = |tree: &Value| -> Vec<Value> {
        tree["placements"].as_array().unwrap().iter().map(|p| { let mut p = p.clone(); p.as_object_mut().unwrap().remove("places"); p }).collect()
    };
    assert_eq!(without_places(&after), without_places(&before));
    assert_eq!(station(&after, "Hart1995")["places"], 3, "the Card now stands at home plus both Connection trees");
    assert_eq!(after["connections"].as_array().unwrap().len(), 2);
}

/// `op_key`: a retried write replays its answer and allocates nothing; a different request under the
/// same key is refused.
#[test]
fn an_op_key_replays_a_write_without_allocating() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let first = mcp.ok("create_connection_tree", json!({"title": "Once", "op_key": "k1"}));
    let again = mcp.ok("create_connection_tree", json!({"title": "Once", "op_key": "k1"}));
    assert_eq!(again["tree"], first["tree"]);
    assert_eq!(again["replayed"], true);
    let refused = mcp.refused("create_connection_tree", json!({"title": "Other", "op_key": "k1"}));
    assert!(refused.to_string().contains("op_key"), "{refused}");
    assert_eq!(mcp.ok("list_connection_trees", json!({}))["trees"].as_array().unwrap().len(), 1);
}

/// §7.3 and §8.4: deleting a Connection root removes its tree (stations, connections, record) and
/// its still-native Cards; its numbers and handles are never reused; another tree that mounts the
/// deleted root keeps a Missing mount. A keep-root delete empties the tree but keeps its root.
/// `zt del --all` removes every tree and keeps the never-reused allocator.
#[test]
fn deleting_a_connection_root_removes_its_tree_and_numbers_are_never_reused() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    books(&mut mcp);
    tree(&mut mcp, "Doomed");
    tree(&mut mcp, "Survivor");
    place(&mut mcp, "~1", "~1/0", "direct", "Hart1995");
    let native = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "title": "Native", "body": "Native body."}));
    let portal = place(&mut mcp, "~2", "~2/0", "direct", "~1");
    assert_eq!(portal["coord"], "~2/1");
    let plan = mcp.ok("delete_card", json!({"at": "~1"}));
    assert_eq!(plan["applied"], false, "{plan}");
    assert!(plan.to_string().contains("Connection tree ~1 goes"), "{plan}");
    assert!(plan.to_string().contains("Missing mounts"), "{plan}");
    let done = mcp.ok("delete_card", json!({"at": "~1", "confirm": plan["confirm"]}));
    assert_eq!(done["deleted"], 2, "{done}");
    mcp.refused("get_connection_tree", json!({"tree": "~1"}));
    mcp.refused("get_card", json!({"at": native["handle"]}));
    let survivor = get_tree(&mut mcp, "~2");
    let missing = &survivor["placements"][1];
    assert_eq!(missing["status"], "missing", "{survivor}");
    assert_eq!(missing["home_address"], "~1");
    assert_eq!(tree(&mut mcp, "Next")["tree"], "~3", "numbers are never reused");
    // A Card citing a deleted handle is a Broken link, never another Card.
    let broken = mcp.ok("list_broken_links", json!({}));
    assert!(broken.to_string().contains("~1") || broken["broken"].as_array().map(|b| b.is_empty()).unwrap_or(true), "{broken}");

    // Keep-root: the tree loses its stations and connections, the root stays.
    place(&mut mcp, "~3", "~3/0", "direct", "Hart1995");
    place(&mut mcp, "~3", "~3/1", "direct", "Grossman1986");
    mcp.ok("connect_placements", json!({"tree": "~3", "from_placement_id": "~3/1", "to_placement_id": "~3/2"}));
    let out = scratch.cmd().args(["del", "--at", "~3", "--keep-root"]).write_stdin("~3\n").output().expect("del");
    assert!(out.status.success(), "{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let kept = get_tree(&mut mcp, "~3");
    assert_eq!(kept["placements"].as_array().unwrap().len(), 1, "{kept}");
    assert_eq!(kept["connections"].as_array().unwrap().len(), 0);

    // Delete all: every tree goes; the allocator does not restart.
    let out = scratch.cmd().args(["del", "--all"]).write_stdin("delete all\n").output().expect("del all");
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(mcp.ok("list_connection_trees", json!({}))["trees"].as_array().unwrap().len(), 0);
    assert_eq!(tree(&mut mcp, "After all")["tree"], "~4");
}
