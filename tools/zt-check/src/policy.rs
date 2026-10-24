//! The rule catalog and report policy: which rules a `--cards` report prints and
//! counts, and the default set every digest caller relies on. Formatting and
//! identity stay in the core; a policy reads the familiar fault lines.

use crate::text::{pattern, ps_in};
use regex::Regex;
use std::sync::LazyLock;

/// The rules self-check.ps1 reports by default: every hard rule but R6.
pub const DEFAULT_RULES: &str =
    "R2,R3,R4,R5,R7,R9,R12,R13,R14,R15,R16,R17,R20,R21,R22,I1,I2,I3,I4,I7,L1,L2,E1,E2,F1,F2,F3";

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
}
