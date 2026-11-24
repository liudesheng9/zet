//! Dump and portable export of the coordinate storage (docs/multi-tree-coordinates-proposal.md §8.4,
//! Phase 1 exit gate "`zt dp` round-trip verified"): `zt dp` carries a versioned
//! `coordinates.json` with every identity, handle, tree, station, connection and retired station
//! from the same read; a scratch archive rebuilt from the dump resolves the old `place:<uuid>`
//! bookmark, the promoted `[[~1#1]]` handle and every `connection_id`. `zt export ~n` packages one
//! Connection tree in placement order with its connections.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;

struct Dump {
    mapping: BTreeMap<String, String>,
    manifest: Value,
    coordinates: Value,
    files: BTreeMap<String, String>,
}

fn dump(scratch: &ScratchBox) -> Dump {
    scratch.cmd().arg("dp").write_stdin("1\n").assert().success();
    let folder = scratch.archive_root.join("dump");
    let path = fs::read_dir(&folder)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.extension().is_some_and(|x| x == "zip"))
        .expect("a ZIP dump");
    let mut archive = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
    let mut out = Dump { mapping: BTreeMap::new(), manifest: Value::Null, coordinates: Value::Null, files: BTreeMap::new() };
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let Some((_, relative)) = name.split_once('/') else { continue };
        let relative = relative.to_string();
        let mut body = String::new();
        if entry.read_to_string(&mut body).is_err() {
            continue;
        }
        match relative.as_str() {
            "mapping.json" => out.mapping = serde_json::from_str(&body).unwrap(),
            "archive.json" => out.manifest = serde_json::from_str(&body).unwrap(),
            "coordinates.json" => out.coordinates = serde_json::from_str(&body).unwrap(),
            "locks.json" => {}
            _ => {
                out.files.insert(relative, body);
            }
        }
    }
    out
}

fn strip_front_matter(markdown: &str) -> &str {
    if let Some(rest) = markdown.strip_prefix("---\n")
        && let Some(end) = rest.find("\n---\n")
    {
        return &rest[end + 5..];
    }
    markdown
}

/// The test's loader (the product has no restore command): rebuild the Cards, then the coordinate
/// storage with its original identities, from the dump alone, and let `zt up` reinstall triggers.
fn install(target: &ScratchBox, source: &ScratchBox, dump: &Dump) {
    target.cmd().arg("down").assert().success();
    let conn = writer::open(target.archive_root.join("zt.sqlite3")).unwrap();
    writer::strip_coordinates(&conn);
    let tx = conn.unchecked_transaction().unwrap();
    let workspaces = dump.manifest["workspaces"].as_object().unwrap();
    let types = dump.manifest["types"].as_object().unwrap();
    let mut addresses: Vec<&String> = dump.mapping.keys().collect();
    addresses.reverse();
    for address in addresses {
        let markdown = &dump.files[&dump.mapping[address]];
        let workspace = workspaces[address].as_str().unwrap();
        let card_type = types.get(address).and_then(Value::as_str).unwrap_or("idea");
        let (location, citation_key, is_lit, bibtex, text) = match dump.files.get(&format!("{address}.bib")) {
            Some(bibtex) => (None, Some(address.as_str()), 1, Some(bibtex.as_str()), strip_front_matter(markdown)),
            None => (Some(address.as_str()), None, 0, None, markdown.as_str()),
        };
        let is_topic = is_lit == 0 && address.ends_with("/0");
        tx.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type],
        )
        .unwrap();
    }
    tx.execute("UPDATE metadata SET value = ?1 WHERE key = 'next_topic_id'", [dump.manifest["next_topic_id"].to_string()]).unwrap();
    // The coordinate tables, as the source archive defines them.
    let source_db = writer::open(source.archive_root.join("zt.sqlite3")).unwrap();
    for table in ["card_identity", "card_handles", "connection_trees", "placements", "placement_edges", "retired_placements"] {
        let sql: String = source_db.query_row("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |r| r.get(0)).unwrap();
        tx.execute_batch(&sql).unwrap();
    }
    let c = &dump.coordinates;
    assert_eq!(c["version"], 1);
    for i in c["identities"].as_array().unwrap() {
        let live = i["status"] == "live";
        let row: Option<i64> = if live {
            Some(tx.query_row("SELECT row_id FROM cards WHERE location = ?1 OR citation_key = ?1", [i["address"].as_str().unwrap()], |r| r.get(0)).unwrap())
        } else {
            None
        };
        tx.execute(
            "INSERT INTO card_identity(card_id, card_row, status, last_address, last_title, retired_at_rev) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![i["card_id"].as_str(), row, i["status"].as_str(), i["last_address"].as_str(), i["last_title"].as_str(), i["retired_at_rev"].as_i64()],
        )
        .unwrap();
    }
    let insert_all = |table: &str, rows: &Value| {
        for row in rows.as_array().unwrap() {
            let map = row.as_object().unwrap();
            let columns: Vec<&String> = map.keys().collect();
            let sql = format!(
                "INSERT INTO {table}({}) VALUES({})",
                columns.iter().map(|c| c.as_str()).collect::<Vec<_>>().join(", "),
                (1..=columns.len()).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ")
            );
            let values: Vec<rusqlite::types::Value> = columns
                .iter()
                .map(|c| match &map[*c] {
                    Value::Null => rusqlite::types::Value::Null,
                    Value::Number(n) => rusqlite::types::Value::Integer(n.as_i64().unwrap()),
                    Value::String(s) => rusqlite::types::Value::Text(s.clone()),
                    other => panic!("unexpected {other}"),
                })
                .collect();
            tx.execute(&sql, rusqlite::params_from_iter(values)).unwrap();
        }
    };
    insert_all("card_handles", &c["handles"]);
    insert_all("connection_trees", &c["connection_trees"]);
    insert_all("placements", &c["placements"]);
    insert_all("placement_edges", &c["placement_edges"]);
    insert_all("retired_placements", &c["retired_placements"]);
    tx.execute("INSERT INTO metadata(key, value) VALUES('coordinates_schema_version', '1')", []).unwrap();
    tx.execute("INSERT INTO metadata(key, value) VALUES('next_connection_id', ?1)", [c["next_connection_id"].as_str().unwrap()]).unwrap();
    tx.commit().unwrap();
    drop(conn);
    target.cmd().arg("up").assert().success();
    target.cmd().arg("reindex").assert().success();
}

/// §8.4 Dump: the round trip keeps every identity, handle, station, connection and bookmark.
#[test]
fn a_dump_rebuilds_an_archive_where_handles_bookmarks_and_connections_resolve() {
    let source = ScratchBox::up();
    let mut mcp = source.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex": "@book{Hart1995, title={Firms}}"}));
    mcp.ok("create_topic", json!({"title": "Home topic", "body": "topic"}));
    mcp.ok("create_successor", json!({"at": "0/0", "title": "First", "body": "first"}));
    mcp.ok("create_connection_tree", json!({"title": "Portable map", "purpose": "literature-group"}));
    let hart = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Hart1995", "gloss": "The book."}));
    let native = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/1", "title": "要提升的论断", "body": "论断。"}));
    let stay = mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/2", "title": "留在树里的卡", "body": format!("见 [[~1#1]] 与 [[Hart1995]]。")}));
    let edge = mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": native["placement_id"], "to_placement_id": hart["placement_id"], "direction": "directed", "relation_label": "extends"}));
    let plan = mcp.ok("promote_card", json!({"at": "~1#1", "to": "0/2"}));
    mcp.ok("promote_card", json!({"at": "~1#1", "to": "0/2", "confirm": plan["confirm"]}));
    let gui = Gui::connect(source.cmd());
    let bookmark = format!("place:{}", hart["placement_id"].as_str().unwrap());
    gui.post("/api/create", &json!({"kind": "topic", "workspace": "human", "text": format!("# Human\n\nCites [[~1#1]] and bookmarks [the book's station]({bookmark}).\n")}));
    let before = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    drop(mcp);

    let d = dump(&source);
    let c = &d.coordinates;
    assert_eq!(c["version"], 1, "{c}");
    assert_eq!(c["handles"].as_array().unwrap().len(), 3, "~1, ~1#1, ~1#2: {c}");
    assert_eq!(c["placement_edges"][0]["connection_id"], edge["connection_id"]);
    assert!(d.mapping.contains_key("0/2") && d.mapping.contains_key("~1#2") && d.mapping.contains_key("~1"), "{:?}", d.mapping.keys());

    let target = ScratchBox::up();
    install(&target, &source, &d);
    let mut mcp = target.mcp(&[]);
    assert_eq!(mcp.ok("get_card", json!({"at": "~1#1"}))["address"], "0/2", "the promoted handle resolves");
    assert_eq!(mcp.ok("get_card", json!({"at": &bookmark}))["address"], "Hart1995", "the bookmark resolves");
    let after = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    let ids = |v: &Value| -> Vec<(String, String)> {
        v["placements"].as_array().unwrap().iter().map(|p| (p["placement_id"].as_str().unwrap().to_string(), p["coord"].as_str().unwrap().to_string())).collect()
    };
    assert_eq!(ids(&after), ids(&before));
    assert_eq!(after["connections"][0]["connection_id"], edge["connection_id"]);
    assert_eq!(after["connections"][0]["relation_label"], "extends");
    assert_eq!(after["purpose"], "literature-group");
    assert_eq!(mcp.ok("get_card", json!({"at": stay["handle"]}))["title"], "留在树里的卡");
    let out = target.cmd().args(["status", "--check-links"]).output().unwrap();
    let report: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(report["ok"], true, "{report}");
    // The never-reused allocator travels too.
    assert_eq!(mcp.ok("create_connection_tree", json!({"title": "Next"}))["tree"], "~2");
}

/// §8.4 Portable export: `zt export ~1` packages the tree in placement order, one copy per included
/// Card, the tree connections by package-local endpoints, and distinguishes Missing mounts.
#[test]
fn a_connection_tree_exports_in_placement_order_with_its_connections() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({"bibtex": "@book{Gone2020, title={Gone}}"}));
    mcp.ok("create_successor", json!({"at": "Gone2020", "title": "将被删除", "body": "内容。\n\n来源：[[Gone2020]] §1"}));
    mcp.ok("create_literature", json!({"bibtex": "@book{Kept2020, title={Kept}}"}));
    mcp.ok("create_connection_tree", json!({"title": "Export me"}));
    let a = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Kept2020"}));
    let b = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Gone2020/1"}));
    mcp.ok("create_connection_card", json!({"tree": "~1", "parent_coord": "~1/2", "title": "本地卡", "body": "见 [[Kept2020]]。"}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": b["placement_id"], "to_placement_id": a["placement_id"], "direction": "directed", "relation_label": "uses"}));
    let plan = mcp.ok("delete_card", json!({"at": "Gone2020/1"}));
    mcp.ok("delete_card", json!({"at": "Gone2020/1", "confirm": plan["confirm"]}));
    let out_dir = scratch.archive_root.join("exported");
    let out = scratch.cmd().args(["export", "~1", "--out"]).arg(&out_dir).output().expect("export");
    assert!(out.status.success(), "{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let data_file = fs::read_dir(&out_dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "json") || p.to_string_lossy().ends_with(".data.js")).expect("export data");
    let text = fs::read_to_string(&data_file).unwrap();
    let json_start = text.find('{').unwrap();
    let json_end = text.rfind('}').unwrap();
    let data: Value = serde_json::from_str(&text[json_start..=json_end]).expect("export JSON");
    let tree = &data["connection_tree"];
    assert_eq!(tree["handle"], "~1", "{data}");
    let stations = tree["stations"].as_array().unwrap();
    let coords: Vec<&str> = stations.iter().map(|s| s["coord"].as_str().unwrap()).collect();
    assert_eq!(coords, vec!["~1/0", "~1/1", "~1/2", "~1/3"]);
    assert_eq!(stations[2]["state"], "missing", "{data}");
    assert_eq!(stations[1]["state"], "in-package");
    let conn = &tree["connections"][0];
    assert_eq!(conn["from"], stations[2]["station"]);
    assert_eq!(conn["state"], "in-package local assertion");
    assert!(!text.contains(scratch.archive_root.to_string_lossy().as_ref()), "no local path leaks");
    assert!(!text.contains("row_id"), "no row identity leaks");
}
