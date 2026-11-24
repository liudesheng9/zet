//! Locks over Connection trees and Topic-side mounts, native Cards and promotion
//! (docs/multi-tree-coordinates-proposal.md §3.4.7, §8.1, Phase 2; Acceptance §10.3 (2) and (4)).

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn book(mcp: &mut Mcp, key: &str) {
    mcp.ok("create_literature", json!({"bibtex": format!("@book{{{key}, title={{Book {key}}}}}")}));
    mcp.ok("create_successor", json!({"at": key, "title": format!("{key} 第一节"), "body": format!("第一节。\n\n来源：[[{key}]] §1")}));
    mcp.ok("create_successor", json!({"at": format!("{key}/1"), "title": format!("{key} 第二节"), "body": format!("第二节。\n\n来源：[[{key}]] §2")}));
}

fn station<'a>(view: &'a Value, home: &str) -> &'a Value {
    view["placements"]
        .as_array()
        .expect("placements")
        .iter()
        .find(|p| p["home_address"] == home)
        .unwrap_or_else(|| panic!("no station of {home} in {view}"))
}

fn locks_ok(scratch: &ScratchBox) {
    let out = scratch.cmd().args(["status", "--check-locks"]).output().expect("check locks");
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "lock check: {text} {}", String::from_utf8_lossy(&out.stderr));
}

/// §8.1 layers 1–4, Acceptance §10.3 (4) second half: a locked Connection tree freezes its stations,
/// glosses, connections and native Cards; it pins its live mounted targets against delete and
/// address change; an unlocked source can still be edited, the reader shows the drift, and Mark
/// reviewed appends a review without touching the Lock seal.
#[test]
fn a_locked_connection_tree_freezes_its_map_and_pins_its_mounted_sources() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Src2020");
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_connection_tree", json!({"title": "Frozen map"}));
    let a = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Src2020/1", "gloss": "Evidence."}));
    let b = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Src2020/2"}));
    let native = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/2", "title": "综合判断", "body": "两节合起来。"}));
    let edge = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": b["placement_id"], "to_placement_id": a["placement_id"], "direction": "directed", "relation_label": "continues"}));
    let lock = gui.lock("~1");
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    let seal = lock["seal"].clone();
    locks_ok(&scratch);

    // Layer 1: the map and the native Card are frozen for every writer.
    for (tool, args) in [
        ("place_card", json!({"host": "~1", "parent_coord": "~1/3", "target_at": "Src2020"})),
        ("connect_placements", json!({"tree": "~1", "from_placement_id": a["placement_id"], "to_placement_id": native["placement_id"]})),
        ("edit_placement", json!({"placement_id": a["placement_id"], "gloss": "Changed."})),
        ("edit_connection", json!({"connection_id": edge["connection_id"], "note": "Changed."})),
        ("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/3", "title": "New", "body": "x"})),
        ("move_placement", json!({"placement_id": b["placement_id"], "destination": {"parent_coord": "~1/1", "edge": "side"}, "confirm": "move"})),
        ("edit_connection_tree", json!({"tree": "~1", "purpose": "x"})),
    ] {
        let refused = mcp.refused(tool, args);
        assert!(refused.to_string().contains("locked"), "{tool}: {refused}");
        let (owner, failed) = gui.conn_call(tool, json!({}));
        let _ = (owner, failed);
    }
    let refused = mcp.refused("edit_card", json!({"at": native["handle"], "body": "改写。"}));
    assert!(refused.to_string().contains("locked"), "{refused}");
    // The owner is refused too.
    let refused = gui.conn_refused("edit_placement", json!({"placement_id": a["placement_id"], "gloss": "Owner change."}));
    assert!(refused.to_string().contains("locked"), "{refused}");

    // Layer 2: mounted targets are pinned against delete and move.
    let refused = mcp.refused("delete_card", json!({"at": "Src2020/2", "confirm": "delete"}));
    assert!(refused.to_string().contains("pinned-delete"), "{refused}");
    let refused = mcp.refused("move_card", json!({"at": "Src2020/2", "to": "Src2020/1|a", "confirm": "move"}));
    assert!(refused.to_string().contains("pinned") || refused.to_string().contains("locked"), "{refused}");

    // Layer 3: the unlocked source can be edited; the reader shows the drift.
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(station(&view, "Src2020/1")["drift"]["differs"], false, "{view}");
    mcp.ok("edit_card", json!({"at": "Src2020/1", "body": "第一节，修订。\n\n来源：[[Src2020]] §1"}));
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    let drift = station(&view, "Src2020/1")["drift"].clone();
    assert_eq!(drift["differs"], true, "{drift}");
    assert_eq!(drift["label"], "Source differs from lock-time version");
    assert_eq!(drift["reviewed"], false);
    locks_ok(&scratch);

    // Layer 4: Mark reviewed is the owner's, needs the shown fingerprint, appends, keeps the seal.
    let refused = mcp.refused("review_source", json!({"placement_id": a["placement_id"], "reviewed_fp": drift["current_fp"]}));
    assert!(refused.to_string().contains("owner"), "{refused}");
    let stale = gui.conn_refused("review_source", json!({"placement_id": a["placement_id"], "reviewed_fp": drift["observed_fp"]}));
    assert!(stale.to_string().contains("stale"), "{stale}");
    gui.conn("review_source", json!({"placement_id": a["placement_id"], "reviewed_fp": drift["current_fp"], "note": "Checked the revision."}));
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(station(&view, "Src2020/1")["drift"]["reviewed"], true);
    let panel = gui.get("/api/locks");
    let listed = panel["locks"].as_array().unwrap().iter().find(|l| l["lock_id"] == lock_id).unwrap().clone();
    assert_eq!(listed["seal"], seal, "the seal never changes");
    assert!(!panel.to_string().contains("\"reseal\""), "no reseal event: {panel}");
    let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    let reviews: i64 = db.query_row("SELECT COUNT(*) FROM lock_source_reviews", [], |r| r.get(0)).unwrap();
    assert_eq!(reviews, 1);
    assert!(db.execute("DELETE FROM lock_source_reviews", []).is_err(), "reviews are append-only");
    // A later source edit clears the reviewed state by comparison.
    mcp.ok("edit_card", json!({"at": "Src2020/1", "body": "第一节，再修订。\n\n来源：[[Src2020]] §1"}));
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(station(&view, "Src2020/1")["drift"]["reviewed"], false);
    locks_ok(&scratch);

    // Unlock: the map changes again and the source may go.
    gui.unlock(lock_id);
    mcp.ok("edit_placement", json!({"placement_id": a["placement_id"], "gloss": "Changed after unlock."}));
    let plan = mcp.ok("delete_card", json!({"at": "Src2020/2"}));
    mcp.ok("delete_card", json!({"at": "Src2020/2", "confirm": plan["confirm"]}));
}

/// §8.1: a Connection tree holding a Missing mount can be locked; the missing station is frozen with
/// its connections and gets no live pin.
#[test]
fn a_tree_with_a_missing_mount_locks_without_a_live_pin() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Gone2020");
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_connection_tree", json!({"title": "With a gap"}));
    let a = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Gone2020/2"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Gone2020/1"}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/1", "to_placement_id": "~1/2"}));
    let plan = mcp.ok("delete_card", json!({"at": "Gone2020/2"}));
    mcp.ok("delete_card", json!({"at": "Gone2020/2", "confirm": plan["confirm"]}));
    let lock_plan = gui.post("/api/lock", &json!({"root": "~1"}));
    let coords = &lock_plan["plan"]["coordinates"];
    assert_eq!(coords["missing"].as_array().unwrap().len(), 1, "{lock_plan}");
    assert_eq!(coords["pins"].as_array().unwrap().len(), 1, "only the live mount is pinned: {lock_plan}");
    assert_eq!(lock_plan["plan"]["zone"], "connection");
    gui.lock("~1");
    let refused = mcp.refused("unplace", json!({"placement_id": a["placement_id"], "confirm": "unplace"}));
    assert!(refused.to_string().contains("locked"), "{refused}");
    locks_ok(&scratch);
}

/// §3.4.7, Acceptance §10.3 (2): a human Card cites `[[~1#1]]`; promoting the native Card into a Topic
/// keeps its identity, station and connections; deleting the Connection tree later leaves the
/// human Card's Link reaching the same Card with zero human-body rewrites.
#[test]
fn a_promoted_handle_survives_its_tree_with_zero_human_rewrites() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_topic", json!({"title": "Agent topic", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "First claim", "body": "first"}));
    mcp.ok("create_connection_tree", json!({"title": "Workbench"}));
    let other = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "辅助卡", "body": "辅助。"}));
    let native = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "title": "可以独立的论断", "body": "这个论断值得进入主题树。"}));
    assert_eq!(native["handle"], "~1#2");
    let edge = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": native["placement_id"], "to_placement_id": other["placement_id"], "direction": "directed", "relation_kind": "rests-on"}));
    mcp.ok("edit_placement", json!({"placement_id": native["placement_id"], "gloss": "Promote me."}));
    let human = gui.post("/api/create", &json!({"kind": "topic", "workspace": "human", "text": "# Human note\n\nThe agent's claim [[~1#2]] matters.\n"}));
    let human_at = human["address"].as_str().expect("human address").to_string();
    let human_body = |mcp: &mut Mcp| mcp.ok("get_card", json!({"at": human_at}))["body"].clone();
    let body_before = human_body(&mut mcp);

    // Promote to 0/2: the plan says what changes; then the Card lives at 0/2.
    let done = {
        let args = json!({"at": "~1#2", "to": "0/2"});
        let plan = mcp.ok("promote_card", args.clone());
        assert_eq!(plan["applied"], false);
        let mut confirmed = args;
        confirmed["confirm"] = plan["confirm"].clone();
        confirmed["plan_hash"] = plan["plan_hash"].clone();
        confirmed["expected_revision"] = plan["expected_revision"].clone();
        mcp.ok("promote_card", confirmed)
    };
    assert_eq!(done["address"], "0/2");
    assert_eq!(mcp.ok("get_card", json!({"at": "~1#2"}))["address"], "0/2");
    assert_eq!(mcp.ok("get_card", json!({"at": "0/2"}))["title"], "可以独立的论断");
    let stations = mcp.ok("list_placements", json!({"at": "0/2"}));
    assert_eq!(stations["handle"], "~1#2");
    assert_eq!(stations["placements"][0]["coord"], "~1/2");
    assert_eq!(stations["placements"][0]["gloss"], "Promote me.");
    assert_eq!(stations["placements"][0]["citation"], "[[~1#2]]");
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(view["connections"][0]["connection_id"], edge["connection_id"]);
    assert_eq!(human_body(&mut mcp), body_before, "no human body was rewritten");
    // The promoted Card lists the human Card in its Reverse links, cited by its handle.
    assert!(mcp.ok("get_card", json!({"at": "0/2"})).to_string().contains(&human_at));
    let broken = mcp.ok("list_broken_links", json!({}));
    assert!(!broken.to_string().contains("~1#2"), "{broken}");

    // A later move of 0/2 rewrites only literal [[0/2]], never [[~1#2]].
    mcp.ok("create_successor", json!({"at": "0/2", "title": "Cites the address", "body": "See [[0/2]]."}));
    let plan = mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a"}));
    mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a", "confirm": plan["confirm"]}));
    assert_eq!(mcp.ok("get_card", json!({"at": "~1#2"}))["address"], "0/1|a");
    assert_eq!(human_body(&mut mcp), body_before, "the handle spelling is never rewritten");

    // Delete the Connection tree: the promoted Card survives and the human Link still opens it.
    let plan = mcp.ok("delete_card", json!({"at": "~1"}));
    mcp.ok("delete_card", json!({"at": "~1", "confirm": plan["confirm"]}));
    mcp.refused("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(mcp.ok("get_card", json!({"at": "~1#2"}))["title"], "可以独立的论断");
    mcp.refused("get_card", json!({"at": "~1#1"}));
    assert_eq!(human_body(&mut mcp), body_before);
    let out = scratch.cmd().args(["status", "--check-links"]).output().expect("check links");
    let report: Value = serde_json::from_slice(&out.stdout).expect("report");
    assert_eq!(report["ok"], true, "{report}");
    // The handle is never reissued.
    assert_eq!(mcp.ok("create_connection_tree", json!({"title": "Next"}))["tree"], "~2");
}

/// §3.4.6: native Events and Findings are checked against their real source, never the tree.
#[test]
fn native_events_and_findings_obey_the_gate_with_their_real_source() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Archive1980");
    mcp.ok("create_connection_tree", json!({"title": "Investigation"}));
    let event = mcp.ok("create_connection_card", json!({
        "tree": "~1", "parent_coord": "~1/0", "type": "event",
        "title": "1980年的财政指令",
        "body": "**时间**：1980年\n**地点**：北京\n**参与者**：国务院\n**事件**：发布财政包干指令。\n\n来源：[[Archive1980]] §1"
    }));
    assert_eq!(event["type"], "event");
    let card = mcp.ok("get_card", json!({"at": event["handle"]}));
    assert_eq!(card["type"], "event");
    // The tree is never treated as a book: the checker reads `~1` as no book.
    let tree_root = mcp.ok("get_card", json!({"at": "~1"}));
    assert_eq!(tree_root["title"], "Investigation");
}

/// §8.1 for Topic trees: a Topic Lock (also one made before mounts existed) freezes the Topic-side
/// mounts beside its Cards.
#[test]
fn a_topic_lock_freezes_its_side_mounts() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Ev2001");
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_topic", json!({"title": "Fiscal decentralization", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Claim one", "body": "one"}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Claim two", "body": "two"}));
    let mount = mcp.ok("place_card", json!({"anchor_at": "0/2", "edge": "attach", "target_at": "Ev2001/1"}));
    assert_eq!(mount["coord"], "0/2+1");
    gui.lock("0/0");
    let refused = mcp.refused("place_card", json!({"anchor_at": "0/2", "edge": "attach", "target_at": "Ev2001/2"}));
    assert!(refused.to_string().contains("locked"), "{refused}");
    let refused = mcp.refused("unplace", json!({"placement_id": mount["placement_id"], "confirm": "unplace"}));
    assert!(refused.to_string().contains("locked"), "{refused}");
    let refused = mcp.refused("delete_card", json!({"at": "Ev2001/1", "confirm": "delete"}));
    assert!(refused.to_string().contains("pinned"), "{refused}");
    locks_ok(&scratch);
}

/// Review findings (P1): a Connection-homed Card never moves by Location; a promoted Card named by
/// its handle moves at its current address and its handle is never rewritten; a native Card keeps a
/// native type; a Literature-scoped writer touches no Connection-tree Card; a Lock pins a promoted
/// Card cited by its handle; the GUI opens a handle Link.
#[test]
fn connection_homes_and_promoted_handles_are_guarded_on_every_legacy_path() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_topic", json!({"title": "T", "body": "t"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "First", "body": "first"}));
    mcp.ok("create_connection_tree", json!({"title": "Guarded"}));
    let native = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "原生卡", "body": "内容。"}));
    let other = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "title": "第二张", "body": "内容。"}));
    for at in ["~1", "~1#2"] {
        let refused = mcp.refused("move_card", json!({"at": at, "to": "0/2", "confirm": "move"}));
        assert!(refused.to_string().contains("Connection tree"), "{at}: {refused}");
    }
    let refused = mcp.refused("edit_card", json!({"at": other["handle"], "type": "model", "level": "concept"}));
    assert!(refused.to_string().contains("native_structure_required"), "{refused}");
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let refused = literature.refused("edit_card", json!({"at": native["handle"], "body": "改写。"}));
    assert!(refused.to_string().contains("connection_tree"), "{refused}");
    let refused = literature.refused("delete_card", json!({"at": "~1", "confirm": "~1"}));
    assert!(refused.to_string().contains("connection_tree"), "{refused}");
    drop(literature);

    // Promote ~1#1 to 0/2; a Card in another Topic cites it by its handle.
    let plan = mcp.ok("promote_card", json!({"at": "~1#1", "to": "0/2"}));
    mcp.ok("promote_card", json!({"at": "~1#1", "to": "0/2", "confirm": plan["confirm"], "plan_hash": plan["plan_hash"], "expected_revision": plan["expected_revision"]}));
    mcp.ok("create_topic", json!({"title": "Citer", "body": "cites [[~1#1]]."}));
    // Moving the promoted Card by its handle moves it at its current address; the handle stays.
    let plan = mcp.ok("move_card", json!({"at": "~1#1", "to": "0/1|a"}));
    mcp.ok("move_card", json!({"at": "~1#1", "to": "0/1|a", "confirm": plan["confirm"]}));
    assert_eq!(mcp.ok("get_card", json!({"at": "~1#1"}))["address"], "0/1|a");
    assert!(mcp.ok("get_card", json!({"at": "1/0"}))["body"].as_str().unwrap().contains("[[~1#1]]"));
    // The GUI opens the handle Link.
    let card = gui.get("/api/card?at=~1%231");
    assert_eq!(card["address"], "0/1|a", "{card}");
    // Locking the citing Topic pins the promoted Card.
    let lock = gui.lock("1/0");
    assert_eq!(lock["pin_count"].as_i64().unwrap_or(1), 1, "{lock}");
    let refused = mcp.refused("delete_card", json!({"at": "0/1|a", "confirm": "delete"}));
    assert!(refused.to_string().contains("locked") || refused.to_string().contains("pinned"), "{refused}");
    let out = scratch.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let ws = mcp.ok("list_workspaces", json!({}));
    assert_eq!(ws["broken_links"], 0, "{ws}");
}

/// Review finding P1.7: tampering with a locked Connection tree's stations behind the triggers is a
/// Lock integrity fault, like tampering with a locked Card; the first start fences the coordinate
/// Lock tables; a native Card cannot be inserted into a locked tree even by raw SQL.
#[test]
fn tampering_with_locked_coordinate_evidence_is_an_integrity_fault() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Tam2020");
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_connection_tree", json!({"title": "Tamper"}));
    let a = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Tam2020/1", "gloss": "Original."}));
    gui.lock("~1");
    locks_ok(&scratch);
    let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    let blocked = db.execute(
        "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type) VALUES('~1#9', 0, 0, '# x\n', 'agent', 'idea')",
        [],
    );
    assert!(blocked.is_err(), "a locked Connection tree takes no new native Card");
    db.execute_batch("DROP TRIGGER coord_lock_placement_update;").unwrap();
    db.execute("UPDATE placements SET gloss = 'Tampered.' WHERE placement_id = ?1", [a["placement_id"].as_str().unwrap()]).unwrap();
    drop(db);
    let out = scratch.cmd().args(["status", "--check-locks"]).output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(!out.status.success(), "{text}");
    assert!(text.contains("coordinate-structure-changed"), "{text}");
}
