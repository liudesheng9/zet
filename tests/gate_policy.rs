//! S03 (#145): the archive-specific Card-check policy through the real `zt
//! config` commands, over two isolated scratch archives.
//!
//! The verification seam of the issue: real config commands against two isolated
//! archives, checker evaluation using the resulting snapshot, and persisted
//! metadata/revision inspection. The live archive is never touched.

use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::json;
use std::fs;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;
use zt_check::core::{Check, Limits};
use zt_check::policy::PolicySet;

struct Home {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl Home {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let home = Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
        };
        home.run(&[
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ]);
        home.run(&["up"]);
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    /// Run one command and require success, returning its stdout.
    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().expect("zt command");
        assert!(
            out.status.success(),
            "zt {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).to_string()
    }

    fn conn(&self) -> Connection {
        Connection::open(self.archive_root.join("zt.sqlite3")).expect("open archive")
    }

    fn overrides(&self) -> Vec<(String, String)> {
        let conn = self.conn();
        let mut statement = conn
            .prepare("SELECT key, value FROM metadata WHERE key LIKE 'gate.%' ORDER BY key")
            .expect("prepare");
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query")
            .collect::<Result<Vec<_>, _>>()
            .expect("rows")
    }

    fn revision(&self) -> u64 {
        self.conn()
            .query_row(
                "SELECT value FROM metadata WHERE key = 'archive_revision'",
                [],
                |row| row.get::<_, String>(0),
            )
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(0)
    }

    /// The policy hash the config output publishes.
    fn policy_hash(&self) -> String {
        self.run(&["config", "show"])
            .lines()
            .find_map(|line| line.strip_prefix("policy_hash="))
            .expect("config show publishes the policy hash")
            .to_string()
    }

    /// The effective policy exactly as the archive stores it.
    fn policy(&self) -> PolicySet {
        PolicySet::from_pairs(self.overrides()).expect("stored policy rows are valid")
    }

    /// One Card checked under this archive's effective policy.
    fn check(&self, workspace: &str, body: &str) -> Vec<String> {
        let cards = vec![json!({
            "address": "9/1",
            "workspace": workspace,
            "title": "短标题",
            "body": body,
        })];
        Check::new(Limits::default(), false, "")
            .with_policies(self.policy())
            .run(&cards)
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

/// S03-AC01/AC02/AC04/AC05: one archive's declared overrides are validated,
/// persisted, evaluated and reset while the other archive keeps the defaults.
#[test]
fn one_archive_policy_is_configured_validated_and_reset_while_another_stays_default() {
    let a = Home::new();
    let b = Home::new();
    let (a_revision, b_revision) = (a.revision(), b.revision());
    let (a_hash, b_hash) = (a.policy_hash(), b.policy_hash());
    assert_eq!(a_hash, b_hash, "two fresh archives have the default policy");
    assert_eq!(a_hash.len(), 64, "the policy hash is a SHA-256");
    assert!(a.overrides().is_empty() && b.overrides().is_empty());

    // The declaration lands in the selected archive's metadata and moves its
    // revision. No process-global file is involved.
    let out = a.run(&["config", "set", "gate.agent.R2", "advise"]);
    assert_eq!(out, "gate.agent.R2=advise\n");
    assert_eq!(
        a.overrides(),
        [("gate.agent.R2".to_string(), "advise".to_string())]
    );
    assert_eq!(a.revision(), a_revision + 1);
    assert_ne!(a.policy_hash(), a_hash, "a policy change gets a new hash");
    assert!(b.overrides().is_empty());
    assert_eq!(b.revision(), b_revision, "the other archive's revision is untouched");
    assert_eq!(b.policy_hash(), b_hash, "the other archive's policy is untouched");
    // The override lives in the archive's metadata, not in the process-global
    // config file and not in a second database (R0117).
    let config_text = fs::read_to_string(a.config_dir.join("config.toml")).unwrap_or_default();
    assert!(!config_text.contains("gate."), "{config_text}");

    // The resulting snapshot decides a Card: advice in the changed archive,
    // still a blocker in the untouched one. The human Workspace of the changed
    // archive keeps its own default.
    let sentence = "甲".repeat(41);
    assert_eq!(
        a.check("agent", &sentence),
        [format!("9/1 R2 (advisory) (41 characters > 40) {sentence}")]
    );
    assert_eq!(
        b.check("agent", &sentence),
        [format!("9/1 R2 (41 characters > 40) {sentence}")]
    );
    assert_eq!(
        a.check("human", &sentence),
        [format!("9/1 R2 (advisory) (41 characters > 40) {sentence}")],
        "the human Workspace keeps its default"
    );

    // `zt config show` names the archive, the hash, the revision and every
    // effective override (R0118).
    let shown = a.run(&["config", "show"]);
    assert!(
        shown.contains(&format!("policy_archive={}", a.archive_root.display())),
        "{shown}"
    );
    assert!(
        shown.contains(&format!("policy_revision={}", a_revision + 1)),
        "{shown}"
    );
    assert!(shown.contains("policy_hash="), "{shown}");
    assert!(shown.contains("gate.agent.R2=advise"), "{shown}");
    assert!(shown.contains("overrides: 1"), "{shown}");

    // The full table makes the defaults as visible as the override, and names
    // the graph-only and mandatory rules.
    let table = a.run(&["config", "policy"]);
    let row = |rule: &str| {
        table
            .lines()
            .find(|line| line.starts_with(&format!("{rule} ")))
            .unwrap_or_else(|| panic!("{rule} is in the policy table: {table}"))
            .to_string()
    };
    assert!(row("R2").contains("advise*"), "{}", row("R2"));
    assert!(!row("R2").contains("block"), "{}", row("R2"));
    assert!(row("E2").contains("block") && !row("E2").contains('*'), "{}", row("E2"));
    assert!(row("R6").contains("off") && row("R6").contains("advise"), "{}", row("R6"));
    assert!(row("I1").contains("advise"), "{}", row("I1"));
    assert!(row("I5").contains("graph"), "{}", row("I5"));
    assert!(
        row("T4").contains("advise") && row("T4").contains("configurable"),
        "{}",
        row("T4")
    );
    assert!(row("T1").contains("block"), "{}", row("T1"));
    assert!(row("T7").contains("mandatory"), "{}", row("T7"));
    assert!(
        table.contains("invariants: envelope, fields, address, links, targets, access, locks"),
        "{table}"
    );

    // Every invalid declaration is refused with an explanation, writes no row,
    // and moves no revision.
    for (key, value, why) in [
        ("gate.agent.R1", "block", "unknown rule `R1`"),
        ("gate.agents.R2", "block", "unknown Workspace `agents`"),
        ("gate.agent.R2", "warn", "unknown policy value `warn`"),
        ("gate.human.access", "off", "mandatory service invariant"),
        ("gate.human.envelope", "off", "mandatory service invariant"),
        ("gate.agent.I5", "block", "graph-only audit rule"),
        ("gate.agent.T7", "block", "mandatory rule"),
        ("gate.agent.T8", "block", "graph-only audit rule"),
        ("gate.agent.T10", "block", "Term-facing label of R22"),
    ] {
        let out = a
            .cmd()
            .args(["config", "set", key, value])
            .output()
            .expect("zt config set");
        assert!(!out.status.success(), "{key}={value} must be refused");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(stderr.contains(why), "{key}={value}: {stderr}");
        assert_eq!(a.overrides().len(), 1, "{key}={value} wrote a row");
        assert_eq!(a.revision(), a_revision + 1, "{key}={value} moved the revision");
    }

    // A repeated value writes nothing; an explicit reset removes the row and
    // restores the default policy hash.
    assert_eq!(a.run(&["config", "set", "gate.agent.R2", "advise"]), "gate.agent.R2=advise\n");
    assert_eq!(a.revision(), a_revision + 1);
    assert_eq!(
        a.run(&["config", "set", "gate.agent.R2", "default"]),
        "gate.agent.R2=default\n"
    );
    assert!(a.overrides().is_empty());
    assert_eq!(a.revision(), a_revision + 2);
    assert_eq!(a.policy_hash(), a_hash, "the default policy is back");
    assert_eq!(b.revision(), b_revision);
    assert_eq!(b.policy_hash(), b_hash);
}

/// S03-AC01: an unconfigured or absent archive answers the policy as
/// unavailable without breaking the rest of `zt config show`.
#[test]
fn an_unselected_archive_reports_the_policy_as_unavailable() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let config_dir = tmp.path().join("config");
    let command = || {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    };
    let out = command().args(["config", "show"]).output().expect("config show");
    assert!(out.status.success());
    let shown = String::from_utf8_lossy(&out.stdout);
    assert!(shown.contains("archive_root=<unset>"), "{shown}");
    assert!(
        shown.contains("policy: unavailable (archive_root is unset)"),
        "{shown}"
    );
    let out = command().args(["config", "policy"]).output().expect("config policy");
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("archive_root is not configured"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
