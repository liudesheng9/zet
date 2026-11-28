//! Seam 4 TDD tests: MCP discovery for Coordinate tools, Terminal Session unified coordinate
//! navigation/verbs & stale bookmark recovery, GUI `connections.js` coord_id-keyed rules,
//! Dump v2 (`coordinates.json`), and Portable v4 export with repeated occurrences
//! (docs/multi-coordinate-cards-proposal.md §§8.9, 11 P4).

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;

use gui_client::Gui;
use mcp_client::ScratchBox;
use serde_json::{Value, json};
use std::fs;
use tempfile::tempdir;

#[test]
fn mcp_tools_list_exposes_all_new_coordinate_tools_without_duplicates() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let reply = mcp.request("tools/list", json!({}));
    let tools = reply["result"]["tools"].as_array().expect("tools array");
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    // No duplicate tool names.
    let unique: std::collections::BTreeSet<&str> = names.iter().copied().collect();
    assert_eq!(unique.len(), names.len(), "duplicate MCP tool names found");

    for required in [
        "list_coordinates",
        "get_tree_coordinates",
        "add_coordinate",
        "add_coordinates",
        "move_coordinate",
        "reorder_coordinates",
        "detach_coordinate",
        "refile_card",
        "rebind_coordinate",
        "delete_card",
    ] {
        assert!(unique.contains(required), "missing MCP tool `{required}`");
    }
}

#[test]
fn dump_v2_and_portable_v4_export_preserve_repeated_coordinates_and_owner_workspace() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    // Human Topic 0 with Card A, plus agent Literature LitA with Note 1.
    let _ = gui.post("/api/create", &json!({"kind": "topic", "workspace": "human", "text": "# Human Topic\n\nHuman root."}));
    let _ = mcp.ok(
        "create_literature",
        json!({"bibtex": "@book{LitA,\n author={A},\n title={Lit A},\n year={2024}\n}", "title": "Lit A", "body": "Book."}),
    );
    let _ = mcp.ok("create_successor", json!({"at": "LitA", "title": "Agent Note", "body": "Agent body."}));

    // Mount Agent Note TWICE in a human Connection tree ~1 and once in Human Topic 1!
    let ct = gui.conn("create_connection_tree", json!({"workspace": "human", "title": "Portable Multi"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let root_id = ct["root_placement_id"].as_str().unwrap().to_string();
    let c1 = gui.conn("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_id, "edge": "direct", "target_at": "LitA/1", "gloss": "First seat"}));
    let c1_id = c1["coord_id"].as_str().unwrap().to_string();
    let c2 = gui.conn("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1_id, "edge": "direct", "target_at": "LitA/1", "gloss": "Second seat"}));
    let c2_id = c2["coord_id"].as_str().unwrap().to_string();
    let _ = gui.conn(
        "connect_placements",
        json!({"tree": &handle, "from_placement_id": &c1_id, "to_placement_id": &c2_id, "direction": "directed", "relation_label": "echoes"}),
    );

    // 1. Export Connection tree ~1 -> Portable v4 with deduplicated Card body, owner_workspace,
    //    and BOTH repeated coordinates!
    let out_dir = tempdir().expect("export dir");
    let out_path = out_dir.path().join("conn-export");
    let exp = scratch
        .cmd()
        .args(["export", &handle, "--out", out_path.to_str().unwrap()])
        .output()
        .expect("zt export");
    assert!(
        exp.status.success(),
        "zt export failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&exp.stdout),
        String::from_utf8_lossy(&exp.stderr)
    );
    let cards_json: Value = serde_json::from_str(&fs::read_to_string(out_path.join("cards.json")).unwrap()).unwrap();
    assert_eq!(cards_json["version"], 4, "Connection tree export must be Portable v4");
    let exported_cards = cards_json["cards"].as_array().unwrap();
    let lit_copies = exported_cards.iter().filter(|c| c["address"] == "LitA/1").count();
    assert_eq!(lit_copies, 1, "Portable v4 must deduplicate repeated Card bodies");
    let lit_card = exported_cards.iter().find(|c| c["address"] == "LitA/1").unwrap();
    assert_eq!(lit_card["workspace"], "agent", "mounted agent Card keeps owner_workspace offline");

    let coords = cards_json["coordinates"]
        .as_array()
        .or_else(|| cards_json["connection_tree"]["stations"].as_array())
        .expect("exported coordinates");
    let lit_coords = coords.iter().filter(|s| s["address"] == "LitA/1").count();
    assert_eq!(lit_coords, 2, "Portable v4 must retain both repeated Coordinates of LitA/1");

    // 2. Also export a Topic tree with a Secondary Coordinate -> Portable v4!
    let t1 = gui.conn("get_tree_coordinates", json!({"tree": "0"}));
    let t1_root_id = t1["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/0").unwrap()["coord_id"].as_str().unwrap().to_string();
    let _ = gui.conn("add_coordinate", json!({"tree": "0", "parent_coord_id": &t1_root_id, "edge": "direct", "target_at": "LitA/1"}));
    let topic_out = out_dir.path().join("topic-export");
    let exp_topic = scratch
        .cmd()
        .args(["export", "0/0", "--out", topic_out.to_str().unwrap()])
        .output()
        .expect("zt export topic");
    assert!(exp_topic.status.success(), "topic export failed: {}", String::from_utf8_lossy(&exp_topic.stderr));
    let topic_json: Value = serde_json::from_str(&fs::read_to_string(topic_out.join("cards.json")).unwrap()).unwrap();
    assert_eq!(topic_json["version"], 4, "Topic tree with Secondary Coordinates must export Portable v4");

    // 3. `zt dp` writes coordinates.json version 2 with unified trees, coordinates, coordinate_edges!
    scratch.cmd().arg("dp").write_stdin("1\n").assert().success();
    let zip_path = fs::read_dir(scratch.archive_root.join("dump"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().and_then(|s| s.to_str()) == Some("zip"))
        .expect("dump zip archive");
    let mut zip = zip::ZipArchive::new(fs::File::open(&zip_path).unwrap()).unwrap();
    let mut raw = String::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        if entry.name().ends_with("/coordinates.json") || entry.name() == "coordinates.json" {
            std::io::Read::read_to_string(&mut entry, &mut raw).unwrap();
            break;
        }
    }
    assert!(!raw.is_empty(), "coordinates.json not found in dump zip");
    let coord_dump: Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(coord_dump["version"], 2, "dump coordinates.json must be version 2");
    assert!(coord_dump["trees"].as_array().unwrap().len() >= 3);
    assert!(coord_dump["coordinates"].as_array().unwrap().len() >= 5);
    assert_eq!(coord_dump["coordinate_edges"].as_array().unwrap().len(), 1);
}
