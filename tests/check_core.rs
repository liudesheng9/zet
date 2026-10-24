//! The service links the checker's process-free core (S01 / #143). The dependency
//! is a path dependency with default features disabled: no `check`/`mcp` adapter,
//! no `zt mcp` client, no platform binding, and no dependency cycle. The evaluation
//! is in-process and takes only the entry and the limits.

use serde_json::json;

#[test]
fn the_service_evaluates_card_legality_with_the_checker_core() {
    let title = "一".repeat(41);
    let cards = vec![json!({ "address": "K/1", "title": title, "body": "" })];
    let lines = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "").run(&cards);
    assert_eq!(
        lines,
        [format!("K/1 R12 (41 characters > 40) {title}")],
        "the core must state the rule and the measured size"
    );
    assert!(!zt_check::VERSION.is_empty(), "the checker names its version");
}

#[test]
fn the_core_needs_no_context_for_a_local_text_fault() {
    // The layer rules read the entry's category; without one only the language
    // rules run, so the service can check a draft before it reads any archive.
    let cards = vec![json!({ "address": "K/2", "title": "清算价值决定债务的回收率", "body": "甲；乙。" })];
    let lines = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "").run(&cards);
    assert_eq!(lines, ["K/2 R3 甲；乙。"]);
}
