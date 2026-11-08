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
    cited_read::answer_parts(&staging, &cited_read::default_fields);
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
fn a_new_job_size_keeps_the_written_drafts() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    // morony2005iraq, 2026-11-06: the job size doubled at job 145 of 396. A job with a draft keeps its rows;
    // the rest are grouped at the new size under ids no job used, and a further plan changes nothing.
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s, "-PerJob", "1"]));
    std::fs::write(draft_path(&staging, "t002"), json!([jones()]).to_string()).unwrap();
    let line = last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    assert_eq!(line["revision"], json!(2));
    let plan: Value = serde_json::from_str(&std::fs::read_to_string(staging.join("cited-cards").join("plan.json")).unwrap()).unwrap();
    assert_eq!(plan["per_job"], json!(12));
    assert_eq!(plan["jobs"], json!([{"id":"t002","rows":[2]},{"id":"t004","rows":[1,3]}]));
    let again = last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    assert_eq!(again["revision"], json!(2));
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
    // 2026-11-08 (docs/digest-job-messages-proposal.md): a job is one message, and up to four jobs run as one
    // conversation; the second job is the first conversation's "then" message.
    assert_eq!(jobs.len(), 1, "two jobs, one conversation: {jobs:?}");
    assert_eq!(jobs[0]["id"], json!("t001"));
    let then = jobs[0]["then"].as_array().expect("a later message").clone();
    assert_eq!(then.len(), 1);
    assert_eq!(then[0]["id"], json!("t002"));
    assert_eq!(then[0]["answer"], json!(draft_path(&staging, "t002").to_string_lossy().to_string()));
    let later: Vec<String> = then[0]["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
    assert_eq!(later.len(), 1, "a later message reads only its own job: {later:?}");
    let next = std::fs::read_to_string(&later[0]).unwrap();
    assert!(next.contains("Brown") || next.contains("第 3 条"), "the later message holds its own work:\n{next}");
    assert!(next.contains("SELF-CHECK: zt-digest cited-cards -Step precheck") && next.contains("t002"), "{next}");
    assert!(next.contains("第一条消息"), "it points to the shared material:\n{next}");
    assert!(!next.contains("契约"), "the Term list travels once, in the first message:\n{next}");
    let default = last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    assert_eq!(default["ok"], json!(true), "{default}");
    let frozen: Value = serde_json::from_str(&std::fs::read_to_string(staging.join("cited-cards").join("plan.json")).unwrap()).unwrap();
    assert_eq!(frozen["per_job"], json!(12), "a job holds twelve works by default");
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
    let before = mcp.revision();
    let applied = last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["ok"], json!(true), "{applied}");
    assert_eq!(applied["committed"], json!(2), "{applied}");
    assert_eq!(applied["complete"], Value::Null, "{applied}");
    // atkinson2015lectures (2026-11-02): 1,589 rows, one write call per Card at about 5 s each. The new
    // Cards go in identified batches: one transaction for both, one for the list's cells.
    assert!(mcp.revision() - before <= 2, "the new Cards are one batch: {} transactions", mcp.revision() - before);

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
/// its authors' initials (\`AJR (2001, 2012)\`), and a \`Forthcoming\` entry has no year.
const NOBEL: &str = "# 1 Introduction\n# 1 引言\nThe elites resisted, as Jaszi 1929 (p. 80) records.\nThis line names no work.\nThe 2SLS results are robust, as discussed at length in AJR (2001, 2012).\nThis line names no work either.\nSee Acemoglu and Robinson (forthcoming) for a framework.\nAs Salter (1966) showed, factor prices matter.\nThis example is drawn from Boycko *et al.* (1995: ch. 2).\nBoycko (1994) is another work.\n\n# References\n\nAcemoglu, Daron, Simon Johnson, and James A. Robinson. 2012. “The Colonial Origins of Comparative Development: An Empirical Investigation: Reply.” American Economic Review 102 (6): 3077–3110.\\\nAcemoglu, Daron, and James A. Robinson. Forthcoming. “Culture, Institutions and Social Equilibria: A Framework.” Journal of Economic Literature.\\\nJászi, Oscar. 1929. The Dissolution of the Habsburg Monarchy. University of Chicago Press.\\\nSalter W. E. G. 1966. Productivity and Technical Change. 2nd ed. Cambridge: Cambridge Univ. Press.\\\nBoyco, M., Shleifer, A., and Vishny, R. (1995). *Privatizing Russia*, Cambridge, Mass.: MIT Press.\\\n\n# Appendix\n";

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
    assert_eq!(line["rows"], json!(5), "{line}");
    cited_read::answer_parts(&staging, &|text: &str| {
        if text.contains("Forthcoming") {
            return Some(("Acemoglu, Daron, and James A. Robinson".into(), String::new(), "Culture, Institutions and Social Equilibria: A Framework".into()));
        }
        if text.starts_with("Boyco") {
            return Some(("Boyco, M., Shleifer, A., and Vishny, R.".into(), "1995".into(), "Privatizing Russia".into()));
        }
        cited_read::default_fields(text)
    });
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |row: u64, line: u64, alias: Option<&str>| {
        let mut location = json!({ "line": line, "use": "本书在这里引用它。" });
        if let Some(alias) = alias { location["as"] = json!(alias); }
        json!({ "row": row, "short_title": "文献", "why": "本书引用它说明一个事实。", "role": "它提供了一个事实。", "relationship": "本书依赖它。", "relations": ["依赖"], "locations": [location] })
    };
    // Live backfill of acemoglu2010labor: `Salter W. E. G. 1966.` has no comma after the surname.
    // Live backfill of hart1995firms: the list spells `Boyco`, the text `Boycko`; one letter apart, with the year.
    std::fs::write(&draft, json!([item(1, 5, Some("AJR")), item(2, 7, None), item(3, 3, None), item(4, 8, None), item(5, 9, None)]).to_string()).unwrap();
    let text = precheck(&zt, &staging, &draft);
    assert!(text.trim_end().ends_with("faults: 0"), "{text}");

    // An alias is checked: it must stand in the line with the year, and start like the surname.
    std::fs::write(&draft, json!([item(1, 5, Some("XYZ"))]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"));
    std::fs::write(&draft, json!([item(1, 3, Some("AJR"))]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"));
    // A spelling one letter apart needs the work's year in that line.
    std::fs::write(&draft, json!([item(5, 10, None)]).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row5 C3"));
}

/// Live backfill of grossman2002integration (2026-10-31): a parser fix shortened the list from 43 to 28 rows
/// after 32 Cited Cards were published. A complete publication removes the Cited Cards beyond the list's rows.
#[test]
fn a_complete_publication_removes_cited_cards_beyond_the_rows() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s, "-PerJob", "2"]));
    std::fs::write(draft_path(&staging, "t001"), json!([smith(), jones()]).to_string()).unwrap();
    std::fs::write(draft_path(&staging, "t002"), json!([brown()]).to_string()).unwrap();
    assert_eq!(last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]))["complete"], json!(true));
    assert_eq!(mcp.call("get_cited", json!({ "at": "tiny2026" }))["cards"].as_array().unwrap().len(), 3);

    // The source loses its last two entries: the list has one row, and two Cards go. Deleting a Side successor
    // renumbers the later ones (grossman2002integration: \`1|h|ae\` did not exist after \`1|h|ac\` went), so the
    // stale Cards go from the last one back.
    let source = std::fs::read_to_string(staging.join("book.md")).unwrap()
        .replace("Brown, L. (2005). A Listed Work. Press.\\\n", "")
        .replace("Jones, K. (2001). Contracts and Control. Review of Tests, 2:3–4.\\\n", "");
    std::fs::write(staging.join("book.md"), source).unwrap();
    assert_eq!(last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]))["rows"], json!(1));
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s, "-PerJob", "2"]));
    let applied = last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["complete"], json!(true), "{applied}");
    assert_eq!(applied["removed"], json!(2), "{applied}");
    let cited = mcp.call("get_cited", json!({ "at": "tiny2026" }));
    assert_eq!(cited["coverage"], json!({ "rows": 1, "cards": 1, "missing": 0 }), "{cited}");
    assert_eq!(cited["faults"], json!([]), "{cited}");
}

/// Live backfill of ebrill2001modern (2026-10-31): the reference itself holds a word the Term list avoids
/// (\`CVAT, VIVAT, and All That\`, where the list writes 补偿性增值税 and avoids CVAT). The \`文献：\` line quotes the
/// reference and is never reworded, so R6 does not count there; it still counts in the written sections.
#[test]
fn r6_leaves_the_quoted_reference_line_alone() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "vat2026", "VAT");
    mcp.call(
        "create_successor",
        json!({ "at": "vat2026/1", "kind": "side", "type": "term", "title": "术语表：《VAT》的 1 个术语",
                "body": "本卡是术语表。\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 补偿性增值税 | CVAT | 对跨州交易补征的增值税 | §1 |\n\n来源：[[vat2026]]" }),
    );
    let source = "# 1 Introduction\n# 1 引言\nAs Keen (2000) shows, federal systems need new forms.\n\n# References\n\nKeen, Michael, 2000, “CVAT, VIVAT, and All That: New Forms of VAT for Federal Systems,” Canadian Tax Journal, Vol. 48, pp. 409–24.\\\n\n# Appendix\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "vat2026", "VAT", source, json!([{ "anchor": "vat2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |relationship: &str| json!([{ "row": 1, "short_title": "联邦制下的新型增值税", "why": "本书引用它说明联邦制需要新型增值税。", "role": "它提供补偿性增值税的方案。", "relationship": relationship, "relations": ["依赖"], "locations": [{ "line": 3, "use": "本书用它说明联邦制。" }] }]);
    std::fs::write(&draft, item("本书依赖它的补偿性增值税方案。").to_string()).unwrap();
    let text = precheck(&zt, &staging, &draft);
    assert!(text.trim_end().ends_with("faults: 0"), "the quoted reference line is no R6 fault:\n{text}");
    std::fs::write(&draft, item("本书依赖它的 CVAT 方案。").to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 R6"), "a written section still counts");
}

/// atkinson2015lectures (2026-11-02): 16 Cited Cards stayed not ready because the book's own section titles
/// (`公司税的影响`, `资本课税的归宿`) hold a word the Term list avoids. The tool renders those titles in the
/// `引用处：` line and before each location; no writer can reword them, so R6 counts only the writer's text.
#[test]
fn r6_leaves_a_quoted_section_title_alone() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "tax2026", "Tax");
    mcp.call(
        "create_successor",
        json!({ "at": "tax2026/1", "kind": "side", "type": "term", "title": "术语表：《Tax》的 1 个术语",
                "body": "本卡是术语表。\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 公司利润税 | 公司税 | 对公司利润征收的税 | §1 |\n\n来源：[[tax2026]]" }),
    );
    let source = "# 1 Corporation Tax\n# 1 公司税的影响\nAs Keen (2000) shows, the tax falls on capital.\n\n# References\n\nKeen, Michael, 2000, “Taxing Profits,” Canadian Tax Journal, Vol. 48, pp. 409–24.\\n\n# Appendix\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "tax2026", "Tax", source, json!([{ "anchor": "tax2026/1", "headings": ["1 公司税的影响"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |used: &str| json!([{ "row": 1, "short_title": "利润的课税", "why": "本书引用它说明公司利润税落在资本上。", "role": "它给出公司利润税归宿的结论。", "relationship": "本书依赖它的结论。", "relations": ["依赖"], "locations": [{ "line": 3, "use": used }] }]);
    std::fs::write(&draft, item("本书用它说明公司利润税落在资本上。").to_string()).unwrap();
    let text = precheck(&zt, &staging, &draft);
    assert!(text.trim_end().ends_with("faults: 0"), "the quoted section title is no R6 fault:\n{text}");
    std::fs::write(&draft, item("本书用它说明公司税落在资本上。").to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 R6"), "the writer's own text still counts");
}

/// 2026-11-07: the agentic Cited list replaced the parser on every finished book. A new reading changes the
/// rows; each live Cited Card whose work the new list still holds is read back and carried (no writer job),
/// the old drafts move aside, and only a new work goes to a writer. The carried Card keeps its text.
#[test]
fn a_changed_list_carries_the_live_cards_of_the_works_it_keeps() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    std::fs::write(draft_path(&staging, "t001"), json!([smith(), jones(), brown()]).to_string()).unwrap();
    assert_eq!(last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]))["complete"], json!(true));
    // A Card of the tree links Smith's Cited Card; the Link must follow Smith when the rows move.
    let smith_card = mcp.call("get_cited", json!({ "at": "tiny2026" }))["cards"][0]["address"].as_str().unwrap().to_string();
    let note = mcp.call("create_successor", json!({ "at": "tiny2026/2", "title": "谈判的引用", "body": format!("本书引用 Smith 说明谈判，见 [[{smith_card}]]。\n\n来源：[[tiny2026]] §2") }));
    let note = note["address"].as_str().unwrap().to_string();

    let source = std::fs::read_to_string(staging.join("book.md")).unwrap().replace("Smith, J. (1999).", "Adams, B. (1990). Early Work. Press.\\\nSmith, J. (1999).");
    std::fs::write(staging.join("book.md"), source).unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    assert_eq!(last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]))["rows"], json!(4));
    let plan = last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    assert_eq!(plan["carried"], json!(1), "{plan}");
    assert!(staging.join("cited-cards").join("drafts-old-1").join("t001.json").is_file(), "the old drafts move aside");
    let jobs = last_json(&zt.digest(&["cited-cards", "-Step", "jobs", "-Staging", s]));
    let listed: Vec<&str> = jobs["jobs"].as_array().unwrap().iter().map(|j| j["id"].as_str().unwrap()).collect();
    assert_eq!(listed, ["t002"], "only the new work goes to a writer: {jobs}");
    let mut adams = brown();
    adams["row"] = json!(1);
    std::fs::write(draft_path(&staging, "t002"), json!([adams]).to_string()).unwrap();
    let applied = last_json(&zt.digest(&["cited-cards", "-Step", "apply", "-Staging", s, "-Apply"]));
    assert_eq!(applied["complete"], json!(true), "{applied}");
    let cited = mcp.call("get_cited", json!({ "at": "tiny2026" }));
    assert_eq!(cited["coverage"], json!({ "rows": 4, "cards": 4, "missing": 0 }), "{cited}");
    assert_eq!(cited["faults"], json!([]), "{cited}");
    let second = mcp.call("get_card", json!({ "at": cited["cards"][1]["address"] }));
    assert!(second["title"].as_str().unwrap().contains("谈判的作用"), "{second}");
    assert!(second["body"].as_str().unwrap().contains("本书依赖它关于谈判的结论。"), "{second}");
    assert_eq!(applied["relinked"], json!(1), "{applied}");
    let linked = mcp.call("get_card", json!({ "at": note }));
    let smith_now = cited["cards"][1]["address"].as_str().unwrap();
    assert!(linked["body"].as_str().unwrap().contains(&format!("[[{smith_now}]]")), "the Link follows Smith to {smith_now}: {linked}");
}

/// Live backfill of hoyland2015path (2026-11-07): an anonymous work has no surname and one long title word
/// (`Chronicle of 741`); the text names it with that word and its number, which counts as a title word.
#[test]
fn an_anonymous_work_is_named_by_its_title_word_and_number() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "anon2026", "Anon");
    let source = "# 1 Introduction\n# 1 引言\nThe Chronicle of 741, §15, records the battle.\nThe Chronicle of 754 is another work.\n\n# Bibliography\n\nChronicle of 741 trans. in Hoyland, Seeing Islam, Excursus B.\\n\n# Appendix\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "anon2026", "Anon", source, json!([{ "anchor": "anon2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &|_| Some((String::new(), String::new(), "Chronicle of 741".into())));
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |line: u64| json!([{ "row": 1, "short_title": "741 年编年史", "why": "本书引用它记述战役。", "role": "它提供战役的记载。", "relationship": "本书依赖它的记载。", "relations": ["依赖"], "locations": [{ "line": line, "use": "本书用它记述战役。" }] }]);
    std::fs::write(&draft, item(3).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).trim_end().ends_with("faults: 0"), "the title word and its number name the work");
    std::fs::write(&draft, item(6).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"), "another number names another work");
}

/// Live backfill of morony2005iraq (2026-11-07): an author index lists later works of an author without the
/// author (`“Djaysh,” 582`); the text cites such a work by its one title word, which then names it.
#[test]
fn a_work_without_a_surname_and_a_one_word_title_is_named_by_that_word() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "index2026", "Index");
    let source = "# 1 Introduction\n# 1 引言\nAccording to Abū Yūsuf (Kharāj, pp. 27, 29) the share is fixed.\n\nOther text.\nNo work here.\n\n# AUTHOR INDEX\n\nKharāj, 37, 585\\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "index2026", "Index", source, json!([{ "anchor": "index2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s, "-Heading", "AUTHOR INDEX"]));
    cited_read::answer_parts(&staging, &|_| Some((String::new(), String::new(), "Kharāj".into())));
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |line: u64| json!([{ "row": 1, "short_title": "税收之书", "why": "本书引用它说明战利品的分配。", "role": "它提供分配的规则。", "relationship": "本书依赖它的规则。", "relations": ["依赖"], "locations": [{ "line": line, "use": "本书用它说明分配。" }] }]);
    std::fs::write(&draft, item(3).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).trim_end().ends_with("faults: 0"), "the one title word names the work");
    std::fs::write(&draft, item(6).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"), "a line without the word does not");
}

/// Live backfill of morony2005iraq (2026-11-07): the list was read again after its writers ran and before any Card
/// was published. The drafts of the previous plan are carried by their entries, so no writer runs again.
#[test]
fn a_list_read_again_before_publication_carries_the_written_drafts() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().unwrap();
    let staging = prepared(&zt, &mut mcp, dir.path());
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    std::fs::write(draft_path(&staging, "t001"), json!([smith(), jones(), brown()]).to_string()).unwrap();

    let source = std::fs::read_to_string(staging.join("book.md")).unwrap().replace("Smith, J. (1999).", "Adams, B. (1990). Early Work. Press.\\\nSmith, J. (1999).");
    std::fs::write(staging.join("book.md"), source).unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &cited_read::default_fields);
    assert_eq!(last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]))["rows"], json!(4));
    assert!(mcp.call("get_cited", json!({ "at": "tiny2026" }))["cards"].as_array().unwrap().is_empty(), "nothing is published yet");
    let plan = last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    assert_eq!(plan["carried"], json!(1), "{plan}");
    let jobs = last_json(&zt.digest(&["cited-cards", "-Step", "jobs", "-Staging", s]));
    let listed: Vec<&str> = jobs["jobs"].as_array().unwrap().iter().map(|j| j["id"].as_str().unwrap()).collect();
    assert_eq!(listed, ["t002"], "only the new work goes to a writer: {jobs}");
}

/// Live backfill of scheiner2022baghdad (2026-11-07): the list writes `al-Sharaf, M.`, the notes `Sharaf,
/// *al-Taṣawwuf*`; a surname is also found without its Arabic article.
#[test]
fn an_arabic_surname_is_found_without_its_article() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "sharaf2026", "Sharaf");
    let source = "# 1 Introduction\n# 1 引言\nSome scholars claim this (Sharaf, al-Taṣawwuf, 136).\n\nOther text.\nNo work here.\n\n# Bibliography\n\nal-Sharaf, M., al-Taṣawwuf al-islāmī fī madrasat Baghdād, Alexandria 1972.\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "sharaf2026", "Sharaf", source, json!([{ "anchor": "sharaf2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &|_| Some(("al-Sharaf, M.".into(), "1972".into(), "al-Taṣawwuf al-islāmī fī madrasat Baghdād".into())));
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |line: u64| json!([{ "row": 1, "short_title": "巴格达学派的苏非主义", "why": "本书引用它说明一种看法。", "role": "它提供这种看法。", "relationship": "本书对照它的看法。", "relations": ["对照"], "locations": [{ "line": line, "use": "本书用它说明看法。" }] }]);
    std::fs::write(&draft, item(3).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).trim_end().ends_with("faults: 0"), "Sharaf names al-Sharaf");
    std::fs::write(&draft, item(6).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"));
}

/// Live run of sijpesteijn2013shaping (2026-11-08): the list writes `Ibn Yūnus al-Miṣrī (d. 347/958), Ta'rīkh
/// al-Miṣriyyīn`, the notes `Ibn Yūnus, Ta'rīkh, I, 269`; an Arabic name opening on Ibn or Abū is found by its
/// first two words.
#[test]
fn an_arabic_name_is_found_by_its_first_two_words() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "yunus2026", "Yunus");
    let source = "# 1 Introduction\n# 1 引言\nThe taxes were kept (Ibn Yūnus, Ta'rīkh, I, 269-70).\n\nOther text.\nAnother ruler (Ibn Sa'd, Ṭabaqāt, 12).\n\n# Bibliography\n\nIbn Yūnus al-Miṣrī (d. 347/958), Ta'rīkh al-Miṣriyyīn. Ed. 'Abd al-Faṭṭāḥ, Beirut, 1421/2000.\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "yunus2026", "Yunus", source, json!([{ "anchor": "yunus2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &|_| Some(("Ibn Yūnus al-Miṣrī".into(), "1421/2000".into(), "Ta'rīkh al-Miṣriyyīn".into())));
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |line: u64| json!([{ "row": 1, "short_title": "埃及人史", "why": "本书引用它说明税收。", "role": "它提供税收的记载。", "relationship": "本书依赖它的记载。", "relations": ["依赖"], "locations": [{ "line": line, "use": "本书用它说明税收。" }] }]);
    std::fs::write(&draft, item(3).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).trim_end().ends_with("faults: 0"), "Ibn Yūnus names Ibn Yūnus al-Miṣrī");
    std::fs::write(&draft, item(6).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"), "Ibn Sa'd names another author");
}

/// Live run of sijpesteijn2013shaping (2026-11-08): the list writes `Ibn al-Biṭrīq (d. 328/940), Das Annalenwerk
/// des Eutychios von Alexandrien`, the notes `Eutychius, Annalenwerk, 120`; a long title word spelled one letter
/// apart counts as a title word.
#[test]
fn a_long_title_word_spelled_one_letter_apart_names_the_work() {
    let Some(program) = zt_program() else { return };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    book(&mut mcp, "eutychius2026", "Eutychius");
    let source = "# 1 Introduction\n# 1 引言\nOr six months (Eutychius, Annalenwerk, 122, § 283).\n\nOther text.\nThe Annalenwerk is a genre of its own.\n\n# Bibliography\n\nIbn al-Biṭrīq (d. 328/940), Das Annalenwerk des Eutychios von Alexandrien. Tr. M. Breydy, Louvain, 1985.\n";
    let dir = tempfile::tempdir().unwrap();
    let staging = staging(dir.path(), "eutychius2026", "Eutychius", source, json!([{ "anchor": "eutychius2026/1", "headings": ["1 引言"] }]));
    let s = staging.to_str().unwrap();
    last_json(&zt.digest(&["cited", "-Step", "plan", "-Staging", s]));
    cited_read::answer_parts(&staging, &|_| Some(("Ibn al-Biṭrīq".into(), "1985".into(), "Das Annalenwerk des Eutychios von Alexandrien".into())));
    last_json(&zt.digest(&["cited", "-Step", "apply", "-Staging", s, "-Apply"]));
    last_json(&zt.digest(&["cited-cards", "-Step", "plan", "-Staging", s]));
    let draft = draft_path(&staging, "t001");
    std::fs::create_dir_all(draft.parent().unwrap()).unwrap();
    let item = |line: u64| json!([{ "row": 1, "short_title": "欧提基乌斯编年史", "why": "本书引用它核对征服事件。", "role": "它提供围城的时长。", "relationship": "本书对照它的记载。", "relations": ["对照"], "locations": [{ "line": line, "use": "本书用它说明围城的时长。" }] }]);
    std::fs::write(&draft, item(3).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).trim_end().ends_with("faults: 0"), "Eutychius, Annalenwerk names the work");
    std::fs::write(&draft, item(6).to_string()).unwrap();
    assert!(precheck(&zt, &staging, &draft).contains("row1 C3"), "one title word alone does not");
}
