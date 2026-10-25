//! S28 (#170, US093/US095): the candidate release manifest and the compatible set it names.
//!
//! The release has four related but distinct contracts: the executable versions, the checker's
//! rule versions, the archive schema generation, and the frozen workflow build versions. The
//! manifest records the candidate set; the installed frozen registry, the source blueprints and
//! the running build must all agree with it, so a deployment cannot mix incompatible products.
//!
//! This test is the static half of the release verification seam (`scripts/verify-release.ps1` is
//! the installed-and-running half). It fails when a version was raised in one place and not in
//! another, when an installed frozen script is not the build its lock records, when the installed
//! set drifted from the current module/base sources, or when a new workflow version reused frozen
//! history instead of leaving it in place.

use assert_cmd::Command;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const MANIFEST: &str = "docs/term-cards-links-and-check-gate-release-manifest.json";
const REGISTRY: &str = ".claude/workflow-framework";
const WORKFLOWS: &str = ".claude/workflows";
const SOURCES: &str = "scripts/book-digest/workflow";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read_json(path: &Path) -> Value {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("{} is not JSON: {error}", path.display()))
}

fn manifest() -> Value {
    read_json(&root().join(MANIFEST))
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The first `version = "..."` line in a crate's `[package]` table. The dependency tables below it
/// also carry `version =`, so the scan stops at the first one after `[package]`.
fn crate_version(relative: &str) -> String {
    let path = root().join(relative);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let mut in_package = false;
    for line in text.lines() {
        let line = line.trim();
        if line == "[package]" {
            in_package = true;
            continue;
        }
        if in_package && line.starts_with('[') {
            break;
        }
        if in_package && let Some(rest) = line.strip_prefix("version = \"") {
            return rest.trim_end_matches('"').to_string();
        }
    }
    panic!("{} has no [package] version", path.display());
}

/// A lockfile's recorded version of one package, `None` when the package is absent.
fn lock_version(relative: &str, package: &str) -> Option<String> {
    let path = root().join(relative);
    let text = fs::read_to_string(&path).unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        if line.trim() != format!("name = \"{package}\"") {
            continue;
        }
        for next in lines.by_ref() {
            let next = next.trim();
            if let Some(rest) = next.strip_prefix("version = \"") {
                return Some(rest.trim_end_matches('"').to_string());
            }
            if next.starts_with("[[package]]") {
                break;
            }
        }
    }
    None
}

fn semver(value: &str) -> (u64, u64, u64) {
    let mut parts = value.split('.');
    let major = parts.next().and_then(|part| part.parse().ok());
    let minor = parts.next().and_then(|part| part.parse().ok());
    let patch = parts.next().and_then(|part| part.parse().ok());
    match (major, minor, patch) {
        (Some(major), Some(minor), Some(patch)) => (major, minor, patch),
        _ => panic!("`{value}` is not a semantic version"),
    }
}

/// S28-AC01: the manifest keeps the executable, rule, schema and workflow contracts apart, and
/// each names the same value as the code that carries it.
#[test]
fn the_release_manifest_distinguishes_the_four_compatibility_contracts() {
    let manifest = manifest();
    assert_eq!(manifest["schema_version"], serde_json::json!(1));
    assert_eq!(manifest["issue"], serde_json::json!(170));
    assert_eq!(manifest["slice"], "S28");

    let executable = &manifest["contracts"]["executable"];
    assert_eq!(executable["zt"], crate_version("Cargo.toml"));
    assert_eq!(executable["zt"], env!("CARGO_PKG_VERSION"));
    assert_eq!(executable["zt_check"], crate_version("tools/zt-check/Cargo.toml"));
    assert_eq!(executable["zt_check"], zt_check::VERSION);
    assert_eq!(executable["zt_digest"], crate_version("tools/zt-digest/Cargo.toml"));
    // Both lockfiles resolve the checker the release names, so the tools link the same rules.
    assert_eq!(lock_version("Cargo.lock", "zt-check").as_deref(), executable["zt_check"].as_str());
    assert_eq!(
        lock_version("tools/zt-digest/Cargo.lock", "zt-check").as_deref(),
        executable["zt_check"].as_str()
    );

    let rule = &manifest["contracts"]["rule"];
    assert_eq!(rule["rule_version"], zt_check::contract::RULE_VERSION);
    assert_eq!(rule["diagnostic_schema"], zt_check::contract::DIAGNOSTIC_SCHEMA);
    assert_eq!(rule["applicability_version"], zt_check::contract::APPLICABILITY_VERSION);

    let schema = &manifest["contracts"]["archive_schema"];
    assert_eq!(schema["generation"], "2");
    assert_eq!(schema["card_format"], "markdown");
    assert_eq!(schema["term_field"], "term_name");
    assert_eq!(schema["supported_read_generations"], serde_json::json!(["1", "2"]));

    let capabilities = zt_check::contract::Capabilities::current();
    assert_eq!(manifest["capabilities"]["term_links"], capabilities.term_links);
    assert_eq!(manifest["capabilities"]["term_cards"], capabilities.term_cards);
    assert!(capabilities.term_links && capabilities.term_cards, "the release understands Term data");

    // S28-AC06: the quality gate ships advisory; promotion is the owner's, never a timer.
    assert_eq!(manifest["policy"]["default_stage"], "advisory");
    assert_eq!(manifest["policy"]["mandatory_invariants_active"], true);
    assert_eq!(manifest["policy"]["promotion"], "owner");

    // The executable publishes the same values, so a running-service check needs no source tree.
    let out = Command::cargo_bin("zt")
        .expect("zt binary")
        .args(["version", "--json"])
        .output()
        .expect("zt version --json");
    assert!(
        out.status.success(),
        "zt version --json failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let published: Value = serde_json::from_slice(&out.stdout).expect("one JSON object");
    assert_eq!(published["zt"], executable["zt"]);
    assert_eq!(published["checker"], executable["zt_check"]);
    assert_eq!(published["rule_version"], rule["rule_version"]);
    assert_eq!(published["diagnostic_schema"], rule["diagnostic_schema"]);
    assert_eq!(published["applicability_version"], rule["applicability_version"]);
    assert_eq!(published["schema"], schema["generation"]);
    assert_eq!(published["capabilities"]["term_links"], capabilities.term_links);
    assert_eq!(published["capabilities"]["term_cards"], capabilities.term_cards);
}

/// The manifest names every skill the coordinated install ships, and no directory was left out.
#[test]
fn the_manifest_names_every_release_skill() {
    let manifest = manifest();
    let mut shipped: Vec<String> = fs::read_dir(root().join("skills"))
        .expect("skills directory")
        .filter_map(|entry| {
            let entry = entry.ok()?;
            entry
                .path()
                .join("SKILL.md")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().to_string())
        })
        .collect();
    shipped.sort();
    let mut named: Vec<String> = manifest["skills"]
        .as_array()
        .expect("skills is an array")
        .iter()
        .map(|value| value.as_str().expect("a skill name").to_string())
        .collect();
    named.sort();
    assert_eq!(named, shipped, "the manifest names every installable skill once");
    assert!(
        named.contains(&"zt-digest-protocol".to_string()) && named.contains(&"zt".to_string()),
        "the coordinated install carries the agent guidance"
    );
}

/// S28-AC02/AC03: both installed variants are the builds their locks record, assembled from the
/// current base/module sources, and the previous frozen builds stay in the registry history.
#[test]
fn the_installed_frozen_workflows_are_the_manifest_builds() {
    let manifest = manifest();
    let workflow = &manifest["contracts"]["workflow"];
    let base_version = workflow["base"].as_str().expect("a base version");
    let base_source = root().join(SOURCES).join("zt-digest-base.blueprint.json");
    let base = read_json(&base_source);
    assert_eq!(base["kind"], "base");
    assert_eq!(base["version"], base_version, "the base source carries the release version");

    let retained = manifest["retained_workflows"]
        .as_object()
        .expect("retained_workflows is an object");
    for name in ["zt-econ-digest", "zt-math-digest"] {
        let expected = workflow[name].as_str().expect("a variant version");
        let source_path = root().join(SOURCES).join(format!("{name}.blueprint.json"));
        let source = read_json(&source_path);
        assert_eq!(source["version"], expected, "{name} source carries the release version");
        assert_eq!(source["base"], "./zt-digest-base.blueprint.json");

        let registry = root().join(REGISTRY).join(name);
        let lock = read_json(&registry.join("lock.json"));
        assert_eq!(lock["protocol"], "workflow-framework-protocol/1");
        assert_eq!(lock["name"], name);
        assert_eq!(lock["version"], expected, "{name} lock carries the release version");
        assert_eq!(lock["script"], format!("{WORKFLOWS}/{name}.js"));

        let installed = root().join(lock["script"].as_str().expect("a script path"));
        assert!(installed.is_file(), "the frozen build exists: {}", installed.display());
        assert_eq!(
            sha256(&installed),
            lock["sha256"].as_str().expect("a script hash"),
            "{name}'s installed script is the build its lock records"
        );

        let composed = read_json(&registry.join("blueprint.json"));
        assert_eq!(composed["version"], expected);
        assert_eq!(composed["composedFrom"]["base"], "zt-digest-base");
        assert_eq!(
            composed["composedFrom"]["version"], base_version,
            "{name} was composed from the release base"
        );

        // Assembled from the current sources, not edited after install.
        let sources = lock["sources"].as_array().expect("the lock records its sources");
        assert!(!sources.is_empty(), "{name} records its source blueprints");
        for source in sources {
            let file = source["file"].as_str().expect("a source file");
            let recorded = source["sha256"].as_str().expect("a source hash");
            let relative = file.replace('\\', "/");
            assert_eq!(
                sha256(&root().join(&relative)),
                recorded,
                "{name}'s recorded source {relative} is the current source file"
            );
        }

        // The previous frozen build stays; the new one is a strictly higher, separate build.
        let old_version = retained[name].as_str().expect("a retained version");
        assert!(
            semver(expected) > semver(old_version),
            "{name} rose from {old_version} to {expected}"
        );
        for version in [old_version, expected] {
            let script = registry.join("history").join(version).join(format!("{name}.js"));
            assert!(script.is_file(), "{name} keeps its {version} build: {}", script.display());
        }
        assert_eq!(
            sha256(&registry.join("history").join(expected).join(format!("{name}.js"))),
            lock["sha256"].as_str().expect("a script hash"),
            "{name}'s new build is the one archived under its release version"
        );
    }
}
