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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LinkStatus {
    Valid,
    Broken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassifiedLink<'a> {
    pub(crate) target: &'a str,
    pub(crate) macro_range: Range<usize>,
    pub(crate) status: LinkStatus,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BrokenLink {
    pub(crate) source_address: String,
    pub(crate) source_title: String,
    pub(crate) target: String,
    pub(crate) source_line: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkLifecycle {
    cards: Vec<CardSnapshot>,
    addresses: BTreeSet<String>,
}

impl LinkLifecycle {
    pub(crate) fn from_cards(cards: Vec<CardSnapshot>) -> Self {
        let addresses = cards.iter().map(|card| card.address.clone()).collect();
        Self { cards, addresses }
    }

    pub(crate) fn classify_for_rendering<'a>(&self, body: &'a str) -> Vec<ClassifiedLink<'a>> {
        LinkBody::for_rendering(body)
            .occurrences()
            .iter()
            .map(|occurrence| ClassifiedLink {
                target: occurrence.target,
                macro_range: occurrence.macro_range.clone(),
                status: if (crate::is_valid_location(occurrence.target)
                    || crate::literature::validate_citation_key(occurrence.target).is_ok())
                    && self.addresses.contains(occurrence.target)
                {
                    LinkStatus::Valid
                } else {
                    LinkStatus::Broken
                },
            })
            .collect()
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

    pub(crate) fn reverse_sections(&self) -> Result<BTreeMap<String, String>> {
        let titles: BTreeMap<&str, &str> = self
            .cards
            .iter()
            .map(|card| (card.address.as_str(), card.title.as_str()))
            .collect();
        let mut inbound: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
        for source in &self.cards {
            let body = LinkBody::parse(&source.body)?;
            let mut seen = BTreeSet::new();
            for occurrence in body.occurrences() {
                // A Term use is no reasoning Link: its own "Term uses" section is design 10.3's
                // later slice (S11, #153), so the ordinary Reverse links ignore it here.
                if occurrence.form != LinkForm::Ordinary {
                    continue;
                }
                if self.addresses.contains(occurrence.target) && seen.insert(occurrence.target) {
                    inbound
                        .entry(occurrence.target)
                        .or_default()
                        .insert(&source.address);
                }
            }
        }
        Ok(self
            .cards
            .iter()
            .map(|card| {
                let sources = inbound
                    .get(card.address.as_str())
                    .into_iter()
                    .flat_map(|sources| sources.iter())
                    .map(|source| (*source, titles.get(*source).copied().unwrap_or_default()));
                (card.address.clone(), crate::card::reverse_section(sources))
            })
            .collect())
    }

    pub(crate) fn broken_links(&self) -> Result<Vec<BrokenLink>> {
        let mut broken = Vec::new();
        for card in &self.cards {
            for occurrence in LinkBody::parse(&card.body)?.occurrences() {
                if !self.addresses.contains(occurrence.target) {
                    broken.push(BrokenLink {
                        source_address: card.address.clone(),
                        source_title: card.title.clone(),
                        target: occurrence.target.to_string(),
                        source_line: line_around(&card.body, occurrence.macro_range.start)
                            .to_string(),
                    });
                }
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

    /// R0254/S10-AC07: the tolerant scan classifies a Term use by its address, an ordinary
    /// Reverse section ignores it (its own section is S11), and a missing Term target is Broken.
    #[test]
    fn rendering_broken_reads_and_reverse_links_see_a_term_use_by_its_address() {
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot {
            address: "0/1".to_string(),
            title: "Target".to_string(),
            body: "target".to_string(),
        }]);
        assert_eq!(
            lifecycle.classify_for_rendering("见 [[均衡@0/1]] 与 [[幽灵@0/9]]"),
            [
                ClassifiedLink {
                    target: "0/1",
                    macro_range: 4..18,
                    status: LinkStatus::Valid,
                },
                ClassifiedLink {
                    target: "0/9",
                    macro_range: 23..37,
                    status: LinkStatus::Broken,
                },
            ]
        );

        let lifecycle = LinkLifecycle::from_cards(vec![
            CardSnapshot {
                address: "0/1".to_string(),
                title: "Target".to_string(),
                body: "target".to_string(),
            },
            CardSnapshot {
                address: "0/2".to_string(),
                title: "User".to_string(),
                body: "见 [[均衡@0/1]] 与 [[幽灵@0/9]]".to_string(),
            },
        ]);
        assert_eq!(
            lifecycle.reverse_sections().expect("reverse links")["0/1"],
            "",
            "a Term use is no ordinary Reverse link"
        );
        assert_eq!(
            lifecycle.broken_links().expect("broken links"),
            [BrokenLink {
                source_address: "0/2".to_string(),
                source_title: "User".to_string(),
                target: "0/9".to_string(),
                source_line: "见 [[均衡@0/1]] 与 [[幽灵@0/9]]".to_string(),
            }]
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
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot {
            address: "0/1".to_string(),
            title: "Target".to_string(),
            body: "target body".to_string(),
        }]);

        assert_eq!(
            lifecycle.classify_for_rendering("valid [[0/1]], broken [[Missing]]"),
            [
                ClassifiedLink {
                    target: "0/1",
                    macro_range: 6..13,
                    status: LinkStatus::Valid,
                },
                ClassifiedLink {
                    target: "Missing",
                    macro_range: 22..33,
                    status: LinkStatus::Broken,
                },
            ]
        );
    }

    #[test]
    fn edit_validation_preserves_old_broken_targets_but_rejects_new_ones() {
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot {
            address: "0/2".to_string(),
            title: "Valid Target".to_string(),
            body: "target body".to_string(),
        }]);

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
        let lifecycle = LinkLifecycle::from_cards(vec![CardSnapshot {
            address: "0/1".to_string(),
            title: "Target".to_string(),
            body: "target body".to_string(),
        }]);

        assert_eq!(
            lifecycle.classify_for_rendering("ok [[0/1]] bad [[0/01]] tail [[open"),
            [
                ClassifiedLink {
                    target: "0/1",
                    macro_range: 3..10,
                    status: LinkStatus::Valid,
                },
                ClassifiedLink {
                    target: "0/01",
                    macro_range: 15..23,
                    status: LinkStatus::Broken,
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
            CardSnapshot {
                address: "0/2".to_string(),
                title: "Later Source".to_string(),
                body: "[[0/1]] and again [[0/1]]".to_string(),
            },
            CardSnapshot {
                address: "0/1".to_string(),
                title: "Target".to_string(),
                body: "target".to_string(),
            },
            CardSnapshot {
                address: "0/0".to_string(),
                title: "Earlier Source".to_string(),
                body: "[[0/1]] and broken [[Missing]]".to_string(),
            },
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
            CardSnapshot {
                address: "0/0".to_string(),
                title: "Source".to_string(),
                body: "first [[Missing]] and [[Missing]]\nvalid [[0/1]]\nsecond [[Other]]"
                    .to_string(),
            },
            CardSnapshot {
                address: "0/1".to_string(),
                title: "Target".to_string(),
                body: "target".to_string(),
            },
        ]);

        assert_eq!(
            lifecycle.broken_links().expect("derive Broken links"),
            [
                BrokenLink {
                    source_address: "0/0".to_string(),
                    source_title: "Source".to_string(),
                    target: "Missing".to_string(),
                    source_line: "first [[Missing]] and [[Missing]]".to_string(),
                },
                BrokenLink {
                    source_address: "0/0".to_string(),
                    source_title: "Source".to_string(),
                    target: "Missing".to_string(),
                    source_line: "first [[Missing]] and [[Missing]]".to_string(),
                },
                BrokenLink {
                    source_address: "0/0".to_string(),
                    source_title: "Source".to_string(),
                    target: "Other".to_string(),
                    source_line: "second [[Other]]".to_string(),
                },
            ]
        );
    }
}
