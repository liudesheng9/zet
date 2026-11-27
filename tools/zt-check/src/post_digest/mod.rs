//! Post-digest structural and model audits internalized into `zt-check` (`cli` feature).
//!
//! These checks are strictly read-only and live outside the process-free write-gate core,
//! so they never alter `RULE_VERSION` or require rebuilding/restarting `zt.exe` while still
//! reusing `core`, `text`, `formal`, and `empirical` in-process.

pub mod charts;
pub mod draft_diff;
pub mod network;
pub mod premises;
pub mod settings;
pub mod stage;
pub mod tree;
