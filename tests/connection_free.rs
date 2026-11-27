//! Free Connection trees (docs/connection-trees-free-proposal.md, zt 0.70.0): a root without
//! meaning (an optional title, read as untitled when it is its own handle), any set of Cards mounted
//! at once in one block, many trees listed by number or newest first and filtered by handle, the
//! projection of every Connection tree that crosses a Topic or Literature tree (stations, structure
//! lines to the nearest in-plane ancestor with hops, connection lines, exits; the root never counts),
//! and the review fixes of Astra round 1: a delete confirmation bound to the plan it showed, a
//! keyed all-skipped batch that keeps its receipt, the overview's one-pass tree counts, and the
//! Link label of an untitled root.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;
#[path = "support/writer.rs"]
mod writer;

use gui_client::Gui;
use mcp_client::{Mcp, ScratchBox};
use serde_json::json;

/// One book `key` with a root and `n` notes in one Direct chain: `key/1` … `key/n`.
fn book(mcp: &mut Mcp, key: &str, n: usize) {
    mcp.ok(
        "create_literature",
        json!({"bibtex": format!("@book{{{key}, title={{Book {key}}}}}")}),
    );
    let mut at = key.to_string();
    for i in 1..=n {
        mcp.ok("create_successor", json!({"at": at, "title": format!("{key} 第{i}节"), "body": format!("第{i}节的内容。\n\n来源：[[{key}]] §{i}")}));
        at = format!("{key}/{i}");
    }
}

fn stations(mcp: &mut Mcp, tree: &str) -> Vec<(String, String)> {
    let view = mcp.ok("get_connection_tree", json!({"tree": tree}));
    view["placements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            (
                p["coord"].as_str().unwrap().to_string(),
                p["home_address"].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn coord_of(mcp: &mut Mcp, tree: &str, home: &str) -> String {
    stations(mcp, tree)
        .into_iter()
        .find(|(_, h)| h == home)
        .map(|(c, _)| c)
        .unwrap_or_else(|| panic!("{home} is not in {tree}"))
}

/// D2: a tree starts with nothing written; its root is an ordinary Card titled with its own
/// handle, listed as untitled until someone writes a title, and a body alone is acceptable too.
#[test]
fn a_root_has_no_meaning_and_may_stay_empty() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let made = mcp.ok("create_connection_tree", json!({}));
    assert_eq!(made["tree"], "~1");
    let root = mcp.ok("get_card", json!({"at": "~1"}));
    assert_eq!(
        root["title"], "~1",
        "an untitled root is stored with its handle as title: {root}"
    );
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(view["title"], "", "{view}");
    assert_eq!(view["untitled"], true, "{view}");

    // A body alone, no title.
    mcp.ok(
        "create_connection_tree",
        json!({"body": "只写正文，不写标题。"}),
    );
    let second = mcp.ok("get_connection_tree", json!({"tree": "~2"}));
    assert_eq!(second["untitled"], true, "{second}");
    assert!(
        mcp.ok("get_card", json!({"at": "~2"}))["body"]
            .as_str()
            .unwrap()
            .contains("只写正文")
    );

    // Writing a title later is an ordinary edit; the tree is then named by it.
    mcp.ok("edit_card", json!({"at": "~1", "title": "企业边界"}));
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert_eq!(view["title"], "企业边界");
    assert_eq!(view["untitled"], false);

    // A seeded tree needs no title either.
    book(&mut mcp, "Sd2020", 2);
    let args = json!({"from": "search", "source_params": {"cards": ["Sd2020/1", "Sd2020/2"]}});
    let plan = mcp.ok("seed_connection_tree", args.clone());
    assert!(
        plan.to_string().contains("untitled Connection tree"),
        "{plan}"
    );
    let mut confirmed = args;
    confirmed["confirm"] = plan["confirm"].clone();
    confirmed["plan_hash"] = plan["plan_hash"].clone();
    confirmed["expected_revision"] = plan["expected_revision"].clone();
    let seeded = mcp.ok("seed_connection_tree", confirmed);
    assert_eq!(seeded["tree"], "~3");
    assert_eq!(
        mcp.ok("get_connection_tree", json!({"tree": "~3"}))["untitled"],
        true
    );

    // D3: newest first on request, numbers by default; a filter reads the handle too.
    let listed = mcp.ok("list_connection_trees", json!({"sort": "recent"}));
    let order: Vec<&str> = listed["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tree"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["~3", "~2", "~1"], "{listed}");
    let listed = mcp.ok("list_connection_trees", json!({}));
    let order: Vec<&str> = listed["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["tree"].as_str().unwrap())
        .collect();
    assert_eq!(order, ["~1", "~2", "~3"], "{listed}");
    let found = mcp.ok("list_connection_trees", json!({"query": "~2"}));
    assert_eq!(found["trees"].as_array().unwrap().len(), 1, "{found}");
    assert_eq!(found["trees"][0]["tree"], "~2");

    // A Link to an untitled root shows its address alone, never "~2 ~2".
    mcp.ok("create_topic", json!({"title": "Topic", "body": "topic"}));
    mcp.ok(
        "create_successor",
        json!({"at": "0/0", "title": "Cites the tree", "body": "See [[~2]]."}),
    );
    let gui = Gui::connect(scratch.cmd());
    let page = gui.get("/api/card?at=0%2F1");
    let html = page["html"].as_str().unwrap_or_default();
    assert!(
        html.contains("<span class=\"zt-link-addr\">~2</span></a>"),
        "{html}"
    );
}

/// D1: a set of Cards mounts in one block: a Direct chain after the parent (the old successor
/// continues after the last, its gloss reviewed once), or consecutive Side branches before a
/// sibling. Duplicates and Cards already in the tree are placed once and reported; a batch that
/// places nothing is a successful no-op that keeps its receipt; a bad target writes nothing.
#[test]
fn a_set_of_cards_mounts_in_one_block() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Bk2020", 6);
    mcp.ok("create_connection_tree", json!({"title": "块"}));
    mcp.ok(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Bk2020/1"}),
    );
    mcp.ok(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Bk2020/6"}),
    );
    mcp.ok(
        "edit_placement",
        json!({"placement_id": "~1/2", "gloss": "最后一节。"}),
    );

    // A Direct block between ~1/1 and ~1/2: the displaced station's gloss is reviewed once.
    let args = json!({"host": "~1", "parent_coord": "~1/1", "edge": "direct", "targets": ["Bk2020/2", "Bk2020/3", "Bk2020/2", "Bk2020/1"]});
    let plan = mcp.ok("place_card", args.clone());
    assert_eq!(plan["applied"], false, "{plan}");
    assert!(plan.to_string().contains("review gloss of ~1/2"), "{plan}");
    let station_id = mcp.ok("list_placements", json!({"at": "~1/2"}))["placements"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["coord"] == "~1/2")
        .unwrap()["placement_id"]
        .clone();
    // The decision is part of the request: plan again with it, then confirm that plan.
    let mut reviewed = args.clone();
    reviewed["gloss_updates"] = json!({station_id.as_str().unwrap(): "最后一节。"});
    let plan = mcp.ok("place_card", reviewed.clone());
    assert_eq!(plan["applied"], false, "{plan}");
    let mut confirmed = reviewed;
    confirmed["confirm"] = plan["confirm"].clone();
    confirmed["plan_hash"] = plan["plan_hash"].clone();
    confirmed["expected_revision"] = plan["expected_revision"].clone();
    let done = mcp.ok("place_card", confirmed);
    assert_eq!(done["placed"].as_array().unwrap().len(), 2, "{done}");
    assert_eq!(
        done["already_placed"].as_array().unwrap().len(),
        1,
        "Bk2020/1 stands already: {done}"
    );
    let order: Vec<String> = stations(&mut mcp, "~1")
        .into_iter()
        .map(|(_, h)| h)
        .collect();
    assert_eq!(
        order,
        ["~1", "Bk2020/1", "Bk2020/2", "Bk2020/3", "Bk2020/6"],
        "a Direct block in order"
    );

    // A Side block before an existing Side sibling.
    mcp.ok(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/1", "edge": "side", "target_at": "Bk2020/5"}),
    );
    let side = mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "edge": "side", "before": "~1/1|a", "targets": ["Bk2020", "Bk2020/4"]}));
    assert_eq!(side["applied"], true, "{side}");
    assert_eq!(coord_of(&mut mcp, "~1", "Bk2020"), "~1/1|a");
    assert_eq!(coord_of(&mut mcp, "~1", "Bk2020/4"), "~1/1|b");
    assert_eq!(coord_of(&mut mcp, "~1", "Bk2020/5"), "~1/1|c");

    // All already placed: a successful no-op whose receipt replays after an unplace.
    let again =
        json!({"host": "~1", "parent_coord": "~1/1", "targets": ["Bk2020/5"], "op_key": "k-batch"});
    let noop = mcp.ok("place_card", again.clone());
    assert_eq!(noop["applied"], true, "{noop}");
    assert_eq!(noop["changed"], false, "{noop}");
    let c = coord_of(&mut mcp, "~1", "Bk2020/5");
    let plan = mcp.ok("unplace", json!({"placement_id": c}));
    mcp.ok("unplace", json!({"placement_id": c, "confirm": plan["confirm"], "plan_hash": plan["plan_hash"], "expected_revision": plan["expected_revision"]}));
    let replay = mcp.ok("place_card", again);
    assert_eq!(replay["replayed"], true, "{replay}");
    assert!(
        !stations(&mut mcp, "~1")
            .iter()
            .any(|(_, h)| h == "Bk2020/5"),
        "the replay recreated no station"
    );

    // A bad last target refuses the whole batch; nothing is placed.
    let before = stations(&mut mcp, "~1").len();
    mcp.refused(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/1", "targets": ["Bk2020/5", "Nowhere/9"]}),
    );
    assert_eq!(
        stations(&mut mcp, "~1").len(),
        before,
        "nothing of a refused batch stays"
    );
    // A gloss belongs to one station; a Topic-side mount takes one Card.
    let refused = mcp.refused(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/1", "targets": ["Bk2020/5"], "gloss": "x"}),
    );
    assert!(
        refused
            .to_string()
            .contains("a gloss belongs to one station"),
        "{refused}"
    );
    mcp.ok("create_topic", json!({"title": "主题", "body": "主题。"}));
    let refused = mcp.refused(
        "place_card",
        json!({"edge": "attach", "anchor_at": "0/0", "targets": ["Bk2020/5"]}),
    );
    assert!(
        refused
            .to_string()
            .contains("Topic-side mount takes one Card"),
        "{refused}"
    );
}

/// D4: the projection of a Connection tree onto the Literature tree it crosses: each in-plane
/// station joins its nearest in-plane ancestor (hops count the stations outside), tree connections
/// between in-plane stations are their own lines, exits count the neighbours outside, and the root
/// never counts. A Missing mount is outside the plane but still a hop.
#[test]
fn a_literature_tree_shows_the_part_of_each_connection_tree_that_crosses_it() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Pl2020", 3);
    book(&mut mcp, "Ot2020", 2);
    mcp.ok("create_connection_tree", json!({}));
    // ~1/1 Pl2020/1 → ~1/2 Ot2020/1 (outside) → ~1/3 Pl2020/3; ~1/1|a Pl2020/2 (Side, in plane).
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "targets": ["Pl2020/1", "Ot2020/1", "Pl2020/3"]}));
    mcp.ok(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/1", "edge": "side", "target_at": "Pl2020/2"}),
    );
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/3", "to_placement_id": "~1/1|a", "direction": "directed", "relation_label": "依赖"}));
    mcp.ok(
        "connect_placements",
        json!({"tree": "~1", "from_placement_id": "~1/1|a", "to_placement_id": "~1/2"}),
    );
    mcp.ok(
        "connect_placements",
        json!({"tree": "~1", "from_placement_id": "~1/1", "to_placement_id": "~1/0"}),
    );
    let gui = Gui::connect(scratch.cmd());
    let answer = gui.get("/api/tree?id=Pl2020");
    let cross = &answer["crossings"]["~1"];
    assert_eq!(cross["untitled"], true, "{answer}");
    assert_eq!(cross["stations"].as_array().unwrap().len(), 3, "{cross}");
    let lines = cross["lines"].as_array().unwrap();
    let find = |kind: &str, from: &str, to: &str| {
        lines
            .iter()
            .find(|l| l["kind"] == kind && l["from"] == from && l["to"] == to)
            .cloned()
    };
    let through = find("structure", "Pl2020/1", "Pl2020/3")
        .unwrap_or_else(|| panic!("no line through the outside station: {cross}"));
    assert_eq!(through["hops"], 1, "{through}");
    assert_eq!(through["edge"], "direct");
    let side = find("structure", "Pl2020/1", "Pl2020/2")
        .unwrap_or_else(|| panic!("no Side line: {cross}"));
    assert_eq!(side["hops"], 0);
    assert_eq!(side["edge"], "side");
    let said = find("connection", "Pl2020/3", "Pl2020/2")
        .unwrap_or_else(|| panic!("no connection line: {cross}"));
    assert_eq!(said["relation_label"], "依赖");
    assert!(
        said["from_placement_id"].is_string() && said["to_placement_id"].is_string(),
        "{said}"
    );
    assert_eq!(
        lines.len(),
        3,
        "the root is in no plane and draws no line: {cross}"
    );
    // Exits: Pl2020/1 → Ot2020/1 (its Direct successor, outside); Pl2020/3 ← Ot2020/1 (its parent);
    // Pl2020/2 — Ot2020/1 (a connection). The connection to the root counts nowhere.
    assert_eq!(cross["exits"]["Pl2020/1"], 1, "{cross}");
    assert_eq!(cross["exits"]["Pl2020/3"], 1, "{cross}");
    assert_eq!(cross["exits"]["Pl2020/2"], 1, "{cross}");
    // The other book sees its own part: one station, no line, its exits.
    let other = gui.get("/api/tree?id=Ot2020");
    let theirs = &other["crossings"]["~1"];
    assert_eq!(theirs["stations"].as_array().unwrap().len(), 1, "{other}");
    assert!(theirs["lines"].as_array().unwrap().is_empty(), "{other}");
    assert_eq!(
        theirs["exits"]["Ot2020/1"], 3,
        "parent, Direct successor and one connection: {other}"
    );
    // A tree no Connection tree crosses answers an empty projection.
    mcp.ok(
        "create_literature",
        json!({"bibtex": "@book{Lone2020, title={Lone}}"}),
    );
    assert_eq!(gui.get("/api/tree?id=Lone2020")["crossings"], json!({}));

    // A deleted home Card leaves a Missing mount: outside the plane, still a hop.
    let plan = mcp.ok("delete_card", json!({"at": "Ot2020"}));
    mcp.ok(
        "delete_card",
        json!({"at": "Ot2020", "confirm": plan["confirm"], "plan_hash": plan["plan_hash"]}),
    );
    let answer = gui.get("/api/tree?id=Pl2020");
    let cross = &answer["crossings"]["~1"];
    let lines = cross["lines"].as_array().unwrap();
    let through = lines
        .iter()
        .find(|l| l["from"] == "Pl2020/1" && l["to"] == "Pl2020/3")
        .expect("line through the Missing mount");
    assert_eq!(through["hops"], 1, "{cross}");
    assert_eq!(cross["exits"]["Pl2020/1"], 1, "{cross}");
}

/// D5 blocker (Astra round 1): a delete confirmation is bound to the plan it showed. A Card added to
/// the tree between the preview and the confirmation makes the old plan stale; the new plan applies.
/// The tree's mounted Cards stay at home.
#[test]
fn a_tree_delete_applies_only_the_plan_it_showed() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Dl2020", 1);
    mcp.ok("create_connection_tree", json!({"title": "要删的树"}));
    mcp.ok(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Dl2020/1"}),
    );
    let gui = Gui::connect(scratch.cmd());
    let plan = gui.post("/api/delete", &json!({"at": "~1"}));
    assert!(plan["plan_hash"].is_string(), "{plan}");
    // Another client writes a native Card into the tree meanwhile.
    mcp.ok(
        "create_connection_card",
        json!({"tree": "~1", "parent_coord": "~1/1", "title": "后来的一张", "body": "后来写的。"}),
    );
    let (status, refused) = gui.post_with(
        "/api/delete",
        &json!({"at": "~1", "confirm": plan["confirm"], "plan_hash": plan["plan_hash"]}),
    );
    assert_ne!(status, 200, "{refused}");
    assert!(refused.to_string().contains("stale plan"), "{refused}");
    assert!(
        mcp.ok("list_connection_trees", json!({}))["trees"]
            .as_array()
            .unwrap()
            .len()
            == 1,
        "nothing was deleted"
    );
    let plan = gui.post("/api/delete", &json!({"at": "~1"}));
    let done = gui.post(
        "/api/delete",
        &json!({"at": "~1", "confirm": plan["confirm"], "plan_hash": plan["plan_hash"]}),
    );
    assert_eq!(done["deleted"], 2, "the root and its native Card: {done}");
    assert!(
        mcp.ok("list_connection_trees", json!({}))["trees"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        mcp.ok("get_card", json!({"at": "Dl2020/1"}))["address"],
        "Dl2020/1",
        "a mounted Card stays at home"
    );
    // The MCP plan names its hash too.
    let plan = mcp.ok("delete_card", json!({"at": "Dl2020/1"}));
    assert!(plan["plan_hash"].is_string(), "{plan}");
}

/// D3 (Astra round 1): the overview counts each tree's home Cards in one pass over the Cards; a
/// Connection tree's count is its root and its native Cards, never its mounts.
#[test]
fn the_overview_counts_each_tree_in_one_pass() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Ov2020", 3);
    mcp.ok("create_topic", json!({"title": "主题", "body": "主题。"}));
    mcp.ok(
        "create_successor",
        json!({"at": "0/0", "title": "一", "body": "一。"}),
    );
    mcp.ok("create_connection_tree", json!({}));
    mcp.ok(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/0", "targets": ["Ov2020/1", "Ov2020/2"]}),
    );
    mcp.ok(
        "create_connection_card",
        json!({"tree": "~1", "parent_coord": "~1/2", "title": "原生", "body": "原生的一张。"}),
    );
    let gui = Gui::connect(scratch.cmd());
    let overview = gui.get("/api/overview");
    let count = |tree: &str| {
        overview["trees"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["tree"] == tree)
            .unwrap_or_else(|| panic!("{tree} missing: {overview}"))["count"]
            .clone()
    };
    assert_eq!(count("Ov2020"), 4);
    assert_eq!(count("0"), 2);
    assert_eq!(count("~1"), 2, "the root and one native Card");
}

/// Astra round 1: a portable export of a Connection tree keeps each mounted Card's own Workspace.
/// An agent book mounted in the owner's tree reads offline as an agent Card.
#[test]
fn an_exported_connection_tree_keeps_each_mounted_cards_workspace() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Ws2020", 1);
    let gui = Gui::connect(scratch.cmd());
    let made = gui.conn("create_connection_tree", json!({}));
    assert_eq!(made["workspace"], "human", "{made}");
    gui.conn(
        "place_card",
        json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Ws2020/1"}),
    );
    let out_dir = scratch.archive_root.join("exported");
    let out = scratch
        .cmd()
        .args(["export", "~1", "--out"])
        .arg(&out_dir)
        .output()
        .expect("export");
    assert!(
        out.status.success(),
        "{} {}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let data_file = std::fs::read_dir(&out_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.extension().is_some_and(|x| x == "json") || p.to_string_lossy().ends_with(".data.js")
        })
        .expect("export data");
    let text = std::fs::read_to_string(&data_file).unwrap();
    let data: serde_json::Value =
        serde_json::from_str(&text[text.find('{').unwrap()..=text.rfind('}').unwrap()])
            .expect("export JSON");
    let cards = data["cards"].as_array().expect("cards");
    let mounted = cards
        .iter()
        .find(|c| c["address"] == "Ws2020/1")
        .unwrap_or_else(|| panic!("{data}"));
    assert_eq!(mounted["workspace"], "agent", "{mounted}");
    let root = cards
        .iter()
        .find(|c| c["address"] == "~1")
        .unwrap_or_else(|| panic!("{data}"));
    assert!(
        root.get("workspace").is_none(),
        "the tree's own Cards share its Workspace: {root}"
    );
}

/// Astra round 2: a delete plan is bound to the state behind its lines too. A tree connection
/// written between the preview and the confirmation changes no address and no line, and still
/// makes the old confirmation stale. An all-skipped batch confirmed on an old plan is stale, never
/// a silent success; `extract_branch` needs no title.
#[test]
fn old_confirmations_never_apply_to_a_changed_tree() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    book(&mut mcp, "Cf2020", 3);
    mcp.ok("create_connection_tree", json!({}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "targets": ["Cf2020/1", "Cf2020/2"]}));
    let plan = mcp.ok("delete_card", json!({"at": "~1"}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/1", "to_placement_id": "~1/2", "relation_label": "新写的"}));
    let refused = mcp.refused("delete_card", json!({"at": "~1", "confirm": plan["confirm"], "plan_hash": plan["plan_hash"]}));
    assert!(refused.to_string().contains("stale plan"), "{refused}");
    assert_eq!(mcp.ok("list_connection_trees", json!({}))["trees"].as_array().unwrap().len(), 1);

    // A plan to mount Cf2020/3 before a glossed station; another client mounts it meanwhile.
    mcp.ok("edit_placement", json!({"placement_id": "~1/2", "gloss": "第二节。"}));
    let args = json!({"host": "~1", "parent_coord": "~1/1", "targets": ["Cf2020/3"]});
    let plan = mcp.ok("place_card", args.clone());
    assert_eq!(plan["applied"], false, "{plan}");
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/2", "target_at": "Cf2020/3"}));
    let mut confirmed = args;
    confirmed["confirm"] = plan["confirm"].clone();
    confirmed["plan_hash"] = plan["plan_hash"].clone();
    confirmed["expected_revision"] = plan["expected_revision"].clone();
    let refused = mcp.refused("place_card", confirmed);
    assert!(refused.to_string().contains("stale"), "{refused}");

    // extract_branch without a title: an untitled copy.
    let args = json!({"source_tree": "~1", "root_placement_id": "~1/2"});
    let plan = mcp.ok("extract_branch", args.clone());
    assert!(plan.to_string().contains("untitled Connection tree"), "{plan}");
    let mut confirmed = args;
    confirmed["confirm"] = plan["confirm"].clone();
    confirmed["plan_hash"] = plan["plan_hash"].clone();
    confirmed["expected_revision"] = plan["expected_revision"].clone();
    let done = mcp.ok("extract_branch", confirmed);
    let made = done["tree"].as_str().unwrap_or_else(|| panic!("{done}")).to_string();
    assert_eq!(mcp.ok("get_connection_tree", json!({"tree": made}))["untitled"], true);
}
