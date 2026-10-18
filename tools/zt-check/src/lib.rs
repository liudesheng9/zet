//! zt-check as a library: the Card quality check and the `zt mcp` client.
//! The zt-check binary and zt-digest (tools/zt-digest) both use it, so a
//! digest step runs the check in its own process, on its own `zt mcp`.

/// The checker's own version, so a run report can name the binary that checked its Cards.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod check;
pub mod graph;
pub mod mcp;
