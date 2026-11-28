//! A Card's coordinates in every tree (zt 0.67.0): the page reads, with each Topic or Literature
//! tree, where its Cards also stand (Connection-tree stations and Topic-side mounts), so the Map
//! and the reader can stamp every coordinate on the Card's head.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;

use gui_client::Gui;
use mcp_client::ScratchBox;
use serde_json::json;

#[test]
fn a_tree_answer_names_where_its_cards_also_stand() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex": "@book{Coord2020, title={Coordinates}}"}));
    mcp.ok("create_successor", json!({"at": "Coord2020", "title": "一", "body": "一。\n\n来源：[[Coord2020]] §1"}));
    mcp.ok("create_successor", json!({"at": "Coord2020/1", "title": "二", "body": "二。\n\n来源：[[Coord2020]] §2"}));
    mcp.ok("create_literature", json!({"bibtex": "@book{Other2020, title={Other}}"}));
    mcp.ok("create_connection_tree", json!({"title": "Reading line"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Coord2020", "gloss": "the book"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Coord2020/2"}));
    mcp.ok("create_topic", json!({"title": "Topic", "body": "t"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Anchor", "body": "a"}));
    mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord": "0/1", "edge": "side", "target_at": "Coord2020/2"}));

    let gui = Gui::connect(scratch.cmd());
    let tree = gui.get("/api/tree?id=Coord2020");
    let elsewhere = &tree["elsewhere"];
    assert_eq!(elsewhere["Coord2020"][0]["coord"], "~1/1", "{tree}");
    assert_eq!(elsewhere["Coord2020"][0]["host"], "~1");
    assert_eq!(elsewhere["Coord2020"][0]["gloss"], "the book");
    let second: Vec<&str> = elsewhere["Coord2020/2"].as_array().unwrap().iter().filter_map(|p| p["coord"].as_str()).collect();
    assert_eq!(second, vec!["0/1|a", "~1/2"], "a station and a Topic Secondary Coordinate, in coordinate order");
    assert!(elsewhere.get("Coord2020/1").is_none(), "a Card that stands nowhere else has no entry");
    // Another book with the same prefix of letters is not read into this one.
    let other = gui.get("/api/tree?id=Other2020");
    assert_eq!(other["elsewhere"], json!({}));
}
