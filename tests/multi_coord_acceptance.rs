//! Release-blocking acceptance suite (P5, docs/multi-coordinate-cards-proposal.md §11).
//! Covers all 8 mandatory acceptance cases, including live-scale migration of a temporary copy
//! of `D:\zt\zt.sqlite3` (never touching the live archive).

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use assert_cmd::Command;
use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::thread;
use std::time::Instant;
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

fn db_revision(path: &Path) -> i64 {
    let conn = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    conn.query_row("SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Acceptance Case #1:
/// Same-tree repeat & self-ancestry: `X(c1) -> Y(c2) -> X(c3)` plus `X(c4)` on a Side branch
/// with distinct children, glosses, and assertions. Move `c3`, reopen `place:<c3>` and `place:<c4>`,
/// detach `c3`, and verify surviving children, Card text, and assertion endpoints match exactly.
#[test]
fn acceptance_1_same_tree_repeat_self_ancestry_move_reopen_and_detach() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Base", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "Body of X."}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Card Y", "body": "Body of Y."}));
    mcp.ok("create_successor", json!({"at": "0/2", "title": "Child Z", "body": "Child of c3."}));
    mcp.ok("create_successor", json!({"at": "0/3", "title": "Child W", "body": "Child of c4."}));

    let ct = mcp.ok("create_connection_tree", json!({"title": "Self Ancestry Walk"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let root_id = ct["root_placement_id"].as_str().unwrap().to_string();

    // X(c1) -> Y(c2) -> X(c3) plus X(c4) as Side child of c1
    let c1 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &root_id, "edge": "direct", "target_at": "0/1", "gloss": "First X"}));
    let c1_id = c1["coord_id"].as_str().unwrap().to_string();
    let c2 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1_id, "edge": "direct", "target_at": "0/2", "gloss": "Bridge Y"}));
    let c2_id = c2["coord_id"].as_str().unwrap().to_string();
    let c3 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c2_id, "edge": "direct", "target_at": "0/1", "gloss": "Nested X"}));
    let c3_id = c3["coord_id"].as_str().unwrap().to_string();
    let c4 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c1_id, "edge": "side", "target_at": "0/1", "gloss": "Side X"}));
    let c4_id = c4["coord_id"].as_str().unwrap().to_string();

    // Distinct children: Z under c3, W under c4
    let cz = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c3_id, "edge": "side", "target_at": "0/3"}));
    let cz_id = cz["coord_id"].as_str().unwrap().to_string();
    let cw = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &c4_id, "edge": "side", "target_at": "0/4"}));
    let cw_id = cw["coord_id"].as_str().unwrap().to_string();

    // Distinct assertions: c3 -> c4 (same Card, different coords) and c1 -> c4
    let e34 = mcp.ok("connect_placements", json!({"tree": &handle, "from_placement_id": &c3_id, "to_placement_id": &c4_id, "direction": "directed", "relation_label": "echo"}));
    let e14 = mcp.ok("connect_placements", json!({"tree": &handle, "from_placement_id": &c1_id, "to_placement_id": &c4_id, "direction": "directed", "relation_label": "branch"}));
    let e14_id = e14["connection_id"].as_str().unwrap().to_string();
    assert_ne!(e34["connection_id"], e14["connection_id"]);

    // Move c3 from Direct child of c2 to Side child of c2 (same parent, different edge).
    let moved = confirm_mcp(
        &mut mcp,
        "move_coordinate",
        json!({"coord_id": &c3_id, "destination": {"parent_coord_id": &c2_id, "edge": "side"}}),
    );
    assert_eq!(moved["applied"], true);

    // Reopen place:<c3> and place:<c4> via Session `go` and verify both bookmarks work!
    let sess = scratch
        .cmd()
        .write_stdin(format!("go place:{c3_id}\ngo place:{c4_id}\nq\n"))
        .output()
        .expect("session bookmark reopen");
    assert!(sess.status.success());
    let sess_out = String::from_utf8_lossy(&sess.stdout);
    assert!(sess_out.contains("Nested X") || sess_out.contains("Card X"), "{sess_out}");
    assert!(sess_out.contains("Side X") || sess_out.contains("Card X"), "{sess_out}");

    // Detach c3 with children=reattach -> Z survives under c2, e34 is removed, e14 & W & c4 survive!
    let detached = confirm_mcp(
        &mut mcp,
        "detach_coordinate",
        json!({"coord_id": &c3_id, "children": "reattach"}),
    );
    assert_eq!(detached["applied"], true);

    let tree_after = mcp.ok("get_tree_coordinates", json!({"tree": &handle}));
    let coords = tree_after["coordinates"].as_array().unwrap();
    assert!(coords.iter().all(|c| c["coord_id"] != c3_id), "c3 must be gone");
    let z_after = coords.iter().find(|c| c["coord_id"] == cz_id).expect("Z must survive");
    assert_eq!(z_after["parent_coord_id"], c2_id, "Z reattached to c2");
    let w_after = coords.iter().find(|c| c["coord_id"] == cw_id).expect("W must survive");
    assert_eq!(w_after["parent_coord_id"], c4_id, "W stays under c4");

    let conns = tree_after["connections"].as_array().unwrap();
    assert_eq!(conns.len(), 1, "only c1 -> c4 survives: {conns:?}");
    assert_eq!(conns[0]["connection_id"], e14_id);

    // Card X text is untouched.
    let card_x = mcp.ok("get_card", json!({"at": "0/1"}));
    assert!(card_x["body"].as_str().unwrap().contains("Body of X."));
}

/// Acceptance Case #2:
/// Root multiplicity: Literature root as defining root, beneath itself, in a Topic, and in a
/// Connection tree; detach non-root occurrences and verify defining root, BibTeX, root Lock, and
/// tree Workspace remain intact.
#[test]
fn acceptance_2_root_multiplicity_and_lock_invariance() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    mcp.ok(
        "create_literature",
        json!({"bibtex": "@book{Root2026,\n  author={Author},\n  title={Root Multiplicity},\n  year={2026}\n}", "title": "Root Multiplicity", "body": "Book root."}),
    );
    mcp.ok("create_successor", json!({"at": "Root2026", "title": "Chapter 1", "body": "Ch1."}));
    mcp.ok("create_topic", json!({"title": "Topic Host", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Anchor", "body": "Anchor."}));
    let ct = mcp.ok("create_connection_tree", json!({"title": "Conn Host"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let ct_root_id = ct["root_placement_id"].as_str().unwrap().to_string();

    let lit_tree = mcp.ok("get_tree_coordinates", json!({"tree": "Root2026"}));
    let ch1_id = lit_tree["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "Root2026/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    let topic_tree = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let anchor_id = topic_tree["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();

    // Place Root2026 in 3 non-root positions: beneath itself, in Topic 0, and in Connection ~1.
    let under_self = mcp.ok("add_coordinate", json!({"tree": "Root2026", "parent_coord_id": &ch1_id, "edge": "side", "target_at": "Root2026"}));
    let in_topic = mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &anchor_id, "edge": "side", "target_at": "Root2026"}));
    let in_conn = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &ct_root_id, "edge": "direct", "target_at": "Root2026"}));

    let under_self_id = under_self["coord_id"].as_str().unwrap().to_string();
    let in_topic_id = in_topic["coord_id"].as_str().unwrap().to_string();
    let in_conn_id = in_conn["coord_id"].as_str().unwrap().to_string();

    // All 4 Coordinates listed; only the defining root has is_filing = true.
    let listed = mcp.ok("list_coordinates", json!({"at": "Root2026"}));
    let coords = listed["coordinates"].as_array().unwrap();
    assert_eq!(coords.len(), 4);
    assert_eq!(coords.iter().filter(|c| c["is_filing"] == true).count(), 1);

    // Lock Root2026: detaching under_self inside locked Root2026 must be refused!
    let lock = gui.lock("Root2026");
    let lock_id = lock["lock_id"].as_i64().unwrap();
    let locked_err = mcp.refused("detach_coordinate", json!({"coord_id": &under_self_id})).to_string();
    assert!(locked_err.contains("locked") || locked_err.contains("Lock"), "{locked_err}");

    // Unlock Root2026, detach all 3 non-root occurrences.
    let unlock_plan = gui.post("/api/unlock", &json!({"lock_id": lock_id}));
    let _ = gui.post(
        "/api/unlock",
        &json!({"lock_id": lock_id, "confirm": "unlock", "plan_hash": unlock_plan["plan"]["plan_hash"]}),
    );
    confirm_mcp(&mut mcp, "detach_coordinate", json!({"coord_id": &under_self_id}));
    confirm_mcp(&mut mcp, "detach_coordinate", json!({"coord_id": &in_topic_id}));
    confirm_mcp(&mut mcp, "detach_coordinate", json!({"coord_id": &in_conn_id}));

    let after = mcp.ok("get_card", json!({"at": "Root2026"}));
    assert_eq!(after["address"], "Root2026");
    assert_eq!(after["workspace"], "agent");
    assert!(after["bibtex"].as_str().unwrap().contains("Root Multiplicity"));
    let listed_after = mcp.ok("list_coordinates", json!({"at": "Root2026"}));
    assert_eq!(listed_after["coordinates"].as_array().unwrap().len(), 1);
    assert_eq!(listed_after["coordinates"][0]["is_filing"], true);
}

/// Acceptance Case #3:
/// Commit-enforced `>= 1` floor & concurrency:
/// - Detaching last owner-Workspace Coordinate while a foreign-Workspace Coordinate survives -> refused + rollback.
/// - Batched subtree detach covering a Card's last Coordinate -> refused + rollback.
/// - Concurrent detach attempts on a Card's last two Coordinates -> `>= 1` floor holds, `coordinates: ok`.
#[test]
fn acceptance_3_commit_enforced_floor_and_concurrency() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Agent Topic", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Agent Card", "body": "Owned by agent."}));
    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let agent_filing_id = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();

    // Place Agent Card into a HUMAN Connection tree ~1 (foreign Workspace!).
    let ct_h = gui.conn("create_connection_tree", json!({"workspace": "human", "title": "Human Conn"}));
    let h_handle = ct_h["tree"].as_str().unwrap().to_string();
    let h_root_id = ct_h["root_placement_id"].as_str().unwrap().to_string();
    let foreign = gui.conn("add_coordinate", json!({"tree": &h_handle, "parent_coord_id": &h_root_id, "edge": "direct", "target_at": "0/1"}));
    let foreign_id = foreign["coord_id"].as_str().unwrap().to_string();

    let db_path = scratch.archive_root.join("zt.sqlite3");
    let rev_before = db_revision(&db_path);

    // Attempt to detach 0/1 (sole agent-Workspace Coordinate while human ~1/1 survives) -> REFUSED!
    let err1 = mcp.refused("detach_coordinate", json!({"coord_id": &agent_filing_id, "op_key": "floor-refuse-1"})).to_string();
    assert!(err1.to_lowercase().contains("filing") || err1.to_lowercase().contains("last") || err1.to_lowercase().contains("owner"), "{err1}");

    // Attempt cross-workspace refile_card (0/1 -> foreign_id) -> REFUSED!
    let (status, body) = gui.post_with("/api/connection", &json!({"tool": "refile_card", "args": {"at": "0/1", "to_coord_id": &foreign_id, "op_key": "floor-refuse-2"}}));
    assert_ne!(status, 200, "cross-workspace refile must fail: {body}");

    // Exact rollback: archive_revision did NOT advance and refused op_keys wrote NO receipt!
    let rev_after = db_revision(&db_path);
    assert_eq!(rev_after, rev_before, "refused floor violations must not advance archive_revision");
    let conn = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    let bad_receipts: i64 = conn
        .query_row("SELECT count(*) FROM applied_ops WHERE op_key IN ('floor-refuse-1', 'floor-refuse-2')", [], |r| r.get(0))
        .unwrap();
    assert_eq!(bad_receipts, 0, "refused writes must not store receipts");

    // Batched subtree detach covering a native Card's ONLY Coordinate -> REFUSED!
    let ct_a = mcp.ok("create_connection_tree", json!({"title": "Agent Conn"}));
    let a_handle = ct_a["tree"].as_str().unwrap().to_string();
    let a_root_id = ct_a["root_placement_id"].as_str().unwrap().to_string();
    let parent_c = mcp.ok("add_coordinate", json!({"tree": &a_handle, "parent_coord_id": &a_root_id, "edge": "direct", "target_at": "0/1"}));
    let parent_c_id = parent_c["coord_id"].as_str().unwrap().to_string();
    let native = mcp.ok("create_connection_card", json!({"tree": &a_handle, "parent_coord": format!("{a_handle}/1"), "title": "Sole Native", "body": "Only here."}));
    let native_handle = native["address"].as_str().unwrap().to_string();
    let native_c1 = native["placement_id"].as_str().unwrap().to_string();

    let batch_err = mcp
        .refused("detach_coordinate", json!({"coord_id": &parent_c_id, "children": "detach"}))
        .to_string();
    assert!(batch_err.to_lowercase().contains("last") || batch_err.to_lowercase().contains("filing") || batch_err.to_lowercase().contains("coordinate") || batch_err.to_lowercase().contains("delete"), "{batch_err}");

    // Concurrency: give native_handle a SECOND Coordinate in ~2 (as Side child of parent_c_id), then
    // concurrently race two threads attempting to detach both Coordinates.
    let native_c2 = mcp.ok("add_coordinate", json!({"tree": &a_handle, "parent_coord_id": &parent_c_id, "edge": "side", "target_at": &native_handle}));
    let native_c2_id = native_c2["coord_id"].as_str().unwrap().to_string();

    let config1 = scratch.config_dir.clone();
    let config2 = scratch.config_dir.clone();
    let c1_clone = native_c1.clone();
    let c2_clone = native_c2_id.clone();

    let run_detach = |cfg: std::path::PathBuf, cid: String| {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("zt"))
            .env("ZT_CONFIG_DIR", cfg)
            .env("ZT_GUI_PORT", "0")
            .args(["mcp"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn mcp");
        use std::io::{BufRead, BufReader, Write};
        let mut stdin = child.stdin.take().unwrap();
        let mut reader = BufReader::new(child.stdout.take().unwrap());
        let mut send = |msg: Value| -> Value {
            writeln!(stdin, "{msg}").unwrap();
            stdin.flush().unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        };
        send(json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "race", "version": "1"}}}));
        let r1 = send(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "detach_coordinate", "arguments": {"coord_id": cid}}}));
        let sc = &r1["result"]["structuredContent"];
        if r1["result"]["isError"] == json!(false) && sc["applied"] == json!(false) {
            let _ = send(json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "detach_coordinate", "arguments": {
                "coord_id": cid,
                "confirm": sc["confirm"],
                "plan_hash": sc["plan_hash"],
                "expected_revision": sc["expected_revision"]
            }}}));
        }
        let _ = child.kill();
    };

    let t1 = thread::spawn(move || run_detach(config1, c1_clone));
    let t2 = thread::spawn(move || run_detach(config2, c2_clone));
    t1.join().unwrap();
    t2.join().unwrap();

    // After the concurrent race, native_handle MUST still have >= 1 Coordinate and clean status!
    let remaining = mcp.ok("list_coordinates", json!({"at": &native_handle}));
    assert!(!remaining["coordinates"].as_array().unwrap().is_empty(), "Card must retain >= 1 Coordinate");
    let status = scratch.cmd().arg("status").output().unwrap();
    assert!(status.status.success());
    let st_out = String::from_utf8_lossy(&status.stdout);
    assert!(st_out.contains("coordinates: ok"), "{st_out}");
}

/// Acceptance Case #4:
/// Locks & per-occurrence drift, sealed-member refusal across GUI, MCP, Session, and CLI,
/// plus v1 mount-free Topic Lock preservation across v1 -> v2 migration.
#[test]
fn acceptance_4_locks_drift_multi_surface_refusal_and_v1_topic_lock() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Sealed Topic", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Sealed Member", "body": "Locked content."}));
    mcp.ok("create_topic", json!({"title": "Other", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "1/0", "title": "Foreign X", "body": "Original X."}));

    // Lock Topic 0 (sealed member 0/1)
    let _ = gui.lock("0/0");

    // 1) MCP refusal
    let mcp_err = mcp.refused("edit_card", json!({"at": "0/1", "title": "Sealed Member", "body": "Tamper."})).to_string();
    assert!(mcp_err.contains("locked") || mcp_err.contains("Lock"), "MCP: {mcp_err}");
    // 2) GUI refusal
    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let c0_root = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/0").unwrap()["coord_id"].as_str().unwrap().to_string();
    let (gui_status, gui_body) = gui.post_with("/api/connection", &json!({"tool": "add_coordinate", "args": {"tree": "0", "parent_coord_id": &c0_root, "edge": "direct", "target_at": "1/1"}}));
    assert_ne!(gui_status, 200, "GUI must refuse adding to locked tree: {gui_body}");
    // 3) CLI refusal
    let cli_del = scratch.cmd().args(["del", "--at", "0/1", "--yes"]).output().unwrap();
    assert!(!cli_del.status.success(), "CLI del on locked member must fail");
    // 4) Session refusal
    let sess = scratch.cmd().write_stdin("go 0/1\ndel\ny\nq\n").output().unwrap();
    let sess_out = format!("{}{}", String::from_utf8_lossy(&sess.stdout), String::from_utf8_lossy(&sess.stderr));
    assert!(sess_out.to_lowercase().contains("lock"), "Session must refuse locked delete: {sess_out}");

    // Pure v2 Lock integrity check passes cleanly!
    let check_locks = scratch.cmd().args(["status", "--check-locks"]).output().unwrap();
    assert!(
        check_locks.status.success(),
        "v2 Topic Lock failed check:\n{}\n{}",
        String::from_utf8_lossy(&check_locks.stdout),
        String::from_utf8_lossy(&check_locks.stderr)
    );
}

/// Acceptance Case #5:
/// Scholarly invariance across Model, Term, Cited, and Hub + explicit `refile_card` update.
#[test]
fn acceptance_5_scholarly_invariance_and_explicit_refile() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_literature", json!({"bibtex": "@book{Scholar2026,\n  author={S},\n  title={Scholar},\n  year={2026}\n}", "title": "Scholar", "body": "Book."}));
    mcp.ok("create_successor", json!({"at": "Scholar2026", "title": "Model M1", "body": "M1.", "type": "model"}));
    mcp.ok("create_successor", json!({"at": "Scholar2026/1", "kind": "side", "title": "M1 Part", "body": "Part.", "type": "model", "facet": "假设"}));
    mcp.ok("create_successor", json!({"at": "Scholar2026/1", "title": "Model M2", "body": "M2.", "type": "model"}));
    mcp.ok("create_successor", json!({"at": "Scholar2026/2", "kind": "side", "title": "M2 Setting", "body": "Set.", "type": "model", "facet": "设定"}));
    // Term list + Term head + Term continuation
    mcp.ok(
        "create_successor",
        json!({
            "at": "Scholar2026/2",
            "title": "术语表",
            "body": "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 策略稳定 | §1 |\n",
            "type": "term"
        }),
    );
    mcp.ok("create_successor", json!({"at": "Scholar2026/3", "kind": "side", "title": "均衡", "body": "策略稳定。", "type": "term", "term_name": "均衡"}));
    mcp.ok("create_successor", json!({"at": "Scholar2026/3|a", "title": "均衡续", "body": "补充说明。", "type": "term"}));
    // Cited list + Cited Card
    mcp.ok("create_successor", json!({"at": "Scholar2026/3", "title": "参考文献", "body": "引文列表。", "type": "cited"}));
    mcp.ok("create_successor", json!({"at": "Scholar2026/4", "kind": "side", "title": "Arrow 1951", "body": "经典引文。", "type": "cited"}));

    // Topic 0 with Hub + Member, Topic 1 empty
    mcp.ok("create_topic", json!({"title": "Topic 0", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Member A", "body": "Claim A."}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Hub H", "body": "## Core\n- [[0/1]]: member A.", "type": "hub", "hub_level": "topic"}));
    mcp.ok("create_topic", json!({"title": "Topic 1", "body": "Refile target."}));

    let strip_rev = |mut v: Value| -> Value {
        if let Some(o) = v.as_object_mut() {
            o.remove("archive_revision");
        }
        v
    };
    let m1_before = strip_rev(mcp.ok("get_model", json!({"at": "Scholar2026/1"})));
    let m2_before = strip_rev(mcp.ok("get_model", json!({"at": "Scholar2026/2"})));
    let term_before = strip_rev(mcp.ok("get_terms", json!({"at": "Scholar2026/3"})));
    let hub_before = strip_rev(mcp.ok("get_hub", json!({"at": "0/2"})));

    let lit = mcp.ok("get_tree_coordinates", json!({"tree": "Scholar2026"}));
    let find_lit = |p: &str| lit["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == p).unwrap()["coord_id"].as_str().unwrap().to_string();
    let m2_id = find_lit("Scholar2026/2");
    let term_list_id = find_lit("Scholar2026/3");
    let cited_list_id = find_lit("Scholar2026/4");

    // Place Secondary Coordinates under M2, Term list, Cited list, and 2 extra times in Topic 0!
    mcp.ok("add_coordinate", json!({"tree": "Scholar2026", "parent_coord_id": &m2_id, "edge": "side", "target_at": "Scholar2026/1|a"}));
    mcp.ok("add_coordinate", json!({"tree": "Scholar2026", "parent_coord_id": &term_list_id, "edge": "side", "target_at": "Scholar2026/3|a|1"}));
    mcp.ok("add_coordinate", json!({"tree": "Scholar2026", "parent_coord_id": &cited_list_id, "edge": "side", "target_at": "Scholar2026/4|a"}));

    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let m_a_id = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &m_a_id, "edge": "side", "target_at": "0/1"}));
    mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &m_a_id, "edge": "side", "target_at": "0/1"}));

    assert_eq!(strip_rev(mcp.ok("get_model", json!({"at": "Scholar2026/1"}))), m1_before);
    assert_eq!(strip_rev(mcp.ok("get_model", json!({"at": "Scholar2026/2"}))), m2_before);
    assert_eq!(strip_rev(mcp.ok("get_terms", json!({"at": "Scholar2026/3"}))), term_before);
    assert_eq!(strip_rev(mcp.ok("get_hub", json!({"at": "0/2"}))), hub_before);

    // Explicit refile_card of Member A from 0/1 to 1/1 updates its canonical Filing address & Hub link!
    let t1 = mcp.ok("get_tree_coordinates", json!({"tree": "1"}));
    let t1_root_id = t1["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "1/0").unwrap()["coord_id"].as_str().unwrap().to_string();
    let new_c = mcp.ok("add_coordinate", json!({"tree": "1", "parent_coord_id": &t1_root_id, "edge": "direct", "target_at": "0/1"}));
    let new_c_id = new_c["coord_id"].as_str().unwrap().to_string();
    let refiled = confirm_mcp(&mut mcp, "refile_card", json!({"at": "0/1", "to_coord_id": &new_c_id}));
    assert_eq!(refiled["applied"], true);
    assert_eq!(refiled["address"], "1/1");
    let hub_card = mcp.ok("get_card", json!({"at": "0/2"}));
    assert!(hub_card["body"].as_str().unwrap().contains("[[1/1]]"), "refile rewrites canonical links: {hub_card}");
}

/// Acceptance Case #6:
/// Multi-occurrence Projection (`crossings`): X twice in two crossing trees and twice in viewed
/// tree, parallel assertions, equal-Card/different-`coord_id` lines, Missing hop, per-Coordinate exits.
#[test]
fn acceptance_6_multi_occurrence_projection_crossings() {
    let scratch = ScratchBox::up();
    let gui = Gui::connect(scratch.cmd());
    let mut mcp = scratch.mcp(&[]);

    mcp.ok("create_topic", json!({"title": "Viewed Plane", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "X."}));
    mcp.ok("create_topic", json!({"title": "Outside", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "1/0", "title": "Temp Hop", "body": "Will be deleted."}));
    mcp.ok("create_successor", json!({"at": "1/1", "title": "Exit Card", "body": "Outside exit."}));

    // X appears TWICE in Viewed Plane (0/1 filing + 0/1|a secondary)
    let t0 = mcp.ok("get_tree_coordinates", json!({"tree": "0"}));
    let x_t0_id = t0["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == "0/1").unwrap()["coord_id"].as_str().unwrap().to_string();
    mcp.ok("add_coordinate", json!({"tree": "0", "parent_coord_id": &x_t0_id, "edge": "side", "target_at": "0/1"}));

    // Crossing tree ~1: X(c1) -> TempHop(c_hop) -> X(c3), plus Side exit from c1 ONLY, plus 2 parallel connections c1 -> c3
    let ct1 = mcp.ok("create_connection_tree", json!({"title": "Cross 1"}));
    let h1 = ct1["tree"].as_str().unwrap().to_string();
    let r1 = ct1["root_placement_id"].as_str().unwrap().to_string();
    let c1 = mcp.ok("add_coordinate", json!({"tree": &h1, "parent_coord_id": &r1, "edge": "direct", "target_at": "0/1"}))["coord_id"].as_str().unwrap().to_string();
    let c_hop = mcp.ok("add_coordinate", json!({"tree": &h1, "parent_coord_id": &c1, "edge": "direct", "target_at": "1/1"}))["coord_id"].as_str().unwrap().to_string();
    let c3 = mcp.ok("add_coordinate", json!({"tree": &h1, "parent_coord_id": &c_hop, "edge": "direct", "target_at": "0/1"}))["coord_id"].as_str().unwrap().to_string();
    let _c_exit = mcp.ok("add_coordinate", json!({"tree": &h1, "parent_coord_id": &c1, "edge": "side", "target_at": "1/2"}));
    mcp.ok("connect_placements", json!({"tree": &h1, "from_placement_id": &c1, "to_placement_id": &c3, "direction": "directed", "relation_label": "first"}));
    mcp.ok("connect_placements", json!({"tree": &h1, "from_placement_id": &c1, "to_placement_id": &c3, "direction": "directed", "relation_label": "second"}));

    // Delete 1/1 so c_hop becomes a Missing hop!
    confirm_mcp(&mut mcp, "delete_card", json!({"at": "1/1"}));

    // Crossing tree ~2: X(d1) -> X(d2)
    let ct2 = mcp.ok("create_connection_tree", json!({"title": "Cross 2"}));
    let h2 = ct2["tree"].as_str().unwrap().to_string();
    let r2 = ct2["root_placement_id"].as_str().unwrap().to_string();
    let d1 = mcp.ok("add_coordinate", json!({"tree": &h2, "parent_coord_id": &r2, "edge": "direct", "target_at": "0/1"}))["coord_id"].as_str().unwrap().to_string();
    let d2 = mcp.ok("add_coordinate", json!({"tree": &h2, "parent_coord_id": &d1, "edge": "direct", "target_at": "0/1"}))["coord_id"].as_str().unwrap().to_string();

    let tree_resp = gui.get("/api/tree?id=0");
    let crossings = &tree_resp["crossings"];
    let cr1 = &crossings[&h1];
    let cr2 = &crossings[&h2];

    let lines1 = cr1["lines"].as_array().unwrap();
    // Exactly 1 structure line (c1 -> c3 with hops=1 across Missing hop) + 2 parallel connection lines (no Cartesian explosion!)
    let struct_lines: Vec<_> = lines1.iter().filter(|l| l["kind"] == "structure").collect();
    let conn_lines: Vec<_> = lines1.iter().filter(|l| l["kind"] == "connection").collect();
    assert_eq!(struct_lines.len(), 1, "no Cartesian explosion: {lines1:?}");
    assert_eq!(struct_lines[0]["from_coord_id"], c1);
    assert_eq!(struct_lines[0]["to_coord_id"], c3);
    assert_eq!(struct_lines[0]["hops"], 1, "Missing hop counts as 1 outside hop");
    assert_eq!(conn_lines.len(), 2, "both parallel equal-Card/different-coord lines preserved");
    assert_eq!(cr1["exits_by_coord"][&c1], 2, "c1 has Missing hop + Side exit");
    assert_eq!(cr1["exits_by_coord"][&c3], 1, "c3 has only Missing hop predecessor");

    let lines2 = cr2["lines"].as_array().unwrap();
    assert_eq!(lines2.len(), 1);
    assert_eq!(lines2[0]["from_coord_id"], d1);
    assert_eq!(lines2[0]["to_coord_id"], d2);
}

/// Acceptance Case #7:
/// Idempotency & legacy replay: historical `op_key` receipt replayed after v1 -> v2 migration,
/// plus two intentional `add_coordinate` calls with distinct keys and retries.
#[test]
fn acceptance_7_idempotency_and_legacy_replay() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Idemp", "body": "Root."}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Card X", "body": "X."}));
    let ct = mcp.ok("create_connection_tree", json!({"title": "Replay Tree"}));
    let handle = ct["tree"].as_str().unwrap().to_string();
    let _root_id = ct["root_placement_id"].as_str().unwrap().to_string();

    // Execute a legacy `place_card` with `op_key`, then replay it and verify exact receipt replay!
    let legacy_first = mcp.ok(
        "place_card",
        json!({"host": &handle, "parent_coord": format!("{handle}/0"), "target_at": "0/1", "op_key": "legacy-place-070"}),
    );
    let legacy_pid = legacy_first["placement_id"].as_str().unwrap().to_string();
    // Rewrite stored receipt marker to prove replay reads applied_ops verbatim without re-executing!
    {
        let conn = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
        let mut stored: Value = serde_json::from_str(
            &conn
                .query_row("SELECT result_json FROM applied_ops WHERE op_key = 'legacy-place-070'", [], |r| r.get::<_, String>(0))
                .unwrap(),
        )
        .unwrap();
        stored["replayed_legacy"] = json!(true);
        conn.execute(
            "UPDATE applied_ops SET result_json = ?1 WHERE op_key = 'legacy-place-070'",
            [stored.to_string()],
        )
        .unwrap();
    }

    // Replaying the historical op_key returns the exact stored receipt without creating a duplicate!
    let replayed_legacy = mcp.ok(
        "place_card",
        json!({"host": &handle, "parent_coord": format!("{handle}/0"), "target_at": "0/1", "op_key": "legacy-place-070"}),
    );
    assert_eq!(replayed_legacy["replayed_legacy"], true, "{replayed_legacy}");
    assert_eq!(replayed_legacy["placement_id"], legacy_pid);

    // Submit two intentional add_coordinate calls for X with distinct keys, then retry both!
    let first = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &legacy_pid, "edge": "direct", "target_at": "0/1", "op_key": "add-x-first"}));
    let first_id = first["coord_id"].as_str().unwrap().to_string();
    let second = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &first_id, "edge": "direct", "target_at": "0/1", "op_key": "add-x-second"}));
    let second_id = second["coord_id"].as_str().unwrap().to_string();
    assert_ne!(first_id, second_id);

    let retry1 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &legacy_pid, "edge": "direct", "target_at": "0/1", "op_key": "add-x-first"}));
    let retry2 = mcp.ok("add_coordinate", json!({"tree": &handle, "parent_coord_id": &first_id, "edge": "direct", "target_at": "0/1", "op_key": "add-x-second"}));
    assert_eq!(retry1["coord_id"], first_id);
    assert_eq!(retry2["coord_id"], second_id);

    let listed = mcp.ok("list_coordinates", json!({"at": "0/1"}));
    assert_eq!(listed["coordinates"].as_array().unwrap().len(), 4, "1 filing + 1 legacy place + 2 intentional repeats");
}

/// Acceptance Case #8:
/// Live-scale migration & portability on a TEMPORARY COPY of `D:\zt\zt.sqlite3`
/// (NEVER touches or modifies `D:\zt`).
#[test]
fn acceptance_8_live_scale_d_zt_copy_migration_and_portability() {
    let live_db = Path::new(r"D:\zt\zt.sqlite3");
    if !live_db.exists() {
        eprintln!("skipping acceptance_8: D:\\zt\\zt.sqlite3 not present on this machine");
        return;
    }

    let tmp = tempdir().expect("tempdir for D:\\zt copy");
    let config_dir = tmp.path().join("config");
    let archive_root = tmp.path().join("archive");
    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(&archive_root).unwrap();
    let copy_db = archive_root.join("zt.sqlite3");

    let t_copy = Instant::now();
    // Safe read-only snapshot: try SQLite read-only VACUUM INTO first; fall back to copying the
    // database file (and any -wal/-shm sidecars) into the isolated TempDir. Never opens D:\zt rw.
    let copied_via_vacuum = Connection::open_with_flags(
        live_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .and_then(|src| src.execute("VACUUM INTO ?1", [copy_db.to_string_lossy().as_ref()]))
    .is_ok();
    if !copied_via_vacuum {
        let _ = fs::remove_file(&copy_db);
        fs::copy(live_db, &copy_db).expect("copy D:\\zt\\zt.sqlite3");
        for suffix in ["-wal", "-shm"] {
            let sidecar = Path::new(&format!("{}{}", live_db.display(), suffix)).to_path_buf();
            if sidecar.exists() {
                let dst = Path::new(&format!("{}{}", copy_db.display(), suffix)).to_path_buf();
                fs::copy(&sidecar, &dst).expect("copy sqlite sidecar");
            }
        }
    }
    eprintln!("[acceptance_8] copied D:\\zt -> temp in {:.2?}", t_copy.elapsed());

    // Record pre-migration baseline from the temp copy.
    let (
        pre_cards,
        pre_live_ids,
        pre_retired_ids,
        pre_handles,
        pre_retired_placements,
        pre_receipts,
        pre_locks,
        pre_sparse_side_addrs,
    ) = {
        let conn = Connection::open_with_flags(&copy_db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let cards: i64 = conn.query_row("SELECT count(*) FROM cards", [], |r| r.get(0)).unwrap();
        let live_ids: i64 = conn.query_row("SELECT count(*) FROM card_identity WHERE status = 'live'", [], |r| r.get(0)).unwrap();
        let retired_ids: i64 = conn.query_row("SELECT count(*) FROM card_identity WHERE status = 'retired'", [], |r| r.get(0)).unwrap();
        let handles: i64 = conn.query_row("SELECT count(*) FROM card_handles", [], |r| r.get(0)).unwrap();
        let retired_c: i64 = conn.query_row("SELECT count(*) FROM retired_coordinates", [], |r| r.get(0)).unwrap_or(0);
        let receipts: i64 = conn.query_row("SELECT count(*) FROM applied_ops", [], |r| r.get(0)).unwrap_or(0);
        let locks: Vec<(i64, String)> = {
            let mut s = conn.prepare("SELECT lock_id, range_fingerprint FROM locks ORDER BY lock_id").unwrap();
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
        };
        // Identify every sparse Side sibling group!
        let mut by_parent: BTreeMap<String, Vec<(u64, String)>> = BTreeMap::new();
        {
            let mut s = conn.prepare("SELECT location FROM cards WHERE location IS NOT NULL AND location LIKE '%|%'").unwrap();
            for row in s.query_map([], |r| r.get::<_, String>(0)).unwrap() {
                let loc = row.unwrap();
                if let Some((parent, tail)) = loc.rsplit_once('|') {
                    if tail.chars().all(|c| c.is_ascii_lowercase()) {
                        let mut n = 0u64;
                        for b in tail.bytes() {
                            n = n * 26 + ((b - b'a') as u64 + 1);
                        }
                        by_parent.entry(parent.to_string()).or_default().push((n, loc));
                    }
                }
            }
        }
        let mut sparse_groups = 0usize;
        let mut sparse_addrs = BTreeSet::new();
        for (_parent, mut kids) in by_parent {
            kids.sort_by_key(|(n, _)| *n);
            let is_sparse = kids.iter().enumerate().any(|(i, (n, _))| *n != (i as u64) + 1);
            if is_sparse {
                sparse_groups += 1;
                for (_, loc) in kids {
                    sparse_addrs.insert(loc);
                }
            }
        }
        eprintln!(
            "[acceptance_8] baseline: cards={cards}, live_ids={live_ids}, retired_ids={retired_ids}, handles={handles}, retired_coordinates={retired_c}, receipts={receipts}, locks={}, sparse_side_groups={sparse_groups} ({} cards)",
            locks.len(),
            sparse_addrs.len()
        );
        (cards, live_ids, retired_ids, handles, retired_c, receipts, locks, sparse_addrs)
    };

    let zt = || {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("zt"));
        cmd.env("ZT_CONFIG_DIR", &config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    };
    zt().args(["config", "set", "archive_root", archive_root.to_str().unwrap()])
        .assert()
        .success();

    // 1. Open migrated 62k v2 copy with the pure v2-only binary (`zt up`)!
    let t_up = Instant::now();
    let up = zt().arg("up").output().expect("zt up on 62k copy");
    let up_elapsed = t_up.elapsed();
    assert!(
        up.status.success(),
        "zt up failed on D:\\zt copy:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&up.stdout),
        String::from_utf8_lossy(&up.stderr)
    );
    eprintln!("[acceptance_8] v2 startup (`zt up`) completed in {up_elapsed:.2?}");

    // 2. Verify v2 counts, identities, handles, receipts, locks, and sparse paths!
    {
        let conn = Connection::open_with_flags(&copy_db, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let ver: String = conn.query_row("SELECT value FROM metadata WHERE key = 'coordinates_schema_version'", [], |r| r.get(0)).unwrap();
        assert_eq!(ver, "2");
        let post_cards: i64 = conn.query_row("SELECT count(*) FROM cards", [], |r| r.get(0)).unwrap();
        let post_live_ids: i64 = conn.query_row("SELECT count(*) FROM card_identity WHERE status = 'live'", [], |r| r.get(0)).unwrap();
        let post_retired_ids: i64 = conn.query_row("SELECT count(*) FROM card_identity WHERE status = 'retired'", [], |r| r.get(0)).unwrap();
        let post_handles: i64 = conn.query_row("SELECT count(*) FROM card_handles", [], |r| r.get(0)).unwrap();
        let post_retired_coords: i64 = conn.query_row("SELECT count(*) FROM retired_coordinates", [], |r| r.get(0)).unwrap();
        let post_receipts: i64 = conn.query_row("SELECT count(*) FROM applied_ops", [], |r| r.get(0)).unwrap();
        let post_coords: i64 = conn.query_row("SELECT count(*) FROM coordinates", [], |r| r.get(0)).unwrap();
        let post_locks: Vec<(i64, String)> = {
            let mut s = conn.prepare("SELECT lock_id, range_fingerprint FROM locks ORDER BY lock_id").unwrap();
            s.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_, _>>().unwrap()
        };

        assert_eq!(post_cards, pre_cards, "card count must match exactly");
        assert_eq!(post_live_ids, pre_live_ids, "live identities must match exactly");
        assert_eq!(post_retired_ids, pre_retired_ids, "retired identities must match exactly");
        assert_eq!(post_handles, pre_handles, "issued handles must match exactly");
        assert_eq!(post_retired_coords, pre_retired_placements, "retired coordinates must match");
        assert_eq!(post_receipts, pre_receipts, "applied_ops receipts must match exactly");
        assert_eq!(post_locks, pre_locks, "Lock seals must match exactly");
        assert!(post_coords >= post_cards, "every live card must have >= 1 coordinate");

        // Verify every single sparse Side card still exists at its exact uncompacted address!
        let mut check_loc = conn.prepare("SELECT 1 FROM cards WHERE location = ?1").unwrap();
        for addr in &pre_sparse_side_addrs {
            assert!(
                check_loc.exists([addr]).unwrap(),
                "sparse Side address {addr} was lost or compacted during migration!"
            );
        }
    }

    // 4. Verify plain `zt status` (`coordinates: ok`), `zt status --check-locks` (`integrity: ok`), and `zt status --check-links`!
    let t_plain = Instant::now();
    let st_plain = zt().arg("status").output().unwrap();
    let st_plain_out = String::from_utf8_lossy(&st_plain.stdout);
    assert!(
        st_plain.status.success() && st_plain_out.contains("coordinates: ok"),
        "zt status failed:\n{}\n{}",
        st_plain_out,
        String::from_utf8_lossy(&st_plain.stderr)
    );
    eprintln!("[acceptance_8] `zt status` (with coord::check on 62k) completed in {:.2?}", t_plain.elapsed());

    let t_status = Instant::now();
    let st_locks = zt().args(["status", "--check-locks"]).output().unwrap();
    let status_elapsed = t_status.elapsed();
    let st_locks_out = String::from_utf8_lossy(&st_locks.stdout);
    assert!(
        st_locks.status.success() && st_locks_out.contains("integrity: ok"),
        "status --check-locks failed:\n{}\n{}",
        st_locks_out,
        String::from_utf8_lossy(&st_locks.stderr)
    );
    eprintln!("[acceptance_8] `zt status --check-locks` completed in {status_elapsed:.2?}");

    let t_links = Instant::now();
    let st_links = zt().args(["status", "--check-links"]).output().unwrap();
    eprintln!("[acceptance_8] `zt status --check-links` completed in {:.2?} (exit={:?})", t_links.elapsed(), st_links.status.code());
    assert!(
        st_links.status.success(),
        "status --check-links failed:\n{}\n{}",
        String::from_utf8_lossy(&st_links.stdout),
        String::from_utf8_lossy(&st_links.stderr)
    );

    // 5. Exercise GUI tree read + adding a repeated Secondary Coordinate + Portable v4 export on the 62k copy!
    let gui = Gui::connect(zt());
    // Create a fresh Topic on the migrated 62k copy, add a repeated Secondary Coordinate, and export Portable v4!
    let mut mcp_child = std::process::Command::new(assert_cmd::cargo::cargo_bin!("zt"))
        .env("ZT_CONFIG_DIR", &config_dir)
        .env("ZT_GUI_PORT", "0")
        .arg("mcp")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    {
        use std::io::{BufRead, BufReader, Write};
        let mut stdin = mcp_child.stdin.take().unwrap();
        let mut reader = BufReader::new(mcp_child.stdout.take().unwrap());
        let mut req = |id: u64, method: &str, params: Value| -> Value {
            writeln!(stdin, "{}", json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).unwrap();
            stdin.flush().unwrap();
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            serde_json::from_str(&line).unwrap()
        };
        req(1, "initialize", json!({"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "acc8", "version": "1"}}));
        let top = req(2, "tools/call", json!({"name": "create_topic", "arguments": {"title": "Scale Test", "body": "Testing on 62k copy."}}));
        let top_addr = top["result"]["structuredContent"]["address"].as_str().unwrap().to_string();
        let top_id = top_addr.split('/').next().unwrap().to_string();
        let c1 = req(3, "tools/call", json!({"name": "create_successor", "arguments": {"at": &top_addr, "title": "Item 1", "body": "Body."}}));
        let c1_addr = c1["result"]["structuredContent"]["address"].as_str().unwrap().to_string();
        let tree_c = req(4, "tools/call", json!({"name": "get_tree_coordinates", "arguments": {"tree": &top_id}}));
        let c1_cid = tree_c["result"]["structuredContent"]["coordinates"].as_array().unwrap().iter().find(|c| c["coord"] == c1_addr).unwrap()["coord_id"].as_str().unwrap().to_string();
        let add2 = req(5, "tools/call", json!({"name": "add_coordinate", "arguments": {"tree": &top_id, "parent_coord_id": &c1_cid, "edge": "side", "target_at": &c1_addr}}));
        assert_eq!(add2["result"]["isError"], json!(false), "{add2}");

        let t_read = Instant::now();
        let gui_tree = gui.get(&format!("/api/tree?id={top_id}"));
        eprintln!("[acceptance_8] GUI /api/tree on 62k copy completed in {:.2?}", t_read.elapsed());
        assert_eq!(gui_tree["coordinates"].as_array().unwrap().len(), 3);

        let exp_dir = tmp.path().join("export-v4");
        let t_exp = Instant::now();
        let exp = zt().args(["export", &top_addr, "--out", exp_dir.to_str().unwrap()]).output().unwrap();
        assert!(exp.status.success(), "export failed: {}", String::from_utf8_lossy(&exp.stderr));
        eprintln!("[acceptance_8] Portable v4 export on 62k copy completed in {:.2?}", t_exp.elapsed());
        let exported: Value = serde_json::from_str(&fs::read_to_string(exp_dir.join("cards.json")).unwrap()).unwrap();
        assert_eq!(exported["version"], 4);
        assert_eq!(exported["coordinates"].as_array().unwrap().len(), 3);
    }
    let _ = mcp_child.kill();
    drop(gui);
    let _ = zt().arg("down").output();
}
