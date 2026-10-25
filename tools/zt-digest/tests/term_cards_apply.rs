//! The Term-card publication seam (S21, #165): `terms -Step apply` writes a new book's list as a
//! nameless Term Card, and `term-cards -Step apply|report` publish the longest ready contiguous row
//! prefix as Side children of that list with their continuation parts, then verify every committed
//! Card by reading it back.
//!
//! The commands run over a real scratch archive and a staged source fixture. A stand-in writer
//! writes the exact JSON drafts and a stand-in reviewer answers the mathematics review, so every
//! row reaches `ready` before publication. The tests read the archive back through `zt mcp`: the
//! exact head/part addresses, titles, bodies and Term names, the archive revision, and the state a
//! lost publication reply leaves behind.
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

    fn card(&mut self, at: &str) -> Value {
        self.call("get_card", json!({"at": at}))
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

/// The same list with a third row, for the prefix tests.
const TABLE3: &str = "本卡是《Test Book》的术语表。本书的卡片用左列的术语，不用右列的词。

术语表

| 术语 | 不用 | 意思 | 首次定义 |
| --- | --- | --- | --- |
| 均衡 | 平衡 | 双方都不改变选择的状态 | §1 |
| 剩余控制权 | 剩余索取权 | 合同没有写明的决定权 | §2 |
| 企业边界 | — | 企业自己完成的活动范围 | §3 |

来源：[[testbook2026]]";

const SECTIONS: &str = "均衡定义为 $x^*=\\arg\\max_x u(x)$，它是没有人愿意偏离的策略。\n剩余控制权定义为合同没有写明的决定权。\n企业边界是指企业自己完成的活动范围。\n";

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

/// A book with a typed, nameless Term list (the shape `terms -Step apply` writes for a new book)
/// and one section anchor beside the root.
fn book(mcp: &mut Mcp) -> String {
    book_with(mcp, TABLE, "2")
}

/// The same book with the list body the caller wants.
fn book_with(mcp: &mut Mcp, table: &str, terms: &str) -> String {
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
        json!({"at":"testbook2026/1","kind":"side","type":"term","title":format!("术语表：《Test Book》的 {terms} 个术语"),"body":table}),
    )["address"]
        .as_str()
        .expect("list address")
        .to_string()
}

/// A book whose list is the pre-term `术语表` Idea Card the earlier Terms stage wrote.
fn legacy_book(mcp: &mut Mcp) -> String {
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
        json!({"at":"testbook2026/1","kind":"side","title":"术语表：《Test Book》的 2 个术语","body":TABLE}),
    )["address"]
        .as_str()
        .expect("legacy list address")
        .to_string()
}

/// A mathematical draft whose derivation is long enough to render a head and continuation parts:
/// the renderer splits a derivation at a whole step boundary once a Card holds `MaxStepsPerCard`
/// steps, so twenty steps make the second Card a part.
fn math_draft_with_steps(steps: usize) -> Value {
    let mut draft = math_draft();
    let list: Vec<Value> = (1..=steps)
        .map(|step| {
            json!({
                "text": format!("第 {step} 步由前一式代入得到。"),
                "basis": "H1",
                "formula": format!("x_{step}=x"),
            })
        })
        .collect();
    draft["derivations"][0]["steps"] = json!(list);
    draft
}

/// The same with the writer's mathematical draft chosen by the caller.
fn ready_with(staged: &Path, zt: &Zt, math: Value) -> (Value, Value) {
    let stage = staged.to_str().unwrap();
    ok_json(
        &zt.digest(&["term-cards", "-Step", "plan", "-Staging", stage]),
        "plan",
    );
    write_output(staged, "t001", &[math]);
    write_output(staged, "t002", &[concept_draft()]);
    let said = ok_json(
        &zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]),
        "check",
    );
    let review = ok_json(
        &zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]),
        "review-plan",
    );
    if let Some(job) = review["jobs"].as_array().and_then(|jobs| jobs.first()) {
        std::fs::write(review_answer_path(job), "{\"ok\":true}").expect("write the review answer");
    }
    let readiness = ok_json(
        &zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]),
        "review-plan 2",
    );
    if readiness["complete"] != json!(true) {
        let check = read_json(&work(staged).join("check").join("t001.json"));
        panic!(
            "the fixture is not ready: {}",
            serde_json::to_string_pretty(&check["terms"][0]).unwrap()
        );
    }
    (said, readiness)
}

#[test]
fn the_terms_stage_publishes_a_new_books_list_as_a_nameless_term_card() {
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
    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    std::fs::create_dir_all(staged.join("terms").join("out")).expect("make terms out");
    std::fs::write(
        staged.join("terms").join("out").join("terms.json"),
        json!({"terms":[
            {"term":"均衡","avoid":["平衡"],"meaning":"双方都不改变选择的状态","section":"§1"},
            {"term":"剩余控制权","avoid":[],"meaning":"合同没有写明的决定权","section":"§2"}
        ]})
        .to_string(),
    )
    .expect("write the terms answer");

    let applied = ok_json(
        &zt.digest(&[
            "terms",
            "-Step",
            "apply",
            "-Staging",
            staged.to_str().unwrap(),
            "-Apply",
        ]),
        "terms apply",
    );
    let address = applied["card"].as_str().expect("the list address");
    let list = mcp.card(address);
    assert_eq!(list["type"], "term", "a new book's list is a Term Card from the start");
    assert_eq!(list["term"], Value::Null, "the list carries no Term name");
    assert!(
        list["title"].as_str().unwrap_or_default().starts_with("术语表：《Test Book》"),
        "{list}"
    );
    assert!(list["body"].as_str().unwrap().contains("| 剩余控制权 |"), "{list}");
}

/// A book with a typed Term list and both rows ready, plus the staged folder that owns them.
fn ready_book(zt: &Zt, mcp: &mut Mcp) -> (tempfile::TempDir, PathBuf, String) {
    ready_book_with(zt, mcp, math_draft())
}

/// The same book with the caller's mathematical draft.
fn ready_book_with(zt: &Zt, mcp: &mut Mcp, math: Value) -> (tempfile::TempDir, PathBuf, String) {
    let list = book(mcp);
    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    let (checked, readiness) = ready_with(&staged, zt, math);
    assert_eq!(checked["jobs"].as_array().map_or(0, Vec::len), 2, "{checked}");
    assert_eq!(readiness["complete"], true, "{readiness}");
    (holder, staged, list)
}

fn digest(zt: &Zt, staged: &Path, step: &str, switch: bool) -> Output {
    let stage = staged.to_str().unwrap().to_string();
    let mut args = vec!["term-cards", "-Step", step, "-Staging", stage.as_str()];
    if switch {
        args.push("-Apply");
    }
    zt.digest(&args)
}

fn side_children(mcp: &mut Mcp, key: &str, list: &str) -> Vec<Value> {
    let tree = mcp.call("get_tree", json!({"tree": key}));
    tree["cards"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|card| card["parent"] == json!(list) && card["edge"] == json!("side"))
        .collect()
}

fn direct_children(mcp: &mut Mcp, key: &str, head: &str) -> Vec<Value> {
    let tree = mcp.call("get_tree", json!({"tree": key}));
    tree["cards"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|card| card["parent"] == json!(head) && card["edge"] == json!("direct"))
        .collect()
}

#[test]
fn the_report_reads_publication_coverage_without_writing_a_card() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (_holder, staged, list) = ready_book(&zt, &mut mcp);
    let revision = mcp.revision();

    let report = ok_json(&digest(&zt, &staged, "report", false), "report");
    assert_eq!(report["step"], "report");
    assert_eq!(report["complete"], false, "nothing is published yet: {report}");
    assert_eq!(report["rows"], 2);
    assert_eq!(report["ready"], 2);
    assert_eq!(report["committed"], 0);
    assert_eq!(report["ready_unpublished"], 2);
    assert_eq!(report["cards_written"], 0);
    assert_eq!(report["code"], "ready_unpublished");
    assert_eq!(mcp.revision(), revision, "the report wrote a Card");

    // The two artifacts carry the same coverage: per row the readiness evidence and the slot state.
    let json = read_json(&work(&staged).join("report.json"));
    assert_eq!(json["kind"], "term_cards_report");
    assert_eq!(json["list"]["address"], json!(list));
    assert_eq!(json["rows"][0]["row"], 1);
    assert_eq!(json["rows"][0]["term"], "均衡");
    assert_eq!(json["rows"][0]["readiness"]["status"], "ready");
    assert_eq!(json["rows"][0]["publication"]["status"], "ready_unpublished");
    assert_eq!(json["rows"][1]["publication"]["status"], "ready_unpublished");
    assert_eq!(json["staged"].as_array().map_or(0, Vec::len), 2);
    let md = std::fs::read_to_string(work(&staged).join("report.md")).expect("report.md");
    assert!(md.contains("| 1 | 均衡 |"), "{md}");
    assert!(md.contains("ready_unpublished"), "{md}");
    assert!(md.contains("Not complete"), "{md}");
}

#[test]
fn apply_publishes_the_ready_prefix_as_side_children_with_their_parts_and_verifies_them() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (_holder, staged, list) = ready_book_with(&zt, &mut mcp, math_draft_with_steps(20));
    let revision = mcp.revision();

    let applied = ok_json(&digest(&zt, &staged, "apply", true), "apply");
    assert_eq!(applied["step"], "apply");
    assert_eq!(applied["complete"], true, "{applied}");
    assert_eq!(applied["committed"], 2);
    assert_eq!(applied["ready_unpublished"], 0);
    assert_eq!(applied["retyped"], false);
    assert!(applied["cards_written"].as_u64().unwrap() >= 2, "{applied}");
    assert!(mcp.revision() > revision, "publication wrote Cards");

    // The heads are the list's immediate Side children, in row order, each a Term Card whose exact
    // title and Term name are the table row's spelling.
    let sides = side_children(&mut mcp, "testbook2026", &list);
    assert_eq!(sides.len(), 2, "{sides:?}");
    assert_eq!(sides[0]["term"], "均衡");
    assert_eq!(sides[0]["title"], "均衡");
    assert_eq!(sides[0]["type"], "term");
    assert_eq!(sides[1]["term"], "剩余控制权");
    assert_eq!(sides[1]["title"], "剩余控制权");
    let first = sides[0]["address"].as_str().unwrap().to_string();
    let second = sides[1]["address"].as_str().unwrap().to_string();
    assert!(first.starts_with(&format!("{list}|")), "{first}");
    assert!(second.starts_with(&format!("{list}|")), "{second}");

    // The mathematical term's continuation parts are the head's Direct chain, and each names its
    // head with an ordinary Link in the `续：` line.
    let parts = direct_children(&mut mcp, "testbook2026", &first);
    assert!(!parts.is_empty(), "the math draft renders at least one part");
    assert!(
        parts[0]["title"].as_str().unwrap_or_default().starts_with("推导（续 2/"),
        "{parts:?}"
    );
    let part = mcp.card(parts[0]["address"].as_str().unwrap());
    assert_eq!(part["term"], "均衡");
    assert!(
        part["body"].as_str().unwrap().contains(&format!("[[{first}]]")),
        "{}",
        part["body"]
    );
    // A concept term renders one Card with no continuation part.
    assert!(direct_children(&mut mcp, "testbook2026", &second).is_empty());

    // The report (written by the same run) records the exact addresses and the receipt keys.
    let json = read_json(&work(&staged).join("report.json"));
    assert_eq!(json["complete"], true);
    assert_eq!(json["rows"][0]["publication"]["head"], json!(first));
    let receipts = json["rows"][0]["publication"]["receipts"].as_array().unwrap();
    assert_eq!(receipts.len(), 1 + parts.len(), "{receipts:?}");
    assert!(
        receipts[0]["op_key"].as_str().unwrap().contains("/apply/r001"),
        "{receipts:?}"
    );
    assert_eq!(receipts[0]["address"], json!(first));
    assert!(
        receipts[1]["op_key"].as_str().unwrap().contains("/apply/r001-p01"),
        "{receipts:?}"
    );
    let state = read_json(&work(&staged).join("state.json"));
    assert_eq!(state["published"]["rows"]["1"]["head"]["address"], json!(first));
    assert_eq!(
        state["published"]["rows"]["2"]["head"]["address"],
        json!(second)
    );

    // A second run finds its own publication by content: no Card, no new revision, no duplicate.
    let revision = mcp.revision();
    let again = ok_json(&digest(&zt, &staged, "apply", true), "apply 2");
    assert_eq!(again["complete"], true);
    assert_eq!(again["cards_written"], 0);
    assert_eq!(again["cards_replayed"], 0, "nothing is sent: the prefix is verified by reading it");
    assert_eq!(again["batches"], 0);
    assert_eq!(mcp.revision(), revision, "a completed publication writes nothing on a retry");
    assert_eq!(side_children(&mut mcp, "testbook2026", &list).len(), 2);
}

/// A three-row book: the math row, a concept row whose draft the caller writes, and a third row.
fn three_row_book(zt: &Zt, mcp: &mut Mcp, second: Value) -> (tempfile::TempDir, PathBuf, String) {
    let list = book_with(mcp, TABLE3, "3");
    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    let stage = staged.to_str().unwrap();
    ok_json(
        &zt.digest(&[
            "term-cards", "-Step", "plan", "-Staging", stage, "-PerJob", "1",
        ]),
        "plan",
    );
    write_output(&staged, "t001", &[math_draft()]);
    write_output(&staged, "t002", &[second]);
    write_output(&staged, "t003", &[concept_draft_for("企业边界", "§3", "企业自己完成的活动范围")]);
    ok_json(
        &zt.digest(&["term-cards", "-Step", "check", "-Staging", stage]),
        "check",
    );
    // The mathematical row still owes its independent review; approve it so the only unresolved
    // row is the caller's own draft.
    let review = ok_json(
        &zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]),
        "review-plan",
    );
    for job in review["jobs"].as_array().into_iter().flatten() {
        std::fs::write(review_answer_path(job), "{\"ok\":true}").expect("write the review answer");
    }
    ok_json(
        &zt.digest(&["term-cards", "-Step", "review-plan", "-Staging", stage]),
        "review-plan 2",
    );
    (holder, staged, list)
}

/// A concept draft for any row of the fixture list.
fn concept_draft_for(term: &str, section: &str, meaning: &str) -> Value {
    let mut draft = concept_draft();
    draft["term"] = json!(term);
    draft["first_defined"] = json!(section);
    draft["book_definition"]["section"] = json!(section);
    draft["book_definition"]["text"] = json!([format!("{section} 把它定义为{meaning}。")]);
    draft
}

#[test]
fn the_prefix_stops_at_the_first_unready_row_and_a_repair_continues_it_in_order() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    // Row 2's draft is not ready: its `first_defined` is missing, so the draft schema refuses it.
    let mut faulty = concept_draft();
    faulty["first_defined"] = json!("");
    faulty["book_definition"]["section"] = json!("");
    let (_holder, staged, list) = three_row_book(&zt, &mut mcp, faulty);
    let readiness = read_json(&work(&staged).join("check").join("readiness.json"));
    assert_eq!(readiness["terms"][1]["status"], "needs_fix", "{readiness}");

    let revision = mcp.revision();
    let applied = ok_json(&digest(&zt, &staged, "apply", true), "apply");
    assert_eq!(applied["complete"], false);
    assert_eq!(applied["committed"], 1, "{applied}");
    assert_eq!(applied["ready_unpublished"], 1, "row 3 stays staged: {applied}");
    assert_eq!(applied["refused"], 1);
    assert_eq!(applied["code"], "not_ready");
    assert_eq!(applied["stopped"], 2, "{applied}");
    // Exactly row 1's head was published: row 2 has no placeholder and row 3 waits in order.
    let sides = side_children(&mut mcp, "testbook2026", &list);
    assert_eq!(sides.len(), 1, "{sides:?}");
    assert_eq!(sides[0]["term"], "均衡");
    let json = read_json(&work(&staged).join("report.json"));
    assert_eq!(json["rows"][1]["publication"]["status"], "refused");
    assert_eq!(json["rows"][1]["publication"]["head"], Value::Null);
    assert_eq!(json["rows"][2]["publication"]["status"], "ready_unpublished");
    assert_eq!(json["staged"].as_array().map_or(0, Vec::len), 1);
    let md = std::fs::read_to_string(work(&staged).join("report.md")).unwrap();
    assert!(md.contains("Stopped: not_ready"), "{md}");

    // The writer repairs row 2. The next check makes it ready and the next apply publishes rows 2
    // and 3 in table order, after the row 1 head that is already accepted.
    write_output(
        &staged,
        "t002",
        &[concept_draft_for("剩余控制权", "§2", "合同没有写明的决定权")],
    );
    let checked = ok_json(
        &zt.digest(&["term-cards", "-Step", "check", "-Staging", staged.to_str().unwrap()]),
        "check 2",
    );
    assert_eq!(checked["counts"]["ready"], 3, "{checked}");
    let applied = ok_json(&digest(&zt, &staged, "apply", true), "apply 2");
    assert_eq!(applied["complete"], true, "{applied}");
    assert_eq!(applied["cards_written"].as_u64().unwrap() >= 2, true, "{applied}");
    assert!(mcp.revision() > revision);
    let sides = side_children(&mut mcp, "testbook2026", &list);
    let terms: Vec<String> = sides
        .iter()
        .map(|card| card["term"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(terms, ["均衡", "剩余控制权", "企业边界"], "{sides:?}");
}

#[test]
fn an_unrelated_side_child_stops_publication_without_creating_a_head() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (_holder, staged, list) = ready_book(&zt, &mut mcp);
    // Someone else filed a note beside the list: it occupies the first row's slot.
    mcp.call(
        "create_successor",
        json!({"at":list,"kind":"side","title":"一条笔记","body":"这不是术语卡。"}),
    );
    let revision = mcp.revision();
    let output = digest(&zt, &staged, "apply", true);
    assert_eq!(output.status.code(), Some(1), "a conflict is the operator's stop");
    let said = last_json(&output);
    assert_eq!(said["complete"], false);
    assert_eq!(said["conflicted"], 1, "{said}");
    assert_eq!(said["code"], "conflicted", "{said}");
    assert_eq!(said["cards_written"], 0, "no head is inserted beside the note: {said}");
    assert_eq!(mcp.revision(), revision, "a conflict writes no Card");
    let json = read_json(&work(&staged).join("report.json"));
    assert_eq!(json["rows"][0]["publication"]["status"], "conflicted");
    assert_eq!(json["rows"][0]["publication"]["reason"]["code"], "unrelated_side");
    assert_eq!(
        json["rows"][0]["publication"]["reason"]["address"],
        json!(format!("{list}|a"))
    );
    assert_eq!(side_children(&mut mcp, "testbook2026", &list).len(), 1);
}

#[test]
fn a_changed_accepted_head_is_a_conflict_and_is_never_overwritten() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (_holder, staged, list) = ready_book(&zt, &mut mcp);
    ok_json(&digest(&zt, &staged, "apply", true), "apply");
    let head = side_children(&mut mcp, "testbook2026", &list)[0]["address"]
        .as_str()
        .unwrap()
        .to_string();
    let published = mcp.card(&head);
    // The owner edits the accepted head by hand.
    let edited = published["body"]
        .as_str()
        .unwrap()
        .replace("来源：", "编辑过。\n\n来源：");
    mcp.call(
        "edit_card",
        json!({"at": head, "body": edited, "base": published["text"]}),
    );
    let revision = mcp.revision();

    let output = digest(&zt, &staged, "apply", true);
    assert_eq!(output.status.code(), Some(1));
    let said = last_json(&output);
    assert_eq!(said["conflicted"], 1, "{said}");
    assert_eq!(said["cards_written"], 0);
    assert_eq!(mcp.revision(), revision, "the accepted head is not rewritten");
    assert_eq!(
        mcp.card(&head)["body"].as_str().unwrap(),
        edited,
        "the owner's edit stays exactly as authored"
    );
    let json = read_json(&work(&staged).join("report.json"));
    assert_eq!(json["rows"][0]["publication"]["reason"]["code"], "changed_head");
}

#[test]
fn a_legacy_table_is_retyped_metadata_only_before_its_heads() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let list = legacy_book(&mut mcp);
    let before = mcp.card(&list);
    assert_eq!(before["type"], "idea", "the fixture is the pre-term table");
    let holder = tempfile::tempdir().expect("work");
    let staged = staging(holder.path(), "book", "testbook2026");
    let (checked, readiness) = ready_with(&staged, &zt, math_draft());
    assert_eq!(checked["counts"]["needs_review"], 1, "{checked}");
    assert_eq!(readiness["complete"], true, "{readiness}");
    let plan = read_json(&work(&staged).join("plan.json"));
    assert_eq!(plan["list"]["legacy"], true);
    assert_eq!(plan["list"]["type"], "idea");

    let applied = ok_json(&digest(&zt, &staged, "apply", true), "apply");
    assert_eq!(applied["complete"], true, "{applied}");
    assert_eq!(applied["retyped"], true);
    // The retype is metadata only: the same address, title, body and Links, and a Term type.
    let after = mcp.card(&list);
    assert_eq!(after["type"], "term");
    assert_eq!(after["title"], before["title"]);
    assert_eq!(after["body"], before["body"]);
    assert_eq!(after["text"], before["text"]);
    assert_eq!(mcp.card(&list)["address"], json!(list));
    assert_eq!(side_children(&mut mcp, "testbook2026", &list).len(), 2);
    let json = read_json(&work(&staged).join("report.json"));
    assert_eq!(json["list"]["retyped"], true);
    assert_eq!(json["list"]["observed"]["from"], "idea");
    assert_eq!(json["list"]["observed"]["to"], "term");
    assert_eq!(json["list"]["observed"]["body_sha256"], json!(plan["list"]["body_sha256"]));

    // A resume after the retype re-reads a Term list; the frozen plan keeps its revision and hash
    // and is reused byte for byte, because the list's kind is no list content. The publication
    // steps always read the list's current kind themselves, so no step trusts the frozen one.
    let again = ok_json(
        &zt.digest(&["term-cards", "-Step", "plan", "-Staging", staged.to_str().unwrap()]),
        "plan 2",
    );
    assert_eq!(again["revision"], plan["revision"], "the retype is no new plan revision");
    assert_eq!(again["plan_hash"], plan["plan_hash"]);
    let replanned = read_json(&work(&staged).join("plan.json"));
    assert_eq!(replanned["list"]["legacy"], true, "the frozen read is kept as it was frozen");
    let report = ok_json(&digest(&zt, &staged, "report", false), "report");
    assert_eq!(report["complete"], true, "{report}");
    let coverage = read_json(&work(&staged).join("report.json"));
    assert_eq!(coverage["list"]["type"], "term", "the report reads the list as it is now");
    assert_eq!(coverage["list"]["retyped"], false, "a resumed publication retypes nothing");
}

#[test]
fn a_lost_report_is_recovered_by_reading_the_archive_not_by_publishing_again() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (_holder, staged, list) = ready_book(&zt, &mut mcp);
    ok_json(&digest(&zt, &staged, "apply", true), "apply");
    let revision = mcp.revision();
    let heads = side_children(&mut mcp, "testbook2026", &list).len();
    // A crash after the batch committed and before the artifacts were written loses the derived
    // state and the report; the archive is the only evidence left.
    std::fs::remove_file(work(&staged).join("state.json")).expect("drop state");
    std::fs::remove_file(work(&staged).join("report.json")).expect("drop report");

    let recovered = ok_json(&digest(&zt, &staged, "apply", true), "apply 2");
    assert_eq!(recovered["complete"], true, "{recovered}");
    assert_eq!(recovered["cards_written"], 0, "no second head: {recovered}");
    assert_eq!(recovered["batches"], 0);
    assert_eq!(mcp.revision(), revision, "the recovery writes no Card");
    assert_eq!(side_children(&mut mcp, "testbook2026", &list).len(), heads);
    let state = read_json(&work(&staged).join("state.json"));
    assert_eq!(state["published"]["rows"]["1"]["head"]["address"], heads_address(&mut mcp, &list, 0));
}

fn heads_address(mcp: &mut Mcp, list: &str, index: usize) -> Value {
    side_children(mcp, "testbook2026", list)[index]["address"].clone()
}

#[test]
fn a_changed_list_invalidates_the_frozen_plan_before_any_write() {
    let program = zt_program().expect(
        "this seam needs a zt binary: build the repository (cargo build) or set ZT_TEST_ZT",
    );
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let (_holder, staged, list) = ready_book(&zt, &mut mcp);
    let before = mcp.card(&list);
    let edited = before["body"]
        .as_str()
        .unwrap()
        .replace("双方都不改变选择的状态", "双方都不愿改变选择的状态");
    mcp.call(
        "edit_card",
        json!({"at": list, "body": edited, "base": before["text"]}),
    );
    let revision = mcp.revision();
    let output = digest(&zt, &staged, "apply", true);
    assert_eq!(output.status.code(), Some(1));
    let said = last_json(&output);
    assert_eq!(said["code"], "plan_stale", "{said}");
    assert!(
        said["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason.as_str().unwrap_or_default().contains("list.body_sha256")),
        "{said}"
    );
    assert_eq!(mcp.revision(), revision, "a stale plan writes no Card");
    assert!(side_children(&mut mcp, "testbook2026", &list).is_empty());
    assert!(
        !work(&staged).join("report.json").exists(),
        "a stale plan publishes no report"
    );

    // The report step refuses the same plan with the same typed answer.
    let reported = digest(&zt, &staged, "report", false);
    assert_eq!(reported.status.code(), Some(1));
    assert_eq!(last_json(&reported)["code"], "plan_stale");
}







