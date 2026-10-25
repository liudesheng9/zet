//! zt-check as a library: one process-free core of the Card quality check, the
//! text semantics it reads, and the report policy; plus, with the `cli` feature,
//! the command-line input adapter and the `zt mcp` client.
//!
//! The core, text, and policy modules are always available, so the zt service
//! links this crate without a gate process, a second parser, or a dependency
//! cycle. The zt-check binary and zt-digest (tools/zt-digest) add the `cli`
//! feature, so a digest step still reads Cards through its own `zt mcp`.

/// The checker's own version, so a run report can name the binary that checked its Cards.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod contract;
pub mod core;
pub mod policy;
pub mod text;

#[cfg(feature = "cli")]
pub mod check;
#[cfg(feature = "cli")]
pub mod graph;
#[cfg(feature = "cli")]
pub mod mcp;
