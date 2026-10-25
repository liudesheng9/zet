//! Term content and the T rules (S09, #152): the proposal's draft JSON renders into a real
//! head/part chain, the live write gate applies the same T1–T10 evaluator to the stored Card, a
//! manually edited fault is refused with the diagnostic the draft check reports, and the book
//! audit reports the T8 coverage and table-shape facts.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

struct TestHome {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl TestHome {
    /// A configured archive with its service up. `stage` is the write-gate stage; the default of
    /// a fresh archive is advisory.
    fn new(stage: &str) -> Self {
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
            home.archive_root.to_str().unwrap(),
        ]);
        home.run(&["up"]);
        home.run(&["config", "set", "gate.stage", stage]);
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

    fn db(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db")
    }

    fn text(&self, at: &str) -> String {
        self.db()
            .query_row(
                "SELECT text FROM cards WHERE location = ?1 OR citation_key = ?1",
                [at],
                |row| row.get(0),
            )
            .expect("stored text")
    }

    fn revision(&self) -> i64 {
        self.db()
            .query_row(
                "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0)
    }

    fn cards(&self) -> i64 {
        self.db()
            .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
            .expect("card count")
    }

    fn mcp(&self) -> Mcp {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
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
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "zt-term-tests", "version": "1" } }),
        );
        mcp.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }
}

impl Drop for TestHome {
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
        let reply: Value = serde_json::from_str(&line).expect("MCP reply is JSON");
        assert_eq!(reply["id"], id, "replies arrive in request order");
        reply
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

    fn err(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(
            is_error,
            "{name}({arguments}) should fail but returned {value}"
        );
        value
    }
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

/// One agent Literature tree `key` whose anchor holds a Term list at `<key>/1|a`.
fn book(home: &TestHome, mcp: &mut Mcp, key: &str, table: &str) -> (String, String) {
    let _ = home;
    mcp.ok(
        "create_literature",
        json!({ "bibtex": format!("@book{{{key}, title={{Test Book}}}}") }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({ "at": key, "title": "第一章", "body": "设置。" }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({
            "at": anchor,
            "kind": "side",
            "title": "术语表：测试",
            "type": "term",
            "body": table,
        }),
    ));
    (anchor, list)
}

fn rows() -> Vec<String> {
    vec!["均衡".to_string(), "弹性".to_string()]
}

fn math_draft() -> Value {
    json!({
        "term": "均衡",
        "original": "equilibrium",
        "category": "math",
        "first_defined": "§1.1",
        "explanation": [
            "均衡是双方都不改变选择的状态。",
            "本书用它说明市场的稳定点。",
            "均衡不等于最优。"
        ],
        "book_definition": { "text": ["§1.1 把均衡定义为一个不动点。"], "section": "§1.1", "quote": "" },
        "hypotheses": [
            { "id": "H1", "text": "策略集是紧凸集。" },
            { "id": "H2", "text": "收益函数连续。" }
        ],
        "definition": {
            "formula": "x^* \\in \\arg\\max_{x} u(x) \\tag{T1}",
            "reading": ["$x^*$ 是没有人愿意偏离的策略。"]
        },
        "symbols": [
            { "symbol": "$x^*$", "meaning": "均衡策略", "type": "紧凸集", "auxiliary": false },
            { "symbol": "$u$", "meaning": "收益函数", "type": "函数", "auxiliary": false }
        ],
        "equivalent_forms": [ { "formula": "u(x^*)\\ge u(x)", "reason": "由 (T1) 直接得到。" } ],
        "derivations": [
            {
                "claim": "均衡是纳什均衡",
                "steps": [
                    { "text": "起点是定义 (T1)。", "basis": "定义 (T1)", "formula": "x^*=x" },
                    { "text": "两边比较收益。", "basis": "H2", "formula": "u(x^*)\\ge u(x)" }
                ],
                "result": "均衡是纳什均衡。"
            }
        ],
        "limits": [ { "case": "无", "text": "本定义没有需要说明的极限情形。", "formula": "" } ],
        "example": ["取 $u(x)=-x^2$。", "唯一均衡是 $x^*=0$。"],
        "intuition": ["没有人愿意单方面偏离。", "均衡是相互一致的选择。"],
        "distinctions": [ { "term": "弹性", "text": "弹性度量反应程度。" } ],
        "misreadings": ["均衡不是最优。"],
        "role_in_book": ["本书第一章用均衡定义市场结果。"]
    })
}

fn concept_draft() -> Value {
    json!({
        "term": "均衡",
        "category": "concept",
        "first_defined": "§1.1",
        "explanation": ["均衡是双方都不改变选择的状态。", "本书用它说明市场的稳定点。", "均衡不等于最优。"],
        "book_definition": { "text": ["§1.1 把均衡定义为一个不动点。"], "section": "§1.1", "quote": "" },
        "distinctions": [ { "term": "弹性", "text": "弹性度量反应程度。" } ],
        "role_in_book": ["本书第一章用均衡定义市场结果。"]
    })
}

fn term_row() -> zt_check::text::TermRow {
    zt_check::text::TermRow {
        term: "均衡".to_string(),
        avoid: vec!["平衡".to_string()],
        meaning: "双方都不改变策略".to_string(),
        defined: "§1.1".to_string(),
    }
}

fn render(draft: &Value, head: &str) -> zt_check::term::Rendered {
    let draft = zt_check::term::parse_draft(draft, &rows()).expect("a valid draft");
    zt_check::term::render(
        &draft,
        &term_row(),
        "Test2026",
        head,
        &zt_check::core::Limits::default(),
    )
    .expect("renders")
}

/// Create a named Term Card under `at`.
fn create(mcp: &mut Mcp, at: &str, term: &str, title: &str, body: &str, kind: &str) -> Value {
    mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": kind, "title": title, "type": "term", "term": term, "body": body }),
    )
}

/// The `(rule, fault, key)` triples of one refusal's structured diagnostics.
fn diagnostics(value: &Value) -> Vec<(String, String, String)> {
    value["diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|diagnostic| {
            (
                diagnostic["rule"].as_str().unwrap_or_default().to_string(),
                diagnostic["fault"].as_str().unwrap_or_default().to_string(),
                diagnostic["key"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn has_fault(value: &Value, rule: &str, fault: &str) -> bool {
    diagnostics(value)
        .iter()
        .any(|(known_rule, known_fault, _)| known_rule == rule && known_fault == fault)
}

/// The check entry a live read would hand the checker for this Term Card.
fn term_entry(title: &str, body: &str) -> Value {
    json!({
        "address": "Test2026/1|a|a",
        "type": "term",
        "term": "均衡",
        "term_role": "card",
        "title": title,
        "body": body,
        "term_rows": rows(),
    })
}

#[test]
fn a_rendered_math_head_passes_the_gate_and_a_manual_fault_gets_the_same_diagnostic() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let rendered = render(&math_draft(), "Test2026/1|a|a");
    let head = create(
        &mut mcp,
        &list,
        "均衡",
        &rendered.head.title,
        &rendered.head.body,
        "side",
    );
    let at = address(head.clone());
    assert_eq!(head["type"], json!("term"));
    assert!(head["check"]["stage"] == json!("enforce"));
    // Only the approximate T4 advice remains (if any).
    let warnings = head["warnings"].as_array().cloned().unwrap_or_default();
    assert!(
        warnings
            .iter()
            .all(|warning| warning["rule"] == json!("T4")),
        "{warnings:#?}"
    );
    let card = mcp.ok("get_card", json!({ "at": &at }));
    assert_eq!(card["term_role"], json!("card"));
    assert_eq!(card["type"], json!("term"));

    // A manual edit that drops a basis must report the T5 fault the same evaluator reports for
    // the draft's own rendered text.
    let faulty = rendered.head.body.replace("依据：定义 (T1)", "依据：");
    let draft_report =
        zt_check::core::Check::new(zt_check::core::Limits::default(), false, "")
            .report(&[term_entry(&rendered.head.title, &faulty)]);
    let expected = draft_report
        .iter()
        .find(|diagnostic| diagnostic.rule == "T5" && diagnostic.fault == "missing_basis")
        .expect("the draft check reports the missing basis");
    let before = home.text(&at);
    let revision = home.revision();
    let cards = home.cards();
    let refusal = mcp.err("edit_card", json!({ "at": &at, "body": &faulty }));
    assert_eq!(refusal["error"], json!("card_check"));
    let reported = diagnostics(&refusal);
    assert!(
        reported
            .iter()
            .any(|(rule, fault, key)| rule == "T5"
                && fault == "missing_basis"
                && key == &expected.key),
        "the gate reports the same T5 diagnostic: {reported:#?} vs {}",
        expected.key
    );
    assert_eq!(home.text(&at), before, "a refusal stores nothing");
    assert_eq!(home.revision(), revision);
    assert_eq!(home.cards(), cards);
}

#[test]
fn a_math_part_publishes_with_its_continuation_link_and_inherits_the_definition() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let mut draft = math_draft();
    draft["derivations"] = json!([ {
        "claim": "均衡是纳什均衡",
        "steps": (0..24).map(|index| json!({
            "text": format!("第 {} 步做一次变换。", index + 1),
            "basis": "代数",
            "formula": format!("x_{{{}}}=1", index + 1),
        })).collect::<Vec<_>>(),
        "result": "均衡是纳什均衡。"
    } ]);
    let rendered = render(&draft, "@@term_head");
    assert_eq!(rendered.parts.len(), 1);
    let head = create(
        &mut mcp,
        &list,
        "均衡",
        &rendered.head.title,
        &rendered.head.body,
        "side",
    );
    let head_at = address(head);
    // The part references the head by address; the draft used the batch reference.
    let part_body = rendered.parts[0].body.replace("@@term_head", &head_at);
    let part = create(
        &mut mcp,
        &head_at,
        "均衡",
        &rendered.parts[0].title,
        &part_body,
        "direct",
    );
    let part_at = address(part);
    let card = mcp.ok("get_card", json!({ "at": &part_at }));
    assert_eq!(card["term_role"], json!("part"));
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    let parts = terms["parts"].as_array().expect("parts");
    assert_eq!(parts.len(), 1, "{terms:#?}");
    assert_eq!(parts[0]["address"], json!(part_at));
    assert_eq!(parts[0]["head"], json!(head_at));
    // The part inherits the head's mathematical definition: it carries no `## 数学定义` itself.
    assert!(!part_body.contains("## 数学定义"), "{part_body}");
    // A part whose category contradicts its head is a T3 chain fault.
    let wrong = part_body.replace("类别：数学概念", "类别：概念");
    let refusal = mcp.err("edit_card", json!({ "at": &part_at, "body": &wrong }));
    assert!(has_fault(&refusal, "T3", "chain_category"), "{refusal:#?}");
}

#[test]
fn a_concept_term_refuses_a_formal_section_and_the_auto_link_opt_out_is_validated() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let mut concept = concept_draft();
    concept["link"] = json!(false);
    concept["link_reason"] = json!("这是过于一般的词。");
    let rendered = render(&concept, "Test2026/1|a|a");
    assert!(rendered.head.body.contains("自动链接：否\n自动链接理由：这是过于一般的词。"));
    let at = address(create(
        &mut mcp,
        &list,
        "均衡",
        &rendered.head.title,
        &rendered.head.body,
        "side",
    ));
    // A concept term that gains a derivation section is refused.
    let with_derivation = rendered.head.body.replace(
        "## 区别",
        "## 推导：均衡\n\n1. 起点。依据：定义。\n\n结果：均衡。\n\n## 区别",
    );
    let refusal = mcp.err("edit_card", json!({ "at": &at, "body": &with_derivation }));
    assert!(
        has_fault(&refusal, "T3", "formal_section_on_concept"),
        "{refusal:#?}"
    );
    // A stored opt-out without its reason is refused by T1.
    let without_reason = rendered
        .head
        .body
        .replace("自动链接理由：这是过于一般的词。\n", "");
    let refusal = mcp.err("edit_card", json!({ "at": &at, "body": &without_reason }));
    assert!(has_fault(&refusal, "T1", "auto_link_reason"), "{refusal:#?}");
    // `自动链接：是` is not a recognized opt-out value.
    let wrong_value = rendered.head.body.replace("自动链接：否", "自动链接：是");
    let refusal = mcp.err("edit_card", json!({ "at": &at, "body": &wrong_value }));
    assert!(has_fault(&refusal, "T1", "auto_link_value"), "{refusal:#?}");
}

#[test]
fn a_skipped_step_phrase_is_refused_and_a_named_step_is_allowed() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let rendered = render(&concept_draft(), "Test2026/1|a|a");
    let at = address(create(
        &mut mcp,
        &list,
        "均衡",
        "均衡",
        &rendered.head.body,
        "side",
    ));
    let skipped = rendered
        .head
        .body
        .replace("均衡不等于最优。", "显然均衡成立。");
    let refusal = mcp.err("edit_card", json!({ "at": &at, "body": &skipped }));
    assert!(has_fault(&refusal, "T6", "skipped_step"), "{refusal:#?}");
    let named = rendered.head.body.replace(
        "均衡不等于最优。",
        "由步骤 1，同理可得均衡成立。",
    );
    mcp.ok("edit_card", json!({ "at": &at, "body": &named }));
}

#[test]
fn a_symbol_gap_is_advice_only_and_the_write_is_stored() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let rendered = render(&math_draft(), "Test2026/1|a|a");
    let body = rendered
        .head
        .body
        .replace("u(x^*)\\ge u(x)$$", "\\zeta(x^*)\\ge u(x)$$");
    let accepted = create(&mut mcp, &list, "均衡", "均衡", &body, "side");
    let warnings = accepted["warnings"].as_array().cloned().unwrap_or_default();
    assert!(
        warnings
            .iter()
            .any(|warning| warning["rule"] == json!("T4")),
        "the approximate symbol gap is advice and the write is stored: {warnings:#?}"
    );
    assert_eq!(accepted["check"]["stage"], json!("enforce"));
}

#[test]
fn the_book_audit_reports_t8_coverage_and_table_shape_faults() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let table = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n| 利率 | 利息 | 资金的价格 | §3.1 | 多出来的一列 |\n\n<!-- 隐藏的注释 -->\n";
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", table);
    // Two heads for one row: the row coverage fault is a book audit fact, not a write refusal.
    let rendered = render(&math_draft(), "Test2026/1|a|a");
    create(
        &mut mcp,
        &list,
        "均衡",
        &rendered.head.title,
        &rendered.head.body,
        "side",
    );
    create(
        &mut mcp,
        &list,
        "均衡",
        &rendered.head.title,
        &rendered.head.body,
        "side",
    );
    // A head with no row can only exist through the advisory stage (an enforced write would
    // refuse it by T2); create it there, then enforce again and audit.
    home.run(&["config", "set", "gate.stage", "advisory"]);
    create(&mut mcp, &list, "不在表里", "不在表里", "术语：不在表里", "side");
    home.run(&["config", "set", "gate.stage", "enforce"]);
    let terms = mcp.ok("get_terms", json!({ "at": &list }));
    let faults = terms["faults"].as_array().expect("faults");
    let has = |fault: &str| {
        faults.iter().any(|row| row["fault"] == json!(fault))
    };
    assert!(has("term_duplicate"), "{faults:#?}");
    assert!(has("term_row_without_card"), "{faults:#?}");
    assert!(has("term_card_without_row"), "{faults:#?}");
    assert!(has("extra_column"), "{faults:#?}");
    assert!(has("hidden_comment"), "{faults:#?}");
    // The T8 coverage faults carry their rule id, so the audit can group them.
    for row in faults {
        if ["term_duplicate", "term_row_without_card", "term_card_without_row"]
            .contains(&row["fault"].as_str().unwrap_or_default())
        {
            assert_eq!(row["rule"], json!("T8"), "{row:#?}");
        }
    }
}

#[test]
fn a_term_use_on_an_ordinary_card_is_t7_advice() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let rendered = render(&math_draft(), "Test2026/1|a|a");
    let head_at = address(create(
        &mut mcp,
        &list,
        "均衡",
        &rendered.head.title,
        &rendered.head.body,
        "side",
    ));
    // A Model Card carries a Term use: a mismatched label is advice, not a refusal, and the
    // write is stored with the T7 warning.
    let model = mcp.ok(
        "create_successor",
        json!({
            "at": anchor,
            "title": "均衡模型",
            "type": "model",
            "body": format!("说明 [[平衡@{head_at}]] 的用法。"),
        }),
    );
    let warnings = model["warnings"].as_array().cloned().unwrap_or_default();
    assert!(
        warnings
            .iter()
            .any(|warning| warning["rule"] == json!("T7") && warning["fault"] == json!("label_mismatch")),
        "{warnings:#?}"
    );
    // The matching label raises no T7 advice.
    let clean = mcp.ok(
        "create_successor",
        json!({
            "at": anchor,
            "kind": "side",
            "title": "均衡的另一处用法",
            "type": "model",
            "body": format!("说明 [[均衡@{head_at}]] 的用法。"),
        }),
    );
    let warnings = clean["warnings"].as_array().cloned().unwrap_or_default();
    assert!(
        warnings.iter().all(|warning| warning["rule"] != json!("T7")),
        "{warnings:#?}"
    );
}

#[test]
fn a_manual_over_prose_card_is_refused_by_t9() {
    let home = TestHome::new("enforce");
    let mut mcp = home.mcp();
    let (_anchor, list) = book(&home, &mut mcp, "Test2026", TABLE);
    let rendered = render(&concept_draft(), "Test2026/1|a|a");
    let at = address(create(
        &mut mcp,
        &list,
        "均衡",
        "均衡",
        &rendered.head.body,
        "side",
    ));
    let long = "甲".repeat(1700);
    let body = rendered
        .head
        .body
        .replace("均衡不等于最优。", &format!("{long}。"));
    let refusal = mcp.err("edit_card", json!({ "at": &at, "body": &body }));
    assert!(has_fault(&refusal, "T9", "prose_size"), "{refusal:#?}");
}
