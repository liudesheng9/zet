//! The `zt` binary an integration test runs: `ZT_TEST_ZT` when set, otherwise the newest of the
//! repository builds (`target/debug`, `target/release`) and the `zt` on PATH, by `zt version`.
//! A stale repository build (an older zt left in target/debug) never wins over a newer installed zt.
//! A test file includes it with `#[path = "support/zt_program.rs"] mod zt_program;`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The numbers of a `zt version` line (`zt 0.49.0` -> [0, 49, 0]).
pub fn parse_version(line: &str) -> Option<Vec<u64>> {
    let version = line.trim().strip_prefix("zt ")?.split_whitespace().next()?;
    version.split('.').map(|part| part.parse().ok()).collect()
}

/// The candidate with the highest version; on a tie the earlier candidate wins.
pub fn newest(candidates: Vec<(PathBuf, Vec<u64>)>) -> Option<PathBuf> {
    let mut best: Option<(PathBuf, Vec<u64>)> = None;
    for (program, version) in candidates {
        if best.as_ref().is_none_or(|(_, top)| version > *top) {
            best = Some((program, version));
        }
    }
    best.map(|(program, _)| program)
}

fn version_of(program: &Path) -> Option<Vec<u64>> {
    let output = Command::new(program).arg("version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    parse_version(&String::from_utf8_lossy(&output.stdout))
}

pub fn zt_program() -> Option<PathBuf> {
    if let Ok(program) = std::env::var("ZT_TEST_ZT") {
        return Some(PathBuf::from(program));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let builds = [root.join("target/debug/zt.exe"), root.join("target/release/zt.exe")];
    let candidates = builds
        .into_iter()
        .filter(|candidate| candidate.is_file())
        .chain([PathBuf::from("zt")])
        .filter_map(|program| version_of(&program).map(|version| (program, version)))
        .collect();
    newest(candidates)
}
