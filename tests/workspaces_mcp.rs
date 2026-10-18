//! Workspaces and the `zt mcp` server: two Workspaces with Topic and
//! Literature Zones, Links across them, no moves between them, and an MCP
//! client that may read both but change only the agent Workspace.

use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

#[path = "support/host_tools.rs"]
mod host_tools;

struct TestHome {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

#[test]
fn ordinary_and_identified_writes_refuse_a_duplicate_leading_title_but_preserve_legacy_metadata_edits()
 {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let title = "税率提高会限制交易";
    let (error, failed) = mcp.call(
        "create_topic",
        json!({"title":title,"body":format!("# {title}\n\n这是说明。") }),
    );
    assert!(
        failed && error.to_string().contains("duplicate leading title"),
        "{error}"
    );
    assert_eq!(mcp.ok("list_trees", json!({}))["trees"], json!([]));
    mcp.ok("create_topic", json!({"title":"税收"}));
    mcp.ok(
        "create_successor",
        json!({"at":"0/0","title":title,"body":"税率改变了交易双方的选择。"}),
    );
    for body in [
        format!("{title}\n\n这是说明。"),
        format!("# {title} ###\n\n这是说明。"),
    ] {
        let (error, failed) = mcp.call("edit_card", json!({"at":"0/1","body":body}));
        assert!(
            failed && error.to_string().contains("duplicate leading title"),
            "{error}"
        );
    }
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/1"}))["body"],
        "税率改变了交易双方的选择。"
    );
    let (error,failed)=mcp.call("apply_batch",json!({"items":[
        {"op_key":"duplicate/group/1/create/good","client_ref":"good","tool":"create_successor","arguments":{"at":"0/1","title":"暂时的新卡"}},
        {"op_key":"duplicate/group/1/create/bad","client_ref":"bad","tool":"create_successor","arguments":{"at":"@@good","title":title,"body":format!("# {title}\n\n这是说明。")}}
    ]}));
    assert!(failed && error.to_string().contains("duplicate leading title"));
    assert_eq!(
        mcp.ok("get_tree", json!({"tree":"0"}))["cards"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let legacy =
        format!("# {title}\n\n# {title}\n\n原有的待审内容。\n\n<!-- zt:reverse-links -->\n");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).unwrap();
    conn.execute("UPDATE cards SET text=?1 WHERE location='0/1'", [&legacy])
        .unwrap();
    drop(conn);
    mcp.ok("edit_card", json!({"at":"0/1","type":"event"}));
    let preserved = mcp.ok("get_card", json!({"at":"0/1"}));
    assert_eq!(preserved["type"], "event");
    assert_eq!(preserved["body"], format!("# {title}\n\n原有的待审内容。"));
    mcp.ok(
        "edit_card",
        json!({"at":"0/1","body":"修复后的正文陈述一个完整论点。"}),
    );
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/1"}))["body"],
        "修复后的正文陈述一个完整论点。"
    );
    for body in [
        format!("    # {title}"),
        format!("> # {title}"),
        format!("```text\n# {title}\n```"),
    ] {
        let made = mcp.ok(
            "create_successor",
            json!({"at":"0/1","kind":"side","title":title,"body":body}),
        );
        assert_eq!(
            mcp.ok("get_card", json!({"at":made["address"]}))["body"],
            body,
            "literal code/quotation is not an authored leading title"
        );
    }
}
#[test]
fn a_ledger_can_distinguish_archive_replacement_from_a_consistent_restore_at_the_same_path() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let before = mcp.ok("get_cards", json!({"addresses":[],"view":"text"}));
    let identity = before["archive_identity"].clone();
    let id = identity["id"].as_str().expect("stable archive identity");
    assert!(id.len() == 32 && id.bytes().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(
        identity["path"],
        fs::canonicalize(&home.archive_root)
            .unwrap()
            .to_string_lossy()
            .as_ref()
    );
    mcp.ok("create_topic", json!({"title":"Retained archive"}));
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_identity"],
        identity
    );
    let saved = home.archive_root.join("consistent-snapshot.sqlite3");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).unwrap();
    conn.execute("VACUUM INTO ?1", [saved.to_string_lossy().as_ref()])
        .unwrap();
    drop(conn);
    drop(mcp);
    home.cmd().arg("down").assert().success();
    fs::rename(
        home.archive_root.join("zt.sqlite3"),
        home.archive_root.join("original.sqlite3"),
    )
    .unwrap();
    home.cmd().arg("up").assert().success();
    let mut fresh = home.mcp();
    let replacement =
        fresh.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_identity"].clone();
    assert_ne!(
        replacement["id"], identity["id"],
        "a different database at the same path must not reuse pending ledger work"
    );
    assert_eq!(replacement["path"], identity["path"]);
    drop(fresh);
    home.cmd().arg("down").assert().success();
    fs::copy(&saved, home.archive_root.join("zt.sqlite3")).unwrap();
    home.cmd().arg("up").assert().success();
    let mut restored = home.mcp();
    assert_eq!(
        restored.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_identity"],
        identity
    );
    assert_eq!(
        restored.ok("get_card", json!({"at":"0/0"}))["title"],
        "Retained archive"
    );
    home.cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .success();
    assert_eq!(
        restored.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_identity"],
        identity,
        "clearing Cards keeps the archive identity"
    );
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).unwrap();
    conn.execute("DELETE FROM metadata WHERE key='archive_id'", [])
        .unwrap();
    let (missing, failed) = restored.call("get_cards", json!({"addresses":[],"view":"text"}));
    assert!(failed && missing.to_string().contains("missing archive identity"));
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM metadata WHERE key='archive_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0, "a read must not invent a replacement identity");
    conn.execute(
        "INSERT INTO metadata(key,value) VALUES('archive_id','invalid')",
        [],
    )
    .unwrap();
    let (invalid, failed) = restored.call("get_cards", json!({"addresses":[],"view":"text"}));
    assert!(failed && invalid.to_string().contains("invalid archive identity"));
    conn.execute("UPDATE metadata SET value=?1 WHERE key='archive_id'", [id])
        .unwrap();
}
#[test]
fn an_oversized_preview_reply_is_rolled_back_before_the_mcp_reply_limit() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"large-draft/group/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Large draft"}},
        {"op_key":"large-draft/group/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"@@topic","title":"Large claim"}}
    ]}));
    let mut base = "large-draft/group/1/create/source".to_string();
    for i in 0..4 {
        let key = format!("large-draft/group/1/grow/paragraph-{i}");
        mcp.ok("apply_batch",json!({"items":[{"op_key":key,"client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":base,"owned_paragraph":{"text":format!("{i} {}", "x".repeat(1000000))}}}]}));
        base = key;
    }
    let revision =
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"].clone();
    let request = json!({"preview":true,"items":[{"op_key":"large-draft/group/1/link/new","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":base,"owned_paragraph":{"text":"依赖 [[0/0]]，后者给出了共同的讨论范围。"}}}]});
    let (error, failed) = mcp.call("apply_batch", request.clone());
    assert!(failed);
    assert_eq!(error["code"], "read_limit");
    assert_eq!(error["limit_kind"], "reply_bytes");
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        revision
    );
    let mut actual = request;
    actual.as_object_mut().unwrap().remove("preview");
    assert_eq!(
        mcp.ok("apply_batch", actual)["results"][0]["replayed"],
        false,
        "the rejected preview retained no receipt or authored change"
    );
}

#[test]
fn a_batch_preview_uses_real_validators_but_retains_no_cards_receipts_or_allocation() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let create = json!({"preview":true,"items":[
        {"op_key":"draft/group-1/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Draft scope"}},
        {"op_key":"draft/group-1/1/create/target","client_ref":"target","tool":"create_successor","arguments":{"at":"@@topic","title":"Constraint"}},
        {"op_key":"draft/group-1/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"@@target","title":"Claim","body":"Independent prose."}},
        {"op_key":"draft/group-1/1/create/link","client_ref":"link","tool":"edit_card","arguments":{"at":"@@source","base_receipt":"draft/group-1/1/create/source","owned_paragraph":{"text":"依赖 [[@@target]]，后者给出共同的约束。"}}}
    ]});
    let revision =
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"].clone();
    let draft = mcp.ok("apply_batch", create.clone());
    assert_eq!(draft["preview"], true);
    assert_eq!(draft["applied"], false);
    assert_eq!(draft["complete"], true);
    assert_eq!(draft["archive_revision"], revision);
    let source = draft["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["address"] == "0/2")
        .unwrap();
    assert_eq!(
        source["body"],
        "Independent prose.\n\n依赖 [[0/1]]，后者给出共同的约束。"
    );
    assert_eq!(draft["paragraphs"][0]["address"], "0/2");
    assert_eq!(draft["paragraphs"][0]["links"], json!(["0/1"]));
    assert_eq!(draft["paragraphs"][0]["kinds"], json!(["0/1"]));
    assert_eq!(mcp.ok("list_trees", json!({}))["trees"], json!([]));
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        revision
    );
    assert_eq!(
        mcp.ok("apply_batch", create.clone()),
        draft,
        "a preview retains no operation receipts or allocation"
    );
    let mut actual = create.clone();
    actual.as_object_mut().unwrap().remove("preview");
    let applied = mcp.ok("apply_batch", actual);
    assert!(
        applied["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["replayed"] == false)
    );
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/2"}))["text"],
        source["text"]
    );
    let replay = mcp.ok("apply_batch", create.clone());
    assert!(
        replay["results"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["replayed"] == true)
    );
    mcp.ok(
        "edit_card",
        json!({"at":"0/2","body":"Independent later correction."}),
    );
    let (conflict, failed) = mcp.call("apply_batch", create);
    assert!(
        failed,
        "historical receipts cannot preview a different current output"
    );
    assert_eq!(conflict["code"], "write_conflict");
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/2"}))["body"],
        "Independent later correction."
    );
}

#[test]
fn an_owned_paragraph_cannot_be_hidden_by_the_existing_cards_code_context() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    for (i, body) in [
        "Intro.\n\n```text\nAn unfinished code block.",
        "`An unfinished inline code span.",
        "Normal context.",
    ]
    .iter()
    .enumerate()
    {
        mcp.ok("apply_batch",json!({"items":[
            {"op_key":format!("run-hidden/case-{i}/1/create/topic"),"client_ref":"topic","tool":"create_topic","arguments":{"title":"Code context"}},
            {"op_key":format!("run-hidden/case-{i}/1/create/source"),"client_ref":"source","tool":"create_successor","arguments":{"at":"@@topic","title":"Claim","body":body}},
            {"op_key":format!("run-hidden/case-{i}/1/create/target"),"client_ref":"target","tool":"create_successor","arguments":{"at":"@@source","title":"Constraint"}}
        ]}));
        let (conflict,failed)=mcp.call("apply_batch",json!({"items":[{"op_key":format!("run-hidden/case-{i}/1/link/paragraph"),"client_ref":"source","tool":"edit_card","arguments":{"at":format!("{i}/1"),"base_receipt":format!("run-hidden/case-{i}/1/create/source"),"owned_paragraph":{"text":format!("依赖 [[{i}/2]]，这里的 `value` 是共同约束。")}}}]}));
        let card = mcp.ok("get_card", json!({"at":format!("{i}/1")}));
        if i < 2 {
            assert!(failed, "a new owned paragraph cannot silently become code");
            assert_eq!(conflict["code"], "write_conflict");
            assert_eq!(card["body"], *body);
        } else {
            assert!(!failed, "{conflict}");
            assert_eq!(card["outbound"][0]["address"], format!("{i}/2"));
            assert_eq!(
                card["body"],
                format!("{body}\n\n依赖 [[{i}/2]]，这里的 `value` 是共同约束。")
            );
        }
    }
}

#[test]
fn legacy_paragraph_adoption_is_explicit_and_ambiguous_owned_spans_refuse() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let paragraph = "依赖 [[0/1]]，后者给出共同的约束。";
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-adopt/group-1/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Adoption"}},
        {"op_key":"run-adopt/group-1/1/create/target","client_ref":"target","tool":"create_successor","arguments":{"at":"@@topic","title":"Constraint"}},
        {"op_key":"run-adopt/group-1/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"@@target","title":"Legacy claim","body":format!("Intro.\n\n{paragraph}")}}
    ]}));
    let owned = |mcp: &mut Mcp,
                 key: &str,
                 base: &str,
                 text: &str,
                 replaces: Option<&str>,
                 adopt: bool| {
        mcp.call("apply_batch",json!({"items":[{"op_key":key,"client_ref":"source","tool":"edit_card","arguments":{"at":"0/2","base_receipt":base,"owned_paragraph":{"text":text,"replaces_receipt":replaces,"adopt_legacy":adopt}}}]}))
    };
    let edit = |mcp: &mut Mcp, key: &str, base: &str, body: &str| {
        mcp.ok("apply_batch",json!({"items":[{"op_key":key,"client_ref":"source","tool":"edit_card","arguments":{"at":"0/2","base_receipt":base,"body":body}}]}))
    };
    let original = "run-adopt/group-1/1/create/source";
    let (_, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/1/link/unowned",
        original,
        paragraph,
        Some(original),
        false,
    );
    assert!(
        failed,
        "a normal create receipt cannot claim paragraph ownership"
    );
    let (_, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/1/link/append",
        original,
        paragraph,
        None,
        false,
    );
    assert!(failed);
    let revision =
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"].clone();
    let ownership = "run-adopt/group-1/1/link/adopt";
    let (adopted, failed) = owned(&mut mcp, ownership, original, paragraph, None, true);
    assert!(!failed, "{adopted}");
    assert_eq!(
        adopted["results"][0]["result"]["owned_paragraph"]["adopted"],
        true
    );
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        revision
    );
    let replacement = "特例 [[0/1]]，本命题只覆盖其中一种情形。";
    let duplicate = format!("Intro.\n\n{paragraph}\n\n{paragraph}");
    let duplicate_base = "run-adopt/group-1/2/review/duplicates";
    edit(&mut mcp, duplicate_base, ownership, &duplicate);
    let (conflict, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/2/link/correct",
        duplicate_base,
        replacement,
        Some(ownership),
        false,
    );
    assert!(failed);
    assert_eq!(conflict["code"], "write_conflict");
    assert_eq!(conflict["observed_body"], duplicate);
    let (_, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/2/link/adopt",
        duplicate_base,
        paragraph,
        None,
        true,
    );
    assert!(failed);
    assert_eq!(mcp.ok("get_card", json!({"at":"0/2"}))["body"], duplicate);
    let fenced = format!("Intro.\n\n```text\n{paragraph}\n```");
    let fenced_base = "run-adopt/group-1/3/review/fenced";
    edit(&mut mcp, fenced_base, duplicate_base, &fenced);
    let (conflict, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/3/link/correct",
        fenced_base,
        replacement,
        Some(ownership),
        false,
    );
    assert!(failed);
    assert!(
        conflict["error"]
            .as_str()
            .unwrap()
            .contains("absent or changed")
    );
    assert_eq!(mcp.ok("get_card", json!({"at":"0/2"}))["body"], fenced);
    let changed = format!("Intro.\n\n{paragraph} Changed by another writer.");
    let changed_base = "run-adopt/group-1/4/review/changed";
    edit(&mut mcp, changed_base, fenced_base, &changed);
    let (_, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/4/link/correct",
        changed_base,
        replacement,
        Some(ownership),
        false,
    );
    assert!(failed);
    assert_eq!(mcp.ok("get_card", json!({"at":"0/2"}))["body"], changed);
    let legacy = format!("Intro.\n\n{replacement}");
    let legacy_base = "run-adopt/group-1/5/review/legacy";
    edit(&mut mcp, legacy_base, changed_base, &legacy);
    let revision =
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"].clone();
    let (adopted, failed) = owned(
        &mut mcp,
        "run-adopt/group-1/5/link/adopt",
        legacy_base,
        replacement,
        Some(ownership),
        true,
    );
    assert!(!failed, "{adopted}");
    assert_eq!(
        adopted["results"][0]["result"]["owned_paragraph"]["adopted"],
        true
    );
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        revision
    );
    assert_eq!(mcp.ok("get_card", json!({"at":"0/2"}))["body"], legacy);
    let current = "run-adopt/group-1/5/link/adopt";
    let mixed = json!({"items":[{"op_key":"run-adopt/group-1/6/link/mixed","client_ref":"source","tool":"edit_card","arguments":{"at":"0/2","base_receipt":current,"body":"Must not replace other content.","owned_paragraph":{"text":paragraph}}}]});
    assert!(mcp.err("apply_batch", mixed).contains("cannot combine"));
    let oversized = json!({"items":[{"op_key":"run-adopt/group-1/6/link/large","client_ref":"source","tool":"edit_card","arguments":{"at":"0/2","base_receipt":current,"owned_paragraph":{"text":"x".repeat(1024*1024+1)}}}]});
    assert!(
        mcp.err("apply_batch", oversized)
            .contains("1048576 authored bytes")
    );
    let first = "run-adopt/group-1/6/link/first";
    let mut invalid = json!({"items":[
        {"op_key":first,"client_ref":"first","tool":"edit_card","arguments":{"at":"0/2","base_receipt":current,"owned_paragraph":{"text":"扩展 [[0/1]]，两者使用不同的条件。"}}},
        {"op_key":"run-adopt/group-1/6/link/second","client_ref":"second","tool":"edit_card","arguments":{"at":"0/2","base_receipt":first,"owned_paragraph":{"text":"依赖 [[MissingBook]]，不允许不存在的目标。"}}}
    ]});
    assert!(
        mcp.err("apply_batch", invalid.clone())
            .contains("does not exist")
    );
    assert_eq!(mcp.ok("get_card", json!({"at":"0/2"}))["body"], legacy);
    invalid["items"][1]["arguments"]["owned_paragraph"]["text"] =
        json!("对照 [[0/1]]，这里展示另一条推理途径。");
    let retried = mcp.ok("apply_batch", invalid);
    assert_eq!(
        retried["results"][0]["replayed"], false,
        "a rolled-back paragraph left no receipt"
    );
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/2"}))["body"],
        format!(
            "{legacy}\n\n扩展 [[0/1]]，两者使用不同的条件。\n\n对照 [[0/1]]，这里展示另一条推理途径。"
        )
    );
    let fenced_input = json!({"items":[{"op_key":"run-adopt/group-1/7/link/fenced-input","client_ref":"source","tool":"edit_card","arguments":{"at":"0/2","base_receipt":"run-adopt/group-1/6/link/second","owned_paragraph":{"text":format!("```text\n{paragraph}\n```")}}}]});
    assert!(
        mcp.err("apply_batch", fenced_input)
            .contains("one paragraph")
    );
}

#[test]
fn an_owned_paragraph_handles_a_bare_heading_and_retains_its_own_line_endings() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-bare/group-1/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Bare root"}},
        {"op_key":"run-bare/group-1/1/create/target","client_ref":"target","tool":"create_successor","arguments":{"at":"@@topic","title":"Constraint"}}
    ]}));
    home.db()
        .execute(
            "UPDATE cards SET text='# Bare root' WHERE location='0/0'",
            [],
        )
        .unwrap();
    let paragraph = "依赖 [[0/1]]，\r\n后者给出共同的约束。";
    mcp.ok("apply_batch",json!({"items":[{"op_key":"run-bare/group-1/1/link/paragraph","client_ref":"topic","tool":"edit_card","arguments":{"at":"0/0","base_receipt":"run-bare/group-1/1/create/topic","owned_paragraph":{"text":paragraph}}}]}));
    let card = mcp.ok("get_card", json!({"at":"0/0"}));
    assert_eq!(card["title"], "Bare root");
    assert_eq!(card["body"], paragraph.replace("\r\n", "\n"));
    assert!(card["text"].as_str().unwrap().contains(paragraph));
    let replacement = "特例 [[0/1]]，本卡讨论较窄的情形。";
    mcp.ok("apply_batch",json!({"items":[{"op_key":"run-bare/group-1/2/link/paragraph","client_ref":"topic","tool":"edit_card","arguments":{"at":"0/0","base_receipt":"run-bare/group-1/1/link/paragraph","owned_paragraph":{"text":replacement,"replaces_receipt":"run-bare/group-1/1/link/paragraph"}}}]}));
    assert_eq!(mcp.ok("get_card", json!({"at":"0/0"}))["body"], replacement);
}

#[test]
fn an_owned_correction_preserves_legacy_authored_bytes_when_reverse_links_change() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-bytes/group-1/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Bytes"}},
        {"op_key":"run-bytes/group-1/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"@@topic","title":"Claim","body":"An independent formula.\n\n$$\nx^2+y^2=z^2\n$$\n\n![Figure](https://example.test/figure.png)"}},
        {"op_key":"run-bytes/group-1/1/create/target","client_ref":"target","tool":"create_successor","arguments":{"at":"@@source","title":"Constraint"}}
    ]}));
    let old = "依赖 [[0/2]]，后者给出共同的约束。";
    mcp.ok("apply_batch",json!({"items":[{"op_key":"run-bytes/group-1/1/link/paragraph","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-bytes/group-1/1/create/source","owned_paragraph":{"text":old}}}]}));
    let card = mcp.ok("get_card", json!({"at":"0/1"}));
    // Prepare a valid retained Markdown fixture with legacy transport/framing.
    // Assertions still observe the real MCP and integrity CLI, not database rows.
    let raw = card["text"]
        .as_str()
        .unwrap()
        .replace('\n', "\r\n")
        .replacen(
            "# Claim\r\n\r\n",
            "\r\n \t\r\n# Claim ###\r\n\r\n \t\r\n",
            1,
        );
    home.db()
        .execute("UPDATE cards SET text=?1 WHERE location='0/1'", [&raw])
        .unwrap();
    let replacement = "特例 [[0/2]]，本命题只覆盖其中一种情形。";
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-bytes/group-1/2/link/paragraph","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-bytes/group-1/1/link/paragraph","owned_paragraph":{"text":replacement,"replaces_receipt":"run-bytes/group-1/1/link/paragraph"}}},
        {"op_key":"run-bytes/group-1/2/link/carrier","client_ref":"carrier","tool":"create_successor","arguments":{"at":"0/2","title":"Inbound carrier","body":"依赖 [[0/1]]，这里给出另一条作者链接。"}}
    ]}));
    let after = mcp.ok("get_card", json!({"at":"0/1"}));
    let marker = "<!-- zt:reverse-links -->";
    assert_eq!(
        after["text"]
            .as_str()
            .unwrap()
            .split(marker)
            .next()
            .unwrap(),
        raw.split(marker).next().unwrap().replace(old, replacement)
    );
    assert_eq!(after["inbound"][0]["address"], "0/3");
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn receipt_owned_paragraph_is_corrected_without_replacing_other_authored_content() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let body = "An independent observation.\n\n$$\nx^2+y^2=z^2\n$$\n\n![Figure](https://example.test/figure.png)";
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-paragraph/group-1/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Paragraphs"}},
        {"op_key":"run-paragraph/group-1/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"@@topic","title":"Independent claim","body":body}},
        {"op_key":"run-paragraph/group-1/1/create/target","client_ref":"target","tool":"create_successor","arguments":{"at":"@@source","title":"A related constraint"}}
    ]}));
    let paragraph = "依赖 [[0/2]]，后者给出共同的约束。";
    let appended=mcp.ok("apply_batch",json!({"items":[{"op_key":"run-paragraph/group-1/1/link/paragraph","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-paragraph/group-1/1/create/source","owned_paragraph":{"text":paragraph}}}]}));
    assert_eq!(
        appended["results"][0]["result"]["owned_paragraph"]["text"],
        paragraph
    );
    let replay=mcp.ok("apply_batch",json!({"items":[{"op_key":"run-paragraph/group-1/1/link/paragraph","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-paragraph/group-1/1/create/source","owned_paragraph":{"text":paragraph,"replaces_receipt":null,"adopt_legacy":false}}}]}));
    assert_eq!(replay["results"][0]["replayed"], true);
    let before = mcp.ok("get_card", json!({"at":"0/1"}));
    assert_eq!(before["body"], format!("{body}\n\n{paragraph}"));
    let replacement = "特例 [[0/2]]，本命题只覆盖其中一种情形。";
    let correction = json!({"items":[{"op_key":"run-paragraph/group-1/2/link/paragraph","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-paragraph/group-1/1/link/paragraph","owned_paragraph":{"text":replacement,"replaces_receipt":"run-paragraph/group-1/1/link/paragraph"}}}]});
    mcp.ok("apply_batch", correction.clone());
    let after = mcp.ok("get_card", json!({"at":"0/1"}));
    assert_eq!(
        after["text"],
        before["text"]
            .as_str()
            .unwrap()
            .replace(paragraph, replacement)
    );
    let revision =
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"].clone();
    assert_eq!(
        mcp.ok("apply_batch", correction)["results"][0]["replayed"],
        true
    );
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        revision
    );
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/1"}))["body"],
        after["body"]
    );
}

#[test]
fn identified_authored_bases_ignore_reverse_links_but_detect_metadata_changes() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-base/card-1/1/create/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Bases"}},
        {"op_key":"run-base/card-1/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"@@topic","title":"Source","body":"Original body."}},
        {"op_key":"run-base/card-1/1/create/target","client_ref":"target","tool":"create_successor","arguments":{"at":"@@source","title":"Other claim"}}
    ]}));
    mcp.ok(
        "edit_card",
        json!({"at":"0/2","body":"依赖 [[0/1]]，这里给出独立的理由。"}),
    );
    let (wrong,is_error)=mcp.call("apply_batch",json!({"items":[{"op_key":"run-base/card-1/2/edit/wrong","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","body":"Wrong receipt must refuse.","base_receipt":"run-base/card-1/1/create/target"}}]}));
    assert!(
        is_error,
        "a base receipt for another Card must not authorize this edit"
    );
    assert_eq!(wrong["code"], "write_conflict");
    let updated=mcp.ok("apply_batch",json!({"items":[{"op_key":"run-base/card-1/2/edit/body","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","body":"Accepted after inbound change.","base_receipt":"run-base/card-1/1/create/source"}}]}));
    let base = updated["results"][0]["result"]["after_fingerprint"].clone();
    mcp.ok("edit_card", json!({"at":"0/1","type":"event"}));
    let (conflict,is_error)=mcp.call("apply_batch",json!({"items":[{"op_key":"run-base/card-1/3/edit/stale","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","body":"Must refuse.","base_fingerprint":base}}]}));
    assert!(
        is_error,
        "a stale authored metadata base must not overwrite the Card"
    );
    assert_eq!(conflict["code"], "write_conflict");
    assert_eq!(conflict["observed_body"], "Accepted after inbound change.");
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/1"}))["body"],
        "Accepted after inbound change."
    );
    assert!(
        mcp.err(
            "edit_card",
            json!({"at":"0/1","body":"Must also refuse.","base_fingerprint":base})
        )
        .contains("only available within apply_batch")
    );
    assert!(mcp.err("apply_batch",json!({"items":[{"op_key":"run-base/card-1/4/edit/unguarded","client_ref":"source","tool":"edit_card","arguments":{"at":"0/1","body":"An unguarded overwrite must refuse."}}]})).contains("requires an authored base"));
}

#[test]
fn a_maximum_batch_publishes_one_revision_and_one_combined_link_refresh() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let before = mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"]
        .as_u64()
        .unwrap();
    let items:Vec<Value>=(0..64).map(|i|json!({"op_key":format!("run-maximum/group-1/1/build/item-{i}"),"client_ref":format!("item-{i}"),"tool":"create_topic","arguments":{"title":format!("Topic {i}")}})).collect();
    mcp.ok("apply_batch", json!({"items":items}));
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        before + 1
    );
    let output = home
        .cmd()
        .args(["status", "--check-links"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["last_write"]["indexed_cards"], 64);
    assert_eq!(report["last_write"]["refreshed_cards"], 64);
}
#[test]
fn batch_identity_and_access_failures_leave_no_partial_writes() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let valid = json!({"op_key":"run-identity/group-1/1/apply/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Guarded"}});
    for key in ["ad-hoc", "run/group/1/apply/item/attempt-2"] {
        let mut item = valid.clone();
        item["op_key"] = json!(key);
        assert!(
            mcp.err("apply_batch", json!({"items":[item]}))
                .contains("op_key must have")
        );
    }
    let mut bad_ref = valid.clone();
    bad_ref["client_ref"] = json!("bad/ref");
    assert!(
        mcp.err("apply_batch", json!({"items":[bad_ref]}))
            .contains("client_ref")
    );
    let mut human = valid.clone();
    human["arguments"]["workspace"] = json!("human");
    assert!(
        mcp.err("apply_batch", json!({"items":[human]}))
            .contains("read-only")
    );
    let destructive = json!({"op_key":"run-identity/group-1/1/apply/delete","client_ref":"delete","tool":"delete_card","arguments":{"at":"@@topic","confirm":"delete"}});
    assert!(
        mcp.err("apply_batch", json!({"items":[valid,destructive]}))
            .contains("unsupported batch tool")
    );
    assert_eq!(mcp.ok("list_trees", json!({}))["trees"], json!([]));
}

#[test]
fn a_receipt_binds_local_references_to_their_original_allocated_addresses() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let topic = json!({"op_key":"run-bind/group-1/1/build/topic-a","client_ref":"root","tool":"create_topic","arguments":{"title":"Original root"}});
    let claim = json!({"op_key":"run-bind/group-1/1/build/claim","client_ref":"claim","tool":"create_successor","arguments":{"at":"@@root","title":"Claim"}});
    mcp.ok(
        "apply_batch",
        json!({"items":[topic.clone(),claim.clone()]}),
    );
    let replacement = json!({"op_key":"run-bind/group-1/1/build/topic-b","client_ref":"root","tool":"create_topic","arguments":{"title":"Different root"}});
    assert!(
        mcp.err("apply_batch", json!({"items":[replacement,claim.clone()]}))
            .contains("operation key conflict")
    );
    assert_eq!(
        mcp.ok("list_trees", json!({}))["trees"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let next = json!({"op_key":"run-bind/group-1/1/build/next","client_ref":"next","tool":"create_successor","arguments":{"at":"@@claim","title":"Next claim","body":"依赖 [[@@claim]]。"}});
    let extended = mcp.ok("apply_batch", json!({"items":[topic,claim,next]}));
    assert_eq!(extended["results"][0]["replayed"], true);
    assert_eq!(extended["results"][1]["replayed"], true);
    assert_eq!(extended["results"][2]["result"]["address"], "0/2");
}

#[test]
fn batch_edits_use_the_existing_base_guard_and_keep_authored_fingerprints() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Edits"}));
    mcp.ok("apply_batch",json!({"items":[{"op_key":"run-edit/card-1/1/create/source","client_ref":"source","tool":"create_successor","arguments":{"at":"0/0","title":"Original title","body":"Old body."}}]}));
    let before = mcp.ok("get_card", json!({"at":"0/1"}));
    let batch = json!({"items":[{"op_key":"run-edit/card-1/1/apply/body","client_ref":"card","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-edit/card-1/1/create/source","body":"A real event happened in 2026.","type":"event"}}]});
    let receipt = mcp.ok("apply_batch", batch.clone());
    let result = &receipt["results"][0]["result"];
    assert!(result["before_fingerprint"].as_str().is_some());
    assert_ne!(result["before_fingerprint"], result["after_fingerprint"]);
    let card = mcp.ok("get_card", json!({"at":"0/1"}));
    assert_eq!(card["body"], "A real event happened in 2026.");
    assert_eq!(card["title"], "Original title");
    assert_eq!(card["type"], "event");
    assert_eq!(
        mcp.ok("apply_batch", batch)["results"][0]["result"],
        *result
    );
    let stale = json!({"items":[{"op_key":"run-edit/card-1/2/apply/body","client_ref":"card","tool":"edit_card","arguments":{"at":"0/1","base_receipt":"run-edit/card-1/1/create/source","body":"Stale overwrite."}}]});
    assert!(mcp.err("apply_batch", stale).contains("authored base changed"));
    assert!(mcp.err("edit_card",json!({"at":"0/1","base":before["text"],"body":"Raw stale overwrite."})).contains("changed since"));
    assert_eq!(
        mcp.ok("get_card", json!({"at":"0/1"}))["body"],
        card["body"]
    );
}

#[test]
fn batch_successor_creation_preserves_model_level_and_math_form() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let result=mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-model/model-1/1/build/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Models"}},
        {"op_key":"run-model/model-1/1/build/model","client_ref":"model","tool":"create_successor","arguments":{"at":"@@topic","title":"Theorem","type":"model","level":"formal","form":"theorem"}}
    ]}));
    let address = result["results"][1]["result"]["address"].clone();
    let model = mcp.ok("get_card", json!({"at":address}));
    assert_eq!(model["level"], "formal");
    assert_eq!(model["form"], "theorem");
}

#[test]
fn a_batch_creates_a_literature_tree_and_resolves_its_local_reference() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let result=mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-book/group-1/1/apply/book","client_ref":"book","tool":"create_literature","arguments":{"bibtex":"@book{BatchBook,title={Atomic book},year={2026}}","body":"Book introduction."}},
        {"op_key":"run-book/group-1/1/apply/claim","client_ref":"claim","tool":"create_successor","arguments":{"at":"@@book","title":"Claim","body":"来源 [[@@book]]。"}}
    ]}));
    assert_eq!(result["results"][0]["result"]["address"], "BatchBook");
    assert_eq!(result["results"][1]["result"]["address"], "BatchBook/1");
    let book = mcp.ok("get_card", json!({"at":"BatchBook"}));
    assert_eq!(book["title"], "Atomic book");
    assert_eq!(book["body"], "Book introduction.");
    assert_eq!(book["inbound"][0]["address"], "BatchBook/1");
}

#[test]
fn batch_limits_refuse_oversized_requests_before_any_card_is_created() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let items:Vec<Value>=(0..65).map(|i|json!({"op_key":format!("run-limits/group-1/1/apply/item-{i}"),"client_ref":format!("item-{i}"),"tool":"create_topic","arguments":{"title":"Bounded"}})).collect();
    assert!(
        mcp.err("apply_batch", json!({"items":items}))
            .contains("64 items")
    );
    assert!(mcp.err("apply_batch",json!({"items":[{"op_key":"run-limits/group-1/1/apply/large","client_ref":"large","tool":"create_topic","arguments":{"title":"Bounded","body":"x".repeat(1024*1024)}}]})).contains("1048576 authored bytes"));
    assert_eq!(mcp.ok("list_trees", json!({}))["trees"], json!([]));
}

#[test]
fn batch_receipts_canonicalize_defaults_object_order_and_transport_line_endings() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let first=mcp.ok("apply_batch",json!({"items":[
        {"op_key":"run-canonical/group-1/1/apply/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Canonical Topic","body":"First line.\r\n\r\nSecond line."}},
        {"op_key":"run-canonical/group-1/1/apply/claim","client_ref":"claim","tool":"create_successor","arguments":{"at":"@@topic","title":"Claim"}}
    ]}));
    let again=mcp.ok("apply_batch",json!({"items":[
        {"tool":"create_topic","arguments":{"workspace":"agent","body":"First line.\n\nSecond line.","title":"Canonical Topic"},"client_ref":"topic","op_key":"run-canonical/group-1/1/apply/topic"},
        {"arguments":{"kind":"","type":"idea","facet":null,"emphasis":null,"level":null,"form":null,"body":"","title":"Claim","at":"@@topic"},"tool":"create_successor","client_ref":"claim","op_key":"run-canonical/group-1/1/apply/claim"}
    ]}));
    for i in 0..2 {
        assert_eq!(again["results"][i]["result"], first["results"][i]["result"]);
        assert_eq!(again["results"][i]["replayed"], true);
    }
}

#[test]
fn a_committed_batch_replays_original_addresses_after_the_client_loses_its_answer() {
    let home = TestHome::new();
    home.configure_and_up();
    let batch = json!({"items":[
        {"op_key":"run-replay/group-1/1/apply/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Replay Topic"}},
        {"op_key":"run-replay/group-1/1/apply/claim","client_ref":"claim","tool":"create_successor","arguments":{"at":"@@topic","title":"Claim","body":"来源 [[@@topic]]。"}}
    ]});
    let mut mcp = home.mcp();
    let first = mcp.ok("apply_batch", batch.clone());
    let revision =
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"].clone();
    drop(mcp); // A retry has no process-local address map or result.
    let mut retry = home.mcp();
    let replay = retry.ok("apply_batch", batch.clone());
    for i in 0..2 {
        assert_eq!(
            replay["results"][i]["result"],
            first["results"][i]["result"]
        );
        assert_eq!(replay["results"][i]["replayed"], true);
    }
    assert_eq!(
        retry.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        revision
    );
    assert_eq!(
        retry.ok("list_trees", json!({}))["trees"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let mut conflict = batch;
    conflict["items"][0]["arguments"]["title"] = json!("Changed accepted plan");
    assert!(
        retry
            .err("apply_batch", conflict)
            .contains("operation key conflict")
    );
    assert_eq!(
        retry.ok("get_card", json!({"at":"0/0"}))["title"],
        "Replay Topic"
    );
}

#[test]
fn a_failed_batch_rolls_back_earlier_creates_and_topic_allocation() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let before = mcp.ok("get_cards", json!({"addresses":[],"view":"text"}));
    let error=mcp.err("apply_batch",json!({"items":[
        {"op_key":"run-atomic/group-1/1/apply/topic","client_ref":"topic","tool":"create_topic","arguments":{"title":"Rolled back"}},
        {"op_key":"run-atomic/group-1/1/apply/claim","client_ref":"claim","tool":"create_successor","arguments":{"at":"@@topic","title":"Invalid final claim","body":"参照 [[MissingBook]]。"}}
    ]}));
    assert!(
        error.contains("link target `MissingBook` does not exist"),
        "{error}"
    );
    assert_eq!(mcp.ok("list_trees", json!({}))["trees"], json!([]));
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["archive_revision"],
        before["archive_revision"]
    );
    assert_eq!(
        mcp.ok("create_topic", json!({"title":"First committed Topic"}))["address"],
        "0/0"
    );
}

#[test]
fn bulk_text_has_the_summary_facts_needed_by_scoped_digest_checks() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Facts"}));
    mcp.ok("create_successor", json!({"at":"0/0","title":"Premise"}));
    mcp.ok("create_successor", json!({"at":"0/1","title":"Later removed"}));
    mcp.ok("create_successor",json!({"at":"0/1","kind":"side","title":"Claim","body":"## 说明\n\n依赖 [[0/1]]，也参照 [[0/1]]。另见 [[0/2]] 和 [[0/2]]。\n\n```text\n[[MissingBook]]\n```"}));
    let deletion = mcp.ok("delete_card", json!({"at":"0/2"}));
    let deleted=mcp.ok("delete_card", json!({"at":"0/2","confirm":deletion["confirm"]}));
    assert_eq!(deleted["applied"],true);
    let full = mcp.ok("get_card", json!({"at":"0/1|a"}));
    assert!(full["outbound"].as_array().unwrap().iter().any(|link|link["broken"]==true));
    let tree = mcp.ok("get_tree", json!({"tree":"0"}));
    let summary = tree["cards"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["address"] == "0/1|a")
        .unwrap();
    let text =
        mcp.ok("get_cards", json!({"addresses":["0/1|a"],"view":"text"}))["cards"][0].clone();
    for key in ["excerpt", "links", "kinds", "edge"] {
        assert_eq!(
            text[key], summary[key],
            "bulk text preserves the existing summary fact {key}"
        );
    }
    assert_eq!(text["parent_address"], full["parent"]["address"]);
    assert_eq!(text["outbound_count"], full["outbound"].as_array().unwrap().len());
    assert_eq!(text["outbound_count"], 2);
    assert!(text.get("model").is_none() && text.get("hubs").is_none());
}

#[test]
fn a_model_neighbours_own_unrelated_model_does_not_expand_the_read_scope() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    for title in ["Requested model", "Neighbour model", "Unrelated model"] {
        mcp.ok("create_topic", json!({"title":title}));
    }
    for n in 0..3 {
        mcp.ok(
            "create_successor",
            json!({"at":format!("{n}/0"),"title":format!("Model {n}"),"type":"model"}),
        );
    }
    mcp.ok(
        "create_successor",
        json!({"at":"0/1","kind":"side","title":"Required part","facet":"设定"}),
    );
    mcp.ok(
        "edit_card",
        json!({"at":"0/1","body":"参照 [[1/1]]，比较相关模型。"}),
    );
    mcp.ok(
        "edit_card",
        json!({"at":"1/1","body":"参照 [[2/1]]，比较另一个模型。"}),
    );
    let expected = mcp.ok("get_model", json!({"at":"0/1"}));
    home.db()
        .execute(
            "UPDATE cards SET text='malformed unrelated Markdown' WHERE location='2/1'",
            [],
        )
        .unwrap();
    assert_eq!(mcp.ok("get_model", json!({"at":"0/1"})), expected);
}

#[test]
fn bulk_text_and_revision_remain_one_read_view_during_another_process_edits() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut reader = home.mcp();
    reader.ok("create_topic", json!({"title":"Concurrent read"}));
    reader.ok(
        "create_successor",
        json!({"at":"0/0","title":"Versioned claim","body":"Version 0."}),
    );
    let base =
        reader.ok("get_cards", json!({"addresses":["0/1"],"view":"text"}))["archive_revision"]
            .as_u64()
            .unwrap();
    let mut writer = home.mcp();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let writer_barrier = barrier.clone();
    let worker = thread::spawn(move || {
        writer_barrier.wait();
        for version in 1..=20 {
            writer.ok(
                "edit_card",
                json!({"at":"0/1","body":format!("Version {version}.")}),
            );
        }
    });
    barrier.wait();
    for _ in 0..12 {
        let view = reader.ok(
            "get_cards",
            json!({"addresses":vec!["0/1";100],"view":"text"}),
        );
        let version = view["archive_revision"].as_u64().unwrap() - base;
        for card in view["cards"].as_array().unwrap() {
            assert_eq!(
                card["body"],
                format!("Version {version}."),
                "Card body and revision must belong to the same committed read view"
            );
        }
    }
    worker.join().unwrap();
    assert_eq!(
        reader.ok("get_cards", json!({"addresses":["0/1"],"view":"text"}))["archive_revision"],
        base + 20
    );
}

#[test]
fn hub_policy_uses_indexed_links_and_metadata_without_parsing_unrelated_bodies() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Hub subject"}));
    mcp.ok("create_successor", json!({"at":"0/0","title":"Anchor"}));
    mcp.ok(
        "create_successor",
        json!({"at":"0/1","title":"Second claim"}),
    );
    mcp.ok("create_successor",json!({"at":"0/2","title":"Candidate","body":"参照 [[0/1]] 和 [[0/2]]，说明两个成员的联系。"}));
    mcp.ok("create_successor",json!({"at":"0/1","kind":"side","type":"hub","level":"topic","title":"Subject hub","body":"## 问题\n- [[0/1]] 给出问题的起点和主要边界\n\n## 联系\n- [[0/2]] 解释第二个命题与原问题的联系"}));
    mcp.ok("create_topic", json!({"title":"Neighbour"}));
    mcp.ok("create_successor",json!({"at":"1/0","title":"External discussion","body":"参照 [[0/1]] 和 [[0/2]]，提出邻近主题的问题。"}));
    let calls = [
        ("get_hub", json!({"at":"0/1|a","neighbour_links":1})),
        (
            "list_hubs",
            json!({"workspace":"agent","topic_min":2,"line_min":2,"neighbour_links":1}),
        ),
    ];
    let expected = calls
        .iter()
        .map(|(name, args)| mcp.ok(name, args.clone()))
        .collect::<Vec<_>>();
    assert_eq!(expected[0]["candidates"][0]["address"], "0/3");
    home.db().execute("UPDATE cards SET text='# External discussion\n\nmalformed [[unclosed' WHERE location='1/1'",[]).unwrap();
    for ((name, args), value) in calls.into_iter().zip(expected) {
        assert_eq!(
            mcp.ok(name, args),
            value,
            "full Snapshot hub policy parity for {name}"
        );
    }
}

#[test]
fn indexed_lists_preserve_counts_filters_and_repeated_broken_occurrences() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Indexed lists"}));
    mcp.ok("create_successor", json!({"at":"1/0","title":"Anchor"}));
    mcp.ok(
        "create_successor",
        json!({"at":"1/1","kind":"side","title":"Temporary target"}),
    );
    mcp.ok("create_successor",json!({"at":"1/1","title":"Finding","type":"finding","body":"参照 [[1/1|a]] 和 [[1/1|a]]，并与 [[0/1]] 对照。"}));
    mcp.ok("delete_card", json!({"at":"1/1|a","confirm":"delete"}));
    let lists = [
        ("list_workspaces", json!({})),
        ("list_trees", json!({})),
        ("list_links", json!({})),
        (
            "list_links",
            json!({"workspace":"human","cross_workspace_only":true}),
        ),
        ("list_broken_links", json!({"workspace":"agent"})),
    ];
    let expected = lists
        .iter()
        .map(|(name, args)| mcp.ok(name, args.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        expected[2]["links"].as_array().unwrap().len(),
        2,
        "list_links keeps its distinct-target contract"
    );
    assert_eq!(
        expected[4]["broken"].as_array().unwrap().len(),
        2,
        "broken reporting preserves occurrences"
    );
    assert_eq!(expected[0]["broken_links"], 2);
    // Persistence fault outside the sources whose body lines the list needs.
    home.db()
        .execute(
            "UPDATE cards SET text='# Anchor\n\nmalformed [[unclosed' WHERE location='1/1'",
            [],
        )
        .unwrap();
    for ((name, args), expected) in lists.into_iter().zip(expected) {
        assert_eq!(
            mcp.ok(name, args),
            expected,
            "indexed {name} matches the full Snapshot result"
        );
    }
}

#[test]
fn tree_bodies_and_title_export_use_exact_selection_order_and_revision() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();
    mcp.ok(
        "create_literature",
        json!({"bibtex":"@book{Case_Book,title={Selected book}}","body":"Root notes."}),
    );
    mcp.ok(
        "create_literature",
        json!({"bibtex":"@book{CaseXBook,title={Other book}}"}),
    );
    let mut previous = "Case_Book".to_string();
    for n in 1..=12 {
        let made = mcp.ok(
            "create_successor",
            json!({"at":previous,"title":format!("Claim {n}"),"body":format!("Body {n}.")}),
        );
        previous = made["address"].as_str().unwrap().to_string();
    }
    let tree = mcp.ok("get_tree", json!({"tree":"Case_Book/2","bodies":true}));
    let cards = tree["cards"].as_array().unwrap();
    assert_eq!(cards.len(), 13);
    assert_eq!(cards[0]["address"], "Case_Book");
    assert_eq!(cards[0]["bibtex"], "@book{Case_Book,title={Selected book}}");
    for n in 1..=12 {
        assert_eq!(cards[n]["address"], format!("Case_Book/{n}"));
        assert_eq!(cards[n]["body"], format!("Body {n}."));
        assert!(cards[n]["text"].is_string());
    }
    let titles = mcp.ok(
        "export_titles",
        json!({"workspace":"agent","zone":"literature"}),
    );
    assert_eq!(titles["count"], 14);
    assert_eq!(titles["archive_revision"], tree["archive_revision"]);
    assert!(titles["archive_revision"].is_u64());
    for row in titles["titles"].as_array().unwrap() {
        assert_eq!(row["workspace"], "agent");
        assert!(row["title"].is_string());
        assert!(row.get("body").is_none() && row.get("text").is_none());
    }
    assert_eq!(
        mcp.ok("export_titles", json!({"workspace":"human","zone":"topic"}))["count"],
        2
    );
    assert!(
        mcp.err("get_tree", json!({"tree":"case_Book"}))
            .contains("does not exist")
    );
    let summary = mcp.ok("get_tree", json!({"tree":"Case_Book"}));
    assert!(summary["cards"][0].get("body").is_none());
    let sentinel = mcp.ok("create_topic", json!({"title":"Unrelated sentinel"}))["address"]
        .as_str()
        .unwrap()
        .to_string();
    home.db()
        .execute(
            "UPDATE cards SET text='malformed unrelated Markdown' WHERE location=?1",
            [sentinel],
        )
        .unwrap();
    assert_eq!(
        mcp.ok("get_tree", json!({"tree":"Case_Book","bodies":true}))["cards"],
        tree["cards"]
    );
    assert_eq!(
        mcp.ok(
            "export_titles",
            json!({"workspace":"agent","zone":"literature"})
        )["titles"],
        titles["titles"]
    );
}

#[test]
fn bulk_reads_reject_oversize_encoded_replies_and_the_server_remains_usable() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok(
        "create_topic",
        json!({"title":"Large text","body":"x".repeat(2_200_000)}),
    );
    let (value, error) = mcp.call("get_cards", json!({"addresses":["0/0"],"view":"text"}));
    assert!(error, "the complete MCP reply must fit in 8 MiB");
    assert_eq!(value["code"], "read_limit");
    assert_eq!(value["limit_kind"], "reply_bytes");
    assert_eq!(value["limit"], 8 * 1024 * 1024);
    assert!(value["actual"].as_u64().unwrap() > 8 * 1024 * 1024);
    assert_eq!(value["complete"], false);
    assert!(value.get("cards").is_none());
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[]}))["cards"],
        json!([])
    );
    let (tree, error) = mcp.call("get_tree", json!({"tree":"0","bodies":true}));
    assert!(error);
    assert_eq!(tree["code"], "read_limit");
    assert_eq!(tree["complete"], false);
    mcp.ok(
        "edit_card",
        json!({"at":"0/0","title":"t".repeat(4_300_000)}),
    );
    let (titles, error) = mcp.call("export_titles", json!({}));
    assert!(error);
    assert_eq!(titles["code"], "read_limit");
    assert_eq!(titles["complete"], false);
}

#[test]
fn bulk_read_limits_fail_explicitly_instead_of_returning_a_truncated_success() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Bounded"}));
    let (value, error) = mcp.call(
        "get_cards",
        json!({"addresses":vec!["0/0";201],"view":"text"}),
    );
    assert!(error, "a too-large request must fail");
    assert_eq!(value["code"], "read_limit");
    assert_eq!(value["limit_kind"], "addresses");
    assert_eq!(value["limit"], 200);
    assert_eq!(value["actual"], 201);
    assert_eq!(value["complete"], false);
    assert!(value.get("cards").is_none());
    assert!(
        mcp.err("get_cards", json!({"addresses":["0/0"],"view":42}))
            .contains("view")
    );
    assert!(
        mcp.err("get_cards", json!({"addresses":[42],"view":"text"}))
            .contains("string")
    );
}

#[test]
fn indexed_model_context_matches_the_gui_full_snapshot_for_shared_math_and_human_links() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Result"}));
    mcp.ok("create_successor",json!({"at":"0/0","title":"Main theorem","type":"model","level":"formal","form":"theorem","body":"(H1) A hypothesis. (C1) A conclusion."}));
    mcp.ok("create_successor",json!({"at":"0/1","kind":"side","title":"Main setting","facet":"前提","body":"A defined setting."}));
    mcp.ok("create_successor",json!({"at":"0/1|a","title":"Main proof","facet":"证明：关键","emphasis":"core","body":"依赖 [[0/1|a]]，说明这一步。"}));
    mcp.ok(
        "create_literature",
        json!({"bibtex":"@book{Shared_Book,title={Shared book}}"}),
    );
    mcp.ok("create_successor",json!({"at":"Shared_Book","title":"Shared lemma","type":"model","form":"lemma","level":"formal"}));
    mcp.ok(
        "create_successor",
        json!({"at":"Shared_Book/1","kind":"side","title":"Shared setting","facet":"前提"}),
    );
    mcp.ok("create_successor",json!({"at":"Shared_Book/1|a","title":"Shared derivation","facet":"推导：公共","body":"依赖 [[Shared_Book/1|a]]，完成共同推导。"}));
    mcp.ok("edit_card",json!({"at":"0/1","body":"(H1) A hypothesis. (C1) A conclusion.\n\n结构：[[0/1|a]]、[[0/1|a|1]]、[[Shared_Book/1|a|1]]。"}));
    mcp.ok("edit_card",json!({"at":"Shared_Book/1","body":"结构：[[Shared_Book/1|a]]、[[Shared_Book/1|a|1]]、[[0/1|a|1]]。"}));
    mcp.ok("create_topic", json!({"title":"Premise"}));
    mcp.ok("create_successor",json!({"at":"1/0","title":"Premise definition","type":"model","form":"definition","level":"concept","body":"(C1) A defined object."}));
    mcp.ok(
        "create_successor",
        json!({"at":"0/1","title":"First plain user","body":"依赖 [[1/1]]，用于第一个外部问题。"}),
    );
    mcp.ok(
        "create_successor",
        json!({"at":"0/2","title":"Second plain user","body":"依赖 [[1/1]]，用于第二个外部问题。"}),
    );
    mcp.ok(
        "create_successor",
        json!({"at":"0/3","title":"User of the first user","body":"依赖 [[0/2]]，继续外部讨论。"}),
    );
    mcp.ok("edit_card",json!({"at":"0/1|a|1","body":"依赖 [[0/1|a]]、[[Shared_Book/1|a|1]]、[[1/1]] 和 [[Shared_Book/1|a]]，推出结论。"}));
    mcp.ok("create_topic", json!({"title":"Theory"}));
    mcp.ok("create_successor",json!({"at":"2/0","title":"Theory overview","type":"model","form":"theory","body":"## 主要结果\n- [[0/1]]\n- [[Shared_Book/1]]\n\n## 对象\n- [[1/1]]"}));
    mcp.ok("create_successor",json!({"at":"0/1","kind":"side","title":"Reader hub","type":"hub","level":"line","body":"## 构建\n- [[0/1|a|1]] 说明主结果关键证明与前提的关系\n- [[Shared_Book/1|a|1]] 提供可复用的公共推导步骤"}));
    home.cmd_with_editor("# Human discussion\n\n参照 [[0/1|a|1]]，由读者提出讨论。\n")
        .args(["t", "Human discussion"])
        .assert()
        .success();
    let addresses = [
        "0/1",
        "0/1|a",
        "0/1|a|1",
        "Shared_Book/1|a|1",
        "1/1",
        "2/1",
        "0/1|b",
    ];
    let mut models = std::collections::BTreeMap::new();
    let mut full_cards = Vec::new();
    for at in addresses {
        let oracle = home.gui_card(at);
        let actual = mcp.ok("get_card", json!({"at":at}));
        for (field, value) in actual.as_object().unwrap() {
            if field != "writable" {
                assert_eq!(
                    value, &oracle[field],
                    "full Snapshot field {field} for {at}"
                );
            }
        }
        full_cards.push(actual);
        if oracle["model"].is_object() {
            let mut model = oracle["model"].clone();
            let head = model["head"]["address"].as_str().unwrap().to_string();
            model["head"]["body"] = home.gui_card(&head)["body"].clone();
            for part in model["parts"].as_array_mut().unwrap() {
                part["body"] = home.gui_card(part["address"].as_str().unwrap())["body"].clone();
            }
            assert_eq!(
                mcp.ok("get_model", json!({"at":at})),
                model,
                "full Snapshot model for {at}"
            );
            models.insert(at.to_string(), model);
        }
    }
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":addresses}))["cards"],
        json!(full_cards),
        "a bulk union keeps every full Card projection equivalent"
    );
    mcp.ok("create_topic", json!({"title":"Unrelated sentinel"}));
    home.db()
        .execute(
            "UPDATE cards SET text='malformed unrelated Markdown' WHERE location='4/0'",
            [],
        )
        .unwrap();
    for (at, expected) in models {
        assert_eq!(mcp.ok("get_model", json!({"at":at})), expected);
    }
}

#[test]
fn full_card_reads_use_local_context_and_default_bulk_matches_single_card() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Local tree"}));
    mcp.ok(
        "create_successor",
        json!({"at":"0/0","title":"Local source","body":"参照 [[0/0]]，有明确理由。"}),
    );
    mcp.ok("create_topic", json!({"title":"Unrelated"}));
    let before = mcp.ok("get_card", json!({"at":"0/1"}));
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":["0/1"]}))["cards"][0],
        before
    );
    // A malformed unrelated Card must not make a local read parse the archive.
    home.db()
        .execute(
            "UPDATE cards SET text='bad unrelated stored Markdown' WHERE location='1/0'",
            [],
        )
        .unwrap();
    assert_eq!(mcp.ok("get_card", json!({"at":"0/1"})), before);
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":["0/1"]}))["cards"][0],
        before
    );
    assert!(mcp.err("get_card", json!({"at":"1/0"})).contains("heading"));
}

#[test]
fn bulk_text_reads_preserve_order_missing_addresses_and_one_archive_revision() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();
    mcp.ok(
        "create_literature",
        json!({"bibtex":"@book{Bulk_Book,title={Bulk book}}","body":"Book notes."}),
    );
    let expected = mcp.ok("get_card", json!({"at":"Bulk_Book"}));
    let response = mcp.ok(
        "get_cards",
        json!({"addresses":["Bulk_Book","Absent","0/1","Bulk_Book"],"view":"text"}),
    );
    assert_eq!(response["complete"], false);
    assert_eq!(response["missing"], json!(["Absent"]));
    assert!(response["archive_revision"].is_u64());
    let cards = response["cards"].as_array().unwrap();
    assert_eq!(
        cards
            .iter()
            .map(|c| c["address"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Bulk_Book", "0/1", "Bulk_Book"]
    );
    for key in [
        "address",
        "workspace",
        "type",
        "facet",
        "level",
        "form",
        "title",
        "text",
        "body",
        "bibtex",
    ] {
        assert_eq!(cards[0][key], expected[key], "text projection field {key}");
    }
    assert!(cards[0].get("model").is_none());
    assert_eq!(cards[1]["workspace"], "human");
    assert_eq!(
        mcp.ok("get_cards", json!({"addresses":[],"view":"text"}))["complete"],
        true
    );
}

#[test]
fn saving_unchanged_shell_text_and_literature_metadata_keeps_the_revision() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok(
        "create_topic",
        json!({"title":"Unchanged","body":"Keep this claim."}),
    );
    mcp.ok(
        "create_literature",
        json!({"bibtex":"@book{SameBook,title={Same book}}","body":"Keep these notes."}),
    );
    let revision = || -> Value {
        let out = home
            .cmd()
            .args(["status", "--check-links"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<Value>(&out).unwrap()["archive_revision"].clone()
    };
    let before = revision();
    for (at, part) in [
        ("0/0", None),
        ("SameBook", Some("text")),
        ("SameBook", Some("metadata")),
    ] {
        let card = mcp.ok("get_card", json!({"at":at}));
        let content = if part == Some("metadata") {
            card["bibtex"].as_str().unwrap()
        } else {
            card["text"].as_str().unwrap()
        };
        let mut cmd = home.cmd_with_editor(content);
        cmd.args(["e", "--at", at]);
        if let Some(part) = part {
            cmd.args(["--part", part]);
        }
        cmd.assert().success();
        assert_eq!(
            revision(),
            before,
            "an unchanged {at} {part:?} save is not a new archive version"
        );
        assert_eq!(mcp.ok("get_card", json!({"at":at}))["text"], card["text"]);
    }
}

#[test]
fn link_index_can_be_checked_and_rebuilt_without_changing_authored_cards() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let topic = mcp.ok(
        "create_topic",
        json!({"title":"Index topic","body":"An entry point."}),
    );
    let root = topic["address"].as_str().unwrap();
    let made = mcp.ok("create_successor", json!({"at":root,"kind":"direct","title":"A reasoned claim","body":format!("依赖 [[{root}]]，说明前提。") }));
    let address = made["address"].as_str().unwrap();
    let before = mcp.ok("get_card", json!({"at":address}));

    let checked = home
        .cmd()
        .args(["status", "--check-links"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&checked).expect("structured Link integrity report");
    assert_eq!(report["ok"], true);
    assert_eq!(report["links"], 1);
    home.cmd().arg("reindex").assert().success();
    let after = mcp.ok("get_card", json!({"at":address}));
    assert_eq!(after["text"], before["text"]);
    let target = mcp.ok("get_card", json!({"at":root}));
    assert!(
        target["text"]
            .as_str()
            .unwrap()
            .contains("A reasoned claim")
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn ordinary_link_edits_only_index_the_changed_card_and_refresh_its_neighbours() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let topic = mcp.ok("create_topic", json!({"title":"Related","body":"Entry"}));
    let target = mcp.ok(
        "create_successor",
        json!({"at":topic["address"],"title":"Premise","kind":"direct","body":"A premise."}),
    );
    let at = target["address"].as_str().unwrap();
    let source = mcp.ok("create_successor", json!({"at":at,"kind":"side","title":"Source","body":format!("依赖 [[{at}]]，也借用 [[{at}]]。") }));
    for n in 0..20 {
        mcp.ok(
            "create_topic",
            json!({"title":format!("Unrelated {n}"),"body":"Independent body."}),
        );
    }
    mcp.ok(
        "edit_card",
        json!({"at":source["address"],"title":"Renamed source"}),
    );
    let checked = home
        .cmd()
        .args(["status", "--check-links"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: Value = serde_json::from_slice(&checked).unwrap();
    assert_eq!(
        report["links"], 2,
        "occurrences are retained, not deduplicated"
    );
    assert_eq!(
        report["last_write"]["indexed_cards"], 1,
        "unrelated Card bodies must not be indexed again"
    );
    assert_eq!(report["last_write"]["refreshed_cards"], 2);
    let target = mcp.ok("get_card", json!({"at":at}));
    assert!(target["text"].as_str().unwrap().contains("Renamed source"));
    mcp.ok(
        "edit_card",
        json!({"at":source["address"],"body":"The reference was removed."}),
    );
    let target = mcp.ok("get_card", json!({"at":at}));
    assert!(!target["text"].as_str().unwrap().contains("Renamed source"));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn metadata_and_content_edits_advance_one_revision_and_no_op_edits_do_not() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let root = mcp.ok("create_topic", json!({"title":"Models"}));
    let card = mcp.ok(
        "create_successor",
        json!({"at":root["address"],"kind":"direct","title":"A model","body":"A setting."}),
    );
    let revision = || -> u64 {
        let out = home
            .cmd()
            .args(["status", "--check-links"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        serde_json::from_slice::<Value>(&out).unwrap()["archive_revision"]
            .as_u64()
            .unwrap()
    };
    let before = revision();
    mcp.ok(
        "edit_card",
        json!({"at":card["address"],"type":"model","level":"formal","form":"proposition"}),
    );
    assert_eq!(revision(), before + 1);
    mcp.ok(
        "edit_card",
        json!({"at":card["address"],"type":"model","level":"formal","form":"proposition"}),
    );
    assert_eq!(revision(), before + 1);
    mcp.ok(
        "edit_card",
        json!({"at":card["address"],"title":"A revised model","level":"concept","form":"lemma"}),
    );
    assert_eq!(
        revision(),
        before + 2,
        "one transaction for authored text and metadata"
    );
}

#[test]
fn legacy_link_migration_names_a_bad_card_and_can_be_retried_after_repair() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    let card = mcp.ok(
        "create_topic",
        json!({"title":"Preserved legacy topic","body":"The original body."}),
    );
    let at = card["address"].as_str().unwrap();
    let original = mcp.ok("get_card", json!({"at":at}))["text"]
        .as_str()
        .unwrap()
        .to_string();
    drop(mcp);
    home.cmd().arg("down").assert().success();
    // Model an archive created before the index and an invalid persisted body.
    // This is storage-boundary fault injection, not a query used to assert behavior.
    let db = home.db();
    db.execute_batch("DROP TABLE links; DELETE FROM metadata WHERE key='links_index_version';")
        .unwrap();
    db.execute(
        "UPDATE cards SET text='# Bad\n\n[[invalid target]]\n' WHERE location=?1",
        [at],
    )
    .unwrap();
    drop(db);
    home.cmd()
        .arg("up")
        .assert()
        .failure()
        .stderr(predicate::str::contains(format!("Card `{at}`")));
    let db = home.db();
    db.execute(
        "UPDATE cards SET text=?1 WHERE location=?2",
        rusqlite::params![original, at],
    )
    .unwrap();
    drop(db);
    home.cmd().arg("up").assert().success();
    let mut mcp = home.mcp();
    assert_eq!(mcp.ok("get_card", json!({"at":at}))["text"], original);
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[cfg(windows)]
#[test]
fn stale_lock_sharing_violation_is_retried_until_the_handle_is_released() {
    use std::os::windows::fs::OpenOptionsExt;
    let home = TestHome::new();
    home.configure_and_up();
    let path = home.archive_root.join("zt.edit.lock");
    fs::write(&path, "zt edit lock\npid 999999\n").unwrap();
    let (sent, ready) = std::sync::mpsc::channel();
    let holder = std::thread::spawn(move || {
        let handle = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(3)
            .open(path)
            .unwrap();
        sent.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(800));
        drop(handle);
    });
    ready.recv().unwrap();
    let output = home.cmd().arg("reindex").output().unwrap();
    holder.join().unwrap();
    assert!(
        output.status.success(),
        "a transient sharing violation must be retried: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn structural_changes_keep_the_link_index_in_step_and_protect_human_authorship() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Structure"}));
    mcp.ok(
        "create_successor",
        json!({"at":"0/0","kind":"direct","title":"First"}),
    );
    mcp.ok(
        "create_successor",
        json!({"at":"0/1","kind":"side","title":"Target"}),
    );
    mcp.ok(
        "create_successor",
        json!({"at":"0/1","kind":"side","title":"Source","body":"依赖 [[0/1|a]]。"}),
    );
    mcp.ok(
        "move_card",
        json!({"at":"0/1|b","to":"0/2","confirm":"move"}),
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
    mcp.ok("insert_card",json!({"before":"0/1|a","title":"Inserted","body":"A local continuation.","confirm":"insert"}));
    assert!(
        mcp.ok("get_card", json!({"at":"0/2"}))["body"]
            .as_str()
            .unwrap()
            .contains("[[0/1|a|1]]")
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
    mcp.ok(
        "delete_card",
        json!({"at":"0/1|a","into":"0/1","confirm":"delete"}),
    );
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
    mcp.ok(
        "create_literature",
        json!({"bibtex":"@book{IndexBook,title={Indexed book}}","body":"依赖 [[0/1]]。"}),
    );
    mcp.ok("create_successor",json!({"at":"IndexBook","kind":"direct","title":"Literature claim","body":"参照 [[0/2]]。"}));
    mcp.ok("edit_literature_metadata",json!({"at":"IndexBook","bibtex":"@book{RenamedIndexBook,title={Renamed indexed book}}","confirm":"move"}));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
    home.cmd_with_editor("# Human author\n\n参照 [[0/2]]。\n")
        .args(["t", "Human author"])
        .assert()
        .success();
    let refusal = mcp.err(
        "move_card",
        json!({"at":"0/2","to":"0/1|a","confirm":"move"}),
    );
    assert!(refusal.contains("human workspace"));
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

#[test]
fn an_explicit_reindex_repairs_corrupt_derived_rows_without_changing_card_text() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({"title":"Kept target"}));
    mcp.ok(
        "create_successor",
        json!({"at":"0/0","kind":"direct","title":"Kept source","body":"依赖 [[0/0]]，保持原文。"}),
    );
    let before = mcp.ok("get_card", json!({"at":"0/0"}))["text"].clone();
    // Corruption injection at the persistence boundary; assert recovery via CLI/MCP.
    home.db().execute("DELETE FROM links", []).unwrap();
    let out = home
        .cmd()
        .args(["status", "--check-links"])
        .assert()
        .failure()
        .get_output()
        .stdout
        .clone();
    let faults: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(
        faults["faults"][0]["first_missing"][0],
        json!("0/1"),
        "identify the first faulty occurrence source"
    );
    home.cmd().arg("reindex").assert().success();
    assert_eq!(mcp.ok("get_card", json!({"at":"0/0"}))["text"], before);
    home.cmd()
        .args(["status", "--check-links"])
        .assert()
        .success();
}

impl TestHome {
    fn gui_card(&self, at: &str) -> Value {
        use std::io::Read;
        let url = fs::read_to_string(self.archive_root.join("zt.gui")).unwrap();
        let port: u16 = url
            .trim()
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches('/')
            .parse()
            .unwrap();
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut page = String::new();
        stream.read_to_string(&mut page).unwrap();
        let token = page
            .split("name=\"zt-token\" content=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        http_request(
            port,
            "GET",
            &format!("/api/card?at={}", at.replace('|', "%7C")),
            token,
            None,
        )
    }

    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_dir = tmp.path().join("config");
        let archive_root = tmp.path().join("archive");
        Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn cmd_with_editor(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
        )
        .expect("editor script");
        let mut cmd = self.cmd();
        cmd.env(
            "EDITOR",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                script.display()
            ),
        );
        cmd.env("ZT_EDITOR_TEXT", text);
        cmd
    }

    fn configure(&self) {
        self.cmd()
            .args([
                "config",
                "set",
                "archive_root",
                self.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    fn configure_and_up(&self) {
        self.configure();
        self.cmd().arg("up").assert().success();
    }

    fn db(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db")
    }

    fn text(&self, address: &str) -> String {
        self.db()
            .query_row(
                "SELECT text FROM cards WHERE location = ?1 OR citation_key = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card text")
    }

    fn workspace(&self, address: &str) -> String {
        self.db()
            .query_row(
                "SELECT workspace FROM cards WHERE location = ?1 OR citation_key = ?1",
                [address],
                |row| row.get(0),
            )
            .expect("card workspace")
    }

    fn exists(&self, address: &str) -> bool {
        self.db()
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1 OR citation_key = ?1)",
                [address],
                |row| row.get::<_, i64>(0),
            )
            .expect("exists")
            == 1
    }

    /// A human Topic `0/0` with one Card `0/1`, made with shell commands.
    fn human_topic(&self) {
        self.cmd_with_editor("# Human Topic\n\nmine\n")
            .args(["t", "Human Topic"])
            .assert()
            .success()
            .stdout("0/0\n");
        self.cmd_with_editor("# Human Card\n\nmy idea\n")
            .args(["n", "--at", "0/0"])
            .assert()
            .success()
            .stdout("0/1\n");
    }

    fn mcp(&self) -> Mcp {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        let init = mcp.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "zt-tests", "version": "1" },
            }),
        );
        assert_eq!(init["result"]["serverInfo"]["name"], "zt");
        mcp.notify("notifications/initialized");
        mcp
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Mcp {
    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write MCP message");
        self.stdin.flush().expect("flush MCP message");
    }

    fn notify(&mut self, method: &str) {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }));
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read MCP reply");
        let reply: Value = serde_json::from_str(&line).expect("MCP reply is JSON");
        assert_eq!(reply["id"], id, "replies arrive in request order");
        reply
    }

    /// Call a tool; return its structured result and whether it failed.
    fn call(&mut self, name: &str, arguments: Value) -> (Value, bool) {
        let reply = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &reply["result"];
        assert!(
            result["content"][0]["text"].is_string(),
            "tool results carry text content: {reply}"
        );
        (
            result["structuredContent"].clone(),
            result["isError"].as_bool().expect("isError flag"),
        )
    }

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(!is_error, "{name}({arguments}) failed: {value}");
        value
    }

    fn err(&mut self, name: &str, arguments: Value) -> String {
        let (value, is_error) = self.call(name, arguments.clone());
        assert!(
            is_error,
            "{name}({arguments}) should fail but returned {value}"
        );
        value["error"].as_str().expect("error message").to_string()
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn shell_and_session_create_trees_in_the_named_workspace_and_root_shows_its_zones() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd_with_editor("# Agent Topic\n\ntheirs\n")
        .args(["t", "--ws", "agent", "Agent Topic"])
        .assert()
        .success()
        .stdout("1/0\n");
    home.cmd_with_editor("# Agent Card\n\nsee [[0/1]]\n")
        .args(["n", "--at", "1/0"])
        .assert()
        .success()
        .stdout("1/1\n");
    home.cmd()
        .args(["t", "--ws", "shared", "Nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown workspace `shared`"));

    assert_eq!(home.workspace("0/0"), "human");
    assert_eq!(home.workspace("0/1"), "human");
    assert_eq!(home.workspace("1/0"), "agent");
    assert_eq!(
        home.workspace("1/1"),
        "agent",
        "successors join their tree's workspace"
    );
    assert!(
        home.text("0/1").contains("- [[1/1]] Agent Card"),
        "Links cross workspaces and generate Reverse links"
    );

    home.cmd().arg("ws").assert().success().stdout(
        "human (Human): 1 topic trees, 0 literature trees, 2 cards; MCP access: read-only\n\
         agent (AI agent): 1 topic trees, 0 literature trees, 2 cards; MCP access: read-write\n",
    );

    let output = home
        .cmd()
        .write_stdin(
            concat!(
                "ws agent\n",
                "ls\n",
                "t Session Agent Topic\n",
                "# Session Agent Topic\n\nfrom the session\n",
                "\x13",
                "go 0/1\n",
                "go\n",
                "ws\n",
                "q\n"
            )
            .as_bytes(),
        )
        .output()
        .expect("line session");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with(
        "ROOT\nworkspace: human (switch with ws agent)\ntopics:\n0/0 Human Topic\nliterature:\n"
    ));
    assert!(stdout.contains(
        "ROOT\nworkspace: agent (switch with ws human)\ntopics:\n1/0 Agent Topic\nliterature:\n"
    ));
    assert!(stdout.contains("agent workspace topics and literature: 1\n1/0 Agent Topic\n"));
    assert!(
        stdout.contains("location: 0/1\n# Human Card"),
        "go follows Links into the other workspace"
    );
    assert!(
        stdout.contains(
            "ROOT\nworkspace: human (switch with ws agent)\ntopics:\n0/0 Human Topic\nliterature:\n"
        ),
        "ROOT follows the Pointer's workspace"
    );
    assert!(stdout.contains("* human (Human): 1 topic trees"));
    assert!(stdout.contains("  agent (AI agent): 2 topic trees"));
    assert_eq!(
        home.workspace("2/0"),
        "agent",
        "t creates in the current workspace"
    );
}

#[test]
fn cards_never_move_between_workspaces_even_for_the_owner() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd_with_editor("# Agent Topic\n\ntheirs\n")
        .args(["t", "--ws", "agent", "Agent Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Agent Card\n\nbody\n")
        .args(["n", "--at", "1/0"])
        .assert()
        .success();
    home.cmd_with_editor("# Agent Card 2\n\nbody\n")
        .args(["n", "--at", "1/1"])
        .assert()
        .success();

    home.cmd()
        .args(["mv", "--at", "1/2", "0/2"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cards cannot move between workspaces: `1/2` is in the agent workspace and `0/2` is in the human workspace",
        ));
    home.cmd()
        .args(["mv", "--at", "0/1", "1/1|a"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Cards cannot move between workspaces",
        ));
    assert!(home.exists("1/2") && home.exists("0/1"));

    home.cmd()
        .args(["mv", "--at", "1/2", "1/1|a"])
        .write_stdin("move\n")
        .assert()
        .success();
    assert!(home.exists("1/1|a"));
    assert_eq!(home.workspace("1/1|a"), "agent");
}

#[test]
fn up_adds_workspaces_to_an_existing_archive_with_every_card_in_the_human_workspace() {
    let home = TestHome::new();
    home.configure();
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("old db");
    conn.execute_batch(
        "CREATE TABLE cards (
            row_id INTEGER PRIMARY KEY,
            location TEXT UNIQUE,
            citation_key TEXT UNIQUE,
            is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
            is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
            bibtex TEXT,
            text TEXT NOT NULL
         );
         CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO metadata VALUES('card_format', 'markdown');
         INSERT INTO metadata VALUES('next_topic_id', '1');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
           VALUES('0/0', NULL, 1, 0, NULL, '# Old Topic\n\nold\n\n<!-- zt:reverse-links -->\n');
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
           VALUES(NULL, 'Old2020', 0, 1, '@book{Old2020, title={Old}}', '# Old\n\n<!-- zt:reverse-links -->\n');",
    )
    .expect("pre-workspace archive");
    drop(conn);

    home.cmd().arg("up").assert().success();
    assert_eq!(home.workspace("0/0"), "human");
    assert_eq!(home.workspace("Old2020"), "human");
    assert!(
        home.db()
            .execute(
                "UPDATE cards SET workspace = 'shared' WHERE location = '0/0'",
                []
            )
            .is_err(),
        "the schema accepts only the two workspaces"
    );
    home.cmd()
        .arg("ws")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "human (Human): 1 topic trees, 1 literature trees, 2 cards",
        ));
    home.cmd().arg("down").assert().success();
    home.cmd().arg("up").assert().success();
    assert_eq!(home.workspace("0/0"), "human");
}

#[test]
fn mcp_reads_both_workspaces_but_changes_only_the_agent_workspace() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let human_topic_text = home.text("0/0");
    let mut mcp = home.mcp();

    let tools = mcp.request("tools/list", json!({}));
    let names = tools["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    for expected in [
        "list_workspaces",
        "list_trees",
        "get_tree",
        "get_card",
        "search_cards",
        "list_links",
        "list_broken_links",
        "create_topic",
        "create_literature",
        "create_successor",
        "edit_card",
        "edit_literature_metadata",
        "delete_card",
        "move_card",
    ] {
        assert!(
            names.contains(&expected.to_string()),
            "missing tool {expected}"
        );
    }

    let workspaces = mcp.ok("list_workspaces", json!({}));
    assert_eq!(workspaces["workspaces"][0]["id"], "human");
    assert_eq!(workspaces["workspaces"][0]["access"], "read-only");
    assert_eq!(workspaces["workspaces"][0]["zones"]["topic"]["trees"], 1);
    assert_eq!(workspaces["workspaces"][1]["id"], "agent");
    assert_eq!(workspaces["workspaces"][1]["access"], "read-write");

    // Reads cover the human workspace.
    let card = mcp.ok("get_card", json!({ "at": "0/1" }));
    assert_eq!(
        (
            card["title"].as_str(),
            card["workspace"].as_str(),
            card["writable"].as_bool()
        ),
        (Some("Human Card"), Some("human"), Some(false))
    );
    assert_eq!(card["parent"]["address"], "0/0");

    // Writes to the human workspace are refused and change nothing.
    let read_only = "the human workspace is read-only for AI agents";
    assert!(
        mcp.err(
            "create_topic",
            json!({ "title": "Sneaky", "workspace": "human" })
        )
        .contains(read_only)
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "title": "Sneaky" })
        )
        .contains(read_only)
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/1", "kind": "side", "title": "Sneaky" })
        )
        .contains(read_only)
    );
    assert!(
        mcp.err("edit_card", json!({ "at": "0/1", "body": "rewritten" }))
            .contains(read_only)
    );
    assert!(
        mcp.err("delete_card", json!({ "at": "0/1", "confirm": "delete" }))
            .contains(read_only)
    );
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "0/1", "to": "0/0", "confirm": "move" })
        )
        .contains(read_only)
    );
    assert!(home.exists("0/1") && !home.exists("0/2") && !home.exists("0/1|a"));
    assert_eq!(home.text("0/0"), human_topic_text);

    // The agent grows its own Topic tree, linking into the human workspace.
    let topic = mcp.ok(
        "create_topic",
        json!({ "title": "Agent Research", "body": "What the agent is exploring." }),
    );
    assert_eq!(
        (topic["address"].as_str(), topic["workspace"].as_str()),
        (Some("1/0"), Some("agent"))
    );
    let first = mcp.ok(
        "create_successor",
        json!({ "at": "1/0", "title": "Builds on the human", "body": "Extends [[0/1]]." }),
    );
    assert_eq!(first["address"], "1/1");
    let side = mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "A tangent" }),
    );
    assert_eq!(side["address"], "1/1|a");
    assert_eq!(home.workspace("1/1|a"), "agent");
    assert!(
        home.text("0/1")
            .contains("## Reverse links\n\n- [[1/1]] Builds on the human"),
        "the human Card's generated Reverse links show the agent's Link"
    );
    assert!(home.text("0/1").starts_with("# Human Card\n\nmy idea\n"));

    // Literature Zone.
    let literature = mcp.ok(
        "create_literature",
        json!({ "bibtex": "@article{Agent2026, title = {Agent Paper}}", "body": "Summary." }),
    );
    assert_eq!(literature["address"], "Agent2026");
    assert!(
        mcp.err(
            "create_literature",
            json!({ "bibtex": "@article{Human2026, title = {X}}", "workspace": "human" })
        )
        .contains(read_only)
    );
    assert_eq!(
        mcp.ok(
            "create_successor",
            json!({ "at": "Agent2026", "title": "Note" })
        )["address"],
        "Agent2026/1"
    );

    let trees = mcp.ok("list_trees", json!({ "workspace": "agent" }));
    let roots = trees["trees"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tree| {
            (
                tree["root"].as_str().unwrap(),
                tree["kind"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(roots, [("1/0", "topic"), ("Agent2026", "literature")]);
    let literature_zone = mcp.ok(
        "list_trees",
        json!({ "workspace": "agent", "zone": "literature" }),
    );
    assert_eq!(literature_zone["trees"].as_array().unwrap().len(), 1);
    let tree = mcp.ok("get_tree", json!({ "tree": "1/1|a" }));
    assert_eq!(
        (
            tree["root"].as_str(),
            tree["zone"].as_str(),
            tree["cards"].as_array().unwrap().len()
        ),
        (Some("1/0"), Some("topic"), 3)
    );

    let hits = mcp.ok("search_cards", json!({ "query": "idea" }));
    assert_eq!(hits["results"][0]["address"], "0/1");
    let hits = mcp.ok(
        "search_cards",
        json!({ "query": "idea", "workspace": "agent" }),
    );
    assert!(hits["results"].as_array().unwrap().is_empty());

    let links = mcp.ok("list_links", json!({ "cross_workspace_only": true }));
    assert_eq!(
        links["links"],
        json!([{
            "source": "1/1",
            "source_workspace": "agent",
            "target": "0/1",
            "target_workspace": "human",
            "broken": false,
        }])
    );

    // Editing the agent's own Cards.
    mcp.ok(
        "edit_card",
        json!({ "at": "1/1", "body": "Extends [[0/1]] and [[Agent2026]]." }),
    );
    assert!(
        home.text("1/1")
            .starts_with("# Builds on the human\n\nExtends [[0/1]] and [[Agent2026]].\n")
    );
    mcp.ok(
        "edit_card",
        json!({ "at": "1/1|a", "title": "A better tangent" }),
    );
    assert!(home.text("1/1|a").starts_with("# A better tangent\n"));
    assert!(
        mcp.err("edit_card", json!({ "at": "1/1", "text": "no heading" }))
            .contains("`# Title` heading")
    );
    assert!(
        mcp.err(
            "edit_card",
            json!({ "at": "1/1", "body": "x", "base": "stale text" })
        )
        .contains("changed since it was opened")
    );

    // Moves stay inside the agent workspace.
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "1/1|a", "to": "0/2", "confirm": "move" })
        )
        .contains("Cards cannot move between workspaces")
    );
    let plan = mcp.ok("move_card", json!({ "at": "1/1|a", "to": "1/2" }));
    assert_eq!(
        (plan["applied"].as_bool(), plan["confirm"].as_str()),
        (Some(false), Some("move"))
    );
    assert!(home.exists("1/1|a") && !home.exists("1/2"));
    let moved = mcp.ok(
        "move_card",
        json!({ "at": "1/1|a", "to": "1/2", "confirm": "move" }),
    );
    assert_eq!(moved["applied"], true);
    assert!(home.exists("1/2") && !home.exists("1/1|a"));

    // Renaming the agent's Citation key rewrites the agent's Links to it.
    let plan = mcp.ok(
        "edit_literature_metadata",
        json!({ "at": "Agent2026", "bibtex": "@article{AgentPaper2026, title = {Agent Paper}}" }),
    );
    assert_eq!(plan["confirm"], "move");
    let renamed = mcp.ok(
        "edit_literature_metadata",
        json!({
            "at": "Agent2026",
            "bibtex": "@article{AgentPaper2026, title = {Agent Paper}}",
            "confirm": "move",
        }),
    );
    assert_eq!(renamed["address"], "AgentPaper2026");
    assert!(home.exists("AgentPaper2026/1"));
    assert!(home.text("1/1").contains("[[AgentPaper2026]]"));

    // Deleting needs the confirmation the plan names.
    let plan = mcp.ok("delete_card", json!({ "at": "1/0" }));
    assert_eq!(plan["confirm"], "1/0");
    assert!(mcp.ok("delete_card", json!({ "at": "1/0", "confirm": "delete" }))["applied"] == false);
    assert!(home.exists("1/0"));
    let deleted = mcp.ok("delete_card", json!({ "at": "1/0", "confirm": "1/0" }));
    assert_eq!(deleted["deleted"], 3);
    assert!(!home.exists("1/0") && !home.exists("1/1") && !home.exists("1/2"));
    assert!(home.exists("0/1"), "the human tree is untouched");
    assert!(
        !home.text("0/1").contains("Reverse links"),
        "the agent's Link is gone from the human Card's Reverse links"
    );
    assert!(
        mcp.ok("list_broken_links", json!({}))["broken"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn mcp_refuses_agent_changes_that_would_rewrite_links_in_human_cards() {
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    let mut mcp = home.mcp();
    mcp.ok("create_topic", json!({ "title": "Agent" }));
    mcp.ok("create_successor", json!({ "at": "1/0", "title": "Base" }));
    mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "Side A" }),
    );
    mcp.ok(
        "create_successor",
        json!({ "at": "1/1", "kind": "side", "title": "Side B" }),
    );
    mcp.ok("create_successor", json!({ "at": "1/1", "title": "Next" }));
    mcp.ok(
        "create_literature",
        json!({ "bibtex": "@book{AgentBook, title = {Agent Book}}" }),
    );
    // The human links to agent Cards from their own tree.
    home.cmd_with_editor("# Human Notes\n\nSee [[1/1|b]], [[1/2]], and [[AgentBook]].\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success()
        .stdout("0/2\n");
    let human_text = home.text("0/2");

    let refused = "which is read-only for AI agents";
    assert!(
        mcp.err(
            "move_card",
            json!({ "at": "1/2", "to": "1/1|a|1", "confirm": "move" })
        )
        .contains(&format!(
            "rewrite Links in human workspace Card `0/2`, {refused}"
        ))
    );
    assert!(
        mcp.err("delete_card", json!({ "at": "1/1|a", "confirm": "delete" }))
            .contains(refused),
        "compacting 1/1|b to 1/1|a would rewrite the human Link"
    );
    assert!(
        mcp.err(
            "edit_literature_metadata",
            json!({ "at": "AgentBook", "bibtex": "@book{AgentBook2, title = {Agent Book}}", "confirm": "move" })
        )
        .contains(refused)
    );
    assert_eq!(home.text("0/2"), human_text);
    assert!(home.exists("1/1|a") && home.exists("1/1|b") && home.exists("1/2"));
    assert!(home.exists("AgentBook"));

    // Deleting a linked agent Card leaves a Broken link without editing the human body.
    let deleted = mcp.ok("delete_card", json!({ "at": "1/2", "confirm": "delete" }));
    assert_eq!(deleted["deleted"], 1);
    assert!(
        home.text("0/2")
            .starts_with("# Human Notes\n\nSee [[1/1|b]], [[1/2]], and [[AgentBook]].\n")
    );
    let broken = mcp.ok("list_broken_links", json!({ "workspace": "human" }));
    assert_eq!(broken["broken"][0]["target"], "1/2");
    assert_eq!(broken["broken"][0]["workspace"], "human");

    // The owner may still do what the agent may not.
    home.cmd()
        .args(["del", "--at", "1/1|a"])
        .write_stdin("delete\n")
        .assert()
        .success();
    assert!(
        home.text("0/2").contains("[[1/1|a]]"),
        "owner deletes compact and rewrite Links"
    );
}

#[test]
fn mcp_protocol_errors_and_service_state_are_reported() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut mcp = home.mcp();
    assert_eq!(mcp.request("ping", json!({}))["result"], json!({}));
    assert_eq!(
        mcp.request("bogus/method", json!({}))["error"]["code"],
        -32601
    );
    assert_eq!(
        mcp.request(
            "tools/call",
            json!({ "name": "no_such_tool", "arguments": {} })
        )["error"]["code"],
        -32602
    );
    assert!(
        mcp.err("get_card", json!({ "at": "9/9" }))
            .contains("card `9/9` does not exist")
    );
    assert!(
        mcp.err("create_topic", json!({ "title": "two\nlines" }))
            .contains("single-line")
    );
    assert!(
        mcp.err(
            "create_successor",
            json!({ "at": "0/0", "kind": "sideways", "title": "x" })
        )
        .contains("unknown successor kind `sideways`")
    );

    home.cmd().arg("down").assert().success();
    assert!(
        mcp.err("list_workspaces", json!({}))
            .contains("the ZT service is not available; run `zt up`")
    );
    home.cmd().arg("up").assert().success();
    assert_eq!(mcp.ok("list_workspaces", json!({}))["cards"], 0);
}

fn http_request(port: u16, method: &str, path: &str, token: &str, body: Option<&str>) -> Value {
    use std::io::Read;
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    let body = body.unwrap_or_default();
    let request = format!(
        "{method} {path} HTTP/1.1\r\nConnection: close\r\nHost: 127.0.0.1:{port}\r\nX-ZT-Token: {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = String::new();
    stream.read_to_string(&mut response).expect("read response");
    let start = response.find("\r\n\r\n").expect("header end") + 4;
    serde_json::from_str(&response[start..]).expect("JSON response")
}

#[test]
fn gui_creates_trees_in_either_workspace_and_reports_workspaces() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut gui = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .arg("gui")
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start zt gui");
    let mut stdout = BufReader::new(gui.stdout.take().expect("gui stdout"));
    let mut first_line = String::new();
    stdout.read_line(&mut first_line).expect("gui url line");
    let port: u16 = first_line
        .trim()
        .strip_prefix("zt gui: http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("gui prints its URL")
        .parse()
        .expect("port");
    let page = {
        use std::io::Read;
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        let mut page = String::new();
        stream.read_to_string(&mut page).unwrap();
        page
    };
    assert!(page.contains("id=\"workspace-switch\""));
    let token = page
        .split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("token")
        .to_string();
    let api = |method: &str, path: &str, body: Option<&str>| {
        http_request(port, method, path, &token, body)
    };

    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","text":"# Mine\n"}"##)
        )["address"],
        "0/0"
    );
    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","workspace":"agent","text":"# Theirs\n\nsee [[0/0]]\n"}"##)
        )["address"],
        "1/0"
    );
    assert_eq!(
        api(
            "POST",
            "/api/literature",
            Some(
                r##"{"workspace":"agent","bibtex":"@book{GuiBook, title={Gui Book}}","text":"notes"}"##
            )
        )["address"],
        "GuiBook"
    );
    assert!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"topic","workspace":"nowhere","text":"# X\n"}"##)
        )["error"]
            .as_str()
            .unwrap()
            .contains("unknown workspace")
    );
    // The GUI is the owner's tool: it may edit the agent workspace too.
    assert_eq!(
        api(
            "POST",
            "/api/create",
            Some(r##"{"kind":"direct","at":"1/0","text":"# Owner note\n"}"##)
        )["address"],
        "1/1"
    );
    assert_eq!(home.workspace("1/1"), "agent");

    let overview = api("GET", "/api/overview", None);
    assert_eq!(overview["workspaces"][0]["id"], "human");
    assert_eq!(overview["workspaces"][0]["agent_writable"], false);
    assert_eq!(overview["workspaces"][1]["id"], "agent");
    assert_eq!(overview["workspaces"][1]["topic_trees"], 1);
    assert_eq!(overview["workspaces"][1]["literature_trees"], 1);
    let trees = overview["trees"].as_array().unwrap();
    assert_eq!(
        trees
            .iter()
            .map(|tree| (
                tree["root"].as_str().unwrap(),
                tree["workspace"].as_str().unwrap(),
                tree["kind"].as_str().unwrap()
            ))
            .collect::<Vec<_>>(),
        [
            ("0/0", "human", "topic"),
            ("1/0", "agent", "topic"),
            ("GuiBook", "agent", "literature")
        ]
    );
    let card = api("GET", "/api/card?at=0%2F0", None);
    assert_eq!(card["workspace"], "human");
    assert_eq!(card["inbound"][0]["address"], "1/0");
    assert_eq!(card["inbound"][0]["workspace"], "agent");
    let card = api("GET", "/api/card?at=1%2F0", None);
    assert_eq!(card["outbound"][0]["workspace"], "human");
    assert!(
        api(
            "POST",
            "/api/move",
            Some(r#"{"at":"1/1","to":"0/1","confirm":"move"}"#)
        )["error"]
            .as_str()
            .unwrap()
            .contains("Cards cannot move between workspaces")
    );

    assert!(gui.wait().expect("gui exits").success());
}

#[cfg(windows)]
fn zip_backend_available() -> bool {
    StdCommand::new(host_tools::host_tar())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(not(windows))]
fn zip_backend_available() -> bool {
    StdCommand::new("zip")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// One Card as SQLite stores it: address, kind, Workspace, BibTeX, and text.
type CardRow = (String, &'static str, String, Option<String>, String);

#[test]
fn dump_records_workspaces_and_next_topic_id_so_it_replicates_the_archive() {
    if !zip_backend_available() {
        return;
    }
    let home = TestHome::new();
    home.configure_and_up();
    home.human_topic();
    home.cmd()
        .write_stdin(
            "l\n@book{HumanLit, title={Human Lit}}\n\x13notes on [[0/1]]\n\x13q\n".as_bytes(),
        )
        .assert()
        .success();
    {
        let mut mcp = home.mcp();
        mcp.ok(
            "create_topic",
            json!({ "title": "Agent Topic", "body": "builds on [[0/1]]" }),
        );
        mcp.ok(
            "create_successor",
            json!({ "at": "1/0", "title": "Agent Card" }),
        );
        mcp.ok("create_topic", json!({ "title": "Discarded" }));
        mcp.ok("delete_card", json!({ "at": "2/0", "confirm": "2/0" }));
        mcp.ok(
            "create_literature",
            json!({ "bibtex": "@article{AgentLit,\n  title = {Agent Lit}\n}\n", "body": "agent notes" }),
        );
        mcp.ok(
            "create_successor",
            json!({ "at": "AgentLit", "title": "Agent Lit Note" }),
        );
    }
    assert_eq!(home.workspace("HumanLit"), "human");

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 7 cards to "));

    let dump_dir = home.archive_root.join("dump");
    let archive_path = fs::read_dir(&dump_dir)
        .expect("dump directory")
        .map(|entry| entry.expect("dump entry").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "zip"))
        .expect("zip dump");
    let folder = archive_path
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let mut archive = zip::ZipArchive::new(fs::File::open(&archive_path).unwrap()).unwrap();
    let mut read = |name: &str| {
        let mut body = String::new();
        std::io::Read::read_to_string(
            &mut archive
                .by_name(&format!("{folder}/{name}"))
                .unwrap_or_else(|_| panic!("dump entry {name}")),
            &mut body,
        )
        .expect("UTF-8 entry");
        body
    };
    let mapping: std::collections::BTreeMap<String, String> =
        serde_json::from_str(&read("mapping.json")).expect("mapping.json");
    let manifest: Value = serde_json::from_str(&read("archive.json")).expect("archive.json");

    // The manifest names a Workspace for exactly the dumped addresses.
    let workspaces = manifest["workspaces"]
        .as_object()
        .expect("workspaces object");
    assert_eq!(
        workspaces.keys().collect::<Vec<_>>(),
        mapping.keys().collect::<Vec<_>>()
    );
    // Topic 2 was deleted, so the next Topic number is not derivable from the Cards.
    assert_eq!(manifest["next_topic_id"], 3);
    let stored_next: String = home
        .db()
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored_next, "3");

    // Rebuild every Card from the dump alone.
    let mut rebuilt: Vec<CardRow> = mapping
        .iter()
        .map(|(address, file)| {
            let markdown = read(file);
            let (bibtex, text) = match markdown.strip_prefix("---\nzt_kind: literature\n") {
                Some(rest) => {
                    let closing = rest.find("\n---\n").expect("front matter end");
                    (
                        Some(read(&format!("{address}.bib"))),
                        rest[closing + 5..].to_string(),
                    )
                }
                None => (None, markdown),
            };
            let kind = if bibtex.is_some() {
                "literature"
            } else if address.ends_with("/0") {
                "topic"
            } else {
                "regular"
            };
            let workspace = workspaces[address].as_str().expect("workspace").to_string();
            (address.clone(), kind, workspace, bibtex, text)
        })
        .collect();
    rebuilt.sort();

    let conn = home.db();
    let mut stmt = conn
        .prepare(
            "SELECT COALESCE(location, citation_key), is_topic, is_lit, bibtex, text, workspace
             FROM cards ORDER BY COALESCE(location, citation_key)",
        )
        .unwrap();
    let mut stored: Vec<CardRow> = stmt
        .query_map([], |row| {
            let kind = match (row.get::<_, i64>(1)?, row.get::<_, i64>(2)?) {
                (_, 1) => "literature",
                (1, _) => "topic",
                _ => "regular",
            };
            Ok((row.get(0)?, kind, row.get(5)?, row.get(3)?, row.get(4)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    stored.sort();

    assert_eq!(rebuilt, stored);
    let by_workspace = |workspace: &str| {
        stored
            .iter()
            .filter(|row| row.2 == workspace)
            .map(|row| (row.0.as_str(), row.1))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        by_workspace("human"),
        [
            ("0/0", "topic"),
            ("0/1", "regular"),
            ("HumanLit", "literature")
        ]
    );
    assert_eq!(
        by_workspace("agent"),
        [
            ("1/0", "topic"),
            ("1/1", "regular"),
            ("AgentLit", "literature"),
            ("AgentLit/1", "regular")
        ]
    );
}
