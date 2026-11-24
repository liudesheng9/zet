//! Regressions from the adversarial review of multi-tree coordinates (Astra, 2026-11-24, P2):
//! receipts and revisions, impossible splices, delete previews, Dependency Lens membership and
//! order, the third role in public readers, and review provenance.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;

use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn revision(mcp: &mut Mcp) -> u64 {
    mcp.ok("list_connection_trees", json!({}))["archive_revision"].as_u64().unwrap()
}

fn book(mcp: &mut Mcp, key: &str) {
    mcp.ok("create_literature", json!({"bibtex": format!("@book{{{key}, title={{Book {key}}}}}")}));
    mcp.ok("create_successor", json!({"at": key, "title": format!("{key} 一"), "body": format!("一。\n\n来源：[[{key}]] §1")}));
}

/// P2.9: a keyed no-op keeps its receipt (a retry after a later removal replays, never acts
/// again), and one write advances the revision exactly once.
#[test]
fn keyed_no_ops_keep_receipts_and_writes_advance_one_revision() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Rc2020");
    mcp.ok("create_connection_tree", json!({"title": "Receipts"}));
    let before = revision(&mut mcp);
    mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "一张", "body": "内容。"}));
    assert_eq!(revision(&mut mcp), before + 1, "one write, one revision");
    let placed = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Rc2020"}));
    let again = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Rc2020", "op_key": "k-place"}));
    assert_eq!(again["already_placed"], true);
    let plan = mcp.ok("unplace", json!({"placement_id": placed["placement_id"]}));
    mcp.ok("unplace", json!({"placement_id": placed["placement_id"], "confirm": plan["confirm"], "plan_hash": plan["plan_hash"], "expected_revision": plan["expected_revision"]}));
    let retry = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Rc2020", "op_key": "k-place"}));
    assert_eq!(retry["replayed"], true, "{retry}");
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(view["placements"].as_array().unwrap().len(), 2, "the retry made no new station: {view}");
}

/// P2.10 and P2.11: a reattach the root cannot take is refused before any plan; Side ordinals stay
/// dense; delete previews name the stations and mounts a delete removes or leaves missing.
#[test]
fn impossible_splices_are_refused_and_delete_previews_name_every_loss() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Sp2020");
    mcp.create_tree_with_sides();
    let refused = mcp.refused("unplace", json!({"placement_id": "~1/1", "children": "reattach"}));
    assert!(refused.to_string().contains("root takes no Side"), "{refused}");
    // Keep-root names the stations and connections it removes, even when no Card goes.
    let out = scratch.cmd().args(["del", "--at", "~1", "--keep-root"]).write_stdin("no\n").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("keeps its root station; its other 3 station(s) and 1 tree connection(s) are removed"), "{text}");
    // A native Card's delete names the Missing mount it leaves in its own tree.
    let native = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1|b", "edge": "direct", "title": "原生", "body": "内容。"}));
    let plan = mcp.ok("delete_card", json!({"at": native["handle"]}));
    assert!(plan.to_string().contains("as a Missing mount"), "{plan}");
    // A Topic Card's delete names the mounts beside it.
    mcp.ok("create_topic", json!({"title": "T", "body": "t"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "One", "body": "one"}));
    mcp.ok("place_card", json!({"anchor_at": "0/1", "edge": "attach", "target_at": "Sp2020/1"}));
    let plan = mcp.ok("delete_card", json!({"at": "0/1"}));
    assert!(plan.to_string().contains("1 Topic-side mount(s) beside"), "{plan}");
}

trait Fixture {
    fn create_tree_with_sides(&mut self);
}

impl Fixture for Mcp {
    /// `~1/1` (a mount) with two Side branches and no Direct continuation, under the root.
    fn create_tree_with_sides(&mut self) {
        self.ok("create_connection_tree", json!({"title": "Sides"}));
        self.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Sp2020"}));
        self.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "edge": "side", "title": "甲", "body": "甲。"}));
        self.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "edge": "side", "title": "乙", "body": "乙。"}));
        self.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/1|a", "to_placement_id": "~1/1|b"}));
    }
}

/// P2.12 and P2.13: a unit's lens reads every part the Model report counts (a Direct part too),
/// reports a proof step that reaches into another unit, keeps a self-citation as a cycle, refuses
/// a chosen order that skips a Card or puts a dependent first, and counts an "objects to" label as
/// no authored dependence.
#[test]
fn the_lens_follows_model_membership_and_checks_chosen_orders() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex": "@book{Lens2020, title={Lens}}"}));
    mcp.ok("create_successor", json!({"at": "Lens2020", "title": "第一章", "body": "章。\n\n来源：[[Lens2020]] §1"}));
    let def = address(&mcp.ok("create_successor", json!({"at": "Lens2020/1", "kind": "side", "type": "model", "form": "definition", "level": "concept", "title": "定义 D：紧致", "body": "**定义**：\n- (C1) 紧致。"})));
    let other = address(&mcp.ok("create_successor", json!({"at": "Lens2020/1", "kind": "side", "type": "model", "form": "theorem", "level": "formal", "title": "定理 S：另一个定理", "body": "**陈述**：\n- (H1) 另一个结论。"})));
    let s_premise = address(&mcp.ok("create_successor", json!({"at": other, "kind": "side", "facet": "前提", "title": "前提：S 的前提", "body": "前提。"})));
    let s_step = address(&mcp.ok("create_successor", json!({"at": s_premise, "facet": "证明：S 的一步", "title": "证明：S 的一步", "body": format!("依赖 [[{s_premise}]]。")})));
    let head = address(&mcp.ok("create_successor", json!({"at": "Lens2020/1", "kind": "side", "type": "model", "form": "theorem", "level": "formal", "title": "定理 T：主定理", "body": "**陈述**：\n- (H1) 主结论。"})));
    let premise = address(&mcp.ok("create_successor", json!({"at": head, "kind": "side", "facet": "前提", "title": "前提：T 的前提", "body": "前提。"})));
    // A Direct part after the premise, which the head does not link, still belongs to T.
    let step = address(&mcp.ok("create_successor", json!({"at": premise, "facet": "证明：用 D", "title": "证明：用 D", "body": format!("依赖 [[{premise}]]。依据 [[{def}]]：紧致。依据 [[{s_step}]]：借用 S 的一步。")})));
    let lens = mcp.ok("get_dependencies", json!({"at": head, "scope": "unit"}));
    assert!(lens["carriers"].as_array().unwrap().iter().any(|c| c == step.as_str()), "{lens}");
    let prereqs: Vec<&str> = lens["prerequisites"].as_array().unwrap().iter().filter_map(|p| p["address"].as_str()).collect();
    assert!(prereqs.contains(&def.as_str()), "{lens}");
    assert!(lens["reach_in"].as_array().unwrap().iter().any(|r| r["target"] == s_step.as_str()), "a proof step of S is reached into: {lens}");
    // A Card citing itself is a cycle.
    mcp.ok("create_topic", json!({"title": "Self", "body": "self"}));
    let me = address(&mcp.ok("create_successor", json!({"at": "0/0", "title": "自指", "body": "内容。"})));
    mcp.ok("edit_card", json!({"at": me, "body": format!("依据 [[{me}]]：自己。")}));
    let selfish = mcp.ok("get_dependencies", json!({"at": me}));
    assert_eq!(selfish["cycles"].as_array().unwrap().len(), 1, "{selfish}");
    // A chosen order must name every Card once and keep prerequisites first outside a cycle.
    let a = address(&mcp.ok("create_successor", json!({"at": me, "title": "甲", "body": "甲。"})));
    let b = address(&mcp.ok("create_successor", json!({"at": a, "title": "乙", "body": format!("依据 [[{a}]]。")})));
    let refused = mcp.refused("seed_connection_tree", json!({"from": "dependencies", "source_params": {"at": b, "order": [b]}, "title": "x"}));
    assert!(refused.to_string().contains("every Card"), "{refused}");
    let refused = mcp.refused("seed_connection_tree", json!({"from": "dependencies", "source_params": {"at": b, "order": [b, a]}, "title": "x"}));
    assert!(refused.to_string().contains("prerequisite"), "{refused}");
    // An "objects to" connection is no authored dependence in the overlay.
    mcp.ok("create_connection_tree", json!({"title": "Overlay"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": a}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": b}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/2", "to_placement_id": "~1/1", "direction": "directed", "relation_label": "objects to"}));
    let overlaid = mcp.ok("get_dependencies", json!({"at": b, "overlay_tree": "~1"}));
    let classes: Vec<&str> = overlaid["overlay"]["connections"].as_array().unwrap().iter().filter_map(|c| c["class"].as_str()).collect();
    assert_eq!(classes, vec!["tree_local"], "{overlaid}");
}

/// P2.16: the third role in public readers: list_trees lists Connection trees only when asked by
/// zone; export_titles' Topic zone holds no Connection-tree Card; zt lsbk names Missing mounts.
#[test]
fn public_readers_know_the_connection_role() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Pr2020");
    mcp.ok("create_topic", json!({"title": "Topic", "body": "t"}));
    mcp.ok("create_connection_tree", json!({"title": "Readers"}));
    mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "原生", "body": "内容。"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Pr2020/1"}));
    let all = mcp.ok("list_trees", json!({}));
    assert!(!all.to_string().contains("\"~1\""), "two Zones by default: {all}");
    let connection = mcp.ok("list_trees", json!({"zone": "connection"}));
    assert_eq!(connection["trees"][0]["tree"], "~1", "{connection}");
    assert_eq!(connection["trees"][0]["kind"], "connection");
    assert_eq!(connection["trees"][0]["count"], 2, "root and one native: {connection}");
    let topics = mcp.ok("export_titles", json!({"zone": "topic"}));
    assert!(!topics.to_string().contains("~1"), "{topics}");
    let plan = mcp.ok("delete_card", json!({"at": "Pr2020/1"}));
    mcp.ok("delete_card", json!({"at": "Pr2020/1", "confirm": plan["confirm"]}));
    let out = scratch.cmd().arg("lsbk").output().unwrap();
    assert!(String::from_utf8_lossy(&out.stdout).contains("missing mount ~1/2 in ~1: Pr2020/1"), "{}", String::from_utf8_lossy(&out.stdout));
}

/// P2.17: a source review names the Lock it was shown under; after an unlock and a new Lock it is
/// refused as stale.
#[test]
fn a_review_shown_under_one_lock_is_refused_under_another() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Rv2020");
    let gui = Gui::connect(scratch.cmd());
    mcp.ok("create_connection_tree", json!({"title": "Reviewed"}));
    let a = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Rv2020/1"}));
    let first = gui.lock("~1");
    mcp.ok("edit_card", json!({"at": "Rv2020/1", "body": "改。\n\n来源：[[Rv2020]] §1"}));
    let drift = mcp.ok("get_connection_tree", json!({"tree": "~1"}))["placements"][1]["drift"].clone();
    gui.unlock(first["lock_id"].as_i64().unwrap());
    gui.lock("~1");
    let refused = gui.conn_refused("review_source", json!({"placement_id": a["placement_id"], "reviewed_fp": drift["current_fp"], "lock_id": drift["lock_id"]}));
    assert!(refused.to_string().contains("stale review"), "{refused}");
}
