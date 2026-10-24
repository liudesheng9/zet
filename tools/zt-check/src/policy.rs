//! The rule catalog and report policy: which rules a `--cards` report prints and
//! counts, the default severity and scope of every rule, and the legacy ratchet
//! that keeps an old Card editable without grandfathering a newly introduced
//! fault (design 5.4 and 6.3). Formatting and identity stay in the core.

use crate::core::{Diagnostic, Scope, Severity, SUBSTANTIVE};
use crate::text::{pattern, prop, ps_eq, ps_in, ps_string, text};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

/// The rules self-check.ps1 reports by default: every hard rule but R6.
pub const DEFAULT_RULES: &str =
    "R2,R3,R4,R5,R7,R9,R12,R13,R14,R15,R16,R17,R20,R21,R22,I1,I2,I3,I4,I7,L1,L2,E1,E2,F1,F2,F3";

/// The advisory rules of the current default policy: every line they raise is
/// advice. I7 is not here: it has one advisory subcase (`own_source_unknown`)
/// and one blocking one, so the core marks that subcase itself.
pub const ADVISORY_RULES: &[&str] = &["R20", "R21", "I1", "I6", "L3", "E5"];

/// The whole-archive graph rules: reported by the audit, not decided per write
/// (design 6.2). Their scope names that boundary in every structured report.
pub const GRAPH_RULES: &[&str] = &["I5", "I6", "E4", "E5", "F4", "L3"];

/// The default severity of one rule before an archive policy overrides it (S03).
pub fn severity(rule: &str) -> Severity {
    if ps_in(rule, ADVISORY_RULES) {
        Severity::Advise
    } else {
        Severity::Block
    }
}

/// What evidence a rule reads under the default policy.
pub fn scope(rule: &str) -> Scope {
    if ps_in(rule, GRAPH_RULES) {
        Scope::Graph
    } else {
        Scope::Local
    }
}

/// The assembled result of one Card check: every diagnostic in the core's
/// traversal order, and the number that still block after the legacy ratchet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckReport {
    pub diagnostics: Vec<Diagnostic>,
    pub blockers: usize,
}

impl CheckReport {
    /// A report with no legacy allowance: a new Card, or an audit run. Blocking
    /// diagnostics block, advisory diagnostics stay advice (design 6.3).
    pub fn of(diagnostics: Vec<Diagnostic>) -> Self {
        let blockers = diagnostics.iter().filter(|diagnostic| diagnostic.blocking()).count();
        Self {
            diagnostics,
            blockers,
        }
    }

    /// The familiar lines, in the report's order.
    pub fn lines(&self) -> Vec<String> {
        self.diagnostics
            .iter()
            .map(Diagnostic::line)
            .collect::<Vec<_>>()
    }
}

/// The facts that decide whether one rule applies to a Card and means the same
/// thing after a metadata change (design 6.3, R0146). A rule is compared only
/// when its signature is unchanged; the old version is read with its own
/// metadata, so a retyped Card must satisfy the requirements its type makes
/// applicable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applicability {
    category: String,
    level: String,
    part: bool,
    derivation: bool,
    shared: bool,
    book: String,
    term: String,
}

impl Applicability {
    /// The facts one entry declares about itself. An entry that names no type,
    /// level, or book is compared by what it does say: the bundle contract is
    /// explicit that absent context is not a pass.
    pub fn of(entry: &Value) -> Self {
        let facet = text(prop(entry, "facet"));
        Self {
            category: ps_string(prop(entry, "category")).to_lowercase(),
            level: ps_string(prop(entry, "level")).to_lowercase(),
            part: !facet.is_empty(),
            derivation: crate::core::is_derivation_facet(&facet),
            shared: ps_eq(&text(prop(entry, "line")), "shared"),
            book: ps_string(prop(entry, "book")),
            // The future Term rules decide their role from this authored name;
            // keeping it here makes an unnamed term rule follow a term change.
            term: ps_string(prop(entry, "term")),
        }
    }

    /// Whether `rule` has the same applicability and meaning for both versions.
    pub fn comparable(&self, proposed: &Applicability, rule: &str) -> bool {
        self.signature(rule) == proposed.signature(rule)
    }

    fn signature(&self, rule: &str) -> String {
        match rule {
            // The plain text rules read the authored text the same way whatever
            // the Card's type is: a moved or retyped Card keeps those faults.
            "R2" | "R3" | "R4" | "R5" | "R7" | "R9" | "R12" | "R18" | "R19" | "R20" | "R21"
            | "R22" => "text".to_string(),
            // A shared part follows its home model's term table; leaving the
            // shared list or changing the table changes what R6 means.
            "R6" => format!("terms:{}", self.shared),
            // The model rules apply by facet, derivation, and formal level.
            "R13" => format!("part:{}:{}", self.part, self.level),
            "R14" | "R15" | "R16" | "R17" => {
                format!("derivation:{}:{}", self.derivation, self.level)
            }
            // The Idea rules apply to an Idea or an anchor.
            "I1" => format!("claim:{}", self.in_category(&["idea", "anchor"])),
            "I2" | "I4" | "I7" => format!("idea:{}", self.in_category(&["idea"])),
            "I3" => format!(
                "source:{}:{}",
                self.in_category(&["idea", "event", "restatement-entry"]),
                self.book
            ),
            "I5" => format!("substantive:{}", self.in_category(SUBSTANTIVE)),
            // The Event and Finding rules apply by type; F2 and I3 also read the
            // book they name, so another book is another requirement.
            "E1" | "E2" | "E4" | "E5" => {
                format!("event:{}", self.in_category(&["event"]))
            }
            "F1" | "F3" | "F4" => format!("finding:{}", self.in_category(&["finding"])),
            "F2" => format!(
                "source:{}:{}",
                self.in_category(&["finding"]),
                self.book
            ),
            "L1" | "L2" | "L3" => format!(
                "layer:{}",
                self.in_category(&[
                    "idea",
                    "anchor",
                    "event",
                    "finding",
                    "restatement-entry",
                ])
            ),
            // A rule this policy does not name keeps the conservative treatment:
            // it is compared only when every metadata fact is unchanged, so a new
            // rule cannot inherit across a retype, a shared-list, book or term
            // transition before its own signature is named here.
            _ => format!(
                "other:{}:{}:{}:{}:{}:{}:{}",
                self.category,
                self.level,
                self.part,
                self.derivation,
                self.shared,
                self.book,
                self.term
            ),
        }
    }

    fn in_category(&self, names: &[&str]) -> bool {
        names.contains(&self.category.as_str())
    }
}

/// Mark one proposed report against the report of the same Cards' old authored
/// versions (design 6.3). Every comparable old occurrence is consumed once, in
/// multiplicity, by an occurrence with the same `(address, rule, fault, key)`;
/// a consumed diagnostic becomes `legacy: true` advice. An occurrence the old
/// version did not carry keeps its severity and can block.
///
/// `comparable` answers whether a rule keeps its applicability and meaning
/// across this Card's metadata change; a new Card passes a closure that answers
/// `false` for every rule.
pub fn ratchet(
    before: &[Diagnostic],
    proposed: Vec<Diagnostic>,
    comparable: impl Fn(&Diagnostic) -> bool,
) -> CheckReport {
    let mut old: HashMap<(String, String, String, String), usize> = HashMap::new();
    for diagnostic in before {
        *old.entry(identity(diagnostic)).or_default() += 1;
    }
    let mut remaining = proposed;
    for diagnostic in &mut remaining {
        if !comparable(diagnostic) {
            continue;
        }
        if let Some(count) = old.get_mut(&identity(diagnostic))
            && *count > 0
        {
            *count -= 1;
            diagnostic.legacy = true;
            diagnostic.severity = Severity::Advise;
        }
    }
    CheckReport::of(remaining)
}

fn identity(diagnostic: &Diagnostic) -> (String, String, String, String) {
    (
        diagnostic.address.clone(),
        diagnostic.rule.clone(),
        diagnostic.fault.clone(),
        diagnostic.key.clone(),
    )
}

// self-check.ps1.
pattern!(FAULT_LINE, r"^\S+ ([RIELF]\d+) ");

pattern!(LIST_SPLIT, r"[,\s]+");

/// The lines of the rules asked for; advisory lines are shown but not counted.
pub fn filter_rules(lines: &[String], rules: &str) -> (Vec<String>, usize) {
    let allowed = LIST_SPLIT
        .split(rules)
        .filter(|rule| !rule.is_empty())
        .collect::<Vec<_>>();
    let mut kept = Vec::new();
    let mut faults = 0;
    for line in lines {
        let Some(caps) = FAULT_LINE.captures(line) else {
            continue;
        };
        if !ps_in(&caps[1], &allowed) {
            continue;
        }
        kept.push(line.clone());
        if !line.contains("(advisory)") {
            faults += 1;
        }
    }
    (kept, faults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn diagnostic(address: &str, rule: &str, fault: &str, key: &str, message: &str) -> Diagnostic {
        Diagnostic {
            address: address.to_string(),
            rule: rule.to_string(),
            fault: fault.to_string(),
            key: key.to_string(),
            message: message.to_string(),
            severity: severity(rule),
            legacy: false,
            scope: scope(rule),
        }
    }

    #[test]
    fn the_rule_filter_keeps_asked_rules_and_counts_hard_faults() {
        let lines = vec![
            "a/1 R6 uses x".to_string(),
            "a/1 R3 x；y".to_string(),
            "a/1 I1 (advisory) the title states no claim: x".to_string(),
        ];
        let (kept, count) = filter_rules(&lines, DEFAULT_RULES);
        assert_eq!(
            kept,
            [
                "a/1 R3 x；y",
                "a/1 I1 (advisory) the title states no claim: x"
            ]
        );
        assert_eq!(count, 1);
    }

    /// S02-AC02/AC06: a fault the old authored version also carried becomes
    /// legacy advice and stops blocking; a new Card has no allowance at all.
    #[test]
    fn a_retained_fault_becomes_legacy_advice_and_stops_blocking() {
        let before = [diagnostic("K/1", "R3", "semicolon", "甲；乙。", "甲；乙。")];
        let report = ratchet(&before, before.to_vec(), |_| true);
        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0].legacy);
        assert_eq!(report.diagnostics[0].severity, Severity::Advise);
        assert_eq!(report.blockers, 0);
        assert_eq!(report.lines(), ["K/1 R3 (advisory) 甲；乙。"]);

        // A new Card has no old version, so the same fault still blocks.
        let fresh = ratchet(&[], before.to_vec(), |_| true);
        assert!(!fresh.diagnostics[0].legacy);
        assert_eq!(fresh.blockers, 1);
    }

    /// S02-AC03: a second copy of an already faulty sentence is a new occurrence,
    /// while removing one of two old copies leaves the remaining one inherited.
    #[test]
    fn the_ratchet_counts_occurrences_not_merely_identities() {
        let one = diagnostic("K/1", "R3", "semicolon", "甲；乙。", "甲；乙。");
        let twice = [one.clone(), one.clone()];
        let duplicated = ratchet(&[one.clone()], twice.to_vec(), |_| true);
        assert!(duplicated.diagnostics[0].legacy);
        assert!(!duplicated.diagnostics[1].legacy);
        assert_eq!(duplicated.diagnostics[1].severity, Severity::Block);
        assert_eq!(duplicated.blockers, 1);

        let removed = ratchet(&twice, vec![one.clone()], |_| true);
        assert!(removed.diagnostics[0].legacy);
        assert_eq!(removed.blockers, 0);
    }

    /// S02-AC03: identity is the semantic key, not the printed line. A paragraph
    /// moved to another line keeps its fault; two different fragments that clip
    /// to the same prefix stay distinct.
    #[test]
    fn the_ratchet_compares_semantic_keys_not_printed_lines() {
        let moved = ratchet(
            &[diagnostic(
                "K/1",
                "R18",
                "repeated_prose",
                "同一段。",
                "repeated prose paragraph at lines 2-2 and 6-6",
            )],
            vec![diagnostic(
                "K/1",
                "R18",
                "repeated_prose",
                "同一段。",
                "repeated prose paragraph at lines 4-4 and 9-9",
            )],
            |_| true,
        );
        assert!(moved.diagnostics[0].legacy);
        assert_eq!(moved.blockers, 0);

        let prefix = "同".repeat(88);
        let old = diagnostic(
            "K/1",
            "R2",
            "sentence_length",
            &format!("41 characters > 40: {prefix}甲和乙"),
            &format!("(41 characters > 40) {prefix}甲和乙"),
        );
        let new = diagnostic(
            "K/1",
            "R2",
            "sentence_length",
            &format!("41 characters > 40: {prefix}丙和丁"),
            &format!("(41 characters > 40) {prefix}丙和丁"),
        );
        assert_eq!(old.line().len(), new.line().len());
        let distinct = ratchet(&[old], vec![new], |_| true);
        assert!(!distinct.diagnostics[0].legacy);
        assert_eq!(distinct.blockers, 1);
    }

    /// S02-AC05: a rule whose applicability and meaning changed is not
    /// comparable, so a newly relevant requirement cannot be called legacy.
    #[test]
    fn applicability_signatures_grandfather_only_unchanged_meaning() {
        let idea = Applicability::of(&json!({ "category": "idea" }));
        let event = Applicability::of(&json!({ "category": "event" }));
        assert!(!idea.comparable(&event, "E2"), "Event rules are newly applicable");
        assert!(!idea.comparable(&event, "I2"), "Idea rules no longer apply");
        assert!(idea.comparable(&event, "R3"), "the text rules mean the same");

        let concept = Applicability::of(&json!({ "category": "part", "facet": "推导", "level": "concept" }));
        let formal = Applicability::of(&json!({ "category": "part", "facet": "推导", "level": "formal" }));
        assert!(!concept.comparable(&formal, "R13"));
        assert!(!concept.comparable(&formal, "R15"));
        assert!(concept.comparable(&formal, "R2"));

        let shared = Applicability::of(&json!({ "category": "part", "facet": "推导", "line": "shared" }));
        let own = Applicability::of(&json!({ "category": "part", "facet": "推导" }));
        assert!(!shared.comparable(&own, "R6"), "a term-table transition changes R6");

        let book_a = Applicability::of(&json!({ "category": "idea", "book": "A" }));
        let book_b = Applicability::of(&json!({ "category": "idea", "book": "B" }));
        assert!(!book_a.comparable(&book_b, "I3"), "another book is another source requirement");
        assert!(book_a.comparable(&book_b, "R2"));

        let same = Applicability::of(&json!({ "category": "event", "level": "formal", "facet": "设定" }));
        let also = Applicability::of(&json!({ "category": "event", "level": "formal", "facet": "设定" }));
        assert!(same.comparable(&also, "E2"));
        // A card retyped from Idea to Event keeps a style fault but not an Idea one.
        let before = [diagnostic("K/9", "E2", "missing_labels", "labels:参与者", "missing labels: 参与者")];
        let report = ratchet(&before, before.to_vec(), |diagnostic| {
            idea.comparable(&event, &diagnostic.rule)
        });
        assert!(!report.diagnostics[0].legacy);
        assert_eq!(report.blockers, 1);
    }

    /// S02-AC05 (R0146/R0151): a rule the catalog does not name yet keeps the
    /// conservative treatment: every metadata fact a future rule could read must
    /// be unchanged, so a shared-list, book or term transition never grandfathers
    /// an unnamed rule. The named rules keep their own narrower signatures.
    #[test]
    fn an_unnamed_rule_compares_every_metadata_fact() {
        let own = Applicability::of(&json!({ "category": "part", "facet": "推导" }));
        let shared = Applicability::of(&json!({ "category": "part", "facet": "推导", "line": "shared" }));
        assert!(!own.comparable(&shared, "T9"), "a shared transition is no comparison");
        let book_a = Applicability::of(&json!({ "category": "idea", "book": "A" }));
        let book_b = Applicability::of(&json!({ "category": "idea", "book": "B" }));
        assert!(!book_a.comparable(&book_b, "T9"), "another book is no comparison");
        let term = Applicability::of(&json!({ "category": "part", "facet": "推导", "term": "替代弹性" }));
        assert!(!own.comparable(&term, "T9"), "a term transition is no comparison");
        assert!(own.comparable(&own, "T9"));
        // A named rule keeps its own narrower signature.
        assert!(!book_a.comparable(&book_b, "I3"));
        assert!(own.comparable(&shared, "R13"));
    }

    #[test]
    fn a_report_counts_only_blocking_diagnostics_and_prints_the_familiar_lines() {
        let report = CheckReport::of(vec![
            diagnostic("K/1", "R3", "semicolon", "甲；乙。", "甲；乙。"),
            diagnostic("K/1", "I1", "no_claim_title", "税", "the title states no claim: 税"),
        ]);
        assert_eq!(report.blockers, 1);
        assert_eq!(
            report.lines(),
            ["K/1 R3 甲；乙。", "K/1 I1 (advisory) the title states no claim: 税"]
        );
    }

    /// S02-AC07: a rule-version change can change which fault is recognized.
    /// Two versions that agree on the text but name different fault codes are not
    /// the same occurrence, so an update cannot silently inherit the old finding.
    #[test]
    fn a_changed_fault_code_is_not_inherited() {
        let old = diagnostic("K/1", "R12", "title_size", "甲；乙", "two claims: 甲；乙");
        let new = diagnostic("K/1", "R12", "two_claims", "甲；乙", "two claims: 甲；乙");
        let report = ratchet(&[old], vec![new], |_| true);
        assert!(!report.diagnostics[0].legacy);
        assert_eq!(report.blockers, 1);
    }
}
