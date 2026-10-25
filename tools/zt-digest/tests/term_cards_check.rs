//! The Term-card check/review/fix seam (S19, #162): the commands run over a real scratch archive
//! and a staged source fixture. A stand-in writer writes the exact JSON drafts, the check step
//! checks them, one stand-in reviewer answer approves or refuses the exact mathematical draft, and
//! the one fix returns to the writer's own job through `continue`/`fallback`.
//!
//! The archive revision and the Term list's authored text prove that no step writes a Card: the
//! readiness result is not publication.
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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read artifact")).expect("JSON")
}

fn write_output(staged: &Path, job: &str, items: &[Value]) {
    std::fs::write(
        work(staged).join("out").join(format!("{job}.json")),
        json!({"terms": items}).to_string(),
    )
    .expect("write writer output");
}

/// The complete mathematical draft the ZT checker's own Term tests use.
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

fn concept_draft() -> Value {
    json!({
        "term": "剩余控制权",
        "category": "concept",
        "first_defined": "§2",
        "explanation": [
            "剩余控制权是对资产用途的决定权。",
            "本书用它区分所有权与控制。",
            "这个概念不等于日常的经营权。"
        ],
        "book_definition": { "text": ["§2 把它定义为合同没有写明的决定权。"], "section": "§2", "quote": "" },
        "distinctions": [ { "term": "均衡", "text": "均衡指双方都不改变选择。" } ],
        "misreadings": ["这个概念不是剩余收益。"],
        "role_in_book": ["本书第二章用它解释企业边界。"]
    })
}

fn review_answer_path(request: &Value) -> PathBuf {
    PathBuf::from(request["answer"].as_str().expect("the review job names its answer"))
}

fn review_stem_of(job: &Value) -> String {
    Path::new(job["answer"].as_str().unwrap())
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

#[test]
fn the_term_drafts_become_ready_only_after_their_exact_review_and_never_publish() {
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
    let list = mcp.call(
        "create_successor",
        json!({"at":"testbook2026/1","kind":"side","type":"term","title":"术语表：《Test Book》的 2 个术语","body":TABLE}),
    )["address"]
        .as_str()
        .expect("list address")
        .to_string();
    let revision = mcp.revision();
    let authored = mcp.call("get_card", json!({"at":list}));

    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    let stage = staged.to_str().unwrap();

    let planned = zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]);
    let said = ok_json(&planned, "plan");
    assert_eq!(said["terms"], 2);
    assert_eq!(said["math"], 1);
    assert_eq!(said["jobs"][0]["id"], "t001", "{said}");

    // A stand-in writer: the math job writes a draft with a skipped step, the concept job writes
    // a complete concept draft.
    let mut faulty = math_draft();
    faulty["derivations"][0]["steps"][0]["text"] = json!("显然成立。");
    write_output(&staged, "t001", &[faulty]);
    write_output(&staged, "t002", &[concept_draft()]);

    let checked = zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]);
    let said = ok_json(&checked, "check");
    assert_eq!(said["jobs"][0]["ok"], false);
    assert_eq!(said["jobs"][1]["ok"], true);
    assert_eq!(said["counts"]["needs_fix"], 1);
    assert_eq!(said["counts"]["ready"], 1);
    let check = read_json(&work(&staged).join("check").join("t001.json"));
    assert!(
        check["terms"][0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["rule"] == json!("T6")),
        "{check}"
    );
    let readiness = read_json(&work(&staged).join("check").join("readiness.json"));
    assert_eq!(readiness["complete"], false);
    assert_eq!(readiness["publication"]["state"], "unverified");
    assert_eq!(mcp.revision(), revision, "the check step wrote to the archive");

    // One finite fix goes back to the writer's own job.
    let fixed = zt.digest(&["term-cards", "-Step", "fix-plan", "-Staging", stage]);
    let said = ok_json(&fixed, "fix-plan");
    assert_eq!(said["jobs"].as_array().unwrap().len(), 1, "{said}");
    assert_eq!(said["jobs"][0]["id"], "t001-fix");
    assert_eq!(said["jobs"][0]["continue"], "t001");
    assert_eq!(said["jobs"][0]["output"], work(&staged).join("out").join("t001.json").to_str().unwrap());
    let fallback = said["jobs"][0]["fallback"].as_array().unwrap();
    assert!(
        fallback.iter().any(|path| path.as_str() == Some(work(&staged).join("out").join("t001.json").to_str().unwrap())),
        "{said}"
    );
    let fix = std::fs::read_to_string(work(&staged).join("fix").join("t001-fix.md")).unwrap();
    assert!(fix.contains("显然"), "{fix}");

    // The writer's one fix changes the draft; the check passes and the mathematical draft now owes
    // its independent review.
    write_output(&staged, "t001", &[math_draft()]);
    let checked = zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]);
    let said = ok_json(&checked, "check");
    assert_eq!(said["jobs"][0]["ok"], true, "{said}");
    assert_eq!(said["counts"]["needs_review"], 1);
    assert_eq!(said["counts"]["ready"], 1);

    let review = zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]);
    let said = ok_json(&review, "review-plan");
    assert_eq!(said["jobs"].as_array().unwrap().len(), 1, "{said}");
    assert_eq!(said["jobs"][0]["id"], "t001-r001-review");
    let request = read_json(&work(&staged).join("check").join("t001.json"));
    let draft_sha = request["terms"][0]["draft_sha256"].as_str().unwrap();
    let render_sha = request["terms"][0]["render_sha256"].as_str().unwrap();
    let request_file = said["jobs"][0]["request"].as_str().unwrap();
    let text = std::fs::read_to_string(request_file).unwrap();
    assert!(text.contains(draft_sha), "{text}");
    assert!(text.contains(render_sha), "{text}");
    assert!(text.contains("plan_revision=1"), "{text}");
    assert!(text.contains("\"均衡\""), "{text}");
    // A fresh conversation: the review job continues no writer job.
    assert!(
        said["jobs"][0].get("continue").is_none(),
        "the review must run in a fresh conversation: {said}"
    );
    assert_eq!(said["jobs"][0]["row"], 1);

    // A stand-in reviewer approves the exact draft.
    std::fs::write(
        review_answer_path(&said["jobs"][0]),
        "{\"ok\":true}",
    )
    .expect("write the review answer");
    let again = zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]);
    let said = ok_json(&again, "review-plan");
    assert_eq!(said["jobs"], json!([]));
    assert_eq!(said["complete"], true, "{said}");
    assert_eq!(said["counts"]["ready"], 2);
    assert_eq!(mcp.revision(), revision, "a readiness result published a Card");

    // The archive still holds the Term list exactly as planned: no step wrote a Card.
    let after = mcp.call("get_card", json!({"at":list}));
    assert_eq!(after["text"], authored["text"]);
    assert_eq!(after["title"], authored["title"]);
}

#[test]
fn a_refused_review_gets_one_fix_and_a_second_refusal_is_final() {
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
    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    let stage = staged.to_str().unwrap();
    ok_json(&zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]), "plan");
    write_output(&staged, "t001", &[math_draft()]);
    write_output(&staged, "t002", &[concept_draft()]);
    let said = ok_json(
        &zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]),
        "check",
    );
    assert_eq!(said["jobs"][0]["ok"], true);

    // The first independent review refuses the derivation.
    let said = ok_json(
        &zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]),
        "review-plan",
    );
    assert_eq!(said["jobs"].as_array().unwrap().len(), 1);
    let answer = review_answer_path(&said["jobs"][0]);
    std::fs::write(&answer, json!({"ok":false,"faults":[{"where":"derivations[0].steps[1]","what":"这一步的符号不对。","fix":"按 H1 重写这一步。"}]}).to_string()).unwrap();

    // The refusal is carried back to the writer's own conversation as the one permitted fix.
    let said = ok_json(
        &zt.digest(&["term-cards", "-Step", "fix-plan", "-Staging", stage]),
        "fix-plan",
    );
    assert_eq!(said["jobs"].as_array().unwrap().len(), 1, "{said}");
    assert_eq!(said["jobs"][0]["continue"], "t001");
    assert_eq!(said["planned"][0]["row"], 1);
    let fix = std::fs::read_to_string(work(&staged).join("fix").join("t001-fix.md")).unwrap();
    assert!(fix.contains("这一步的符号不对。"), "{fix}");
    assert!(fix.contains("按 H1 重写这一步。"), "{fix}");
    let state = read_json(&work(&staged).join("state.json"));
    assert_eq!(state["fixes"].as_array().unwrap().len(), 1);

    // The writer changes the mathematical draft. The old approval no longer binds it, so a fresh
    // review is planned, and its refusal is final.
    let mut changed = math_draft();
    changed["derivations"][0]["steps"][1]["text"] = json!("两边比较收益，结论不变。");
    write_output(&staged, "t001", &[changed]);
    ok_json(
        &zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]),
        "check 2",
    );
    let said = ok_json(
        &zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]),
        "review-plan 2",
    );
    assert_eq!(said["jobs"].as_array().unwrap().len(), 1, "{said}");
    let fresh_stem = review_stem_of(&said["jobs"][0]);
    assert!(fresh_stem.starts_with("t001-r001-"), "{fresh_stem}");
    std::fs::write(
        review_answer_path(&said["jobs"][0]),
        json!({"ok":false,"faults":[{"where":"derivations[0].steps[1]","what":"改写仍然不成立。","fix":"回到 H1 的原式。"}]}).to_string(),
    )
    .unwrap();

    let said = ok_json(
        &zt.digest(&["term-cards", "-Step", "fix-plan", "-Staging", stage]),
        "fix-plan 2",
    );
    assert_eq!(said["jobs"], json!([]), "a second refusal gets no further fix: {said}");
    assert!(
        said["final"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["reason"] == json!("review_refused_after_fix")),
        "{said}"
    );
    let state = read_json(&work(&staged).join("state.json"));
    assert_eq!(
        state["fixes"].as_array().unwrap().len(),
        1,
        "one fix per term and plan revision"
    );
    let readiness = read_json(&work(&staged).join("check").join("readiness.json"));
    assert_eq!(readiness["terms"][0]["status"], "refused");
    assert_eq!(readiness["terms"][0]["reason"], "review_refused_after_fix");
    assert_eq!(readiness["terms"][1]["status"], "ready");
    assert_eq!(readiness["complete"], false);
    assert_eq!(readiness["publication"]["state"], "unverified");
}
