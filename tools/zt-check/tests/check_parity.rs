//! The extraction's parity contract (S01 / #143): the executable, the library
//! adapter (`check::run_lines`), and the process-free core adapter print the same
//! ordered lines for the recorded fixtures. The core adapter is the shape the
//! service check will call (S04): the core over entries and context the caller
//! already materialized, with no file read, no `zt mcp` client, and no process.

#![cfg(feature = "cli")]

use serde_json::Value;
use std::process::Command;
use zt_check::core::{Check, Limits};

const CARD_STYLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scripts/book-digest/tests/card-style-fixture.json"
);
const CARD_STYLE_EXPECTED: &str = include_str!("fixtures/card-style-expected.txt");
const CARD_LAYER: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../scripts/book-digest/tests/card-layer-fixture.json"
);
const CARD_LAYER_EXPECTED: &str = include_str!("fixtures/card-layer-expected.txt");
const LEGACY_BUNDLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/legacy-bundle.json"
);
const LEGACY_EXPECTED: &str = include_str!("fixtures/legacy-expected.txt");
const LEGACY_RULE_VERSION_BUNDLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/legacy-rule-version-bundle.json"
);
const LEGACY_RULE_VERSION_EXPECTED: &str = include_str!("fixtures/legacy-rule-version-expected.txt");

/// The installed executable's lines, as the digest scripts read them.
fn binary(args: &[&str]) -> Vec<String> {
    let output = Command::new(env!("CARGO_BIN_EXE_zt-check"))
        .args(args)
        .output()
        .expect("run zt-check");
    assert!(
        output.status.success(),
        "zt-check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8 output")
        .lines()
        .map(str::to_string)
        .collect()
}

/// The in-process library adapter zt-digest calls.
fn library(args: &[&str]) -> Vec<String> {
    let args = args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>();
    zt_check::check::run_lines(&args, None).expect("run the library check")
}

/// The gate adapter: the process-free core over the fixture's entries, setting
/// body, and shared parts, exactly the inputs the service materializes in its
/// transaction.
fn core_lines(path: &str, setting: &str, limits: Limits) -> Vec<String> {
    let raw = std::fs::read_to_string(path).expect("read the fixture");
    let cards: Value = serde_json::from_str(&raw).expect("the fixture is JSON");
    let cards = cards.as_array().expect("the fixture is an array of entries");
    let setting_body = (!setting.is_empty())
        .then(|| {
            cards
                .iter()
                .find(|card| card["address"] == setting)
                .and_then(|card| card["body"].as_str())
                .map(str::to_string)
        })
        .flatten();
    // A shared part follows the term table of its home model.
    let shared = cards
        .iter()
        .filter(|card| card["line"] == "shared")
        .filter_map(|card| card["address"].as_str().map(str::to_string))
        .collect::<Vec<_>>();
    let mut lines = Check::new(limits, false, "")
        .with_terms(setting_body.as_deref())
        .with_shared(shared)
        .run(cards);
    let count = lines.len();
    lines.push(format!("faults: {count}"));
    lines
}

#[test]
fn the_card_style_fixture_prints_the_same_ordered_lines_everywhere() {
    let expected = CARD_STYLE_EXPECTED.lines().collect::<Vec<_>>();
    let args = ["--json", CARD_STYLE, "--setting", "K/1|a|a"];
    assert_eq!(binary(&args), expected);
    assert_eq!(library(&args), expected);
    assert_eq!(core_lines(CARD_STYLE, "K/1|a|a", Limits::default()), expected);
}

#[test]
fn the_card_layer_fixture_prints_the_same_ordered_lines_everywhere() {
    let expected = CARD_LAYER_EXPECTED.lines().collect::<Vec<_>>();
    let args = ["--json", CARD_LAYER, "--max-idea-chars", "250"];
    assert_eq!(binary(&args), expected);
    assert_eq!(library(&args), expected);
    assert_eq!(
        core_lines(
            CARD_LAYER,
            "",
            Limits { max_idea_chars: 250, ..Limits::default() }
        ),
        expected
    );
}

#[test]
fn the_executable_states_the_library_version() {
    assert_eq!(
        binary(&["--version"]),
        [format!("zt-check {}", zt_check::VERSION)]
    );
}

/// S02 (#144): a draft bundle that carries both versions prints the same ordered
/// report through the executable and the library adapter. The retained faults
/// are advice, the duplicate occurrence is gone, and the two newly applicable
/// Event formats count as the only blocking faults.
#[test]
fn the_legacy_bundle_reports_the_same_ordered_lines_everywhere() {
    let expected = LEGACY_EXPECTED.lines().collect::<Vec<_>>();
    let args = ["--json", LEGACY_BUNDLE];
    assert_eq!(binary(&args), expected);
    assert_eq!(library(&args), expected);
}

/// S02-AC07/R0150/R0151: a changed rule version is one current snapshot applied
/// to both sides. The tightened `--max-sentences 8` makes the old authored
/// version faulty too, so the unchanged part stays editable as legacy advice,
/// while a new part with the same body must meet the current requirement.
#[test]
fn a_changed_rule_version_is_applied_to_both_sides() {
    let expected = LEGACY_RULE_VERSION_EXPECTED.lines().collect::<Vec<_>>();
    let args = [
        "--json",
        LEGACY_RULE_VERSION_BUNDLE,
        "--max-sentences",
        "8",
    ];
    assert_eq!(binary(&args), expected);
    assert_eq!(library(&args), expected);
}

#[test]
fn the_envelope_error_is_the_same_in_the_binary_and_the_library() {
    let not_json = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/envelope/not-json.json"
    );
    let output = Command::new(env!("CARGO_BIN_EXE_zt-check"))
        .args(["--json", not_json])
        .output()
        .expect("run zt-check");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).expect("utf-8 stderr");
    assert!(stderr.starts_with("zt-check: the file is not JSON"), "{stderr}");
    let error = zt_check::check::run_lines(&["--json".to_string(), not_json.to_string()], None)
        .unwrap_err();
    assert!(format!("{error:#}").contains("the file is not JSON"), "{error:#}");
}
