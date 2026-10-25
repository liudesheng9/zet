//! One Card's planned transformation (design 13.2-13.4, S22 / #160).
//!
//! The pass walks the body once, in byte order, with three kinds of step: a protected span (report
//! each link word inside it as a skip), a Term macro (normalize, repair or keep it), and eligible
//! prose (the leftmost-longest link-word scan). A same-book Term macro always reads as its label
//! first, so a stale, missing or retyped target never stops the repair; a foreign book's macro is
//! retained byte for byte; malformed Term-like markup refuses the Card with the exact line.
//!
//! Nothing is reserialized. Every edit is a replacement of one UTF-8 byte range of the authored
//! body, and the Card is refused (`term_link_changed_text`) unless
//! `strip_term_links(new_body) == strip_term_links(old_body)` byte for byte.

use super::matches::{self, Policy, Vocabulary};
use super::spans::{self, Profile, Protection};
use std::collections::{BTreeMap, BTreeSet};

/// The plan's edit kinds, in report order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum EditKind {
    /// Plain prose replaced by a new Term macro.
    Link,
    /// A same-book Term macro re-emitted with its label's one valid head.
    Repair,
    /// A same-book Term macro replaced by its label: stale, repeated, or not linkable.
    Normalize,
}

impl EditKind {
    pub(crate) fn id(self) -> &'static str {
        match self {
            EditKind::Link => "link",
            EditKind::Repair => "repair",
            EditKind::Normalize => "normalize",
        }
    }
}

/// One byte substitution in the authored body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Edit {
    pub(crate) kind: EditKind,
    pub(crate) start: usize,
    pub(crate) end: usize,
    /// The exact term the new macro names; `None` for a normalization to plain text.
    pub(crate) term: Option<String>,
    pub(crate) target: Option<String>,
    pub(crate) label: String,
    pub(crate) before: String,
    pub(crate) after: String,
}

/// One occurrence the pass left alone, with the reason the plan reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Skip {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) text: String,
    pub(crate) reason: &'static str,
}

/// One correct Term use that is already exactly what the pass would emit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Kept {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) term: String,
    pub(crate) target: String,
}

/// One Term macro the pass leaves as authored: a foreign book's use or a non-book target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Retained {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) label: String,
    pub(crate) target: String,
    /// `foreign_book` or `not_book`.
    pub(crate) reason: &'static str,
}

/// Why a Card is refused by the plan instead of being edited.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fault {
    /// `malformed_term_markup`, `malformed_link_markup`, `term_link_changed_text` or
    /// `term_link_edit_overlap`.
    pub(crate) code: &'static str,
    /// The byte offset the diagnostic names, when it has one.
    pub(crate) at: Option<usize>,
    pub(crate) message: String,
}

/// The planned transformation of one Card.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CardPlan {
    pub(crate) edits: Vec<Edit>,
    pub(crate) skips: Vec<Skip>,
    pub(crate) kept: Vec<Kept>,
    pub(crate) retained: Vec<Retained>,
    pub(crate) new_body: String,
    pub(crate) fault: Option<Fault>,
}

impl CardPlan {
    /// Whether the Card's authored body changes.
    pub(crate) fn changed(&self) -> bool {
        !self.edits.is_empty()
    }
}

/// The counts one plan contributes to the book report.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Counts {
    pub(crate) links_added: usize,
    pub(crate) links_repaired: usize,
    pub(crate) links_kept: usize,
    pub(crate) links_removed: usize,
    pub(crate) retained_foreign: usize,
    pub(crate) retained_other: usize,
    /// Skip reason → count, in the report's fixed order.
    pub(crate) skipped: BTreeMap<&'static str, usize>,
}

impl Counts {
    pub(crate) fn add(&mut self, plan: &CardPlan) {
        for edit in &plan.edits {
            match edit.kind {
                EditKind::Link => self.links_added += 1,
                EditKind::Repair => self.links_repaired += 1,
                EditKind::Normalize => self.links_removed += 1,
            }
        }
        self.links_kept += plan.kept.len();
        for retained in &plan.retained {
            if retained.reason == "foreign_book" {
                self.retained_foreign += 1;
            } else {
                self.retained_other += 1;
            }
        }
        for skip in &plan.skips {
            *self.skipped.entry(skip.reason).or_default() += 1;
        }
    }

    /// The skip map as the report prints it: every seen reason in a stable order.
    pub(crate) fn skipped_json(&self) -> serde_json::Value {
        let mut map = serde_json::Map::new();
        for reason in SKIP_REASONS {
            let count = self.skipped.get(reason).copied().unwrap_or(0);
            map.insert((*reason).to_string(), serde_json::json!(count));
        }
        for (reason, count) in &self.skipped {
            map.entry((*reason).to_string())
                .or_insert_with(|| serde_json::json!(count));
        }
        serde_json::Value::Object(map)
    }
}

/// The skip reasons, in report order: protected spans first, then the matcher's own decisions.
pub(crate) const SKIP_REASONS: [&str; 22] = [
    "code",
    "chart",
    "math",
    "markdown_link",
    "html",
    "heading",
    "table",
    "source",
    "metadata",
    "generated",
    "own_term",
    "incomplete_code",
    "incomplete_math",
    "avoid_word",
    "no_head",
    "duplicate_head",
    "auto_link_off",
    "short",
    "ambiguous",
    "no_row",
    "repeat",
    "crosses",
];

/// Plan one Card. `book` is the Citation key the tree belongs to.
pub(crate) fn plan_card(
    body: &str,
    profile: &Profile<'_>,
    vocabulary: &Vocabulary,
    policy: Policy,
    book: &str,
) -> CardPlan {
    let occurrences = match zt_check::text::parse_links_at(body) {
        Ok(occurrences) => occurrences,
        Err(refused) => {
            let code = match refused.fault {
                zt_check::text::LinkFault::LabelLength
                | zt_check::text::LinkFault::SecondSeparator => "malformed_term_markup",
                _ => "malformed_link_markup",
            };
            return CardPlan {
                fault: Some(Fault {
                    code,
                    at: Some(refused.at),
                    message: refused.to_string(),
                }),
                new_body: body.to_string(),
                ..CardPlan::default()
            };
        }
    };
    let macros: BTreeMap<usize, (&str, &str)> = occurrences
        .iter()
        .filter(|occurrence| occurrence.form == zt_check::text::LinkForm::Term)
        .filter_map(|occurrence| {
            Some((
                occurrence.macro_range.start,
                (occurrence.label?, occurrence.target),
            ))
        })
        .collect();
    let layout = spans::layout(body, profile);
    let mut plan = CardPlan {
        new_body: body.to_string(),
        ..CardPlan::default()
    };
    let mut used: BTreeSet<usize> = BTreeSet::new();
    let mut span_index = 0usize;
    let mut run_index = 0usize;
    let mut index = 0usize;
    while index < body.len() {
        while span_index < layout.spans.len() && layout.spans[span_index].range.end <= index {
            span_index += 1;
        }
        if let Some(span) = layout.spans.get(span_index).filter(|span| span.range.start == index) {
            let end = span.range.end;
            if span.protection == Protection::Link {
                if let Some((label, target)) = macros.get(&index) {
                    handle_macro(
                        &mut plan,
                        vocabulary,
                        policy,
                        book,
                        profile,
                        body,
                        label,
                        target,
                        index..end,
                        &mut used,
                    );
                }
            } else if span.protection.reportable() {
                for (start, end) in vocabulary.link_occurrences_in(body, &span.range) {
                    plan.skips.push(Skip {
                        start,
                        end,
                        text: body[start..end].to_string(),
                        reason: span.protection.id(),
                    });
                }
            }
            index = end;
            continue;
        }
        while run_index < layout.runs.len() && layout.runs[run_index].end <= index {
            run_index += 1;
        }
        if let Some(run) = layout.runs.get(run_index).filter(|run| run.start <= index && index < run.end) {
            let run_end = run.end;
            match vocabulary.longest_at(body, index) {
                Some((end, word)) if end <= run_end => {
                    if let Some(reason) = word.blocker {
                        plan.skips.push(Skip {
                            start: index,
                            end,
                            text: body[index..end].to_string(),
                            reason,
                        });
                    } else if let Some(link) = word.link {
                        let allowed = policy == Policy::All || !used.contains(&link);
                        let term = &vocabulary.terms[link];
                        if allowed {
                            used.insert(link);
                            let head = term.link_head().unwrap_or_default();
                            plan.edits.push(Edit {
                                kind: EditKind::Link,
                                start: index,
                                end,
                                term: Some(term.term.clone()),
                                target: Some(head.to_string()),
                                label: term.term.clone(),
                                before: body[index..end].to_string(),
                                after: format!("[[{}@{head}]]", term.term),
                            });
                        } else {
                            plan.skips.push(Skip {
                                start: index,
                                end,
                                text: body[index..end].to_string(),
                                reason: "repeat",
                            });
                        }
                    }
                    index = end;
                }
                Some(_) => {
                    // A longer word starts here but leaves this run: never cross a protected
                    // boundary, and do not link a shorter word the longer one would have covered.
                    plan.skips.push(Skip {
                        start: index,
                        end: run_end,
                        text: body[index..run_end].to_string(),
                        reason: "crosses",
                    });
                    index = run_end;
                }
                None => index += char_len(body, index),
            }
            continue;
        }
        index += char_len(body, index);
    }
    if plan.edits.is_empty() {
        return plan;
    }
    match apply(body, &plan.edits) {
        Ok(new_body) => {
            if zt_check::text::strip_term_links(&new_body) != zt_check::text::strip_term_links(body) {
                plan.fault = Some(Fault {
                    code: "term_link_changed_text",
                    at: None,
                    message: "the planned body would not show the same visible text; the Card is refused"
                        .to_string(),
                });
                plan.edits.clear();
                plan.kept.clear();
                plan.retained.clear();
                plan.new_body = body.to_string();
            } else {
                plan.new_body = new_body;
            }
        }
        Err(at) => {
            plan.fault = Some(Fault {
                code: "term_link_edit_overlap",
                at: Some(at),
                message: "the planned edits overlap; the Card is refused".to_string(),
            });
            plan.edits.clear();
            plan.kept.clear();
            plan.retained.clear();
            plan.new_body = body.to_string();
        }
    }
    plan
}

/// Handle one same-book, foreign-book or non-book Term macro.
#[allow(clippy::too_many_arguments)]
fn handle_macro(
    plan: &mut CardPlan,
    vocabulary: &Vocabulary,
    policy: Policy,
    book: &str,
    profile: &Profile<'_>,
    body: &str,
    label: &str,
    target: &str,
    range: std::ops::Range<usize>,
    used: &mut BTreeSet<usize>,
) {
    let before = body[range.clone()].to_string();
    let owner = matches::literature_owner(target);
    if owner != Some(book) {
        plan.retained.push(Retained {
            start: range.start,
            end: range.end,
            label: label.to_string(),
            target: target.to_string(),
            reason: if owner.is_some() { "foreign_book" } else { "not_book" },
        });
        return;
    }
    // R0383: a Term Card's own exact term is protected everywhere in its own head and parts, so
    // an authored self-link reads as its label again and the pass creates no self-reference.
    let own = profile.own_term == Some(label);
    let linkable = matches::link_word(vocabulary, label).filter(|_| !own);
    match linkable {
        Some((head, index)) if policy == Policy::All || !used.contains(&index) => {
            used.insert(index);
            let after = format!("[[{label}@{head}]]");
            if before == after {
                plan.kept.push(Kept {
                    start: range.start,
                    end: range.end,
                    term: label.to_string(),
                    target: head.to_string(),
                });
            } else {
                plan.edits.push(Edit {
                    kind: EditKind::Repair,
                    start: range.start,
                    end: range.end,
                    term: Some(label.to_string()),
                    target: Some(head.to_string()),
                    label: label.to_string(),
                    before,
                    after,
                });
            }
        }
        _ => {
            // The label is the authored visible text; a stale, repeated or unlinkable use reads
            // as plain prose again, and no byte is guessed or deleted.
            plan.edits.push(Edit {
                kind: EditKind::Normalize,
                start: range.start,
                end: range.end,
                term: None,
                target: None,
                label: label.to_string(),
                before,
                after: label.to_string(),
            });
        }
    }
}

/// Apply the ordered, disjoint substitutions. Answers the first overlap's offset.
fn apply(body: &str, edits: &[Edit]) -> Result<String, usize> {
    let mut out = String::with_capacity(body.len());
    let mut offset = 0usize;
    for edit in edits {
        if edit.start < offset || edit.end < edit.start || edit.end > body.len() {
            return Err(edit.start);
        }
        out.push_str(&body[offset..edit.start]);
        out.push_str(&edit.after);
        offset = edit.end;
    }
    out.push_str(&body[offset..]);
    Ok(out)
}

/// The 1-based line number that holds `at`, and that line's text.
pub(crate) fn line_of(body: &str, at: usize) -> (usize, String) {
    let at = at.min(body.len());
    let line = body[..at].matches('\n').count() + 1;
    let start = body[..at].rfind('\n').map_or(0, |index| index + 1);
    let end = body[at..]
        .find('\n')
        .map_or(body.len(), |index| at + index);
    (line, body[start..end].to_string())
}

fn char_len(body: &str, index: usize) -> usize {
    body[index..].chars().next().map_or(1, char::len_utf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::term_link::matches::Term;

    fn term(row: usize, name: &str, avoid: &[&str], heads: &[&str], auto_link: bool) -> Term {
        Term {
            row,
            term: name.to_string(),
            avoid: avoid.iter().map(|word| word.to_string()).collect(),
            heads: heads.iter().map(|head| head.to_string()).collect(),
            auto_link,
        }
    }

    /// The fixture book's vocabulary: longer names, an avoid word, a nonlinkable longer name, two
    /// qualified homonyms, a Latin acronym, and a named Term Card that publishes no row.
    fn book() -> Vocabulary {
        Vocabulary::build(
            &[
                term(1, "替代弹性", &["替代弹性系数"], &["K/3"], true),
                term(2, "生产函数", &[], &["K/4"], true),
                term(3, "净生产函数", &[], &["K/5"], true),
                term(4, "要素价格", &[], &["K/6"], true),
                term(5, "相对要素价格", &[], &[], true),
                term(6, "GDP", &[], &["K/7"], true),
                term(7, "均衡", &[], &["K/10"], true),
                term(8, "学习（个体行为）", &[], &["K/8"], true),
                term(9, "学习（产业生产率）", &[], &["K/9"], true),
            ],
            &["无行术语".to_string()],
        )
    }

    fn plan(body: &str, policy: Policy) -> CardPlan {
        plan_card(body, &Profile::default(), &book(), policy, "K")
    }

    fn assert_preserved(body: &str, plan: &CardPlan) {
        assert!(plan.fault.is_none(), "{:?}", plan.fault);
        assert_eq!(
            zt_check::text::strip_term_links(&plan.new_body),
            zt_check::text::strip_term_links(body),
            "the visible prose changed"
        );
        for edit in &plan.edits {
            assert!(body.is_char_boundary(edit.start));
            assert!(body.is_char_boundary(edit.end));
            assert_eq!(&body[edit.start..edit.end], edit.before);
        }
    }

    fn edits(plan: &CardPlan) -> Vec<(&'static str, &str, &str)> {
        plan.edits
            .iter()
            .map(|edit| (edit.kind.id(), edit.before.as_str(), edit.after.as_str()))
            .collect()
    }

    #[test]
    fn first_links_the_first_occurrence_and_all_links_every_one() {
        let body = "生产函数影响生产函数。生产函数再次出现。";
        let first = plan(body, Policy::First);
        assert_eq!(
            edits(&first),
            [("link", "生产函数", "[[生产函数@K/4]]")]
        );
        assert_eq!(
            first
                .skips
                .iter()
                .filter(|skip| skip.reason == "repeat")
                .count(),
            2
        );
        assert_preserved(body, &first);

        let all = plan(body, Policy::All);
        assert_eq!(all.edits.len(), 3);
        assert_preserved(body, &all);
    }

    #[test]
    fn a_longer_word_and_an_avoid_word_consume_the_text_they_cover() {
        let body = "净生产函数与生产函数；替代弹性系数与替代弹性；相对要素价格与要素价格。";
        let planned = plan(body, Policy::All);
        assert_eq!(
            edits(&planned),
            [
                ("link", "净生产函数", "[[净生产函数@K/5]]"),
                ("link", "生产函数", "[[生产函数@K/4]]"),
                ("link", "替代弹性", "[[替代弹性@K/3]]"),
                ("link", "要素价格", "[[要素价格@K/6]]"),
            ]
        );
        let reasons: Vec<(&str, &str)> = planned
            .skips
            .iter()
            .map(|skip| (skip.text.as_str(), skip.reason))
            .collect();
        assert_eq!(
            reasons,
            [("替代弹性系数", "avoid_word"), ("相对要素价格", "no_head")]
        );
        assert_preserved(body, &planned);
    }

    #[test]
    fn protected_spans_report_their_skip_reason_and_never_link() {
        let body = "`生产函数` 与 $生产函数$ 与\n\n| 生产函数 |\n| --- |\n\n## 生产函数\n\n来源：[[K]] 生产函数\n";
        let planned = plan(body, Policy::All);
        assert!(planned.edits.is_empty(), "{:?}", planned.edits);
        for reason in ["code", "math", "table", "heading", "source"] {
            assert_eq!(
                planned.skips.iter().filter(|skip| skip.reason == reason).count(),
                1,
                "{reason}: {:?}",
                planned.skips
            );
        }
        assert_preserved(body, &planned);
    }

    #[test]
    fn a_correct_macro_is_kept_and_a_later_plain_occurrence_is_no_second_use() {
        let body = "[[均衡@K/10]] 与 均衡。";
        let planned = plan(body, Policy::First);
        assert!(planned.edits.is_empty());
        assert_eq!(planned.kept.len(), 1);
        assert_eq!(planned.kept[0].target, "K/10");
        assert_eq!(
            planned.skips.iter().filter(|skip| skip.reason == "repeat").count(),
            1
        );
    }

    #[test]
    fn a_stale_or_mismatched_macro_is_repaired_to_its_label_one_head() {
        // The label names 均衡, whose one head is K/10; the macro's old target is another head.
        let stale = plan("[[均衡@K/7]]", Policy::First);
        assert_eq!(edits(&stale), [("repair", "[[均衡@K/7]]", "[[均衡@K/10]]")]);
        // The label is the visible word, so a mismatched label repairs to that word's own head.
        let mismatched = plan("[[GDP@K/10]]", Policy::First);
        assert_eq!(edits(&mismatched), [("repair", "[[GDP@K/10]]", "[[GDP@K/7]]")]);
        // A label that is no link word keeps its text and loses only the stale macro.
        let avoided = plan("[[替代弹性系数@K/3]]", Policy::First);
        assert_eq!(
            edits(&avoided),
            [("normalize", "[[替代弹性系数@K/3]]", "替代弹性系数")]
        );
        let unrowed = plan("[[无行术语@K/3]]", Policy::First);
        assert_eq!(edits(&unrowed), [("normalize", "[[无行术语@K/3]]", "无行术语")]);
    }

    #[test]
    fn an_earlier_plain_word_wins_the_first_use_and_the_later_macro_reads_as_its_label() {
        let body = "均衡与[[均衡@K/10]]。";
        let first = plan(body, Policy::First);
        assert_eq!(
            edits(&first),
            [
                ("link", "均衡", "[[均衡@K/10]]"),
                ("normalize", "[[均衡@K/10]]", "均衡"),
            ]
        );
        assert_eq!(first.new_body, "[[均衡@K/10]]与均衡。");
        // With every occurrence, the correct macro stays exact and the plain word links too.
        let all = plan(body, Policy::All);
        assert_eq!(edits(&all), [("link", "均衡", "[[均衡@K/10]]")]);
        assert_eq!(all.kept.len(), 1);
        assert_eq!(all.new_body, "[[均衡@K/10]]与[[均衡@K/10]]。");
        assert_preserved(body, &first);
        assert_preserved(body, &all);
    }

    #[test]
    fn a_second_pass_changes_nothing_and_returns_the_same_body() {
        let bodies = [
            "[[均衡@K/10]] 与 均衡。",
            "均衡与[[均衡@K/10]]。",
            "生产函数影响生产函数。\n\n## 均衡\n\n`生产函数` 与 $均衡$。\n\n| 均衡 |\n| --- |\n\n来源：[[K]] 均衡\n",
            "替代弹性系数与替代弹性、GDP 与 GDPR、学习（个体行为）与学习。",
            "第一段 $x 未闭合\n第二段 生产函数\n",
        ];
        for body in bodies {
            for policy in [Policy::First, Policy::All] {
                let planned = plan(body, policy);
                assert_preserved(body, &planned);
                let again = plan(&planned.new_body, policy);
                assert!(
                    again.edits.is_empty(),
                    "{policy:?} {body:?}: {:?}",
                    again.edits
                );
                assert_eq!(again.new_body, planned.new_body, "{body:?}");
            }
        }
    }

    #[test]
    fn foreign_and_non_book_macros_are_retained_and_reported() {
        let body = "[[均衡@bookB/3]] 与 [[均衡@12/3]] 与 均衡";
        let planned = plan(body, Policy::First);
        assert_eq!(edits(&planned), [("link", "均衡", "[[均衡@K/10]]")]);
        let retained: Vec<(&str, &str)> = planned
            .retained
            .iter()
            .map(|retained| (retained.reason, retained.target.as_str()))
            .collect();
        assert_eq!(retained, [("foreign_book", "bookB/3"), ("not_book", "12/3")]);
        assert_preserved(body, &planned);
    }

    #[test]
    fn malformed_markup_refuses_the_card_and_names_the_line() {
        for (body, code, line) in [
            ("第一行\n\n[[均衡@K/10@x]]\n", "malformed_term_markup", 3),
            ("第一行\n\n[[均衡@K/10\n", "malformed_link_markup", 3),
            ("[[@K/10]]", "malformed_term_markup", 1),
        ] {
            let planned = plan(body, Policy::First);
            let fault = planned.fault.unwrap_or_else(|| panic!("{body:?} was not refused"));
            assert_eq!(fault.code, code, "{body:?}");
            assert!(fault.message.contains("invalid"), "{:?}", fault.message);
            let (got_line, _) = line_of(body, fault.at.unwrap());
            assert_eq!(got_line, line, "{body:?}");
            assert!(planned.edits.is_empty());
            assert_eq!(planned.new_body, body);
        }
    }

    #[test]
    fn a_term_split_by_emphasis_is_not_linked() {
        let body = "生产**函数**与生产函数。";
        let planned = plan(body, Policy::First);
        assert_eq!(edits(&planned), [("link", "生产函数", "[[生产函数@K/4]]")]);
        assert_eq!(&planned.new_body[.."生产**函数**与".len()], "生产**函数**与");
        assert_preserved(body, &planned);
    }

    #[test]
    fn a_card_never_links_its_own_term_and_an_other_term_there_still_links() {
        let body = "均衡是稳定的。生产函数决定产出。均衡不等于最优。";
        let profile = Profile {
            own_term: Some("均衡"),
            term_card: true,
        };
        let planned = plan_card(body, &profile, &book(), Policy::All, "K");
        assert_eq!(edits(&planned), [("link", "生产函数", "[[生产函数@K/4]]")]);
        assert_eq!(
            planned
                .skips
                .iter()
                .filter(|skip| skip.reason == "own_term")
                .count(),
            2
        );
        assert_preserved(body, &planned);
    }

    /// The real seam's property: over many generated bodies, a pass never changes the visible
    /// text, every edit is an exact byte replacement, and a second pass is a no-op.
    #[test]
    fn generated_bodies_keep_the_visible_text_and_are_idempotent() {
        const PIECES: [&str; 30] = [
            "生产函数",
            "净生产函数",
            "替代弹性",
            "替代弹性系数",
            "相对要素价格",
            "要素价格",
            "GDP",
            "GDPR",
            "均衡",
            "学习（个体行为）",
            "学习",
            " `代码生产函数` ",
            " $生产函数$ ",
            " $$均衡$$ ",
            " [[均衡@K/10]] ",
            " [[均衡@bad/1]] ",
            " [[均衡@12/3]] ",
            " [[均衡@bookB/2]] ",
            " **",
            "** ",
            "\\",
            " ",
            "，",
            "\n",
            "# 生产函数\n",
            "| 生产函数 |\n",
            "来源：",
            "无行术语",
            " $x 未闭合 ",
            "<span>均衡</span>",
        ];
        let vocabulary = book();
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (state >> 33) as usize
        };
        for _ in 0..80 {
            let mut body = String::new();
            for _ in 0..3 + next() % 8 {
                body.push_str(PIECES[next() % PIECES.len()]);
            }
            for policy in [Policy::First, Policy::All] {
                let planned = plan_card(&body, &Profile::default(), &vocabulary, policy, "K");
                assert_preserved(&body, &planned);
                let again = plan_card(
                    &planned.new_body,
                    &Profile::default(),
                    &vocabulary,
                    policy,
                    "K",
                );
                assert!(
                    again.edits.is_empty(),
                    "{policy:?} {body:?}: {:?}",
                    again.edits
                );
                assert_eq!(again.new_body, planned.new_body);
            }
        }
    }

    #[test]
    fn an_authored_self_link_in_its_own_term_card_reads_as_its_label() {
        let body = "[[均衡@K/10]] 是稳定的状态。";
        let profile = Profile {
            own_term: Some("均衡"),
            term_card: true,
        };
        let planned = plan_card(body, &profile, &book(), Policy::All, "K");
        assert_eq!(
            edits(&planned),
            [("normalize", "[[均衡@K/10]]", "均衡")]
        );
        assert_preserved(body, &planned);
        let again = plan_card(&planned.new_body, &profile, &book(), Policy::All, "K");
        assert!(again.edits.is_empty(), "{:?}", again.edits);
    }
}
