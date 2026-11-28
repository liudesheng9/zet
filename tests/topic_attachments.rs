//! Topic Secondary Coordinates (MC1: supersedes legacy `<loc>+k` side mounts):
//! Legacy `edge: "attach"` is retired; outside evidence is mounted directly onto standard Side/Direct
//! Coordinates inside a Topic tree (`add_coordinate`), leaving every home projection, Hub coverage,
//! and Link index byte-identical, following its anchor across moves by `coord_id`, becoming a Missing
//! coordinate when its source is deleted, and going with its deleted anchor.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn projections(scratch: &ScratchBox, mcp: &mut Mcp, hub: &str) -> Value {
    let mut tree = mcp.ok("get_tree", json!({"tree": "0", "bodies": true}));
    tree.as_object_mut().unwrap().remove("archive_revision");
    let mut links = mcp.ok("list_links", json!({}));
    links.as_object_mut().map(|m| m.remove("archive_revision"));
    let mut hub = mcp.ok("get_hub", json!({"at": hub}));
    strip_revision(&mut hub);
    let mut broken = mcp.ok("list_broken_links", json!({}));
    broken.as_object_mut().unwrap().remove("missing_mounts");
    let mut card = mcp.ok("get_card", json!({"at": "0/2"}));
    strip_revision(&mut card);
    let out = scratch.cmd().args(["status", "--check-links"]).output().expect("check links");
    let mut check: Value = serde_json::from_slice(&out.stdout).expect("report");
    strip_revision(&mut check);
    json!({"tree": tree, "links": links, "hub": hub, "broken": broken, "card": card, "check": check})
}

fn strip_revision(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("archive_revision");
            map.remove("last_write");
            for v in map.values_mut() {
                strip_revision(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip_revision),
        _ => {}
    }
}

fn evidence_book(mcp: &mut Mcp) -> String {
    mcp.ok("create_literature", json!({"bibtex": "@book{Ev1984, title={Township Output}}"}));
    mcp.ok("create_successor", json!({"at": "Ev1984", "title": "第一章", "body": "本章。\n\n来源：[[Ev1984]] §1"}));
    address(&mcp.ok("create_successor", json!({
        "at": "Ev1984/1", "kind": "side", "type": "event",
        "title": "1982年的承包浪潮",
        "body": "**时间**：1982年\n**地点**：县域\n**参与者**：县政府\n**事件**：推行财政承包。\n\n来源：[[Ev1984]] §1"
    })))
}

fn coord_id_of(mcp: &mut Mcp, tree: &str, path: &str) -> String {
    let coords = mcp.ok("get_tree_coordinates", json!({"tree": tree}));
    coords["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == path)
        .unwrap_or_else(|| panic!("coord {path} not found in {coords}"))["coord_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_side_mount_beside_an_occupied_slot_changes_no_home_projection() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let event = evidence_book(&mut mcp);
    mcp.ok("create_topic", json!({"title": "Fiscal decentralization", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "One", "body": "one"}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Two", "body": "two"}));
    mcp.ok("create_successor", json!({"at": "0/2", "kind": "side", "title": "Two a", "body": "two a"}));
    mcp.ok("create_successor", json!({"at": "0/2", "title": "Three", "body": "See [[0/2|a]] and [[0/2]]."}));
    let hub = address(&mcp.ok("create_successor", json!({"at": "0/3", "kind": "side", "type": "hub", "level": "line", "title": "线索：一到三", "body": "- [[0/1]] 开端\n- [[0/2]] 推进\n- [[0/3]] 结论"})));
    let before = projections(&scratch, &mut mcp, &hub);

    // MC1: legacy edge: "attach" is retired cleanly.
    let retired = mcp.refused("place_card", json!({"anchor_at": "0/2", "edge": "attach", "target_at": &event}));
    assert!(retired.to_string().contains("MC1") || retired.to_string().contains("add_coordinate"), "{retired}");

    // Place the external Event natively onto a Side Coordinate beside 0/2 (0/2|b, leaving 0/2|a untouched).
    let anchor_coord = coord_id_of(&mut mcp, "0", "0/2");
    let mount = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": anchor_coord, "edge": "side", "target_at": &event, "gloss": "County-level timing."}));
    assert_eq!(mount["coord"], "0/2|b");
    assert_eq!(projections(&scratch, &mut mcp, &hub), before, "a Secondary Coordinate changes no home projection");
    let read = mcp.ok("get_card", json!({"at": "0/2|b"}));
    assert_eq!(read["address"], event);
    assert_eq!(read["station"]["coord"], "0/2|b");
    assert_eq!(read["station"]["gloss"], "County-level timing.");
    let event_card = mcp.ok("get_card", json!({"at": &event}));
    assert!(!event_card.to_string().contains("0/2"), "the mounted Card is never written: {event_card}");

    // Detach the Secondary Coordinate; every projection stays identical and 0/2|b is free again.
    let plan = mcp.ok("detach_coordinate", json!({"coord_id": mount["coord_id"]}));
    mcp.ok("detach_coordinate", json!({"coord_id": mount["coord_id"], "confirm": plan["confirm"], "plan_hash": plan["plan_hash"], "expected_revision": plan["expected_revision"]}));
    assert_eq!(projections(&scratch, &mut mcp, &hub), before, "and so does its detach");
    assert_eq!(address(&mcp.ok("create_successor", json!({"at": "0/2", "kind": "side", "title": "Two b", "body": "two b"}))), "0/2|b");
}

#[test]
fn side_mounts_follow_their_anchor_and_their_source() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let event = evidence_book(&mut mcp);
    mcp.ok("create_topic", json!({"title": "Zero", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "One", "body": "one"}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Two", "body": "two"}));
    mcp.ok("create_topic", json!({"title": "One topic", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "1/0", "title": "Elsewhere", "body": "elsewhere"}));
    let anchor_coord = coord_id_of(&mut mcp, "0", "0/2");
    let first = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &anchor_coord, "edge": "side", "target_at": &event}));
    assert_eq!(first["coord"], "0/2|a");
    let second = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &anchor_coord, "edge": "side", "target_at": "Ev1984/1"}));
    assert_eq!(second["coord"], "0/2|b");

    // The anchor moves inside its Topic: Secondary Coordinate children follow by parent_coord_id.
    let plan = mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a"}));
    mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a", "confirm": plan["confirm"]}));
    assert_eq!(mcp.ok("list_placements", json!({"at": &event}))["placements"][0]["coord"], "0/1|a|a");

    // ...and into another Topic: Secondary Coordinate children follow into Topic 1.
    let plan = mcp.ok("move_card", json!({"at": "0/1|a", "to": "1/1|a"}));
    mcp.ok("move_card", json!({"at": "0/1|a", "to": "1/1|a", "confirm": plan["confirm"]}));
    let now = mcp.ok("list_placements", json!({"at": &event}));
    assert_eq!(now["placements"][0]["coord"], "1/1|a|a", "{now}");
    assert_eq!(now["placements"][0]["host"], "1/0");

    // The source is deleted at its home: the Secondary Coordinate stays as a Missing coordinate.
    let plan = mcp.ok("delete_card", json!({"at": &event}));
    assert!(plan.to_string().contains("Missing"), "{plan}");
    mcp.ok("delete_card", json!({"at": &event, "confirm": plan["confirm"]}));
    let broken = mcp.ok("list_broken_links", json!({}));
    let missing = broken["missing_mounts"].as_array().unwrap();
    assert_eq!(missing.len(), 1, "{broken}");
    assert_eq!(missing[0]["coord"], "1/1|a|a");
    assert_eq!(missing[0]["last_address"], event);

    // The anchor is deleted: its Secondary Coordinate children go with it.
    let plan = mcp.ok("delete_card", json!({"at": "1/1|a"}));
    mcp.ok("delete_card", json!({"at": "1/1|a", "confirm": plan["confirm"]}));
    let broken = mcp.ok("list_broken_links", json!({}));
    assert!(broken.get("missing_mounts").is_none(), "{broken}");
    assert_eq!(mcp.ok("list_placements", json!({"at": "Ev1984/1"}))["placements"].as_array().unwrap().len(), 0);
}
