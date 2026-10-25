//! S29 (#171): the legal batch paths and their measured cost (R0553).
//!
//! `src/batch.rs` bounds a real batch at 64 items and a preview sequence at 1024. The design's
//! benchmark asks for a legal 64-item real batch and a 200-item preview sequence, and says not to
//! pretend an invalid 200-item real batch is legal. This file proves the 200-item preview is a
//! legal, complete plan that stores nothing, then measures both legal paths on a scratch archive.
//!
//! The timing test is `#[ignore]`d (it is a measurement, not a pass/fail case). Run it with:
//!
//!   cargo test --release --test gate_batches -- --ignored --nocapture
//!
//! The archive is a scratch one (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never
//! touched.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// The real batch's item bound and the preview sequence's bound (src/batch.rs).
const REAL_BATCH: usize = 64;
const PREVIEW_SEQUENCE: usize = 200;

/// A body the rules accept as advice only, so a legal plan reports no refusal.
const BODY: &str = "这一段说明背景并给出充分理由。这一段说明结果如何出现。它让读者看到条件。";

struct Home {
    _tmp: TempDir,
    config_dir: PathBuf,
    archive_root: PathBuf,
}

impl Home {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
        };
        home.run(&[
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().expect("archive root"),
        ]);
        home.run(&["up"]);
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().expect("zt command");
        assert!(
            out.status.success(),
            "zt {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn conn(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open archive")
    }

    fn cards(&self) -> i64 {
        self.conn()
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("card count")
    }

    fn receipts(&self) -> i64 {
        self.conn()
            .query_row("SELECT COUNT(*) FROM applied_ops", [], |row| row.get(0))
            .unwrap_or(0)
    }

    fn revision(&self) -> i64 {
        self.conn()
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    }

    /// The stored Link-index write statistics of the last write.
    fn last_write(&self) -> Value {
        let value: Option<String> = self
            .conn()
            .query_row(
                "SELECT value FROM metadata WHERE key = 'link_write_stats'",
                [],
                |row| row.get(0),
            )
            .ok();
        value
            .and_then(|value| serde_json::from_str(&value).ok())
            .unwrap_or(Value::Null)
    }

    fn archive_bytes(&self) -> u64 {
        fs::metadata(self.archive_root.join("zt.sqlite3"))
            .map(|meta| meta.len())
            .unwrap_or(0)
    }

    fn mcp(&self) -> Mcp {
        let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        mcp.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "zt-tests", "version": "1" } }),
        );
        mcp.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Mcp {
    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write MCP message");
        self.stdin.flush().expect("flush MCP message");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read MCP reply");
        serde_json::from_str(&line).expect("MCP reply is JSON")
    }

    fn call(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let reply = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &reply["result"];
        (
            result["structuredContent"].clone(),
            result["isError"].as_bool().expect("isError flag"),
        )
    }

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(!is_error, "{name}({arguments}) failed: {value}");
        value
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

/// The 64 create items of one real batch: direct successors of `at`, all with the same passing
/// body.
fn real_batch_items(at: &str, run: usize) -> Value {
    chain_items(at, REAL_BATCH, "s29/batch", run)
}

/// The 200 create items of one preview sequence. A preview sequence addresses its items by their
/// zero-based index, so each item continues the one before it.
fn preview_items(at: &str, run: usize) -> Value {
    // The op_key's third component is a positive plan revision, so the run index starts at one.
    chain_items(at, PREVIEW_SEQUENCE, "s29/preview", run + 1)
}

/// A chain of direct successors: item 0 under `at`, every later item under the candidate the item
/// before it created. A batch of independent direct successors of one Card would collide on the
/// same address, which no legal digest batch does.
fn chain_items(at: &str, count: usize, prefix: &str, run: usize) -> Value {
    let items: Vec<Value> = (0..count)
        .map(|index| {
            let parent = if index == 0 {
                at.to_string()
            } else {
                format!("@@{}", index - 1)
            };
            json!({
                "op_key": format!("{prefix}/{run}/build/item-{index}"),
                "client_ref": format!("{index}"),
                "tool": "create_successor",
                "arguments": {
                    "at": parent,
                    "title": format!("批卡 {index}"),
                    "body": BODY,
                }
            })
        })
        .collect();
    json!(items)
}

fn topic(mcp: &mut Mcp, title: &str) -> String {
    mcp.ok("create_topic", json!({ "title": title, "body": "主题说明。" }))["address"]
        .as_str()
        .expect("topic address")
        .to_string()
}

/// R0553: the design's correction of the proposal's invalid 200-item real batch. A 200-item
/// preview sequence is legal (the bound is 1024), it checks every item, and it persists nothing.
#[test]
fn a_two_hundred_item_preview_sequence_is_a_legal_plan_and_stores_nothing() {
    let home = Home::new();
    let mut mcp = home.mcp();
    let at = topic(&mut mcp, "预览主题");
    let cards = home.cards();
    let revision = home.revision();
    let receipts = home.receipts();
    let draft = mcp.ok(
        "apply_batch",
        json!({ "preview": true, "preview_items": preview_items(&at, 1) }),
    );
    assert_eq!(draft["preview"], json!(true), "{draft}");
    assert_eq!(draft["applied"], json!(false), "{draft}");
    assert_eq!(draft["complete"], json!(true), "{draft}");
    assert_eq!(
        draft["scan"],
        json!({"items": PREVIEW_SEQUENCE, "checked": PREVIEW_SEQUENCE, "refused": 0, "blocked": 0}),
        "{draft}"
    );
    assert_eq!(
        draft["results"].as_array().map(Vec::len),
        Some(PREVIEW_SEQUENCE),
        "every planned item resolves: {draft}"
    );
    assert_eq!(
        draft["cards"].as_array().map(Vec::len),
        Some(PREVIEW_SEQUENCE),
        "every checked item projects a Card: {draft}"
    );
    assert_eq!(draft["archive_revision"], json!(revision), "{draft}");
    assert_eq!(home.cards(), cards, "the preview stored no Card");
    assert_eq!(home.revision(), revision, "the preview moved no revision");
    assert_eq!(home.receipts(), receipts, "the preview kept no receipt");
}

/// R0553: a legal 64-item real batch publishes one revision and one combined Link refresh, and
/// every item is stored.
#[test]
fn a_sixty_four_item_real_batch_publishes_one_revision_and_keeps_every_card() {
    let home = Home::new();
    let mut mcp = home.mcp();
    let at = topic(&mut mcp, "批主题");
    let cards = home.cards();
    let revision = home.revision();
    let done = mcp.ok("apply_batch", json!({ "items": real_batch_items(&at, 1) }));
    assert_eq!(
        done["results"].as_array().map(Vec::len),
        Some(REAL_BATCH),
        "{done}"
    );
    assert_eq!(
        home.cards(),
        cards + REAL_BATCH as i64,
        "every item of the legal batch is stored"
    );
    assert_eq!(
        home.revision(),
        revision + 1,
        "one real batch publishes one revision"
    );
    assert_eq!(
        home.receipts(),
        REAL_BATCH as i64,
        "every item keeps its receipt"
    );
    let stats = home.last_write();
    assert_eq!(stats["indexed_cards"], json!(REAL_BATCH), "{stats}");
    assert_eq!(stats["refreshed_cards"], json!(REAL_BATCH), "{stats}");
}

/// The measurement. Five runs of each legal path on one scratch archive; prints one JSON line with
/// median/p95 wall time, per-item microseconds, the revision and receipt deltas, the combined Link
/// refresh counts and the archive size. Run it in release mode.
#[test]
#[ignore = "measurement: run with --release --ignored --nocapture"]
fn measure_the_legal_batch_paths() {
    const RUNS: usize = 5;
    const WARMUP: usize = 1;
    let home = Home::new();
    let mut mcp = home.mcp();
    let mut real = Vec::new();
    let mut preview = Vec::new();
    let mut real_stats = Value::Null;
    let mut real_revision_delta = 0;
    let mut real_receipts_delta = 0;
    let mut preview_cards = 0;
    for run in 0..(WARMUP + RUNS) {
        // Each chain needs a parent with a free direct-successor slot, so every run starts its own
        // Topic.
        let real_at = topic(&mut mcp, &format!("测量批主题 {run}"));
        let before_cards = home.cards();
        let before_revision = home.revision();
        let before_receipts = home.receipts();
        let started = Instant::now();
        mcp.ok(
            "apply_batch",
            json!({ "items": real_batch_items(&real_at, 100 + run) }),
        );
        let elapsed = started.elapsed();
        if run >= WARMUP {
            real.push(elapsed.as_micros() as u64);
            real_revision_delta = home.revision() - before_revision;
            real_receipts_delta = home.receipts() - before_receipts;
            real_stats = home.last_write();
            assert_eq!(home.cards() - before_cards, REAL_BATCH as i64);
        }
        let preview_at = topic(&mut mcp, &format!("测量预览主题 {run}"));
        let before_cards = home.cards();
        let started = Instant::now();
        let draft = mcp.ok(
            "apply_batch",
            json!({ "preview": true, "preview_items": preview_items(&preview_at, run) }),
        );
        let elapsed = started.elapsed();
        assert_eq!(draft["complete"], json!(true), "{draft}");
        if run >= WARMUP {
            preview.push(elapsed.as_micros() as u64);
            preview_cards = draft["scan"]["checked"].as_u64().unwrap_or(0);
            assert_eq!(home.cards(), before_cards, "a preview stores nothing");
        }
    }
    let mut real_sorted = real.clone();
    real_sorted.sort_unstable();
    let mut preview_sorted = preview.clone();
    preview_sorted.sort_unstable();
    let at_fraction = |samples: &[u64], fraction: f64| -> u64 {
        samples[((samples.len() - 1) as f64 * fraction).round() as usize]
    };
    let report = json!({
        "measurement": "legal batch paths",
        "issue": "#171 (S29)",
        "profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "real_batch": {
            "items": REAL_BATCH,
            "runs": RUNS,
            "median_ms": at_fraction(&real_sorted, 0.5) as f64 / 1000.0,
            "p95_ms": at_fraction(&real_sorted, 0.95) as f64 / 1000.0,
            "max_ms": *real_sorted.last().expect("samples") as f64 / 1000.0,
            "per_item_us": at_fraction(&real_sorted, 0.5) / REAL_BATCH as u64,
            "revision_delta": real_revision_delta,
            "receipts": real_receipts_delta,
            "indexed_cards": real_stats["indexed_cards"],
            "refreshed_cards": real_stats["refreshed_cards"],
            "changed_reverse_sections": real_stats["changed_reverse_sections"],
        },
        "preview_sequence": {
            "items": PREVIEW_SEQUENCE,
            "runs": RUNS,
            "checked": preview_cards,
            "median_ms": at_fraction(&preview_sorted, 0.5) as f64 / 1000.0,
            "p95_ms": at_fraction(&preview_sorted, 0.95) as f64 / 1000.0,
            "max_ms": *preview_sorted.last().expect("samples") as f64 / 1000.0,
            "per_item_us": at_fraction(&preview_sorted, 0.5) / PREVIEW_SEQUENCE as u64,
            "persisted_cards": 0,
        },
        "archive_cards": home.cards(),
        "archive_bytes": home.archive_bytes(),
    });
    println!("{}", serde_json::to_string(&report).expect("report JSON"));
}
