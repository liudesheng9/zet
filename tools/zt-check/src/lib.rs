//! zt-check as a library: the Card quality check and the `zt mcp` client.
//! The zt-check binary and zt-digest (tools/zt-digest) both use it, so a
//! digest step runs the check in its own process, on its own `zt mcp`.

pub mod check;
pub mod mcp;
