//! Which `zt` binary the integration tests run (tests/support/zt_program.rs). A stale repository
//! build (`target/debug/zt.exe` of an older zt) must not win over a newer `zt` on PATH: the Cited
//! seams failed with "card `` does not exist" when zt 0.42.0 from target/debug ran instead of 0.49.0.

#[path = "support/zt_program.rs"]
#[allow(dead_code)]
mod zt_program;

use std::path::PathBuf;
use zt_program::{newest, parse_version};

#[test]
fn a_version_line_reads_as_numbers() {
    assert_eq!(parse_version("zt 0.49.0\n"), Some(vec![0, 49, 0]));
    assert_eq!(parse_version("zt 1.2.10"), Some(vec![1, 2, 10]));
    assert_eq!(parse_version("something else"), None);
    assert_eq!(parse_version("zt x.y"), None);
}

#[test]
fn the_newest_zt_wins_over_a_stale_repository_build() {
    let debug = PathBuf::from("target/debug/zt.exe");
    let path = PathBuf::from("zt");
    let chosen = newest(vec![
        (debug.clone(), vec![0, 42, 0]),
        (path.clone(), vec![0, 49, 0]),
    ]);
    assert_eq!(chosen, Some(path));
    // 0.10 is newer than 0.9: versions compare as numbers, not text.
    let chosen = newest(vec![(debug.clone(), vec![0, 10, 0]), (PathBuf::from("zt"), vec![0, 9, 3])]);
    assert_eq!(chosen, Some(debug));
}

#[test]
fn a_tie_keeps_the_earlier_candidate() {
    let debug = PathBuf::from("target/debug/zt.exe");
    let chosen = newest(vec![(debug.clone(), vec![0, 49, 0]), (PathBuf::from("zt"), vec![0, 49, 0])]);
    assert_eq!(chosen, Some(debug));
    assert_eq!(newest(vec![]), None);
}
