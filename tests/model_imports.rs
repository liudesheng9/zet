//! Shared building blocks between models (#205): the `**引入**` section of a Model Card, the
//! blocks and closures it imports, and the reports that name them, over MCP and the GUI.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
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
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_dir = tmp.path().join("config");
        let archive_root = tmp.path().join("archive");
        let home = Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        };
        home.cmd()
            .args([
                "config",
                "set",
                "archive_root",
                home.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
        home.cmd().arg("up").assert().success();
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn facet(&self, address: &str) -> Option<String> {
        Connection::open(self.archive_root.join("zt.sqlite3"))
            .expect("open db")
            .query_row(
                "SELECT model_facet FROM cards WHERE location = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card facet")
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
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "zt-tests", "version": "1" } }),
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
        serde_json::from_str(&line).expect("MCP reply is JSON")
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

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(is_error, "{name}({arguments}) should fail: {value}");
        value["error"].as_str().expect("error message").to_string()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Gui {
    child: Child,
    port: u16,
    token: String,
}

impl Gui {
    fn start(home: &TestHome) -> Self {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("gui")
            .env("ZT_CONFIG_DIR", &home.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt gui");
        let mut stdout = BufReader::new(child.stdout.take().expect("gui stdout"));
        let mut first_line = String::new();
        stdout.read_line(&mut first_line).expect("gui url line");
        let port: u16 = first_line
            .trim()
            .strip_prefix("zt gui: http://127.0.0.1:")
            .and_then(|rest| rest.strip_suffix('/'))
            .expect("gui prints its URL")
            .parse()
            .expect("port");
        let mut gui = Self {
            child,
            port,
            token: String::new(),
        };
        let (_, _, page) = gui.raw("GET", "/", None);
        gui.token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("token")
            .to_string();
        gui
    }

    /// Status code, headers, and body of one request.
    fn raw(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String, String) {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        let body = body.unwrap_or_default();
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            self.port,
            self.token,
            body.len()
        )
        .expect("send request");
        let mut response = Vec::new();
        stream.read_to_end(&mut response).expect("read response");
        let response = String::from_utf8_lossy(&response).to_string();
        let (head, body) = response.split_once("\r\n\r\n").expect("header end");
        let status = head
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse().ok())
            .expect("status");
        (status, head.to_ascii_lowercase(), body.to_string())
    }

    fn api(&self, method: &str, path: &str, body: Value) -> Value {
        let body = (method == "POST").then(|| body.to_string());
        let (_, _, text) = self.raw(method, path, body.as_deref());
        serde_json::from_str(&text).expect("JSON response")
    }
}

impl Drop for Gui {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Addresses of a model's `build.order`, with their lines.
fn build_order(model: &Value) -> Vec<(String, String)> {
    model["build"]["order"]
        .as_array()
        .expect("build order")
        .iter()
        .map(|entry| {
            (
                entry["address"].as_str().unwrap().to_string(),
                entry["line"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn faults(model: &Value, list: &str) -> Vec<(String, String)> {
    model["build"][list]
        .as_array()
        .unwrap_or_else(|| panic!("build.{list}"))
        .iter()
        .map(|fault| {
            (
                fault["from"].as_str().unwrap_or_default().to_string(),
                fault["to"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
    items
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// Model A (`K2024/1|a`): a setting and three steps, each resting on the one before.
struct Home {
    a: String,
    setting: String,
    steps: Vec<String>,
}

fn model_a(mcp: &mut Mcp) -> Home {
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{K2024, title={Shared Blocks}}" }),
    );
    mcp.ok("create_successor", json!({ "at": "K2024", "title": "第一章" }));
    mcp.ok(
        "create_successor",
        json!({ "at": "K2024/1", "title": "一张想法卡", "body": "不是模型的一部分。" }),
    );
    let a = address(mcp.ok(
        "create_successor",
        json!({ "at": "K2024/1", "kind": "side", "type": "model", "title": "模型 A", "body": "问题：A 问什么？" }),
    ));
    let setting = address(mcp.ok(
        "create_successor",
        json!({ "at": a, "kind": "side", "facet": "设定", "title": "设定：A 的设定", "body": "原始设定。" }),
    ));
    let mut steps = Vec::new();
    let mut before = setting.clone();
    for name in ["第一步", "第二步", "第三步"] {
        let at = address(mcp.ok(
            "create_successor",
            json!({ "at": before, "facet": name, "title": name, "body": format!("依赖 [[{before}]]。") }),
        ));
        before = at.clone();
        steps.push(at);
    }
    Home { a, setting, steps }
}

fn model_card(mcp: &mut Mcp, at: &str, title: &str, body: &str) -> String {
    address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "side", "type": "model", "title": title, "body": body }),
    ))
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("an array: {value}"))
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect()
}

/// I1 (#206): a `**引入**` line imports a part; a `（共用）` line in `结构` is a legacy share; any
/// other Link from the Model Card to another model's part still makes a member during the
/// transition but is a `false_share`; a line that names no part of another model is an
/// `import_target` fault.
#[test]
fn an_import_line_makes_a_member_and_a_stray_link_is_a_false_share() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let Home { a, steps, .. } = model_a(&mut mcp);
    let (p1, legacy, stray) = (&steps[0], &steps[1], &steps[2]);
    let b = model_card(
        &mut mcp,
        "K2024/1",
        "模型 B",
        &format!(
            "问题：B 问什么？\n\n**结构**\n\n- 共用 [[{legacy}]]（共用）：沿用的一步\n\n**引入**\n\n- 单卡 [[{p1}]]：第一步\n\n**关联**\n\n- 对照 [[{stray}]]：只是比较\n"
        ),
    );
    let b_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": b, "kind": "side", "facet": "设定", "title": "设定：B", "body": format!("依赖 [[{p1}]]。") }),
    ));
    let model = mcp.ok("get_model", json!({ "at": b }));
    let order = &model["build"]["order"];
    let entry = |at: &str| order.as_array().unwrap().iter().find(|e| e["address"] == at).cloned().unwrap_or_else(|| panic!("{at} in the build: {model}"));
    assert_eq!(entry(p1)["line"], "shared", "{model}");
    assert_eq!(entry(p1)["import"], json!(p1), "an imported part names its import: {model}");
    assert_eq!(entry(legacy)["line"], "shared");
    assert_eq!(entry(legacy)["legacy"], json!(true), "a （共用） line is a legacy share: {model}");
    assert_eq!(entry(stray)["line"], "shared", "a stray Link still makes a member during the transition");
    assert_eq!(entry(&b_setting)["line"], "main");
    let false_share = model["build"]["false_share"].as_array().expect("build.false_share");
    assert_eq!(false_share.len(), 1, "{model}");
    assert_eq!(false_share[0]["target"], json!(stray));
    assert!(false_share[0]["fix"].as_str().unwrap().contains("**引入**"), "{model}");
    let imports = model["imports"].as_array().expect("imports");
    assert_eq!(imports.len(), 2, "the import and the legacy share: {model}");
    assert_eq!(imports[0]["root"], json!(p1));
    assert_eq!(imports[0]["mode"], "card");
    assert_eq!(imports[0]["home"], json!(a));
    assert_eq!(strings(&imports[0]["members"]), [p1.clone()]);
    assert_eq!(imports[1]["root"], json!(legacy));
    assert_eq!(imports[1]["mode"], "legacy");
    assert!(model["build"]["import_target"].as_array().expect("build.import_target").is_empty(), "{model}");
}

/// I1 (#206): an `**引入**` line must name a part of another model (or a Model Card): a line to a
/// missing Card, to a Card that is not a model's, or to this model's own part is an
/// `import_target` fault, and it makes no import.
#[test]
fn an_import_line_that_names_no_part_of_another_model_is_a_fault() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let Home { steps, .. } = model_a(&mut mcp);
    let b = model_card(&mut mcp, "K2024/1", "模型 B", "问题：B？");
    let b_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": b, "kind": "side", "facet": "设定", "title": "设定：B", "body": "原始设定。" }),
    ));
    // zt refuses a Link to a missing Card, so the line names a Card that a later delete removes.
    let gone = address(mcp.ok(
        "create_successor",
        json!({ "at": steps[2], "facet": "第四步", "title": "第四步", "body": "一步。" }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": b, "body": format!(
            "问题：B？\n\n**引入**\n\n- 单卡 [[{gone}]]：将被删除\n- 单卡 [[K2024/2]]：一张想法卡\n- 单卡 [[{b_setting}]]：自己的设定\n- 单卡 [[{}]]：第一步\n", steps[0]
        ) }),
    );
    mcp.ok("delete_card", json!({ "at": gone, "confirm": "delete" }));
    let model = mcp.ok("get_model", json!({ "at": b }));
    let faults = model["build"]["import_target"].as_array().expect("import_target");
    let targets = faults.iter().map(|f| f["target"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(targets, [gone.as_str(), "K2024/2", b_setting.as_str()], "{model}");
    assert!(faults[0]["fix"].as_str().unwrap().contains("names no Card"), "{model}");
    assert!(faults[1]["fix"].as_str().unwrap().contains("not a Model Card or a Model part"), "{model}");
    assert!(faults[2]["fix"].as_str().unwrap().contains("filed in this model"), "{model}");
    let roots = model["imports"].as_array().unwrap().iter().map(|i| i["root"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(roots, [steps[0].clone()], "only the valid line imports: {model}");
    assert_eq!(model["build"]["self_sufficient"], json!(false), "an import fault keeps the model from being self-sufficient: {model}");
}

/// I1 (#206): the Links of the `**引入**` section never keep a Model Card from changing type; filed
/// parts still do.
#[test]
fn a_model_card_that_only_imports_can_change_type() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let Home { a, steps, .. } = model_a(&mut mcp);
    let c = model_card(
        &mut mcp,
        "K2024/1",
        "模型 C",
        &format!("问题：C？\n\n**引入**\n\n- 单卡 [[{}]]：第一步\n", steps[0]),
    );
    mcp.ok("edit_card", json!({ "at": c, "type": "idea" }));
    let error = mcp.err("edit_card", json!({ "at": a, "type": "idea" }));
    assert!(error.contains("change or move them first"), "{error}");
}

/// Model A grown for blocks: its setting `S` holds a notation card `S|a` and a setting `S|b` that
/// rests on `S`; the first step holds a result `R` (resting on the step and on `S|b`) with a proof
/// of two steps under it.
struct Blocks {
    a: String,
    setting: String,
    notation: String,
    timing: String,
    steps: Vec<String>,
    result: String,
    proof: Vec<String>,
}

fn model_a_with_blocks(mcp: &mut Mcp) -> Blocks {
    let Home { a, setting, steps } = model_a(mcp);
    let side = |mcp: &mut Mcp, at: &str, facet: &str, body: &str| {
        address(mcp.ok(
            "create_successor",
            json!({ "at": at, "kind": "side", "facet": facet, "title": facet, "body": body }),
        ))
    };
    let notation = side(mcp, &setting, "符号表与读法", "| 符号 | 读法 |\n|---|---|\n| $x$ | 产量 |");
    let timing = side(mcp, &setting, "设定：时点", &format!("依赖 [[{setting}]]。"));
    let result = side(mcp, &steps[0], "结果：一个结论", &format!("依赖 [[{}]] 与 [[{timing}]]。", steps[0]));
    let first = side(mcp, &result, "推导：第一段", &format!("依赖 [[{result}]]。"));
    let second = address(mcp.ok(
        "create_successor",
        json!({ "at": first, "facet": "推导：第二段", "title": "推导：第二段", "body": format!("依赖 [[{first}]]。") }),
    ));
    Blocks { a, setting, notation, timing, steps, result, proof: vec![first, second] }
}

/// I2 (#207): `整块 [[S]]` takes the setting `S` and the Cards under its Side successors (the
/// notation card, the setting tree), never its Direct successors (the chapters); the block comes in
/// as one run before the borrower's first step that rests on it.
#[test]
fn a_block_import_takes_the_setting_tree_and_not_the_chapters() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let k = model_a_with_blocks(&mut mcp);
    let b = model_card(
        &mut mcp,
        "K2024/1",
        "模型 B",
        &format!("问题：B？\n\n**引入**\n\n- 整块 [[{}]]：A 的设定\n", k.setting),
    );
    let b_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": b, "kind": "side", "facet": "设定", "title": "设定：B", "body": format!("依赖 [[{}]]。", k.timing) }),
    ));
    let model = mcp.ok("get_model", json!({ "at": b }));
    assert_eq!(
        build_order(&model),
        pairs(&[
            (&k.setting, "shared"),
            (&k.notation, "shared"),
            (&k.timing, "shared"),
            (&b_setting, "main"),
        ]),
        "the whole setting part, as one run before the step that rests on it, and no chapter: {model}"
    );
    let import = &model["imports"][0];
    assert_eq!(import["mode"], "block");
    assert_eq!(import["home"], json!(k.a));
    assert_eq!(strings(&import["members"]), [k.setting.clone(), k.notation.clone(), k.timing.clone()]);
    assert!(import["closure"].as_array().unwrap().is_empty(), "{model}");
    assert!(model["build"]["tolerated"].as_array().unwrap().is_empty(), "{model}");
    assert_eq!(model["build"]["self_sufficient"], json!(true), "{model}");
}

/// I2 (#207): `整块 [[R]]` on a result takes its proof, and the closure brings every part of the home
/// model the block rests on (the step, the setting tree it needs), premises first; nothing is
/// tolerated. A Card of the block names the borrower among its models, and the GUI reads it in the
/// borrower it came from.
#[test]
fn a_block_import_brings_its_closure_and_leaves_nothing_tolerated() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let k = model_a_with_blocks(&mut mcp);
    let c = model_card(
        &mut mcp,
        "K2024/1",
        "模型 C",
        &format!("问题：C？\n\n**引入**\n\n- 整块 [[{}]]：A 的结论与证明\n", k.result),
    );
    let c_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": c, "kind": "side", "facet": "设定", "title": "设定：C", "body": format!("依赖 [[{}]]。", k.proof[1]) }),
    ));
    let model = mcp.ok("get_model", json!({ "at": c }));
    assert_eq!(
        build_order(&model),
        pairs(&[
            (&k.setting, "shared"),
            (&k.steps[0], "shared"),
            (&k.timing, "shared"),
            (&k.result, "shared"),
            (&k.proof[0], "shared"),
            (&k.proof[1], "shared"),
            (&c_setting, "main"),
        ]),
        "premises first, then the block in its own order: {model}"
    );
    let import = &model["imports"][0];
    assert_eq!(strings(&import["members"]), [k.result.clone(), k.proof[0].clone(), k.proof[1].clone()]);
    let mut closure = strings(&import["closure"]);
    closure.sort();
    let mut want = vec![k.setting.clone(), k.steps[0].clone(), k.timing.clone()];
    want.sort();
    assert_eq!(closure, want, "{model}");
    assert!(model["build"]["tolerated"].as_array().unwrap().is_empty(), "nothing left behind: {model}");
    // The proof's last step lists C among its models; the GUI keeps C when the reader comes from it.
    let step = mcp.ok("get_model", json!({ "at": k.proof[1] }));
    let models = step["models"].as_array().unwrap().iter().map(|m| m["address"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(models.contains(&c), "{step}");
    let gui = Gui::start(&home);
    let esc = |at: &str| at.replace('/', "%2F").replace('|', "%7C");
    for at in [&k.proof[1], &k.timing] {
        let card = gui.api("GET", &format!("/api/card?at={}&via={}", esc(at), esc(&c)), json!(null));
        assert_eq!(card["model"]["head"]["address"], json!(c), "{at} read in the borrower: {card}");
    }
}

/// I2 (#207): an import whose block or closure rests on a Card of the borrower's own range is a
/// cycle: one `import_cycle` fault names the import and both Cards.
#[test]
fn an_import_that_rests_on_the_borrower_is_a_cycle() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let k = model_a_with_blocks(&mut mcp);
    let d = model_card(&mut mcp, "K2024/1", "模型 D", "问题：D？");
    let d_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": d, "kind": "side", "facet": "设定", "title": "设定：D", "body": "原始设定。" }),
    ));
    // A's third step grows a part that rests on D's setting; D imports it.
    let leaf = address(mcp.ok(
        "create_successor",
        json!({ "at": k.steps[2], "kind": "side", "facet": "推广", "title": "推广", "body": format!("依赖 [[{d_setting}]]。") }),
    ));
    mcp.ok(
        "edit_card",
        json!({ "at": d, "body": format!("问题：D？\n\n**引入**\n\n- 整块 [[{leaf}]]：A 的推广\n") }),
    );
    let model = mcp.ok("get_model", json!({ "at": d }));
    let cycle = model["build"]["import_cycle"].as_array().expect("import_cycle");
    assert_eq!(cycle.len(), 1, "{model}");
    assert_eq!(cycle[0]["import"], json!(leaf));
    assert_eq!(cycle[0]["from"], json!(leaf));
    assert_eq!(cycle[0]["to"], json!(d_setting));
    assert_eq!(model["build"]["self_sufficient"], json!(false), "{model}");
}


/// I3 (#208): `整块 [[<Model Card>]]` takes a whole model in: every part filed under that Model
/// Card is a shared member (the Model Card itself is not), each after its premises, and the parts
/// list the borrower among their models. A rests-on Link to the imported Model Card itself is still
/// a closure fault, whose fix says to rest on one of its parts.
#[test]
fn a_whole_model_is_a_building_block() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let k = model_a_with_blocks(&mut mcp);
    let e = model_card(
        &mut mcp,
        "K2024/1",
        "模型 E",
        &format!("问题：E？\n\n**引入**\n\n- 整块 [[{}]]：整个模型 A\n", k.a),
    );
    let e_setting = address(mcp.ok(
        "create_successor",
        json!({ "at": e, "kind": "side", "facet": "设定", "title": "设定：E", "body": format!("依赖 [[{}]]。依赖 [[{}]]。", k.steps[2], k.a) }),
    ));
    let model = mcp.ok("get_model", json!({ "at": e }));
    let order = build_order(&model);
    let shared = order.iter().filter(|(_, line)| line == "shared").map(|(at, _)| at.clone()).collect::<Vec<_>>();
    let mut want = vec![k.setting.clone(), k.notation.clone(), k.timing.clone(), k.result.clone(), k.proof[0].clone(), k.proof[1].clone()];
    want.extend(k.steps.iter().cloned());
    let mut got = shared.clone();
    got.sort();
    want.sort();
    assert_eq!(got, want, "every part of A, and not A's Model Card: {model}");
    assert_eq!(order.last().unwrap(), &(e_setting.clone(), "main".to_string()), "{model}");
    let pos = |at: &str| shared.iter().position(|x| x == at).unwrap();
    assert!(pos(&k.setting) < pos(&k.steps[0]) && pos(&k.steps[0]) < pos(&k.steps[2]), "premises first: {model}");
    assert!(pos(&k.steps[0]) < pos(&k.result) && pos(&k.result) < pos(&k.proof[1]), "premises first: {model}");
    let import = &model["imports"][0];
    assert_eq!(import["mode"], "model");
    assert_eq!(import["home"], json!(k.a));
    assert!(model["build"]["import_target"].as_array().unwrap().is_empty(), "{model}");
    let closure = model["build"]["closure"].as_array().unwrap();
    assert_eq!(closure.len(), 1, "only the Link to the Model Card itself: {model}");
    assert_eq!(closure[0]["to"], json!(k.a));
    assert!(closure[0]["fix"].as_str().unwrap().contains("which this model imports whole"), "{model}");
    assert!(model["build"]["tolerated"].as_array().unwrap().is_empty(), "{model}");
    let step = mcp.ok("get_model", json!({ "at": k.steps[2] }));
    let models = step["models"].as_array().unwrap().iter().map(|m| m["address"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(models, [k.a.clone(), e.clone()], "{step}");
}

/// I3 (#208): a whole model comes in with `整块`; `单卡 [[<Model Card>]]` is an `import_target`
/// fault and takes nothing in.
#[test]
fn a_single_card_import_of_a_model_card_is_a_fault() {
    let home = TestHome::new();
    let mut mcp = home.mcp();
    let Home { a, .. } = model_a(&mut mcp);
    let f = model_card(&mut mcp, "K2024/1", "模型 F", &format!("问题：F？\n\n**引入**\n\n- 单卡 [[{a}]]：整个 A？\n"));
    let model = mcp.ok("get_model", json!({ "at": f }));
    let faults = model["build"]["import_target"].as_array().unwrap();
    assert_eq!(faults.len(), 1, "{model}");
    assert!(faults[0]["fix"].as_str().unwrap().contains("整块"), "{model}");
    assert!(model["imports"].as_array().unwrap().is_empty(), "{model}");
    assert!(build_order(&model).is_empty(), "{model}");
}

