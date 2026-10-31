//! The helper declares its Write scope to the server (#226, docs/agent-write-scope-proposal.md D4,
//! D5, D8, D10): `zt-digest call` runs in the Literature scope and passes `-Allow`/`-Beneath` to
//! `zt mcp`, `zt-digest open-call` runs in the open scope, and every refusal is the server's typed
//! `scope_refused`. The client checks of their own are gone, so the server's message is the one a
//! caller reads.
//!
//! The commands run over a real scratch archive with its own service. The test needs a `zt` with
//! the Write scope (0.50.0 or later); `ZT_TEST_ZT` names it.

use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

#[path = "support/zt_program.rs"]
mod zt_program;
use zt_program::zt_program;

struct Scratch {
    dir: tempfile::TempDir,
    config: PathBuf,
    program: PathBuf,
}

impl Scratch {
    /// A scratch archive with its service up, an agent Topic `0/0` with Card `0/1`, and an agent
    /// Literature tree `Smith2024` with Card `Smith2024/1`.
    fn seeded() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config");
        let archive = dir.path().join("archive");
        let program = zt_program().expect("a zt binary (set ZT_TEST_ZT)");
        let scratch = Self { dir, config, program };
        assert!(scratch.zt(&["config", "set", "archive_root", archive.to_str().unwrap()]).status.success());
        let up = scratch.zt(&["up"]);
        assert!(up.status.success(), "up: {}", String::from_utf8_lossy(&up.stderr));
        let seeded = scratch.helper(
            "open-call",
            &[],
            &json!([
                {"tool":"create_topic","arguments":{"title":"企业边界"}},
                {"tool":"create_successor","arguments":{"at":"0/0","title":"企业边界由交易成本决定"}},
                {"tool":"create_literature","arguments":{"bibtex":"@book{Smith2024, title={Firms}, year={2024}}"}},
                {"tool":"create_successor","arguments":{"at":"Smith2024","title":"第一章"}}
            ]),
        );
        assert_eq!(lines(&seeded), ["0 ok 0/0", "1 ok 0/1", "2 ok Smith2024", "3 ok Smith2024/1"], "{}", stderr(&seeded));
        scratch
    }

    fn zt(&self, args: &[&str]) -> Output {
        Command::new(&self.program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .output()
            .expect("run zt")
    }

    /// `zt-digest <tool> - <args>` with these calls on stdin.
    fn helper(&self, tool: &str, args: &[&str], calls: &Value) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_zt-digest"))
            .arg(tool)
            .arg("-")
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_CHECK_ZT", &self.program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run zt-digest");
        child.stdin.take().unwrap().write_all(calls.to_string().as_bytes()).unwrap();
        child.wait_with_output().expect("zt-digest output")
    }

    /// One read through the open helper.
    fn read(&self, tool: &str, arguments: Value) -> Value {
        let out = self.helper("open-call", &[], &json!([{"tool":tool,"arguments":arguments}]));
        let line = lines(&out).into_iter().next().unwrap_or_default();
        serde_json::from_str(line.strip_prefix("0 ").unwrap_or_default())
            .unwrap_or_else(|_| panic!("{tool}: {line} {}", stderr(&out)))
    }

    fn body(&self, at: &str) -> String {
        self.read("get_card", json!({"at":at}))["body"].as_str().unwrap().to_string()
    }

    fn list(&self, name: &str, lines: &[&str]) -> String {
        let path = self.dir.path().join(name);
        std::fs::write(&path, lines.iter().map(|l| format!("{l}\n")).collect::<String>()).unwrap();
        path.to_string_lossy().into_owned()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = self.zt(&["down"]);
    }
}

fn lines(out: &Output) -> Vec<String> {
    String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The typed refusal line the helper prints before `resume:`.
fn refusal(out: &Output) -> Value {
    let line = lines(out)
        .into_iter()
        .find(|line| line.starts_with("scope_refused {"))
        .unwrap_or_else(|| panic!("no scope_refused line: {:?} {}", lines(out), stderr(out)));
    serde_json::from_str(line.strip_prefix("scope_refused ").unwrap()).unwrap()
}

#[test]
fn a_digest_helper_call_meets_the_servers_literature_scope() {
    let scratch = Scratch::seeded();
    let out = scratch.helper("call", &[], &json!([{"tool":"create_successor","arguments":{"at":"0/1","title":"主题卡"}}]));
    assert_eq!(out.status.code(), Some(1));
    let refused = refusal(&out);
    assert_eq!(refused["reason"], "topic_tree");
    assert_eq!(refused["scope"], "literature");
    // The server's own sentence, not a client copy of the rule.
    assert!(refused["message"].as_str().unwrap().starts_with("this connection writes only Literature trees"), "{refused}");
    assert!(lines(&out)[0].starts_with("0 error scope_refused: "), "{:?}", lines(&out));
    assert!(lines(&out).last().unwrap().starts_with("resume: -From 0"));
    assert_eq!(scratch.read("get_tree", json!({"tree":"0"}))["cards"].as_array().unwrap().len(), 2);
}

#[test]
fn the_open_helper_is_its_own_command_and_the_digest_helper_has_no_open_switch() {
    let scratch = Scratch::seeded();
    let out = scratch.helper("call", &["-Open"], &json!([]));
    assert!(!out.status.success());
    assert!(stderr(&out).contains("open-call"), "{}", stderr(&out));
    let out = scratch.helper("open-call", &[], &json!([{"tool":"create_successor","arguments":{"at":"0/1","title":"主题卡"}}]));
    assert_eq!(lines(&out), ["0 ok 0/2"], "{}", stderr(&out));
    // The server's Topic title rule reaches the helper as its typed refusal.
    let out = scratch.helper("open-call", &[], &json!([{"tool":"create_topic","arguments":{"title":"企业 边界"}}]));
    assert!(lines(&out).iter().any(|line| line.starts_with("topic_exists {")), "{:?}", lines(&out));
}

#[test]
fn an_empty_allow_list_is_the_servers_and_covers_a_batch() {
    let scratch = Scratch::seeded();
    let none = scratch.list("none.allow.txt", &[]);
    let batch = json!([{"tool":"apply_batch","arguments":{"items":[
        {"op_key":"r/u/1/s/i0","client_ref":"a","tool":"edit_card","arguments":{"at":"Smith2024/1","body":"x","base_fingerprint":"0"}}
    ]}}]);
    let out = scratch.helper("call", &["-Allow", &none], &batch);
    let refused = refusal(&out);
    assert_eq!(refused["reason"], "not_listed", "{refused}");
    assert_eq!(refused["item"]["index"], 0);
    assert_eq!(scratch.body("Smith2024/1"), "");
}

#[test]
fn a_listed_run_grows_its_cards_and_a_beneath_card_takes_a_side_successor_only() {
    let scratch = Scratch::seeded();
    let mine = scratch.list("mine.allow.txt", &["Smith2024/1"]);
    let out = scratch.helper(
        "call",
        &["-Allow", &mine],
        &json!([
            {"tool":"create_successor","arguments":{"at":"Smith2024/1","title":"第二章"}},
            {"tool":"edit_card","arguments":{"at":"@@0","body":"第二章的要点。"}}
        ]),
    );
    assert_eq!(lines(&out), ["0 ok Smith2024/2", "1 ok Smith2024/2"], "{}", stderr(&out));
    let none = scratch.list("none.allow.txt", &[]);
    let beside = scratch.list("beside.txt", &["Smith2024/1"]);
    let side = scratch.helper("call", &["-Allow", &none, "-Beneath", &beside], &json!([{"tool":"create_successor","arguments":{"at":"Smith2024/1","kind":"side","title":"旁注"}}]));
    assert_eq!(lines(&side), ["0 ok Smith2024/1|a"], "{}", stderr(&side));
    let direct = scratch.helper("call", &["-Allow", &none, "-Beneath", &beside], &json!([{"tool":"create_successor","arguments":{"at":"Smith2024/1","title":"x"}}]));
    assert_eq!(refusal(&direct)["reason"], "beside_only");
}

#[test]
fn an_identified_write_outside_its_allow_list_stops_before_any_chunk_is_written() {
    let scratch = Scratch::seeded();
    let mine = scratch.list("mine.allow.txt", &["Smith2024/1"]);
    let calls: Vec<Value> = (0..70)
        .map(|i| json!({"tool":"create_successor","arguments":{"at":"Smith2024/1","kind":"side","title":format!("旁注 {i}")}}))
        .chain([json!({"tool":"create_successor","arguments":{"at":"0/1","title":"主题卡"}})])
        .collect();
    let out = scratch.helper("call", &["-Allow", &mine, "-OperationPrefix", "run/unit/1/link"], &json!(calls));
    assert_eq!(out.status.code(), Some(2), "{:?} {}", lines(&out), stderr(&out));
    let last: Value = serde_json::from_str(lines(&out).last().unwrap()).unwrap();
    assert_eq!(last["error"], "scope_refused", "{last}");
    // The first 64-item chunk was never committed: the book still has its two Cards.
    assert_eq!(scratch.read("get_tree", json!({"tree":"Smith2024"}))["cards"].as_array().unwrap().len(), 2);
}

#[test]
fn an_append_never_loses_a_concurrent_append() {
    let scratch = Scratch::seeded();
    let runs: Vec<_> = (0..2)
        .map(|writer| {
            let calls: Vec<Value> = (0..15)
                .map(|n| json!({"tool":"append_card","arguments":{"at":"Smith2024/1","text":format!("w{writer}-{n}")}}))
                .collect();
            let mut child = Command::new(env!("CARGO_BIN_EXE_zt-digest"))
                .args(["call", "-"])
                .env("ZT_CONFIG_DIR", &scratch.config)
                .env("ZT_GUI_PORT", "0")
                .env("ZT_CHECK_ZT", &scratch.program)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            child.stdin.take().unwrap().write_all(json!(calls).to_string().as_bytes()).unwrap();
            (writer, child)
        })
        .collect();
    let body_before = scratch.body("Smith2024/1");
    assert!(body_before.is_empty() || body_before.starts_with('w'));
    let mut appended = Vec::new();
    for (writer, child) in runs {
        let out = child.wait_with_output().unwrap();
        for line in lines(&out).iter().filter(|line| line.contains(" ok ")) {
            let n: usize = line.split(' ').next().unwrap().parse().unwrap();
            appended.push(format!("w{writer}-{n}"));
        }
        // A refused append is a conflict the caller sees, never a silent overwrite.
        if let Some(error) = lines(&out).iter().find(|line| line.contains(" error ")) {
            assert!(error.contains("changed since it was opened"), "{error}");
        }
    }
    let body = scratch.body("Smith2024/1");
    for text in &appended {
        assert!(body.lines().any(|line| line == text), "an accepted append `{text}` was lost:\n{body}");
    }
}
