//! The Cited Card seam (#220): `zt-digest cited-cards -Step plan|jobs|precheck|check|apply` over a
//! real scratch archive. The plan groups every row of the Cited list into writer jobs that name the
//! whole source; the precheck renders a draft and applies C3 (the source lines) and the zt-check
//! rules; apply publishes the ready prefix as the list's Side successors in row order.
//!
//! The test needs a `zt` binary built from this repository (or on PATH). `ZT_TEST_ZT` overrides it.

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


fn book(mcp: &mut Mcp, key: &str, title: &str) {
    mcp.call("create_literature", json!({ "bibtex": format!("@article{{{key}, title={{{title}}}}}") }));
    mcp.call(
        "create_successor",
        json!({ "at": key, "title": "1 引言", "body": format!("引言。\n\n来源：[[{key}]] §1") }),
    );
}





/// Three works; Smith (1999) is cited twice, Jones (2001) once, Brown (2005) only in the list.
const SOURCE: &str = "# 1 Introduction\n# 1 引言\nAs Smith (1999) argues, bargaining matters.\n正如 Smith（1999）所说，谈判很重要。\n# 2 Model\n# 2 模型\nWe extend the contract model of Jones (2001).\n本书推广了 Jones（2001）的契约模型。\nThis line names no work.\n\n# References\n\nSmith, J. (1999). Bargaining. Journal of Tests, 1:1–2.\\\nJones, K. (2001). Contracts and Control. Review of Tests, 2:3–4.\\\nBrown, L. (2005). A Listed Work. Press.\\\n\n# Appendix\n";

/// A prepared book: its Literature tree, a Term list, the staging, the Cited list (plan + apply).
fn prepared(zt: &Zt, mcp: &mut Mcp, dir: &Path) -> PathBuf {
    book(mcp, "tiny2026", "Tiny Book");
    mcp.call("create_successor", json!({ "at": "tiny2026/1", "title": "2 模型", "body": "模型。\n\n来源：[[tiny2026]] §2" }));
    mcp.call(
        "create_successor",
        json!({ "at": "tiny2026/1", "kind": "side", "type": "term", "title": "术语表：《Tiny Book》的 1 个术语",
                "body": "本卡是术语表。\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 契约 | 合约 | 双方约定的权利 | §2 |\n\n来源：[[tiny2026]]" }),
    );
    let staging = staging(
        dir,
        "tiny2026",
        "Tiny Book",
        SOURCE,
        json!([
            { "anchor": "tiny2026/1", "headings": ["1 引言"] },
            { "anchor": "tiny2026/2", "headings": ["2 模型"] },
        ]),
    );
    let s = staging.to_str().unwrap();
    assert_eq!(last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]))["rows"], json!(3));
    assert_eq!(last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]))["card"], json!("tiny2026/1|b"));
    staging
}

fn draft_path(staging: &Path, job: &str) -> PathBuf {
    staging.join("cited-cards").join("drafts").join(format!("{job}.json"))
}

fn smith() -> Value {
    json!({
        "row": 1,
        "short_title": "谈判的作用",
        "why": "本书在引言中引用这篇文献，用来说明谈判很重要。",
        "role": "这篇文献给出了本书讨论谈判的出发点。",
        "relationship": "本书依赖它关于谈判的结论。",
        "relations": ["依赖"],
        "locations": [{ "line": 3, "use": "本书用它说明谈判很重要。" }, { "line": 4, "use": "中文译文中的同一处引用。" }],
    })
}

fn jones() -> Value {
    json!({
        "row": 2,
        "short_title": "契约与控制",
        "why": "本书在模型一节引用这篇文献，因为本书的模型以它的契约模型为起点。",
        "role": "这篇文献提供了本书模型的基本设定。",
        "relationship": "本书推广了它的契约模型。",
        "relations": ["推广"],
        "locations": [{ "line": 7, "use": "本书推广它的契约模型。" }],
    })
}

fn brown() -> Value {
    json!({
        "row": 3,
        "short_title": "列出的文献",
        "why": "本书只在参考文献中列出这篇文献，正文没有引用它。",
        "role": "这篇文献在本书正文中没有作用。",
        "relationship": "本书与它只有背景关系。",
        "relations": ["背景"],
        "locations": [],
    })
}

fn precheck(zt: &Zt, staging: &Path, draft: &Path) -> String {
    let out = zt.digest(&["cited-cards", "-Step", "precheck", "-Staging", staging.to_str().unwrap(), "-Draft", draft.to_str().unwrap()]);
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn the_plan_groups_every_row_into_jobs_that_name_the_whole_source_and_writes_no_card() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    let before = mcp.revision();
    let line = last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s, "-PerJob", "2"]));
    assert_eq!(line["ok"], json!(true), "{line}");
    assert_eq!(line["jobs"], json!(2), "three rows in jobs of two: {line}");
    assert_eq!(mcp.revision(), before, "the plan writes no Card");

    let jobs = last_json(&zt.digest(&["cited-cards", "-Step", "jobs", "-Staging", s]));
    let jobs = jobs["jobs"].as_array().unwrap().clone();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0]["id"], json!("t001"));
    assert_eq!(jobs[0]["model"], json!("deepseek-official/deepseek-flash"));
    let files: Vec<String> = jobs[0]["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
    assert!(files.iter().all(|f| Path::new(f).is_file()), "{files:?}");
    let job = std::fs::read_to_string(&files[1]).unwrap();
    assert!(job.contains(&staging.join("book.md").to_string_lossy().to_string()), "the job names the whole source:\n{job}");
    assert!(job.contains("Smith, J. (1999). Bargaining."), "{job}");
    assert!(job.contains("第 3 行"), "the candidate lines: {job}");
    assert!(job.contains("SELF-CHECK: zt-digest cited-cards -Step precheck"), "{job}");
    assert!(job.contains("契约"), "the Term list spellings: {job}");
}

#[test]
fn the_precheck_reads_the_source_lines_the_relation_words_and_a_listed_only_work() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s, "-PerJob", "4"]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();

    std::fs::write(&draft, json!([smith(), jones(), brown()]).to_string()).unwrap();
    let text = precheck(&zt, &staging, &draft);
    assert!(text.trim_end().ends_with("faults: 0"), "{text}");

    // C3: line 9 names no work.
    let mut wrong = smith();
    wrong["locations"] = json!([{ "line": 9, "use": "不存在的引用。" }]);
    std::fs::write(&draft, json!([wrong]).to_string()).unwrap();
    let text = precheck(&zt, &staging, &draft);
    assert!(text.contains("row1 C3"), "{text}");
    // C2: a word outside the set.
    let mut wrong = jones();
    wrong["relations"] = json!(["补充"]);
    std::fs::write(&draft, json!([wrong]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row2 C2"));
    // A work with no in-text location takes only 背景.
    let mut wrong = brown();
    wrong["relations"] = json!(["依赖"]);
    wrong["relationship"] = json!("本书依赖它。");
    std::fs::write(&draft, json!([wrong]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row3 C3"));
    // R6: a word the Term list avoids.
    let mut wrong = jones();
    wrong["relationship"] = json!("本书推广了它的合约模型。");
    std::fs::write(&draft, json!([wrong]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row2 R6"));
}

#[test]
fn apply_publishes_the_ready_prefix_in_row_order_and_fills_the_list_columns() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s, "-PerJob", "2"]));
    let first = draft_path(&staging, "t001");
    std::fs::create_dir_all(first.parent().unwrap()).unwrap();
    std::fs::write(&first, json!([smith(), jones()]).to_string()).unwrap();

    let check = last_json(&zt.digest(&["cited-cards", "-Step", "check", "-Staging", s]));
    assert_eq!(check["ready"], json!(2), "{check}");
    assert_eq!(check["not_ready"], json!(1), "{check}");
    let applied = last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["ok"], json!(true), "{applied}");
    assert_eq!(applied["committed"], json!(2), "{applied}");
    assert_eq!(applied["complete"], Value::Null, "{applied}");

    let cited = mcp.call("get_cited", json!({ "at": "tiny2026" }));
    assert_eq!(cited["cards"][0]["address"], json!("tiny2026/1|b|a"), "{cited}");
    assert_eq!(cited["cards"][1]["address"], json!("tiny2026/1|b|b"));
    assert_eq!(cited["rows"][1]["relation"], json!("推广"));
    assert_eq!(cited["rows"][0]["cited_at"], json!("1 引言（2 处）"), "{cited}");
    assert_eq!(cited["faults"], json!([{ "fault": "cited_row_without_card", "row": 3, "entry": "Brown, L. (2005). A Listed Work. Press." }]));
    let card = mcp.call("get_card", json!({ "at": "tiny2026/1|b|b" }));
    assert_eq!(card["type"], json!("cited"));
    assert_eq!(card["title"], json!("Jones（2001）：契约与控制"));
    let body = card["body"].as_str().unwrap();
    assert!(body.starts_with("文献：Jones, K. (2001). Contracts and Control. Review of Tests, 2:3–4."), "{body}");
    assert!(body.contains("另见 [[tiny2026/2]]"), "{body}");
    assert!(body.contains("来源：[[tiny2026]] 参考文献第 2 条"), "{body}");

    let revision = mcp.revision();
    last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(mcp.revision(), revision, "a second apply changes nothing");

    std::fs::write(draft_path(&staging, "t002"), json!([brown()]).to_string()).unwrap();
    last_json(&zt.digest(&["cited-cards", "-Step", "check", "-Staging", s]));
    let applied = last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["complete"], json!(true), "{applied}");
    let cited = mcp.call("get_cited", json!({ "at": "tiny2026" }));
    assert_eq!(cited["faults"], json!([]), "{cited}");
    assert_eq!(cited["rows"][2]["cited_at"], json!("仅参考文献"));
    let report: Value = serde_json::from_str(&std::fs::read_to_string(staging.join("cited-cards").join("report.json")).unwrap()).unwrap();
    assert_eq!(report["kind"], json!("cited_cards_report"));
    assert_eq!(report["complete"], json!(true));
}

/// Live backfill of acemoglu2025nobel (2026-10-31): the text writes \`Jaszi 1929\` for \`Jászi\`, cites a work by
/// its authors' initials (\`AJR (2001, 2012)\`), and a \`Forthcoming\` entry got its authors from the normalize job.
const NOBEL: &str = "# 1 Introduction\n# 1 引言\nThe elites resisted, as Jaszi 1929 (p. 80) records.\nThis line names no work.\nThe 2SLS results are robust, as discussed at length in AJR (2001, 2012).\nThis line names no work either.\nSee Acemoglu and Robinson (forthcoming) for a framework.\n\n# References\n\nAcemoglu, Daron, Simon Johnson, and James A. Robinson. 2012. “The Colonial Origins of Comparative Development: An Empirical Investigation: Reply.” American Economic Review 102 (6): 3077–3110.\\\nAcemoglu, Daron, and James A. Robinson. Forthcoming. “Culture, Institutions and Social Equilibria: A Framework.” Journal of Economic Literature.\\\nJászi, Oscar. 1929. The Dissolution of the Habsburg Monarchy. University of Chicago Press.\\\n\n# Appendix\n";

#[test]
fn the_precheck_reads_unaccented_names_initials_and_normalized_entries() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "nobel2026", "Nobel");
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "nobel2026", "Nobel", NOBEL, json!([{ "anchor": "nobel2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    let line = last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    assert_eq!(line["unparsed"], json!(1), "{line}");
    let answer = staging.join("cited").join("normalize").join("out").join("n01.json");
    std::fs::write(&answer, json!([{ "row": 2, "authors": "Acemoglu, Daron, and James A. Robinson", "year": "", "title": "Culture, Institutions and Social Equilibria: A Framework", "venue": "Journal of Economic Literature" }]).to_string()).unwrap();
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |row: u64, line: u64, alias: Option<&str>| {
        let mut location = json!({ "line": line, "use": "本书在这里引用它。" });
        if let Some(alias) = alias { location["as"] = json!(alias); }
        json!({ "row": row, "short_title": "文献", "why": "本书引用它说明一个事实。", "role": "它提供了一个事实。", "relationship": "本书依赖它。", "relations": ["依赖"], "locations": [location] })
    };
    std::fs::write(&draft, json!([item(1, 5, Some("AJR")), item(2, 7, None), item(3, 3, None)]).to_string()).unwrap();
    let text = precheck(&zt, &staging, &draft);
    assert!(text.trim_end().ends_with("faults: 0"), "{text}");

    // An alias is checked: it must stand in the line with the year, and start like the surname.
    std::fs::write(&draft, json!([item(1, 5, Some("XYZ"))]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"));
    std::fs::write(&draft, json!([item(1, 3, Some("AJR"))]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"));
}
