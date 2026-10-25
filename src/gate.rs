//! The selected archive's effective Card-check policy (S03, #145).
//!
//! The checker owns the rule catalog and its defaults (`zt_check::policy`). This
//! module owns only what the archive stores: one `metadata` row per declared
//! `gate.<workspace>.<rule>` override, its validated write under the edit lock,
//! and the archive revision the change advances. A missing row means the
//! checker's default; `default` removes the row.
//!
//! The loaded `PolicySet` is the snapshot a write gate decides with. It covers
//! both Workspaces and is loaded once per operation, so a whole atomic batch
//! uses one policy (design 6.1, S03-AC05).

use crate::link_index;
use anyhow::{Result, anyhow, bail};
use rusqlite::Connection;
use std::path::Path;
use zt_check::policy::{self, Change, PolicySet, Workspace};

/// The metadata prefix that is reserved for gate overrides.
pub(crate) const PREFIX: &str = policy::GATE_PREFIX;

/// The selected archive's stored overrides as `(key, value)` rows.
fn rows(conn: &Connection) -> Result<Vec<(String, String)>> {
    let mut statement =
        conn.prepare("SELECT key, value FROM metadata WHERE key LIKE 'gate.%' ORDER BY key")?;
    let rows = statement
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// The archive's effective policy: the checker's defaults plus these rows. A
/// stored row this binary cannot read is an error naming the row, never a
/// silently weaker default (design 6.1, S03-AC02).
pub(crate) fn load(conn: &Connection) -> Result<PolicySet> {
    let ready: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'metadata'",
        [],
        |row| row.get(0),
    )?;
    if ready == 0 {
        bail!("the archive is not initialized; run `zt up`");
    }
    let pairs = rows(conn)?;
    PolicySet::from_pairs(pairs).map_err(|error| anyhow!(error))
}

/// The revision a policy change advances, so a check-contract reader sees it.
pub(crate) fn revision(conn: &Connection) -> Result<u64> {
    link_index::revision(conn)
}

/// One accepted policy change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Outcome {
    pub(crate) workspace: Workspace,
    pub(crate) rule: String,
    pub(crate) change: Change,
    /// Whether a metadata row was written or removed. A repeated request writes
    /// nothing and moves no revision.
    pub(crate) written: bool,
}

impl Outcome {
    /// The line `zt config set gate.…` prints.
    pub(crate) fn line(&self) -> String {
        let setting = match self.change {
            Change::Set(setting) => setting.as_str(),
            Change::Reset => "default",
        };
        format!(
            "{}={setting}",
            policy::gate_key(self.workspace, &self.rule)
        )
    }
}

/// Validate and apply one `gate.<workspace>.<rule>` change in one transaction.
///
/// Validation runs before any write, so a typo, an invariant, a pending rule, or
/// a graph-only blocking override leaves the metadata and the revision exactly
/// as they were.
pub(crate) fn change(conn: &mut Connection, key: &str, value: &str) -> Result<Outcome> {
    let (workspace, rule) = policy::parse_gate_key(key).map_err(|error| anyhow!(error))?;
    let change = Change::parse(value).map_err(|error| anyhow!(error))?;
    let mut policies = load(conn)?;
    let written = policies
        .change(workspace, rule, change)
        .map_err(|error| anyhow!(error))?;
    if written {
        let tx = conn.unchecked_transaction()?;
        match change {
            Change::Set(setting) => {
                tx.execute(
                    "INSERT INTO metadata(key, value) VALUES(?1, ?2)
                     ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                    [key, setting.as_str()],
                )?;
            }
            Change::Reset => {
                tx.execute("DELETE FROM metadata WHERE key = ?1", [key])?;
            }
        }
        link_index::bump_revision(&tx)?;
        tx.commit()?;
    }
    Ok(Outcome {
        workspace,
        rule: rule.to_string(),
        change,
        written,
    })
}

/// The `zt config show` policy lines: the archive they belong to, the policy
/// hash and revision, every effective override, and their count (R0118).
pub(crate) fn summary_lines(root: &Path, policies: &PolicySet, revision: u64) -> Vec<String> {
    let mut lines = vec![
        format!("policy_archive={}", root.display()),
        format!("policy_hash={}", policies.hash()),
        format!("policy_revision={revision}"),
    ];
    for (workspace, rule, setting) in policies.overrides() {
        lines.push(format!(
            "{}={setting}",
            policy::gate_key(workspace, rule)
        ));
    }
    let count = policies.overrides().len();
    lines.push(format!(
        "overrides: {count} (a missing override means the checker's default; `zt config policy` shows every rule)"
    ));
    lines
}

/// The `zt config policy` table: every catalogued rule, its effective setting in
/// both Workspaces, the scope and the state. A `*` marks an explicit archive
/// override.
pub(crate) fn table_lines(policies: &PolicySet) -> Vec<String> {
    let mut lines = vec!["rule  human   agent   scope  state".to_string()];
    for spec in policy::catalog() {
        let cell = |workspace: Workspace| {
            if spec.state == policy::RuleState::Mandatory {
                return "-".to_string();
            }
            let policy = policies.policy(workspace);
            let setting = policy
                .effective(spec.rule)
                .expect("every catalogued rule has a setting");
            // An alias has no separate override: its setting is the named rule's.
            if spec.alias_of.is_none() && policy.is_override(spec.rule) {
                format!("{setting}*")
            } else {
                setting.as_str().to_string()
            }
        };
        let state = match spec.alias_of {
            Some(target) => format!("alias {target}"),
            None => spec.state.as_str().to_string(),
        };
        lines.push(format!(
            "{:<5} {:<7} {:<7} {:<6} {}",
            spec.rule,
            cell(Workspace::Human),
            cell(Workspace::Agent),
            spec.scope.as_str(),
            state
        ));
    }
    lines.push(
        "* = an explicit archive override; every other value is the checker's default".to_string(),
    );
    lines.push(
        "pending = catalogued for a later slice; not enforced and not configurable".to_string(),
    );
    lines.push(
        "mandatory = an existing service invariant; no gate policy can change it".to_string(),
    );
    lines.push(
        "alias <rule> = the same predicate under another label; configure the named rule".to_string(),
    );
    lines.push(format!(
        "invariants: {}",
        policy::invariants()
            .iter()
            .map(|invariant| invariant.name)
            .collect::<Vec<_>>()
            .join(", ")
    ));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use zt_check::policy::Setting;

    /// A fresh isolated archive, as every test suite's own scratch box.
    fn archive() -> (tempfile::TempDir, Connection) {
        let temp = tempfile::tempdir().expect("temp dir");
        crate::initialize_database(temp.path()).expect("initialize database");
        let conn = crate::open_db(temp.path()).expect("open database");
        (temp, conn)
    }

    fn overrides(conn: &Connection) -> Vec<(String, String)> {
        rows(conn).expect("override rows")
    }

    /// S03-AC01/S03-AC02: one override is one metadata row in the selected
    /// archive; setting or resetting it moves the revision once, and every
    /// refusal writes no row and moves no revision.
    #[test]
    fn an_override_is_persisted_in_the_metadata_and_moves_the_revision() {
        let (_temp, mut conn) = archive();
        let before = revision(&conn).expect("revision");
        let default_hash = load(&conn).expect("policy").hash();
        assert!(overrides(&conn).is_empty());
        assert_eq!(
            load(&conn)
                .expect("policy")
                .policy(Workspace::Human)
                .effective("R2"),
            Some(Setting::Advise),
            "the human default is advice"
        );

        let outcome = change(&mut conn, "gate.human.R2", "block").expect("set");
        assert_eq!(outcome.line(), "gate.human.R2=block");
        assert!(outcome.written);
        assert_eq!(
            overrides(&conn),
            [("gate.human.R2".to_string(), "block".to_string())]
        );
        assert_eq!(revision(&conn).expect("revision"), before + 1);
        let set = load(&conn).expect("policy");
        assert_eq!(
            set.policy(Workspace::Human).effective("R2"),
            Some(Setting::Block)
        );
        assert_ne!(set.hash(), default_hash, "a policy change gets a new hash");
        assert_eq!(
            set.policy(Workspace::Agent).effective("R2"),
            Some(Setting::Block),
            "the other Workspace keeps its own default"
        );

        // A repeated value writes nothing and moves no revision.
        let outcome = change(&mut conn, "gate.human.R2", "block").expect("repeat");
        assert!(!outcome.written);
        assert_eq!(revision(&conn).expect("revision"), before + 1);

        // An explicit reset removes the row and moves the revision once.
        let outcome = change(&mut conn, "gate.human.R2", "default").expect("reset");
        assert_eq!(outcome.line(), "gate.human.R2=default");
        assert!(outcome.written);
        assert!(overrides(&conn).is_empty());
        assert_eq!(revision(&conn).expect("revision"), before + 2);
        assert_eq!(load(&conn).expect("policy").hash(), default_hash);

        // A refused change writes no row and moves no revision.
        let cases = [
            ("gate.human.R2", "warn"),
            ("gate.human.R1", "block"),
            ("gate.human.access", "off"),
            ("gate.human.envelope", "off"),
            ("gate.human.T4", "block"),
            ("gate.human.T10", "block"),
            ("gate.aliens.R2", "block"),
            ("gate.agent.I5", "block"),
        ];
        for (key, value) in cases {
            let error = change(&mut conn, key, value)
                .expect_err(&format!("{key}={value} must be refused"))
                .to_string();
            assert!(!error.is_empty(), "{key}");
            assert!(overrides(&conn).is_empty(), "{key} wrote a row");
            assert_eq!(
                revision(&conn).expect("revision"),
                before + 2,
                "{key} moved the revision"
            );
        }
        let error = change(&mut conn, "gate.human.R1", "block")
            .expect_err("unknown rule")
            .to_string();
        assert!(error.contains("unknown rule `R1`"), "{error}");
        let error = change(&mut conn, "gate.human.access", "off")
            .expect_err("invariant")
            .to_string();
        assert!(error.contains("mandatory service invariant"), "{error}");
        let error = change(&mut conn, "gate.agent.I5", "block")
            .expect_err("graph block")
            .to_string();
        assert!(error.contains("graph-only audit rule"), "{error}");

        // A graph rule may be silenced and restored without any graph scan.
        let outcome = change(&mut conn, "gate.agent.I5", "off").expect("silence a graph rule");
        assert!(outcome.written);
        assert_eq!(
            load(&conn)
                .expect("policy")
                .policy(Workspace::Agent)
                .effective("I5"),
            Some(Setting::Off)
        );
        change(&mut conn, "gate.agent.I5", "default").expect("restore");
    }

    /// S03-AC05: two archives hold their own rows and revision; changing one
    /// changes neither the other's policy nor its revision.
    #[test]
    fn two_archives_keep_their_own_policy_and_revision() {
        let (_a_temp, mut a) = archive();
        let (_b_temp, b) = archive();
        let (a_revision, b_revision) = (
            revision(&a).expect("revision"),
            revision(&b).expect("revision"),
        );
        let b_hash = load(&b).expect("policy").hash();
        change(&mut a, "gate.agent.R6", "off").expect("set a");
        assert_eq!(overrides(&a).len(), 1);
        assert!(overrides(&b).is_empty());
        assert_eq!(load(&b).expect("policy").hash(), b_hash);
        assert_eq!(revision(&b).expect("revision"), b_revision);
        assert_eq!(revision(&a).expect("revision"), a_revision + 1);
    }

    /// S03-AC07/R0120: a policy change rewrites no Card and saves no legacy
    /// baseline. It writes the metadata row and advances the revision, nothing
    /// else.
    #[test]
    fn a_policy_change_rewrites_no_card_and_saves_no_baseline() {
        let (_temp, mut conn) = archive();
        conn.execute(
            "INSERT INTO cards(location, is_topic, is_lit, text, workspace, card_type)
             VALUES('1/0', 1, 0, '# 主题\n\n正文。\n', 'agent', 'idea')",
            [],
        )
        .expect("insert a Card");
        let count = |conn: &Connection| -> i64 {
            conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
                .expect("count")
        };
        let cards = count(&conn);
        let revision_before = revision(&conn).expect("revision");
        change(&mut conn, "gate.agent.R6", "block").expect("set");
        assert_eq!(count(&conn), cards, "no Card is created or removed");
        let text: String = conn
            .query_row("SELECT text FROM cards WHERE location = '1/0'", [], |row| row.get(0))
            .expect("stored text");
        assert_eq!(text, "# 主题\n\n正文。\n", "no Card is rewritten");
        assert_eq!(revision(&conn).expect("revision"), revision_before + 1);
        let baselines: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table'
                 AND (name LIKE '%baseline%' OR name LIKE '%fault%')",
                [],
                |row| row.get(0),
            )
            .expect("schema");
        assert_eq!(baselines, 0, "no legacy-fault baseline table is created");
        let gate_rows: i64 = conn
            .query_row("SELECT COUNT(*) FROM metadata WHERE key LIKE 'gate.%'", [], |row| row.get(0))
            .expect("metadata");
        assert_eq!(gate_rows, 1, "one metadata row holds the override");
    }

    /// S03-AC01/R0118: the config output names the archive, the policy hash, the
    /// revision, and every effective override; the full table shows the defaults,
    /// the pending Term rules, the mandatory rule, and the graph scope.
    #[test]
    fn the_policy_output_names_the_archive_its_overrides_and_the_defaults() {
        let (_temp, mut conn) = archive();
        change(&mut conn, "gate.agent.R2", "advise").expect("set");
        let policies = load(&conn).expect("policy");
        let summary = summary_lines(
            Path::new("D:/box/archive"),
            &policies,
            revision(&conn).expect("revision"),
        );
        assert_eq!(summary[0], "policy_archive=D:/box/archive");
        let hash = summary
            .iter()
            .find(|line| line.starts_with("policy_hash="))
            .expect("hash");
        assert_eq!(hash.len(), "policy_hash=".len() + 64, "{hash}");
        assert!(
            summary
                .iter()
                .any(|line| line.starts_with("policy_revision=")),
            "{summary:#?}"
        );
        assert!(summary.contains(&"gate.agent.R2=advise".to_string()), "{summary:#?}");
        assert!(summary.last().expect("footer").starts_with("overrides: 1"), "{summary:#?}");

        let table = table_lines(&policies);
        let header = table.first().expect("header");
        assert!(header.contains("human") && header.contains("agent"), "{header}");
        let row = |rule: &str| {
            table
                .iter()
                .find(|line| line.starts_with(&format!("{rule} ")))
                .unwrap_or_else(|| panic!("{rule} is in the table: {table:#?}"))
                .clone()
        };
        assert!(row("R2").contains("advise*"), "{}", row("R2"));
        assert!(!row("R2").contains("block"), "{}", row("R2"));
        assert!(row("E2").contains("block") && !row("E2").contains('*'), "{}", row("E2"));
        assert!(row("R6").contains("off") && row("R6").contains("advise"), "{}", row("R6"));
        assert!(row("I5").contains("graph"), "{}", row("I5"));
        assert!(row("T4").contains("pending"), "{}", row("T4"));
        assert!(row("T7").contains("mandatory") && !row("T7").contains("block"), "{}", row("T7"));
        assert!(row("T10").contains("alias R22") && !row("T10").contains('*'), "{}", row("T10"));
        assert!(table.iter().any(|line| line.starts_with("invariants: ")), "{table:#?}");
    }
}
