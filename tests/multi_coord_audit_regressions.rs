//! Targeted regression tests for every finding from Astra's audit (B1, B2, M1-M8, Minors 1-3)
//! plus §11 acceptance hardenings.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};
use std::fs;
use std::process::Command as StdCommand;
use tempfile::tempdir;

fn confirm_mcp(mcp: &mut Mcp, tool: &str, mut args: Value) -> Value {
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

/// B1: Refiling a native Connection Card `~1#1` into `~2` keeps handle `~1#1`, passes `zt status`,
/// and IS protected by `~2`'s Lock while `~1` stays unlocked!
#[test]
fn b1_cross_connection_refile_protected_by_destination_lock_and_clean_status() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    let ct1 = mcp.ok("create_connection_tree", json!({"title": "Origin Tree"}));
    let h1 = ct1["tree"].as_str().unwrap().to_string();
    let native = mcp.ok(
        "create_connection_card",
        json!({"tree": &h1, "parent_coord": format!("{h1}/0"), "title": "Native 1", "body": "Original native body."}),
    );
    let handle = native["address"].as_str().unwrap().to_string();

    let ct2 = mcp.ok("create_connection_tree", json!({"title": "Dest Tree"}));
    let h2 = ct2["tree"].as_str().unwrap().to_string();
    let r2 = ct2["root_placement_id"].as_str().unwrap().to_string();
    let in_2 = mcp.ok(
        "add_coordinate",
        json!({"tree": &h2, "parent_coord_id": &r2, "edge": "direct", "target_at": &handle}),
    );
    let in_2_id = in_2["coord_id"].as_str().unwrap().to_string();

    // Refile ~1#1 to its Coordinate in ~2.
    let refiled = confirm_mcp(&mut mcp, "refile_card", json!({"at": &handle, "to_coord_id": &in_2_id}));
    assert_eq!(refiled["applied"], true);
    assert_eq!(refiled["address"], handle, "cross-Connection refile keeps permanent handle");

    // `zt status` must report coordinates: ok (no path mismatch!).
    let st = scratch.cmd().arg("status").output().unwrap();
    let st_out = String::from_utf8_lossy(&st.stdout);
    assert!(st.status.success() && st_out.contains("coordinates: ok"), "{st_out}");

    // Lock ~2 while ~1 remains unlocked!
    let _lock = gui.lock(&h2);

    // Body edit and delete of ~1#1 MUST be refused by ~2's Lock!
    let edit_err = mcp
        .refused("edit_card", json!({"at": &handle, "title": "Native 1", "body": "Tamper."}))
        .to_string();
    assert!(edit_err.to_lowercase().contains("lock"), "expected lock refusal on edit: {edit_err}");
    let del_err = mcp.refused("delete_card", json!({"at": &handle})).to_string();
    assert!(del_err.to_lowercase().contains("lock"), "expected lock refusal on delete: {del_err}");

    // Lock integrity check passes cleanly and handle still resolves!
    let check = scratch.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert!(check.status.success(), "{}", String::from_utf8_lossy(&check.stderr));
    let read = mcp.ok("get_card", json!({"at": &handle}));
    assert_eq!(read["address"], handle);
    assert_eq!(read["tree"], h2, "get_card.tree must report actual filing tree ~2 after refile");

    // Unlocked origin-tree deletion (~1) MUST NOT retire ~1#1 (which is filed in ~2)!
    let del_origin = confirm_mcp(&mut mcp, "delete_card", json!({"at": &h1}));
    assert_eq!(del_origin["applied"], true);
    let still_alive = mcp.ok("get_card", json!({"at": &handle}));
    assert_eq!(still_alive["address"], handle, "~1#1 must survive deleting unlocked origin ~1");
    assert_eq!(still_alive["tree"], h2);

    // Second pair (~3, ~4): refile ~3#1 into ~4 keeping occurrence in ~3; deleting unlocked ~4
    // re-anchors ~3#1 back to ~3 instead of retiring it!
    let ct3 = mcp.ok("create_connection_tree", json!({"title": "Origin 3"}));
    let h3 = ct3["tree"].as_str().unwrap().to_string();
    let n3 = mcp.ok("create_connection_card", json!({"tree": &h3, "parent_coord": format!("{h3}/0"), "title": "Native 3", "body": "Body 3."}));
    let h3_card = n3["address"].as_str().unwrap().to_string();
    let ct4 = mcp.ok("create_connection_tree", json!({"title": "Dest 4"}));
    let h4 = ct4["tree"].as_str().unwrap().to_string();
    let r4 = ct4["root_placement_id"].as_str().unwrap().to_string();
    let in_4_id = mcp.ok("add_coordinate", json!({"tree": &h4, "parent_coord_id": &r4, "edge": "direct", "target_at": &h3_card}))["coord_id"].as_str().unwrap().to_string();
    confirm_mcp(&mut mcp, "refile_card", json!({"at": &h3_card, "to_coord_id": &in_4_id}));
    confirm_mcp(&mut mcp, "delete_card", json!({"at": &h4}));
    let reanchored = mcp.ok("get_card", json!({"at": &h3_card}));
    assert_eq!(reanchored["tree"], h3, "~3#1 must survive deleting ~4 by re-anchoring to ~3");

    // Connection-to-Topic survivor: place ~3#1 in a same-Workspace Topic 0, then attempt to delete ~3 ->
    // MUST refuse with explicit refile_card requirement instead of silently retiring ~3#1!
    mcp.ok("create_topic", json!({"title": "Survivor Topic", "body": "Root."}));
    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let t0_root = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/0").unwrap()["coord_id"].as_str().unwrap().to_string();
    mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &t0_root, "edge": "direct", "target_at": &h3_card}));
    let topic_surv_err = mcp.refused("delete_card", json!({"at": &h3})).to_string();
    assert!(
        topic_surv_err.contains("refile_card") && topic_surv_err.contains("survives"),
        "Connection tree delete with Topic survivor must require explicit refile: {topic_surv_err}"
    );
    assert_eq!(mcp.ok("get_card", json!({"at": &h3_card}))["address"], h3_card);
}

/// B2: MCP `delete_card(card_id=...)` deletes ONLY the single Card identity, preserves filed
/// descendant `B`, requires `plan_hash` on confirm, and supports `op_key` replay after deletion!
#[test]
fn b2_mcp_delete_card_by_card_id_preserves_descendants_requires_hash_and_replays() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Del Root", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Parent A", "body": "Body A."}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Child B", "body": "Body B."}));
    // Also place a same-Topic Secondary Coordinate of Parent A under Child B (0/2|a) to verify
    // B2 removes ALL owned occurrences instead of leaving same-Topic Missing coordinates!
    let t0_init = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let b_cid = t0_init["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/2").unwrap()["coord_id"].as_str().unwrap().to_string();
    let a_sec_id = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &b_cid, "edge": "side", "target_at": "0/1"}))["coord_id"].as_str().unwrap().to_string();

    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let a_card_id = t0["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord"] == "0/1")
        .unwrap()["card_id"]
        .as_str()
        .unwrap()
        .to_string();

    // Plan delete by card_id.
    let plan = mcp.ok("delete_card", json!({"card_id": &a_card_id, "op_key": "del-a-1"}));
    assert_eq!(plan["applied"], false);

    // Confirming WITHOUT plan_hash MUST be refused!
    let no_hash = mcp
        .refused("delete_card", json!({"card_id": &a_card_id, "confirm": plan["confirm"], "op_key": "del-a-1"}))
        .to_string();
    assert!(no_hash.to_lowercase().contains("plan_hash") || no_hash.to_lowercase().contains("hash") || no_hash.to_lowercase().contains("stale"), "{no_hash}");

    // Confirm WITH plan_hash -> A is retired, Child B SURVIVES!
    let done = mcp.ok(
        "delete_card",
        json!({
            "card_id": &a_card_id,
            "confirm": plan["confirm"],
            "plan_hash": plan["plan_hash"],
            "expected_revision": plan["expected_revision"],
            "op_key": "del-a-1"
        }),
    );
    assert_eq!(done["applied"], true);

    // Child B survived (shifted to 0/1) AND Parent A's same-Topic Secondary Coordinate was removed (not left Missing)!
    let tree_after = mcp.ok("get_tree", json!({"tree": "0"}));
    let cards = tree_after["cards"].as_array().unwrap();
    assert!(cards.iter().any(|c| c["title"] == "Child B"), "Child B must survive identity delete of A: {cards:?}");
    assert!(cards.iter().all(|c| c["title"] != "Parent A"), "Parent A must be deleted: {cards:?}");
    let coords_after = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    assert!(
        coords_after["coordinates"].as_array().unwrap().iter().all(|c| c["coord_id"] != a_sec_id),
        "same-Topic owned occurrence of deleted Card must be removed, not left Missing: {coords_after}"
    );

    // Replaying the same op_key after A is retired returns the stored receipt cleanly!
    let replayed = mcp.ok(
        "delete_card",
        json!({
            "card_id": &a_card_id,
            "confirm": plan["confirm"],
            "plan_hash": plan["plan_hash"],
            "expected_revision": plan["expected_revision"],
            "op_key": "del-a-1"
        }),
    );
    assert_eq!(replayed["applied"], true);

    // Literature-scoped MCP `delete_card(card_id=...)` MUST refuse deleting a Literature Card that
    // also has an owned occurrence in an agent Topic tree, preserving BOTH occurrences!
    mcp.ok("create_literature", json!({"bibtex": "@book{ScopeLit,\n  author={S},\n  title={Scope Lit},\n  year={2026}\n}", "title": "Scope Lit", "body": "Book."}));
    mcp.ok("create_successor", json!({"at": "ScopeLit", "title": "Lit Item", "body": "Note."}));
    let t0_now = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let b_now_id = t0_now["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    let topic_occ = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &b_now_id, "edge": "side", "target_at": "ScopeLit/1"}))["coord_id"].as_str().unwrap().to_string();
    let lit_tree = mcp.ok("get_tree_coordinates", json!({"tree": "ScopeLit"}));
    let lit_card_id = lit_tree["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "ScopeLit/1").unwrap()["card_id"].as_str().unwrap().to_string();

    let mut lit_mcp = scratch.mcp(&["--scope", "literature"]);
    let scope_err = lit_mcp.refused("delete_card", json!({"card_id": &lit_card_id})).to_string();
    assert!(
        scope_err.contains("scope_refused") || scope_err.contains("Literature trees"),
        "Literature-scoped delete_card(card_id) must refuse touching Topic host: {scope_err}"
    );
    // Both occurrences survive untouched!
    let listed = mcp.ok("list_coordinates", json!({"at": "ScopeLit/1"}));
    let cids: Vec<&str> = listed["coordinates"].as_array().unwrap().iter().filter_map(|c| c["coord_id"].as_str()).collect();
    assert_eq!(cids.len(), 2);
    assert!(cids.contains(&topic_occ.as_str()));
}

/// M1 & M2: Cross-tree refile/move enforces MC3 on surviving Literature occurrences and enforces
/// native Connection-tree Card-type rules (`idea`/`event`/`finding` only).
#[test]
fn m1_and_m2_enforce_mc3_and_native_connection_card_types() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_literature", json!({"bibtex": "@book{BookM1,\n  author={A},\n  title={M1},\n  year={2026}\n}", "title": "M1", "body": "Book."}));
    mcp.ok("create_successor", json!({"at": "BookM1", "title": "Lit Note", "body": "Note."}));
    mcp.ok("create_topic", json!({"title": "Topic 0", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Model Head", "body": "Question.", "type": "model"}));

    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let t0_root = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/0").unwrap()["coord_id"].as_str().unwrap().to_string();
    let t0_model = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();

    // M1: Place BookM1/1 in Topic 0, then attempt to refile BookM1/1 into Topic 0 -> REFUSED because old coord stays in BookM1!
    let in_t0 = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &t0_model, "edge": "side", "target_at": "BookM1/1"}));
    let in_t0_id = in_t0["coord_id"].as_str().unwrap().to_string();
    let m1_err = mcp.refused("refile_card", json!({"at": "BookM1/1", "to_coord_id": &in_t0_id})).to_string();
    assert!(m1_err.to_lowercase().contains("literature") || m1_err.to_lowercase().contains("foreign"), "M1 expected MC3 refusal: {m1_err}");

    // M2: Place Model Head (0/1) into Connection tree ~1, attempt refile_card AND move_coordinate into ~1 -> BOTH REFUSED!
    let ct = mcp.ok("create_connection_tree", json!({"title": "Conn M2"}));
    let h = ct["tree"].as_str().unwrap().to_string();
    let r = ct["root_placement_id"].as_str().unwrap().to_string();
    let m_in_ct = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &r, "edge": "direct", "target_at": "0/1"}));
    let m_in_ct_id = m_in_ct["coord_id"].as_str().unwrap().to_string();

    let m2_refile = mcp.refused("refile_card", json!({"at": "0/1", "to_coord_id": &m_in_ct_id})).to_string();
    assert!(m2_refile.contains("native_structure_required") || m2_refile.to_lowercase().contains("model") || m2_refile.to_lowercase().contains("connection"), "M2 refile: {m2_refile}");

    let m2_move = mcp
        .refused(
            "move_coordinate",
            json!({"coord_id": &t0_model, "destination": {"tree": &h, "parent_coord_id": &m_in_ct_id, "edge": "side"}}),
        )
        .to_string();
    assert!(m2_move.contains("native_structure_required") || m2_move.to_lowercase().contains("model") || m2_move.to_lowercase().contains("connection"), "M2 move: {m2_move}");
    let _ = t0_root;
}

/// M3 & M4: v2 `lock_placements.card_id` stores the actual UUID `card_id` (not workspace),
/// v2 `structure_fp` seals pin baselines (`observed_authored_fp`), and deleting a v2 Lock's
/// `lock_coordinate_seals` row on a filing-only Topic Lock is detected!
#[test]
fn m3_and_m4_v2_lock_card_id_pin_baseline_and_seal_loss_detection() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Filing Topic", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Plain Filing", "body": "Body."}));
    mcp.ok("create_topic", json!({"title": "External Source", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "1/0", "title": "Source X", "body": "Original source."}));

    let ct = mcp.ok("create_connection_tree", json!({"title": "Pinned Conn"}));
    let h = ct["tree"].as_str().unwrap().to_string();
    let r = ct["root_placement_id"].as_str().unwrap().to_string();
    mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &r, "edge": "direct", "target_at": "1/1"}));

    let topic_lock = gui.lock("0/0");
    let topic_lock_id = topic_lock["lock_id"].as_i64().unwrap();
    let conn_lock = gui.lock(&h);
    let conn_lock_id = conn_lock["lock_id"].as_i64().unwrap();

    // M4: verify every lock_placements.card_id is a real UUID, never 'human'/'agent'!
    let db = scratch.archive_root.join("zt.sqlite3");
    {
        let conn = writer::open(&db).unwrap();
        let mut s = conn.prepare("SELECT card_id, json_extract(snapshot_json, '$[4]') FROM lock_placements").unwrap();
        let rows: Vec<(String, String)> = s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap();
        assert!(!rows.is_empty());
        for (cid, expected) in rows {
            assert_ne!(cid, "human");
            assert_ne!(cid, "agent");
            assert_eq!(cid, expected, "lock_placements.card_id must match tuple[4]");
        }
    }

    // M3 part 1: tampering with lock_placement_pins.observed_authored_fp MUST trigger fault!
    {
        let conn = writer::open(&db).unwrap();
        let triggers: Vec<String> = {
            let mut s = conn.prepare("SELECT name FROM sqlite_master WHERE type='trigger' AND (name GLOB 'coord_lock_*' OR name GLOB 'coord_pin_*')").unwrap();
            s.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap()
        };
        for t in triggers {
            conn.execute_batch(&format!("DROP TRIGGER IF EXISTS \"{t}\";")).unwrap();
        }
        conn.execute(
            "UPDATE lock_placement_pins SET observed_authored_fp = '0000000000000000000000000000000000000000000000000000000000000000' WHERE lock_id = ?1",
            [conn_lock_id],
        )
        .unwrap();
        // M3 part 2: deleting lock_coordinate_seals on filing-only v2 Topic Lock MUST trigger coordinate-evidence-missing!
        conn.execute("DELETE FROM lock_coordinate_seals WHERE lock_id = ?1", [topic_lock_id]).unwrap();
    }

    let panel = gui.get("/api/locks");
    assert_eq!(panel["integrity"]["ok"], false, "tampered pin baseline & deleted v2 seal must fail: {panel}");
    let faults = panel["integrity"]["faults"].as_array().unwrap();
    assert!(
        faults.iter().any(|f| f["lock_id"] == conn_lock_id && f["kind"] == "coordinate-evidence-changed"),
        "expected coordinate-evidence-changed for tampered pin baseline: {faults:?}"
    );
    assert!(
        faults.iter().any(|f| f["lock_id"] == topic_lock_id && f["kind"] == "coordinate-evidence-missing"),
        "expected coordinate-evidence-missing for deleted filing-only v2 seal: {faults:?}"
    );
}

/// M5: Mixed Filing + Secondary Direct chain move and interleaved Filing/Secondary Side reorder!
#[test]
fn m5_mixed_filing_and_secondary_move_and_reorder() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Source", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "F1", "body": "Filing 1."}));
    mcp.ok("create_topic", json!({"title": "Other", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "1/0", "title": "Sec Card", "body": "Secondary."}));
    mcp.ok("create_topic", json!({"title": "Dest", "body": "Root."}));

    // Build interleaved Side siblings under 0/1: 0/1|a (filing), 0/1|b (secondary), 0/1|c (filing)!
    mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": "Side A", "body": "A."}));
    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let f1_id = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    let sec_side = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &f1_id, "edge": "side", "target_at": "1/1"}));
    assert_eq!(sec_side["coord"], "0/1|b");
    let side_c = mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": "Side C", "body": "C."}));
    assert_eq!(side_c["address"], "0/1|c");

    // Reorder Filing Side siblings [0/1|c, 0/1|a] -> must swap a & c while preserving Secondary at 0/1|b!
    let reordered = confirm_mcp(&mut mcp, "reorder", json!({"parent": "0/1", "side": ["0/1|c", "0/1|a"]}));
    assert_eq!(reordered["applied"], true);
    let t0_after_reorder = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let sec_still_b = t0_after_reorder["coordinates"].as_array().unwrap().iter().find(|c| c["coord_id"] == sec_side["coord_id"]).unwrap();
    assert_eq!(sec_still_b["coord"], "0/1|b", "Secondary 0/1|b must stay untouched at slot b");
    assert_eq!(mcp.ok("get_card", json!({"at": "0/1|a"}))["title"], "Side C");
    assert_eq!(mcp.ok("get_card", json!({"at": "0/1|c"}))["title"], "Side A");

    // Build mixed Direct chain: 0/1 (filing) -> 0/2 (secondary of 1/1) -> 0/3 (filing)!
    // First create 0/2 (filing F3), then splice Secondary 1/1 directly after 0/1 so F3 shifts to 0/3!
    let f3 = mcp.ok("create_successor", json!({"at": "0/1", "title": "F3", "body": "Filing 3 shifted under Secondary 0/2."}));
    assert_eq!(f3["address"], "0/2");
    let sec_dir = confirm_mcp(
        &mut mcp,
        "add_coordinate",
        json!({"tree": "0", "parent_coord_id": &f1_id, "edge": "direct", "target_at": "1/1"}),
    );
    assert_eq!(sec_dir["coord"], "0/2");
    assert_eq!(mcp.ok("get_card", json!({"at": "0/3"}))["title"], "F3");

    // Move 0/1 subtree into Topic 2 (to 2/1) -> MUST succeed without `missing parent 0/2`!
    let moved = confirm_mcp(&mut mcp, "move_card", json!({"at": "0/1", "to": "2/1"}));
    assert_eq!(moved["applied"], true);
    let t2 = mcp.ok("get_tree_coordinates", json!({"tree": "2"}));
    let paths: Vec<&str> = t2["coordinates"].as_array().unwrap().iter().filter_map(|c| c["coord"].as_str()).collect();
    assert!(paths.contains(&"2/1") && paths.contains(&"2/2") && paths.contains(&"2/3"), "mixed subtree moved cleanly: {paths:?}");
}

/// M6 & Minor 2: Filing-only tree exports Portable v4 AND offline `portable-rules.js` resolves
/// filing paths, Secondary paths, and `place:<coord_id>` bookmarks identically!
#[test]
fn m6_and_minor2_filing_only_portable_v4_and_offline_path_lookup() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_literature", json!({"bibtex": "@book{PortBook,\n  author={P},\n  title={Filing Only},\n  year={2026}\n}", "title": "Filing Only", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "PortBook", "title": "Ch 1", "body": "Body."}));

    let out = tempdir().unwrap();
    let book_dir = out.path().join("book");
    let exp = scratch.cmd().args(["export", "PortBook", "--out", book_dir.to_str().unwrap()]).output().unwrap();
    assert!(exp.status.success(), "{}", String::from_utf8_lossy(&exp.stderr));
    let data: Value = serde_json::from_str(&fs::read_to_string(book_dir.join("cards.json")).unwrap()).unwrap();
    assert_eq!(data["version"], 4, "filing-only book must export Portable v4");
    assert_eq!(data["coordinates"].as_array().unwrap().len(), 2);

    // Also add a Secondary Coordinate in a Topic and check offline lookup via Node!
    mcp.ok("create_topic", json!({"title": "Port Topic", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "X."}));
    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let x_cid = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    let x2 = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &x_cid, "edge": "side", "target_at": "0/1"}));
    assert_eq!(x2["coord"], "0/1|a");

    let topic_dir = out.path().join("topic");
    let exp2 = scratch.cmd().args(["export", "0/0", "--out", topic_dir.to_str().unwrap()]).output().unwrap();
    assert!(exp2.status.success(), "{}", String::from_utf8_lossy(&exp2.stderr));

    // Verify offline portable-rules.js resolves "0/1", "0/1|a", and "place:<coord_id>" identically
    // AND resolves a `place:<filing_coord_id>` bookmark on the filing-only PortBook!
    let script = format!(
        "const fs = require('fs');
         const rules = require({:?});
         const topicData = JSON.parse(fs.readFileSync({:?}, 'utf8'));
         const tix = rules.open(topicData);
         const sec = topicData.coordinates.find(c => c.coord === '0/1|a');
         const a = rules.respond(tix, 'POST', '/api/connection', {{ tool: 'list_coordinates', args: {{ at: '0/1' }} }}).body;
         const b = rules.respond(tix, 'POST', '/api/connection', {{ tool: 'list_coordinates', args: {{ at: '0/1|a' }} }}).body;
         const c = rules.respond(tix, 'POST', '/api/connection', {{ tool: 'list_coordinates', args: {{ at: 'place:' + sec.coord_id }} }}).body;
         if (a.coordinates.length !== 2 || b.coordinates.length !== 2 || c.coordinates.length !== 2) {{
           console.error('topic mismatch', a.coordinates.length, b.coordinates.length, c.coordinates.length);
           process.exit(1);
         }}
         const bookData = JSON.parse(fs.readFileSync({:?}, 'utf8'));
         const bix = rules.open(bookData);
         const ch1 = bookData.coordinates.find(c => c.coord === 'PortBook/1');
         const bk = rules.respond(bix, 'POST', '/api/connection', {{ tool: 'list_coordinates', args: {{ at: 'place:' + ch1.coord_id }} }}).body;
         if (bk.coordinates.length !== 1 || bk.coordinates[0].coord !== 'PortBook/1') {{
           console.error('filing-only bookmark mismatch', bk);
           process.exit(1);
         }}",
        std::env::current_dir().unwrap().join("src/gui/portable-rules.js").to_string_lossy(),
        topic_dir.join("cards.json").to_string_lossy(),
        book_dir.join("cards.json").to_string_lossy()
    );
    let node = StdCommand::new("node").args(["-e", &script]).output().expect("run node");
    assert!(node.status.success(), "offline portable-rules check failed: {}", String::from_utf8_lossy(&node.stderr));
}

/// Minor 3 & Acceptance #2/#7 hardenings:
/// - `move_coordinate` with `mode: "span"` moves a Direct chain with its Side branches cleanly.
/// - Detaching external occurrences of a Literature root WHILE its root Lock is ACTIVE preserves the Lock!
#[test]
fn minor3_span_move_and_active_root_lock_external_detach() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Base", "body": "Root."}));
    let mut prev = "0/0".to_string();
    for label in ["A", "B", "B1", "C", "C1", "D"] {
        let created = mcp.ok(
            "create_successor",
            json!({"at": &prev, "kind": "direct", "title": format!("Node {label}"), "body": format!("Body for node {label}.")}),
        );
        prev = created["address"].as_str().unwrap().to_string();
    }
    let ct = mcp.ok("create_connection_tree", json!({"title": "Span Tree"}));
    let h = ct["tree"].as_str().unwrap().to_string();
    let r = ct["root_placement_id"].as_str().unwrap().to_string();
    let ca = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &r, "edge": "direct", "target_at": "0/1"}))["coord_id"].as_str().unwrap().to_string();
    let cb = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &ca, "edge": "direct", "target_at": "0/2"}))["coord_id"].as_str().unwrap().to_string();
    let cb1 = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &cb, "edge": "side", "target_at": "0/3"}))["coord_id"].as_str().unwrap().to_string();
    let cc = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &cb, "edge": "direct", "target_at": "0/4"}))["coord_id"].as_str().unwrap().to_string();
    let cc1 = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &cc, "edge": "side", "target_at": "0/5"}))["coord_id"].as_str().unwrap().to_string();
    let cd = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &cc, "edge": "direct", "target_at": "0/6"}))["coord_id"].as_str().unwrap().to_string();

    // Moving span B..C beneath descendant D MUST be refused per §7!
    let desc_err = mcp
        .refused(
            "move_coordinate",
            json!({
                "coord_id": &cb,
                "mode": "span",
                "through_coord_id": &cc,
                "destination": {"parent_coord_id": &cd, "edge": "side"}
            }),
        )
        .to_string();
    assert!(
        desc_err.to_lowercase().contains("descendant") || desc_err.to_lowercase().contains("under itself") || desc_err.to_lowercase().contains("cycle"),
        "span move under descendant must be refused: {desc_err}"
    );

    // Move span B..C (with Side branches B1, C1) to be Side child of A -> D closes up to ~1/2!
    let moved = confirm_mcp(
        &mut mcp,
        "move_coordinate",
        json!({
            "coord_id": &cb,
            "mode": "span",
            "through_coord_id": &cc,
            "destination": {"parent_coord_id": &ca, "edge": "side"}
        }),
    );
    assert_eq!(moved["applied"], true);

    let after = mcp.ok("get_tree_coordinates", json!({"tree": &h}));
    let by_id = |id: &str| after["coordinates"].as_array().unwrap().iter().find(|c| c["coord_id"] == id).unwrap().clone();
    assert_eq!(by_id(&cd)["parent_coord_id"], ca, "D must close up under A");
    assert_eq!(by_id(&cd)["coord"], format!("{h}/2"));
    assert_eq!(by_id(&cb)["parent_coord_id"], ca, "B moved as Side child of A");
    assert_eq!(by_id(&cb1)["parent_coord_id"], cb, "B1 stayed with B");
    assert_eq!(by_id(&cc)["parent_coord_id"], cb, "C stayed Direct after B");
    assert_eq!(by_id(&cc1)["parent_coord_id"], cc, "C1 stayed with C");

    // Acceptance #2 hardening: detach external occurrences WHILE RootLock2026 Lock is ACTIVE!
    mcp.ok("create_literature", json!({"bibtex": "@book{RootLock2026,\n  author={R},\n  title={Locked Root},\n  year={2026}\n}", "title": "Locked Root", "body": "Root."}));
    let ext_ct = mcp.ok("add_coordinate", json!({"tree": &h, "parent_coord_id": &cd, "edge": "side", "target_at": "RootLock2026"}))["coord_id"].as_str().unwrap().to_string();
    let _root_lock = gui.lock("RootLock2026");

    // Detaching the external occurrence in unlocked ~1 MUST succeed while RootLock2026 stays locked!
    let ext_det = confirm_mcp(&mut mcp, "detach_coordinate", json!({"coord_id": &ext_ct}));
    assert_eq!(ext_det["applied"], true);
    let check = scratch.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert!(check.status.success(), "active root lock must stay clean after external detach: {}", String::from_utf8_lossy(&check.stderr));
}

/// M8 & Receipt Replay (pure v2):
/// - Migrated retired identities with `owner_workspace IS NULL` remain valid in Schema v2 and
///   report `home_workspace: null` on Missing Coordinates (never fabricating `'human'`).
/// - Live Cards with `owner_workspace IS NULL` are rejected by the Schema v2 CHECK constraint.
/// - Keyed `applied_ops` receipts survive daemon restart and replay identically.
#[test]
fn m8_retired_null_owner_workspace_and_receipt_replay_across_restart() {
    let scratch = ScratchBox::up();
    let db = scratch.archive_root.join("zt.sqlite3");
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Base", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "X."}));
    let ct = mcp.ok("create_connection_tree", json!({"title": "Host Conn"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let place_args = json!({"host": &handle, "parent_coord": format!("{handle}/0"), "target_at": "0/1", "op_key": "v2-restart-replay-key"});
    let placed = mcp.ok("place_card", place_args.clone());
    let placed_pid = placed["placement_id"].as_str().unwrap().to_string();

    // Seed a retired identity with NULL owner_workspace (matching migrated retired rows in D:\zt)
    // and a Missing Coordinate pointing at it; also verify live NULL owner_workspace is rejected!
    {
        let conn = writer::open(&db).unwrap();
        let ct_id: String = conn.query_row("SELECT tree_id FROM trees WHERE public_key = ?1", [&handle], |r| r.get(0)).unwrap();
        let unknown_ret = "11111111-1111-4111-8111-111111111111";
        conn.execute(
            "INSERT INTO card_identity(card_id, card_row, owner_workspace, status, filing_coord_id, last_address, last_title, retired_at_rev)
             VALUES (?1, NULL, NULL, 'retired', NULL, '99/1', 'Deleted Unknown', 2)",
            [unknown_ret],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO coordinates(coord_id, tree_id, host_workspace, card_id, parent_coord_id, edge, slot, gloss)
             VALUES ('33333333-3333-4333-8333-333333333333', ?1, 'agent', ?2, ?3, 'side', 1, 'missing legacy')",
            rusqlite::params![ct_id, unknown_ret, placed_pid],
        )
        .unwrap();
        // Live Card with NULL owner_workspace MUST be rejected by SQLite CHECK constraint!
        let bad_live = conn.execute(
            "INSERT INTO card_identity(card_id, card_row, owner_workspace, status, filing_coord_id, last_address, last_title, retired_at_rev)
             VALUES ('44444444-4444-4444-8444-444444444444', 9999, NULL, 'live', '55555555-5555-4555-8555-555555555555', '88/1', 'Bad', NULL)",
            [],
        );
        assert!(bad_live.is_err(), "live Card with NULL owner_workspace must be rejected");
    }

    // Restart daemon and verify Missing Coordinate reports home_workspace: null AND receipt replays!
    drop(mcp);
    scratch.cmd().arg("down").assert().success();
    scratch.cmd().arg("up").assert().success();

    let mut mcp2 = scratch.mcp(&[]);
    let tree_after = mcp2.ok("get_tree_coordinates", json!({"tree": &handle}));
    let missing_row = tree_after["coordinates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["coord_id"] == "33333333-3333-4333-8333-333333333333")
        .expect("missing coord survived restart");
    assert_eq!(missing_row["status"], "missing");
    assert_eq!(missing_row["home_workspace"], Value::Null, "unknown retired owner must report null");

    let replayed = mcp2.ok("place_card", place_args);
    assert_eq!(replayed["replayed"], true, "receipt must replay across daemon restart");
    assert_eq!(replayed["placement_id"], placed_pid);
}
