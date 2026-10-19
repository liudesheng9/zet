//! A consumer that closes zt's stdout early is not a command failure.
//!
//! `zt status --check-links | head -1` and `Select-Object -Last 1` close the pipe while the
//! command is still finishing; Rust's `println!` then panics with a broken-pipe write. These
//! tests drive the installed binary's real process boundary with no reader, so the last write
//! fails, and assert that the command reports success without a panic line.

use assert_cmd::prelude::*;
use std::process::{Command, Stdio};

fn zt() -> Command {
    Command::cargo_bin("zt").expect("zt binary")
}

#[test]
fn a_closed_stdout_pipe_is_no_failure() {
    let mut child = zt()
        .arg("version")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zt");
    // No reader for the command's output: the write meets a broken pipe.
    drop(child.stdout.take());
    let output = child.wait_with_output().expect("zt exits");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "a closed stdout pipe must not fail the command: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "a closed stdout pipe must not panic: {stderr}"
    );
}

#[test]
fn an_ordinary_run_still_reports_its_output() {
    let output = zt().arg("version").output().expect("zt runs");
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("zt "), "{stdout}");
}
