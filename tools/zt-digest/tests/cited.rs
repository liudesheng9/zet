//! The Cited list seam (#219): `zt-digest cited -Step plan|precheck|apply` over a real scratch
//! archive and staged sources cut from four real books (Hansen 2021, Hart 1995, Ebrill 2001,
//! Acemoglu 2007). The plan parses the reference list of the whole source, indexes the lines that
//! cite each work, and writes no Card; apply writes the Cited list Card beside the Term list.
//!
//! The test needs a `zt` binary built from this repository (or on PATH): it starts its own service
//! on a temporary archive. `ZT_TEST_ZT` overrides the binary.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

fn zt_program() -> Option<PathBuf> {
    if let Ok(program) = std::env::var("ZT_TEST_ZT") {
        return Some(PathBuf::from(program));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for candidate in [
        root.join("target/debug/zt.exe"),
        root.join("target/release/zt.exe"),
    ] {
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    Command::new("zt")
        .arg("version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|_| PathBuf::from("zt"))
}

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

fn plan_rows(staging: &Path) -> Vec<Value> {
    let plan: Value = serde_json::from_str(
        &std::fs::read_to_string(staging.join("cited").join("plan.json")).expect("cited plan"),
    )
    .expect("plan JSON");
    plan["rows"].as_array().expect("rows").clone()
}

fn row<'a>(rows: &'a [Value], id: &str) -> &'a Value {
    rows.iter().find(|row| row["id"] == id).unwrap_or_else(|| panic!("no row {id}: {rows:#?}"))
}

fn lines_of(row: &Value) -> Vec<u64> {
    row["candidates"].as_array().unwrap().iter().map(|c| c["line"].as_u64().unwrap()).collect()
}

#[test]
fn the_plan_parses_the_whole_hansen_reference_list_and_indexes_the_citing_lines() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "hansen2021demand", "The Demand for Executive Skills");
    let before = mcp.revision();
    let dir = tempfile::tempdir().unwrap();
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
    let out = zt.digest(&["cited", "-Step", "plan", "-Staging", staging.to_str().unwrap()]);
    let line = last_json(&out);
    assert_eq!(line["ok"], json!(true), "{line}");
    assert_eq!(line["rows"], json!(35), "{line}");
    assert_eq!(line["unparsed"], Value::Null, "every Hansen entry splits: {line}");
    assert_eq!(mcp.revision(), before, "the plan writes no Card");

    let rows = plan_rows(&staging);
    assert_eq!(rows.len(), 35);
    assert_eq!(rows[0]["row"], json!(1));
    assert_eq!(rows[0]["id"], json!("adams2018"));
    assert_eq!(rows[0]["surname"], json!("Adams"));
    assert_eq!(rows[0]["title"], json!("Director skill sets"));
    let ids: std::collections::HashSet<_> = rows.iter().map(|row| row["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), 35, "the reference ids are unique");
    assert!(ids.contains("bandiera2015") && ids.contains("bandiera2020"));
    assert!(ids.contains("custodio2013a") && ids.contains("custodio2013b"), "{ids:?}");

    let deming = row(&rows, "deming2017");
    assert_eq!(deming["year"], json!("2017"));
    let lines = lines_of(deming);
    for expected in [13, 14, 16, 46, 47, 48] {
        assert!(lines.contains(&expected), "line {expected} cites Deming (2017): {lines:?}");
    }
    let first = &deming["candidates"][0];
    assert_eq!(first["heading"], json!("1 引言"));
    assert_eq!(first["anchor"], json!("hansen2021demand/1"));
    let later = deming["candidates"].as_array().unwrap().iter().find(|c| c["line"] == 46).unwrap();
    assert_eq!(later["anchor"], json!("hansen2021demand/9"));
    // Deming and Kahn (2018) is another work: the 2017 lines are not its candidates.
    assert!(!lines_of(row(&rows, "deming2018")).contains(&46));
}

#[test]
fn the_plan_reads_repeated_authors_comma_years_and_capital_surnames() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    for (key, file) in [("hart1995firms", "hart.md"), ("ebrill2001modern", "ebrill.md"), ("acemoglu2007equilibrium", "acemoglu.md")] {
        book(&mut mcp, key, key);
        let staging = staging(dir.path(), key, key, &fixture(file), json!([{ "anchor": format!("{key}/1"), "headings": ["1 引言", "1. 引言"] }]));
        let out = zt.digest(&["cited", "-Step", "plan", "-Staging", staging.to_str().unwrap()]);
        assert_eq!(last_json(&out)["ok"], json!(true), "{key}: {}", String::from_utf8_lossy(&out.stderr));
    }

    let hart = plan_rows(&dir.path().join("hart1995firms"));
    assert_eq!(hart.len(), 7);
    assert!(hart[1]["entry"].as_str().unwrap().starts_with("Adler, B. E. (1994)."), "{}", hart[1]);
    assert_eq!(hart[1]["id"], json!("adler1994"));
    assert!(hart[4]["entry"].as_str().unwrap().starts_with("Aghion, P., and Bolton, P. (1992)."), "{}", hart[4]);
    assert!(hart[5]["entry"].as_str().unwrap().starts_with("Aghion, P. and Tirole, J. (1994)."), "{}", hart[5]);
    assert_eq!(hart[6]["id"], json!("aghion1995"));
    assert!(lines_of(row(&hart, "aghion1992")).contains(&4));
    assert!(lines_of(row(&hart, "aghion1987")).is_empty());

    let ebrill = plan_rows(&dir.path().join("ebrill2001modern"));
    assert_eq!(ebrill.len(), 7);
    assert_eq!(ebrill[0]["id"], json!("abiorwerth1998"));
    assert_eq!(ebrill[5]["id"], json!("atkinson1980"));
    assert_eq!(ebrill[5]["title"], json!("Lectures on Public Economics"));
    assert!(ebrill[5]["entry"].as_str().unwrap().starts_with("Atkinson, Anthony B., and Joseph E. Stiglitz, 1980, Lectures"), "{}", ebrill[5]);

    let ecta = plan_rows(&dir.path().join("acemoglu2007equilibrium"));
    assert_eq!(ecta.len(), 8);
    assert_eq!(ecta[0]["id"], json!("acemoglu1998"));
    assert_eq!(ecta[0]["surname"], json!("Acemoglu"));
    assert!(!ecta[0]["entry"].as_str().unwrap().contains("[1374"), "page markers are cut: {}", ecta[0]);
    assert_eq!(ecta[2]["id"], json!("acemoglu2003a"));
    assert_eq!(ecta[3]["id"], json!("acemoglu2003b"));
    assert_eq!(ecta[7]["id"], json!("aghion1998"));
    assert_eq!(lines_of(&ecta[0]), vec![3, 4], "the English line and its translation");
}

/// Live run of the parser over the seven live books (2026-10-31): the Nobel lecture, the JPE paper and Becker et al.
/// use the Chicago style (`Surname, First. 2000. “Title.”`), Hart wraps a long entry onto a line that starts in
/// capitals, and Hart repeats its whole list in Chinese translation under the same heading.
#[test]
fn the_plan_reads_chicago_years_wrapped_entries_and_skips_a_translated_list() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "chicago2026", "Chicago");
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "chicago2026", "Chicago", &fixture("chicago.md"), json!([{ "anchor": "chicago2026/1", "headings": ["1 引言"] }]));
    let out = zt.digest(&["cited", "-Step", "plan", "-Staging", staging.to_str().unwrap()]);
    let line = last_json(&out);
    assert_eq!(line["rows"], json!(8), "{line}");
    assert_eq!(line["unparsed"], Value::Null, "every entry splits: {line}");
    let rows = plan_rows(&staging);
    let ids: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["abernathy2000", "acemoglu2003a", "acemoglu2003b", "acemoglu2002", "acemoglu2008", "arkolakis2012", "aumann1974", "aumann1994"], "{rows:#?}");
    assert_eq!(rows[0]["title"], json!("The Dynamics of Global Dominance: European Overseas Empires, 1415–1980"));
    assert!(rows[3]["entry"].as_str().unwrap().starts_with("Acemoglu, Daron. 2002. “Directed Technical Change.”"), "{}", rows[3]);
    assert_eq!(rows[3]["title"], json!("Directed Technical Change"));
    assert_eq!(rows[5]["surname"], json!("Arkolakis"));
    assert!(rows[7]["entry"].as_str().unwrap().starts_with("Aumann, Robert J., Gertner, R., and Scharfstein, D. (1994)."), "{}", rows[7]);
    assert_eq!(rows[7]["title"], json!("Anatomy of Financial Distress: An Examination of Junk-Bond Issuers"), "the wrapped line joins its entry");
    assert!(rows.iter().all(|r| !r["raw"].as_str().unwrap().contains("斯坦福")), "the translated list is no entry");
}

const SMALL: &str = "# 1 Introduction\n# 1 引言\nAs Smith (1999) argues, bargaining matters.\n\n# References\n\nSmith, J. (1999). Bargaining. Journal of Tests, 1:1–2.\\\nJones, K. Unpublished notes on contracts.\\\n\n# Appendix\n";

#[test]
fn an_entry_without_a_year_goes_to_a_checked_normalize_job_and_apply_writes_the_list_once() {
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

    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    assert_eq!(line["rows"], json!(2), "{line}");
    assert_eq!(line["unparsed"], json!(1), "{line}");
    assert_eq!(line["jobs"], json!(1), "{line}");
    let jobs: Value = serde_json::from_str(&std::fs::read_to_string(line["file"].as_str().unwrap()).unwrap()).unwrap();
    assert_eq!(jobs.as_array().unwrap().len(), 1);
    assert!(jobs[0]["files"].as_array().unwrap().iter().all(|f| Path::new(f.as_str().unwrap()).is_file()), "{jobs}");

    // The normalize answer is accepted only when its surname and year stand in the raw entry.
    let answer = staging.join("cited").join("normalize").join("out").join("n01.json");
    std::fs::create_dir_all(answer.parent().unwrap()).unwrap();
    std::fs::write(&answer, json!([{ "row": 2, "authors": "Brown, K.", "year": "", "title": "Unpublished notes on contracts", "venue": "" }]).to_string()).unwrap();
    let check = zt.digest(&["cited", "-Step", "precheck", "-Staging", s, "-Draft", answer.to_str().unwrap()]);
    let text = String::from_utf8_lossy(&check.stdout).to_string();
    assert!(text.trim_end().ends_with("faults: 1"), "{text}");
    std::fs::write(&answer, json!([{ "row": 2, "authors": "Jones, K.", "year": "", "title": "Unpublished notes on contracts", "venue": "" }]).to_string()).unwrap();
    let check = zt.digest(&["cited", "-Step", "precheck", "-Staging", s, "-Draft", answer.to_str().unwrap()]);
    assert!(String::from_utf8_lossy(&check.stdout).trim_end().ends_with("faults: 0"));

    let applied = last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["ok"], json!(true), "{applied}");
    assert_eq!(applied["card"], json!("small2026/1|b"), "{applied}");
    assert_eq!(applied["rows"], json!(2));
    let cited = mcp.call("get_cited", json!({ "at": "small2026" }));
    assert_eq!(cited["list"], json!("small2026/1|b"));
    assert_eq!(cited["rows"][0]["year"], json!("1999"));
    assert_eq!(cited["rows"][1]["entry"], json!("Jones, K. Unpublished notes on contracts."));
    let card = mcp.call("get_card", json!({ "at": "small2026/1|b" }));
    assert_eq!(card["type"], json!("cited"));
    assert_eq!(card["title"], json!("参考文献：《Small Book》引用的 2 篇文献"));

    let revision = mcp.revision();
    let again = last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(again["card"], json!("small2026/1|b"));
    assert_eq!(mcp.revision(), revision, "a second apply changes nothing");
}
