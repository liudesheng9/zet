//! The service links the checker's process-free core (S01 / #143). The dependency
//! is a path dependency with default features disabled: no `check`/`mcp` adapter,
//! no `zt mcp` client, no platform binding, and no dependency cycle. The evaluation
//! is in-process and takes only the entry and the limits.

use serde_json::json;
use zt_check::core::{Check, Limits, Severity};
use zt_check::policy::{Applicability, ratchet};

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

/// S02 (#144): the service links the same core and policy, so it can decide a
/// proposed edit against the Card's old authored version in-process. The retained
/// style fault becomes legacy advice; the newly applicable Event format blocks.
/// This is the adapter shape S04 (#146) calls inside the write transaction.
#[test]
fn the_service_can_ratchet_an_edit_against_the_old_version_in_process() {
    let before = json!({
        "address": "K/3|a", "category": "idea", "title": "通用汽车收购费雪车身公司",
        "body": "通用汽车收购了费雪车身公司；交易在 1919 年完成。"
    });
    let proposed = json!({
        "address": "K/3|a", "category": "event", "title": "通用汽车收购费雪车身公司",
        "body": "通用汽车收购了费雪车身公司；交易在 1919 年完成。"
    });
    let check = Check::new(Limits::default(), false, "");
    let old_report = check.report(std::slice::from_ref(&before));
    let new_report = check.report(std::slice::from_ref(&proposed));
    let (old_facts, proposed_facts) = (Applicability::of(&before), Applicability::of(&proposed));
    let report = ratchet(&old_report, new_report, |diagnostic| {
        old_facts.comparable(&proposed_facts, &diagnostic.rule)
    });
    // R3 means the same in both roles: a retained occurrence is advice.
    let r3 = report
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule == "R3")
        .expect("R3");
    assert!(r3.legacy);
    assert_eq!(r3.severity, Severity::Advise);
    assert_eq!(r3.to_json()["severity"], json!("advise"));
    assert_eq!(r3.to_json()["legacy"], json!(true));
    // E1 and E2 are newly applicable Event requirements and still block.
    for rule in ["E1", "E2"] {
        assert!(
            report
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == rule && !diagnostic.legacy),
            "{rule} must be a new blocker: {report:#?}"
        );
    }
    assert_eq!(report.blockers, 2, "R3 is advice; E1 and E2 block: {report:#?}");
    assert_eq!(
        report
            .lines()
            .iter()
            .filter(|line| line.contains(" R3 (advisory) "))
            .count(),
        1
    );
}
