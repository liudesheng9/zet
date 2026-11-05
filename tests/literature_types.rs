//! The Literature type (CONTEXT.md): each Literature Card is one of `history`, `economics`,
//! `mathematics` or `others`. A digest declares it when it creates the tree, and a client changes
//! it afterwards. These tests drive the real `zt mcp` process.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};
use std::io::{Read, Write};

const BOOK: &str = "@book{Smith2024, title={Firms and Markets}, author={Smith, A.}, year={2024}}";

fn refusal_text(mcp: &mut Mcp, name: &str, arguments: Value) -> String {
    mcp.refused(name, arguments)["error"].as_str().expect("error message").to_string()
}

fn type_of(mcp: &mut Mcp, root: &str) -> Value {
    let trees = mcp.ok("list_trees", json!({}));
    let tree = trees["trees"]
        .as_array()
        .unwrap()
        .iter()
        .find(|tree| tree["root"] == root)
        .unwrap_or_else(|| panic!("no tree {root}: {trees}"));
    tree["literature_type"].clone()
}

#[test]
fn a_new_literature_tree_is_others_until_something_declares_its_type() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK}));
    assert_eq!(type_of(&mut mcp, "Smith2024"), "others");
    assert_eq!(mcp.ok("get_card", json!({"at":"Smith2024"}))["literature_type"], "others");
}

#[test]
fn a_digest_declares_the_type_when_it_creates_the_tree() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK,"literature_type":"economics"}));
    assert_eq!(type_of(&mut mcp, "Smith2024"), "economics");
}

#[test]
fn a_batch_creates_a_typed_tree_like_a_single_call() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("apply_batch", json!({"items":[{
        "op_key":"run/setup/1/root/a","client_ref":"root","tool":"create_literature",
        "arguments":{"workspace":"agent","bibtex":BOOK,"body":"","literature_type":"history"},
    }]}));
    assert_eq!(type_of(&mut mcp, "Smith2024"), "history");
}

#[test]
fn an_unknown_type_is_refused_and_creates_nothing() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let message = refusal_text(&mut mcp, "create_literature", json!({"bibtex":BOOK,"literature_type":"physics"}));
    assert!(message.contains("physics"), "{message}");
    assert!(message.contains("history, economics, mathematics, others"), "{message}");
    assert_eq!(mcp.ok("list_trees", json!({}))["trees"], json!([]));
}

#[test]
fn a_client_changes_the_type_and_the_archive_revision_moves_once() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK}));
    let changed = mcp.ok("set_literature_type", json!({"at":"Smith2024","literature_type":"mathematics"}));
    assert_eq!(changed["address"], "Smith2024");
    assert_eq!(changed["literature_type"], "mathematics");
    assert_eq!(changed["changed"], true);
    assert_eq!(type_of(&mut mcp, "Smith2024"), "mathematics");
    // The same type again writes nothing.
    let same = mcp.ok("set_literature_type", json!({"at":"Smith2024","literature_type":"mathematics"}));
    assert_eq!(same["changed"], false);
}

#[test]
fn only_a_literature_card_has_a_type() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title":"企业边界"}));
    mcp.ok("create_literature", json!({"bibtex":BOOK}));
    mcp.ok("create_successor", json!({"at":"Smith2024","title":"第一章"}));
    for at in ["0/0", "Smith2024/1"] {
        let message = refusal_text(&mut mcp, "set_literature_type", json!({"at":at,"literature_type":"history"}));
        assert!(message.contains("only a Literature Card"), "{message}");
    }
    assert!(type_of(&mut mcp, "0/0").is_null(), "a Topic tree has no type");
}

#[test]
fn the_type_stays_with_the_card_through_a_citation_key_rename() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK,"literature_type":"history"}));
    mcp.ok("edit_literature_metadata", json!({
        "at":"Smith2024","bibtex":"@book{Jones2024, title={Firms}}","confirm":"move",
    }));
    assert_eq!(type_of(&mut mcp, "Jones2024"), "history");
}

#[test]
fn a_deleted_tree_takes_its_type_with_it() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK,"literature_type":"history"}));
    mcp.ok("delete_card", json!({"at":"Smith2024","confirm":"Smith2024"}));
    mcp.ok("create_literature", json!({"bibtex":BOOK}));
    assert_eq!(type_of(&mut mcp, "Smith2024"), "others");
}

#[test]
fn an_allow_list_connection_changes_no_type() {
    let scratch = ScratchBox::up();
    let mut open = scratch.mcp(&[]);
    open.ok("create_literature", json!({"bibtex":BOOK}));
    let list = scratch.list_file("allow.txt", &["Smith2024"]);
    let mut limited = scratch.mcp(&["--allow", &list]);
    let refusal = limited.refused("set_literature_type", json!({"at":"Smith2024","literature_type":"history"}));
    assert_eq!(refusal["error"], "scope_refused", "{refusal}");
    assert_eq!(type_of(&mut open, "Smith2024"), "others");
}

#[test]
fn a_literature_scope_connection_labels_literature_trees_only() {
    let scratch = ScratchBox::up();
    let mut open = scratch.mcp(&[]);
    open.ok("create_topic", json!({"title":"企业边界"}));
    open.ok("create_literature", json!({"bibtex":BOOK}));
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    literature.ok("set_literature_type", json!({"at":"Smith2024","literature_type":"economics"}));
    let refusal = literature.refused("set_literature_type", json!({"at":"0/0","literature_type":"history"}));
    assert_eq!(refusal["error"], "scope_refused", "{refusal}");
    assert_eq!(refusal["reason"], "topic_tree", "{refusal}");
}

/// The GUI's JSON API, as the page calls it.
struct Gui {
    port: u16,
    token: String,
}

impl Gui {
    fn of(scratch: &ScratchBox) -> Self {
        let out = scratch.cmd().arg("gui").output().expect("zt gui");
        assert!(out.status.success(), "zt gui failed: {out:?}");
        let stdout = String::from_utf8_lossy(&out.stdout);
        let port = stdout
            .lines()
            .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
            .and_then(|rest| rest.split('/').next())
            .and_then(|port| port.parse().ok())
            .expect("zt gui prints its URL");
        let (_, page) = Self::exchange(port, "", "GET", "/", "");
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("the page carries the GUI token")
            .to_string();
        Self { port, token }
    }

    fn exchange(port: u16, token: &str, method: &str, path: &str, body: &str) -> (u16, String) {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
        let request = format!(
            "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).expect("send request");
        let mut reply = String::new();
        stream.read_to_string(&mut reply).expect("read reply");
        let status = reply.split_whitespace().nth(1).expect("status").parse().expect("status number");
        let body = reply.split_once("\r\n\r\n").expect("head").1.to_string();
        (status, body)
    }

    fn request(&self, method: &str, path: &str, body: &str) -> (u16, Value) {
        let (status, text) = Self::exchange(self.port, &self.token, method, path, body);
        (status, serde_json::from_str(&text).unwrap_or_else(|error| panic!("{error}: {text}")))
    }

    fn get(&self, path: &str) -> Value {
        let (status, value) = self.request("GET", path, "");
        assert_eq!(status, 200, "{path}: {value}");
        value
    }

    fn set_type(&self, at: &str, literature_type: &str) -> (u16, Value) {
        let body = json!({"at":at,"literature_type":literature_type}).to_string();
        self.request("POST", "/api/literature/type", &body)
    }

    fn tree_row(&self, root: &str) -> Value {
        let overview = self.get("/api/overview");
        overview["trees"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tree| tree["root"] == root)
            .unwrap_or_else(|| panic!("no tree {root}: {overview}"))
            .clone()
    }
}

#[test]
fn the_gui_shows_the_type_and_changes_it_for_either_workspace() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK,"literature_type":"economics"}));
    let gui = Gui::of(&scratch);
    assert_eq!(gui.tree_row("Smith2024")["literature_type"], "economics");
    assert_eq!(gui.get("/api/card?at=Smith2024")["literature_type"], "economics");

    let before = gui.tree_row("Smith2024")["fingerprint"].clone();
    let fingerprint = gui.get("/api/overview")["fingerprint"].clone();
    let (status, answer) = gui.set_type("Smith2024", "history");
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["changed"], true);
    let row = gui.tree_row("Smith2024");
    assert_eq!(row["literature_type"], "history");
    assert_ne!(row["fingerprint"], before, "the page refetches a tree whose print moved");
    assert_ne!(gui.get("/api/overview")["fingerprint"], fingerprint, "an open page notices the change");
    assert_eq!(gui.get("/api/tree?id=Smith2024")["cards"][0]["literature_type"], "history");

    // The human Workspace is the owner's: the GUI labels its books too, the agent does not.
    let body = json!({"bibtex":"@book{Mine2024, title={My Book}}","text":"# My Book\n","workspace":"human"});
    let (status, created) = gui.request("POST", "/api/literature", &body.to_string());
    assert_eq!(status, 200, "{created}");
    assert_eq!(gui.tree_row("Mine2024")["literature_type"], "others");
    assert_eq!(gui.set_type("Mine2024", "mathematics").0, 200);
    assert_eq!(gui.tree_row("Mine2024")["literature_type"], "mathematics");
    let refusal = mcp.refused("set_literature_type", json!({"at":"Mine2024","literature_type":"history"}));
    assert!(refusal["error"].as_str().unwrap().contains("human"), "{refusal}");
}

#[test]
fn the_gui_refuses_an_unknown_type_with_the_four_names() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK}));
    let (status, answer) = Gui::of(&scratch).set_type("Smith2024", "physics");
    assert_eq!(status, 400, "{answer}");
    assert!(answer["error"].as_str().unwrap().contains("history, economics, mathematics, others"), "{answer}");
}

#[test]
fn the_session_changes_the_type_of_the_literature_card_it_is_at() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK}));
    let output = scratch
        .cmd()
        .write_stdin("go Smith2024\nlt economics\nq\n")
        .output()
        .expect("line Session");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(type_of(&mut mcp, "Smith2024"), "economics");
}

#[test]
fn a_dump_records_every_declared_type() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK,"literature_type":"history"}));
    mcp.ok("create_literature", json!({"bibtex":"@book{Plain2024, title={Plain}}"}));
    scratch.cmd().arg("dp").write_stdin("1\n").assert().success();
    let folder = std::fs::read_dir(scratch.archive_root.join("dump"))
        .expect("dump folder")
        .next()
        .expect("one dump")
        .expect("dump entry")
        .path();
    let mut archive = zip::ZipArchive::new(std::fs::File::open(folder).expect("open")).expect("zip");
    let name = (0..archive.len())
        .map(|at| archive.by_index(at).expect("entry").name().to_string())
        .find(|name| name.ends_with("archive.json"))
        .expect("the dump carries archive.json");
    let mut text = String::new();
    archive.by_name(&name).expect("manifest").read_to_string(&mut text).expect("read");
    let manifest: Value = serde_json::from_str(&text).expect("manifest JSON");
    assert_eq!(
        manifest["literature_types"],
        json!({"Plain2024":"others","Smith2024":"history"})
    );
}

#[test]
fn an_exported_tree_keeps_its_type() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex":BOOK,"literature_type":"history"}));
    let gui = Gui::of(&scratch);
    let out = scratch.archive_root.join("exported");
    scratch
        .cmd()
        .args(["export", "Smith2024", "--out", out.to_str().unwrap()])
        .assert()
        .success();
    let data = std::fs::read_to_string(out.join("cards.json")).expect("cards.json");
    let data: Value = serde_json::from_str(&data).expect("JSON");
    assert_eq!(data["cards"][0]["literature_type"], "history");
    assert_eq!(gui.tree_row("Smith2024")["literature_type"], "history");
}
