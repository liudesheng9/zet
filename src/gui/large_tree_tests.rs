//! #238: the GUI stays responsive with very large successor lists. The Map's focus lens shows a
//! window of a long sibling list and folds the rest into bundles; the page's pure rules run here
//! under Node, as the other `src/gui/*.js` modules do.

use super::*;

/// Run `expression` under Node with `src/gui/layout.js` bound to `L`, and answer the JSON it
/// prints; `None` when Node is not installed (the test then says so and passes).
pub(super) fn layout_js(expression: &str) -> Option<Value> {
    let dir = tempfile::tempdir().expect("temp dir");
    let file = dir.path().join("layout.js");
    fs::write(&file, LAYOUT_JS).expect("layout module");
    let driver = dir.path().join("driver.mjs");
    let script = format!(
        "import {{ createRequire }} from 'node:module';\n\
         const require = createRequire(import.meta.url);\n\
         const L = require(process.argv[2]);\n\
         {TREE_HELPERS}\n\
         process.stdout.write(JSON.stringify(({expression})));\n"
    );
    fs::write(&driver, script).expect("driver");
    let output = match std::process::Command::new("node")
        .arg(&driver)
        .arg(&file)
        .output()
    {
        Ok(output) => output,
        Err(_) => return None,
    };
    assert!(
        output.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).expect("UTF-8 output");
    Some(serde_json::from_str(&text).unwrap_or_else(|error| panic!("driver printed {text:?}: {error}")))
}

/// Trees for the lens tests. `label(n)` is the n-th Side label (1 = a, 27 = aa), as the archive
/// orders them. `sideList(n)` is the Literature tree `B`: root `B`, its Direct successor `B/1`, and
/// `n` Side successors `B/1|a`, `B/1|b`, ... under `B/1`. `mainLine(n)` is the Topic tree `7/0`
/// with a Direct run `7/1` ... `7/n`.
const TREE_HELPERS: &str = r#"
const label = (n) => { let s = ''; while (n > 0) { n -= 1; s = String.fromCharCode(97 + (n % 26)) + s; n = Math.floor(n / 26); } return s; };
const card = (address, parent, edge) => ({ address, parent, edge, links: [], kinds: {} });
const sideList = (n) => {
  const cards = [card('B', null, 'root'), card('B/1', 'B', 'direct')];
  for (let i = 1; i <= n; i += 1) cards.push(card('B/1|' + label(i), 'B/1', 'side'));
  return L.outline(cards, 'B');
};
const mainLine = (n) => {
  const cards = [card('7/0', null, 'root')];
  for (let i = 1; i <= n; i += 1) cards.push(card('7/' + i, i === 1 ? '7/0' : '7/' + (i - 1), 'direct'));
  return L.outline(cards, '7/0');
};
const shape = (view, parent) => {
  const rows = view.nodes.filter((n) => n.kind === 'bundle' ? n.parent === parent : view.index.get(n.address) && n.up === parent);
  return rows.map((n) => (n.kind === 'bundle' ? { bundle: n.count, from: n.from, through: n.through } : n.address));
};
"#;

/// A Card on the path in a list of 1,600 Side successors sees its own slip, eight siblings on each
/// side, the list's first and last slip, and two bundles that hold every other slip of the list.
#[test]
fn a_long_list_on_the_path_shows_a_window_and_two_bundles() {
    let Some(out) = layout_js(
        "(() => {
           const tree = sideList(1600);
           const focus = 'B/1|' + label(800);
           const view = L.focusLens(tree, focus, {});
           return { rows: shape(view, 'B/1'), total: view.nodes.length };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    let rows = out["rows"].as_array().expect("rows");
    let mut expected = vec![json!("B/1|a"), json!({ "bundle": 790, "from": "B/1|b", "through": "B/1|adk" })];
    for n in 792..=808 {
        expected.push(json!(format!("B/1|{}", label(n))));
    }
    expected.push(json!({ "bundle": 791, "from": format!("B/1|{}", label(809)), "through": format!("B/1|{}", label(1599)) }));
    expected.push(json!(format!("B/1|{}", label(1600))));
    assert_eq!(rows, &expected, "{out}");
    // The root, B/1, 19 slips and 2 bundles.
    assert_eq!(out["total"], 23);
}

/// The focus's own long list starts at its first slip: seventeen slips, one bundle, the last slip.
/// A riffle of that bundle (its `anchor` as the window) shows the slips just behind it, and the
/// Pointer stays where it is.
#[test]
fn the_focus_list_starts_at_its_first_slip_and_a_riffle_moves_the_window() {
    let Some(out) = layout_js(
        "(() => {
           const tree = sideList(1600);
           const view = L.focusLens(tree, 'B/1', {});
           const bundle = view.nodes.find((n) => n.kind === 'bundle');
           const riffled = L.focusLens(tree, 'B/1', { windows: { 'B/1': bundle.anchor } });
           return { rows: shape(view, 'B/1'), anchor: bundle.anchor, riffled: shape(riffled, 'B/1'), focus: riffled.focus };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    let mut expected: Vec<Value> = (1..=17).map(|n| json!(format!("B/1|{}", label(n)))).collect();
    expected.push(json!({ "bundle": 1582, "from": format!("B/1|{}", label(18)), "through": format!("B/1|{}", label(1599)) }));
    expected.push(json!(format!("B/1|{}", label(1600))));
    assert_eq!(out["rows"].as_array().unwrap(), &expected, "{out}");
    // The bundle starts at index 17; the riffled window starts there.
    assert_eq!(out["anchor"], 17 + 8);
    let riffled = out["riffled"].as_array().unwrap();
    assert_eq!(riffled[0], json!("B/1|a"));
    assert_eq!(riffled[1], json!({ "bundle": 16, "from": "B/1|b", "through": format!("B/1|{}", label(17)) }));
    assert_eq!(riffled[2], json!(format!("B/1|{}", label(18))), "the window starts at the first slip the bundle held");
    assert_eq!(riffled[18], json!(format!("B/1|{}", label(34))));
    assert_eq!(out["focus"], "B/1", "a riffle never moves the Pointer");
}

/// A list of thirty shows whole, as every list did before; the thirty-first slip opens a window.
#[test]
fn thirty_slips_show_whole_and_thirty_one_open_a_window() {
    let Some(out) = layout_js(
        "(() => ({
           thirty: shape(L.focusLens(sideList(30), 'B/1|a', {}), 'B/1'),
           more: shape(L.focusLens(sideList(31), 'B/1|a', {}), 'B/1'),
         }))()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    let thirty: Vec<Value> = (1..=30).map(|n| json!(format!("B/1|{}", label(n)))).collect();
    assert_eq!(out["thirty"].as_array().unwrap(), &thirty);
    let mut more: Vec<Value> = (1..=17).map(|n| json!(format!("B/1|{}", label(n)))).collect();
    more.push(json!({ "bundle": 13, "from": format!("B/1|{}", label(18)), "through": format!("B/1|{}", label(30)) }));
    more.push(json!(format!("B/1|{}", label(31))));
    assert_eq!(out["more"].as_array().unwrap(), &more);
}

/// A long main line (the root's Direct run) is windowed in the root's column: the bundle hangs
/// below the Card before it, and the Card after a bundle hangs below the bundle, so the column
/// reads as one unbroken run.
#[test]
fn a_long_main_line_hangs_its_bundles_in_the_run() {
    let Some(out) = layout_js(
        "(() => {
           const view = L.focusLens(mainLine(200), '7/100', {});
           const at = (a) => view.index.get(a);
           const bundles = view.nodes.filter((n) => n.kind === 'bundle');
           return {
             rows: shape(view, '7/0'),
             edges: view.edges.filter((e) => e.from.startsWith('bundle') || e.to.startsWith('bundle')).map((e) => [e.from, e.to, e.shape]),
             column: [...new Set(view.nodes.map((n) => n.x))],
             ordered: view.nodes.every((n, i, all) => i === 0 || n.y >= all[i - 1].y),
           };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    let mut rows = vec![json!("7/1"), json!({ "bundle": 90, "from": "7/2", "through": "7/91" })];
    rows.extend((92..=108).map(|n| json!(format!("7/{n}"))));
    rows.push(json!({ "bundle": 91, "from": "7/109", "through": "7/199" }));
    rows.push(json!("7/200"));
    assert_eq!(out["rows"].as_array().unwrap(), &rows, "{out}");
    assert_eq!(
        out["edges"],
        json!([
            ["7/1", "bundle:7/0:1", "down"],
            ["bundle:7/0:1", "7/92", "down"],
            ["7/108", "bundle:7/0:108", "down"],
            ["bundle:7/0:108", "7/200", "down"],
        ])
    );
    assert_eq!(out["column"], json!([0]), "one column");
    assert_eq!(out["ordered"], true, "rows never overlap upward");
}

/// Whole tree unfolds every branch but keeps the window: a long list stays a window and its
/// bundles, and a slip folded in a bundle brings none of its own successors.
#[test]
fn whole_tree_keeps_the_window() {
    let Some(out) = layout_js(
        "(() => {
           const cards = [card('B', null, 'root'), card('B/1', 'B', 'direct')];
           for (let i = 1; i <= 100; i += 1) cards.push(card('B/1|' + label(i), 'B/1', 'side'));
           cards.push(card('B/1|bv|a', 'B/1|bv', 'side'));
           cards.push(card('B/1|b|a', 'B/1|b', 'side'));
           const view = L.focusLens(L.outline(cards, 'B'), 'B', { expandAll: true });
           return {
             count: view.nodes.filter((n) => n.kind !== 'bundle').length,
             bundles: view.nodes.filter((n) => n.kind === 'bundle').map((n) => [n.count, n.cards]),
             shownDeep: view.index.has('B/1|b|a'), hiddenDeep: view.index.has('B/1|bv|a'),
           };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    // B, B/1, 17 slips + the last, and B/1|b|a under a shown slip.
    assert_eq!(out["count"], 21, "{out}");
    // 82 slips folded, one of them (B/1|bv) carrying its own Side successor.
    assert_eq!(out["bundles"], json!([[82, 83]]));
    assert_eq!(out["shownDeep"], true);
    assert_eq!(out["hiddenDeep"], false);
}

/// A Card the focus links to stands up even inside a bundle, and splits it in two.
#[test]
fn a_linked_slip_splits_its_bundle() {
    let Some(out) = layout_js(
        "(() => {
           const tree = sideList(100);
           const target = 'B/1|' + label(60);
           const view = L.focusLens(tree, 'B/1|a', { links: [target] });
           return { rows: shape(view, 'B/1'), role: view.index.get(target).role };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    let mut rows: Vec<Value> = (1..=17).map(|n| json!(format!("B/1|{}", label(n)))).collect();
    rows.push(json!({ "bundle": 42, "from": format!("B/1|{}", label(18)), "through": format!("B/1|{}", label(59)) }));
    rows.push(json!(format!("B/1|{}", label(60))));
    rows.push(json!({ "bundle": 39, "from": format!("B/1|{}", label(61)), "through": format!("B/1|{}", label(99)) }));
    rows.push(json!(format!("B/1|{}", label(100))));
    assert_eq!(out["rows"].as_array().unwrap(), &rows, "{out}");
    assert_eq!(out["role"], "linked");
}

/// S3 (#241): the Map draws a Card only when it meets the camera's view plus a margin; a Card
/// straddling the edge is drawn, an empty view draws nothing. The view is the camera's world
/// rectangle (the plane scales from its top-left corner).
#[test]
fn the_map_draws_what_meets_the_view_and_its_margin() {
    let Some(out) = layout_js(
        "(() => {
           const n = (address, x, y) => ({ address, x, y, w: 100, h: 30 });
           const nodes = [n('in', 50, 50), n('edge', -90, 10), n('margin', 450, 50), n('far', 2000, 50), n('below', 50, 700)];
           const rect = L.worldRect({ x: -20, y: -40, k: 2 }, 800, 600);
           return {
             rect,
             bare: [...L.visible(nodes, rect, 0)].sort(),
             wide: [...L.visible(nodes, rect, 100)].sort(),
             none: [...L.visible(nodes, { x: 0, y: 0, w: 0, h: 0 }, 0)],
           };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    // Camera at (-20, -40) and scale 2 on an 800x600 viewport sees world x 10..410, y 20..320.
    assert_eq!(out["rect"], json!({ "x": 10, "y": 20, "w": 400, "h": 300 }));
    assert_eq!(out["bare"], json!(["edge", "in"]));
    assert_eq!(out["wide"], json!(["edge", "in", "margin"]));
    assert_eq!(out["none"], json!([]));
}

/// S2 (#240): a move glides only the Cards that move and that the eye can follow (on screen at
/// either end); an unchanged Card stays still; a move of more Cards than the limit snaps, as a
/// reduced-motion move does.
#[test]
fn a_move_glides_only_what_moves_on_screen_and_snaps_when_too_many_move() {
    let Some(out) = layout_js(
        "(() => {
           const g = (x, y) => ({ x, y, w: 100, h: 30 });
           const from = new Map([['still', g(0, 0)], ['moves', g(0, 40)], ['offscreen', g(5000, 0)], ['leaves', g(0, 80)]]);
           const to = new Map([['still', g(0, 0)], ['moves', g(0, 120)], ['offscreen', g(5000, 300)], ['enters', g(0, 200)]]);
           const rect = { x: 0, y: 0, w: 800, h: 600 };
           const plan = L.glidePlan(from, to, rect, 150);
           const many = new Map(); const later = new Map();
           for (let i = 0; i < 151; i += 1) { many.set('c' + i, g(0, i)); later.set('c' + i, g(0, i + 1)); }
           return { plan: { snap: plan.snap, moving: [...plan.moving].sort() }, many: L.glidePlan(many, later, rect, 150).snap,
                    limit: L.glidePlan(new Map([...many].slice(0, 150)), later, rect, 150).snap };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    // `enters` has no start (the render gives a new Card the place of its nearest shown ancestor
    // before it plans), `offscreen` never meets the view, and `still` does not move.
    assert_eq!(out["plan"], json!({ "snap": false, "moving": ["moves"] }));
    assert_eq!(out["many"], true, "151 moving Cards snap");
    assert_eq!(out["limit"], false, "150 still glide");
}

/// The Map's own code, from `const map = (() => {` to its closing `})();`.
fn map_js() -> &'static str {
    let start = APP_JS.find("  const map = (() => {").expect("the Map");
    let body = &APP_JS[start..];
    &body[..body.find("\n  })();\n").expect("end of the Map")]
}

/// S2 (#240): a move writes a card's class only when it changed, never reads the viewport's size
/// during a render, animates `transform` alone, and settles its marks with one timer; S3 (#241):
/// only the cards the camera sees have elements, and a pan or zoom culls on the next frame.
#[test]
fn a_map_move_writes_only_what_changed_and_draws_only_what_the_camera_sees() {
    let map = map_js();
    assert!(map.contains("if (el.ztBase === base) return;"), "the class guard");
    assert!(!map.contains("el.className = ['node'"), "no unconditional class write");
    assert!(!map.contains("clientWidth") && !map.contains("clientHeight"), "no layout read in a render");
    assert!(map.contains("const { w: vw, h: vh } = viewportSize();"));
    // A glide frame writes transforms only; sizes are set once, when an element is synced.
    let glide = &map[map.find("function glide(").unwrap()..];
    let glide = &glide[..glide.find("\n    }\n").unwrap()];
    assert!(glide.contains("place(geometry);") && !glide.contains("style.width") && !glide.contains("style.height"), "{glide}");
    let place = &map[map.find("function place(geometry)").unwrap()..];
    let place = &place[..place.find("\n    }\n").unwrap()];
    assert!(place.contains("el.style.transform = transform") && !place.contains("style.width"), "{place}");
    // One timer per render, not one per card.
    assert_eq!(map.matches("setTimeout(").count(), 1, "one settling timer");
    assert!(map.contains("const plan = ZtLayout.glidePlan(from, to,"));
    assert!(map.contains("if (fresh || plan.snap || reduceMotion())"));
    // Culling.
    assert!(map.contains("return ZtLayout.visible(placed, rect, margin(rect));"));
    assert!(map.contains("if (view.index.has(address)) el.remove(); // still in the lens, only out of view"));
    let camera = &APP_JS[APP_JS.find("  function applyCamera() {").unwrap()..];
    let camera = &camera[..camera.find("\n  }\n").unwrap()];
    assert!(camera.contains("if (state.mode === 'map' && state.card) map.cameraMoved();"), "{camera}");
    // Bundles riffle without moving the Pointer.
    assert!(map.contains("if (bundle) { riffle(address); return; }"));
    assert!(map.contains("windows = { ...windows, [bundle.parent]: bundle.anchor };"));
    assert!(map.contains("links: arcs.lens, expandAll: state.expandAll, windows,"));
}

/// The cards keep their layout and style inside them, without clipping what they draw outside
/// (the pointer label, part labels, the Table tab); a bundle is drawn in theme ink alone, thicker
/// the more slips it holds.
#[test]
fn cards_contain_their_layout_and_bundles_use_theme_ink() {
    assert!(APP_CSS.contains("\n.node { contain: layout style; }"));
    assert!(!APP_CSS.contains("contain: layout style paint"), "paint containment would clip the labels");
    for thick in ["thick-1", "thick-2", "thick-3"] {
        let rule = APP_CSS
            .lines()
            .find(|line| line.starts_with(&format!(".node.bundle.{thick} ")))
            .unwrap_or_else(|| panic!("no {thick}"));
        assert!(rule.contains("var(--stock-edge)"), "{rule}");
    }
    let bundle_rules: Vec<&str> = APP_CSS.lines().filter(|line| line.starts_with(".node.bundle")).collect();
    assert!(bundle_rules.len() >= 6);
    for rule in bundle_rules {
        assert!(!rule.contains('#'), "theme tokens only: {rule}");
    }
}

/// A GUI `App` on a fresh archive in `root`.
fn app_at(root: &Path) -> App {
    App {
        root: root.to_path_buf(),
        token: "t".to_string(),
        port: 0,
        charts: Charts::default(),
        snapshots: Snapshots::new(root),
        projections: Projections::installed(),
    }
}

fn get(app: &App, path: &str, query: &[(&str, &str)]) -> Value {
    let query = query.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    app.route(&Method::Get, path, &query, &Value::Null)
        .unwrap_or_else(|error| panic!("{path}: {}", error.message))
}

/// A Topic `0/0` with `0/1` and three Side successors under it.
fn archive_with_sides() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    crate::initialize_database(temp.path()).expect("initialize");
    let root = temp.path();
    let owner = crate::Access::Owner;
    crate::ops::create_topic(root, owner, crate::Workspace::Human, "# 书\n").unwrap();
    for (kind, at, text) in [
        (crate::ops::Successor::Direct, "0/0", "# 一\n\n一。\n"),
        (crate::ops::Successor::Side, "0/1", "# 甲\n\n甲。\n"),
        (crate::ops::Successor::Side, "0/1", "# 乙\n\n乙。\n"),
        (crate::ops::Successor::Side, "0/1", "# 丙\n\n丙。\n"),
    ] {
        crate::ops::create_successor(root, owner, kind, at, crate::CardType::Idea, None, text).unwrap();
    }
    temp
}

/// S4 (#242): the reader's Card names how many Side successors it has and carries only the
/// first (the → key's target); the reorder step asks for all of them.
#[test]
fn a_card_read_carries_its_side_count_and_only_the_first_side() {
    let temp = archive_with_sides();
    let app = app_at(temp.path());
    let card = get(&app, "/api/card", &[("at", "0/1")]);
    assert_eq!(card["sides_count"], 3, "{card}");
    let sides: Vec<&str> = card["sides"].as_array().unwrap().iter().map(|s| s["address"].as_str().unwrap()).collect();
    assert_eq!(sides, ["0/1|a"]);
    let all = get(&app, "/api/card", &[("at", "0/1"), ("sides", "all")]);
    let sides: Vec<&str> = all["sides"].as_array().unwrap().iter().map(|s| s["address"].as_str().unwrap()).collect();
    assert_eq!(sides, ["0/1|a", "0/1|b", "0/1|c"]);
    assert_eq!(all["sides_count"], 3);
    // A Card with no Side successor says so.
    assert_eq!(get(&app, "/api/card", &[("at", "0/1|a")])["sides_count"], 0);
}

/// S4 (#242): a long generated list folds in the reader. More than sixty rows (or chips) show
/// fifty and a control for the rest; sixty or fewer show whole.
#[test]
fn a_long_reader_list_shows_fifty_of_more_than_sixty() {
    let Some(out) = layout_js("[0, 10, 60, 61, 1589].map((n) => L.foldRows(n))") else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    assert_eq!(
        out,
        json!([
            { "shown": 0, "hidden": 0 },
            { "shown": 10, "hidden": 0 },
            { "shown": 60, "hidden": 0 },
            { "shown": 50, "hidden": 11 },
            { "shown": 50, "hidden": 1539 },
        ])
    );
}

/// S4 (#242): the reader cuts a long table before it reaches the page and wires the rest only when
/// it is laid out; chip rows fold at the same bound; the reorder keys ask for every Side successor.
#[test]
fn the_reader_lays_out_long_lists_only_on_request() {
    let reader = &APP_JS[APP_JS.find("  function renderReader() {").unwrap()..];
    let reader = &reader[..reader.find("\n  }\n").unwrap()];
    assert!(reader.contains("if (card.body.trim()) md.append(bodyNodes(card.html));"), "{reader}");
    assert!(!reader.contains("md.innerHTML = card.html"));
    assert!(reader.contains("chipRow(card.outbound, (link) => chip(link))"));
    let body = &APP_JS[APP_JS.find("  function bodyNodes(html) {").unwrap()..];
    let body = &body[..body.find("\n  }\n").unwrap()];
    for part in [
        "const template = document.createElement('template');",
        "const fold = ZtLayout.foldRows(body.rows.length);",
        "for (const row of rest) row.remove();",
        "for (const row of rest) wireMarkdown(row);",
        "text: `Show all ${all} rows`",
    ] {
        assert!(body.contains(part), "{part}: {body}");
    }
    let row = &APP_JS[APP_JS.find("  function chipRow(links, make, total = links.length, loadAll = null) {").unwrap()..];
    let row = &row[..row.find("\n  }\n").unwrap()];
    assert!(row.contains("links.slice(0, shown)") && row.contains("text: `+${hidden} more`"), "{row}");
    assert!(APP_JS.contains("(await api('card?at=' + enc(parent) + '&sides=all')).sides"));
    assert!(APP_CSS.contains(".btn.table-more"), "the table control has its place");
}

/// Two Topics: `0/0` with `0/1`, `0/2`, and `1/0` with `1/1`.
fn archive_with_two_trees() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    crate::initialize_database(temp.path()).expect("initialize");
    let root = temp.path();
    let owner = crate::Access::Owner;
    crate::ops::create_topic(root, owner, crate::Workspace::Human, "# 甲书\n").unwrap();
    crate::ops::create_topic(root, owner, crate::Workspace::Human, "# 乙书\n").unwrap();
    for (at, text) in [("0/0", "# 一\n\n一。\n"), ("0/1", "# 二\n\n二。\n"), ("1/0", "# 三\n\n三。\n")] {
        crate::ops::create_successor(root, owner, crate::ops::Successor::Direct, at, crate::CardType::Idea, None, text).unwrap();
    }
    temp
}

fn prints(app: &App) -> BTreeMap<String, String> {
    get(app, "/api/overview", &[])["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tree| (tree["tree"].as_str().unwrap().to_string(), tree["fingerprint"].as_str().unwrap_or_default().to_string()))
        .collect()
}

/// S5 (#243): the graph and each tree are the same summaries the Snapshot builds, read from its
/// cache; the raw answers the server sends are those summaries byte for byte.
#[test]
fn cached_graph_and_tree_answers_equal_the_summaries() {
    let temp = archive_with_two_trees();
    let app = app_at(temp.path());
    let snapshot = app.snapshot().unwrap();
    let summaries: Vec<Value> = snapshot.cards.iter().map(|card| snapshot.summary(card)).collect();
    assert_eq!(get(&app, "/api/graph", &[])["nodes"], json!(summaries));
    let tree = get(&app, "/api/tree", &[("id", "0")]);
    let members: Vec<Value> = summaries.iter().filter(|s| s["tree"] == "0").cloned().collect();
    assert_eq!(tree["cards"], json!(members));
    assert_eq!(tree["root"], "0/0");
    assert_eq!(tree["title"], "甲书");
    // The raw path sends exactly these values.
    let raw = app.raw_get("/api/graph", &BTreeMap::new()).expect("a raw route").unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), get(&app, "/api/graph", &[]));
    let query = BTreeMap::from([("id".to_string(), "1".to_string())]);
    let raw = app.raw_get("/api/tree", &query).expect("a raw route").unwrap_or_else(|e| panic!("{}", e.message));
    assert_eq!(serde_json::from_str::<Value>(&raw).unwrap(), get(&app, "/api/tree", &[("id", "1")]));
    assert!(app.raw_get("/api/card", &BTreeMap::new()).is_none(), "other routes build their answers");
    let missing = BTreeMap::from([("id".to_string(), "9".to_string())]);
    assert!(app.raw_get("/api/tree", &missing).unwrap().is_err());
}

/// S5 (#243): a tree's fingerprint moves when, and only when, one of its summaries changes: an edit
/// inside it, or a Link into it from another tree (its inbound count changes).
#[test]
fn a_tree_fingerprint_moves_only_with_its_own_summaries() {
    let temp = archive_with_two_trees();
    let root = temp.path();
    let app = app_at(root);
    let first = prints(&app);
    assert_eq!(first.len(), 2);
    assert!(first.values().all(|print| print.len() == 24), "{first:?}");
    let edit = |at: &str, text: &str| {
        ops::edit(root, crate::Access::Owner, at, Some(text), TypeChange::default(), None).unwrap();
    };
    edit("0/2", "# 二\n\n二，改了。\n");
    let second = prints(&app);
    assert_ne!(second["0"], first["0"], "the edited tree moves");
    assert_eq!(second["1"], first["1"], "the other tree stays");
    edit("0/2", "# 二\n\n二，引 [[1/1]]。\n");
    let third = prints(&app);
    assert_ne!(third["0"], second["0"]);
    assert_ne!(third["1"], second["1"], "a Link into tree 1 changes its inbound count");
}

/// S5 (#243): after a change the page fetches the trees whose fingerprint moved or that are new,
/// and drops the trees that are gone; an unchanged box asks for nothing. An overview without tree
/// fingerprints (an older service) asks for the whole graph.
#[test]
fn the_page_refetches_only_the_trees_whose_fingerprint_moved() {
    let Some(out) = layout_js(
        "(() => {
           const old = { a: '1', b: '2', c: '3' };
           return {
             changed: L.changedTrees(old, { a: '1', b: '9', d: '4' }),
             same: L.changedTrees(old, { ...old }),
             first: L.changedTrees(null, { a: '1' }),
             older: L.changedTrees(old, { a: null, b: '2' }),
           };
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    assert_eq!(out["changed"], json!({ "graph": false, "fetch": ["b", "d"], "drop": ["c"] }));
    assert_eq!(out["same"], json!({ "graph": false, "fetch": [], "drop": [] }));
    assert_eq!(out["first"], json!({ "graph": true, "fetch": [], "drop": [] }));
    assert_eq!(out["older"], json!({ "graph": true, "fetch": [], "drop": [] }));
}

/// S5 (#243): `refresh()` reads the overview first and asks for the graph only when the rule says
/// so; otherwise it fetches the changed trees and splices them in, and keeps the open tree unless
/// its fingerprint moved.
#[test]
fn refresh_splices_changed_trees_into_the_page() {
    let refresh = &APP_JS[APP_JS.find("  async function refresh() {").unwrap()..];
    let refresh = &refresh[..refresh.find("\n  }\n").unwrap()];
    for part in [
        "const change = ZtLayout.changedTrees(state.prints, prints);",
        "if (change.graph) setNodes((await api('graph')).nodes, overview.trees);",
        "api('tree?id=' + enc(id))",
        "spliceTrees(change.drop, fetched, overview.trees);",
        "state.prints = prints;",
    ] {
        assert!(refresh.contains(part), "{part}: {refresh}");
    }
    assert!(!refresh.contains("state.tree = null;\n"), "the open tree is kept unless it changed: {refresh}");
    assert!(!refresh.contains("api('graph')]"), "the graph is not fetched on every refresh");
}

/// S6 (#244): while one request is held inside the server (a POST whose body has not all arrived),
/// another read is answered; stopping the pool frees every thread.
#[test]
fn a_held_request_does_not_hold_up_another_read() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let temp = archive_with_sides();
    let server = Arc::new(Server::http("127.0.0.1:0").expect("bind"));
    let port = server.server_addr().to_ip().expect("ip").port();
    let mut app = app_at(temp.path());
    app.port = port;
    let pool = serve(&server, Arc::new(app)).expect("request threads");
    let mut held = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(
        held,
        "POST /api/render HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: t\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{{\"text\""
    )
    .unwrap();
    held.flush().unwrap();
    thread::sleep(Duration::from_millis(200));
    let started = Instant::now();
    let mut read = TcpStream::connect(("127.0.0.1", port)).unwrap();
    read.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(read, "GET /api/card?at=0%2F1 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: t\r\nConnection: close\r\n\r\n").unwrap();
    let mut answer = String::new();
    read.read_to_string(&mut answer).expect("the read is answered while the POST is held");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(answer.contains("\"sides_count\":3"), "{answer}");
    assert!(started.elapsed() < Duration::from_secs(4));
    drop(held);
    pool.stop();
}

/// S6 (#244), #252: writes go one at a time on the writer lane: every POST is handed to it after
/// its body is read, so a slow client never holds the lane, and no POST is routed on a request
/// thread.
#[test]
fn writes_go_to_the_writer_lane_after_their_body() {
    let handle = &GUI_SOURCE[GUI_SOURCE.find("    fn handle(&self, mut request: Request, writes: &Writes) {").unwrap()..];
    let handle = &handle[..handle.find("\n    }\n").unwrap()];
    let body = handle.find("request.as_reader().read_to_string(&mut raw)").expect("the body read");
    let lane = handle.find("writes.send(write)").expect("the writer lane");
    let route = handle.find("let result = self.route(&method, &path, &query, &body);").expect("the route");
    assert!(body < lane && lane < route, "{handle}");
}

/// S8 (#246): the lens benchmark runs the shipped layout over a saved answer, reports the largest
/// lens per tree, and fails a bound that lens breaks.
#[test]
fn the_lens_bench_reports_the_largest_lens_and_enforces_its_bound() {
    let dir = tempfile::tempdir().expect("temp dir");
    let layout = dir.path().join("layout.js");
    fs::write(&layout, LAYOUT_JS).unwrap();
    // One Literature tree: B, B/1 and 1,600 Side successors.
    let mut cards = vec![
        json!({ "address": "B", "tree": "B", "parent": null, "edge": "root", "links": [], "kinds": {} }),
        json!({ "address": "B/1", "tree": "B", "parent": "B", "edge": "direct", "links": [], "kinds": {} }),
    ];
    for n in 1..=1600 {
        cards.push(json!({ "address": format!("B/1|{}", label(n)), "tree": "B", "parent": "B/1", "edge": "side", "links": [], "kinds": {} }));
    }
    let graph = dir.path().join("graph.json");
    fs::write(&graph, json!({ "nodes": cards }).to_string()).unwrap();
    let bench = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/gui-bench/lens-bench.mjs");
    let run = |bound: &str| {
        std::process::Command::new("node")
            .arg(&bench)
            .arg(&graph)
            .args(["--bound", bound, "--every", "40", "--layout"])
            .arg(&layout)
            .output()
    };
    let Ok(ok) = run("200") else {
        eprintln!("node is not installed; the lens bench was not run");
        return;
    };
    assert!(ok.status.success(), "{}", String::from_utf8_lossy(&ok.stderr));
    let report: Value = serde_json::from_slice(&ok.stdout).unwrap();
    assert_eq!(report["ok"], true);
    assert_eq!(report["trees"][0]["cards"], 1602);
    assert!(report["trees"][0]["lens"].as_u64().unwrap() <= 23, "{report}");
    assert!(report["trees"][0]["whole"].as_u64().unwrap() <= 23, "{report}");
    let over = run("10").unwrap();
    assert_eq!(over.status.code(), Some(1), "a lens above the bound fails");
}

/// This module's parent source, so a test can pin how a request is handled.
const GUI_SOURCE: &str = include_str!("../gui.rs");

/// The n-th Side label, 1 = a, 27 = aa (the archive's order).
fn label(mut n: usize) -> String {
    let mut s = String::new();
    while n > 0 {
        n -= 1;
        s.insert(0, (b'a' + (n % 26) as u8) as char);
        n /= 26;
    }
    s
}
