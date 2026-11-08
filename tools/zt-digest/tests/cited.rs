//! The Cited list seam (#219): `zt-digest cited -Step plan|precheck|apply` over a real scratch archive.
//! Since 2026-11-07 DeepSeek reads the reference material: the plan cuts it into parts and writes no Card,
//! the precheck holds each entry to its source lines and every line to an entry, and apply removes
//! repeated works, indexes the citing lines and writes the Cited list Card beside the Term list.
//! `support/cited_read.rs` answers the parts in place of the reading jobs.
//!
//! The test needs a `zt` binary built from this repository (or on PATH): it starts its own service
//! on a temporary archive. `ZT_TEST_ZT` overrides the binary.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

#[path = "support/zt_program.rs"]
mod zt_program;
use zt_program::zt_program;
#[path = "support/cited_read.rs"]
mod cited_read;

struct Zt {
    _dir: tempfile::TempDir,
    config: PathBuf,
    archive: PathBuf,
    program: PathBuf,
}

impl Zt {
    fn new(program: PathBuf) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = dir.path().join("config");
        let archive = dir.path().join("archive");
        let zt = Self {
            _dir: dir,
            config,
            archive,
            program,
        };
        let configured = zt.run(&[
            "config",
            "set",
            "archive_root",
            zt.archive.to_str().unwrap(),
        ]);
        assert!(
            configured.status.success(),
            "config: {}",
            String::from_utf8_lossy(&configured.stderr)
        );
        let up = zt.run(&["up"]);
        assert!(
            up.status.success(),
            "up: {} {}",
            String::from_utf8_lossy(&up.stdout),
            String::from_utf8_lossy(&up.stderr)
        );
        zt
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(&self.program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .output()
            .expect("run zt")
    }

    fn mcp(&self) -> Mcp {
        let mut child = Command::new(&self.program)
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config)
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
            next: 1,
        };
        mcp.request(
            "initialize",
            json!({"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"zt-digest-tests","version":"1"}}),
        );
        mcp.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
        mcp
    }

    fn digest(&self, args: &[&str]) -> Output {
        let program = std::env::var("ZT_TEST_DIGEST")
            .unwrap_or_else(|_| env!("CARGO_BIN_EXE_zt-digest").to_string());
        Command::new(program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_CHECK_ZT", &self.program)
            .output()
            .expect("run zt-digest")
    }
}

impl Drop for Zt {
    fn drop(&mut self) {
        let _ = self.run(&["down"]);
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next: u64,
}

impl Mcp {
    fn send(&mut self, value: &Value) {
        writeln!(self.stdin, "{value}").expect("write mcp");
        self.stdin.flush().expect("flush mcp");
    }

    fn recv(&mut self) -> Value {
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("read mcp");
            assert!(read > 0, "zt mcp closed its stdout");
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                return value;
            }
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next;
        self.next += 1;
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        loop {
            let value = self.recv();
            if value["id"] == json!(id) {
                if !value["error"].is_null() {
                    panic!("mcp {method} failed: {value}");
                }
                return value["result"].clone();
            }
        }
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({"name":tool,"arguments":arguments}));
        if result["isError"] == json!(true) {
            panic!("{tool} was refused: {result}");
        }
        let text = result["content"][0]["text"]
            .as_str()
            .unwrap_or_else(|| panic!("{tool} returned no text: {result}"));
        serde_json::from_str(text).unwrap_or_else(|error| panic!("{tool}: {error}: {text}"))
    }

    fn revision(&mut self) -> u64 {
        self.call("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"]
            .as_u64()
            .expect("archive revision")
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn last_json(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("no output: {}", String::from_utf8_lossy(&output.stderr)));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"))
}


/// A staging folder for `key` whose manifest names `source` (a fixture copied in) and the groups
/// that map a section heading to its anchor.
fn staging(parent: &Path, key: &str, title: &str, source: &str, groups: Value) -> PathBuf {
    let dir = parent.join(key);
    std::fs::create_dir_all(dir.join("sections")).expect("make staging");
    let path = dir.join("book.md");
    std::fs::write(&path, source).expect("write source");
    std::fs::write(
        dir.join("manifest.json"),
        json!({
            "citation_key": key,
            "book_title": title,
            "staging": dir.to_string_lossy(),
            "source": path.to_string_lossy(),
            "source_sha256": "0".repeat(64),
            "groups": groups,
        })
        .to_string(),
    )
    .expect("write manifest");
    dir
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cited").join(name))
        .expect("fixture")
}

fn book(mcp: &mut Mcp, key: &str, title: &str) {
    mcp.call("create_literature", json!({ "bibtex": format!("@article{{{key}, title={{{title}}}}}") }));
    mcp.call(
        "create_successor",
        json!({ "at": key, "title": "1 引言", "body": format!("引言。\n\n来源：[[{key}]] §1") }),
    );
}

fn merged_rows(staging: &Path) -> Vec<Value> {
    let merged: Value = serde_json::from_str(
        &std::fs::read_to_string(staging.join("cited").join("merged.json")).expect("cited merged rows"),
    )
    .expect("merged JSON");
    merged["rows"].as_array().expect("rows").clone()
}

fn row<'a>(rows: &'a [Value], id: &str) -> &'a Value {
    rows.iter().find(|row| row["id"] == id).unwrap_or_else(|| panic!("no row {id}: {rows:#?}"))
}

fn lines_of(row: &Value) -> Vec<u64> {
    row["candidates"].as_array().unwrap().iter().map(|c| c["line"].as_u64().unwrap()).collect()
}

fn precheck(zt: &Zt, staging: &Path, answer: &Path) -> String {
    let out = zt.digest(&["cited", "-Step", "precheck", "-Staging", staging.to_str().unwrap(), "-Draft", answer.to_str().unwrap()]);
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn the_plan_cuts_the_reference_material_into_reading_parts_and_writes_no_card() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "long2026", "Long Book");
    let before = mcp.revision();
    let mut source = String::from("# 1 Introduction\n# 1 引言\nAs Author7 (1907) argues, things matter.\n\n# Bibliography\n\n# 参考文献\n\nWorks are cited by short title in the notes.\n\n");
    for n in 1..=130 {
        source.push_str(&format!("Author{n}, A., Title number {n} (London, {}).\\\n", 1900 + n % 100));
        source.push_str(&format!("作者{n}，题名第 {n} 号（伦敦，{}）。\\\n", 1900 + n % 100));
    }
    source.push_str("\n# Index\n\nAbbasids, 12, 45\n");
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "long2026", "Long Book", &source, json!([{ "anchor": "long2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    assert_eq!(line["ok"], json!(true), "{line}");
    assert_eq!(line["rows"], json!(131), "the note and 130 entries, no translation, no index: {line}");
    assert_eq!(line["parts"], json!(3), "{line}");
    assert_eq!(line["jobs"], json!(1), "{line}");
    assert_eq!(mcp.revision(), before, "the plan writes no Card");
    let jobs: Value = serde_json::from_str(&std::fs::read_to_string(line["file"].as_str().unwrap()).unwrap()).unwrap();
    let job = &jobs[0];
    assert_eq!(job["files"].as_array().unwrap().len(), 2, "{job}");
    assert_eq!(job["then"].as_array().unwrap().len(), 2, "one conversation, one message a part: {job}");
    for file in job["files"].as_array().unwrap().iter().chain(job["then"].as_array().unwrap().iter().flat_map(|t| t["files"].as_array().unwrap())) {
        assert!(Path::new(file.as_str().unwrap()).is_file(), "{file}");
    }
    let first = std::fs::read_to_string(job["files"][1].as_str().unwrap()).unwrap();
    assert!(first.contains("L9: Works are cited by short title in the notes."), "{first}");
    assert!(first.contains("SELF-CHECK: zt-digest cited -Step precheck"), "{first}");
    assert!(!first.contains("作者1，"), "the translation is no material:\n{first}");

    // A job whose parts all pass is not listed again.
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    let again = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    assert_eq!(again["jobs"], json!(0), "{again}");
    assert_eq!(again["settled"], json!(1), "{again}");
}

const SMALL: &str = "# 1 Introduction\n# 1 引言\nAs Smith (1999) argues, bargaining matters.\nAl-Azdi's chronicle (1967) is used here.\n\n# References\n\nSmith, J. (1999). Bargaining. Journal of Tests, 1:1–2.\\\n——(2001). More Bargaining. Journal of Tests, 3:1–2.\\\nal-Azdī: al-Azdī, Abū Zakarīyā, Ta’rīkh al-Mawṣil (Cairo, 1967). Fihrist: see al-Nadīm.\\\nSmith, J. (1999). Bargaining. Journal of Tests, 1:1–2.\\\n\n# Appendix\n";

#[test]
fn the_precheck_holds_each_entry_to_its_lines_and_every_line_to_an_entry() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "small2026", "Small Book");
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "small2026", "Small Book", SMALL, json!([{ "anchor": "small2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    assert_eq!(line["rows"], json!(4), "{line}");
    let answer = staging.join("cited").join("read").join("out").join("r01-1.json");
    let smith = json!({ "lines": [8], "entry": "Smith, J. (1999). Bargaining. Journal of Tests, 1:1–2.", "authors": "Smith, J.", "year": "1999", "title": "Bargaining", "venue": "Journal of Tests" });
    let dash = json!({ "lines": [9], "entry": "——(2001). More Bargaining. Journal of Tests, 3:1–2.", "authors": "Smith, J.", "year": "2001", "title": "More Bargaining", "venue": "Journal of Tests" });
    // One OCR line holds an entry and a cross-reference: the entry is cut out of it, the line counts as covered.
    let azdi = json!({ "lines": [10], "entry": "al-Azdī: al-Azdī, Abū Zakarīyā, Ta’rīkh al-Mawṣil (Cairo, 1967).", "authors": "al-Azdī, Abū Zakarīyā", "year": "1967", "title": "Ta'rīkh al-Mawṣil", "venue": "Cairo" });
    let again = json!({ "lines": [11], "entry": "Smith, J. (1999). Bargaining. Journal of Tests, 1:1–2.", "authors": "Smith, J.", "year": "1999", "title": "Bargaining", "venue": "Journal of Tests" });
    let write = |entries: Value, skip: Value| std::fs::write(&answer, json!({ "entries": entries, "skip": skip }).to_string()).unwrap();

    write(json!([smith, dash, azdi, again]), json!([]));
    let text = precheck(&zt, &staging, &answer);
    assert!(text.trim_end().ends_with("faults: 0"), "{text}");

    let mut wrong = azdi.clone();
    wrong["entry"] = json!("al-Azdi: al-Azdi, Abu Zakariya, Tarikh al-Mawsil (Cairo, 1967).");
    write(json!([smith, dash, wrong, again]), json!([]));
    assert!(precheck(&zt, &staging, &answer).contains("entry3 R2"), "an entry not copied letter by letter");
    let mut wrong = smith.clone();
    wrong["authors"] = json!("Jones, K.");
    write(json!([wrong, dash, azdi, again]), json!([]));
    assert!(precheck(&zt, &staging, &answer).contains("entry1 R3"));
    let mut wrong = dash.clone();
    wrong["authors"] = json!("");
    write(json!([smith, wrong, azdi, again]), json!([]));
    assert!(precheck(&zt, &staging, &answer).contains("entry2 R3"), "a dash entry names the authors it repeats");
    let mut wrong = smith.clone();
    wrong["year"] = json!("");
    write(json!([wrong, dash, azdi, again]), json!([]));
    assert!(precheck(&zt, &staging, &answer).contains("entry1 R4"));
    let mut wrong = smith.clone();
    wrong["title"] = json!("Haggling");
    write(json!([wrong, dash, azdi, again]), json!([]));
    assert!(precheck(&zt, &staging, &answer).contains("entry1 R5"));
    write(json!([smith, dash, azdi]), json!([]));
    let text = precheck(&zt, &staging, &answer);
    assert!(text.contains("L11 R6") && text.contains("Smith, J. (1999)"), "an uncovered line is shown with its text:\n{text}");
    write(json!([smith, dash, azdi]), json!([11]));
    assert!(precheck(&zt, &staging, &answer).trim_end().ends_with("faults: 0"));
    let mut wrong = smith.clone();
    wrong["lines"] = json!([3]);
    write(json!([wrong, dash, azdi, again]), json!([]));
    assert!(precheck(&zt, &staging, &answer).contains("entry1 R1"), "a line outside the job");
}

#[test]
fn apply_waits_for_every_part_then_removes_repeated_works_and_writes_the_list_once() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "small2026", "Small Book");
    let terms = mcp.call(
        "create_successor",
        json!({ "at": "small2026/1", "kind": "side", "type": "term", "title": "术语表：《Small Book》的 0 个术语", "body": "本卡是术语表。\n\n来源：[[small2026]]" }),
    );
    assert_eq!(terms["address"], json!("small2026/1|a"));
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "small2026", "Small Book", SMALL, json!([{ "anchor": "small2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    let refused = zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]);
    assert!(!refused.status.success(), "apply refuses while a part has no answer");
    assert!(String::from_utf8_lossy(&refused.stderr).contains("r01-1"), "{}", String::from_utf8_lossy(&refused.stderr));

    let answer = staging.join("cited").join("read").join("out").join("r01-1.json");
    std::fs::write(&answer, json!({ "entries": [
        { "lines": [8], "entry": "Smith, J. (1999). Bargaining. Journal of Tests, 1:1–2.", "authors": "Smith, J.", "year": "1999", "title": "Bargaining", "venue": "" },
        { "lines": [9], "entry": "——(2001). More Bargaining. Journal of Tests, 3:1–2.", "authors": "Smith, J.", "year": "2001", "title": "More Bargaining", "venue": "" },
        { "lines": [10], "entry": "al-Azdī: al-Azdī, Abū Zakarīyā, Ta’rīkh al-Mawṣil (Cairo, 1967).", "authors": "al-Azdī, Abū Zakarīyā", "year": "1967", "title": "Ta’rīkh al-Mawṣil", "venue": "" },
        { "lines": [11], "entry": "Smith, J. (1999). Bargaining. Journal of Tests, 1:1–2.", "authors": "Smith, J.", "year": "1999", "title": "Bargaining", "venue": "" }
    ], "skip": [] }).to_string()).unwrap();
    let applied = last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["ok"], json!(true), "{applied}");
    assert_eq!(applied["card"], json!("small2026/1|b"), "{applied}");
    assert_eq!(applied["rows"], json!(3), "{applied}");
    assert_eq!(applied["repeated"], json!(1), "{applied}");
    let rows = merged_rows(&staging);
    assert_eq!(rows[1]["entry"], json!("Smith, J. (2001). More Bargaining. Journal of Tests, 3:1–2."), "the dash run names its authors");
    assert_eq!(rows[1]["id"], json!("smith2001"));
    assert_eq!(lines_of(row(&rows, "smith1999")), vec![3]);
    assert_eq!(lines_of(row(&rows, "alazdi1967")), vec![4], "a transliterated name is found folded");
    let cited = mcp.call("get_cited", json!({ "at": "small2026" }));
    assert_eq!(cited["list"], json!("small2026/1|b"));
    assert_eq!(cited["rows"].as_array().unwrap().len(), 3);
    let card = mcp.call("get_card", json!({ "at": "small2026/1|b" }));
    assert_eq!(card["type"], json!("cited"));
    assert_eq!(card["title"], json!("参考文献：《Small Book》引用的 3 篇文献"));

    let revision = mcp.revision();
    let again = last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(again["card"], json!("small2026/1|b"));
    assert_eq!(mcp.revision(), revision, "a second apply changes nothing");
}

#[test]
fn the_material_leaves_out_a_translated_list_and_indexes_the_hansen_citing_lines() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    book(&mut mcp, "grossman2026", "Grossman");
    let gross = staging(dir.path(), "grossman2026", "Grossman", &fixture("grossman.md"), json!([{ "anchor": "grossman2026/1", "headings": ["I. 引言"] }]));
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", gross.to_str().unwrap()]));
    assert_eq!(line["rows"], json!(6), "the six English entries, not their seven translations: {line}");

    book(&mut mcp, "hansen2021demand", "The Demand for Executive Skills");
    let staging = staging(
        dir.path(),
        "hansen2021demand",
        "The Demand for Executive Skills",
        &fixture("hansen.md"),
        json!([
            { "anchor": "hansen2021demand/1", "headings": ["1 引言"] },
            { "anchor": "hansen2021demand/9", "headings": ["5 社交技能需求", "5.1 高管层社交技能需求的建模"] },
        ]),
    );
    let s = staging.to_str().unwrap();
    assert_eq!(last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]))["rows"], json!(35));
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    let applied = last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["rows"], json!(35), "{applied}");
    let rows = merged_rows(&staging);
    let ids: std::collections::HashSet<_> = rows.iter().map(|row| row["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 35, "the reference ids are unique");
    assert!(ids.contains("custodio2013a") && ids.contains("custodio2013b"), "{ids:?}");
    let deming = row(&rows, "deming2017");
    let lines = lines_of(deming);
    for expected in [13, 14, 16, 46, 47, 48] {
        assert!(lines.contains(&expected), "line {expected} cites Deming (2017): {lines:?}");
    }
    assert_eq!(deming["candidates"][0]["anchor"], json!("hansen2021demand/1"));
    assert!(!lines_of(row(&rows, "deming2018")).contains(&46), "Deming and Kahn (2018) is another work");
}

/// Pilot 2026-10-31: an adopted staging names the staged Chinese-only `book.zh.md` as its source, which holds
/// no English reference list. `-Source` names the whole bilingual file; the plan freezes it, and the Cited
/// Card jobs read that file, never the manifest's. 2026-11-07: `-Heading` adds headings, comma-separated.
#[test]
fn an_explicit_source_and_extra_headings_reach_the_frozen_plan() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "small2026", "Small Book");
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "small2026", "Small Book", "# 1 引言\n正如 Smith（1999）所说，谈判很重要。\n", json!([{ "anchor": "small2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    assert_eq!(line["rows"], json!(0), "the Chinese-only source has no list: {line}");

    let whole = dir.path().join("whole.md");
    std::fs::write(&whole, format!("{SMALL}\n# AUTHOR INDEX\n\nFull citations of the works listed here may be found on the pages indicated.\n\nBrown, L., A Listed Work, 12\n")).unwrap();
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s, "-Source", whole.to_str().unwrap()]));
    assert_eq!(line["rows"], json!(4), "{line}");
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s, "-Source", whole.to_str().unwrap(), "-Heading", "Sources,Author Index"]));
    assert_eq!(line["rows"], json!(6), "{line}");
    let plan: Value = serde_json::from_str(&std::fs::read_to_string(staging.join("cited").join("plan.json")).unwrap()).unwrap();
    assert_eq!(plan["source"], json!(whole.to_string_lossy()));
    assert_eq!(plan["heading"], json!("Sources,Author Index"));
    cited_read::answer_parts(&staging, &|text: &str| {
        if text.starts_with("Brown") {
            return Some(("Brown, L.".into(), String::new(), "A Listed Work".into()));
        }
        cited_read::default_fields(text).filter(|_| !text.starts_with("al-Azd") && !text.starts_with("——"))
    });
    let applied = last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["rows"], json!(2), "{applied}");
    let rows = merged_rows(&staging);
    assert_eq!(rows[0]["candidates"][0]["line"], json!(3), "the lines are the whole file's");
    assert_eq!(rows[1]["year"], json!(""), "an author index has no years");

    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let job = std::fs::read_to_string(staging.join("cited-cards").join("jobs").join("t001.md")).unwrap();
    assert!(job.contains(&whole.to_string_lossy().to_string()), "the writer reads the frozen whole source:\n{job}");
}
