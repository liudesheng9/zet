//! The cross-book Dependency Lens and seeding (docs/multi-tree-coordinates-proposal.md §8.3,
//! Phase 3; Acceptance §10.3 (3)): a shared cross-book definition with exact proof-step carriers,
//! dependents, cycles, the Connection-tree overlay that never enlarges the authored closure, and
//! one-time seeding that places a shared prerequisite once and creates no connection.

#[path = "support/gui_client.rs"]
mod gui_client;
#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::{Value, json};

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

struct Math {
    d1: String,
    d2: String,
    t: String,
    t_step: String,
    u: String,
}

/// BookA holds Definition D1 (compactness); BookB holds Definition D2, Theorem T (statement uses D2,
/// a proof step uses D1 across books) and Theorem U (uses D1).
fn math(mcp: &mut Mcp) -> Math {
    for key in ["BookA", "BookB"] {
        mcp.ok("create_literature", json!({"bibtex": format!("@book{{{key}, title={{Analysis {key}}}}}")}));
        mcp.ok("create_successor", json!({"at": key, "title": format!("{key} 第一章"), "body": format!("本章的内容。\n\n来源：[[{key}]] §1")}));
    }
    let side = |mcp: &mut Mcp, at: &str, args: Value| {
        let mut args = args;
        args["at"] = json!(at);
        args["kind"] = json!("side");
        address(mcp.ok("create_successor", args))
    };
    let next = |mcp: &mut Mcp, at: &str, args: Value| {
        let mut args = args;
        args["at"] = json!(at);
        address(mcp.ok("create_successor", args))
    };
    let d1 = side(mcp, "BookA/1", json!({"type": "model", "form": "definition", "level": "concept", "title": "紧致：每个开覆盖有有限子覆盖", "body": "**定义**：\n- (C1) 每个开覆盖都有有限子覆盖。"}));
    let d2 = side(mcp, "BookB/1", json!({"type": "model", "form": "definition", "level": "concept", "title": "连续映射：开集的原像是开集", "body": "**定义**：\n- (C1) 开集的原像是开集。"}));
    let t = side(mcp, "BookB/1", json!({"type": "model", "form": "theorem", "level": "formal", "title": "定理 T：紧集的连续像是紧集", "body": format!("**陈述**：\n- (H1) 依赖 [[{d2}]]：$f$ 是连续映射。\n- (H2) $K$ 紧致。")}));
    let premises = side(mcp, &t, json!({"facet": "前提", "title": "前提：f 连续且 K 紧致", "body": "(H1) 与 (H2)。"}));
    let t_step = next(mcp, &premises, json!({"facet": "证明：取有限子覆盖", "title": "证明：取有限子覆盖", "body": format!("依赖 [[{premises}]]。依据 [[{d1}]]：开覆盖有有限子覆盖，所以像也有。")}));
    let u = side(mcp, "BookB/1", json!({"type": "model", "form": "theorem", "level": "formal", "title": "定理 U：紧集上的连续函数有界", "body": format!("**陈述**：\n- (H1) 依据 [[{d1}]]：$K$ 紧致。")}));
    Math { d1, d2, t, t_step, u }
}

/// Plan a seeding, then confirm that exact plan.
fn seed(mcp: &mut Mcp, args: Value) -> Value {
    let plan = mcp.ok("seed_connection_tree", args.clone());
    assert_eq!(plan["applied"], false, "{plan}");
    let mut confirmed = args;
    confirmed["confirm"] = plan["confirm"].clone();
    confirmed["plan_hash"] = plan["plan_hash"].clone();
    confirmed["expected_revision"] = plan["expected_revision"].clone();
    mcp.ok("seed_connection_tree", confirmed)
}

fn addresses(list: &Value, field: &str) -> Vec<String> {
    list.as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .filter_map(|v| v[field].as_str().map(str::to_string))
        .collect()
}

/// Acceptance §10.3 (3): T and U both rest on D1 (T through a proof step in another book); the lens
/// renders one D1 with the exact proof-step carrier and sentence, lists both users of D1, separates
/// authored evidence from a Connection tree's working connection `U → D2`, keeps the closure
/// authored-only, and seeding places D1 once.
#[test]
fn a_shared_cross_book_definition_has_exact_carriers_users_and_one_seat() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let m = math(&mut mcp);

    let lens = mcp.ok("get_dependencies", json!({"at": m.t, "scope": "unit"}));
    assert_eq!(lens["focus"]["address"], m.t);
    let prereqs = addresses(&lens["prerequisites"], "address");
    assert!(prereqs.contains(&m.d1) && prereqs.contains(&m.d2), "{lens}");
    let to_d1 = lens["edges"].as_array().unwrap().iter().find(|e| e["to"] == m.d1.as_str()).expect("T → D1").clone();
    let evidence = &to_d1["evidence"][0];
    assert_eq!(evidence["carrier"], m.t_step, "the carrier is the proof step: {to_d1}");
    assert_eq!(evidence["sentence"], format!("依据 [[{}]]：开覆盖有有限子覆盖，所以像也有。", m.d1));
    assert!(evidence["provenance"].as_str().unwrap().starts_with("In the Card · "), "{evidence}");
    let step = mcp.ok("get_card", json!({"at": m.t_step}));
    let body = step["body"].as_str().unwrap();
    let range = evidence["macro_byte_range"].as_array().unwrap();
    let (start, end) = (range[0].as_u64().unwrap() as usize, range[1].as_u64().unwrap() as usize);
    assert_eq!(&body[start..end], format!("[[{}]]", m.d1), "the byte range names the exact macro");
    let sentence = evidence["sentence_byte_range"].as_array().unwrap();
    assert_eq!(&body[sentence[0].as_u64().unwrap() as usize..sentence[1].as_u64().unwrap() as usize], evidence["sentence"].as_str().unwrap());
    let to_d2 = lens["edges"].as_array().unwrap().iter().find(|e| e["to"] == m.d2.as_str()).expect("T → D2");
    assert_eq!(to_d2["evidence"][0]["carrier"], m.t, "the statement carries D2");
    // D1's users: T (through its proof step) and U.
    let users = mcp.ok("get_dependencies", json!({"at": m.d1, "scope": "unit"}));
    let dependents: Vec<String> = users["dependents"].as_array().unwrap().iter().map(|d| d["node"]["address"].as_str().unwrap().to_string()).collect();
    assert!(dependents.contains(&m.t) && dependents.contains(&m.u), "{users}");
    assert_eq!(dependents.len(), 2, "one node per unit, however many carriers: {users}");
    // Card scope reads only T's own Card: D2 from its statement, not D1 from its proof step.
    let card_scope = mcp.ok("get_dependencies", json!({"at": m.t, "scope": "card"}));
    let card_prereqs = addresses(&card_scope["prerequisites"], "address");
    assert_eq!(card_prereqs, vec![m.d2.clone()], "{card_scope}");

    // A working Connection tree: T → D1 (also authored) and U → D2 (working only).
    mcp.ok("create_connection_tree", json!({"title": "Compactness map"}));
    for (i, at) in [&m.d1, &m.d2, &m.t, &m.u].iter().enumerate() {
        mcp.ok("place_card", json!({"host": "~1", "parent_coord": format!("~1/{i}"), "target_at": at}));
    }
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/3", "to_placement_id": "~1/1", "direction": "directed", "relation_kind": "rests-on"}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/4", "to_placement_id": "~1/2", "direction": "directed", "relation_label": "也许依赖"}));
    let overlaid = mcp.ok("get_dependencies", json!({"at": m.t, "scope": "unit", "overlay_tree": "~1"}));
    assert_eq!(overlaid["prerequisites"], lens["prerequisites"], "the overlay never enlarges the closure");
    assert_eq!(overlaid["seed_order"], lens["seed_order"]);
    let connections = overlaid["overlay"]["connections"].as_array().unwrap();
    assert!(connections.iter().any(|c| c["class"] == "both" && c["to"] == m.d1.as_str()), "{overlaid}");
    let local = connections.iter().find(|c| c["class"] == "tree_local").expect("U → D2 is tree-local");
    assert_eq!(local["connection"]["provenance"], "In this tree · ~1");
    assert!(overlaid["overlay"]["authored_only"].as_array().unwrap().iter().any(|a| a["to"] == m.d2.as_str()));

    // Seed from a native synthesis that rests on T and U: D1 appears once, prerequisites first.
    mcp.ok("create_topic", json!({"title": "Synthesis", "body": "topic"}));
    let synthesis = address(mcp.ok("create_successor", json!({"at": "0/0", "title": "两条定理共享紧致性", "body": format!("依据 [[{}]] 和 依据 [[{}]]。", m.t, m.u)})));
    let args = json!({"from": "dependencies", "source_params": {"at": synthesis, "scope": "unit", "depth": 2}, "title": "Study order"});
    let plan = mcp.ok("seed_connection_tree", args.clone());
    assert_eq!(plan["applied"], false);
    let seeded = seed(&mut mcp, args);
    let homes: Vec<String> = seeded["stations"].as_array().unwrap().iter().map(|s| s["home_address"].as_str().unwrap().to_string()).collect();
    assert_eq!(homes.iter().filter(|h| **h == m.d1).count(), 1, "D1 is seated once: {seeded}");
    let pos = |a: &str| homes.iter().position(|h| h == a).unwrap();
    assert!(pos(&m.d1) < pos(&m.t) && pos(&m.d1) < pos(&m.u) && pos(&m.d2) < pos(&m.t), "prerequisites first: {homes:?}");
    assert_eq!(*homes.last().unwrap(), synthesis);
    let tree = mcp.ok("get_connection_tree", json!({"tree": seeded["tree"]}));
    assert_eq!(tree["connections"].as_array().unwrap().len(), 0, "seeding creates no connection");
}

/// §8.3: cycles are surfaced, and seeding a cyclic lens needs an explicit order; a selection seeds in
/// its own order with duplicates skipped.
#[test]
fn cycles_are_explicit_and_seeding_keeps_the_chosen_order() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Loop", "body": "topic"}));
    let a = address(mcp.ok("create_successor", json!({"at": "0/0", "title": "A", "body": "A 的内容。"})));
    let b = address(mcp.ok("create_successor", json!({"at": a, "title": "B", "body": format!("依据 [[{a}]]。")})));
    mcp.ok("edit_card", json!({"at": a, "body": format!("依据 [[{b}]]。")}));
    let lens = mcp.ok("get_dependencies", json!({"at": a, "depth": 2}));
    assert_eq!(lens["cycles"].as_array().unwrap().len(), 1, "{lens}");
    let refused = mcp.refused("seed_connection_tree", json!({"from": "dependencies", "source_params": {"at": a, "depth": 2}, "title": "No", "confirm": "seed"}));
    assert!(refused.to_string().contains("cycle"), "{refused}");
    let seeded = seed(&mut mcp, json!({"from": "dependencies", "source_params": {"at": a, "depth": 2, "order": [b, a]}, "title": "Chosen"}));
    let homes: Vec<&str> = seeded["stations"].as_array().unwrap().iter().map(|s| s["home_address"].as_str().unwrap()).collect();
    assert_eq!(homes, vec![b.as_str(), a.as_str()]);
    let picked = seed(&mut mcp, json!({"from": "search", "source_params": {"cards": [a, b, a, "0/99"]}, "title": "Picked"}));
    assert_eq!(picked["stations"].as_array().unwrap().len(), 2);
    assert_eq!(picked["duplicates"], json!([a]));
    assert_eq!(picked["missing"], json!(["0/99"]));
}

/// §8.4: seeding from a Hub keeps the Hub's member order and creates no connection.
#[test]
fn seeding_from_a_hub_keeps_member_order() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({"title": "Hubbed", "body": "topic"}));
    let one = address(mcp.ok("create_successor", json!({"at": "0/0", "title": "一", "body": "一。"})));
    let two = address(mcp.ok("create_successor", json!({"at": one, "title": "二", "body": "二。"})));
    let hub = mcp.ok("create_successor", json!({"at": two, "kind": "side", "type": "hub", "level": "line", "title": "线索：一与二", "body": format!("- [[{two}]] 第二步\n- [[{one}]] 第一步")}));
    let hub = hub["address"].as_str().unwrap().to_string();
    let seeded = seed(&mut mcp, json!({"from": "hub", "source_params": {"at": hub}, "title": "From hub"}));
    let homes: Vec<&str> = seeded["stations"].as_array().unwrap().iter().map(|s| s["home_address"].as_str().unwrap()).collect();
    assert_eq!(homes, vec![two.as_str(), one.as_str()]);
    assert_eq!(seeded["connections"], 0);
}
