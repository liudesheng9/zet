//! The cross-book Dependency Lens (proposal §8.3) and one-time seeding of a Connection tree.

use anyhow::{Result, bail};
use rusqlite::Connection;
use serde_json::Value;

use crate::Access;

/// `get_dependencies`.
pub(crate) fn get(_conn: &Connection, _args: &Value) -> Result<Value> {
    bail!("get_dependencies is not available yet")
}

/// `seed_connection_tree`.
pub(crate) fn seed(
    _conn: &Connection,
    _access: Access,
    _args: &Value,
    _confirm: &mut dyn FnMut(&str, &[String], &Value) -> Result<Option<Value>>,
) -> Result<Value> {
    bail!("seed_connection_tree is not available yet")
}
