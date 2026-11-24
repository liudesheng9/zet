//! The terminal Session at stations (docs/multi-tree-coordinates-proposal.md §9.2): `go` to a
//! coordinate, `n`/`p`/`b` walk the Connection tree, `home` opens the Card at its home and `back`
//! returns to the exact station, `places` and `conns` list occurrences and connections, the
//! mutation verbs run the shared plans, `del` at a mount is refused, and a station detached by
//! another client is reported instead of landing on another Card.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::{Mcp, ScratchBox};
use serde_json::json;
use std::io::{Read, Write};
use std::process::Stdio;
use std::thread;
use std::time::Duration;

fn fixture(mcp: &mut Mcp) {
    mcp.ok("create_literature", json!({"bibtex": "@book{Hart1995, title={Firms}}"}));
    mcp.ok("create_successor", json!({"at": "Hart1995", "title": "第一节", "body": "内容。\n\n来源：[[Hart1995]] §1"}));
    mcp.ok("create_literature", json!({"bibtex": "@book{Gros1986, title={Ownership}}"}));
    mcp.ok("create_connection_tree", json!({"title": "Session map"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/0", "target_at": "Gros1986"}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/1", "target_at": "Hart1995", "gloss": "Compare here."}));
    mcp.ok("place_card", json!({"host": "~1", "parent_coord": "~1/2", "edge": "side", "target_at": "Hart1995/1"}));
    mcp.ok("connect_placements", json!({"tree": "~1", "from_placement_id": "~1/2", "to_placement_id": "~1/1", "direction": "directed", "relation_label": "relaxes"}));
}

fn session(scratch: &ScratchBox, script: &str) -> String {
    let out = scratch.cmd().write_stdin(script).output().expect("session");
    String::from_utf8_lossy(&out.stdout).to_string()
}

#[test]
fn the_session_walks_stations_and_returns_to_the_exact_station() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    fixture(&mut mcp);
    let out = session(&scratch, "go ~1\nn\nn\nb\np\nhome\nback\nplaces\nconns\nq\n");
    assert!(out.contains("station: ~1/0 in ~1 · Session map"), "{out}");
    assert!(out.contains("station: ~1/1 in ~1"), "{out}");
    assert!(out.contains("station: ~1/2 in ~1"), "{out}");
    assert!(out.contains("gloss: Compare here."), "{out}");
    assert!(out.contains("station: ~1/2|a in ~1"), "b goes to the Side branch: {out}");
    assert!(out.contains("home: Hart1995 · literature · agent"), "{out}");
    assert!(out.contains("citation key: Hart1995"), "home opens the Card: {out}");
    let after_home = &out[out.find("citation key: Hart1995").unwrap()..];
    assert!(after_home.contains("station: ~1/2 in ~1"), "back returns to the station: {out}");
    assert!(out.contains("~1/2 in ~1 · Compare here."), "places: {out}");
    assert!(out.contains("out: Firms relaxes Ownership (In this tree · ~1)"), "conns: {out}");
    // A handle opens the Card at its home; a coordinate never stands for a Card in `del`.
    let out = session(&scratch, "go ~1/2\ndel\nmv 1/1\nq\n");
    assert!(out.contains("this is a station"), "{out}");
    assert_eq!(mcp.ok("get_card", json!({"at": "Hart1995"}))["address"], "Hart1995");
}

#[test]
fn station_verbs_run_the_shared_plans() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    fixture(&mut mcp);
    // edit-gloss writes at once; place mounts after the station; connect records a connection.
    let out = session(&scratch, "go ~1/1\nedit-gloss Base case.\nplace Hart1995/1\nq\n");
    assert!(out.contains("gloss: Base case."), "{out}");
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert!(view.to_string().contains("Base case."), "{view}");
    assert_eq!(view["placements"].as_array().unwrap().len(), 4, "Hart1995/1 already stands once: {view}");
    mcp.ok("create_literature", json!({"bibtex": "@book{Holm1999, title={Incentives}}"}));
    let out = session(&scratch, "go ~1/1\nplace Holm1999 side Challenges residual control.\nconnect ~1/1 directed 激励系统质疑\nq\n");
    assert!(out.contains("station: ~1/1|a in ~1"), "{out}");
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert!(view.to_string().contains("激励系统质疑"), "{view}");
    // unplace plans first and needs its confirmation word.
    let out = session(&scratch, "go ~1/1|a\nunplace\nunplace\nq\n");
    assert!(out.contains("type `unplace` to confirm"), "{out}");
    let view = mcp.ok("get_connection_tree", json!({"tree": "~1"}));
    assert!(!view.to_string().contains("Holm1999"), "{view}");
    // move-place plans and confirms with `move`.
    let out = session(&scratch, "go ~1/2|a\nmove-place ~1/1 side\nmove\nq\n");
    assert!(out.contains("station: ~1/1|a in ~1"), "{out}");
}

#[test]
fn a_station_detached_elsewhere_is_reported_and_its_home_opened() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    fixture(&mut mcp);
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &scratch.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("session");
    let mut stdin = child.stdin.take().unwrap();
    writeln!(stdin, "go ~1/2|a").unwrap();
    stdin.flush().unwrap();
    thread::sleep(Duration::from_millis(1500));
    let station = mcp.ok("get_card", json!({"at": "~1/2|a"}))["station"]["placement_id"].as_str().unwrap().to_string();
    let plan = mcp.ok("unplace", json!({"placement_id": station}));
    mcp.ok("unplace", json!({"placement_id": station, "confirm": plan["confirm"]}));
    writeln!(stdin).unwrap();
    writeln!(stdin, "q").unwrap();
    drop(stdin);
    let mut out = String::new();
    child.stdout.take().unwrap().read_to_string(&mut out).unwrap();
    child.wait().unwrap();
    assert!(out.contains("placement removed"), "{out}");
    assert!(out.contains("opened its Card at home `Hart1995/1`"), "{out}");
}
