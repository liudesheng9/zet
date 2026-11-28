//! Seam 2 TDD tests: Unified Coordinate Operations (same-tree repeats, self-ancestry, root
//! multiplicity, MC3 Literature admission, MC4 splice guard & secondary-path link refusal,
//! survivor-set detach floor, refile_card, and op_key idempotency)
//! (docs/multi-coordinate-cards-proposal.md §§3, 4, 6, 7, 11).

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn confirm(mcp: &mut Mcp, tool: &str, mut args: Value) -> Value {
    let plan = mcp.ok(tool, args.clone());
    if plan["applied"] == json!(false) {
        args["confirm"] = plan["confirm"].clone();
        args["plan_hash"] = plan["plan_hash"].clone();
        args["expected_revision"] = plan["expected_revision"].clone();
        mcp.ok(tool, args)
    } else {
        plan
    }
}

#[test]
fn same_tree_multiplicity_self_ancestry_and_independent_children_work() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    let _ = mcp.ok("create_topic", json!({"title": "Topic 0", "body": "Root."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "Initial X."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "title": "Card Y", "body": "Initial Y."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/2", "title": "Card Z", "body": "Initial Z."}));

    let ct = mcp.ok("create_connection_tree", json!({"title": "Cycle Walk"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let root_coord = ct["root_placement_id"].as_str().unwrap().to_string();

    // Build X(c1) -> Y(c2) -> X(c3) plus X(c4) on a Side branch of c1.
    let c1 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_coord, "edge": "direct", "target_at": "0/1"}));
    let c1_id = c1["coord_id"].as_str().unwrap().to_string();
    let c2 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1_id, "edge": "direct", "target_at": "0/2"}));
    let c2_id = c2["coord_id"].as_str().unwrap().to_string();
    let c3 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c2_id, "edge": "direct", "target_at": "0/1", "gloss": "Revisit X after Y"}));
    let c3_id = c3["coord_id"].as_str().unwrap().to_string();
    let c4 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1_id, "edge": "side", "target_at": "0/1", "gloss": "Alternative entry"}));
    let c4_id = c4["coord_id"].as_str().unwrap().to_string();

    assert_ne!(c1_id, c3_id);
    assert_ne!(c1_id, c4_id);
    assert_ne!(c3_id, c4_id);

    // Attach Z as Side child of c3 ONLY.
    let cz = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c3_id, "edge": "side", "target_at": "0/3"}));
    assert_eq!(cz["coord"], format!("{handle}/3|a"));

    // Draw a Tree connection between two different Coordinates of the SAME Card (c3 -> c4).
    let conn_edge = mcp.ok(
        "connect_placements",
        json!({
            "tree": &handle,
            "from_placement_id": &c3_id,
            "to_placement_id": &c4_id,
            "direction": "directed",
            "relation_label": "revisits"
        }),
    );
    assert_eq!(conn_edge["from"]["placement_id"], c3_id);
    assert_eq!(conn_edge["to"]["placement_id"], c4_id);

    // list_coordinates reports ALL 4 Coordinates (0/1 filing + c1 + c3 + c4).
    let listed = mcp.ok("list_coordinates", json!({"at": "0/1"}));
    assert_eq!(listed["coordinates"].as_array().unwrap().len(), 4);

    // Moving c1 under its own descendant c3 creates a coord_id cycle and MUST be refused.
    let cycle_err = mcp
        .refused(
            "move_coordinate",
            json!({"coord_id": &c1_id, "destination": {"parent_coord_id": &c3_id, "edge": "side"}}),
        )
        .to_string();
    assert!(cycle_err.to_lowercase().contains("cycle") || cycle_err.to_lowercase().contains("under itself") || cycle_err.to_lowercase().contains("descendant"), "unexpected error: {cycle_err}");

    // Detaching c3 with children=detach removes c3 & cz and the incident edge, leaving c1, c4, and X intact.
    let detached = confirm(&mut mcp, "detach_coordinate", json!({"coord_id": &c3_id, "children": "detach"}));
    assert_eq!(detached["applied"], true);
    let listed_after = mcp.ok("list_coordinates", json!({"at": "0/1"}));
    assert_eq!(listed_after["coordinates"].as_array().unwrap().len(), 3);
    // Bookmark to retired c3 reports removed.
    let removed_err = mcp.refused("get_card", json!({"at": format!("place:{c3_id}")})).to_string();
    assert!(removed_err.to_lowercase().contains("removed") || removed_err.to_lowercase().contains("detached"), "got: {removed_err}");
}

#[test]
fn root_multiplicity_literature_mc3_and_secondary_link_refusal() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    let _ = mcp.ok("create_topic", json!({"title": "Firms", "body": "Topic root."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/0", "title": "Claim 1", "body": "Body."}));
    let _ = mcp.ok(
        "create_literature",
        json!({
            "bibtex": "@book{Hart1995,\n  author={Hart},\n  title={Firms},\n  year={1995}\n}",
            "title": "Hart 1995",
            "body": "Book root."
        }),
    );
    let _ = mcp.ok("create_successor", json!({"at": "Hart1995", "title": "Ch 1", "body": "Chapter 1."}));

    // MC3: Literature tree admits repeated Coordinates of its own filed Cards (including its root!)
    let lit_tree = mcp.ok("get_tree_coordinates", json!({"tree": "Hart1995"}));
    let ch1_coord_id = lit_tree["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == "Hart1995/1")
        .unwrap()["coord_id"]
        .as_str()
        .unwrap()
        .to_string();

    let repeat_root = mcp.ok(
        "add_coordinate",
        json!({"tree": "Hart1995", "parent_coord_id": &ch1_coord_id, "edge": "side", "target_at": "Hart1995"}),
    );
    assert_eq!(repeat_root["coord"], "Hart1995/1|a");
    let repeat_root_id = repeat_root["coord_id"].as_str().unwrap().to_string();

    // MC3: Literature tree REFUSES foreign Cards in v1.
    let foreign_err = mcp
        .refused(
            "add_coordinate",
            json!({"tree": "Hart1995", "parent_coord_id": &ch1_coord_id, "edge": "side", "target_at": "0/1"}),
        )
        .to_string();
    assert!(foreign_err.to_lowercase().contains("literature") || foreign_err.to_lowercase().contains("foreign"), "got: {foreign_err}");

    // Topic tree admits Literature root Hart1995 onto a standard Side slot (0/1|a)!
    let topic_tree = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let t1_coord_id = topic_tree["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == "0/1")
        .unwrap()["coord_id"]
        .as_str()
        .unwrap()
        .to_string();

    let mounted_in_topic = mcp.ok(
        "add_coordinate",
        json!({"tree": "0", "parent_coord_id": &t1_coord_id, "edge": "side", "target_at": "Hart1995"}),
    );
    assert_eq!(mounted_in_topic["coord"], "0/1|a");
    let mounted_topic_id = mounted_in_topic["coord_id"].as_str().unwrap().to_string();

    // Citing Secondary Coordinate path `[[0/1|a]]` MUST be refused with hint naming `[[Hart1995]]` and `place:<coord_id>`!
    let link_err = mcp
        .refused(
            "create_successor",
            json!({"at": "0/1", "title": "Bad Citation", "body": "See [[0/1|a]] for Hart."}),
        )
        .to_string();
    assert!(link_err.contains("Hart1995") && link_err.contains(&mounted_topic_id), "expected canonical + bookmark hint, got: {link_err}");

    // Legacy `+k` attach write is retired (MC1).
    let attach_err = mcp
        .refused(
            "place_card",
            json!({"anchor_at": "0/1", "edge": "attach", "target_at": "Hart1995"}),
        )
        .to_string();
    assert!(attach_err.to_lowercase().contains("retire") || attach_err.to_lowercase().contains("add_coordinate") || attach_err.to_lowercase().contains("side"), "got: {attach_err}");

    // Detaching both Secondary Coordinates leaves Hart1995 defining root & BibTeX intact.
    let _ = confirm(&mut mcp, "detach_coordinate", json!({"coord_id": &repeat_root_id}));
    let _ = confirm(&mut mcp, "detach_coordinate", json!({"coord_id": &mounted_topic_id}));
    let hart = mcp.ok("get_card", json!({"at": "Hart1995"}));
    assert_eq!(hart["address"], "Hart1995");
    assert!(hart["bibtex"].as_str().unwrap().contains("Hart1995"));
}

#[test]
fn mc4_splice_guard_refile_card_and_survivor_floor_work() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    let _ = mcp.ok("create_topic", json!({"title": "Models", "body": "Root."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/0", "title": "Model M1", "body": "Question.", "type": "model"}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": "Setting", "body": "Agents.", "type": "model", "facet": "设定"}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1|a", "title": "Result", "body": "Outcome.", "type": "model", "facet": "结果"}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "title": "Plain Note", "body": "Extra."}));

    // MC4 clean splice guard: splicing a Secondary Coordinate directly between two active filing
    // Model parts (0/1|a -> 0/1|a|1) MUST be refused!
    let t1 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let setting_id = t1["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == "0/1|a")
        .unwrap()["coord_id"]
        .as_str()
        .unwrap()
        .to_string();
    let splice_err = mcp
        .refused(
            "add_coordinate",
            json!({"tree": "0", "parent_coord_id": &setting_id, "edge": "direct", "target_at": "0/2"}),
        )
        .to_string();
    assert!(splice_err.to_lowercase().contains("side") || splice_err.to_lowercase().contains("model"), "got: {splice_err}");

    // Survivor-set floor: trying to detach the sole Coordinate of Plain Note (0/2) MUST be refused.
    let plain_id = t1["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == "0/2")
        .unwrap()["coord_id"]
        .as_str()
        .unwrap()
        .to_string();
    let floor_err = mcp.refused("detach_coordinate", json!({"coord_id": &plain_id})).to_string();
    assert!(floor_err.to_lowercase().contains("last") || floor_err.to_lowercase().contains("filing") || floor_err.to_lowercase().contains("delete"), "got: {floor_err}");

    // Place Plain Note at a second Coordinate in Topic 1, then refile_card to Topic 1 and detach 0/2!
    let _ = mcp.ok("create_topic", json!({"title": "Topic 1", "body": "Second root."}));
    let t2 = mcp.ok("get_tree_coordinates", json!({"tree": "1"}));
    let t2_root_id = t2["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == "1/0")
        .unwrap()["coord_id"]
        .as_str()
        .unwrap()
        .to_string();

    let c2 = mcp.ok(
        "add_coordinate",
        json!({"tree": "1", "parent_coord_id": &t2_root_id, "edge": "direct", "target_at": "0/2"}),
    );
    let c2_id = c2["coord_id"].as_str().unwrap().to_string();
    assert_eq!(c2["coord"], "1/1");

    // Refile 0/2 -> c2 (1/1).
    let refiled = confirm(&mut mcp, "refile_card", json!({"at": "0/2", "to_coord_id": &c2_id}));
    assert_eq!(refiled["applied"], true);
    assert_eq!(refiled["address"], "1/1");

    // Now 0/2 is a Secondary Coordinate and detaches cleanly!
    let detached = confirm(&mut mcp, "detach_coordinate", json!({"coord_id": &plain_id}));
    assert_eq!(detached["applied"], true);
    let card = mcp.ok("get_card", json!({"at": "1/1"}));
    assert_eq!(card["address"], "1/1");

    // op_key idempotency with two intentional add_coordinate calls for the same Card.
    let ct = mcp.ok("create_connection_tree", json!({"title": "Idem"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let root_id = ct["root_placement_id"].as_str().unwrap().to_string();
    let r1 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_id, "edge": "direct", "target_at": "1/1", "op_key": "k-1"}));
    let r1_retry = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_id, "edge": "direct", "target_at": "1/1", "op_key": "k-1"}));
    assert_eq!(r1["coord_id"], r1_retry["coord_id"]);
    assert_eq!(r1_retry["replayed"], true);
    let r2 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": r1["coord_id"], "edge": "direct", "target_at": "1/1", "op_key": "k-2"}));
    assert_ne!(r1["coord_id"], r2["coord_id"]);
}
