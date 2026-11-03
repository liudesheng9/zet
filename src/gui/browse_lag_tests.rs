//! #248: reads that never wait for writes, and no Term-meaning stall. Tests at the GUI's seams:
//! the Card answer, the Snapshot cache, the request pool and the page's sources.

use super::*;
use std::time::{Duration, Instant};

fn app_at(root: &Path) -> App {
    App {
        root: root.to_path_buf(),
        token: "t".repeat(32),
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

/// A Literature tree `bk` whose Term list `bk/1` has `heads` rows and as many Term heads
/// `bk/1|a`, `bk/1|b`, …, and a Regular Card `bk/2` that uses the fifth Term. Written straight
/// into the table: the gates are not under test, and a thousand gated writes would take minutes.
fn archive_with_term_list(heads: usize) -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    crate::initialize_database(temp.path()).expect("initialize");
    let conn = crate::open_db(temp.path()).unwrap();
    let insert = |location: Option<&str>, key: Option<&str>, lit: bool, text: &str, card_type: &str, name: Option<&str>| {
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, term_name)
             VALUES(?1, ?2, 0, ?3, ?4, ?5, 'agent', ?6, ?7)",
            rusqlite::params![location, key, lit, lit.then_some("@book{bk, title={B}}"), text, card_type, name],
        )
        .unwrap();
    };
    insert(None, Some("bk"), true, "# 书\n\n索引。\n", "idea", None);
    let mut table = String::from("# 术语表\n\n术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n");
    for n in 1..=heads {
        table.push_str(&format!("| 术语{n} | — | 意思{n} | §{n} |\n"));
    }
    insert(Some("bk/1"), None, false, &table, "term", None);
    for n in 1..=heads {
        let name = format!("术语{n}");
        insert(Some(&format!("bk/1|{}", label(n))), None, false, &format!("# {name}\n\n{name}的定义。\n"), "term", Some(&name));
    }
    insert(Some("bk/2"), None, false, "# 一张卡\n\n这里用到[[术语5@bk/1|e]]。\n", "idea", None);
    temp
}

/// #250: a Term link's meaning comes from its own list row, and the first Card that renders a
/// Term link on a fresh Snapshot does not wait for a parse of the list per head. Before #250 the
/// build parsed the whole table once per head: 40 s for atkinson's 748 heads.
#[test]
fn a_term_link_shows_its_meaning_without_a_parse_per_head() {
    let temp = archive_with_term_list(1000);
    let app = app_at(temp.path());
    // Read the Snapshot first, through a Card with no Term link, so only the meanings are timed.
    get(&app, "/api/card", &[("at", "bk/1|a")]);
    let start = Instant::now();
    let card = get(&app, "/api/card", &[("at", "bk/2")]);
    let took = start.elapsed();
    let html = card["html"].as_str().unwrap();
    assert!(html.contains("data-meaning=\"意思5\""), "{html}");
    assert!(took < Duration::from_millis(500), "the first Term-linked Card took {took:?}");
}

/// A small Literature tree `bk` (a Term list of three heads and the Card `bk/2`), for the
/// Snapshot cache tests.
fn small_archive() -> tempfile::TempDir {
    archive_with_term_list(3)
}

/// One change through Card operations, as another client would make it.
fn edit_elsewhere(root: &Path, title: &str) {
    crate::ops::edit(
        root,
        Access::Owner,
        "bk/2",
        Some(&format!("# {title}\n\n这里用到[[术语2@bk/1|b]]。\n")),
        TypeChange::default(),
        None,
    )
    .unwrap();
}

/// Run `read` on its own thread and answer what it returned within `patience`, or `None`.
fn within<T: Send + 'static>(patience: Duration, read: impl FnOnce() -> T + Send + 'static) -> (Option<T>, thread::JoinHandle<()>, std::sync::mpsc::Receiver<T>) {
    let (send, receive) = std::sync::mpsc::channel();
    let handle = thread::spawn(move || {
        let _ = send.send(read());
    });
    let answer = receive.recv_timeout(patience).ok();
    (answer, handle, receive)
}

/// #251: the cache's lock is a read lock. While one reader builds the changed Snapshot, the
/// others are answered from the previous one at once, instead of waiting behind the build. The
/// build ends with the change, and the next read gets it.
#[test]
fn reads_during_a_rebuild_are_answered_from_the_previous_snapshot() {
    let temp = small_archive();
    let snapshots = Arc::new(Snapshots::new(temp.path()));
    let before = snapshots.get().unwrap();
    edit_elsewhere(temp.path(), "改过的卡");
    // Hold every build at its start, as a slow read of a large box would.
    let gate = snapshots.build_gate.lock().unwrap();
    let builder = {
        let snapshots = Arc::clone(&snapshots);
        thread::spawn(move || snapshots.get().unwrap())
    };
    let start = Instant::now();
    while !snapshots.building() {
        assert!(start.elapsed() < Duration::from_secs(5), "the builder never started");
        thread::sleep(Duration::from_millis(5));
    }
    let (stale, _, _) = within(Duration::from_secs(2), {
        let snapshots = Arc::clone(&snapshots);
        move || snapshots.get().unwrap()
    });
    let stale = stale.expect("a read during a build waited for it");
    assert!(Arc::ptr_eq(&stale, &before), "the previous Snapshot answers");
    // A read that needs the change waits for the build, and is not answered before it ends.
    let (early, waiter, late) = within(Duration::from_millis(300), {
        let snapshots = Arc::clone(&snapshots);
        move || snapshots.fresh().unwrap()
    });
    assert!(early.is_none(), "a fresh read must wait for the build");
    drop(gate);
    let built = builder.join().unwrap();
    assert_eq!(built.title("bk/2"), "改过的卡");
    let fresh = late.recv_timeout(Duration::from_secs(5)).expect("the fresh read ends with the build");
    waiter.join().unwrap();
    assert!(Arc::ptr_eq(&fresh, &built));
    assert!(Arc::ptr_eq(&snapshots.get().unwrap(), &built), "the next read gets the change");
}

/// #251: a Card the previous Snapshot lacks is not refused while a build is running: its read
/// waits for the build and opens the new Card.
#[test]
fn a_card_created_elsewhere_opens_while_a_rebuild_runs() {
    let temp = small_archive();
    let app = Arc::new(app_at(temp.path()));
    get(&app, "/api/card", &[("at", "bk/2")]);
    crate::ops::create_successor(
        temp.path(),
        Access::Owner,
        crate::ops::Successor::Direct,
        "bk/2",
        CardType::Idea,
        None,
        "# 新卡\n\n新的。\n",
    )
    .unwrap();
    let gate = app.snapshots.build_gate.lock().unwrap();
    let builder = {
        let app = Arc::clone(&app);
        thread::spawn(move || get(&app, "/api/overview", &[]))
    };
    let start = Instant::now();
    while !app.snapshots.building() {
        assert!(start.elapsed() < Duration::from_secs(5), "the builder never started");
        thread::sleep(Duration::from_millis(5));
    }
    let (_, reader, answer) = within(Duration::from_millis(100), {
        let app = Arc::clone(&app);
        move || get(&app, "/api/card", &[("at", "bk/3")])
    });
    drop(gate);
    builder.join().unwrap();
    let card = answer.recv_timeout(Duration::from_secs(5)).expect("the new Card's read ends");
    reader.join().unwrap();
    assert_eq!(card["title"], "新卡");
}

/// #252: writes have their own lane. While another client holds the edit lock and five POSTs
/// wait for it (more than the GUI's request threads), a Card read is answered at once; when the
/// lock is released every POST is answered, one at a time.
#[test]
fn reads_are_answered_while_writes_wait_for_the_edit_lock() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    let temp = small_archive();
    let server = Arc::new(Server::http("127.0.0.1:0").expect("bind"));
    let port = server.server_addr().to_ip().expect("ip").port();
    let mut app = app_at(temp.path());
    app.port = port;
    let token = app.token.clone();
    let pool = serve(&server, Arc::new(app)).expect("request threads");
    let held = crate::acquire_edit_lock(temp.path(), "another client").expect("edit lock");
    let mut writes = Vec::new();
    for n in 0..5 {
        let mut post = TcpStream::connect(("127.0.0.1", port)).unwrap();
        post.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        // Each write continues the one before it, so the writes succeed only in arrival order.
        let body = json!({ "kind": "direct", "at": format!("bk/{}", 2 + n), "text": format!("# 写{n}\n\n正文。\n") }).to_string();
        write!(
            post,
            "POST /api/create HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
        post.flush().unwrap();
        writes.push(post);
        // Let this POST reach the lane before the next is sent: arrival order is send order.
        thread::sleep(Duration::from_millis(60));
    }
    thread::sleep(Duration::from_millis(300));
    let started = Instant::now();
    let mut read = TcpStream::connect(("127.0.0.1", port)).unwrap();
    read.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    write!(read, "GET /api/card?at=bk%2F2 HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nConnection: close\r\n\r\n").unwrap();
    let mut answer = String::new();
    read.read_to_string(&mut answer).expect("the read is answered while writes wait");
    assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(started.elapsed() < Duration::from_secs(2), "the read waited {:?}", started.elapsed());
    drop(held);
    for mut post in writes {
        let mut answer = String::new();
        post.read_to_string(&mut answer).expect("each write is answered");
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
    }
    // The writes went one at a time, in arrival order.
    let app = app_at(temp.path());
    for n in 0..5 {
        assert_eq!(get(&app, "/api/card", &[("at", &format!("bk/{}", 3 + n))])["title"], format!("写{n}"));
    }
    pool.stop();
}

/// #251/#252: a POST's own change is what the page reads next: after a write is answered, a
/// read never gets a Snapshot from before it, even while another reader's build is running.
#[test]
fn a_read_after_a_gui_write_sees_the_write() {
    let temp = small_archive();
    let app = Arc::new(app_at(temp.path()));
    get(&app, "/api/card", &[("at", "bk/2")]);
    let body = json!({ "kind": "direct", "at": "bk/2", "text": "# 页面写的\n\n正文。\n" });
    if let Err(error) = app.answer_write(&Method::Post, "/api/create", &Default::default(), &body) {
        panic!("the write: {}", error.message);
    }
    let gate = app.snapshots.build_gate.lock().unwrap();
    let builder = {
        let app = Arc::clone(&app);
        thread::spawn(move || get(&app, "/api/overview", &[]))
    };
    let start = Instant::now();
    while !app.snapshots.building() {
        assert!(start.elapsed() < Duration::from_secs(5), "the builder never started");
        thread::sleep(Duration::from_millis(5));
    }
    let (_, reader, answer) = within(Duration::from_millis(100), {
        let app = Arc::clone(&app);
        move || get(&app, "/api/tree", &[("id", "bk")])
    });
    drop(gate);
    builder.join().unwrap();
    let tree = answer.recv_timeout(Duration::from_secs(5)).expect("the read ends");
    reader.join().unwrap();
    assert!(tree.to_string().contains("页面写的"), "the read after the write sees it");
}

use super::large_tree_tests::layout_js;

/// #254: two moves whose answers arrive in reverse order end on the second Card: an answer to a
/// request that is no longer the latest is dropped.
#[test]
fn the_page_applies_only_the_answer_to_its_latest_move() {
    let Some(out) = layout_js(
        "await (async () => {
           const seq = L.latestOnly(); const shown = [];
           const answer = (card, ms) => new Promise((r) => setTimeout(() => r(card), ms));
           const move = async (to, ms) => { const n = seq.begin(); const card = await answer(to, ms); if (seq.isLatest(n)) shown.push(card); };
           await Promise.all([move('hansen2021demand/1', 60), move('hart1995firms/1', 10)]);
           return shown;
         })()",
    ) else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    assert_eq!(out, json!(["hart1995firms/1"]));
}

/// #254: a Card that has not arrived after 300 ms is marked pending, and after 5 s the reader
/// also names what it waits for.
#[test]
fn a_slow_card_is_marked_pending_then_named() {
    let Some(out) = layout_js("[0, 299, 300, 4999, 5000, 30000].map((ms) => L.pendingStage(ms))") else {
        eprintln!("node is not installed; the layout module was not run");
        return;
    };
    assert_eq!(out, json!(["none", "none", "pending", "pending", "slow", "slow"]));
}

/// #254: `loadPointer` aborts the request it supersedes, drops a late answer (Card and tree), and
/// shows the pending mark from `pendingStage`; a dropped answer never toasts or falls back.
#[test]
fn load_pointer_applies_only_its_latest_answer() {
    let start = APP_JS.find("async function loadPointer()").expect("loadPointer");
    let load = &APP_JS[start..start + APP_JS[start..].find("\n  }\n").unwrap()];
    for needle in [
        "const seq = pointerLoads.begin();",
        "new AbortController()",
        "pointerAbort.abort()",
        "if (!pointerLoads.isLatest(seq)) return;",
        "showPending(",
    ] {
        assert!(load.contains(needle), "loadPointer lacks `{needle}`:\n{load}");
    }
    let checks = load.matches("if (!pointerLoads.isLatest(seq)) return;").count();
    assert!(checks >= 3, "after the Card answer, its error and the tree answer: {checks}");
    let catch = &load[load.find("} catch (error) {").expect("the error path")..];
    assert!(
        catch.find("if (!pointerLoads.isLatest(seq)) return;").unwrap() < catch.find("toast(").unwrap(),
        "a superseded request never toasts"
    );
    assert!(APP_JS.contains("async function api(path, body, signal)"), "api passes a signal to fetch");
    assert!(APP_CSS.contains(".reader.pending"), "the pending reader has its look");
}

/// `bk` with 60 Cards that link `bk/2` and 55 that use the Term `bk/1|a`, plus `bk/1|b` linked
/// by three Cards of its own tree.
fn archive_with_many_inbound() -> tempfile::TempDir {
    let temp = archive_with_term_list(3);
    let conn = crate::open_db(temp.path()).unwrap();
    for n in 0..60 {
        let tail = if n < 55 { "这里用到[[术语1@bk/1|a]]。" } else { "" };
        let near = if n < 3 { " 见[[bk/1|b]]。" } else { "" };
        conn.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type) VALUES(?1, 0, 0, ?2, 'agent', 'idea')",
            rusqlite::params![format!("bk/{}", 3 + n), format!("# 卡{n}\n\n参见[[bk/2]]。{tail}{near}\n")],
        )
        .unwrap();
    }
    temp
}

/// #255: a Card answer carries at most fifty inbound, Reverse-link and Term-use entries with
/// their counts, the in-tree inbound sources the Map's arcs need when there are at most sixteen,
/// and the stored text only when asked. `inbound=all` and `text=1` give everything.
#[test]
fn a_card_answer_is_lean_until_the_reader_asks_for_more() {
    let temp = archive_with_many_inbound();
    let app = app_at(temp.path());
    let card = get(&app, "/api/card", &[("at", "bk/2")]);
    assert!(card.get("text").is_none(), "the stored text comes on request");
    assert_eq!(card["inbound"].as_array().unwrap().len(), 50);
    assert_eq!(card["reverse_links"].as_array().unwrap().len(), 50);
    assert_eq!((card["inbound_count"].clone(), card["reverse_count"].clone()), (json!(60), json!(60)));
    assert_eq!(card["inbound_near"], Value::Null, "more than sixteen in-tree sources draw no arcs");
    let all = get(&app, "/api/card", &[("at", "bk/2"), ("inbound", "all"), ("text", "1")]);
    assert_eq!(all["inbound"].as_array().unwrap().len(), 60);
    assert_eq!(all["reverse_links"].as_array().unwrap().len(), 60);
    assert!(all["text"].as_str().unwrap().contains("# 一张卡"), "{}", all["text"]);

    let head = get(&app, "/api/card", &[("at", "bk/1|a")]);
    assert_eq!(head["term_uses"].as_array().unwrap().len(), 50);
    assert_eq!(head["term_use_count"], 55);
    let head = get(&app, "/api/card", &[("at", "bk/1|a"), ("inbound", "all")]);
    assert_eq!(head["term_uses"].as_array().unwrap().len(), 55);

    let near = get(&app, "/api/card", &[("at", "bk/1|b")]);
    assert_eq!(near["inbound_near"], json!(["bk/3", "bk/4", "bk/5"]));
}

/// #255: the overview names the Locks by a fingerprint, so the page reads `/api/locks` only when
/// they changed: a Card write leaves it as it was, and a Lock moves it.
#[test]
fn the_locks_fingerprint_moves_with_the_locks_only() {
    let temp = small_archive();
    let app = app_at(temp.path());
    let before = get(&app, "/api/overview", &[])["locks"].clone();
    assert!(before.as_str().is_some_and(|print| print.len() == 24), "{before}");
    edit_elsewhere(temp.path(), "又改了");
    assert_eq!(get(&app, "/api/overview", &[])["locks"], before, "a Card write is no Lock change");
    let plan = app.route(&Method::Post, "/api/lock", &Default::default(), &json!({ "root": "bk" }))
        .unwrap_or_else(|error| panic!("plan: {}", error.message));
    app.route(
        &Method::Post,
        "/api/lock",
        &Default::default(),
        &json!({ "root": "bk", "confirm": "lock", "plan_hash": plan["plan"]["plan_hash"] }),
    )
    .unwrap_or_else(|error| panic!("lock: {}", error.message));
    assert_ne!(get(&app, "/api/overview", &[])["locks"], before, "a Lock moves it");
}

/// #255: the page reads the Locks only when the overview's fingerprint moved, draws the Map's
/// inbound arcs from `inbound_near`, and fetches what the lean answer leaves out when the reader
/// asks for it: the stored text for the writing desk and the Stored Markdown fold, and every
/// inbound entry for `+N more` and the Term uses.
#[test]
fn the_page_reads_what_the_lean_answer_leaves_out_on_request() {
    let start = APP_JS.find("  async function refresh() {").expect("refresh");
    let refresh = &APP_JS[start..start + APP_JS[start..].find("\n  }\n").unwrap()];
    assert!(refresh.contains("overview.locks !== state.locksPrint"), "{refresh}");
    assert!(!refresh.contains("api('overview'), api('spreads'), api('locks')"), "no Locks read on every refresh");
    assert!(APP_JS.contains("card.inbound_near"), "the Map's arcs come from inbound_near");
    for needle in [
        "async function cardWith(card, part)",
        "card = await cardWith(card, 'text');",
        "(await cardWith(card, 'text')).text",
        "referredBy(await cardWith(card, 'inbound'))",
        "(await cardWith(card, 'inbound')).term_uses",
        "state.locksPrint = locks ? overview.locks : null;",
    ] {
        assert!(APP_JS.contains(needle), "app.js lacks `{needle}`");
    }
}
