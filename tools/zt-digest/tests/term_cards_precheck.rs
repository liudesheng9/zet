//! The Term-card writer's own SELF-CHECK (S27, #166): every writer job file names the exact
//! read-only command that checks the draft the deterministic step later checks, and that command
//! writes neither a Card nor a compiler artifact.
//!
//! The test needs a `zt` binary built from this repository (or on PATH): it starts its own service
//! on a temporary archive. `ZT_TEST_ZT` overrides the binary, `ZT_TEST_DIGEST` the `zt-digest` one.

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
        let configured = zt.run(&["config", "set", "archive_root", zt.archive.to_str().unwrap()]);
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

const TABLE: &str = "本卡是《Test Book》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡 | 平衡 | 双方都不改变选择的状态 | §1 |
| 剩余控制权 | 剩余索取权 | 合同没有写明的决定权 | §2 |

来源：[[testbook2026]]";

const SECTIONS: &str = "均衡定义为 $x^*=\\arg\\max_x u(x)$，它是没有人愿意偏离的策略。\n剩余控制权定义为合同没有写明的决定权。\n";

fn staging(parent: &Path, name: &str, key: &str) -> PathBuf {
    let dir = parent.join(name);
    std::fs::create_dir_all(dir.join("sections")).expect("make sections");
    std::fs::write(
        dir.join("manifest.json"),
        json!({
            "citation_key": key,
            "book_title": "Test Book",
            "staging": dir.to_string_lossy(),
            "source_sha256": "0".repeat(64),
        })
        .to_string(),
    )
    .expect("write manifest");
    std::fs::write(dir.join("sections").join("s01.md"), SECTIONS).expect("write section");
    dir
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

fn ok_json(output: &Output, step: &str) -> Value {
    assert!(
        output.status.success(),
        "{step}: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    last_json(output)
}

fn work(staged: &Path) -> PathBuf {
    staged.join("term-cards")
}

fn write_output(staged: &Path, job: &str, items: &[Value]) {
    std::fs::write(
        work(staged).join("out").join(format!("{job}.json")),
        json!({"terms": items}).to_string(),
    )
    .expect("write writer output");
}

fn math_draft() -> Value {
    json!({
        "term": "均衡",
        "original": "equilibrium",
        "category": "math",
        "first_defined": "§1",
        "explanation": [
            "均衡是双方都不改变选择的状态。",
            "本书用它说明市场的稳定点。",
            "均衡不等于最优。"
        ],
        "book_definition": { "text": ["§1 把均衡定义为一个不动点。"], "section": "§1", "quote": "" },
        "hypotheses": [ { "id": "H1", "text": "策略集是紧凸集。" } ],
        "definition": {
            "formula": "x^* \\in \\arg\\max_{x} u(x) \\tag{T1}",
            "reading": ["$x^*$ 是没有人愿意偏离的策略。"]
        },
        "symbols": [
            { "symbol": "$x^*$", "meaning": "均衡策略", "type": "紧凸集", "auxiliary": false }
        ],
        "equivalent_forms": [ { "formula": "u(x^*)\\ge u(x)", "reason": "由 (T1) 直接得到。" } ],
        "derivations": [ {
            "claim": "均衡是纳什均衡",
            "steps": [
                { "text": "起点是定义 (T1)。", "basis": "定义 (T1)", "formula": "x^*=x" },
                { "text": "两边比较收益。", "basis": "H1", "formula": "u(x^*)\\ge u(x)" }
            ],
            "result": "均衡是纳什均衡。"
        } ],
        "limits": [ { "case": "无", "text": "本定义没有需要说明的极限情形。", "formula": "" } ],
        "example": ["取 $u(x)=-x^2$。", "唯一均衡是 $x^*=0$。"],
        "intuition": ["没有人愿意单方面偏离。", "均衡是相互一致的选择。"],
        "distinctions": [ { "term": "剩余控制权", "text": "剩余控制权指合同没写明的决定权。" } ],
        "misreadings": ["均衡不是最优。"],
        "role_in_book": ["本书第一章用均衡定义市场结果。"]
    })
}

/// The precheck's own lines, without the trailing `faults: N`.
fn fault_lines(output: &Output) -> Vec<String> {
    let stdout = String::from_utf8_lossy(&output.stdout).replace("\r\n", "\n");
    stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn fault_count(lines: &[String]) -> i64 {
    let last = lines.last().unwrap_or_else(|| panic!("the precheck printed nothing"));
    last.strip_prefix("faults: ")
        .and_then(|n| n.parse::<i64>().ok())
        .unwrap_or_else(|| panic!("the precheck printed no `faults: N` line: {}", lines.join(" | ")))
}

#[test]
fn the_term_card_writer_self_check_checks_the_exact_draft_and_writes_nothing() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    mcp.call(
        "create_literature",
        json!({"bibtex":"@book{testbook2026, title={Test Book}}","body":"一本测试书。"}),
    );
    mcp.call(
        "create_successor",
        json!({"at":"testbook2026","kind":"direct","title":"本书的结构","body":"锚点。"}),
    );
    mcp.call(
        "create_successor",
        json!({"at":"testbook2026/1","kind":"side","type":"term","title":"术语表：《Test Book》的 2 个术语","body":TABLE}),
    );
    let revision = mcp.revision();

    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    let stage = staged.to_str().unwrap();
    ok_json(&zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]), "plan");

    // The job file names the exact read-only self-check of that job, and the writer is told that a
    // run without a `faults:` line did not check.
    let said = ok_json(&zt.digest(&["term-cards", "-Step", "jobs", "-Staging", stage]), "jobs");
    let job_file = PathBuf::from(said["jobs"][0]["files"][1].as_str().expect("job file"));
    let job_text = std::fs::read_to_string(&job_file).expect("read job file");
    let self_check = format!(
        "SELF-CHECK: zt-digest term-cards -Step precheck -Staging \"{stage}\" -Jobs t001"
    );
    assert!(job_text.contains(&self_check), "{job_text}");
    assert!(job_text.contains("faults: 0"), "the writer must be told the goal: {job_text}");
    assert!(
        job_text.contains("没有打印 `faults:` 行"),
        "the writer must be told that a run without a `faults:` line is no check: {job_text}"
    );

    // Before the writer writes anything the self-check reports the missing output, and it has
    // written neither a compiler artifact nor a Card.
    let state_before = std::fs::read(work(&staged).join("state.json")).ok();
    let precheck = zt.digest(&["term-cards", "-Step", "precheck", "-Staging", stage, "-Jobs", "t001"]);
    let lines = fault_lines(&precheck);
    assert_eq!(precheck.status.code(), Some(0), "{}", String::from_utf8_lossy(&precheck.stderr));
    assert_eq!(fault_count(&lines), 1, "{}", lines.join(" | "));
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("t001/r001 missing the writer wrote no output file")),
        "{}",
        lines.join(" | ")
    );
    assert!(!work(&staged).join("check").join("t001.json").exists(), "the precheck wrote a check artifact");
    assert!(!work(&staged).join("check").join("readiness.json").exists(), "the precheck wrote readiness");
    assert_eq!(
        std::fs::read(work(&staged).join("state.json")).ok(),
        state_before,
        "the precheck wrote the compiler state"
    );

    // A draft the deterministic check would refuse fails here too, with the same rule and the same
    // provisional address, and still nothing is written.
    let mut faulty = math_draft();
    faulty["derivations"][0]["steps"][0]["text"] = json!("显然成立。");
    write_output(&staged, "t001", &[faulty]);
    let precheck = zt
        .digest(&["term-cards", "-Step", "precheck", "-Staging", stage, "-Jobs", "t001"])
        ;
    let lines = fault_lines(&precheck);
    assert_eq!(precheck.status.code(), Some(0), "{}", String::from_utf8_lossy(&precheck.stderr));
    assert_eq!(fault_count(&lines), 1, "{}", lines.join(" | "));
    assert!(
        lines.iter().any(|line| line.starts_with("t001/r001 T6 ")),
        "{}",
        lines.join(" | ")
    );
    assert!(!work(&staged).join("check").join("t001.json").exists(), "the precheck wrote a check artifact");
    assert_eq!(mcp.revision(), revision, "the precheck wrote to the archive");

    // The deterministic step reads the same draft the same way: its diagnostics are the precheck's.
    let checked = ok_json(
        &zt.digest(&["term-cards", "-Step", "check", "-Staging", stage, "-Jobs", "t001"]),
        "check",
    );
    assert_eq!(checked["jobs"][0]["ok"], false, "{checked}");
    let check = serde_json::from_str::<Value>(
        &std::fs::read_to_string(work(&staged).join("check").join("t001.json")).expect("check artifact"),
    )
    .expect("check JSON");
    let mut rules: Vec<String> = check["terms"][0]["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .map(|diagnostic| diagnostic["rule"].as_str().unwrap_or_default().to_string())
        .collect();
    rules.sort();
    assert_eq!(rules, vec!["T4".to_string(), "T6".to_string()], "{check}");

    // The corrected draft passes the self-check: no blocking line, and the one advisory line keeps
    // the checker's own `(advisory)` tag and is not counted.
    write_output(&staged, "t001", &[math_draft()]);
    let precheck = zt
        .digest(&["term-cards", "-Step", "precheck", "-Staging", stage, "-Jobs", "t001"])
        ;
    let lines = fault_lines(&precheck);
    assert_eq!(fault_count(&lines), 0, "{}", lines.join(" | "));
    assert!(
        lines[..lines.len() - 1].iter().all(|line| line.contains(" (advisory) ")),
        "an uncounted line must carry the checker's advisory tag: {}",
        lines.join(" | ")
    );
    assert_eq!(mcp.revision(), revision, "the precheck wrote to the archive");
}
