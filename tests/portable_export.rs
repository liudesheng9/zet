//! Portable export (#257): `zt export` writes one tree into a folder of exactly two files, and
//! nothing of the rest of the box comes with it. Every test drives the real command against a
//! scratch archive (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`); the live archive is never touched.

#[path = "support/mcp_client.rs"]
mod mcp_client;

use mcp_client::ScratchBox;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

/// The two names an export may hold.
const CARDS: &str = "cards.json";
const VIEWER: &str = "index.html";

fn address(value: Value) -> String {
    value["address"].as_str().expect("address").to_string()
}

/// A book with one reading line and a second tree its Cards point at.
struct Book {
    scratch: ScratchBox,
    first: String,
    second: String,
    outside: String,
}

fn book() -> Book {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    let root = address(mcp.ok("create_topic", json!({ "title": "别的主题" })));
    let outside = address(mcp.ok(
        "create_successor",
        json!({ "at": &root, "title": "树外的卡", "body": "树外的内容。" }),
    ));
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{Test2026, title={Test Book}}" }),
    );
    let first = address(mcp.ok(
        "create_successor",
        json!({ "at": "Test2026", "title": "第一章", "body": "开头。" }),
    ));
    let second = address(mcp.ok(
        "create_successor",
        json!({
            "at": &first,
            "title": "第二章",
            "body": format!("承接 [[{first}]]，也见 [[{outside}]]。"),
        }),
    ));
    Book { scratch, first, second, outside }
}

/// Run `zt export` with these arguments and answer the folder it wrote.
fn export(scratch: &ScratchBox, args: &[&str]) -> PathBuf {
    let out = scratch.cmd().arg("export").args(args).output().expect("zt export");
    assert!(
        out.status.success(),
        "zt export {args:?} failed: {}\n{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let line = String::from_utf8_lossy(&out.stdout).to_string();
    let folder = line
        .lines()
        .find_map(|line| line.split(" to ").nth(1))
        .and_then(|rest| rest.rsplit_once(" ("))
        .map(|(folder, _)| PathBuf::from(folder))
        .unwrap_or_else(|| panic!("the success line names the folder: {line}"));
    assert!(folder.is_dir(), "{folder:?} is the folder of: {line}");
    folder
}

fn cards_json(folder: &Path) -> Value {
    serde_json::from_slice(&fs::read(folder.join(CARDS)).expect("cards.json")).expect("plain JSON")
}

fn card<'a>(export: &'a Value, address: &str) -> &'a Value {
    export["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .find(|card| card["address"] == address)
        .unwrap_or_else(|| panic!("{address} is in the export"))
}

fn file_names(folder: &Path) -> Vec<String> {
    let mut names = fs::read_dir(folder)
        .expect("read the folder")
        .map(|entry| entry.expect("entry").file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    names.sort();
    names
}

#[test]
fn one_tree_lands_in_a_folder_of_two_files() {
    let book = book();
    let folder = export(&book.scratch, &["Test2026"]);
    assert_eq!(file_names(&folder), [CARDS, VIEWER]);
    assert!(
        folder.starts_with(book.scratch.archive_root.join("export")),
        "the default folder lies under the archive root: {folder:?}"
    );

    let data = cards_json(&folder);
    assert_eq!(data["format"], "zt-portable-export");
    assert_eq!(data["version"], 2);
    assert_eq!(data["tree"]["id"], "Test2026");
    assert_eq!(data["tree"]["kind"], "literature");
    assert_eq!(data["tree"]["workspace"], "agent");
    let addresses: Vec<&str> = data["cards"]
        .as_array()
        .expect("cards")
        .iter()
        .map(|card| card["address"].as_str().expect("address"))
        .collect();
    assert_eq!(addresses, ["Test2026", book.first.as_str(), book.second.as_str()]);
}

/// A two-row Term table: `均衡` is published as a head.
const TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | 双方都不改变策略 | §1.1 |\n";
const OTHER_TABLE: &str = "## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 弹性 | 弹性系数 | 价格变化的反应程度 | §2.3 |\n";

/// A term list with one head under `at`; answers the head's address.
fn term_head(mcp: &mut mcp_client::Mcp, at: &str, name: &str, table: &str) -> String {
    let list = address(mcp.ok(
        "create_successor",
        json!({ "at": at, "kind": "side", "title": "术语表：测试", "type": "term", "body": table }),
    ));
    address(mcp.ok(
        "create_successor",
        json!({
            "at": &list,
            "kind": "side",
            "title": name,
            "type": "term",
            "term": name,
            "body": format!(
                "术语：{name}\n\n类别：概念\n\n## 释义\n\n一个定义。\n\n## 在本书中的作用\n\n第一章用它。\n\n来源：[[{at}]] §1.1"
            ),
        }),
    ))
}

/// Keep an image in the scratch archive, as the Scholar bridge would; answers its digest.
fn keep_figure(scratch: &ScratchBox, marker: u8) -> String {
    use sha2::{Digest, Sha256};
    // One real pixel, so a browser decodes it; a trailing byte keeps two figures apart.
    let mut bytes = b"\x89\x50\x4e\x47\x0d\x0a\x1a\x0a\x00\x00\x00\x0d\x49\x48\x44\x52\x00\x00\x00\x01\x00\x00\x00\x01\x08\x06\x00\x00\x00\x1f\x15\xc4\x89\x00\x00\x00\x0d\x49\x44\x41\x54\x78\xda\x63\x64\x60\xf8\x5f\x0f\x00\x02\x87\x01\x80\xeb\x47\xba\x92\x00\x00\x00\x00\x49\x45\x4e\x44\xae\x42\x60\x82".to_vec();
    bytes.push(marker);
    let digest = Sha256::digest(&bytes).iter().map(|byte| format!("{byte:02x}")).collect::<String>();
    let folder = scratch.archive_root.join("artifacts");
    fs::create_dir_all(&folder).expect("artifact folder");
    fs::write(folder.join(format!("{digest}.png")), bytes).expect("artifact");
    digest
}

/// A book that shows everything a Card can show, and a second tree on either side of its Links.
struct Rich {
    scratch: ScratchBox,
    first: String,
    shows: String,
    head: String,
    far_card: String,
    far_head: String,
    figure: String,
    gone: String,
}

fn rich() -> Rich {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_literature", json!({ "bibtex": "@book{Far2026, title={Far Book}}" }));
    let far_card = address(mcp.ok(
        "create_successor",
        json!({ "at": "Far2026", "title": "远处的卡", "body": "远处的内容。" }),
    ));
    let far_head = term_head(&mut mcp, &far_card, "弹性", OTHER_TABLE);
    mcp.ok("create_literature", json!({ "bibtex": "@book{Test2026, title={Test Book}}" }));
    let first = address(mcp.ok(
        "create_successor",
        json!({ "at": "Test2026", "title": "第一章", "body": "开头。" }),
    ));
    let head = term_head(&mut mcp, &first, "均衡", TABLE);
    let figure = keep_figure(&scratch, 1);
    let gone = keep_figure(&scratch, 2);
    let shows = address(mcp.ok(
        "create_successor",
        json!({
            "at": &first,
            "title": "第二章",
            "body": format!(
                "见 [[{first}]]，又见 [[{far_card}]]。\n\n质量是 $x^2$。\n\n均衡 [[均衡@{head}]] 与弹性 [[弹性@{far_head}]]。\n\n![图一](artifact:{figure})\n\n![图一再现](artifact:{figure})\n\n![不在了](artifact:{gone})\n\n读 [原书](scholar:77#page=3)。\n\n```chart\n<div>[[{first}]] 和 [[{far_card}]]</div>\n```\n"
            ),
        }),
    ));
    // A Card of the other tree that points in: its title must not travel.
    mcp.ok(
        "create_successor",
        json!({
            "at": &far_card,
            "title": "从别的树指进来",
            "body": format!("别的树读 [[{first}]]，也用均衡 [[均衡@{head}]]。"),
        }),
    );
    fs::remove_file(scratch.archive_root.join("artifacts").join(format!("{gone}.png")))
        .expect("the figure leaves the archive");
    drop(mcp);
    Rich { scratch, first, shows, head, far_card, far_head, figure, gone }
}

#[test]
fn a_bare_export_is_a_usage_error_and_nothing_exports_the_box() {
    let book = book();
    for args in [&[][..], &["--out", "x"][..], &["--all"][..], &["a", "b"][..]] {
        let out = book.scratch.cmd().arg("export").args(args).output().expect("zt export");
        assert!(!out.status.success(), "zt export {args:?} must fail");
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("usage: zt export <tree>"),
            "{args:?}: {out:?}"
        );
    }
    assert!(!book.scratch.archive_root.join("export").exists());
}

#[test]
fn a_tree_that_does_not_exist_is_named_and_leaves_nothing() {
    let book = book();
    let out = book.scratch.cmd().args(["export", "Nope2026"]).output().expect("zt export");
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("tree `Nope2026` does not exist"),
        "{out:?}"
    );
    assert!(!book.scratch.archive_root.join("export").exists());
}

#[test]
fn any_address_names_its_tree_and_a_topic_tree_exports_too() {
    let book = book();
    let by_address = export(&book.scratch, &[&book.second]);
    assert_eq!(cards_json(&by_address)["tree"]["id"], "Test2026");

    let topic = export(&book.scratch, &["0"]);
    let data = cards_json(&topic);
    assert_eq!(data["tree"]["id"], "0");
    assert_eq!(data["tree"]["root"], "0/0");
    assert_eq!(data["tree"]["kind"], "topic");
    let addresses: Vec<&str> =
        data["cards"].as_array().unwrap().iter().map(|c| c["address"].as_str().unwrap()).collect();
    assert_eq!(addresses, ["0/0", book.outside.as_str()]);
}

#[test]
fn cards_come_in_reading_order_with_their_edges() {
    let scratch = ScratchBox::up();
    let mut mcp = scratch.mcp(&[]);
    mcp.ok("create_topic", json!({ "title": "顺序" }));
    // 0/1 .. 0/11: ten comes after nine, although "0/10" sorts before "0/2".
    let mut at = "0/0".to_string();
    for number in 1..=11 {
        at = address(mcp.ok(
            "create_successor",
            json!({ "at": at, "title": format!("第 {number} 张"), "body": "内容。" }),
        ));
    }
    // Two Side successors of 0/2, the first with a Direct successor.
    let side_a = address(mcp.ok(
        "create_successor",
        json!({ "at": "0/2", "kind": "side", "title": "旁一", "body": "内容。" }),
    ));
    let side_b = address(mcp.ok(
        "create_successor",
        json!({ "at": "0/2", "kind": "side", "title": "旁二", "body": "内容。" }),
    ));
    let side_a_next = address(mcp.ok(
        "create_successor",
        json!({ "at": &side_a, "kind": "direct", "title": "旁一续", "body": "内容。" }),
    ));
    drop(mcp);
    assert_eq!((side_a.as_str(), side_b.as_str()), ("0/2|a", "0/2|b"));
    let data = cards_json(&export(&scratch, &["0"]));
    let order: Vec<&str> =
        data["cards"].as_array().unwrap().iter().map(|c| c["address"].as_str().unwrap()).collect();
    assert_eq!(
        order,
        [
            "0/0", "0/1", "0/2", "0/2|a", side_a_next.as_str(), "0/2|b", "0/3", "0/4", "0/5",
            "0/6", "0/7", "0/8", "0/9", "0/10", "0/11",
        ]
    );
    assert_eq!(card(&data, "0/2|a")["edge"], "side");
    assert_eq!(card(&data, "0/2|a")["parent"], "0/2");
    assert_eq!(card(&data, &side_a_next)["edge"], "direct");
    assert_eq!(card(&data, &side_a_next)["parent"], "0/2|a");
    assert_eq!(card(&data, "0/3")["parent"], "0/2");
    assert_eq!(card(&data, "0/0")["edge"], "root");
}

#[test]
fn a_card_carries_its_data_and_its_rendered_html() {
    let scene = rich();
    let data = cards_json(&export(&scene.scratch, &["Test2026"]));
    let root = card(&data, "Test2026");
    assert_eq!(root["kind"], "literature");
    assert_eq!(root["title"], "Test Book");
    assert!(root["bibtex"].as_str().unwrap().contains("@book{Test2026"));

    let head = card(&data, &scene.head);
    assert_eq!(head["type"], "term");
    assert_eq!(head["term"], "均衡");

    let shows = card(&data, &scene.shows);
    assert_eq!(shows["title"], "第二章");
    assert_eq!(shows["type"], "idea");
    assert!(shows["text"].as_str().unwrap().contains("# 第二章"), "{shows}");
    assert!(shows["html"].as_str().unwrap().contains("<p>"), "{shows}");
    assert_eq!(shows["term_use_targets"], json!([scene.head, scene.far_head]));
}

#[test]
fn nothing_generated_and_nothing_of_the_machine_travels() {
    let scene = rich();
    let folder = export(&scene.scratch, &["Test2026"]);
    let raw = fs::read_to_string(folder.join(CARDS)).expect("cards.json");
    let root = scene.scratch.archive_root.to_str().unwrap().to_string();
    for absent in [
        "zt:reverse-links",
        "Reverse links",
        "Term uses",
        "从别的树指进来",
        "row_id",
        "pinned_by",
        "archive_root",
        root.as_str(),
        root.replace('\\', "\\\\").as_str(),
    ] {
        assert!(!raw.contains(absent), "{absent:?} must not travel");
    }
}

#[test]
fn a_link_inside_routes_and_a_link_outside_stays_inert() {
    let scene = rich();
    let data = cards_json(&export(&scene.scratch, &["Test2026"]));
    let html = card(&data, &scene.shows)["html"].as_str().unwrap().to_string();
    assert!(html.contains(&format!("href=\"#/card/{}\"", scene.first)), "{html}");
    assert!(!html.contains(&format!("#/card/{}", scene.far_card)), "no route out: {html}");
    assert!(html.contains("class=\"zt-link outside\""), "{html}");
    assert!(html.contains("远处的卡"), "the outside chip keeps its title: {html}");

    assert!(html.contains("class=\"zt-link zt-term\""), "{html}");
    assert!(html.contains("双方都不改变策略"), "an inside Term link keeps its meaning: {html}");
    assert!(html.contains("class=\"zt-link zt-term outside\""), "{html}");
    assert!(html.contains("价格变化的反应程度"), "an outside Term link keeps its meaning: {html}");

    let outside = data["outside"].as_array().expect("outside");
    let far = outside.iter().find(|row| row["address"] == scene.far_card.as_str()).expect("far card");
    assert_eq!(far["title"], "远处的卡");
    assert_eq!(far["from"], json!([scene.shows]));
    assert!(outside.iter().any(|row| row["address"] == scene.far_head.as_str()), "{outside:?}");
}

#[test]
fn a_figure_travels_once_and_a_lost_figure_is_counted() {
    let scene = rich();
    let out = scene.scratch.cmd().args(["export", "Test2026"]).output().expect("zt export");
    let line = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(line.contains("1 figures are not in this archive"), "{line}");
    let folder = line.lines().find_map(|l| l.split(" to ").nth(1)).and_then(|r| r.rsplit_once(" (")).unwrap().0;
    let data = cards_json(Path::new(folder));

    let artifacts = data["artifacts"].as_object().expect("artifacts");
    assert_eq!(artifacts.len(), 1, "{artifacts:?}");
    assert_eq!(artifacts[&scene.figure]["type"], "image/png");
    // The eight signature bytes open every PNG's base64 with the same letters.
    assert!(artifacts[&scene.figure]["base64"].as_str().unwrap().starts_with("iVBORw0KGgo"));

    let html = card(&data, &scene.shows)["html"].as_str().unwrap();
    assert_eq!(html.matches(&format!("data-artifact=\"{}\"", scene.figure)).count(), 2, "{html}");
    assert!(html.contains(&format!("data-artifact=\"{}\"", scene.gone)), "{html}");
    assert!(!html.contains("/artifact/"), "no server route: {html}");
}

#[test]
fn a_document_link_is_its_label_and_a_chart_travels_as_a_page() {
    let scene = rich();
    let data = cards_json(&export(&scene.scratch, &["Test2026"]));
    let html = card(&data, &scene.shows)["html"].as_str().unwrap();
    assert!(html.contains("原书"), "{html}");
    assert!(!html.contains("/scholar/"), "no server route: {html}");

    let charts = data["charts"].as_object().expect("charts");
    assert_eq!(charts.len(), 1, "{charts:?}");
    let (id, page) = charts.iter().next().unwrap();
    assert!(html.contains(&format!("data-chart=\"{id}\"")), "{html}");
    let page = page.as_str().unwrap();
    assert!(page.contains("__ZT_THEME__"), "the reader fills the theme: {page}");
    // A chart cannot open a Card that is not in the export.
    let links = page.split("window.ZT_LINKS = ").nth(1).unwrap().split(";</script>").next().unwrap();
    let links: Value = serde_json::from_str(links).expect("chart links");
    assert_eq!(links[&scene.first]["exists"], true);
    assert_eq!(links[&scene.far_card]["exists"], false);
    assert_eq!(links[&scene.far_card]["title"], "远处的卡");
}

#[test]
fn out_names_an_empty_folder_and_refuses_a_full_one() {
    let book = book();
    let base = tempfile::tempdir().expect("tempdir");
    let new_folder = base.path().join("deep").join("export-here");
    let wrote = export(&book.scratch, &["Test2026", "--out", new_folder.to_str().unwrap()]);
    assert_eq!(wrote, new_folder);
    assert_eq!(file_names(&new_folder), [CARDS, VIEWER]);

    let empty = base.path().join("empty");
    fs::create_dir(&empty).expect("empty folder");
    assert_eq!(export(&book.scratch, &["--out", empty.to_str().unwrap(), "Test2026"]), empty);

    let full = base.path().join("full");
    fs::create_dir(&full).expect("full folder");
    fs::write(full.join("keep.txt"), "mine").expect("a file");
    let out = book.scratch.cmd().args(["export", "Test2026", "--out"]).arg(&full).output().expect("zt");
    assert!(!out.status.success(), "{out:?}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("not empty"), "{out:?}");
    assert_eq!(file_names(&full), ["keep.txt"]);
}

#[test]
fn a_failed_export_leaves_no_half_folder() {
    let book = book();
    let base = tempfile::tempdir().expect("tempdir");
    let blocked = base.path().join("a-file");
    fs::write(&blocked, "in the way").expect("a file");
    let out = book.scratch.cmd().args(["export", "Test2026", "--out"]).arg(blocked.join("x")).output().expect("zt");
    assert!(!out.status.success(), "{out:?}");
    assert_eq!(file_names(base.path()), ["a-file"]);
}

#[test]
fn two_exports_of_one_revision_differ_only_in_the_time() {
    let scene = rich();
    let base = tempfile::tempdir().expect("tempdir");
    let one = base.path().join("one");
    let two = base.path().join("two");
    export(&scene.scratch, &["Test2026", "--out", one.to_str().unwrap()]);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    export(&scene.scratch, &["Test2026", "--out", two.to_str().unwrap()]);
    let (mut first, mut second) = (cards_json(&one), cards_json(&two));
    assert_ne!(first["exported"]["at"], second["exported"]["at"]);
    first["exported"]["at"] = json!(null);
    second["exported"]["at"] = json!(null);
    assert_eq!(first, second);
}

#[test]
fn the_viewer_is_the_same_file_for_every_tree_and_holds_no_card() {
    let scene = rich();
    let test = export(&scene.scratch, &["Test2026"]);
    let far = export(&scene.scratch, &["Far2026"]);
    let viewer = fs::read(test.join(VIEWER)).expect("viewer");
    assert_eq!(viewer, fs::read(far.join(VIEWER)).expect("viewer"));
    let text = String::from_utf8(viewer).expect("UTF-8");
    assert!(!text.contains("第一章"));
    assert!(!text.contains("Test2026"));
}

/// Run the real-browser check on this export. A machine with no Node or no browser skips it and
/// says so, as the page-module tests do; `cargo test` itself needs no extra program.
fn browser_check(scene: &Rich, folder: &Path) -> Option<Value> {
    let facts = json!({
        "root": "Test2026",
        "first": scene.first,
        "first_title": "第一章",
        "shows": scene.shows,
        "shows_title": "第二章",
        "head": scene.head,
        "head_title": "均衡",
        "far_title": "远处的卡",
        "term_meaning": "双方都不改变策略",
        "other_tree_title": "从别的树指进来",
        "body_word": "质量是",
    });
    let base = tempfile::tempdir().expect("tempdir");
    let scene_file = base.path().join("scene.json");
    fs::write(&scene_file, facts.to_string()).expect("scene");
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/portable-export/check-viewer.mjs");
    let output = match std::process::Command::new("node")
        .arg(script)
        .arg("--folder")
        .arg(folder)
        .arg("--scene")
        .arg(&scene_file)
        .output()
    {
        Ok(output) => output,
        Err(_) => {
            eprintln!("node is not installed; the reader was not opened");
            return None;
        }
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let line = text.lines().last().unwrap_or_default();
    let report: Value = serde_json::from_str(line)
        .unwrap_or_else(|_| panic!("check printed {text:?}\n{}", String::from_utf8_lossy(&output.stderr)));
    if report.get("skipped").is_some() {
        eprintln!("no browser here; the reader was not opened");
        return None;
    }
    Some(report)
}

#[test]
fn a_real_browser_opens_the_export_from_disk() {
    let scene = rich();
    let folder = export(&scene.scratch, &["Test2026"]);
    if let Some(report) = browser_check(&scene, &folder) {
        assert_eq!(report["failures"], json!([]), "{report}");
    }
}

/// The running GUI of a scratch box: its port and its token.
fn gui_of(scratch: &ScratchBox) -> (u16, String) {
    let out = scratch.cmd().arg("gui").output().expect("zt gui");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let port: u16 = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("zt gui: http://127.0.0.1:"))
        .and_then(|rest| rest.trim_end_matches('/').parse().ok())
        .unwrap_or_else(|| panic!("zt gui prints its URL: {text}"));
    let (_, page) = http(port, "GET", "/", "", "");
    let token = page
        .split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("the page carries the GUI token")
        .to_string();
    (port, token)
}

fn http(port: u16, method: &str, path: &str, token: &str, body: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect to the GUI");
    let request = format!(
        "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("send");
    let mut reply = Vec::new();
    stream.read_to_end(&mut reply).expect("read");
    let reply = String::from_utf8_lossy(&reply).to_string();
    let status = reply.split(' ').nth(1).and_then(|code| code.parse().ok()).expect("status line");
    let body = reply.split_once("\r\n\r\n").map(|(_, body)| body.to_string()).unwrap_or_default();
    (status, body)
}

fn post_export(port: u16, token: &str, body: &Value) -> (u16, Value) {
    let (status, text) = http(port, "POST", "/api/export", token, &body.to_string());
    (status, serde_json::from_str(&text).unwrap_or_else(|_| panic!("JSON answer: {text}")))
}

#[test]
fn the_gui_route_writes_the_same_two_files_into_the_export_folder() {
    let book = book();
    let (port, token) = gui_of(&book.scratch);
    let (status, answer) = post_export(port, &token, &json!({ "tree": book.second }));
    assert_eq!(status, 200, "{answer}");
    assert_eq!(answer["tree"], "Test2026", "any address names its tree");
    assert_eq!(answer["cards"], 3);
    assert_eq!(answer["outside"], 1);
    assert_eq!(answer["missing"], 0);
    assert!(answer["bytes"].as_u64().unwrap() > 1_000_000, "{answer}");
    let folder = PathBuf::from(answer["folder"].as_str().expect("folder"));
    assert!(folder.starts_with(book.scratch.archive_root.join("export")), "{folder:?}");
    assert_eq!(file_names(&folder), [CARDS, VIEWER]);
    assert_eq!(cards_json(&folder)["tree"]["id"], "Test2026");
}

#[test]
fn the_gui_route_refuses_what_it_cannot_export_and_writes_nothing() {
    let book = book();
    let (port, token) = gui_of(&book.scratch);
    let (status, answer) = post_export(port, &token, &json!({ "tree": "Nope2026" }));
    assert_eq!(status, 400);
    assert_eq!(answer["error"], "tree `Nope2026` does not exist");
    let (status, answer) = post_export(port, &token, &json!({}));
    assert_eq!(status, 400);
    assert_eq!(answer["error"], "missing `tree` field");
    let (status, _) = post_export(port, "wrong", &json!({ "tree": "Test2026" }));
    assert_eq!(status, 403);
    assert!(!book.scratch.archive_root.join("export").exists());
}

#[test]
fn a_real_browser_exports_from_the_running_gui() {
    let scene = rich();
    let (port, _) = gui_of(&scene.scratch);
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/portable-export/check-viewer.mjs");
    let output = match std::process::Command::new("node")
        .arg(script)
        .arg("--live")
        .arg(format!("http://127.0.0.1:{port}/"))
        .arg("--tree")
        .arg("Test2026")
        .output()
    {
        Ok(output) => output,
        Err(_) => {
            eprintln!("node is not installed; the GUI was not opened");
            return;
        }
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let report: Value = serde_json::from_str(text.lines().last().unwrap_or_default())
        .unwrap_or_else(|_| panic!("check printed {text:?}\n{}", String::from_utf8_lossy(&output.stderr)));
    if report.get("skipped").is_none() {
        assert_eq!(report["failures"], json!([]), "{report}");
    }
}
