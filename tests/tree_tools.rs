//! The span, outline, reorder and relink tools (PRD #228; S4 #232, S5 #233;
//! docs/tree-operations-and-batch-proposal.md Part C) over a real `zt mcp`: each plans first and
//! applies on its confirmation word (relink applies at once), and each is a batch item too.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

/// Topic `0/0` with the chain 一 `0/1` .. 五 `0/5`; 二 `0/2` has the Side branch 二甲 `0/2|a`; 五 links
/// 二 and 三; 一 has the Side successors 甲 `0/1|a`, 乙 `0/1|b`, 丙 `0/1|c`.
fn seeded(scratch: &ScratchBox) -> Mcp {
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "书"}));
    for (at, title, body) in [
        ("0/0", "一", "一的正文。"),
        ("0/1", "二", "二的正文。"),
        ("0/2", "三", "三的正文。"),
        ("0/3", "四", "四的正文。"),
        ("0/4", "五", "五的正文。见 [[0/2]] 和 [[0/3]]。"),
    ] {
        mcp.ok("create_successor", json!({"at": at, "title": title, "body": body}));
    }
    mcp.ok("create_successor", json!({"at": "0/2", "kind": "side", "title": "二甲", "body": "二甲的正文。"}));
    for title in ["甲", "乙", "丙"] {
        mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "title": title, "body": format!("{title}的正文。")}));
    }
    mcp
}

fn title(mcp: &mut Mcp, at: &str) -> String {
    mcp.ok("get_card", json!({"at": at}))["title"].as_str().unwrap_or_default().to_string()
}

fn body(mcp: &mut Mcp, at: &str) -> String {
    mcp.ok("get_card", json!({"at": at}))["body"].as_str().unwrap_or_default().to_string()
}

fn links_ok(scratch: &ScratchBox) {
    let out = scratch.cmd().args(["status", "--check-links"]).output().expect("check links");
    let report: Value = serde_json::from_slice(&out.stdout).expect("check report");
    assert_eq!(report["ok"], true, "{report}");
}

/// S4-AC: `move_span` plans first, then moves a span with its Side successors; the chain closes up
/// and Links follow.
#[test]
fn move_span_plans_then_moves_a_span_with_its_branches() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let request = json!({"from": "0/2", "through": "0/3", "side_of": "0/5"});
    let plan = mcp.ok("move_span", request.clone());
    assert_eq!(plan["applied"], false, "{plan}");
    assert_eq!(plan["confirm"], "move");
    assert_eq!(title(&mut mcp, "0/2"), "二", "a plan moves nothing");
    let mut confirmed = request;
    confirmed["confirm"] = json!("move");
    let done = mcp.ok("move_span", confirmed);
    assert_eq!(done["applied"], true, "{done}");
    // 四 and 五 close up to 0/2 and 0/3; the span becomes 五's Side branch with 二甲 still on 二.
    assert_eq!(title(&mut mcp, "0/2"), "四");
    assert_eq!(title(&mut mcp, "0/3"), "五");
    assert_eq!(title(&mut mcp, "0/3|a"), "二");
    assert_eq!(title(&mut mcp, "0/3|a|a"), "二甲");
    assert_eq!(title(&mut mcp, "0/3|a|1"), "三");
    assert!(body(&mut mcp, "0/3").contains("见 [[0/3|a]] 和 [[0/3|a|1]]"), "{}", body(&mut mcp, "0/3"));
    links_ok(&scratch);
}

/// S4-AC: `delete_span` closes the chain; a Link to the span that would land on a Card that moved
/// up is refused unless `into` re-points it.
#[test]
fn delete_span_closes_the_chain_and_needs_into_for_links_that_would_land() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let refusal = mcp.refused("delete_span", json!({"from": "0/2", "through": "0/3", "confirm": "delete"}));
    assert!(refusal.to_string().contains("pass `into`"), "{refusal}");
    assert_eq!(title(&mut mcp, "0/2"), "二");
    let done = mcp.ok("delete_span", json!({"from": "0/2", "through": "0/3", "into": "0/1", "confirm": "delete"}));
    assert_eq!(done["applied"], true, "{done}");
    assert_eq!(title(&mut mcp, "0/2"), "四", "the rest of the chain moved up");
    assert_eq!(title(&mut mcp, "0/3"), "五");
    assert!(body(&mut mcp, "0/3").contains("见 [[0/1]] 和 [[0/1]]"), "{}", body(&mut mcp, "0/3"));
    let tree = mcp.ok("get_tree", json!({"tree": "0"})).to_string();
    assert!(!tree.contains("二甲"), "the span's Side branch went with it");
    links_ok(&scratch);
}

/// S4-AC: indent then outdent of the same Card restores every Location.
#[test]
fn indent_then_outdent_restores_every_location() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    // The tree without its revision, which every write advances.
    let tree = |mcp: &mut Mcp| {
        let mut tree = mcp.ok("get_tree", json!({"tree": "0", "bodies": true}));
        tree.as_object_mut().unwrap().remove("archive_revision");
        tree
    };
    let before = tree(&mut mcp);
    let indented = mcp.ok("indent", json!({"at": "0/3", "confirm": "move"}));
    assert_eq!(indented["applied"], true, "{indented}");
    assert_eq!(title(&mut mcp, "0/2|b"), "三", "三 became 二's last Side branch");
    assert_eq!(title(&mut mcp, "0/3"), "四");
    mcp.ok("outdent", json!({"at": "0/2|b", "confirm": "move"}));
    assert_eq!(tree(&mut mcp), before);
    let refusal = mcp.refused("indent", json!({"at": "0/1", "confirm": "move"}));
    assert!(refusal.to_string().contains("nothing before"), "{refusal}");
    links_ok(&scratch);
}

/// S5-AC: `reorder` puts Side successors in a new order with their branches; a list that misses a
/// child is refused and moves nothing.
#[test]
fn reorder_puts_side_successors_in_order_and_refuses_an_incomplete_list() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let refusal = mcp.refused("reorder", json!({"parent": "0/1", "side": ["0/1|c", "0/1|a"], "confirm": "move"}));
    assert!(refusal.to_string().contains("reorder_set_mismatch"), "{refusal}");
    assert_eq!(title(&mut mcp, "0/1|a"), "甲");
    let done = mcp.ok("reorder", json!({"parent": "0/1", "side": ["0/1|c", "0/1|b", "0/1|a"], "confirm": "move"}));
    assert_eq!(done["applied"], true, "{done}");
    assert_eq!(
        ["0/1|a", "0/1|b", "0/1|c"].map(|at| title(&mut mcp, at)),
        ["丙", "乙", "甲"]
    );
    links_ok(&scratch);
}

/// S5-AC: `relink` re-points ordinary Links and Term uses without deleting anything; a Term use
/// keeps its label and must still name a Term head.
#[test]
fn relink_repoints_links_and_term_uses_and_keeps_labels() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let list = mcp.ok("create_successor", json!({"at": "0/5", "kind": "side", "type": "term", "title": "术语表", "body": "本书的术语。"}))["address"].clone();
    let list = list.as_str().unwrap();
    let old = mcp.ok("create_successor", json!({"at": list, "kind": "side", "type": "term", "term": "旧词", "title": "旧词", "body": "术语：旧词"}))["address"].clone();
    let new = mcp.ok("create_successor", json!({"at": list, "kind": "side", "type": "term", "term": "新词", "title": "新词", "body": "术语：新词"}))["address"].clone();
    let (old, new) = (old.as_str().unwrap().to_string(), new.as_str().unwrap().to_string());
    mcp.ok("edit_card", json!({"at": "0/4", "body": format!("四用[[词@{old}]]，也见 [[{old}]]。")}));
    let done = mcp.ok("relink", json!({"from": old, "to": new}));
    assert_eq!(done["applied"], true, "{done}");
    assert_eq!(done["link_macros_rewritten"], 2, "{done}");
    assert_eq!(body(&mut mcp, "0/4"), format!("四用[[词@{new}]]，也见 [[{new}]]。"));
    assert_eq!(title(&mut mcp, &old), "旧词", "relink deletes nothing");
    // A Term use must name a Term head: relinking it to an ordinary Card is refused.
    let refusal = mcp.refused("relink", json!({"from": new, "to": "0/1"}));
    assert!(refusal.to_string().to_lowercase().contains("term"), "{refusal}");
    let stray = mcp.refused("relink", json!({"from": new, "to": old, "sources": ["0/2"]}));
    assert!(stray.to_string().contains("has no Link"), "{stray}");
    links_ok(&scratch);
}

/// S4/S5-AC: the new tools are batch items with start-state addresses: a span move, then an indent
/// named by a start address the move renamed.
#[test]
fn the_new_tools_are_batch_items_with_start_state_addresses() {
    let scratch = ScratchBox::up();
    let mut mcp = seeded(&scratch);
    let item = |n: usize, tool: &str, arguments: Value| {
        json!({"op_key": format!("tidy/span/1/step/i{n}"), "client_ref": format!("s{n}"), "tool": tool, "arguments": arguments})
    };
    let items = json!([
        item(0, "move_span", json!({"from": "0/4", "through": "0/5", "before": "0/2"})),
        item(1, "indent", json!({"at": "0/3"})),
        item(2, "reorder", json!({"parent": "0/1", "side": ["0/1|b", "0/1|a", "0/1|c"]})),
    ]);
    let preview = mcp.ok("apply_batch", json!({"items": items, "preview": true}));
    assert_eq!(preview["complete"], true, "{preview}");
    let done = mcp.ok("apply_batch", json!({"items": items, "confirm": preview["plan_hash"]}));
    assert_eq!(done["applied"], true, "{done}");
    let moved = &done["plan"]["moved"];
    // 三 (start 0/3) was renamed by the span move, then indented under the Card before it.
    let three = moved["0/3"].as_str().expect("三 moved");
    assert_eq!(title(&mut mcp, three), "三");
    assert!(three.contains('|'), "三 is now a Side branch: {three}");
    assert_eq!(title(&mut mcp, "0/1|a"), "乙");
    links_ok(&scratch);
}

/// S4/S5-AC: a connection under an allow list may not run the new tools, single or batched.
#[test]
fn an_allow_list_refuses_the_new_tools() {
    let scratch = ScratchBox::up();
    drop(seeded(&scratch));
    let list = scratch.list_file("allow.txt", &["0/2", "0/3"]);
    let mut listed = scratch.mcp(&["--allow", &list]);
    for (tool, arguments) in [
        ("move_span", json!({"from": "0/2", "side_of": "0/3"})),
        ("delete_span", json!({"from": "0/2"})),
        ("indent", json!({"at": "0/3"})),
        ("outdent", json!({"at": "0/2|a"})),
        ("reorder", json!({"parent": "0/1", "side": ["0/1|b", "0/1|a", "0/1|c"]})),
        ("relink", json!({"from": "0/2", "to": "0/3"})),
    ] {
        let refusal = listed.refused(tool, arguments);
        assert_eq!(refusal["error"], "scope_refused", "{tool}: {refusal}");
        assert_eq!(refusal["reason"], "structural", "{tool}: {refusal}");
    }
}
