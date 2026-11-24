//! Topic-side mounts `<loc>+k` (docs/multi-tree-coordinates-proposal.md §3.2, §5, Phase 4;
//! Acceptance §10.3 (1)): outside evidence mounted beside a Topic home Card takes no `|a` slot,
//! rewrites no Link, changes no home projection, Hub coverage or structural report, follows its
//! anchor by identity, and leaves a Missing mount when its source is deleted.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// Every home projection a mount must leave untouched: the Topic tree with bodies, the Link list,
/// the Hub report, the Broken links, the Link-index check, and one Card read.
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

/// Acceptance §10.3 (1): mount an external Event at `0/2+1` beside `0/2` while `0/2|a` is occupied,
/// then detach it: every home address, body, Link index row, report and Hub result is identical
/// before, during and after; the next Side successor of `0/2` is still `0/2|b`.
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

    let mount = mcp.ok("place_card", json!({"anchor_at": "0/2", "edge": "attach", "target_at": event, "gloss": "County-level timing."}));
    assert_eq!(mount["coord"], "0/2+1");
    assert_eq!(projections(&scratch, &mut mcp, &hub), before, "a mount changes no home projection");
    let read = mcp.ok("get_card", json!({"at": "0/2+1"}));
    assert_eq!(read["address"], event);
    assert_eq!(read["station"]["coord"], "0/2+1");
    assert_eq!(read["station"]["gloss"], "County-level timing.");
    let event_card = mcp.ok("get_card", json!({"at": event}));
    assert!(!event_card.to_string().contains("0/2"), "the mounted Card is never written: {event_card}");
    // A Topic Card's own home Cards are not mounted beside it.
    let refused = mcp.refused("place_card", json!({"anchor_at": "0/1", "edge": "attach", "target_at": "0/3"}));
    assert!(refused.to_string().contains("home in Topic"), "{refused}");
    // A Card stands once in a Topic: mounting it again answers its station.
    let again = mcp.ok("place_card", json!({"anchor_at": "0/1", "edge": "attach", "target_at": event}));
    assert_eq!(again["already_placed"], true);

    let plan = mcp.ok("unplace", json!({"placement_id": mount["placement_id"]}));
    mcp.ok("unplace", json!({"placement_id": mount["placement_id"], "confirm": plan["confirm"]}));
    assert_eq!(projections(&scratch, &mut mcp, &hub), before, "and so does its detach");
    assert_eq!(address(&mcp.ok("create_successor", json!({"at": "0/2", "kind": "side", "title": "Two b", "body": "two b"}))), "0/2|b");
}

/// §3.1 "Topic-side mount ... anchored to a Topic home Card by identity": the mounts beside one Card
/// keep their order (`before`), follow the anchor through a move inside its Topic and into another
/// Topic, go with a deleted anchor, and become a Missing mount when the source is deleted.
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
    let first = mcp.ok("place_card", json!({"anchor_at": "0/2", "edge": "attach", "target_at": event}));
    let second = mcp.ok("place_card", json!({"anchor_at": "0/2", "edge": "attach", "target_at": "Ev1984/1", "before": first["placement_id"]}));
    assert_eq!(second["coord"], "0/2+1");
    let stations = mcp.ok("list_placements", json!({"at": event}));
    assert_eq!(stations["placements"][0]["coord"], "0/2+2");
    // Reorder: move the first mount back to the front.
    let plan = mcp.ok("move_placement", json!({"placement_id": first["placement_id"], "destination": {"anchor_at": "0/2", "before": second["placement_id"]}}));
    mcp.ok("move_placement", json!({"placement_id": first["placement_id"], "destination": {"anchor_at": "0/2", "before": second["placement_id"]}, "confirm": plan["confirm"]}));
    assert_eq!(mcp.ok("list_placements", json!({"at": event}))["placements"][0]["coord"], "0/2+1");

    // The anchor moves inside its Topic: the mounts follow by identity.
    let plan = mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a"}));
    mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a", "confirm": plan["confirm"]}));
    assert_eq!(mcp.ok("list_placements", json!({"at": event}))["placements"][0]["coord"], "0/1|a+1");
    // ...and into another Topic: the mounts move to that Topic's host.
    let plan = mcp.ok("move_card", json!({"at": "0/1|a", "to": "1/1|a"}));
    mcp.ok("move_card", json!({"at": "0/1|a", "to": "1/1|a", "confirm": plan["confirm"]}));
    let now = mcp.ok("list_placements", json!({"at": event}));
    assert_eq!(now["placements"][0]["coord"], "1/1|a+1", "{now}");
    assert_eq!(now["placements"][0]["host"], "1/0");

    // The source is deleted at its home: the mount stays as a Missing mount.
    let plan = mcp.ok("delete_card", json!({"at": event}));
    assert!(plan.to_string().contains("Missing mounts"), "{plan}");
    mcp.ok("delete_card", json!({"at": event, "confirm": plan["confirm"]}));
    let broken = mcp.ok("list_broken_links", json!({}));
    let missing = broken["missing_mounts"].as_array().unwrap();
    assert_eq!(missing.len(), 1, "{broken}");
    assert_eq!(missing[0]["coord"], "1/1|a+1");
    assert_eq!(missing[0]["last_address"], event);

    // The anchor is deleted: its mounts go with it.
    let plan = mcp.ok("delete_card", json!({"at": "1/1|a"}));
    mcp.ok("delete_card", json!({"at": "1/1|a", "confirm": plan["confirm"]}));
    let broken = mcp.ok("list_broken_links", json!({}));
    assert_eq!(broken["missing_mounts"].as_array().unwrap().len(), 0, "{broken}");
    assert_eq!(mcp.ok("list_placements", json!({"at": "Ev1984/1"}))["placements"].as_array().unwrap().len(), 0);
}
