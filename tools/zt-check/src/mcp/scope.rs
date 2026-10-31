//! The process-wide declaration of the Literature scope (docs/literature-only-digest-design.md,
//! docs/agent-write-scope-proposal.md). A digest builds the book's Literature tree and nothing else,
//! so a digest process sets the Literature scope once, before its first archive call; its `zt mcp`
//! then starts with `--scope literature` and the server refuses every write outside it. The rule
//! itself is `crate::scope::literature_violation`, which the server and a draft check share.

pub use crate::scope::{Violation, is_topic_address, literature_violation, topic_links};
use std::sync::atomic::{AtomicBool, Ordering};

static LITERATURE_ONLY: AtomicBool = AtomicBool::new(false);

/// Puts this process in the Literature scope (true) or back in the open scope (false).
pub fn set_literature_only(on: bool) {
    LITERATURE_ONLY.store(on, Ordering::SeqCst);
}

/// Whether this process is in the Literature scope.
pub fn literature_only() -> bool {
    LITERATURE_ONLY.load(Ordering::SeqCst)
}