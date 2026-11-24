//! Regressions from the third adversarial review of multi-tree coordinates (Astra, 2026-11-24):
//! Literature-scoped rewrites checked on the real mapping, keep-root delete previews, handle Links
//! in summaries and portable routes, shared-part users, overlay kinds, the `来源` line rule, and
//! dense Topic-side shelves in `zt status`.

#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};
use std::fs;

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn book(mcp: &mut Mcp, key: &str) {
    mcp.ok("create_literature", json!({"bibtex": format!("@book{{{key}, title={{Book {key}}}}}")}));
    mcp.ok("create_successor", json!({"at": key, "title": format!("{key} 一"), "body": format!("一。\n\n来源：[[{key}]] §1")}));
}

fn finding_body(source: &str) -> String {
    format!("**测量**：乡镇工业产值。\n**数据**：1984年县级统计。\n**方法**：县际比较。\n**结果**：产值上升约三成。\n**局限**：样本只有十个县。\n{source}")
}

/// P1.2: under the Literature scope, a delete whose compaction renumbers a sibling that a
/// Connection-tree Card cites is refused, although the deleted Card itself is uncited; a metadata
/// edit that keeps the Citation key passes although a Connection-tree Card cites the book; a Link
/// by permanent handle is never a reason to refuse.
#[test]
fn literature_scope_checks_the_real_rewrite_mapping() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Cmp2020");
    let a = address(&mcp.ok("create_successor", json!({"at": "Cmp2020/1", "kind": "side", "title": "甲", "body": "甲。\n\n来源：[[Cmp2020]] §1"})));
    let b = address(&mcp.ok("create_successor", json!({"at": "Cmp2020/1", "kind": "side", "title": "乙", "body": "乙。\n\n来源：[[Cmp2020]] §1"})));
    assert_eq!((a.as_str(), b.as_str()), ("Cmp2020/1|a", "Cmp2020/1|b"));
    mcp.ok("create_connection_tree", json!({"title": "Citer"}));
    mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "引用者", "body": format!("见 [[Cmp2020]] 与 [[{b}]]。")}));
    let mut literature = scratch.mcp(&["--scope", "literature"]);
    let refused = literature.refused("delete_card", json!({"at": a}));
    let text = refused.to_string();
    assert!(text.contains("connection_tree") && text.contains("~1#1"), "{refused}");
    // A Side insert before the cited sibling shifts it: refused too (fourth review).
    let refused = literature.refused("create_successor", json!({"at": "Cmp2020/1", "kind": "side", "before": b, "title": "插入", "body": "插入。\n\n来源：[[Cmp2020]] §1"}));
    assert!(refused.to_string().contains("connection_tree"), "{refused}");
    // The same key: no Link is rewritten, so nothing is refused.
    literature.ok("edit_literature_metadata", json!({"at": "Cmp2020", "bibtex": "@book{Cmp2020, title={A new title}}"}));
    // The last Side Card compacts nothing: its delete rewrites no Connection-tree Card (the Link
    // that cited it becomes broken, which is the ordinary delete rule).
    literature.ok("delete_card", json!({"at": b}));
}

/// P2.11: `del ~1 --keep-root` with one native Card and no mounts elsewhere predicts no Missing
/// mount: the emptied tree removes the native Card's home station with the rest.
#[test]
fn keep_root_predicts_no_missing_mount_for_its_own_natives() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_connection_tree", json!({"title": "Alone"}));
    mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "原生", "body": "内容。"}));
    let out = scratch.cmd().args(["del", "--at", "~1", "--keep-root"]).write_stdin("no\n").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("keeps its root station"), "{text}");
    assert!(!text.contains("Missing mount"), "{text}");
    // Deleting the native Card alone names its home station once, never as a station elsewhere.
    let plan = mcp.ok("delete_card", json!({"at": "~1#1"}));
    let plan = plan.to_string();
    assert!(plan.contains("leaves its home station"), "{plan}");
    assert!(!plan.contains("other tree"), "{plan}");
}

/// P1.4 and P1.8: a Topic export holding a promoted Card cited by its handle keeps the relation kind
/// in the citing Card's summary and routes the Link to the Card's address while it shows the handle.
#[test]
fn handle_links_keep_kinds_and_route_to_addresses_in_a_topic_export() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Routes", "body": "t"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "One", "body": "one"}));
    mcp.ok("create_connection_tree", json!({"title": "Promote"}));
    mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "title": "原生", "body": "内容。"}));
    let plan = mcp.ok("promote_card", json!({"at": "~1#1", "to": "0/2"}));
    mcp.ok("promote_card", json!({"at": "~1#1", "to": "0/2", "confirm": plan["confirm"], "plan_hash": plan["plan_hash"], "expected_revision": plan["expected_revision"]}));
    mcp.ok("edit_card", json!({"at": "0/1", "body": "依据 [[~1#1]]：它成立。\n\n```chart\n<div>[[~1#1]]</div>\n```"}));
    let out_dir = scratch.archive_root.join("exported-topic");
    let out = scratch.cmd().args(["export", "0", "--out"]).arg(&out_dir).output().expect("export");
    assert!(out.status.success(), "{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let mut text = String::new();
    for entry in fs::read_dir(&out_dir).unwrap().filter_map(|e| e.ok()) {
        if entry.path().is_file() {
            text.push_str(&String::from_utf8_lossy(&fs::read(entry.path()).unwrap()));
        }
    }
    let at: String = text.find(r#""address":"0/1""#).map(|i| text[i..].chars().take(500).collect()).unwrap_or_default();
    assert!(text.contains(r#""kinds":{"0/2":"rests-on"}"#), "the handle Link keeps its relation kind: {at}");
    assert!(at.contains("#/card/0/2"), "the handle Link routes to the address: {at}");
    assert!(at.contains("~1#1") && !text.contains("#/card/~1"), "the citation shows the handle, no route spells it: {at}");
    // A chart's handle Link carries the address it opens (fourth review).
    assert!(text.contains(r#"\"home\":\"0/2\""#) || text.contains(r#""home":"0/2""#), "the chart link names its home");
}

/// P2.12 and P2.13: a Model that only imports another Model's proof part is no user of what that
/// part rests on; an authored "illustrates" Link and a local rests-on connection are two claims.
#[test]
fn imports_are_no_use_and_overlay_kinds_must_match() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex": "@book{Imp2020, title={Imports}}"}));
    mcp.ok("create_successor", json!({"at": "Imp2020", "title": "章", "body": "章。\n\n来源：[[Imp2020]] §1"}));
    let d = address(&mcp.ok("create_successor", json!({"at": "Imp2020/1", "kind": "side", "type": "model", "form": "definition", "level": "concept", "title": "定义 D", "body": "**定义**：\n- (C1) D。"})));
    let t = address(&mcp.ok("create_successor", json!({"at": "Imp2020/1", "kind": "side", "type": "model", "form": "theorem", "level": "formal", "title": "定理 T", "body": "**陈述**：\n- (H1) T。"})));
    let premise = address(&mcp.ok("create_successor", json!({"at": t, "kind": "side", "facet": "前提", "title": "前提：T", "body": "前提。"})));
    let step = address(&mcp.ok("create_successor", json!({"at": premise, "facet": "证明：T 的一步", "title": "证明：T 的一步", "body": format!("依赖 [[{premise}]]。依据 [[{d}]]：D 成立。")})));
    let u = address(&mcp.ok("create_successor", json!({"at": "Imp2020/1", "kind": "side", "type": "model", "form": "theorem", "level": "formal", "title": "定理 U", "body": format!("**陈述**：\n- (H1) U。\n\n**引入**\n- 单卡 [[{step}]]")})));
    let users = mcp.ok("get_dependencies", json!({"at": d, "scope": "unit"}));
    let names: Vec<&str> = users["dependents"].as_array().unwrap().iter().filter_map(|x| x["node"]["address"].as_str()).collect();
    assert!(names.contains(&t.as_str()), "{users}");
    assert!(!names.contains(&u.as_str()), "an import is no membership: {users}");

    // Overlay: A illustrates B (authored), A rests on B (local connection).
    let topic = address(&mcp.ok("create_topic", json!({"title": "Overlay", "body": "o"})));
    let b = address(&mcp.ok("create_successor", json!({"at": topic, "title": "乙", "body": "乙。"})));
    let a = address(&mcp.ok("create_successor", json!({"at": b, "title": "甲", "body": format!("例示 [[{b}]]：一个例子。")})));
    mcp.ok("create_connection_tree", json!({"title": "Kinds"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": b}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": a}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/2", "to_placement_id": "~1/1", "direction": "directed", "kind": "rests-on"}));
    let overlaid = mcp.ok("get_dependencies", json!({"at": a, "kinds": ["illustrates"], "overlay_tree": "~1"}));
    let classes: Vec<&str> = overlaid["overlay"]["connections"].as_array().unwrap().iter().filter_map(|c| c["class"].as_str()).collect();
    assert_eq!(classes, vec!["tree_local"], "{overlaid}");
    let authored = overlaid["overlay"]["authored_only"].as_array().unwrap();
    assert_eq!(authored.len(), 1, "the illustrates edge stays authored only: {overlaid}");
}

/// P2.14: a native Finding whose `来源` line names a general source is not checked against a book
/// another line names.
#[test]
fn the_source_line_decides_a_native_findings_book() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Src2020");
    mcp.ok("create_connection_tree", json!({"title": "Findings"}));
    let body = finding_body("**来源**：田野访谈 2019\nSource: [[Src2020]] 表 1");
    let card = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/0", "type": "finding", "title": "访谈中的产值", "body": body}));
    assert!(!card.to_string().contains("source_wrong_book"), "{card}");
    // A tab after the list mark is the same label for the gate and the Finding rules.
    let body = finding_body("-\t**来源**：田野访谈 2020\nSource: [[Src2020]] 表 2");
    let card = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "type": "finding", "title": "第二次访谈中的产值", "body": body}));
    assert!(!card.to_string().contains("source_wrong_book"), "{card}");
}

/// P2.10: a Topic shelf whose stored ordinals have a gap is a fault in `zt status`.
#[test]
fn status_reports_a_shelf_with_a_gap() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Gap2020");
    mcp.ok("create_topic", json!({"title": "Shelf", "body": "s"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "One", "body": "one"}));
    mcp.ok("place_card", json!({"anchor_at": "0/1", "edge": "attach", "target_at": "Gap2020"}));
    mcp.ok("place_card", json!({"anchor_at": "0/1", "edge": "attach", "target_at": "Gap2020/1"}));
    let healthy = scratch.cmd().arg("status").output().unwrap();
    assert!(String::from_utf8_lossy(&healthy.stdout).contains("coordinates: ok"), "{healthy:?}");
    let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    db.execute("UPDATE placements SET ordinal = 3 WHERE edge = 'attach' AND ordinal = 2", []).unwrap();
    drop(db);
    let out = scratch.cmd().arg("status").output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("coordinates: 1 fault(s)") && text.contains("side_mounts_not_dense"), "{text}");
}
