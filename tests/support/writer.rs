//! A direct SQLite connection to a test archive that may write it like a current `zt` writer.
//!
//! Since multi-tree coordinates (docs/multi-tree-coordinates-proposal.md §10.1.2) every table of an
//! upgraded archive is fenced: a connection that lacks the `zt_writer_epoch` function (an older
//! `zt`, or plain SQLite) cannot write it. The identity triggers on `cards` also call `zt_uuid_v4`
//! and `zt_card_title`. Tests that seed or tamper with an archive through raw SQL open it here, so
//! they act as a current-format writer; a test that proves the fence opens plain SQLite instead.

#![allow(dead_code)]

use rusqlite::Connection;
use rusqlite::functions::FunctionFlags;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT: AtomicU64 = AtomicU64::new(1);

/// Open the archive at `path` with the current writer functions registered.
pub fn open(path: impl AsRef<Path>) -> rusqlite::Result<Connection> {
    let conn = Connection::open(path)?;
    register(&conn);
    Ok(conn)
}

/// Register the current writer functions on an existing connection.
pub fn register(conn: &Connection) {
    conn.create_scalar_function("zt_writer_epoch", 0, FunctionFlags::SQLITE_UTF8, |_| Ok(1_i64))
        .expect("register zt_writer_epoch");
    conn.create_scalar_function("zt_uuid_v4", 0, FunctionFlags::SQLITE_UTF8, |_| {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos() as u64;
        Ok(format!(
            "{:08x}-{:04x}-4{:03x}-8{:03x}-{:012x}",
            (t >> 32) as u32,
            (t & 0xffff) as u16,
            (n >> 12) & 0xfff,
            n & 0xfff,
            (t ^ n.rotate_left(17)) & 0xffff_ffff_ffff
        ))
    })
    .expect("register zt_uuid_v4");
    conn.create_scalar_function("zt_card_title", 1, FunctionFlags::SQLITE_UTF8, |ctx| {
        let text: Option<String> = ctx.get(0)?;
        let text = text.unwrap_or_default();
        Ok(text
            .lines()
            .find(|line| !line.trim().is_empty())
            .and_then(|line| line.trim().strip_prefix("# "))
            .unwrap_or_default()
            .trim()
            .to_string())
    })
    .expect("register zt_card_title");
}

/// Remove the multi-tree coordinate storage from a test archive (its tables, triggers and
/// generation), so a fixture can rebuild the archive into the exact shape of a release that
/// predates it. The next `zt up` installs it again.
pub fn strip_coordinates(conn: &Connection) {
    let triggers: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'trigger'
                 AND (name LIKE 'writer_epoch_%' OR name LIKE 'identity_%'
                      OR name IN ('cards_row_id_immutable', 'card_identity_id_immutable',
                                  'card_identity_never_deleted', 'card_handles_permanent_update',
                                  'card_handles_permanent_delete'))",
            )
            .expect("trigger list");
        stmt.query_map([], |r| r.get::<_, String>(0))
            .expect("triggers")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("trigger names")
    };
    for trigger in triggers {
        conn.execute_batch(&format!("DROP TRIGGER {trigger};")).expect("drop trigger");
    }
    for table in [
        "retired_placements",
        "placement_edges",
        "placements",
        "connection_trees",
        "card_handles",
        "card_identity",
    ] {
        conn.execute_batch(&format!("DROP TABLE IF EXISTS {table};")).expect("drop table");
    }
    conn.execute(
        "DELETE FROM metadata WHERE key IN ('coordinates_schema_version', 'next_connection_id')",
        [],
    )
    .expect("forget the coordinate generation");
}
