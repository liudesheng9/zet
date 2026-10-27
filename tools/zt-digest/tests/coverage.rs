//! The `coverage` command on a staged fixture (L2, #185): paragraph units, line-range and
//! file-only spans, skips, a draft's adds and skips, and the `-Step` line. The command reads
//! files only, so this seam needs no archive and no service.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn stage() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    std::fs::create_dir_all(root.join("sections")).unwrap();
    let manifest = json!({
        "citation_key": "K",
        "groups": [
            {"n": 1, "id": "g01", "anchor": "K/1", "sections": ["s01.md"]},
            {"n": 2, "id": "g02", "anchor": "K/2", "sections": ["s02.md"]}
        ]
    });
    std::fs::write(root.join("manifest.json"), manifest.to_string()).unwrap();
    // Three units, at lines 1, 3 and 5.
    let first = ["第一段的正文，长度超过二十个字，是一个段落单元。", "", "第二段的正文，长度超过二十个字，是一个段落单元。", "", "第三段的正文，长度超过二十个字，是一个段落单元。", ""].join("\n");
    std::fs::write(root.join("sections/s01.md"), first).unwrap();
    let second = ["另一节唯一的段落，长度超过二十个字，是一个段落单元。", ""].join("\n");
    std::fs::write(root.join("sections/s02.md"), second).unwrap();
    // The anchor's span names only a file (C2); the note's span is the first unit.
    let log = "K/1\t\\\\?\\D:\\book\\sections\\s01.md\nK/1|b\tsections/s01.md:1\n";
    std::fs::write(root.join("src-g01.txt"), log).unwrap();
    std::fs::write(root.join("src-g02.txt"), "K/2\t\\\\?\\D:\\book\\sections\\s02.md\n").unwrap();
    std::fs::write(root.join("coverage-skips-g01.json"), json!({"skips":[{"src":"sections/s01.md:5","reason":"致谢"}]}).to_string()).unwrap();
    dir
}

fn digest(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_zt-digest"))
        .arg("coverage")
        .args(args)
        .output()
        .expect("run zt-digest coverage")
}

fn stdout(output: &Output) -> Vec<String> {
    String::from_utf8_lossy(&output.stdout).lines().map(|line| line.trim_end_matches('\r').to_string()).collect()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn path(root: &Path) -> String {
    root.to_str().unwrap().to_string()
}

#[test]
fn a_group_check_lists_its_faults_and_counts_them() {
    let dir = stage();
    let out = dir.path();
    let output = digest(&["-Out", &path(out), "-Group", "1"]);
    let lines = stdout(&output);
    assert_eq!(output.status.code(), Some(1), "a faulted group exits 1: {}", stderr(&output));
    assert_eq!(
        lines,
        [
            "s01.md C2 file-only K/1",
            "s01.md:3 C1 uncovered \"第二段的正文，长度超过二十个字，是一个段落单元。\"",
            "faults: 2",
        ]
    );
}

#[test]
fn the_whole_run_counts_every_section_and_a_draft_adds_coverage() {
    let dir = stage();
    let out = dir.path();
    let draft = out.join("review.json");
    // The draft adds a note for the uncovered second paragraph and skips the second section's unit.
    std::fs::write(
        &draft,
        json!({
            "adds": [{"span": "sections/s01.md:3", "title": "第二段的标题", "body": "第二段的意思。"}],
            "skips": [{"span": "sections/s02.md:1", "reason": "重复", "names": "sections/s01.md:1"}]
        })
        .to_string(),
    )
    .unwrap();
    let output = digest(&["-Out", &path(out), "-Draft", &path(&draft)]);
    let lines = stdout(&output);
    assert_eq!(lines.last().map(String::as_str), Some("faults: 2"), "{lines:?} {}", stderr(&output));
    assert!(lines.iter().any(|line| line == "s01.md C2 file-only K/1"));
    assert!(lines.iter().any(|line| line == "s02.md C2 file-only K/2"));
    // The added note covers the second paragraph; the first and third of s01, and the first of
    // s02 (skipped), stay as they are.
    assert!(!lines.iter().any(|line| line.contains("s01.md:3-3 C1")));
}

#[test]
fn the_step_writes_coverage_json_and_one_short_line() {
    let dir = stage();
    let out = dir.path();
    std::fs::write(
        out.join("audit-cards.json"),
        json!([
            {"address": "K/1|b", "category": "idea", "body": "第一段说了 63 倍。"},
            {"address": "K/1", "category": "anchor", "body": "索引。"},
            {"address": "K/1|d", "category": "model", "body": "模型 12。"}
        ])
        .to_string(),
    )
    .unwrap();
    let output = digest(&["-Out", &path(out), "-Step"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let lines = stdout(&output);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].chars().count() <= 400, "{} characters", lines[0].chars().count());
    let value: Value = serde_json::from_str(&lines[0]).expect("one JSON line");
    assert_eq!(value["ok"], json!(true));
    assert_eq!(value["units"], json!(4));
    assert_eq!(value["covered"], json!(1));
    assert_eq!(value["skipped"], json!(1));
    assert_eq!(value["uncovered"], json!(2));
    assert_eq!(value["pct"], json!(50));
    assert!(value["detail"].as_str().unwrap().ends_with("coverage.json"));

    let coverage: Value = serde_json::from_str(&std::fs::read_to_string(out.join("coverage.json")).unwrap()).unwrap();
    assert_eq!(coverage["schema_version"], json!(1));
    assert_eq!(coverage["units"], json!(4));
    assert_eq!(coverage["faults"]["c1"], json!(2));
    assert_eq!(coverage["faults"]["c2"], json!(2));
    assert_eq!(coverage["faults"]["c4"], json!(0));
    assert_eq!(coverage["sections"].as_array().unwrap().len(), 2);
    // The anchor's only span names a file, so it carries no lines and no number can be checked;
    // the note's span is line 1, which states no number, so its 63 is the advisory C3.
    let note = coverage["cards"].as_array().unwrap().iter().find(|card| card["address"] == json!("K/1|b")).unwrap();
    assert_eq!(note["c3"], json!(["63"]));
    assert_eq!(coverage["faults"]["c3"], json!(1));
    assert_eq!(coverage["card_source"]["cards"], json!(3));
}

#[test]
fn a_group_draft_counts_the_cards_its_calls_propose() {
    let dir = stage();
    let out = dir.path();
    let draft = out.join("claim.json");
    std::fs::write(
        &draft,
        json!({
            "schema_version": 1,
            "unit": "digest/g001@1",
            "token": 1,
            "disposition": "cards",
            "calls": [{
                "tool": "create_successor",
                "arguments": {"at": "K/1", "kind": "side", "type": "idea", "title": "第二段", "body": "第二段的意思。", "src": "sections/s01.md:3"}
            }]
        })
        .to_string(),
    )
    .unwrap();
    let output = digest(&["-Out", &path(out), "-Group", "1", "-Draft", &path(&draft)]);
    let lines = stdout(&output);
    // The draft's Card covers the second paragraph, and the draft's numbers are not checked
    // because the snapshot names the written Cards only.
    assert!(!lines.iter().any(|line| line.contains("s01.md:3 C1")));
    assert_eq!(lines.last().map(String::as_str), Some("faults: 1"));
}
#[test]
fn a_staging_folder_without_a_manifest_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let output = digest(&["-Out", &dir.path().to_string_lossy()]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("holds no manifest.json"), "{}", stderr(&output));
}

#[test]
fn a_group_the_run_does_not_have_is_refused() {
    let dir = stage();
    let output = digest(&["-Out", &path(dir.path()), "-Group", "9"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("names no group"), "{}", stderr(&output));
}

#[allow(dead_code)]
fn unused(_: PathBuf) {}
