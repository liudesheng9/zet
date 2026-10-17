//! `zt-digest organize-survey` and `zt-digest organize-apply`: the ports of
//! `scripts/book-digest/organize-survey.ps1` and `organize-apply.ps1`, the two script steps of the
//! Organize stage (docs/digest-card-layers-proposal.md, proposal 8).
//!
//!   zt-digest organize-survey -Staging <folder> -Work <folder> [-Notes <file>] [-Similar 0.5]
//!                             [-CrossLinks 4] [-Small 3]
//!   zt-digest organize-apply  -Work <folder> [-DryRun]
//!
//! The survey runs the audit in-process into <Work>\audit and writes survey.md and survey.json;
//! apply carries out decisions.json (Topic merges and Card moves, one zt call at a time, every
//! later address translated through the `moved` maps zt returns) and writes applied.json.
//! `common` holds what these two and `terms` and `skeleton` share.

mod apply;
pub(crate) mod common;
mod survey;

use anyhow::Result;

pub fn survey_cli(argv: &[String]) -> Result<i32> {
    survey::cli(argv)
}

pub fn apply_cli(argv: &[String]) -> Result<i32> {
    apply::cli(argv)
}
