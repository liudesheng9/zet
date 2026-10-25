//! The published check contract (design 8.3, S06 / #148).
//!
//! A check answer is only meaningful together with the rules, policy, limits,
//! and archive it was computed under. This module owns those facts as one
//! process-free value: the running service publishes it beside an archive read
//! (`get_cards`), a draft states what it binds, and the command line refuses to
//! present an answer whose contract is not the one that would decide the write.
//!
//! The rule version is independent of the executable version (R0197): a change
//! to rule semantics, to the visible-text projection, to the limits, or to the
//! diagnostic schema bumps it. Unrelated CLI or packaging changes bump only the
//! crate version, which the contract publishes separately as `checker_version`.

use crate::core::Limits;
use crate::policy::{PolicySet, Setting, Workspace};
use serde_json::{Map, Value, json};

/// The rule semantics, visible-text semantics, limits, and diagnostic schema of
/// this checker. A live check compares it with the running service's; a
/// mismatch never becomes a successful acceptance prediction (R0206).
pub const RULE_VERSION: &str = "1";

/// The version of the published diagnostic shape: the eight fields of
/// `core::Diagnostic::to_json` and the `block`/`advise` and `local`/`graph`
/// names (design 5.4). A new field or a renamed value bumps it.
pub const DIAGNOSTIC_SCHEMA: &str = "1";

/// The version of the rule-applicability signatures `policy::Applicability`
/// reads (design 6.3). Adding or changing a rule's signature bumps it, because
/// a legacy allowance decided by another signature set is a different decision.
pub const APPLICABILITY_VERSION: &str = "1";

/// The effective policy came from the archive that would decide the write.
pub const POLICY_SOURCE_ARCHIVE: &str = "archive";

/// The effective policy is the checker's built-in default: local conformance
/// only, never an acceptance prediction against unknown archive overrides
/// (R0196).
pub const POLICY_SOURCE_BUILT_IN: &str = "built_in";

/// The check families a draft bundle can declare complete (design 8.2). A
/// family a bundle does not declare is one this answer does not cover, so the
/// answer is incomplete rather than a pass (US021).
pub const FAMILIES: [&str; 5] = ["envelope", "local", "graph", "placement", "policy"];

/// What the archive is and where it stands: its identity, path, revision, and
/// write-gate stage (design 8.3, R0203).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveFacts {
    pub id: String,
    pub path: String,
    pub revision: u64,
    pub stage: String,
}

/// The relevant syntax capabilities of the running service (R0204). A draft that
/// uses Term syntax is not checked by a service that does not understand it, so
/// the answer says so instead of passing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub term_links: bool,
    pub term_cards: bool,
}

impl Capabilities {
    /// What this build implements. The Term slices (S07-S12) flip these.
    pub fn current() -> Self {
        Self {
            term_links: false,
            term_cards: false,
        }
    }

    fn to_json(&self) -> Value {
        json!({ "term_links": self.term_links, "term_cards": self.term_cards })
    }

    fn read(value: &Value) -> Option<Self> {
        Some(Self {
            term_links: value.get("term_links")?.as_bool()?,
            term_cards: value.get("term_cards")?.as_bool()?,
        })
    }
}

/// One check contract: the facts that decide whether two check answers are
/// comparable at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contract {
    pub rule_version: String,
    pub diagnostic_schema: String,
    pub applicability_version: String,
    /// The build that computed this answer. Informative: the rule version, not
    /// the executable version, decides comparability (R0197).
    pub checker_version: String,
    pub policy: PolicySet,
    pub policy_hash: String,
    pub limits: Limits,
    /// The archive's gate stage, empty when no archive answered.
    pub stage: String,
    /// `None` when the answer was computed offline.
    pub archive: Option<ArchiveFacts>,
    pub capabilities: Capabilities,
}

impl Contract {
    /// The contract of an answer the running service computed: its effective
    /// policy, the checker's limits, and the archive's identity and stage.
    pub fn served(policy: &PolicySet, limits: &Limits, archive: ArchiveFacts) -> Self {
        Self {
            rule_version: RULE_VERSION.to_string(),
            diagnostic_schema: DIAGNOSTIC_SCHEMA.to_string(),
            applicability_version: APPLICABILITY_VERSION.to_string(),
            checker_version: crate::VERSION.to_string(),
            policy: policy.clone(),
            policy_hash: policy.hash(),
            limits: limits.clone(),
            stage: archive.stage.clone(),
            archive: Some(archive),
            capabilities: Capabilities::current(),
        }
    }

    /// The contract of an offline answer: the checker's built-in defaults, its
    /// own limits, and no archive.
    pub fn built_in(limits: &Limits) -> Self {
        let policy = PolicySet::new();
        Self {
            rule_version: RULE_VERSION.to_string(),
            diagnostic_schema: DIAGNOSTIC_SCHEMA.to_string(),
            applicability_version: APPLICABILITY_VERSION.to_string(),
            checker_version: crate::VERSION.to_string(),
            policy_hash: policy.hash(),
            policy,
            limits: limits.clone(),
            stage: String::new(),
            archive: None,
            capabilities: Capabilities::current(),
        }
    }

    /// The published JSON form: the object `get_cards` carries as
    /// `check_contract` (design 8.3, R0199–R0204).
    pub fn publish(&self) -> Value {
        let mut value = json!({
            "rule_version": self.rule_version,
            "diagnostic_schema": self.diagnostic_schema,
            "applicability_version": self.applicability_version,
            "checker_version": self.checker_version,
            "policy": self.policy_json(),
            "policy_hash": self.policy_hash,
            "limits": self.limits.to_json(),
            "capabilities": self.capabilities.to_json(),
        });
        value["stage"] = match self.archive {
            Some(_) => json!(self.stage),
            None => Value::Null,
        };
        value["archive"] = match &self.archive {
            Some(archive) => json!({
                "id": archive.id,
                "path": archive.path,
                "revision": archive.revision,
            }),
            None => Value::Null,
        };
        value
    }

    /// The effective policy of both Workspaces, as the contract publishes it:
    /// every rule a policy can decide, with the setting that decides it.
    pub fn policy_json(&self) -> Value {
        let mut both = Map::new();
        for workspace in Workspace::ALL {
            let policy = self.policy.policy(workspace);
            let mut settings = Map::new();
            for spec in crate::policy::catalog() {
                if !spec.is_configurable() {
                    continue;
                }
                let Some(setting) = policy.effective(spec.rule) else {
                    continue;
                };
                settings.insert(spec.rule.to_string(), json!(setting.as_str()));
            }
            both.insert(workspace.as_str().to_string(), Value::Object(settings));
        }
        Value::Object(both)
    }

    /// Read a published contract. A reply that omits a required fact, or carries
    /// one this build cannot read, is refused: it is never replaced by a
    /// built-in default, because that would compare two different checkers.
    pub fn read(value: &Value) -> Result<Self, String> {
        let string = |name: &str| -> Result<String, String> {
            value
                .get(name)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| format!("the check contract names no `{name}`"))
        };
        let policy_hash = string("policy_hash")?;
        let policy = read_policy(&value["policy"])?;
        if policy.hash() != policy_hash {
            return Err(format!(
                "the check contract's policy hash {policy_hash} does not match its published policy {}",
                policy.hash()
            ));
        }
        let limits = Limits::from_json(&value["limits"])
            .map_err(|error| format!("the check contract's limits are unreadable: {error}"))?;
        let capabilities = Capabilities::read(&value["capabilities"])
            .ok_or_else(|| "the check contract names no term capabilities".to_string())?;
        let archive = match value.get("archive") {
            None | Some(Value::Null) => None,
            Some(archive) => Some(ArchiveFacts {
                id: archive["id"]
                    .as_str()
                    .ok_or_else(|| "the check contract's archive names no id".to_string())?
                    .to_string(),
                path: archive["path"]
                    .as_str()
                    .ok_or_else(|| "the check contract's archive names no path".to_string())?
                    .to_string(),
                revision: archive["revision"]
                    .as_u64()
                    .ok_or_else(|| "the check contract's archive names no revision".to_string())?,
                stage: value
                    .get("stage")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }),
        };
        Ok(Self {
            rule_version: string("rule_version")?,
            diagnostic_schema: string("diagnostic_schema")?,
            applicability_version: string("applicability_version")?,
            checker_version: value
                .get("checker_version")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            policy,
            policy_hash,
            limits,
            stage: value
                .get("stage")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            archive,
            capabilities,
        })
    }

    /// How this build's rule contract differs from the published one. Empty when
    /// the answer is comparable with what this binary would compute (R0206).
    pub fn version_gaps(&self) -> Vec<String> {
        let mut gaps = Vec::new();
        for (name, ours, theirs) in [
            ("rule_version", RULE_VERSION, self.rule_version.as_str()),
            (
                "diagnostic_schema",
                DIAGNOSTIC_SCHEMA,
                self.diagnostic_schema.as_str(),
            ),
            (
                "applicability_version",
                APPLICABILITY_VERSION,
                self.applicability_version.as_str(),
            ),
        ] {
            if ours != theirs {
                gaps.push(format!("{name}: this zt-check uses {ours}, the published contract says {theirs}"));
            }
        }
        gaps
    }

    /// The limits this run would use that the contract does not publish. A
    /// deviation is an incomplete check of the gate's decision, never a pass.
    pub fn limit_gaps(&self, limits: &Limits) -> Vec<String> {
        let mine = limits.to_json();
        let theirs = self.limits.to_json();
        let mut gaps = Vec::new();
        for (name, ours) in mine.as_object().into_iter().flatten() {
            if theirs.get(name) != Some(ours) {
                gaps.push(format!(
                    "{name}: this run uses {}, the published limit is {}",
                    ours, theirs[name]
                ));
            }
        }
        gaps
    }

    /// Whether this contract's policy is the checker's built-in default.
    pub fn is_built_in_policy(&self) -> bool {
        self.policy_hash == PolicySet::new().hash()
    }
}

/// Read the published per-Workspace effective policy back into a `PolicySet`.
/// Every published rule is a validated gate change, so an unreadable or
/// unconfigurable rule refuses the whole contract.
fn read_policy(value: &Value) -> Result<PolicySet, String> {
    let mut pairs = Vec::new();
    for workspace in Workspace::ALL {
        let Some(settings) = value.get(workspace.as_str()).and_then(Value::as_object) else {
            return Err(format!(
                "the check contract publishes no {} policy",
                workspace.as_str()
            ));
        };
        for (rule, setting) in settings {
            let setting = setting
                .as_str()
                .and_then(Setting::parse)
                .ok_or_else(|| format!("the check contract's {}.{rule} is no setting", workspace.as_str()))?;
            pairs.push((
                crate::policy::gate_key(workspace, rule),
                setting.as_str().to_string(),
            ));
        }
    }
    PolicySet::from_pairs(pairs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive() -> ArchiveFacts {
        ArchiveFacts {
            id: "a".repeat(32),
            path: "D:\\zt".to_string(),
            revision: 7921,
            stage: "advisory".to_string(),
        }
    }

    fn policy() -> PolicySet {
        PolicySet::from_pairs([(
            "gate.agent.R2".to_string(),
            "advise".to_string(),
        )])
        .expect("a valid override")
    }

    #[test]
    fn a_served_contract_publishes_the_facts_that_decide_a_check() {
        let contract = Contract::served(&policy(), &Limits::default(), archive());
        let value = contract.publish();
        assert_eq!(value["rule_version"], json!(RULE_VERSION));
        assert_eq!(value["diagnostic_schema"], json!(DIAGNOSTIC_SCHEMA));
        assert_eq!(value["applicability_version"], json!(APPLICABILITY_VERSION));
        assert_eq!(value["checker_version"], json!(crate::VERSION));
        assert_eq!(value["policy_hash"], json!(policy().hash()));
        assert_eq!(value["policy"]["agent"]["R2"], json!("advise"));
        assert_eq!(value["policy"]["human"]["R2"], json!("advise"));
        assert_eq!(value["policy"]["agent"]["R6"], json!("advise"));
        assert_eq!(value["limits"]["max_sentences"], json!(12));
        assert_eq!(value["stage"], json!("advisory"));
        assert_eq!(value["archive"]["id"], json!("a".repeat(32)));
        assert_eq!(value["archive"]["path"], json!("D:\\zt"));
        assert_eq!(value["archive"]["revision"], json!(7921));
        assert_eq!(value["capabilities"]["term_links"], json!(false));
        assert_eq!(value["capabilities"]["term_cards"], json!(false));
    }

    #[test]
    fn a_published_contract_reads_back_to_the_same_facts() {
        let served = Contract::served(&policy(), &Limits::default(), archive());
        let read = Contract::read(&served.publish()).expect("a published contract reads back");
        assert_eq!(read.rule_version, RULE_VERSION);
        assert_eq!(read.policy_hash, policy().hash());
        assert_eq!(read.limits, Limits::default());
        assert_eq!(read.archive, Some(archive()));
        assert_eq!(read.stage, "advisory");
        assert!(read.version_gaps().is_empty());
        assert!(read.limit_gaps(&Limits::default()).is_empty());
    }

    #[test]
    fn a_contract_without_a_required_fact_is_refused_rather_than_defaulted() {
        let served = Contract::served(&policy(), &Limits::default(), archive());
        for missing in [
            "rule_version",
            "diagnostic_schema",
            "applicability_version",
            "policy_hash",
            "policy",
            "limits",
            "capabilities",
        ] {
            let mut value = served.publish();
            value.as_object_mut().unwrap().remove(missing);
            assert!(
                Contract::read(&value).is_err(),
                "a contract without `{missing}` must be refused"
            );
        }
    }

    #[test]
    fn a_tampered_policy_or_unknown_setting_is_refused() {
        let served = Contract::served(&policy(), &Limits::default(), archive());
        let mut value = served.publish();
        value["policy_hash"] = json!("0".repeat(64));
        let error = Contract::read(&value).expect_err("a hash that does not match is refused");
        assert!(error.contains("does not match"), "{error}");

        let mut value = served.publish();
        value["policy"]["agent"]["R2"] = json!("warn");
        assert!(Contract::read(&value).is_err());
    }

    #[test]
    fn a_built_in_contract_names_its_own_policy_hash_and_no_archive() {
        let contract = Contract::built_in(&Limits::default());
        let value = contract.publish();
        assert_eq!(value["policy_hash"], json!(PolicySet::new().hash()));
        assert_eq!(value["archive"], Value::Null);
        assert_eq!(value["stage"], Value::Null);
        assert!(contract.is_built_in_policy());
        assert!(!Contract::served(&policy(), &Limits::default(), archive()).is_built_in_policy());
    }

    #[test]
    fn a_version_difference_names_the_fact_that_differs() {
        let mut served = Contract::served(&policy(), &Limits::default(), archive());
        served.rule_version = "0".to_string();
        served.diagnostic_schema = "2".to_string();
        let gaps = served.version_gaps();
        assert_eq!(gaps.len(), 2, "{gaps:?}");
        assert!(gaps[0].contains("rule_version: this zt-check uses 1"));
        assert!(gaps[0].contains("says 0"));
        assert!(gaps[1].contains("diagnostic_schema: this zt-check uses 1"));
        assert!(gaps[1].contains("says 2"));
    }

    #[test]
    fn a_limit_difference_names_the_limit_that_differs() {
        let mut served = Contract::served(&policy(), &Limits::default(), archive());
        served.limits.max_chars = 300;
        let mut limits = Limits::default();
        limits.max_chars = 320;
        limits.min_reason = 6;
        let gaps = served.limit_gaps(&limits);
        assert_eq!(gaps.len(), 2, "{gaps:?}");
        assert!(gaps.iter().any(|gap| gap.contains("max_chars: this run uses 320")));
        assert!(gaps.iter().any(|gap| gap.contains("min_reason: this run uses 6")));
    }
}
