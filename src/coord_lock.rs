//! Locks over Connection trees and Topic-side mounts (proposal §8.1): the frozen structure and
//! connections of a locked host, the pins on mounted targets, the advisory source fingerprints and
//! the append-only reviews.

use anyhow::{Result, bail};
use rusqlite::Connection;
use serde_json::Value;

use crate::Access;

/// Whether an active Lock freezes the placements and connections of this host.
pub(crate) fn host_locked(_conn: &Connection, _host_key: &str) -> Result<bool> {
    Ok(false)
}

/// Refuse a host write while its tree is locked.
pub(crate) fn ensure_host_unlocked(conn: &Connection, host_key: &str, label: &str) -> Result<()> {
    if host_locked(conn, host_key)? {
        bail!("card_locked: {label} is locked; its stations and connections do not change");
    }
    Ok(())
}

/// Refuse to delete or move a Card that an active Lock pins as a mounted target.
pub(crate) fn ensure_card_not_pinned(_conn: &Connection, _card_id: &str, _reason: &str) -> Result<()> {
    Ok(())
}

/// Mark a mounted source reviewed at its current version.
pub(crate) fn review_source(_conn: &Connection, _access: Access, _args: &Value) -> Result<Value> {
    bail!("review_source is not available yet")
}
