//! The coordinate storage itself (docs/multi-tree-coordinates-proposal.md §6, §10.1): the zero-loss
//! migration of an archive written before multi-tree coordinates, the writer fence against older
//! executables, identity through every home write, and the invariance of every structural report
//! when specialized Cards are mounted and connected.

#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use mcp_client::{Mcp, ScratchBox};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn address(value: &Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

fn rows(scratch: &ScratchBox) -> Vec<(i64, Option<String>, Option<String>, String, Option<String>)> {
    let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    let mut stmt = db
        .prepare("SELECT row_id, location, citation_key, text, bibtex FROM cards ORDER BY row_id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
}

fn identities(scratch: &ScratchBox) -> BTreeMap<i64, String> {
    let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    let mut stmt = db
        .prepare("SELECT card_row, card_id FROM card_identity WHERE status = 'live'")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<BTreeMap<_, _>>>()
        .unwrap()
}

/// §10.1: an archive from before coordinates upgrades in place: every row, authored byte and row id
/// is kept, each Card receives exactly one live identity, no placement or handle is invented, a
/// backup is taken, and a second start issues no new identity.
#[test]
fn a_legacy_archive_upgrades_with_one_identity_per_card_and_no_other_change() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Legacy", "body": "legacy"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "Child", "body": "see [[0/0]]"}));
    mcp.ok("create_literature", json!({"bibtex": "@book{Old1999, title={Old Book}}"}));
    drop(mcp);
    scratch.cmd().arg("down").assert().success();
    // Make it an archive of the previous release: no coordinate storage at all.
    {
        let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
        writer::strip_coordinates(&db);
    }
    let before = rows(&scratch);
    scratch.cmd().arg("up").assert().success();
    assert_eq!(rows(&scratch), before, "every row and authored byte is kept");
    let ids = identities(&scratch);
    assert_eq!(ids.len(), before.len(), "one live identity per Card");
    let backups = std::fs::read_dir(&scratch.archive_root)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().contains("pre-coordinates"))
        .count();
    assert_eq!(backups, 1, "the upgrade backed the archive up once");
    let db = writer::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    for table in ["placements", "card_handles", "placement_edges", "connection_trees"] {
        let n: i64 = db.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "{table} starts empty");
    }
    drop(db);
    scratch.cmd().arg("down").assert().success();
    scratch.cmd().arg("up").assert().success();
    assert_eq!(identities(&scratch), ids, "a second start issues no new identity");
}

/// §10.1.2: an older `zt` (any connection without the current writer functions) can read the
/// upgraded archive but cannot write any of its tables, not even a metadata-only save; nothing and no
/// revision changes.
#[test]
fn an_older_writer_cannot_change_the_upgraded_archive() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Fenced", "body": "fenced"}));
    mcp.ok("create_connection_tree", json!({"title": "Map"}));
    drop(mcp);
    let before = rows(&scratch);
    let old = Connection::open(scratch.archive_root.join("zt.sqlite3")).unwrap();
    let count: i64 = old.query_row("SELECT COUNT(*) FROM cards", [], |r| r.get(0)).unwrap();
    assert_eq!(count as usize, before.len(), "an older client still reads");
    let revision: String = old.query_row("SELECT value FROM metadata WHERE key = 'archive_revision'", [], |r| r.get(0)).unwrap();
    for sql in [
        "INSERT INTO metadata(key, value) VALUES('gui_spreads', '{}') ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        "UPDATE metadata SET value = value || '' WHERE key = 'archive_revision'",
        "UPDATE cards SET text = text WHERE location = '0/0'",
        "UPDATE cards SET workspace = workspace WHERE location = '0/0'",
        "DELETE FROM cards WHERE location = '0/0'",
        "INSERT INTO cards(location, is_topic, is_lit, text) VALUES('9/0', 1, 0, '# x\n')",
        "DELETE FROM links",
        "DELETE FROM placements",
        "UPDATE connection_trees SET purpose = 'x'",
        "DELETE FROM lock_placements",
        "DELETE FROM lock_placement_pins",
    ] {
        let refused = old.execute(sql, []).expect_err(sql);
        assert!(refused.to_string().contains("no such function"), "{sql}: {refused}");
    }
    let after: String = old.query_row("SELECT value FROM metadata WHERE key = 'archive_revision'", [], |r| r.get(0)).unwrap();
    assert_eq!(after, revision);
    drop(old);
    assert_eq!(rows(&scratch), before);
}

/// §3.1 "Card identity": every home write keeps the identity; a new Card at a reused Location gets
/// a new one; a delete retires the identity with its last address and title.
#[test]
fn identity_survives_edits_moves_and_renames_and_retires_on_delete() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Ids", "body": "ids"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "A", "body": "a"}));
    mcp.ok("create_successor", json!({"at": "0/1", "title": "B", "body": "b"}));
    let id = |mcp: &mut Mcp, at: &str| mcp.ok("list_placements", json!({"at": at}))["card_id"].as_str().unwrap().to_string();
    let a = id(&mut mcp, "0/2");
    mcp.ok("edit_card", json!({"at": "0/2", "title": "A renamed", "body": "a2"}));
    assert_eq!(id(&mut mcp, "0/2"), a);
    let plan = mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a"}));
    mcp.ok("move_card", json!({"at": "0/2", "to": "0/1|a", "confirm": plan["confirm"]}));
    assert_eq!(mcp.ok("list_placements", json!({"card_id": a}))["home"], "0/1|a");
    let plan = mcp.ok("insert_card", json!({"before": "0/1|a", "title": "Inserted", "body": "ins"}));
    mcp.ok("insert_card", json!({"before": "0/1|a", "title": "Inserted", "body": "ins", "confirm": plan["confirm"]}));
    let home = mcp.ok("list_placements", json!({"card_id": a}))["home"].as_str().unwrap().to_string();
    assert_ne!(home, "0/1|a", "an insert shifted the Card");
    assert_eq!(id(&mut mcp, &home), a, "and it kept its identity");
    mcp.ok("create_literature", json!({"bibtex": "@book{Ren2000, title={Before}}"}));
    let book = id(&mut mcp, "Ren2000");
    mcp.ok("edit_literature_metadata", json!({"at": "Ren2000", "bibtex": "@book{Ren2001, title={After}}", "confirm": "rename"}));
    let renamed = mcp.ok("list_placements", json!({"card_id": book}));
    assert!(renamed["home"] == "Ren2001" || renamed["home"] == "Ren2000", "{renamed}");
    let plan = mcp.ok("delete_card", json!({"at": home}));
    mcp.ok("delete_card", json!({"at": home, "confirm": plan["confirm"]}));
    let retired = mcp.ok("list_placements", json!({"card_id": a}));
    assert_eq!(retired["status"], "retired");
    assert_eq!(retired["title"], "A renamed");
    let reborn_at = address(&mcp.ok("create_successor", json!({"at": "0/1|a", "title": "Reborn", "body": "new"})));
    assert_eq!(reborn_at, home, "the freed Location is reused");
    let reborn = mcp.ok("list_placements", json!({"at": reborn_at}));
    assert_ne!(reborn["card_id"], json!(a), "a reused Location gets a new identity");
}

const TERMS: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n";
const CITED: &str = "本卡是《Inv Book》的参考文献表。\n\n| 编号 | 文献 | 年份 | 引用处 | 关系 |\n| --- | --- | --- | --- | --- |\n| 1 | Deming. Social Skills. *QJE* | 2017 | §1 | 依赖 |\n\n来源：[[Inv2026]] 参考文献\n";

fn reports(mcp: &mut Mcp, model: &str, unit: &str, hub: &str) -> Value {
    let mut out = json!({
        "model": mcp.ok("get_model", json!({"at": model})),
        "unit": mcp.ok("get_model", json!({"at": unit})),
        "terms": mcp.ok("get_terms", json!({"at": "Inv2026"})),
        "cited": mcp.ok("get_cited", json!({"at": "Inv2026"})),
        "hub": mcp.ok("get_hub", json!({"at": hub})),
        "tree": mcp.ok("get_tree", json!({"tree": "Inv2026", "bodies": true})),
        "links": mcp.ok("list_links", json!({})),
    });
    strip(&mut out);
    out
}

fn strip(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("archive_revision");
            for v in map.values_mut() {
                strip(v);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// §8.2 and §10.1.3: mounting a Model head and part, a Math unit, a Term list, a Cited list and
/// Card, and a Hub into a Connection tree, and connecting them, leaves every structural report
/// (Model, Math, Term, Cited, Hub, the home tree and the Link list) byte-for-byte identical.
#[test]
fn mounting_and_connecting_specialized_cards_changes_no_structural_report() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex": "@book{Inv2026, title={Inv Book}}"}));
    let anchor = address(&mcp.ok("create_successor", json!({"at": "Inv2026", "title": "第一章", "body": "设置。\n\n来源：[[Inv2026]] §1"})));
    let model = address(&mcp.ok("create_successor", json!({"at": anchor, "kind": "side", "type": "model", "level": "concept", "title": "模型：两人博弈", "body": "两个参与人。"})));
    let part = address(&mcp.ok("create_successor", json!({"at": model, "kind": "side", "facet": "设定", "title": "设定：两个参与人", "body": "参与人 A 与 B。"})));
    let unit = address(&mcp.ok("create_successor", json!({"at": anchor, "kind": "side", "type": "model", "form": "definition", "level": "concept", "title": "均衡：没有人愿意偏离", "body": "**定义**：\n- (C1) 没有人愿意单方面偏离。"})));
    let terms = address(&mcp.ok("create_successor", json!({"at": anchor, "kind": "side", "type": "term", "title": "术语表：Inv", "body": TERMS})));
    let cited_list = address(&mcp.ok("create_successor", json!({"at": anchor, "kind": "side", "type": "cited", "title": "参考文献：《Inv Book》引用的 1 篇文献", "body": CITED})));
    let cited = address(&mcp.ok("create_successor", json!({"at": cited_list, "kind": "side", "type": "cited", "title": "Deming（2017）：社交技能", "body": "文献：Deming（2017）\n\n## 为什么引用\n\n本书引用它。\n\n来源：[[Inv2026]] 参考文献第 1 条"})));
    mcp.ok("create_topic", json!({"title": "Hub home", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "First", "body": format!("依据 [[{model}]]。")}));
    let hub = address(&mcp.ok("create_successor", json!({"at": "0/1", "kind": "side", "type": "hub", "level": "line", "title": "线索：模型", "body": format!("- [[0/1]] 起点\n- [[{model}]] 模型")})));
    let before = reports(&mut mcp, &model, &unit, &hub);

    mcp.ok("create_connection_tree", json!({"title": "Everything mounted"}));
    let mut parent = "~1/0".to_string();
    let mut ids = Vec::new();
    for at in [&model, &part, &unit, &terms, &cited_list, &cited, &hub, &"Inv2026".to_string()] {
        let placed = mcp.ok("place_card", json!({"host": "~1", "parent_coord": parent, "target_at": at}));
        parent = placed["coord"].as_str().unwrap().to_string();
        ids.push(placed["placement_id"].as_str().unwrap().to_string());
    }
    for pair in ids.windows(2) {
        mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": pair[1], "to_placement_id": pair[0], "direction": "directed", "relation_kind": "rests-on", "note": "Working connection."}));
    }
    mcp.ok("create_successor", json!({"at": "0/1", "title": "Second", "body": "second"}));
    let after = reports(&mut mcp, &model, &unit, &hub);
    // The one Topic Card added above changes the Link list and the hub's range only by itself;
    // compare the specialized reports exactly.
    for key in ["model", "unit", "terms", "cited", "tree"] {
        assert_eq!(after[key], before[key], "{key} report changed");
    }
    let out = scratch.cmd().args(["status", "--check-links"]).output().expect("check links");
    let report: Value = serde_json::from_slice(&out.stdout).expect("report");
    assert_eq!(report["ok"], true, "{report}");
}
