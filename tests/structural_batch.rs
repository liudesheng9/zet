//! Structural items in `apply_batch` (PRD #228, S3 #231; docs/tree-operations-and-batch-proposal.md
//! Part B, D1-D3): moves, inserts and deletes commit atomically with creates and edits; every address
//! names a Card on the batch's start state and follows it through earlier items; a plan hash from the
//! preview confirms the batch.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

/// An agent Topic `0/0` whose Card `0/1` has the Side successors 甲 `0/1|a`, 乙 `0/1|b`, 丙 `0/1|c`,
/// then `0/2` 二 (linking 甲 and 丙) with a Side successor 二甲 `0/2|a`, seeded by one batch so the
/// later edit can name its receipt.
fn seeded(scratch: &ScratchBox) -> Mcp {
    let mut mcp = scratch.mcp(&[]);
    let create = |n: usize, at: &str, kind: &str, title: &str, body: &str| {
        json!({"op_key": format!("seed/tree/1/create/i{n}"), "client_ref": format!("s{n}"), "tool": if at.is_empty() { "create_topic" } else { "create_successor" },
               "arguments": if at.is_empty() { json!({"title": title}) } else { json!({"at": at, "kind": kind, "title": title, "body": body}) }})
    };
    let items = json!([
        create(0, "", "", "书", ""),
        create(1, "0/0", "direct", "一", "一的正文。"),
        create(2, "0/1", "side", "甲", "甲的正文。"),
        create(3, "0/1", "side", "乙", "乙的正文。"),
        create(4, "0/1", "side", "丙", "丙的正文。"),
        create(5, "0/1", "direct", "二", "二的正文。见 [[0/1|a]] 和 [[0/1|c]]。"),
        create(6, "0/2", "side", "二甲", "二甲的正文。见 [[0/2]]。"),
    ]);
    let seeded = mcp.ok("apply_batch", json!({"items": items}));
    let addresses: Vec<_> = seeded["results"].as_array().unwrap().iter().map(|r| r["result"]["address"].clone()).collect();
    assert_eq!(addresses, ["0/0", "0/1", "0/1|a", "0/1|b", "0/1|c", "0/2", "0/2|a"].map(Value::from));
    mcp
}

fn title(mcp: &mut Mcp, at: &str) -> String {
    mcp.ok("get_card", json!({"at": at}))["title"].as_str().unwrap().to_string()
}

fn item(n: usize, tool: &str, arguments: Value) -> Value {
    json!({"op_key": format!("tidy/tree/1/move/i{n}"), "client_ref": format!("t{n}"), "tool": tool, "arguments": arguments})
}

/// One reorganization: an edit, a delete with `into`, then three moves named by start-state addresses
/// (`before`, `side_of`, `after`). 甲 moves twice, first under 丙, then to 二, and is named `0/1|a`
/// both times.
fn reorganization() -> Value {
    json!([
        item(0, "edit_card", json!({"at": "0/2", "base_receipt": "seed/tree/1/create/i5", "body": "二的正文。先见 [[0/1|c]]，再见 [[0/1|a]]。"})),
        item(1, "delete_card", json!({"at": "0/2|a", "into": "0/2"})),
        item(2, "move_card", json!({"at": "0/1|c", "before": "0/1|a"})),
        item(3, "move_card", json!({"at": "0/1|a", "side_of": "0/2"})),
        item(4, "move_card", json!({"at": "0/1|b", "after": "0/2"})),
    ])
}

fn links_ok(scratch: &ScratchBox) {
    let out = scratch.cmd().args(["status", "--check-links"]).output().expect("check links");
    let report: Value = serde_json::from_slice(&out.stdout).expect("check report");
    assert_eq!(report["ok"], true, "{report}");
}

/// S3-AC: a preview returns the composed plan and its hash; a real call without the hash answers the
/// plan and writes nothing; with the hash the whole batch commits; every address followed its Card.
#[test]
fn a_structural_batch_previews_plans_then_commits_atomically_with_start_state_addresses() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let items = reorganization();

    let preview = mcp.ok("apply_batch", json!({"items": items, "preview": true}));
    assert_eq!(preview["complete"], true, "{preview}");
    let plan_hash = preview["plan_hash"].as_str().expect("a preview names its plan hash").to_string();
    let plan = &preview["plan"];
    assert_eq!(plan["moved"]["0/1|a"], "0/2|a", "甲 became the first Side successor of 二: {plan}");
    assert_eq!(plan["moved"]["0/1|c"], "0/1|a", "丙 took 甲's place: {plan}");
    assert_eq!(plan["moved"]["0/1|b"], "0/3", "乙 became the Direct successor of 二: {plan}");
    assert_eq!(plan["deleted"], json!(["0/2|a"]));
    assert!(plan["link_macros_rewritten"].as_u64().unwrap() >= 2, "{plan}");

    let unconfirmed = mcp.ok("apply_batch", json!({"items": items}));
    assert_eq!(unconfirmed["applied"], false, "a structural batch without its hash only plans: {unconfirmed}");
    assert_eq!(unconfirmed["confirm"], json!(plan_hash));
    assert_eq!(title(&mut mcp, "0/1|a"), "甲", "nothing was written");
    assert_eq!(title(&mut mcp, "0/2|a"), "二甲");

    let done = mcp.ok("apply_batch", json!({"items": items, "confirm": plan_hash}));
    assert_eq!(done["applied"], true, "{done}");
    assert_eq!(done["plan"]["moved"], plan["moved"]);
    assert_eq!(title(&mut mcp, "0/2|a"), "甲", "甲 holds the label the deleted 二甲 left");
    assert_eq!(title(&mut mcp, "0/3"), "乙");
    assert_eq!(title(&mut mcp, "0/1|a"), "丙");
    let body = mcp.ok("get_card", json!({"at": "0/2"}))["body"].as_str().unwrap().to_string();
    assert!(body.contains("先见 [[0/1|a]]，再见 [[0/2|a]]"), "the edit's Links follow the moves: {body}");
    let tree = mcp.ok("get_tree", json!({"tree": "0"})).to_string();
    assert!(!tree.contains("二甲"), "二甲 was deleted: {tree}");
    links_ok(&scratch);

    // A lost reply: the same request replays every receipt and writes nothing, whatever the revision.
    let tree = mcp.ok("get_tree", json!({"tree": "0", "bodies": true}));
    let again = mcp.ok("apply_batch", json!({"items": items, "confirm": plan_hash}));
    assert!(again["results"].as_array().unwrap().iter().all(|r| r["replayed"] == true), "{again}");
    assert_eq!(mcp.ok("get_tree", json!({"tree": "0", "bodies": true})), tree, "a replay wrote nothing");
}

/// S3-AC: a hash from before an unrelated change answers `plan_changed` and writes nothing.
#[test]
fn a_plan_hash_from_before_a_change_is_refused() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let items = json!([item(0, "move_card", json!({"at": "0/1|c", "before": "0/1|a"}))]);
    let preview = mcp.ok("apply_batch", json!({"items": items, "preview": true}));
    let plan_hash = preview["plan_hash"].clone();
    mcp.ok("create_successor", json!({"at": "0/2", "kind": "side", "title": "后来"}));
    let refusal = mcp.refused("apply_batch", json!({"items": items, "confirm": plan_hash}));
    assert!(refusal.to_string().contains("plan_changed"), "{refusal}");
    assert_eq!(title(&mut mcp, "0/1|c"), "丙", "nothing moved");
}

/// S3-AC: inside a batch a destination names a Card, so `to` is refused; a Card an earlier item
/// deleted cannot be named again; a literal names a Card that existed when the batch started.
#[test]
fn a_structural_batch_refuses_slots_deleted_cards_and_unknown_start_addresses() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let to = mcp.refused("apply_batch", json!({"items": [item(0, "move_card", json!({"at": "0/1|c", "to": "0/2|b"}))], "preview": true}));
    assert!(to.to_string().contains("side_of"), "{to}");
    let gone = mcp.refused(
        "apply_batch",
        json!({"items": [item(0, "delete_card", json!({"at": "0/1|b"})), item(1, "move_card", json!({"at": "0/1|b", "side_of": "0/2"}))], "preview": true}),
    );
    assert!(gone.to_string().contains("card_deleted_in_batch"), "{gone}");
    let unknown = mcp.refused("apply_batch", json!({"items": [item(0, "move_card", json!({"at": "0/9", "side_of": "0/2"}))], "preview": true}));
    assert!(unknown.to_string().contains("start of this batch"), "{unknown}");
    assert_eq!(title(&mut mcp, "0/1|b"), "乙");
}

/// S3-AC: a batch of only creates and edits keeps today's contract: it needs no hash and returns
/// only its results.
#[test]
fn a_batch_of_creates_and_edits_needs_no_hash() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let done = mcp.ok(
        "apply_batch",
        json!({"items": [item(0, "create_successor", json!({"at": "0/2", "kind": "side", "title": "新"}))]}),
    );
    assert_eq!(done["results"][0]["result"]["address"], "0/2|b");
    assert!(done.get("applied").is_none() && done.get("plan_hash").is_none(), "{done}");
}
