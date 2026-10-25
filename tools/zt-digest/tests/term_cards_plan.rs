//! The Term-card plan/jobs seam (S18, #159): the commands run over a real scratch archive and a
//! staged source fixture. The plan reads the book's existing list by role (a Term list, or a
//! pre-term book's legacy `术语表` table), freezes the rows and source evidence, groups the writer
//! jobs, and writes no Card. The archive revision and the list's authored text are the proof.
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

fn staging(parent: &Path, name: &str, key: &str, title: &str, sections: &str) -> PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(dir.join("sections")).expect("make sections");
    std::fs::write(
        dir.join("manifest.json"),
        json!({
            "citation_key": key,
            "book_title": title,
            "staging": dir.to_string_lossy(),
            "source_sha256": "0".repeat(64),
        })
        .to_string(),
    )
    .expect("write manifest");
    std::fs::write(dir.join("sections").join("s01.md"), sections).expect("write section");
    dir
}

const TABLE: &str = "本卡是《Test Book》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 替代弹性 | 替代程度 | 两种要素相互替代的难易 | §1 |
| 剩余控制权 | 残余控制权 | 对资产用途的决定权 | §2 |

来源：[[testbook2026]]";

const SECTIONS: &str = "替代弹性定义为 $\\sigma=-\\frac{d\\ln(Z/L)}{d\\ln(w_Z/w_L)}$。\n剩余控制权是指对资产用途的决定权。\n";

fn last_json(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("no output: {}", String::from_utf8_lossy(&output.stderr)));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"))
}

fn plan_json(staging: &Path) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(staging.join("term-cards").join("plan.json")).expect("plan.json"),
    )
    .expect("plan JSON")
}

#[test]
fn the_term_cards_plan_reads_a_real_list_and_writes_no_card() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    // A pre-term book: the Terms stage wrote a legacy `术语表` table as an Idea Card.
    let root = mcp.call(
        "create_literature",
        json!({"bibtex":"@book{testbook2026, title={Test Book}}","body":"一本测试书。"}),
    )["address"]
        .as_str()
        .expect("citation key")
        .to_string();
    assert_eq!(root, "testbook2026");
    mcp.call(
        "create_successor",
        json!({"at":root,"kind":"direct","title":"本书的结构","body":"锚点。"}),
    );
    let list = mcp.call(
        "create_successor",
        json!({"at":format!("{root}/1"),"kind":"side","title":"术语表：《Test Book》的 2 个术语","body":TABLE}),
    )["address"]
        .as_str()
        .expect("list address")
        .to_string();
    assert_eq!(list, "testbook2026/1|a");
    let revision = mcp.revision();
    let authored = mcp.call("get_card", json!({"at":list}));

    let work = tempfile::tempdir().expect("work");
    let staged = staging(work.path(), "book", &root, "Test Book", SECTIONS);
    let stage = staged.to_str().unwrap();

    // --- plan: the legacy table is adapted and the source evidence is frozen -------------------
    let plan = zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]);
    assert!(
        plan.status.success(),
        "plan: {} {}",
        String::from_utf8_lossy(&plan.stdout),
        String::from_utf8_lossy(&plan.stderr)
    );
    let said = last_json(&plan);
    assert_eq!(said["ok"], true);
    assert_eq!(said["book"], root);
    assert_eq!(said["revision"], 1);
    assert_eq!(said["terms"], 2);
    assert_eq!(said["math"], 1);
    assert_eq!(said["concept"], 1);
    assert_eq!(said["units"], Value::Null);
    assert_eq!(said["jobs"][0]["id"], "t001");
    assert_eq!(said["jobs"][0]["kind"], "math");
    assert_eq!(said["jobs"][1]["id"], "t002");
    assert_eq!(said["jobs"][1]["kind"], "concept");
    let frozen = plan_json(&staged);
    assert_eq!(frozen["list"]["legacy"], true);
    assert_eq!(frozen["list"]["address"], list);
    assert_eq!(frozen["list"]["row_count"], 2);
    assert_eq!(frozen["rows"][0]["term"], "替代弹性");
    assert_eq!(frozen["rows"][0]["defined"], "§1");
    assert_eq!(frozen["rows"][0]["snippets"][0]["file"], "sections/s01.md");
    assert_eq!(frozen["rows"][0]["snippets"][0]["defines"], true);
    assert_eq!(frozen["rows"][1]["classification"]["kind"], "concept");
    assert_eq!(frozen["rows"][1]["neighbors"][0]["term"], "替代弹性");
    assert_eq!(frozen["source"]["sections"][0]["file"], "sections/s01.md");
    assert!(staged.join("term-cards").join("jobs").join("t001.md").is_file());
    assert!(staged.join("term-cards").join("jobs").join("t002.md").is_file());
    assert!(staged.join("term-cards").join("out").is_dir());
    assert!(staged.join("term-cards").join("check").is_dir());
    assert!(staged.join("term-cards").join("review").is_dir());
    let state: Value = serde_json::from_str(
        &std::fs::read_to_string(staged.join("term-cards").join("state.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(state["plan_revision"], 1);
    assert_eq!(state["jobs"]["t001"]["state"], "planned");

    // Planning wrote no Card and advanced no archive revision.
    assert_eq!(mcp.revision(), revision, "planning wrote to the archive");
    let after = mcp.call("get_card", json!({"at":list}));
    assert_eq!(after["text"], authored["text"]);
    assert_eq!(after["title"], authored["title"]);

    // --- jobs: the shared instruction opens every job, output ownership is exact ---------------
    let jobs = zt.digest(&["term-cards", "-Step", "jobs", "-Staging", stage]);
    assert!(jobs.status.success(), "{}", String::from_utf8_lossy(&jobs.stderr));
    let said = last_json(&jobs);
    assert_eq!(said["jobs"].as_array().unwrap().len(), 2);
    let instruction = staged.join("term-cards").join("dsh-term-card-instructions.zh.md");
    assert_eq!(said["jobs"][0]["files"][0], instruction.to_str().unwrap());
    assert_eq!(
        said["jobs"][0]["files"][1],
        staged.join("term-cards").join("jobs").join("t001.md").to_str().unwrap()
    );
    assert_eq!(
        said["jobs"][0]["output"],
        staged.join("term-cards").join("out").join("t001.json").to_str().unwrap()
    );
    assert_eq!(said["jobs"][0]["permissions"]["archive_write"], false);
    // The managed contract travels with the job (S20, #164): the official DeepSeek model at high
    // effort, the Chinese language skill through the job's own field, and no deletion after the
    // answer because the wave keeps its writers through the review/fix opportunity.
    assert_eq!(said["jobs"][0]["model"], "deepseek-official/deepseek-flash");
    assert_eq!(said["jobs"][0]["effort"], "high");
    assert_eq!(said["jobs"][0]["skills"], json!(["zettelkasten", "asd-ste100-zh"]));
    assert_eq!(said["jobs"][0]["delete_after_collect"], false);
    let selected = zt.digest(&[
        "term-cards", "-Step", "jobs", "-Staging", stage, "-Jobs", "t002",
    ]);
    assert_eq!(selected.status.code(), Some(0));
    assert_eq!(last_json(&selected)["jobs"][0]["id"], "t002");
    let unknown = zt.digest(&[
        "term-cards", "-Step", "jobs", "-Staging", stage, "-Jobs", "t999",
    ]);
    assert_ne!(unknown.status.code(), Some(0));

    // --- a missing staged source stops before any file; later slices stop honestly --------------
    let sections = staged.join("sections");
    let backup = staged.join("sections-backup");
    std::fs::rename(&sections, &backup).unwrap();
    let missing = zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]);
    assert!(!missing.status.success());
    assert!(
        String::from_utf8_lossy(&missing.stderr).contains("sections"),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    std::fs::rename(&backup, &sections).unwrap();
    // --- the publication steps publish nothing until a draft is checked and ready (S21, #165) ---
    for (step, apply) in [("apply", true), ("report", false)] {
        let mut args = vec!["term-cards", "-Step", step, "-Staging", stage];
        if apply {
            args.push("-Apply");
        }
        let out = zt.digest(&args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{step}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let said = last_json(&out);
        assert_eq!(said["step"], step);
        assert_eq!(said["complete"], false, "{step}: {said}");
        assert_eq!(said["ready"], 0, "{step}: no draft is checked yet");
        assert_eq!(said["committed"], 0, "{step}");
        assert_eq!(said["refused"], 2, "{step}");
        assert_eq!(said["cards_written"], 0, "{step}");
        assert!(staged.join("term-cards").join("report.json").is_file(), "{step}");
    }
    // The plan's list is a legacy Idea Card, so the ready check found nothing to retype either: the
    // archive and the list's authored text are exactly as the plan read them.
    assert_eq!(mcp.revision(), revision, "a publication without a ready draft wrote a Card");
    assert_eq!(mcp.call("get_card", json!({"at":list}))["text"], authored["text"]);

    // --- the same frozen content keeps revision 1 and the exact plan bytes ---------------------
    let before = std::fs::read(staged.join("term-cards").join("plan.json")).unwrap();
    let again = zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]);
    assert!(again.status.success());
    assert_eq!(last_json(&again)["revision"], 1);
    assert_eq!(
        std::fs::read(staged.join("term-cards").join("plan.json")).unwrap(),
        before
    );
    assert_eq!(mcp.revision(), revision);

    // --- a changed section starts a new plan revision ------------------------------------------
    std::fs::write(
        staged.join("sections").join("s01.md"),
        "剩余控制权记为 $\\rho$，定义为决定权。\n",
    )
    .unwrap();
    let changed = zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]);
    assert!(changed.status.success(), "{}", String::from_utf8_lossy(&changed.stderr));
    let said = last_json(&changed);
    assert_eq!(said["revision"], 2);
    assert_eq!(said["math"], 1);
    let frozen = plan_json(&staged);
    assert_eq!(frozen["revision"], 2);
    assert_eq!(frozen["rows"].as_array().unwrap().len(), 2);

    // --- a Term Card with no name is read by its role, not by its title ------------------------
    let second = mcp.call(
        "create_literature",
        json!({"bibtex":"@book{termbook2026, title={Term Book}}","body":"第二本书。"}),
    )["address"]
        .as_str()
        .expect("second key")
        .to_string();
    mcp.call(
        "create_successor",
        json!({"at":second,"kind":"direct","title":"本书的结构","body":"锚点。"}),
    );
    let typed = mcp.call(
        "create_successor",
        json!({"at":format!("{second}/1"),"kind":"side","type":"term","title":"术语表：《Term Book》的 2 个术语","body":TABLE.replace("[[testbook2026]]", &format!("[[{second}]]")).replace("Test Book", "Term Book")}),
    );
    let typed_address = typed["address"].as_str().expect("typed list").to_string();
    let second_staging = staging(work.path(), "term-book", &second, "Term Book", SECTIONS);
    let before_second = mcp.revision();
    let second_plan = zt.digest(&[
        "term-cards",
        "-Step",
        "plan",
        "-Staging",
        second_staging.to_str().unwrap(),
    ]);
    assert!(
        second_plan.status.success(),
        "type term list: {} {}",
        String::from_utf8_lossy(&second_plan.stdout),
        String::from_utf8_lossy(&second_plan.stderr)
    );
    let said = last_json(&second_plan);
    assert_eq!(said["terms"], 2);
    let frozen = plan_json(&second_staging);
    assert_eq!(frozen["list"]["legacy"], false);
    assert_eq!(frozen["list"]["type"], "term");
    assert_eq!(frozen["list"]["address"], typed_address);
    assert_eq!(mcp.revision(), before_second, "the second plan wrote no Card");
}


