//! Seam 3 TDD tests: Locks v2, per-coord_id advisory drift & review, scholarly invariance
//! (Model/Math/Term/Cited/Hub), multi-occurrence crossings projection, and Dependency Lens overlay
//! (docs/multi-coordinate-cards-proposal.md §§8, 11 Acceptance Cases 4-6).

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;

use gui_client::Gui;
use mcp_client::ScratchBox;
use serde_json::{Value, json};

#[test]
fn scholarly_invariance_across_secondary_coordinates_in_model_term_cited_and_hub() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    let _ = mcp.ok(
        "create_literature",
        json!({
            "bibtex": "@book{BookA,\n  author={Author},\n  title={Book A},\n  year={2022}\n}",
            "title": "Book A",
            "body": "Book."
        }),
    );
    let _ = mcp.ok("create_successor", json!({"at": "BookA", "title": "Model M1", "body": "First model.", "type": "model"}));
    let _ = mcp.ok("create_successor", json!({"at": "BookA/1", "kind": "side", "title": "Assumption", "body": "Core assumption.", "type": "model", "facet": "假设"}));
    let _ = mcp.ok("create_successor", json!({"at": "BookA/1", "title": "Model M2", "body": "Second model.", "type": "model"}));
    let _ = mcp.ok("create_successor", json!({"at": "BookA/2", "kind": "side", "title": "M2 Setting", "body": "Setting.", "type": "model", "facet": "设定"}));

    // Topic with a Hub covering 0/1.
    let _ = mcp.ok("create_topic", json!({"title": "Hub Topic", "body": "Root."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/0", "title": "Member A", "body": "Claim A."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "title": "Topic Hub", "body": "## Core\n- [[0/1]]: member A.", "type": "hub", "hub_level": "topic"}));

    // Snapshot golden reports BEFORE adding Secondary Coordinates.
    let mut m1_before = mcp.ok("get_model", json!({"at": "BookA/1"}));
    let mut m2_before = mcp.ok("get_model", json!({"at": "BookA/2"}));
    let mut hub_before = mcp.ok("get_hub", json!({"at": "0/2"}));

    // Place a Secondary Coordinate of BookA/1|a (M1's part) directly under Model M2 (BookA/2|b)!
    let lit = mcp.ok("get_tree_coordinates", json!({"tree": "BookA"}));
    let m2_coord = lit["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "BookA/2").unwrap()["coord_id"].as_str().unwrap().to_string();
    let sec_part = mcp.ok("add_coordinate", json!({"tree": "BookA", "parent_coord_id": &m2_coord, "edge": "side", "target_at": "BookA/1|a"}));
    assert_eq!(sec_part["coord"], "BookA/2|b");

    // Place Member A (0/1) two MORE times in Topic 0 (so it stands 3x in Topic 0).
    let t1 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let t1_coord = t1["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    let _ = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &t1_coord, "edge": "side", "target_at": "0/1"}));
    let _ = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &t1_coord, "edge": "side", "target_at": "0/1"}));

    // Reports MUST be byte-for-byte identical (excluding archive_revision)!
    let mut m1_after = mcp.ok("get_model", json!({"at": "BookA/1"}));
    let mut m2_after = mcp.ok("get_model", json!({"at": "BookA/2"}));
    let mut hub_after = mcp.ok("get_hub", json!({"at": "0/2"}));
    for v in [&mut m1_before, &mut m2_before, &mut hub_before, &mut m1_after, &mut m2_after, &mut hub_after] {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("archive_revision");
        }
    }
    assert_eq!(m1_after, m1_before, "M1 report must be invariant to Secondary Coordinate");
    assert_eq!(m2_after, m2_before, "M2 report must NOT enroll M1's part via Secondary Coordinate");
    assert_eq!(hub_after, hub_before, "Hub coverage must count distinct filed Cards once");
}

#[test]
fn multi_occurrence_crossings_preserve_same_card_lines_and_per_coord_exits() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    let _ = mcp.ok("create_topic", json!({"title": "Plane T", "body": "Root."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "X."}));
    let _ = mcp.ok("create_topic", json!({"title": "Outside", "body": "Root."}));
    let _ = mcp.ok("create_successor", json!({"at": "1/0", "title": "Outside O", "body": "O."}));

    // In Connection tree ~1: X(c1) -> O(c2) -> X(c3), plus connection c1 -> c3 (same Card, different coords!).
    let ct = mcp.ok("create_connection_tree", json!({"title": "Crossing"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let root_id = ct["root_placement_id"].as_str().unwrap().to_string();
    let c1 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_id, "edge": "direct", "target_at": "0/1"}));
    let c1_id = c1["coord_id"].as_str().unwrap().to_string();
    let c2 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1_id, "edge": "direct", "target_at": "1/1"}));
    let c2_id = c2["coord_id"].as_str().unwrap().to_string();
    let c3 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c2_id, "edge": "direct", "target_at": "0/1"}));
    let c3_id = c3["coord_id"].as_str().unwrap().to_string();
    let _ = mcp.ok(
        "connect_placements",
        json!({"tree": &handle, "from_placement_id": &c1_id, "to_placement_id": &c3_id, "direction": "directed", "relation_label": "loops"}),
    );

    let plane = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let cross = &plane["crossings"][&handle];
    let lines = cross["lines"].as_array().expect("crossing lines");
    assert!(
        lines.iter().any(|l| l["kind"] == "structure" && l["from_coord_id"] == c1_id && l["to_coord_id"] == c3_id && l["hops"] == 1),
        "expected structure line c1 -> c3 with hops=1, got: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l["kind"] == "connection" && l["from_coord_id"] == c1_id && l["to_coord_id"] == c3_id),
        "expected connection line c1 -> c3, got: {lines:?}"
    );
    assert_eq!(cross["exits_by_coord"][&c1_id], 1);
    assert_eq!(cross["exits_by_coord"][&c3_id], 1);

    // Check `elsewhere` also surfaces same-tree Secondary Coordinates!
    let t1_x_id = plane["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    let _ = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &t1_x_id, "edge": "side", "target_at": "0/1"}));
    let plane2 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let else_x = plane2["elsewhere"]["0/1"].as_array().expect("elsewhere for 0/1");
    assert!(
        else_x.iter().any(|e| e["coord"] == "0/1|a"),
        "elsewhere must include same-tree sibling coordinates, got: {else_x:?}"
    );
}

#[test]
fn lock_v2_tracks_per_coord_id_drift_and_blocks_pinned_delete_or_refile() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    // Agent Literature card X (BookB/1) placed TWICE in a human Connection tree ~1!
    let _ = mcp.ok(
        "create_literature",
        json!({"bibtex": "@book{BookB,\n author={B},\n title={Book B},\n year={2023}\n}", "title": "Book B", "body": "Root."}),
    );
    let _ = mcp.ok("create_successor", json!({"at": "BookB", "title": "External X", "body": "Original body."}));

    let ct = gui.conn("create_connection_tree", json!({"workspace": "human", "title": "Locked Workspace"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let root_id = ct["root_placement_id"].as_str().unwrap().to_string();
    let c1a = gui.conn("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_id, "edge": "direct", "target_at": "BookB/1"}));
    let c1a_id = c1a["coord_id"].as_str().unwrap().to_string();
    let c1b = gui.conn("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1a_id, "edge": "direct", "target_at": "BookB/1"}));
    let c1b_id = c1b["coord_id"].as_str().unwrap().to_string();

    // Lock ~1 as owner.
    let lock = gui.lock(&handle);
    let lock_id = lock["lock_id"].as_i64().unwrap();

    // Deleting BookB/1 is refused (pinned-delete).
    let del_err = mcp.refused("delete_card", json!({"at": "BookB/1"})).to_string();
    assert!(del_err.contains("pinned-delete") || del_err.contains("card_locked"), "got: {del_err}");

    // Editing BookB/1 succeeds (external source) and causes advisory drift on BOTH c1a and c1b!
    let _ = mcp.ok("edit_card", json!({"at": "BookB/1", "title": "External X", "body": "Revised body."}));
    let tree_after: Value = gui.conn("get_tree_coordinates", json!({"tree": &handle}));
    let find_coord = |id: &str| {
        tree_after["coordinates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["coord_id"] == id)
            .unwrap()
            .clone()
    };
    let d_a = find_coord(&c1a_id)["drift"].clone();
    let d_b = find_coord(&c1b_id)["drift"].clone();
    assert_eq!(d_a["differs"], true);
    assert_eq!(d_a["reviewed"], false);
    assert_eq!(d_b["differs"], true);
    assert_eq!(d_b["reviewed"], false);

    // Owner reviews ONLY c1a -> c1a clears drift, while c1b STILL asks for review!
    let _ = gui.conn(
        "review_source",
        json!({
            "coord_id": &c1a_id,
            "reviewed_fp": d_a["current_fp"],
            "lock_id": lock_id,
        }),
    );
    let tree_reviewed: Value = gui.conn("get_tree_coordinates", json!({"tree": &handle}));
    let r_a = tree_reviewed["coordinates"].as_array().unwrap().iter().find(|c| c["coord_id"] == c1a_id).unwrap()["drift"].clone();
    let r_b = tree_reviewed["coordinates"].as_array().unwrap().iter().find(|c| c["coord_id"] == c1b_id).unwrap()["drift"].clone();
    assert_eq!(r_a["reviewed"], true, "c1a must be marked reviewed");
    assert_eq!(r_b["reviewed"], false, "c1b must stay unreviewed");

    // Lock integrity check reports zero faults!
    let status = scratch.cmd().args(["status", "--check-locks"]).output().expect("zt status --check-locks");
    assert!(status.status.success(), "lock check failed: {}", String::from_utf8_lossy(&status.stderr));
}
