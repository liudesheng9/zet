//! The Write scope (CONTEXT.md, docs/agent-write-scope-proposal.md): `zt mcp --scope
//! literature|open [--allow f] [--beneath f]` limits which agent Cards one connection may change,
//! checked by the server for every writer and every batch item, and answered with the typed
//! `scope_refused`.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

const BOOK: &str = "@book{Smith2024, title={Firms and Markets}, author={Smith, A.}, year={2024}}";

/// An agent Topic `0/0` with one Card `0/1`, and an agent Literature tree `Smith2024` with one
/// Card `Smith2024/1`, written by an open connection.
fn seeded(scratch: &ScratchBox) -> Mcp {
    let mut open = scratch.mcp(&[]);
    assert_eq!(open.ok("create_topic", json!({"title":"企业边界"}))["address"], "0/0");
    assert_eq!(open.ok("create_successor", json!({"at":"0/0","title":"企业边界由交易成本决定"}))["address"], "0/1");
    assert_eq!(open.ok("create_literature", json!({"bibtex":BOOK}))["address"], "Smith2024");
    assert_eq!(open.ok("create_successor", json!({"at":"Smith2024","title":"第一章"}))["address"], "Smith2024/1");
    open
}

fn assert_refused(refusal: &Value, tool: &str, reason: &str) {
    assert_eq!(refusal["error"], "scope_refused", "{refusal}");
    assert_eq!(refusal["tool"], tool, "{refusal}");
    assert_eq!(refusal["reason"], reason, "{refusal}");
}

#[test]
fn a_literature_session_refuses_a_new_topic_and_stores_nothing() {
    let scratch = ScratchBox::up();
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let refusal = literature.refused("create_topic", json!({"title":"企业边界"}));
    assert_eq!(refusal["error"], "scope_refused", "{refusal}");
    assert_eq!(refusal["tool"], "create_topic");
    assert_eq!(refusal["scope"], "literature");
    assert!(refusal["message"].as_str().unwrap().contains("Topic"), "{refusal}");
    let mut open = scratch.mcp(&[]);
    assert_eq!(open.ok("list_trees", json!({}))["trees"], json!([]));
}

#[test]
fn a_literature_session_writes_literature_trees_and_refuses_every_topic_tree_write() {
    let scratch = ScratchBox::up();
    let mut open = seeded(&scratch);
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    // Reads of either Zone and writes inside a Literature tree pass.
    literature.ok("get_card", json!({"at":"0/1"}));
    assert_eq!(literature.ok("create_successor", json!({"at":"Smith2024/1","title":"交易成本","body":"依赖 [[Smith2024/1]]。"}))["address"], "Smith2024/2");
    literature.ok("edit_card", json!({"at":"Smith2024/2","body":"依赖 [[Smith2024/1]]，更新。"}));
    let refusals = [
        ("create_successor", json!({"at":"0/1","title":"x"}), "topic_tree"),
        ("edit_card", json!({"at":"0/1","body":"x"}), "topic_tree"),
        ("insert_card", json!({"before":"0/1","title":"x","confirm":"insert"}), "topic_tree"),
        ("move_card", json!({"at":"Smith2024/2","to":"0/1","confirm":"move"}), "topic_tree"),
        ("delete_card", json!({"at":"0/1","confirm":"delete"}), "topic_tree"),
        ("create_successor", json!({"at":"Smith2024/1","title":"x","body":"对照 [[0/1]]。"}), "topic_link"),
        ("edit_card", json!({"at":"Smith2024/1","body":"对照 [[0/1]]。"}), "topic_link"),
        ("create_literature", json!({"bibtex":"@book{Jones2020, title={T}}","body":"见 [[0/1]]。"}), "topic_link"),
        ("create_successor", json!({"at":"Smith2024/1","title":"x","type":"hub"}), "hub"),
    ];
    for (tool, arguments, reason) in refusals {
        let refusal = literature.refused(tool, arguments.clone());
        assert_refused(&refusal, tool, reason);
        assert_eq!(refusal["scope"], "literature");
    }
    // Nothing above was stored: the Topic tree and the book keep exactly their Cards.
    let topic = open.ok("get_tree", json!({"tree":"0"}))["cards"].as_array().unwrap().len();
    assert_eq!(topic, 2);
    assert_eq!(open.ok("get_card", json!({"at":"Smith2024/1"}))["body"], "");
    let book = open.ok("get_tree", json!({"tree":"Smith2024"}))["cards"].as_array().unwrap().len();
    assert_eq!(book, 3);
    let trees = open.ok("list_trees", json!({}))["trees"].as_array().unwrap().len();
    assert_eq!(trees, 2);
}

#[test]
fn a_literature_session_refuses_to_change_an_existing_hub_card() {
    let scratch = ScratchBox::up();
    let mut open = seeded(&scratch);
    let hub = open.ok("create_successor", json!({"at":"Smith2024/1","kind":"side","title":"本书索引","type":"hub","body":"- [[Smith2024/1]]：第一章"}));
    let at = hub["address"].as_str().unwrap().to_string();
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let refusal = literature.refused("edit_card", json!({"at":at,"body":"- [[Smith2024/1]]：改"}));
    assert_refused(&refusal, "edit_card", "hub");
    assert_eq!(refusal["at"], at.as_str());
}

#[test]
fn an_undeclared_or_open_session_keeps_todays_behaviour() {
    let scratch = ScratchBox::up();
    seeded(&scratch);
    for arguments in [&[][..], &["--scope", "open"][..]] {
        let mut session = scratch.mcp(arguments);
        session.ok("edit_card", json!({"at":"0/1","body":"对照 [[Smith2024/1]]。"}));
        session.ok("edit_card", json!({"at":"Smith2024/1","body":"对照 [[0/1]]。"}));
    }
}

#[test]
fn a_bad_scope_declaration_stops_the_server_before_its_first_message() {
    let scratch = ScratchBox::up();
    for (arguments, expected) in [
        (&["--scope", "bogus"][..], "expected literature or open"),
        (&["--scope"][..], "needs a value"),
        (&["--allow", "Z:/no/such/list.txt"][..], "cannot read the Write scope list"),
        (&["--whatever"][..], "unknown `zt mcp` argument"),
    ] {
        let output = scratch.mcp_command(arguments).output().expect("run zt mcp");
        assert!(!output.status.success(), "{arguments:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(expected), "{arguments:?}: {stderr}");
    }
}
