//! The copied-book Term pilot seam (S30, #172): `zt-digest term-pilot` plans, preflights, qualifies
//! and records one pilot over a real scratch box and a real prepared run. The test runs the real
//! `term-cards` and `term-link` commands to produce the run's artifacts, so the gate reads the
//! shapes the workflow really writes.
//!
//! The "copy" is a scratch box built by the test with the same tree, and the retained live snapshot
//! is that tree's own `get_tree` reply: the copy comparison, the list and the before state are real.
//! A real `copy-tree.mjs` copy of the live box is the pilot runbook's job; this seam proves the gate
//! over the shapes it reads.
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
        assert!(configured.status.success(), "config: {}", String::from_utf8_lossy(&configured.stderr));
        let up = zt.run(&["up"]);
        assert!(up.status.success(), "up: {} {}", String::from_utf8_lossy(&up.stdout), String::from_utf8_lossy(&up.stderr));
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
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The book's bilingual-looking source, the same shape the backfill seam uses.
const SOURCE: &str = "# 前言\n\n不算。\n\n# 导论\n\n本书研究均衡。\n\n## 第一节\n\n均衡定义为 $x^*=\\arg\\max_x u(x)$，它是没有人愿意偏离的策略。\n\n剩余控制权定义为合同没有写明的决定权。\n\n# 参考\n\n不进入正文。\n";

/// The pre-Term `术语表` table beside the first anchor.
const TABLE: &str = "本卡是《Test Book》的术语表。本书的卡片用左列的术语，不用右列的词。\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变选择的状态 | §1 |\n| 剩余控制权 | 剩余索取权 | 合同没有写明的决定权 | §2 |\n\n来源：[[Key]]";

const KEY: &str = "Key";

fn last_json(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("no output: {} {}", stdout, String::from_utf8_lossy(&output.stderr)));
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

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read artifact")).expect("JSON")
}

/// A real prepared run of `SOURCE` at the current frozen contract, with its Literature tree.
fn prepared(zt: &Zt, dir: &Path) -> PathBuf {
    let source = dir.join("source.md");
    std::fs::write(&source, SOURCE).expect("write source");
    let out = dir.join("run");
    let output = zt.digest(&[
        "prepare",
        "-Source",
        source.to_str().unwrap(),
        "-Out",
        out.to_str().unwrap(),
        "-CitationKey",
        KEY,
        "-BibTeX",
        "@book{Key, title={Test Book}, author={Author}, year={2020}}",
        "-BookTitle",
        "Test Book",
        "-MinUnitChars",
        "1",
        "-StartHeading",
        "导论",
        "-EndHeading",
        "参考",
        "-Variant",
        "econ",
        "-MaxAttempts",
        "3",
        "-DshRunCap",
        "1200",
        "-Apply",
        "-Brief",
    ]);
    assert!(
        output.status.success(),
        "prepare: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    out
}

fn math_draft() -> Value {
    json!({
        "term": "均衡",
        "original": "equilibrium",
        "category": "math",
        "first_defined": "§1",
        "explanation": ["均衡是双方都不改变选择的状态。", "本书用它说明市场的稳定点。", "均衡不等于最优。"],
        "book_definition": { "text": ["§1 把均衡定义为一个不动点。"], "section": "§1", "quote": "" },
        "hypotheses": [ { "id": "H1", "text": "策略集是紧凸集。" } ],
        "definition": { "formula": "x^* \\in \\arg\\max_{x} u(x) \\tag{T1}", "reading": ["$x^*$ 是没有人愿意偏离的策略。"] },
        "symbols": [ { "symbol": "$x^*$", "meaning": "均衡策略", "type": "紧凸集", "auxiliary": false } ],
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
        "explanation": ["剩余控制权是对资产用途的决定权。", "本书用它区分所有权与控制。", "这个概念不等于日常的经营权。"],
        "book_definition": { "text": ["§2 把它定义为合同没有写明的决定权。"], "section": "§2", "quote": "" },
        "distinctions": [ { "term": "均衡", "text": "均衡指双方都不改变选择。" } ],
        "misreadings": ["这个概念不是剩余收益。"],
        "role_in_book": ["本书第二章用它解释企业边界。"]
    })
}

fn work(staged: &Path) -> PathBuf {
    staged.join("term-cards")
}

fn write_output(staged: &Path, job: &str, items: &[Value]) {
    std::fs::create_dir_all(work(&staged).join("out")).expect("make out");
    std::fs::write(
        work(&staged).join("out").join(format!("{job}.json")),
        json!({"terms": items}).to_string(),
    )
    .expect("write writer output");
}

fn review_answer_path(request: &Value) -> PathBuf {
    PathBuf::from(request["answer"].as_str().expect("the review job names its answer"))
}

/// The real Term-card branch and the real Term-link pass of the pilot: plan, writer outputs, the
/// independent review, publication, the deterministic link pass, and the retained second plan.
fn run_term_branch(zt: &Zt, staged: &Path) {
    let stage = staged.to_str().unwrap();
    ok_json(&zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]), "term-cards plan");
    write_output(staged, "t001", &[math_draft()]);
    write_output(staged, "t002", &[concept_draft()]);
    ok_json(&zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]), "term-cards check");
    let review = ok_json(&zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]), "review-plan");
    for job in review["jobs"].as_array().into_iter().flatten() {
        std::fs::write(review_answer_path(job), "{\"ok\":true}").expect("write the review answer");
    }
    let readiness = ok_json(&zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]), "review-plan 2");
    assert_eq!(readiness["complete"], json!(true), "the fixture is not ready: {readiness}");
    ok_json(&zt.digest(&["term-cards", "-Step", "apply", "-Staging", stage, "-Apply"]), "term-cards apply");

    ok_json(&zt.digest(&["term-link", "-Step", "plan", "-Staging", stage]), "term-link plan");
    ok_json(&zt.digest(&["term-link", "-Step", "apply", "-Staging", stage, "-Apply"]), "term-link apply");

    let link = staged.join("term-link");
    std::fs::copy(link.join("plan.json"), link.join("first-plan.json")).expect("retain the first plan");
    ok_json(&zt.digest(&["term-link", "-Step", "plan", "-Staging", stage]), "term-link plan 2");
    std::fs::copy(link.join("plan.json"), link.join("second-plan.json")).expect("retain the second plan");
    std::fs::copy(link.join("first-plan.json"), link.join("plan.json")).expect("restore the first plan");

    let report = zt.digest(&["ledger", "report", "-Out", stage]);
    assert!(
        report.status.success(),
        "ledger report: {} {}",
        String::from_utf8_lossy(&report.stdout),
        String::from_utf8_lossy(&report.stderr)
    );
}

/// The copy folder `copy-tree.mjs` leaves behind, with the live snapshot and its comparison.
fn copy_folder(mcp: &mut Mcp, dir: &Path, tree_key: &str) -> PathBuf {
    let copy = dir.join("copy");
    std::fs::create_dir_all(&copy).expect("make copy");
    let tree = mcp.call("get_tree", json!({"tree": tree_key, "bodies": true}));
    let count = tree["cards"].as_array().map(Vec::len).unwrap_or(0);
    std::fs::write(copy.join("live-tree.json"), serde_json::to_string_pretty(&tree).unwrap()).expect("write live tree");
    std::fs::write(
        copy.join("report.json"),
        serde_json::to_string_pretty(&json!({"live_count": count, "test_count": count, "differences": [], "broken_outside_links": []})).unwrap(),
    )
    .expect("write report");
    copy
}

fn authorization(zt: &Zt, dir: &Path, copy: &Path, copy_tree: &str, live_revision: u64) -> PathBuf {
    let path = dir.join("authorization.json");
    let record = json!({
        "schema_version": 1,
        "pilot": "S30",
        "approved": true,
        "note": "the S30 copied-book pilot test",
        "book": KEY,
        "mode": "backfill",
        "box": {"config_dir": zt.config, "archive_root": zt.archive, "archive_id": "0123456789abcdef0123456789abcdef"},
        "live": {"archive_id": "fedcba9876543210fedcba9876543210", "archive_root": dir.join("live-box"), "revision": live_revision},
        "copy": {"folder": copy, "tree": copy_tree},
        "review": {"math_terms": 3, "concept_terms": 2, "gui_cards": 10},
    });
    std::fs::write(&path, serde_json::to_string_pretty(&record).unwrap()).expect("write authorization");
    path
}

fn pilot(zt: &Zt, step: &str, args: &[&str]) -> Output {
    let mut all = vec!["term-pilot", "-Step", step];
    all.extend_from_slice(args);
    zt.digest(&all)
}

#[test]
fn a_copied_book_pilot_is_planned_preflighted_qualified_and_recorded() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let holder = tempfile::tempdir().expect("work");
    let staged = prepared(&zt, holder.path());
    let stage = staged.to_str().unwrap();
    // The list and two prose Cards the backfill adds to the already digested tree.
    mcp.call("create_successor", json!({"at": format!("{KEY}/1"), "kind": "side", "title": "术语表：《Test Book》的 2 个术语", "body": TABLE}));
    mcp.call("create_successor", json!({"at": format!("{KEY}/1"), "kind": "side", "title": "均衡是稳定的状态", "body": "均衡定义为 $x^*=\\arg\\max_x u(x)$，它是没有人愿意偏离的策略。"}));
    mcp.call("create_successor", json!({"at": format!("{KEY}/1"), "kind": "side", "title": "剩余控制权决定企业边界", "body": "剩余控制权定义为合同没有写明的决定权。"}));
    // The early carrier the copy evidence names: a second Literature tree copied into the box.
    mcp.call("create_literature", json!({"bibtex":"@book{Earlier, title={Earlier Book}, author={A}, year={2019}}", "body":"一本早先的书。"}));
    mcp.call("create_successor", json!({"at":"Earlier", "kind":"direct", "title":"早先的卡片", "body":"早先的正文。"}));
    let copy = copy_folder(&mut mcp, holder.path(), "Earlier");
    let tree = mcp.call("get_tree", json!({"tree": "Earlier"}));
    let revision = tree["archive_revision"].as_u64().expect("tree revision");
    let auth = authorization(&zt, holder.path(), &copy, "Earlier", revision);
    let work_dir = holder.path().join("work");
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");

    // --- plan refuses a copy that does not match the retained snapshot ---
    let broken = holder.path().join("broken-copy");
    std::fs::create_dir_all(&broken).expect("make broken copy");
    std::fs::copy(copy.join("live-tree.json"), broken.join("live-tree.json")).expect("copy snapshot");
    std::fs::write(
        broken.join("report.json"),
        serde_json::to_string(&json!({"live_count": 1, "test_count": 1, "differences": [{"address": format!("{KEY}/1"), "field": "body"}]})).unwrap(),
    )
    .expect("write broken report");
    let refused = pilot(
        &zt,
        "plan",
        &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-File", auth.to_str().unwrap(), "-Copy", broken.to_str().unwrap(), "-Project", project.to_str().unwrap()],
    );
    assert_eq!(refused.status.code(), Some(1), "a mismatched copy must refuse the plan");
    assert!(!work_dir.join("term-pilot-plan.json").exists(), "a refused plan writes no plan record");

    // --- the plan records the copy, the tree, the list and the before state ---
    let planned = ok_json(
        &pilot(
            &zt,
            "plan",
            &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-File", auth.to_str().unwrap(), "-Copy", copy.to_str().unwrap(), "-Project", project.to_str().unwrap()],
        ),
        "plan",
    );
    assert_eq!(planned["book"]["citation_key"], json!(KEY));
    assert_eq!(planned["decision"], json!("pending-human"));
    assert_eq!(planned["box"]["list"]["rows"], json!(2), "{planned}");
    assert_eq!(planned["copy"]["report"]["differences"], json!(0));
    assert_eq!(planned["copy"]["live_tree"]["fingerprint"], planned["copy"]["copied_tree"]["fingerprint"]);
    assert_ne!(planned["box"]["tree"]["fingerprint"], planned["copy"]["live_tree"]["fingerprint"]);
    assert!(planned["before"]["cards"].as_u64().unwrap_or(0) >= 3, "{planned}");
    assert!(work_dir.join("term-pilot-before.json").is_file());
    assert!(planned["binaries"].as_array().map(Vec::len).unwrap_or(0) >= 2, "the plan names the installed binaries: {planned}");

    // --- preflight passes, then refuses a run that already started ---
    std::fs::write(staged.join("zt-box.txt"), zt.config.to_str().unwrap()).expect("pin the box");
    let ready = ok_json(
        &pilot(&zt, "preflight", &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-Project", project.to_str().unwrap()]),
        "preflight",
    );
    assert_eq!(ready["ok"], json!(true), "{ready}");
    std::fs::create_dir_all(work(&staged)).expect("make term-cards");
    std::fs::write(work(&staged).join("report.json"), "{}").expect("start the run");
    let started = pilot(&zt, "preflight", &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-Project", project.to_str().unwrap()]);
    assert_eq!(started.status.code(), Some(1), "a started run must refuse the launch");
    assert_eq!(read_json(&work_dir.join("term-pilot-preflight.json"))["ok"], json!(false));
    std::fs::remove_file(work(&staged).join("report.json")).expect("clear the marker");
    let ready = ok_json(
        &pilot(&zt, "preflight", &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-Project", project.to_str().unwrap()]),
        "preflight again",
    );
    assert_eq!(ready["ok"], json!(true), "{ready}");

    // --- the real Term branch, then the qualification ---
    run_term_branch(&zt, &staged);
    let qualified = ok_json(
        &pilot(&zt, "qualify", &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-Project", project.to_str().unwrap()]),
        "qualify",
    );
    assert_eq!(qualified["rows"]["total"], json!(2), "{qualified}");
    assert_eq!(qualified["rows"]["verified"], json!(2), "{qualified}");
    assert_eq!(qualified["rows"]["faults"], json!([]));
    assert_eq!(qualified["list"]["preserved"], json!(true));
    assert_eq!(qualified["preservation"]["complete"], json!(true));
    assert!(qualified["preservation"]["changed"].as_u64().unwrap_or(0) >= 1, "the link pass changed at least one body: {qualified}");
    assert_eq!(qualified["second"]["no_writes"], json!(true));
    assert_eq!(qualified["diagnostics"]["complete"], json!(true), "{qualified}");
    assert_eq!(qualified["verdict"], json!("complete"), "{qualified}");
    assert_eq!(qualified["decision"], json!("pending-human"));
    assert_eq!(qualified["review"]["math"], json!(1));
    assert_eq!(qualified["review"]["concept"], json!(1));
    assert_eq!(qualified["review"]["shortfall"]["math_terms"], json!(2));
    assert!(work_dir.join("term-pilot-review.json").is_file());
    assert!(work_dir.join("term-pilot-review.md").is_file());

    // --- the owner's decision: an incomplete approval is refused, a rejection is recorded ---
    let mut review = read_json(&work_dir.join("term-pilot-review.json"));
    review["decision"] = json!("approve");
    for section in ["math", "concept", "cards"] {
        for item in review[section].as_array_mut().into_iter().flatten() {
            item["decision"] = json!("accept");
            item["note"] = json!("");
        }
    }
    let approve = holder.path().join("approve.json");
    std::fs::write(&approve, serde_json::to_string(&review).unwrap()).expect("write approve");
    let refused_decision = pilot(&zt, "record", &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-Decision", approve.to_str().unwrap(), "-Project", project.to_str().unwrap()]);
    assert_eq!(refused_decision.status.code(), Some(1), "an incomplete approval must be refused");

    review["decision"] = json!("reject");
    review["unresolved"] = json!(["the test fixture has one math term and one concept term; the pilot book needs three and two"]);
    let reject = holder.path().join("reject.json");
    std::fs::write(&reject, serde_json::to_string(&review).unwrap()).expect("write reject");
    let recorded = ok_json(
        &pilot(&zt, "record", &["-Staging", stage, "-Work", work_dir.to_str().unwrap(), "-Decision", reject.to_str().unwrap(), "-Project", project.to_str().unwrap()]),
        "record",
    );
    assert_eq!(recorded["ok"], json!(true), "{recorded}");
    assert_eq!(recorded["decision"], json!("reject"));
    assert_eq!(recorded["applies"], json!([]), "the pilot writes no Card");
    assert_eq!(recorded["qualification_id"], qualified["qualification_id"]);
    assert!(work_dir.join("term-pilot-decision.json").is_file());
}
