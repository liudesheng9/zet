//! Seam 1 TDD tests: Schema v2, Writer Epoch 2, PRAGMA foreign_keys = ON, commit-enforced >= 1
//! filing_coord_id floor, sparse Side slot preservation, and atomic v1 -> v2 migration
//! (docs/multi-coordinate-cards-proposal.md §§3, 5, 11 P1-P2).

#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use mcp_client::ScratchBox;
use rusqlite::Connection;
use serde_json::json;

#[test]
fn fresh_archive_installs_schema_v2_epoch_2_and_enforces_filing_floor_at_commit() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);

    // Create a Topic tree and a Literature tree through public interfaces.
    let topic = mcp.ok("create_topic", json!({"title": "Foundations", "body": "Root claim."}));
    let topic_root = topic["address"].as_str().expect("topic address").to_string();
    let t1 = mcp.ok("create_successor", json!({"at": &topic_root, "title": "First", "body": "Body."}));
    let t1_addr = t1["address"].as_str().expect("t1 address").to_string();
    let lit = mcp.ok(
        "create_literature",
        json!({
            "bibtex": "@article{Smith2024,\n  author={Smith},\n  title={Paper},\n  year={2024}\n}",
            "title": "Smith 2024",
            "body": "Summary."
        }),
    );
    assert_eq!(lit["address"], "Smith2024");

    // Verify schema v2 metadata, tables, and writer epoch 2.
    let db_path = scratch.archive_root.join("zt.sqlite3");
    let conn = writer::open(&db_path).expect("open db");
    conn.pragma_update(None, "foreign_keys", "ON").expect("fk on");
    let version: String = conn
        .query_row("SELECT value FROM metadata WHERE key = 'coordinates_schema_version'", [], |r| r.get(0))
        .expect("schema version");
    assert_eq!(version, "2", "fresh archive must install coordinates_schema_version = 2");

    // Both Topic and Literature trees exist in unified `trees` and `coordinates`.
    let tree_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM trees", [], |r| r.get(0))
        .expect("trees count");
    assert_eq!(tree_count, 2);

    let (card_id, filing_coord_id, owner_ws): (String, String, String) = conn
        .query_row(
            "SELECT card_id, filing_coord_id, owner_workspace FROM card_identity WHERE last_address = ?1 AND status = 'live'",
            [&t1_addr],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .expect("live identity with filing_coord_id");
    assert!(!card_id.is_empty());
    assert!(!filing_coord_id.is_empty());
    assert_eq!(owner_ws, "agent");

    // Commit-enforced >= 1 floor: deleting a Card's only Coordinate MUST fail at COMMIT.
    conn.execute_batch("BEGIN IMMEDIATE;").expect("begin");
    conn.execute("DELETE FROM coordinates WHERE coord_id = ?1", [&filing_coord_id])
        .expect("deferred fk allows statement");
    let commit_err = conn.execute_batch("COMMIT;");
    assert!(commit_err.is_err(), "COMMIT must fail when a live Card has 0 Coordinates");
    let _ = conn.execute_batch("ROLLBACK;");
    drop(conn);

    // An older epoch-1 writer MUST be refused by the epoch-2 writer fence.
    let old = Connection::open(&db_path).expect("open old");
    old.create_scalar_function(
        "zt_writer_epoch",
        0,
        rusqlite::functions::FunctionFlags::SQLITE_UTF8,
        |_| Ok(1_i64),
    )
    .expect("register epoch 1");
    let old_write = old.execute("UPDATE metadata SET value = '1' WHERE key = 'coordinates_schema_version'", []);
    assert!(old_write.is_err(), "epoch-1 writer must be blocked by epoch-2 fence");
    drop(old);

    // `zt status` reports coordinates: ok.
    let status = scratch.cmd().arg("status").output().expect("zt status");
    assert!(status.status.success(), "zt status failed: {}", String::from_utf8_lossy(&status.stderr));
    let stdout = String::from_utf8_lossy(&status.stdout);
    assert!(stdout.contains("coordinates: ok"), "expected coordinates: ok, got:\n{stdout}");

    // Unknown future schema version ('3') is refused without modifying the database.
    drop(mcp);
    let conn = writer::open(&db_path).expect("open db");
    conn.execute("UPDATE metadata SET value = '3' WHERE key = 'coordinates_schema_version'", [])
        .expect("set future version");
    drop(conn);
    let _ = std::fs::remove_file(scratch.archive_root.join("zt.pid"));
    std::thread::sleep(std::time::Duration::from_millis(80));
    let refused = scratch.cmd().arg("up").output().expect("zt up on v3");
    assert!(!refused.status.success(), "future schema version 3 must be refused on startup");
}

#[test]
fn v2_only_binary_refuses_legacy_v1_generation_and_preserves_sparse_side_slots() {
    let scratch = ScratchBox::up();
    let db = scratch.archive_root.join("zt.sqlite3");
    let mut mcp = scratch.mcp(&[]);

    // Build sparse 0/1|a and 0/1|c (gap at |b preserved by broken link).
    let _ = mcp.ok("create_topic", json!({"title": "Sparse Topic", "body": "Root."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/0", "title": "First", "body": "Body."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": "Side A", "body": "A."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": "Side B", "body": "B."}));
    let _ = mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": "Side C", "body": "C."}));
    let _ = mcp.ok(
        "create_literature",
        json!({
            "bibtex": "@article{LitA,\n  author={Author},\n  title={Book A},\n  year={2020}\n}",
            "title": "LitA",
            "body": "Book."
        }),
    );
    let _ = mcp.ok("create_successor", json!({"at": "LitA", "title": "Note 1", "body": "See [[0/1|b]]."}));
    let plan = mcp.ok("delete_card", json!({"at": "0/1|b"}));
    let _ = mcp.ok(
        "delete_card",
        json!({
            "at": "0/1|b",
            "confirm": plan["confirm"],
            "plan_hash": plan["plan_hash"],
        }),
    );

    // Verify sparse slot 3 (0/1|c) is preserved in v2 `coordinates` without compaction.
    let conn = writer::open(&db).expect("open db");
    let slot_c: i64 = conn
        .query_row(
            "SELECT co.slot FROM coordinates co JOIN card_identity i ON i.filing_coord_id = co.coord_id WHERE i.last_address = '0/1|c'",
            [],
            |r| r.get(0),
        )
        .expect("slot of 0/1|c");
    assert_eq!(slot_c, 3, "sparse slot of 0/1|c MUST remain 3 (never compacted)");
    drop(conn);

    // Stop service and verify a pure v2-only binary cleanly refuses generation '1' without mutating the DB.
    drop(mcp);
    scratch.cmd().arg("down").assert().success();
    {
        let conn = writer::open(&db).expect("open db");
        conn.execute("UPDATE metadata SET value = '1' WHERE key = 'coordinates_schema_version'", [])
            .expect("mark v1");
    }
    let refused = scratch.cmd().arg("up").output().expect("zt up on v1");
    assert!(
        !refused.status.success(),
        "v2-only binary must refuse legacy generation 1"
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(
        stderr.contains("unsupported coordinate storage generation 1"),
        "expected clear generation-1 refusal, got: {stderr}"
    );
    let conn = writer::open(&db).expect("reopen db");
    let ver: String = conn
        .query_row("SELECT value FROM metadata WHERE key = 'coordinates_schema_version'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(ver, "1", "refused generation-1 DB must remain unmodified");
}
