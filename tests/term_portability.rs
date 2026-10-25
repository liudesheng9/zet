//! Term portability (S17, #155; design 15.4): a real `zt dp` archive keeps every raw authored
//! Term macro, the `term_names` manifest beside its type map, every Term list, head and part, the
//! generated Term-use section, and the row-sensitive Lock evidence of the same captured view.
//!
//! The second half loads that payload back into a fresh scratch archive through the documented
//! dump layout (the product promises no restore command, S13/R323) and compares the canonical
//! facts: the same manifest and Markdown bytes, the same computed term structure and Links, the
//! same authored fingerprints and Lock seal, while row identities differ because addresses carry
//! the portable meaning.
//!
//! Both archives are scratch ones (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never
//! touched.

#![allow(dead_code)]

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[path = "support/host_tools.rs"]
mod host_tools;

/// A two-row Term table: `均衡` is published as a head, `弹性` is still missing.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

struct TestHome {
    _tmp: TempDir,
    config_dir: PathBuf,
    archive_root: PathBuf,
}

impl TestHome {
    fn new() -> Self {
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
            "zt {args:?} failed: {}\n{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn db(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db")
    }

    /// One Card's stored facts: Workspace, type, term name and exact text.
    fn stored(&self, at: &str) -> (String, String, Option<String>, String) {
        self.db()
            .query_row(
                "SELECT workspace, card_type, term_name, text FROM cards
                  WHERE location = ?1 OR citation_key = ?1",
                [at],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("stored Card")
    }

    fn text(&self, at: &str) -> String {
        self.stored(at).3
    }

    fn row_id(&self, at: &str) -> i64 {
        self.db()
            .query_row(
                "SELECT row_id FROM cards WHERE location = ?1 OR citation_key = ?1",
                [at],
                |row| row.get(0),
            )
            .expect("row id")
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
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "zt-term-portability-tests", "version": "1" } }),
        );
        mcp.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }

    fn gui(&self) -> Gui {
        let out = self.cmd().arg("gui").output().expect("zt gui");
        assert!(out.status.success(), "zt gui failed: {out:?}");
        let text = String::from_utf8_lossy(&out.stdout);
        let port: u16 = text
            .lines()
            .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
            .and_then(|rest| rest.trim_end_matches('/').parse().ok())
            .expect("zt gui prints its URL");
        let page = http_page(port);
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page carries the GUI token")
            .to_string();
        Gui { port, token }
    }

    /// Install one parsed dump into this archive's database, the way the documented payload
    /// describes the archive: the Workspace of every address, the type of every non-Idea Card,
    /// the exact term names, the exact Markdown, and the Literature rows' Citation key and
    /// BibTeX. The product has no restore command (S13/R323); this is the test's loader.
    ///
    /// Rows are inserted in reverse address order so this archive's row identities are provably
    /// different from the dumped one: addresses and authored fingerprints carry the meaning.
    fn install(&self, dump: &Dump) {
        self.run(&["down"]);
        let conn = self.db();
        let tx = conn.unchecked_transaction().expect("loader transaction");
        let workspaces = dump.manifest["workspaces"].as_object().expect("workspaces");
        let types = dump.manifest["types"].as_object().expect("types");
        let term_names = dump.manifest["term_names"].as_object().expect("term names");
        let mut addresses = dump.mapping.keys().collect::<Vec<_>>();
        addresses.reverse();
        for address in addresses {
            let markdown = &dump.files[&dump.mapping[address]];
            let workspace = workspaces[address].as_str().expect("workspace");
            let card_type = types.get(address).and_then(Value::as_str).unwrap_or("idea");
            let term_name = term_names.get(address).and_then(Value::as_str);
            let (location, citation_key, is_lit, bibtex, text) =
                match dump.files.get(&format!("{address}.bib")) {
                    Some(bibtex) => (
                        None,
                        Some(address.as_str()),
                        1,
                        Some(bibtex.as_str()),
                        strip_front_matter(markdown),
                    ),
                    None => (Some(address.as_str()), None, 0, None, markdown.as_str()),
                };
            // A Topic tree root is `<n>/0`; every other regular Location is a child or a
            // Literature row. The dump records no kind flag because the address grammar is the
            // portable identity.
            let is_topic = is_lit == 0 && address.ends_with("/0");
            tx.execute(
                "INSERT INTO cards(
                    location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type,
                    model_facet, model_emphasis, model_level, hub_level, math_form, term_name)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, NULL, NULL, NULL, NULL, ?9)",
                rusqlite::params![
                    location,
                    citation_key,
                    is_topic,
                    is_lit,
                    bibtex,
                    text,
                    workspace,
                    card_type,
                    term_name
                ],
            )
            .unwrap_or_else(|error| panic!("load `{address}`: {error}"));
        }
        tx.execute(
            "UPDATE metadata SET value = ?1 WHERE key = 'next_topic_id'",
            [dump.manifest["next_topic_id"].to_string()],
        )
        .expect("next Topic id");
        tx.commit().expect("loader commit");
        drop(conn);
        self.run(&["up"]);
        self.run(&["reindex"]);
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
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn post(&self, path: &str, body: &Value) -> Value {
        use std::net::TcpStream;
        let payload = serde_json::to_string(body).expect("JSON body");
        let mut stream = TcpStream::connect(("127.0.0.1", self.port)).expect("connect to gui");
        let request = format!(
            "POST {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{}\r\nX-ZT-Token: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{payload}",
            self.port,
            self.token,
            payload.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let status = response
            .split_whitespace()
            .nth(1)
            .and_then(|code| code.parse::<u16>().ok())
            .expect("status code");
        let start = response.find("\r\n\r\n").expect("header end") + 4;
        let body = serde_json::from_str(&response[start..]).unwrap_or(Value::Null);
        assert_eq!(status, 200, "{path} answered {status}: {body}");
        body
    }

    /// The owner's two-step Lock of one tree root, as the GUI performs it.
    fn lock(&self, root: &str, note: Option<&str>) -> Value {
        let mut request = json!({ "root": root });
        if let Some(note) = note {
            request["note"] = json!(note);
        }
        let plan = self.post("/api/lock", &request);
        assert_eq!(
            plan["applied"],
            json!(false),
            "the first request plans: {plan}"
        );
        let mut confirmed = request.clone();
        confirmed["confirm"] = json!("lock");
        confirmed["plan_hash"] = plan["plan"]["plan_hash"].clone();
        let applied = self.post("/api/lock", &confirmed);
        assert_eq!(
            applied["applied"],
            json!(true),
            "the confirmation commits: {applied}"
        );
        applied["lock"].clone()
    }
}

fn http_page(port: u16) -> String {
    use std::net::TcpStream;
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
    )
    .expect("request page");
    let mut page = String::new();
    stream.read_to_string(&mut page).expect("read page");
    page
}

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// The addresses and Cards of one Term book, plus the Topic Card one of its members links: the
/// Lock's external pin target.
struct Book {
    topic_root: String,
    topic_card: String,
    key: String,
    anchor: String,
    list: String,
    head: String,
    part: String,
    source: String,
}

impl Book {
    /// The Locked tree in address order: its root, the anchor, the list, the head, the part and
    /// the Term-use source.
    fn members(&self) -> Vec<String> {
        vec![
            self.key.clone(),
            self.anchor.clone(),
            self.list.clone(),
            self.head.clone(),
            self.part.clone(),
            self.source.clone(),
        ]
    }
}

/// Create one agent Literature tree with a Term list, a head, a part and one Term use, beside an
/// agent Topic Card that the tree links (the Lock's pin target), and lock the whole tree.
fn locked_book(home: &TestHome, mcp: &mut Mcp) -> Book {
    let topic_root = address(mcp.ok("create_topic", json!({ "title": "定价" })));
    let topic_card = address(mcp.ok(
        "create_successor",
        json!({ "at": &topic_root, "title": "外部目标", "body": "目标。" }),
    ));
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Test2026, title={Test Book}}" }),
    );
    let anchor = address(mcp.ok(
        "create_successor",
        json!({
            "at": "Test2026",
            "title": "第一章",
            "body": format!("设置，外部目标是 [[{topic_card}]]。"),
        }),
    ));
    let list = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "术语表：测试",
            "type": "term",
            "body": TABLE,
        }),
    ));
    let head = address(mcp.ok(
        "create_successor",
        json!({
            "at": &list,
            "kind": "side",
            "title": "均衡",
            "type": "term",
            "term": "均衡",
            "body": format!(
                "术语：均衡\n\n类别：概念\n\n## 释义\n\n双方都不改变策略。\n\n## 在本书中的作用\n\n第一章用它定义市场结果。\n\n来源：[[{anchor}]] §1.1"
            ),
        }),
    ));
    let part = address(mcp.ok(
        "create_successor",
        json!({
            "at": &head,
            "kind": "direct",
            "title": "推导（续 1/2）：均衡",
            "type": "term",
            "term": "均衡",
            "body": format!("类别：概念\n\n续：[[{head}]]\n\n## 推导\n\n1. 由定义直接得到。"),
        }),
    ));
    let source = address(mcp.ok(
        "create_successor",
        json!({
            "at": &anchor,
            "kind": "side",
            "title": "使用者",
            "body": format!("均衡 [[均衡@{head}]] 决定价格，见 [[{topic_card}]]。"),
        }),
    ));
    let book = Book {
        topic_root,
        topic_card,
        key: "Test2026".to_string(),
        anchor,
        list,
        head,
        part,
        source,
    };
    home.gui().lock(&book.key, Some("S17 portability"));
    book
}

/// One extracted `zt dp` payload: the mapping, the manifest, the Lock export and every other
/// entry by its path inside the archive folder (Card Markdown and `<key>.bib`).
struct Dump {
    mapping: BTreeMap<String, String>,
    manifest: Value,
    manifest_text: String,
    locks: Value,
    files: BTreeMap<String, String>,
}

fn single_dump_archive(home: &TestHome) -> PathBuf {
    let dump_root = home.archive_root.join("dump");
    let mut archives = fs::read_dir(&dump_root)
        .expect("dump directory")
        .map(|entry| entry.expect("dump entry").path())
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    archives.sort();
    assert_eq!(archives.len(), 1, "expected one dump archive: {archives:?}");
    archives.remove(0)
}

/// The dump root folder's name, needed to address an entry by its path.
fn dump_folder(archive: &mut zip::ZipArchive<fs::File>) -> String {
    let mut names = Vec::new();
    for index in 0..archive.len() {
        names.push(archive.by_index(index).expect("entry").name().to_string());
    }
    names
        .iter()
        .find_map(|name| name.strip_suffix("/mapping.json").map(str::to_string))
        .expect("the dump carries mapping.json")
}

fn dump(home: &TestHome) -> Dump {
    home.cmd().arg("dp").write_stdin("1\n").assert().success();
    let path = single_dump_archive(home);
    let mut archive =
        zip::ZipArchive::new(fs::File::open(&path).expect("open ZIP")).expect("read ZIP");
    let folder = dump_folder(&mut archive);
    let mut dump = Dump {
        mapping: BTreeMap::new(),
        manifest: Value::Null,
        manifest_text: String::new(),
        locks: Value::Null,
        files: BTreeMap::new(),
    };
    let mut archives = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("ZIP entry");
        let name = entry.name().to_string();
        if !name.starts_with(&format!("{folder}/")) {
            continue;
        }
        let relative = name[folder.len() + 1..].to_string();
        let mut body = String::new();
        entry.read_to_string(&mut body).expect("entry text");
        match relative.as_str() {
            "mapping.json" => {
                dump.mapping = serde_json::from_str(&body).expect("mapping JSON");
            }
            "archive.json" => {
                dump.manifest_text = body.clone();
                dump.manifest = serde_json::from_str(&body).expect("manifest JSON");
            }
            "locks.json" => dump.locks = serde_json::from_str(&body).expect("locks JSON"),
            _ if relative.starts_with("artifacts/") => {
                archives.push(relative);
            }
            _ => {
                dump.files.insert(relative, body);
            }
        }
    }
    assert!(dump.manifest.is_object(), "the dump carries archive.json");
    assert!(dump.locks.is_object(), "the dump carries locks.json");
    assert!(archives.is_empty(), "no artifact belongs to these Cards");
    dump
}

/// The Card text of a Literature Markdown file: everything after its YAML front matter.
fn strip_front_matter(markdown: &str) -> &str {
    let opened = markdown
        .strip_prefix("---\n")
        .unwrap_or_else(|| panic!("a Literature file starts with front matter: {markdown:?}"));
    let end = opened
        .find("\n---\n")
        .unwrap_or_else(|| panic!("front matter has an end: {markdown:?}"));
    &opened[end + 5..]
}

/// One Lock's portable member fingerprint map: address to authored fingerprint.
fn member_fingerprints(lock: &Value) -> BTreeMap<String, String> {
    lock["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| {
            (
                member["address"]
                    .as_str()
                    .expect("member address")
                    .to_string(),
                member["fingerprint"]
                    .as_str()
                    .expect("member fingerprint")
                    .to_string(),
            )
        })
        .collect()
}

/// One Lock's pin pairs in the export's own order: `(source address, target address)`.
fn pin_pairs(lock: &Value) -> Vec<(String, String)> {
    lock["pins"]
        .as_array()
        .expect("pins")
        .iter()
        .map(|pin| {
            (
                pin["source"].as_str().expect("pin source").to_string(),
                pin["target"].as_str().expect("pin target").to_string(),
            )
        })
        .collect()
}

fn lock_of(locks: &Value) -> &Value {
    let locks = locks["locks"].as_array().expect("locks");
    assert_eq!(locks.len(), 1, "one active Lock: {locks:#?}");
    &locks[0]
}

fn host_zip_backend_available() -> bool {
    StdCommand::new(host_tools::host_tar())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[test]
fn a_dump_keeps_raw_term_macros_names_types_heads_and_parts() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    let mut mcp = home.mcp();
    let book = locked_book(&home, &mut mcp);

    // The label read is opt-in and marked; the raw body is the default and is what the dump
    // writes. A projected label is never a stored byte.
    let macro_text = format!("[[均衡@{}]]", book.head);
    let raw = mcp.ok("get_card", json!({ "at": &book.source }));
    assert_eq!(
        raw["body"],
        json!(format!(
            "均衡 {macro_text} 决定价格，见 [[{}]]。",
            book.topic_card
        ))
    );
    assert!(raw.get("term_links").is_none(), "raw is unmarked: {raw}");
    let label = mcp.ok(
        "get_card",
        json!({ "at": &book.source, "term_links": "label" }),
    );
    assert_eq!(label["term_links"], json!("label"));
    assert_eq!(
        label["body"],
        json!(format!("均衡 均衡 决定价格，见 [[{}]]。", book.topic_card)),
        "only the Term macro becomes its visible label"
    );

    // The computed structure the archive holds before the dump.
    let terms = mcp.ok("get_terms", json!({ "at": &book.list }));
    assert_eq!(terms["heads"][0]["address"], json!(book.head));
    assert_eq!(terms["parts"][0]["address"], json!(book.part));
    assert_eq!(terms["uses"][0]["source"], json!(book.source));
    assert_eq!(terms["uses"][0]["label"], json!("均衡"));

    let dump = dump(&home);

    // Every Card of the tree is exported: both heads and parts, the list, the use source and
    // the Topic Cards the tree links.
    let expected_mapping: Vec<&String> = dump.mapping.keys().collect();
    let mut expected_addresses = vec![
        &book.topic_root,
        &book.topic_card,
        &book.key,
        &book.anchor,
        &book.list,
        &book.head,
        &book.part,
        &book.source,
    ];
    expected_addresses.sort();
    assert_eq!(
        expected_mapping, expected_addresses,
        "the mapping holds every address and nothing else"
    );

    // `types` keeps the Term list, head and part; `term_names` sits beside it with only the
    // named Cards, and every other manifest entry keeps its existing convention.
    let mut expected_types = serde_json::Map::new();
    expected_types.insert(book.list.clone(), json!("term"));
    expected_types.insert(book.head.clone(), json!("term"));
    expected_types.insert(book.part.clone(), json!("term"));
    assert_eq!(dump.manifest["types"], Value::Object(expected_types));
    let mut expected_names = serde_json::Map::new();
    expected_names.insert(book.head.clone(), json!("均衡"));
    expected_names.insert(book.part.clone(), json!("均衡"));
    assert_eq!(dump.manifest["term_names"], Value::Object(expected_names));
    let mut expected_workspaces = serde_json::Map::new();
    for address in &expected_addresses {
        expected_workspaces.insert((*address).clone(), json!("agent"));
    }
    assert_eq!(
        dump.manifest["workspaces"],
        Value::Object(expected_workspaces)
    );
    assert_eq!(dump.manifest["next_topic_id"], json!(1));

    // Every exported Markdown file is the exact stored text — a Literature root adds only its
    // documented front matter: no projected label replaces a macro, and the generated sections
    // travel with the Card.
    for (address, filename) in &dump.mapping {
        let markdown = &dump.files[filename];
        let text = if dump.files.contains_key(&format!("{address}.bib")) {
            assert!(
                markdown.starts_with("---\nzt_kind: literature\n"),
                "a Literature root keeps its front matter: {markdown:?}"
            );
            strip_front_matter(markdown)
        } else {
            markdown.as_str()
        };
        assert_eq!(
            text,
            home.text(address),
            "`{address}` is dumped byte for byte"
        );
    }
    assert_eq!(
        dump.files[&format!("{}.bib", book.key)],
        "@book{Test2026, title={Test Book}}",
        "the exact BibTeX bytes travel beside the Literature Markdown"
    );
    assert!(
        dump.files[&dump.mapping[&book.source]].contains(&macro_text),
        "the raw Term macro is exported, never its projected label"
    );
    assert!(
        dump.files[&dump.mapping[&book.head]]
            .contains(&format!("## Term uses\n\n- [[{}]] 使用者\n", book.source)),
        "the generated Term-use section is part of the exported text"
    );

    // The Lock export names every member with its address, row identity and term-aware authored
    // fingerprint, and the external pin the tree's Links created.
    let lock = lock_of(&dump.locks);
    assert_eq!(lock["root"], json!(book.key));
    assert_eq!(lock["workspace"], json!("agent"));
    assert_eq!(dump.locks["integrity"]["ok"], json!(true));
    let members = member_fingerprints(lock);
    let mut expected_members = book.members();
    expected_members.sort();
    assert_eq!(
        members.keys().cloned().collect::<Vec<_>>(),
        expected_members
    );
    for member in &book.members() {
        let row_id = home.row_id(member);
        let stored = lock["members"]
            .as_array()
            .expect("members")
            .iter()
            .find(|row| row["address"] == json!(member))
            .unwrap_or_else(|| panic!("member `{member}`"));
        assert_eq!(
            stored["row_id"],
            json!(row_id),
            "row ids are captured evidence"
        );
    }
    assert_eq!(
        pin_pairs(lock),
        vec![
            (book.anchor.clone(), book.topic_card.clone()),
            (book.source.clone(), book.topic_card.clone()),
        ],
        "each member Link to the external Card is one pin, in export order"
    );
}

#[test]
fn a_dumped_archive_loads_back_with_the_same_term_structure_links_and_lock_fingerprints() {
    if !host_zip_backend_available() {
        return;
    }

    let source = TestHome::new();
    let mut source_mcp = source.mcp();
    let book = locked_book(&source, &mut source_mcp);
    let dump_a = dump(&source);
    let lock_a = lock_of(&dump_a.locks).clone();
    let terms_a = source_mcp.ok("get_terms", json!({ "at": &book.list }));

    // A second, empty archive receives the whole payload through its documented layout.
    let loaded = TestHome::new();
    loaded.install(&dump_a);
    let mut loaded_mcp = loaded.mcp();

    // The computed structure and the raw authored text come back the same: the Term list's
    // rows, its head and part, the use with its label, the coverage and every fault, and the
    // statement fingerprint of a Card whose body holds a Term macro.
    let terms_b = loaded_mcp.ok("get_terms", json!({ "at": &book.list }));
    assert_eq!(terms_b["list"], terms_a["list"]);
    assert_eq!(terms_b["rows"], terms_a["rows"]);
    assert_eq!(terms_b["heads"], terms_a["heads"]);
    assert_eq!(terms_b["parts"], terms_a["parts"]);
    assert_eq!(terms_b["uses"], terms_a["uses"]);
    assert_eq!(terms_b["coverage"], terms_a["coverage"]);
    assert_eq!(terms_b["faults"], terms_a["faults"]);
    for at in [&book.head, &book.part, &book.source, &book.list] {
        let a = source_mcp.ok("get_card", json!({ "at": at }));
        let b = loaded_mcp.ok("get_card", json!({ "at": at }));
        for field in [
            "text",
            "body",
            "type",
            "term",
            "term_role",
            "statement_fingerprint",
        ] {
            assert_eq!(b[field], a[field], "`{at}` keeps its {field}");
        }
    }
    let card_b = loaded_mcp.ok("get_card", json!({ "at": &book.head }));
    assert_eq!(card_b["term"], json!("均衡"));
    assert_eq!(card_b["term_role"], json!("card"));
    assert!(
        loaded
            .text(&book.source)
            .contains(&format!("[[均衡@{}]]", book.head)),
        "the raw Term macro survives the load"
    );

    // The canonical fingerprint comparison: locking the same tree in the loaded archive must
    // reproduce the source Lock's seal and every member fingerprint, because those cover the
    // addresses, the exact texts, the types and the term names. The row identities differ,
    // because they are audit evidence of the captured database, not portable meaning.
    loaded.gui().lock(&book.key, Some("S17 portability"));
    let dump_b = dump(&loaded);
    let lock_b = lock_of(&dump_b.locks).clone();
    assert_eq!(lock_b["root"], lock_a["root"]);
    assert_eq!(lock_b["tree"], lock_a["tree"]);
    assert_eq!(lock_b["workspace"], lock_a["workspace"]);
    assert_eq!(
        lock_b["seal"], lock_a["seal"],
        "the term names and texts are part of the authored identity"
    );
    assert_eq!(
        member_fingerprints(&lock_b),
        member_fingerprints(&lock_a),
        "every member keeps its exact authored fingerprint"
    );
    assert_eq!(pin_pairs(&lock_b), pin_pairs(&lock_a));
    assert_eq!(dump_b.locks["integrity"]["ok"], json!(true));

    let rows_a = lock_a["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| member["row_id"].as_i64().expect("row id"))
        .collect::<Vec<_>>();
    let rows_b = lock_b["members"]
        .as_array()
        .expect("members")
        .iter()
        .map(|member| member["row_id"].as_i64().expect("row id"))
        .collect::<Vec<_>>();
    assert_ne!(
        rows_a, rows_b,
        "the loaded rows have their own identities: the seal is not row-dependent"
    );
    assert_ne!(source.row_id(&book.head), loaded.row_id(&book.head));
    for member in &book.members() {
        assert_eq!(
            source.text(member),
            loaded.text(member),
            "`{member}` keeps its exact authored text"
        );
    }

    // The whole exported payload is reproducible from the load: the manifest, the mapping and
    // every Markdown/BibTeX entry are byte-identical to the source dump.
    assert_eq!(
        dump_b.manifest_text, dump_a.manifest_text,
        "the manifest is reproduced byte for byte"
    );
    assert_eq!(dump_b.mapping, dump_a.mapping);
    assert_eq!(
        dump_b.files, dump_a.files,
        "every Card file and BibTeX entry is reproduced byte for byte"
    );

    // Both archives pass their own derived-Link and Lock-integrity checks.
    loaded.run(&["status", "--check-links"]);
    loaded.run(&["status", "--check-locks"]);
    source.run(&["status", "--check-links"]);
}
