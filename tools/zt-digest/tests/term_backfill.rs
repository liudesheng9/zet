//! The Term backfill seam (S25, #168; design 14.3 and 16): `zt-digest prepare -Validate` and
//! `-Sections` inspect one already digested book before any provider work: its Workspace, its Term
//! list, its existing descendants and occupancy, its staged source and the owner's Lock. The
//! sections-only branch reuses (or reconstructs byte for byte) the staged source slices and binds
//! their fingerprints; it creates no root, anchor, group, model or narrative Card.
//!
//! The tests run over a real scratch archive and a real prepared run (the tool's own `prepare`
//! command), so the ledger, the run contract, the Literature tree and the staged source are the
//! ones a backfill later acts on. `ZT_TEST_ZT` overrides the `zt` binary, `ZT_TEST_DIGEST` the
//! `zt-digest` one.

use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};

fn zt_program() -> Option<PathBuf> {
    if let Ok(program) = std::env::var("ZT_TEST_ZT") {
        return Some(PathBuf::from(program));
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for candidate in [root.join("target/debug/zt.exe"), root.join("target/release/zt.exe")] {
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
        let program = std::env::var("ZT_TEST_DIGEST").unwrap_or_else(|_| env!("CARGO_BIN_EXE_zt-digest").to_string());
        Command::new(program)
            .args(args)
            .env("ZT_CONFIG_DIR", &self.config)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_CHECK_ZT", &self.program)
            .output()
            .expect("run zt-digest")
    }

    /// The running service's GUI: its port and the token the page carries.
    fn gui(&self) -> Gui {
        let said = self.run(&["gui"]);
        assert!(said.status.success(), "gui: {}", String::from_utf8_lossy(&said.stderr));
        let stdout = String::from_utf8_lossy(&said.stdout).into_owned();
        let port = stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
            .and_then(|rest| rest.trim_end_matches('/').parse().ok())
            .unwrap_or_else(|| panic!("no GUI URL: {stdout}"));
        let gui = Gui { port, token: String::new() };
        let page = gui.get("/");
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page carries the GUI token")
            .to_string();
        Gui { port, token }
    }
}

impl Drop for Zt {
    fn drop(&mut self) {
        let _ = self.run(&["down"]);
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn get(&self, path: &str) -> String {
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect gui");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nContent-Length: 0\r\n\r\n",
            self.port
        )
        .expect("write gui");
        let mut page = String::new();
        stream.read_to_string(&mut page).expect("read gui page");
        page
    }

    fn post(&self, path: &str, body: &Value) -> Value {
        let payload = body.to_string();
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect gui");
        write!(
            stream,
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            self.port,
            self.token,
            payload.len(),
            payload
        )
        .expect("write gui");
        let mut page = String::new();
        stream.read_to_string(&mut page).expect("read gui answer");
        let body = page.split("\r\n\r\n").nth(1).unwrap_or(&page);
        serde_json::from_str(body).unwrap_or_else(|error| panic!("{error}: {page}"))
    }

    /// The owner's two-step Lock: plan, then confirm its hash.
    fn lock(&self, root: &str) -> Value {
        let request = json!({ "root": root, "note": "the Term backfill seam" });
        let plan = self.post("/api/lock", &request);
        assert_eq!(plan["applied"], false, "the first request only plans: {plan}");
        let mut confirmed = request.clone();
        confirmed["confirm"] = json!("lock");
        confirmed["plan_hash"] = plan["plan"]["plan_hash"].clone();
        let applied = self.post("/api/lock", &confirmed);
        assert_eq!(applied["applied"], true, "the confirmation commits: {applied}");
        applied["lock"].clone()
    }

    fn unlock(&self, lock_id: i64) {
        let plan = self.post("/api/unlock", &json!({ "lock_id": lock_id }));
        assert_eq!(plan["applied"], false, "unlock plans first: {plan}");
        let applied = self.post(
            "/api/unlock",
            &json!({"lock_id": lock_id, "confirm": "unlock", "plan_hash": plan["plan"]["plan_hash"]}),
        );
        assert_eq!(applied["applied"], true, "unlock commits: {applied}");
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

/// The book's bilingual-looking source: two Chinese sections inside the digest range and one
/// outside it. A backfill must reconstruct exactly these slices.
const SOURCE: &str = "# 前言\n\n不算。\n\n# 导论\n\n本书研究均衡。\n\n## 第一节\n\n均衡定义为 $x^*=\\arg\\max_x u(x)$，它是没有人愿意偏离的策略。\n\n剩余控制权定义为合同没有写明的决定权。\n\n# 参考\n\n不进入正文。\n";

/// The pre-Term Term table: a `术语表` Idea Card beside the first anchor.
const TABLE: &str = "本卡是《Test Book》的术语表。本书的卡片用左列的术语，不用右列的词。\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变选择的状态 | §1 |\n| 剩余控制权 | 剩余索取权 | 合同没有写明的决定权 | §2 |\n\n来源：[[Key]]";

fn last_json(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_else(|| panic!("no output: {} {}", stdout, String::from_utf8_lossy(&output.stderr)));
    serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"))
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read artifact")).expect("JSON")
}

/// A real prepared run of `SOURCE`. `term_layers` false prepares the target a book digested before
/// the Term work had (no `termcards` or `termlink` layer), which is the old staging of the issue's
/// seam; true prepares the current target. Returns the staging folder.
fn prepared(zt: &Zt, dir: &Path, name: &str, term_layers: bool) -> PathBuf {
    let source = dir.join("source.md");
    std::fs::write(&source, SOURCE).expect("write source");
    let out = dir.join(name);
    let mut args = vec![
        "prepare",
        "-Source",
        source.to_str().unwrap(),
        "-Out",
        out.to_str().unwrap(),
        "-CitationKey",
        "Key",
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
    ];
    if !term_layers {
        args.extend(["-RunOptions", r#"{"termCards":false,"termLink":false}"#]);
    }
    args.extend(["-Apply", "-Brief"]);
    let output = zt.digest(&args);
    assert!(
        output.status.success(),
        "prepare: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    out
}

/// The table beside the book's first anchor.
fn add_list(mcp: &mut Mcp, body: &str) -> String {
    mcp.call(
        "create_successor",
        json!({"at":"Key/1","kind":"side","title":"术语表：《Test Book》的 2 个术语","body":body}),
    )["address"]
        .as_str()
        .expect("list address")
        .to_string()
}

// ==================================================================================================
// Validate: one already prepared book, its list and its staged source, without a Card write
// ==================================================================================================

#[test]
fn a_prepared_book_validates_its_list_source_and_fingerprints_without_writing_a_card() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    let list = add_list(&mut mcp, TABLE);
    let revision = mcp.revision();
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert!(
        output.status.success(),
        "validate: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(true), "{line}");
    assert_eq!(line["step"], json!("validate"));
    assert_eq!(line["book"], json!("Key"));
    assert_eq!(line["workspace"], json!("agent"));
    // The book's one list, read by role (the legacy 术语表 table), with its exact rows.
    assert_eq!(line["list"]["address"], json!(list));
    assert_eq!(line["list"]["legacy"], json!(true));
    assert_eq!(line["list"]["rows"], json!(2));
    assert_eq!(line["list"]["terms"], json!(["均衡", "剩余控制权"]));
    // Nothing is published beside it yet.
    assert_eq!(line["tree"]["heads"], json!(0));
    assert_eq!(line["tree"]["parts"], json!(0));
    assert_eq!(line["tree"]["occupying"], json!([]));
    // The staged source is reused, and its fingerprints are bound.
    assert_eq!(line["source"]["reused"], json!(true));
    assert_eq!(line["source"]["sections"], json!(2));
    assert!(line["source"]["sha256"].as_str().is_some_and(|sha| sha.len() == 64));
    assert!(line["list_sha256"].as_str().is_some_and(|sha| sha.len() == 64));
    assert_eq!(line["conflicts"], json!([]));
    assert_eq!(line["complete"], json!(true));
    assert_eq!(line["lock"], Value::Null);
    // Validation writes no Card and changes no revision.
    assert_eq!(mcp.revision(), revision, "validation must not write a Card");
    // The record on disk binds the same facts for the stages and the person who reviews them.
    let record = read_json(&staged.join("backfill.json"));
    assert_eq!(record["kind"], json!("backfill_preparation"));
    assert_eq!(record["step"], json!("validate"));
    assert_eq!(record["list"]["address"], json!(list));
    assert_eq!(record["list"]["sha256"], line["list_sha256"]);
    assert_eq!(record["source"]["sha256"], line["source"]["sha256"]);
    assert_eq!(record["source"]["reused"], json!(true));
    let manifest = read_json(&staged.join("manifest.json"));
    assert_eq!(manifest["backfill"]["list_sha256"], line["list_sha256"]);
}

// ==================================================================================================
// Validate: no Term list is a typed conflict before any provider work, and the run halts
// ==================================================================================================

#[test]
fn a_book_with_no_term_list_is_a_typed_conflict_and_writes_nothing() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    let revision = mcp.revision();
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert_eq!(output.status.code(), Some(1), "a conflict is not a crash: {}", String::from_utf8_lossy(&output.stderr));
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(false), "{line}");
    assert_eq!(line["code"], json!("backfill_conflict"));
    let conflicts = line["conflicts"].as_array().expect("conflicts");
    assert_eq!(conflicts.len(), 1, "{line}");
    assert_eq!(conflicts[0]["code"], json!("no_list"));
    assert!(conflicts[0]["message"].as_str().unwrap().contains("Term list"), "{line}");
    assert_eq!(line["complete"], json!(false));
    assert_eq!(mcp.revision(), revision, "a refused validation writes no Card");
    // The refused record keeps the conflict for the person who fixes the book.
    let record = read_json(&staged.join("backfill.json"));
    assert_eq!(record["complete"], json!(false));
    assert_eq!(record["conflicts"][0]["code"], json!("no_list"));
    // No Term-card plan is invented: the compiler was never dispatched.
    assert!(!staged.join("term-cards").exists());
}

// ==================================================================================================
// Validate: the owner's Lock stops the backfill and names the owner action
// ==================================================================================================

#[test]
fn a_locked_book_stops_validation_with_the_owner_action() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    add_list(&mut mcp, TABLE);
    let gui = zt.gui();
    let lock = gui.lock("Key");
    let lock_id = lock["lock_id"].as_i64().expect("lock id");
    // Taking the Lock is itself an archive write; the validation that follows must leave this
    // revision alone.
    let revision = mcp.revision();
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert_eq!(output.status.code(), Some(2), "a Lock is the typed exit 2: {} {}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(false), "{line}");
    assert_eq!(line["code"], json!("card_locked"));
    assert_eq!(line["error"], json!("the Literature tree Key is locked by the owner"));
    assert_eq!(line["tree"], json!("Key"));
    assert_eq!(line["lock_id"], json!(lock_id));
    let hint = line["hint"].as_str().unwrap();
    assert!(hint.contains("unlock the tree in the GUI"), "{hint}");
    assert!(!hint.contains("/api/") && !hint.contains("Access"), "{hint}");
    assert_eq!(mcp.revision(), revision, "a locked validation writes no Card");

    gui.unlock(lock_id);
    let again = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert!(again.status.success(), "unlocked: {} {}", String::from_utf8_lossy(&again.stdout), String::from_utf8_lossy(&again.stderr));
    assert_eq!(last_json(&again)["ok"], json!(true));
}

// ==================================================================================================
// Sections: reuse the prepared slices, and reconstruct a lost one byte for byte
// ==================================================================================================

fn bytes_of(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("book.zh.md".to_string(), std::fs::read(dir.join("book.zh.md")).expect("book")),
    ];
    let mut names: Vec<String> = std::fs::read_dir(dir.join("sections"))
        .expect("sections")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| !name.eq_ignore_ascii_case("index.md"))
        .collect();
    names.sort();
    for name in names {
        files.push((format!("sections/{name}"), std::fs::read(dir.join("sections").join(&name)).expect("section")));
    }
    files
}

#[test]
fn sections_are_reused_when_they_match_the_prepared_run() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    add_list(&mut mcp, TABLE);
    let before = bytes_of(&staged);
    let revision = mcp.revision();
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Sections", "-Brief"]);
    assert!(
        output.status.success(),
        "sections: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(true), "{line}");
    assert_eq!(line["step"], json!("sections"));
    assert_eq!(line["source"]["reused"], json!(true));
    assert_eq!(line["source"]["rebuilt"], json!([]));
    assert_eq!(line["source"]["sections"], json!(2));
    assert_eq!(line["source"]["bound"], json!(true));
    assert_eq!(line["conflicts"], json!([]));
    assert_eq!(bytes_of(&staged), before, "reused slices are left byte for byte");
    assert_eq!(mcp.revision(), revision, "the sections branch writes no Card");
}

#[test]
fn a_lost_section_and_book_text_are_reconstructed_byte_for_byte() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    add_list(&mut mcp, TABLE);
    let before = bytes_of(&staged);
    let revision = mcp.revision();
    // The staging lost one slice and the whole-book text (a copy, a cleanup or a crash).
    std::fs::remove_file(staged.join("sections").join("s02.md")).expect("remove section");
    std::fs::remove_file(staged.join("book.zh.md")).expect("remove book text");
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Sections", "-Brief"]);
    assert!(
        output.status.success(),
        "sections: {} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(true), "{line}");
    assert_eq!(line["source"]["reused"], json!(false));
    assert_eq!(line["source"]["rebuilt"], json!(["s02.md", "book.zh.md"]));
    assert_eq!(line["source"]["bound"], json!(true));
    assert_eq!(bytes_of(&staged), before, "the reconstruction reproduces the prepared bytes");
    assert_eq!(mcp.revision(), revision, "the reconstruction writes no Card");
}

#[test]
fn a_changed_source_is_a_conflict_and_leaves_the_staging_untouched() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    add_list(&mut mcp, TABLE);
    let before = bytes_of(&staged);
    let revision = mcp.revision();
    // The book Markdown moved on after the digest; a backfill must not silently stage another text.
    let source = staged.join("source.md");
    let source = if source.is_file() {
        source
    } else {
        dir.path().join("source.md")
    };
    let mut text = std::fs::read_to_string(&source).expect("source");
    text.push_str("\n补写的一段。\n");
    std::fs::write(&source, text).expect("rewrite source");
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Sections", "-Brief"]);
    assert_eq!(output.status.code(), Some(1), "a changed source is a conflict: {}", String::from_utf8_lossy(&output.stderr));
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(false), "{line}");
    assert_eq!(line["conflicts"][0]["code"], json!("source_changed"));
    assert_eq!(bytes_of(&staged), before, "a refused reconstruction rewrites no slice");
    assert_eq!(mcp.revision(), revision);
}

// ==================================================================================================
// Conflicts: an occupied slot, duplicate rows and a missing source are shown before dispatch
// ==================================================================================================

#[test]
fn an_occupying_side_child_is_a_conflict_before_provider_work() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    let list = add_list(&mut mcp, TABLE);
    // Someone filed a note beside the list: it occupies the first row's slot.
    mcp.call(
        "create_successor",
        json!({"at": list, "kind": "side", "title": "一条笔记", "body": "这不是术语卡。"}),
    );
    let revision = mcp.revision();
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert_eq!(output.status.code(), Some(1));
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(false), "{line}");
    assert_eq!(line["conflicts"][0]["code"], json!("occupying_children"));
    let message = line["conflicts"][0]["message"].as_str().unwrap();
    assert!(message.contains("not a named Term Card"), "{message}");
    assert_eq!(line["tree"]["heads"], json!(1));
    assert_eq!(line["tree"]["occupying"].as_array().unwrap().len(), 1);
    assert_eq!(mcp.revision(), revision);
}

#[test]
fn a_list_that_repeats_a_term_is_a_conflict() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    add_list(
        &mut mcp,
        "术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 状态 | §1 |\n| 均衡 | — | 又一次 | §2 |",
    );
    let revision = mcp.revision();
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert_eq!(output.status.code(), Some(1));
    let line = last_json(&output);
    assert_eq!(line["conflicts"][0]["code"], json!("duplicate_rows"), "{line}");
    assert!(line["conflicts"][0]["message"].as_str().unwrap().contains("均衡"), "{line}");
    assert_eq!(mcp.revision(), revision);
}

#[test]
fn a_missing_source_is_a_conflict() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let mut mcp = zt.mcp();
    let dir = tempfile::tempdir().expect("dir");
    let staged = prepared(&zt, dir.path(), "Key", false);
    add_list(&mut mcp, TABLE);
    let revision = mcp.revision();
    let manifest = read_json(&staged.join("manifest.json"));
    std::fs::remove_file(manifest["source"].as_str().unwrap()).expect("the prepared run's source file");
    let staged_text = staged.to_str().unwrap().to_string();
    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert_eq!(output.status.code(), Some(1));
    let line = last_json(&output);
    assert_eq!(line["conflicts"][0]["code"], json!("missing_source"), "{line}");
    assert_eq!(mcp.revision(), revision);
}

// ==================================================================================================
// A staging folder that is no prepared run cannot be backfilled
// ==================================================================================================

#[test]
fn a_staging_folder_without_a_frozen_contract_is_not_prepared() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    let zt = Zt::new(program);
    let dir = tempfile::tempdir().expect("dir");
    let staged = dir.path().join("Key");
    std::fs::create_dir_all(&staged).expect("staging");
    std::fs::write(
        staged.join("manifest.json"),
        json!({"citation_key": "Key", "book_title": "Test Book", "staging": staged.to_string_lossy(), "source": dir.path().join("source.md").to_string_lossy()}).to_string(),
    )
    .expect("manifest");
    let staged_text = staged.to_str().unwrap().to_string();

    let output = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
    assert_eq!(output.status.code(), Some(1));
    let line = last_json(&output);
    assert_eq!(line["ok"], json!(false), "{line}");
    assert_eq!(line["conflicts"][0]["code"], json!("not_prepared"), "{line}");
}
// ==================================================================================================
// Old and new staging: the Term units register past the prepared target and the pass waits on them
// ==================================================================================================

#[test]
fn old_and_new_staging_both_register_the_term_units_and_leave_the_link_pass_waiting() {
    let Some(program) = zt_program() else {
        eprintln!("skipped: no zt binary");
        return;
    };
    for term_layers in [false, true] {
        // One scratch archive per prepared target: both use the Citation key Key, and a second
        // Literature tree with that key cannot exist in one archive.
        let zt = Zt::new(program.clone());
        let mut mcp = zt.mcp();
        let dir = tempfile::tempdir().expect("dir");
        let staged = prepared(&zt, dir.path(), "Key", term_layers);
        let list = add_list(&mut mcp, TABLE);
        let staged_text = staged.to_str().unwrap().to_string();

        let validated = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
        assert!(
            validated.status.success(),
            "validate (term layers {term_layers}): {} {}",
            String::from_utf8_lossy(&validated.stdout),
            String::from_utf8_lossy(&validated.stderr)
        );
        assert_eq!(last_json(&validated)["ok"], json!(true));

        // The compiler plans the Term cards and registers its job and book units in the prepared
        // run's own ledger, whether or not the prepared target already carried the Term layers.
        let before_plan = mcp.revision();
        let planned = zt.digest(&["term-cards", "-Step", "plan", "-Staging", &staged_text]);
        assert!(
            planned.status.success(),
            "term-cards plan (term layers {term_layers}): {} {}",
            String::from_utf8_lossy(&planned.stdout),
            String::from_utf8_lossy(&planned.stderr)
        );
        let plan = last_json(&planned);
        assert_eq!(plan["ok"], json!(true), "{plan}");
        assert_eq!(plan["units"]["book"], json!("termcards/book@1"), "{plan}");
        assert!(plan["units"]["jobs"].as_object().is_some_and(|jobs| !jobs.is_empty()), "{plan}");
        assert_eq!(mcp.revision(), before_plan, "the Term-card plan writes no Card");
        // A resumed backfill reconciles the frozen plan and its units instead of allocating new ones.
        let again = zt.digest(&["term-cards", "-Step", "plan", "-Staging", &staged_text]);
        assert!(again.status.success(), "plan again: {}", String::from_utf8_lossy(&again.stderr));
        assert_eq!(last_json(&again)["units"]["book"], json!("termcards/book@1"));
        let ledger = rusqlite::Connection::open(staged.join("ledger.sqlite3")).expect("ledger");
        let books: i64 = ledger
            .query_row("SELECT count(*) FROM units WHERE id='termcards/book@1'", [], |row| row.get(0))
            .expect("count");
        assert_eq!(books, 1, "one book unit, never a duplicate");
        // The staged plan is debt the validation shows; it is not a conflict and blocks no preparation.
        let with_debt = zt.digest(&["prepare", "-Out", &staged_text, "-Validate", "-Brief"]);
        assert!(with_debt.status.success(), "validate with a plan: {}", String::from_utf8_lossy(&with_debt.stderr));
        let debt = last_json(&with_debt);
        assert_eq!(debt["drafts"]["plan_revision"], json!(1), "{debt}");
        assert_eq!(debt["drafts"]["complete"], json!(false));
        assert_eq!(debt["conflicts"], json!([]));

        // The first publication retypes the legacy table to a Term list metadata only; the
        // deterministic pass reads a Term list by role, so the fixture takes that one step here.
        mcp.call("edit_card", json!({"at": list, "type": "term"}));

        // The deterministic pass plans and registers its own unit; with the Term-card book unit not
        // done it stays blocked, never linked and never failed as a provider fault.
        let before_link = mcp.revision();
        let link = zt.digest(&["term-link", "-Step", "plan", "-Staging", &staged_text]);
        assert!(
            link.status.success(),
            "term-link plan (term layers {term_layers}): {} {}",
            String::from_utf8_lossy(&link.stdout),
            String::from_utf8_lossy(&link.stderr)
        );
        assert_eq!(last_json(&link)["unit"], json!("termlink/book@1"));
        // The pass is registered but not complete: it waits on the Term-card book unit. A gate that
        // is not settled answers its own exit code and still prints the layer's view.
        let gate = zt.digest(&["ledger", "gate", "-Out", &staged_text, "-Layer", "termlink", "-Brief"]);
        let view = last_json(&gate);
        assert_eq!(view["complete"], json!(false), "{view}");
        assert_eq!(view["counts"]["registered"], json!(1), "{view}");
        let ledger = rusqlite::Connection::open(staged.join("ledger.sqlite3")).expect("ledger");
        let needs: Vec<String> = ledger
            .prepare("SELECT needs_id FROM deps WHERE unit_id='termlink/book@1' ORDER BY needs_id")
            .expect("deps")
            .query_map([], |row| row.get(0))
            .expect("query")
            .filter_map(|row| row.ok())
            .collect();
        assert!(needs.iter().any(|unit| unit == "termcards/book@1"), "{needs:?}");
        // No Card was published: both plans are read-only over the archive.
        assert_eq!(mcp.revision(), before_link, "the Term-link plan writes no Card");
    }
}
