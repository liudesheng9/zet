use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

/// The shared lexical scan of `tools/zt-check/src/text.rs` (design 10.1, S10 / #151): macro shape,
/// the Term label grammar, code-span/fence exclusions, and the math-literal rule for a Term-like
/// example. The root wrapper below owns the address grammar and the Link lifecycle.
pub(crate) use zt_check::text::{
    LinkForm, LinkOccurrence, code_ranges, fenced_code_ranges, link_occurrences,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkBody<'a> {
    text: &'a str,
    occurrences: Vec<LinkOccurrence<'a>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkRewrite {
    pub(crate) body: String,
    pub(crate) count: usize,
}

impl<'a> LinkBody<'a> {
    pub(crate) fn parse(text: &'a str) -> Result<Self> {
        let occurrences = zt_check::text::parse_links(text)?;
        // The root's grammar: an ordinary Link names a Location or a Citation key, while a Term
        // link names a Regular Location only (R0252).
        for occurrence in &occurrences {
            let valid = match occurrence.form {
                LinkForm::Ordinary => {
                    crate::is_valid_location(occurrence.target)
                        || crate::literature::validate_citation_key(occurrence.target).is_ok()
                }
                LinkForm::Term => crate::is_valid_regular_location(occurrence.target),
            };
            if !valid {
                bail!(
                    "invalid {}link macro target `{}`",
                    if occurrence.form == LinkForm::Term {
                        "Term "
                    } else {
                        ""
                    },
                    occurrence.target
                );
            }
        }
        Ok(Self { text, occurrences })
    }

    /// Every `[[target]]` outside code, valid or not; it never fails.
    pub(crate) fn for_rendering(text: &'a str) -> Self {
        Self {
            text,
            occurrences: link_occurrences(text),
        }
    }

    pub(crate) fn occurrences(&self) -> &[LinkOccurrence<'a>] {
        &self.occurrences
    }

    pub(crate) fn rewrite_targets(&self, mapping: &BTreeMap<String, String>) -> LinkRewrite {
        let mut body = String::with_capacity(self.text.len());
        let mut offset = 0;
        let mut count = 0;
        for occurrence in &self.occurrences {
            body.push_str(&self.text[offset..occurrence.target_range.start]);
            if let Some(replacement) = mapping.get(occurrence.target) {
                body.push_str(replacement);
                count += 1;
            } else {
                body.push_str(occurrence.target);
            }
            offset = occurrence.target_range.end;
        }
        body.push_str(&self.text[offset..]);
        LinkRewrite { body, count }
    }
}

/// Validate one authored body's Links against the archive (R0252, R0254; design 10.3).
///
/// An ordinary Link to a new target must reach a Card, while an old Broken target may stay at any
/// occurrence count. A new Term use must reach a Term head; an old invalid Term use may stay only
/// with the same label and target, up to its old multiplicity, so changing an invalid target is
/// never retention. `exists` answers whether a Card is stored at an address and `is_head` whether
/// that Card is a Term head.
pub(crate) fn validate(
    body: &str,
    old_body: Option<&str>,
    exists: impl Fn(&str) -> Result<bool>,
    is_head: impl Fn(&str) -> Result<bool>,
) -> Result<()> {
    let mut old_broken = BTreeSet::new();
    let mut retained: BTreeMap<(String, String), usize> = BTreeMap::new();
    if let Some(old) = old_body {
        // The old body is read tolerantly: a stored Card can hold a target that has since gone,
        // and that is exactly the use a later edit may retain.
        for occurrence in LinkBody::for_rendering(old).occurrences() {
            match occurrence.form {
                LinkForm::Ordinary => {
                    if !exists(occurrence.target)? {
                        old_broken.insert(occurrence.target.to_string());
                    }
                }
                LinkForm::Term => {
                    if !is_head(occurrence.target)? {
                        let label = occurrence.label.unwrap_or_default().to_string();
                        *retained
                            .entry((label, occurrence.target.to_string()))
                            .or_default() += 1;
                    }
                }
            }
        }
    }
    let body = LinkBody::parse(body)?;
    for occurrence in body.occurrences() {
        match occurrence.form {
            LinkForm::Ordinary => {
                if !exists(occurrence.target)? && !old_broken.contains(occurrence.target) {
                    bail!("link target `{}` does not exist", occurrence.target);
                }
            }
            LinkForm::Term => {
                if is_head(occurrence.target)? {
                    continue;
                }
                let label = occurrence.label.unwrap_or_default().to_string();
                let key = (label, occurrence.target.to_string());
                if let Some(left) = retained.get_mut(&key)
                    && *left > 0
                {
                    *left -= 1;
                    continue;
                }
                if !exists(occurrence.target)? {
                    bail!("Term link target `{}` does not exist", occurrence.target);
                }
                bail!("Term link target `{}` is not a Term head", occurrence.target);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CardSnapshot {
    pub(crate) address: String,
    pub(crate) title: String,
    pub(crate) body: String,
    /// Whether this Card is a currently valid Term head: the only role a new Term use may name
    /// (S11, #153). It is computed by the caller from the same stored identities `term::is_head`
    /// reads, so a Term use to another role is a semantic fault even though the row exists.
    pub(crate) term_head: bool,
}

impl CardSnapshot {
    /// A snapshot of a Card that is not (or not known to be) a Term head.
    #[cfg(test)]
    pub(crate) fn card(
        address: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            address: address.into(),
            title: title.into(),
            body: body.into(),
            term_head: false,
        }
    }

    /// A snapshot of a currently valid Term head.
    #[cfg(test)]
    pub(crate) fn term_head(
        address: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            term_head: true,
            ..Self::card(address, title, body)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LinkStatus {
    Valid,
    Broken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassifiedLink<'a> {
    pub(crate) target: &'a str,
    /// The authored word of a Term use; `None` for an ordinary Link (S13, #156).
    pub(crate) label: Option<&'a str>,
    pub(crate) form: LinkForm,
    pub(crate) macro_range: Range<usize>,
    pub(crate) status: LinkStatus,
    /// Why a Broken occurrence is unusable, so a reader can explain it without guessing: a
    /// missing target or a Term use of an existing Card that is no current head. `None` for a
    /// Valid occurrence.
    pub(crate) reason: Option<BrokenReason>,
}

/// Why a Link occurrence is Broken (S11, #153; design 10.3).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BrokenReason {
    /// The target address holds no Card.
    Missing,
    /// The target row exists, but it is not a currently valid Term head.
    NotHead,
}

impl BrokenReason {
    pub(crate) fn id(self) -> &'static str {
        match self {
            BrokenReason::Missing => "missing",
            BrokenReason::NotHead => "not_head",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrokenLink {
    pub(crate) source_address: String,
    pub(crate) source_title: String,
    pub(crate) target: String,
    /// The authored word of a Term use; `None` for an ordinary Link.
    pub(crate) label: Option<String>,
    pub(crate) reason: BrokenReason,
    pub(crate) source_line: String,
}

impl BrokenLink {
    /// The target part of the one line `lsbk` prints in the shell and in the Session (S11, #153):
    /// an ordinary Broken link keeps its address alone, while a Broken Term use names the authored
    /// label and why the use is invalid, without changing the earlier `source title -> target`
    /// shape.
    pub(crate) fn detail(&self) -> String {
        match (self.label.as_deref(), self.reason) {
            (Some(label), BrokenReason::Missing) => {
                format!("{} (term `{label}`, missing)", self.target)
            }
            (Some(label), BrokenReason::NotHead) => {
                format!("{} (term `{label}`, not a Term head)", self.target)
            }
            (None, _) => self.target.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkLifecycle {
    cards: Vec<CardSnapshot>,
    addresses: BTreeSet<String>,
    term_heads: BTreeSet<String>,
}

impl LinkLifecycle {
    pub(crate) fn from_cards(cards: Vec<CardSnapshot>) -> Self {
        let addresses = cards.iter().map(|card| card.address.clone()).collect();
        let term_heads = cards
            .iter()
            .filter(|card| card.term_head)
            .map(|card| card.address.clone())
            .collect();
        Self {
            cards,
            addresses,
            term_heads,
        }
    }

    pub(crate) fn classify_for_rendering<'a>(&self, body: &'a str) -> Vec<ClassifiedLink<'a>> {
        LinkBody::for_rendering(body)
            .occurrences()
            .iter()
            .map(|occurrence| {
                let reason = self.broken_reason(occurrence);
                ClassifiedLink {
                    target: occurrence.target,
                    label: occurrence.label,
                    form: occurrence.form,
                    macro_range: occurrence.macro_range.clone(),
                    status: if reason.is_some() {
                        LinkStatus::Broken
                    } else {
                        LinkStatus::Valid
                    },
                    reason,
                }
            })
            .collect()
    }

    /// Whether one authored occurrence reaches the target its form requires (design 10.3): an
    /// ordinary Link needs the stored row, while a Term use also needs that row to be a Term head.
    fn reaches_valid_target(&self, occurrence: &LinkOccurrence) -> bool {
        let grammar_valid = match occurrence.form {
            LinkForm::Ordinary => {
                crate::is_valid_location(occurrence.target)
                    || crate::literature::validate_citation_key(occurrence.target).is_ok()
            }
            LinkForm::Term => crate::is_valid_regular_location(occurrence.target),
        };
        grammar_valid
            && self.addresses.contains(occurrence.target)
            && (occurrence.form == LinkForm::Ordinary
                || self.term_heads.contains(occurrence.target))
    }

    /// Why one occurrence is unusable, for both the Broken-link report and the reader (S11, #153;
    /// S13, #156). One rule keeps the two surfaces from disagreeing about what is broken.
    fn broken_reason(&self, occurrence: &LinkOccurrence) -> Option<BrokenReason> {
        if self.reaches_valid_target(occurrence) {
            return None;
        }
        if occurrence.form == LinkForm::Term && self.addresses.contains(occurrence.target) {
            Some(BrokenReason::NotHead)
        } else {
            Some(BrokenReason::Missing)
        }
    }

    #[cfg(test)]
    pub(crate) fn validate_edit<'a>(
        &self,
        body: &'a str,
        old_body: Option<&str>,
    ) -> Result<LinkBody<'a>> {
        let exists = |at: &str| Ok(self.addresses.contains(at));
        validate(body, old_body, exists, |_| Ok(false))?;
        LinkBody::parse(body)
    }

    /// The generated tail of every snapshot Card (S11, #153; design 10.3): ordinary Reverse links
    /// and Term uses are separate sections, each deduplicated by source and ordered by address,
    /// and a source that holds both forms to one target appears once in each section. Generated
    /// text is never parsed here: only each Card's authored body is scanned.
    pub(crate) fn reverse_sections(&self) -> Result<BTreeMap<String, String>> {
        let titles: BTreeMap<&str, &str> = self
            .cards
            .iter()
            .map(|card| (card.address.as_str(), card.title.as_str()))
            .collect();
        let mut ordinary: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        let mut term_uses: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for source in &self.cards {
            let body = LinkBody::parse(&source.body)?;
            let mut seen = BTreeSet::new();
            for occurrence in body.occurrences() {
                if !self.addresses.contains(occurrence.target) {
                    continue;
                }
                // Deduplicate by (form, target) within one source, so the same source may appear
                // once in each section when it writes both an ordinary Link and a Term use.
                let is_term = occurrence.form == LinkForm::Term;
                if !seen.insert((is_term, occurrence.target)) {
                    continue;
                }
                let bucket = if is_term { &mut term_uses } else { &mut ordinary };
                bucket
                    .entry(occurrence.target)
                    .or_default()
                    .insert(&source.address);
            }
        }
        Ok(self
            .cards
            .iter()
            .map(|card| {
                let sources = |inbound: &BTreeMap<&str, BTreeSet<&str>>| {
                    inbound
                        .get(card.address.as_str())
                        .into_iter()
                        .flat_map(|sources| sources.iter())
                        .map(|source| {
                            (
                                source.to_string(),
                                titles.get(*source).copied().unwrap_or_default().to_string(),
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let ordinary = sources(&ordinary);
                let term_uses = sources(&term_uses);
                (
                    card.address.clone(),
                    crate::card::reverse_section(
                        ordinary.iter().map(|(a, t)| (a.as_str(), t.as_str())),
                        term_uses.iter().map(|(a, t)| (a.as_str(), t.as_str())),
                    ),
                )
            })
            .collect())
    }

    /// Every Broken occurrence (S11, #153): a missing target for either form, plus a Term use whose
    /// existing target row is not a Term head. The term label is carried so `lsbk`, the MCP report
    /// and the GUI can name the authored word rather than only the address.
    pub(crate) fn broken_links(&self) -> Result<Vec<BrokenLink>> {
        let mut broken = Vec::new();
        for card in &self.cards {
            for occurrence in LinkBody::parse(&card.body)?.occurrences() {
                let Some(reason) = self.broken_reason(occurrence) else {
                    continue;
                };
                broken.push(BrokenLink {
                    source_address: card.address.clone(),
                    source_title: card.title.clone(),
                    target: occurrence.target.to_string(),
                    label: occurrence.label.map(str::to_string),
                    reason,
                    source_line: line_around(&card.body, occurrence.macro_range.start).to_string(),
                });
            }
        }
        Ok(broken)
    }
}

pub(crate) fn line_around(text: &str, index: usize) -> &str {
    let start = text[..index].rfind('\n').map_or(0, |newline| newline + 1);
    let end = text[index..]
        .find('\n')
        .map_or(text.len(), |newline| index + newline);
    text[start..end].trim_end_matches('\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_body_reports_exact_occurrence_and_target_ranges() {
        let body = LinkBody::parse("see [[0/1]] and [[LitA]]").expect("valid Link body");

        assert_eq!(
            body.occurrences(),
            [
                LinkOccurrence {
                    target: "0/1",
                    label: None,
                    form: LinkForm::Ordinary,
                    macro_range: 4..11,
                    target_range: 6..9,
                    label_range: None,
                },
                LinkOccurrence {
                    target: "LitA",
                    label: None,
                    form: LinkForm::Ordinary,
                    macro_range: 16..24,
                    target_range: 18..22,
                    label_range: None,
                },
            ]
        );
    }

    /// R0250–R0254 (S10, #151): the root wrapper reuses the shared scan, so a Term link keeps its
    /// label and exact byte ranges, and a move rewrites only the address bytes.
    #[test]
    fn term_links_keep_their_label_and_ranges_through_a_target_rewrite() {
        let text = "见 [[均衡@2/1|a]] 与 [[0/1]]。";
        let body = LinkBody::parse(text).expect("a valid Term link");
        let occurrence = &body.occurrences()[0];
        assert_eq!(occurrence.form, LinkForm::Term);
        assert_eq!(occurrence.target, "2/1|a");
        assert_eq!(occurrence.label, Some("均衡"));
        assert_eq!(&text[occurrence.label_range.clone().unwrap()], "均衡");
        assert_eq!(&text[occurrence.target_range.clone()], "2/1|a");
        assert_eq!(&text[occurrence.macro_range.clone()], "[[均衡@2/1|a]]");

        let mapping = BTreeMap::from([("2/1|a".to_string(), "2/2|b".to_string())]);
        assert_eq!(
            body.rewrite_targets(&mapping),
            LinkRewrite {
                body: "见 [[均衡@2/2|b]] 与 [[0/1]]。".to_string(),
                count: 1,
            }
        );
    }

    #[test]
    fn a_term_link_target_is_a_regular_location_and_its_label_is_shared_grammar() {
        for (text, expected) in [
            ("[[均衡@0/0]]", "invalid Term link macro target `0/0`"),
            (
                "[[均衡@LitA]]",
                "invalid Term link macro target `LitA`",
            ),
            (
                "[[a@b@2/1|a]]",
                "invalid Term link macro: one `@` separates the label and the address",
            ),
            ("[[@2/1|a]]", "invalid Term link label: 1-40 characters on one line"),
        ] {
            let error = LinkBody::parse(text).expect_err("invalid Term link");
            assert_eq!(error.to_string(), expected, "{text:?}");
        }
        // A Term target in a Literature tree is a Regular Location like any other.
        assert!(LinkBody::parse("[[均衡@Book1990/1|a]]").is_ok());
    }

    /// S11-AC02/S11-AC03/S11-AC06/R0264-R0266: a valid Term use gets its own generated section,
    /// a source that writes both forms appears once in each section, and a Term use whose target
    /// is missing or is not a Term head is Broken with its authored label.
    #[test]
    fn term_uses_get_their_own_reverse_section_and_broken_uses_carry_the_label() {
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot::term_head(
            "0/1",
            "Target",
            "target",
        )]);
        assert_eq!(
            lifecycle.classify_for_rendering("见 [[均衡@0/1]] 与 [[幽灵@0/9]]"),
            [
                ClassifiedLink {
                    target: "0/1",
                    label: Some("均衡"),
                    form: LinkForm::Term,
                    macro_range: 4..18,
                    status: LinkStatus::Valid,
                    reason: None,
                },
                ClassifiedLink {
                    target: "0/9",
                    label: Some("幽灵"),
                    form: LinkForm::Term,
                    macro_range: 23..37,
                    status: LinkStatus::Broken,
                    reason: Some(BrokenReason::Missing),
                },
            ]
        );

        let lifecycle = LinkLifecycle::from_cards(vec![
            CardSnapshot::term_head("0/1", "Target", "target"),
            CardSnapshot::card(
                "0/2",
                "User",
                "见 [[均衡@0/1]] 与 [[幽灵@0/9]] and [[0/1]]",
            ),
            CardSnapshot::card("0/3", "Not a head", "term row"),
            CardSnapshot::card("0/4", "Non-head user", "见 [[均衡@0/3]]"),
        ]);
        let reverse = lifecycle.reverse_sections().expect("reverse links");
        assert_eq!(
            reverse["0/1"],
            "## Reverse links\n\n- [[0/2]] User\n\n## Term uses\n\n- [[0/2]] User\n",
            "one source with both forms appears once in each section"
        );
        assert_eq!(
            reverse["0/3"],
            "## Term uses\n\n- [[0/4]] Non-head user\n",
            "a use of an existing non-head row is still listed under the Card it names"
        );
        assert_eq!(
            reverse["0/2"], "",
            "a source with no inbound Links ends at the marker"
        );
        assert_eq!(
            lifecycle.broken_links().expect("broken links"),
            [
                BrokenLink {
                    source_address: "0/2".to_string(),
                    source_title: "User".to_string(),
                    target: "0/9".to_string(),
                    label: Some("幽灵".to_string()),
                    reason: BrokenReason::Missing,
                    source_line: "见 [[均衡@0/1]] 与 [[幽灵@0/9]] and [[0/1]]".to_string(),
                },
                BrokenLink {
                    source_address: "0/4".to_string(),
                    source_title: "Non-head user".to_string(),
                    target: "0/3".to_string(),
                    label: Some("均衡".to_string()),
                    reason: BrokenReason::NotHead,
                    source_line: "见 [[均衡@0/3]]".to_string(),
                },
            ]
        );

        // A Term use to an existing row that is no Term head is Broken for rendering too.
        assert_eq!(
            lifecycle.classify_for_rendering("[[均衡@0/3]]"),
            [ClassifiedLink {
                target: "0/3",
                label: Some("均衡"),
                form: LinkForm::Term,
                macro_range: 0..14,
                status: LinkStatus::Broken,
                reason: Some(BrokenReason::NotHead),
            }]
        );
    }

    /// S11-AC06: the shell and Session line keeps its old shape for an ordinary Broken Link and
    /// names the authored label and the reason for a Broken Term use.
    #[test]
    fn a_broken_term_use_detail_names_its_label_and_reason() {
        let broken = |label: Option<&str>, reason| BrokenLink {
            source_address: "0/2".to_string(),
            source_title: "User".to_string(),
            target: "0/1".to_string(),
            label: label.map(str::to_string),
            reason,
            source_line: "line".to_string(),
        };
        assert_eq!(broken(None, BrokenReason::Missing).detail(), "0/1");
        assert_eq!(
            broken(Some("均衡"), BrokenReason::Missing).detail(),
            "0/1 (term `均衡`, missing)"
        );
        assert_eq!(
            broken(Some("均衡"), BrokenReason::NotHead).detail(),
            "0/1 (term `均衡`, not a Term head)"
        );
    }

    #[test]
    fn strict_link_body_rejects_invalid_targets_and_unmatched_delimiters() {
        for (text, expected) in [
            ("[[0/01]]", "invalid link macro target `0/01`"),
            ("[[bad key]]", "invalid link macro target `bad key`"),
            ("before [[0/1", "invalid link macro"),
            ("before ]]", "invalid link macro"),
        ] {
            let error = LinkBody::parse(text).expect_err("invalid Link body");
            assert_eq!(error.to_string(), expected);
        }
    }

    #[test]
    fn lifecycle_classifies_links_against_immutable_card_snapshots() {
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot::card("0/1", "Target", "target body")]);

        assert_eq!(
            lifecycle.classify_for_rendering("valid [[0/1]], broken [[Missing]]"),
            [
                ClassifiedLink {
                    target: "0/1",
                    label: None,
                    form: LinkForm::Ordinary,
                    macro_range: 6..13,
                    status: LinkStatus::Valid,
                    reason: None,
                },
                ClassifiedLink {
                    target: "Missing",
                    label: None,
                    form: LinkForm::Ordinary,
                    macro_range: 22..33,
                    status: LinkStatus::Broken,
                    reason: Some(BrokenReason::Missing),
                },
            ]
        );
    }

    #[test]
    fn edit_validation_preserves_old_broken_targets_but_rejects_new_ones() {
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot::card("0/2", "Valid Target", "target body")]);

        lifecycle
            .validate_edit(
                "keep [[Gone]] twice [[Gone]] and valid [[0/2]]",
                Some("old [[Gone]]"),
            )
            .expect("an old Broken target may remain at any occurrence count");

        let error = lifecycle
            .validate_edit("keep [[Gone]] and add [[Other]]", Some("old [[Gone]]"))
            .expect_err("a new Broken target must fail");
        assert_eq!(error.to_string(), "link target `Other` does not exist");
    }

    #[test]
    fn rendering_classification_keeps_valid_prefixes_and_marks_invalid_targets_broken() {
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot::card("0/1", "Target", "target body")]);

        assert_eq!(
            lifecycle.classify_for_rendering("ok [[0/1]] bad [[0/01]] tail [[open"),
            [
                ClassifiedLink {
                    target: "0/1",
                    label: None,
                    form: LinkForm::Ordinary,
                    macro_range: 3..10,
                    status: LinkStatus::Valid,
                    reason: None,
                },
                ClassifiedLink {
                    target: "0/01",
                    label: None,
                    form: LinkForm::Ordinary,
                    macro_range: 15..23,
                    status: LinkStatus::Broken,
                    reason: Some(BrokenReason::Missing),
                },
            ]
        );
    }

    #[test]
    fn rewriting_changes_only_mapped_target_ranges_and_counts_occurrences() {
        let body =
            LinkBody::parse("x [[0/1]] [[Old]] plain Old\r\n[[0/1]]").expect("valid Link body");
        let mapping = BTreeMap::from([
            ("0/1".to_string(), "0/2|a".to_string()),
            ("Old".to_string(), "New".to_string()),
        ]);

        assert_eq!(
            body.rewrite_targets(&mapping),
            LinkRewrite {
                body: "x [[0/2|a]] [[New]] plain Old\r\n[[0/2|a]]".to_string(),
                count: 3,
            }
        );
    }

    #[test]
    fn reverse_sections_deduplicate_each_source_and_order_sources_deterministically() {
        let lifecycle = LinkLifecycle::from_cards(vec![
            CardSnapshot::card("0/2", "Later Source", "[[0/1]] and again [[0/1]]"),
            CardSnapshot::card("0/1", "Target", "target"),
            CardSnapshot::card("0/0", "Earlier Source", "[[0/1]] and broken [[Missing]]"),
        ]);

        assert_eq!(
            lifecycle.reverse_sections().expect("derive Reverse links")["0/1"],
            "## Reverse links\n\n- [[0/0]] Earlier Source\n- [[0/2]] Later Source\n"
        );
    }

    #[test]
    fn links_inside_markdown_code_are_plain_text() {
        let body = "`[[nope]]` and ``a ]] b`` then [[0/1]]\n```rust\nlet x = v[[0]];\n```\n~~~\n]]\n~~~\n[[LitA]]";
        let targets = LinkBody::parse(body)
            .expect("code is ignored by strict parsing")
            .occurrences()
            .iter()
            .map(|occurrence| occurrence.target)
            .collect::<Vec<_>>();
        assert_eq!(targets, ["0/1", "LitA"]);
        assert_eq!(
            LinkBody::parse("```\n[[x]]\nunclosed")
                .expect("an unclosed fence runs to the end")
                .occurrences(),
            []
        );
        assert_eq!(
            LinkBody::parse("`unmatched [[0/1]]")
                .expect("an unmatched backtick is literal")
                .occurrences()
                .len(),
            1
        );
    }

    #[test]
    fn broken_links_preserve_each_occurrence_and_its_exact_source_line() {
        let lifecycle = LinkLifecycle::from_cards(vec![
            CardSnapshot::card(
                "0/0",
                "Source",
                "first [[Missing]] and [[Missing]]\nvalid [[0/1]]\nsecond [[Other]]",
            ),
            CardSnapshot::card("0/1", "Target", "target"),
        ]);

        assert_eq!(
            lifecycle.broken_links().expect("derive Broken links"),
            [
                BrokenLink {
                    source_address: "0/0".to_string(),
                    source_title: "Source".to_string(),
                    target: "Missing".to_string(),
                    label: None,
                    reason: BrokenReason::Missing,
                    source_line: "first [[Missing]] and [[Missing]]".to_string(),
                },
                BrokenLink {
                    source_address: "0/0".to_string(),
                    source_title: "Source".to_string(),
                    target: "Missing".to_string(),
                    label: None,
                    reason: BrokenReason::Missing,
                    source_line: "first [[Missing]] and [[Missing]]".to_string(),
                },
                BrokenLink {
                    source_address: "0/0".to_string(),
                    source_title: "Source".to_string(),
                    target: "Other".to_string(),
                    label: None,
                    reason: BrokenReason::Missing,
                    source_line: "second [[Other]]".to_string(),
                },
            ]
        );
    }
}
