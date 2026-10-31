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
fn an_empty_allow_list_makes_a_session_read_only() {
    let scratch = ScratchBox::up();
    seeded(&scratch);
    let none = scratch.list_file("none.allow.txt", &[]);
    let mut reader = scratch.mcp(&["--allow", &none]);
    reader.ok("get_tree", json!({"tree":"Smith2024"}));
    assert_refused(&reader.refused("edit_card", json!({"at":"Smith2024/1","body":"x"})), "edit_card", "not_listed");
    assert_refused(&reader.refused("create_successor", json!({"at":"Smith2024/1","title":"x"})), "create_successor", "not_listed");
    for (tool, arguments) in [
        ("create_topic", json!({"title":"新题"})),
        ("create_literature", json!({"bibtex":"@book{Jones2020, title={T}}"})),
        ("edit_literature_metadata", json!({"at":"Smith2024","bibtex":BOOK})),
        ("move_card", json!({"at":"Smith2024/1","to":"Smith2024/1|a","confirm":"move"})),
        ("insert_card", json!({"before":"Smith2024/1","title":"x","confirm":"insert"})),
        ("delete_card", json!({"at":"Smith2024/1","confirm":"delete"})),
    ] {
        assert_refused(&reader.refused(tool, arguments), tool, "structural");
    }
}

#[test]
fn a_listed_session_grows_its_listed_cards_and_the_cards_it_made() {
    let scratch = ScratchBox::up();
    let mut open = seeded(&scratch);
    let list = scratch.list_file("mine.allow.txt", &["Smith2024/1"]);
    let mut writer = scratch.mcp(&["--scope", "literature", "--allow", &list]);
    writer.ok("edit_card", json!({"at":"Smith2024/1","body":"第一章的要点。"}));
    let made = writer.ok("create_successor", json!({"at":"Smith2024/1","title":"第二章"}))["address"].as_str().unwrap().to_string();
    assert_eq!(made, "Smith2024/2");
    writer.ok("edit_card", json!({"at":made,"body":"第二章的要点。"}));
    writer.ok("create_successor", json!({"at":made,"kind":"side","title":"第二章旁注"}));
    // A Card another connection made is not this connection's, even in the same tree.
    let theirs = open.ok("create_successor", json!({"at":"Smith2024/2","title":"第三章"}))["address"].as_str().unwrap().to_string();
    assert_refused(&writer.refused("edit_card", json!({"at":theirs,"body":"x"})), "edit_card", "not_listed");
    // A Side placement before a sibling renumbers Cards the list does not name.
    let refusal = writer.refused("create_successor", json!({"at":"Smith2024/1","kind":"side","before":"Smith2024/1|a","title":"x"}));
    assert_refused(&refusal, "create_successor", "renumbers");
    // A second connection with the same list does not inherit the first one's Cards.
    let mut again = scratch.mcp(&["--allow", &list]);
    assert_refused(&again.refused("edit_card", json!({"at":"Smith2024/2","body":"x"})), "edit_card", "not_listed");
}

#[test]
fn a_beneath_card_takes_only_a_new_side_successor() {
    let scratch = ScratchBox::up();
    seeded(&scratch);
    let none = scratch.list_file("none.allow.txt", &[]);
    let beside = scratch.list_file("beside.txt", &["Smith2024/1"]);
    let mut writer = scratch.mcp(&["--allow", &none, "--beneath", &beside]);
    let side = writer.ok("create_successor", json!({"at":"Smith2024/1","kind":"side","title":"旁注"}))["address"].as_str().unwrap().to_string();
    writer.ok("edit_card", json!({"at":side,"body":"旁注的要点。"}));
    assert_refused(&writer.refused("create_successor", json!({"at":"Smith2024/1","title":"x"})), "create_successor", "beside_only");
    assert_refused(&writer.refused("edit_card", json!({"at":"Smith2024/1","body":"x"})), "edit_card", "beside_only");
}

fn item(n: usize, tool: &str, arguments: Value) -> Value {
    json!({"op_key":format!("run/unit/1/apply/item-{n}"),"client_ref":format!("i{n}"),"tool":tool,"arguments":arguments})
}

#[test]
fn every_batch_item_is_checked_and_a_refused_batch_stores_nothing() {
    let scratch = ScratchBox::up();
    let mut open = seeded(&scratch);
    let none = scratch.list_file("none.allow.txt", &[]);
    let mut reader = scratch.mcp(&["--allow", &none]);
    let batch = json!({"items":[item(0,"edit_card",json!({"at":"Smith2024/1","body":"x","base_fingerprint":"0"}))]});
    let refusal = reader.refused("apply_batch", batch);
    assert_refused(&refusal, "edit_card", "not_listed");
    assert_eq!(refusal["item"]["index"], 0, "{refusal}");
    assert!(refusal["message"].as_str().unwrap().starts_with("batch item 0 (edit_card): "), "{refusal}");
    // A later refused item rolls back the earlier items of the same batch.
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let batch = json!({"items":[
        item(1,"create_successor",json!({"at":"Smith2024/1","title":"新卡"})),
        item(2,"create_successor",json!({"at":"0/1","title":"主题卡"})),
    ]});
    let refusal = literature.refused("apply_batch", batch);
    assert_refused(&refusal, "create_successor", "topic_tree");
    assert_eq!(refusal["item"]["index"], 1, "{refusal}");
    assert_eq!(open.ok("get_tree", json!({"tree":"Smith2024"}))["cards"].as_array().unwrap().len(), 2);
    assert_eq!(open.ok("get_tree", json!({"tree":"0"}))["cards"].as_array().unwrap().len(), 2);
}

#[test]
fn a_batch_grows_the_cards_it_makes_and_they_stay_the_sessions_cards() {
    let scratch = ScratchBox::up();
    seeded(&scratch);
    let list = scratch.list_file("mine.allow.txt", &["Smith2024/1"]);
    let mut writer = scratch.mcp(&["--scope", "literature", "--allow", &list]);
    let done = writer.ok("apply_batch", json!({"items":[
        item(0,"create_successor",json!({"at":"Smith2024/1","title":"第二章"})),
        item(1,"edit_card",json!({"at":"@@i0","base_receipt":"run/unit/1/apply/item-0","body":"第二章的要点。"})),
        item(2,"create_successor",json!({"at":"@@i0","kind":"side","title":"旁注"})),
    ]}));
    assert_eq!(done["results"][0]["result"]["address"], "Smith2024/2");
    writer.ok("edit_card", json!({"at":"Smith2024/2|a","body":"旁注的要点。"}));
}

#[test]
fn a_replayed_create_counts_as_the_replaying_sessions_card() {
    let scratch = ScratchBox::up();
    seeded(&scratch);
    let list = scratch.list_file("mine.allow.txt", &["Smith2024/1"]);
    let create = item(0, "create_successor", json!({"at":"Smith2024/1","title":"第二章"}));
    scratch.mcp(&["--allow", &list]).ok("apply_batch", json!({"items":[create.clone()]}));
    // A resumed run replays its receipt in a new connection and then continues the Card it made.
    let mut resumed = scratch.mcp(&["--allow", &list]);
    let done = resumed.ok("apply_batch", json!({"items":[
        create,
        item(1,"edit_card",json!({"at":"@@i0","base_receipt":"run/unit/1/apply/item-0","body":"续写。"})),
    ]}));
    assert_eq!(done["results"][0]["replayed"], true);
    assert_eq!(done["results"][1]["result"]["address"], "Smith2024/2");
}

#[test]
fn a_preview_reports_each_scope_refusal_and_keeps_checking_independent_items() {
    let scratch = ScratchBox::up();
    let mut open = seeded(&scratch);
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let draft = literature.ok("apply_batch", json!({"preview":true,"items":[
        item(0,"create_successor",json!({"at":"0/1","title":"主题卡"})),
        item(1,"create_successor",json!({"at":"@@i0","title":"依赖被拒的卡"})),
        item(2,"create_successor",json!({"at":"Smith2024/1","title":"独立的卡"})),
    ]}));
    assert_eq!(draft["complete"], false, "{draft}");
    assert_eq!(draft["refusals"].as_array().unwrap().len(), 1, "{draft}");
    assert_eq!(draft["refusals"][0]["index"], 0);
    assert_eq!(draft["refusals"][0]["refusal"]["error"], "scope_refused");
    assert_eq!(draft["refusals"][0]["refusal"]["reason"], "topic_tree");
    assert_eq!(draft["blocked"][0]["index"], 1, "{draft}");
    assert_eq!(draft["results"].as_array().unwrap().len(), 1);
    assert_eq!(draft["results"][0]["result"]["address"], "Smith2024/2");
    assert_eq!(open.ok("get_tree", json!({"tree":"Smith2024"}))["cards"].as_array().unwrap().len(), 2);
}

#[test]
fn an_agent_topic_with_the_same_title_is_refused_in_every_scope() {
    let scratch = ScratchBox::up();
    let mut open = scratch.mcp(&[]);
    open.ok("create_topic", json!({"title":"Price Theory"}));
    // The same title once whitespace, punctuation and case are ignored.
    let refusal = open.refused("create_topic", json!({"title":"price, theory!"}));
    assert_eq!(refusal["error"], "topic_exists", "{refusal}");
    assert_eq!(refusal["root"], "0/0");
    assert_eq!(refusal["existing_title"], "Price Theory");
    assert_eq!(refusal["similarity"], 1.0);
    // The same rule holds for a create_topic item of a batch, which then stores nothing.
    let batch = open.refused("apply_batch", json!({"items":[item(0,"create_topic",json!({"title":"PRICE THEORY"}))]}));
    assert_eq!(batch["error"], "topic_exists", "{batch}");
    assert_eq!(open.ok("list_trees", json!({}))["trees"].as_array().unwrap().len(), 1);
}

#[test]
fn by_default_a_similar_but_different_title_is_a_new_topic() {
    let scratch = ScratchBox::up();
    let mut open = scratch.mcp(&[]);
    // Each pair reaches bigram similarity 0.8 or more and still names another subject.
    for (first, second) in [("Topic 0", "Topic 1"), ("Related", "Unrelated 0"), ("Price Theory", "Price Theories")] {
        open.ok("create_topic", json!({"title":first}));
        open.ok("create_topic", json!({"title":second}));
    }
    assert_eq!(open.ok("list_trees", json!({}))["trees"].as_array().unwrap().len(), 6);
}

#[test]
fn the_owner_may_refuse_near_titles_with_an_archive_threshold() {
    let scratch = ScratchBox::up();
    let mut open = scratch.mcp(&[]);
    open.ok("create_topic", json!({"title":"Price Theory"}));
    scratch.cmd().args(["config", "set", "scope.topic_similarity", "0.8"]).assert().success()
        .stdout("scope.topic_similarity=0.8\n");
    // Bigram similarity 0.857 reaches 0.8.
    let near = open.refused("create_topic", json!({"title":"Price Theories"}));
    assert_eq!(near["error"], "topic_exists", "{near}");
    assert_eq!(near["similarity"], 0.857);
    scratch.cmd().args(["config", "set", "scope.topic_similarity", "default"]).assert().success()
        .stdout("scope.topic_similarity=default\n");
    open.ok("create_topic", json!({"title":"Price Theories"}));
    for bad in ["0", "1.5", "most"] {
        scratch.cmd().args(["config", "set", "scope.topic_similarity", bad]).assert().failure()
            .stderr(predicates::str::contains("must be a number above 0 and at most 1"));
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
