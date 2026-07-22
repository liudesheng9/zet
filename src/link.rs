use anyhow::{Result, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkOccurrence<'a> {
    pub(crate) target: &'a str,
    pub(crate) macro_range: Range<usize>,
    pub(crate) target_range: Range<usize>,
}

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
        Ok(Self {
            text,
            occurrences: scan_occurrences(text, true)?,
        })
    }

    fn for_rendering(text: &'a str) -> Self {
        Self {
            text,
            occurrences: scan_occurrences(text, false)
                .expect("tolerant Link scanning cannot return an error"),
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

fn scan_occurrences(text: &str, strict: bool) -> Result<Vec<LinkOccurrence<'_>>> {
    let mut occurrences = Vec::new();
    let mut offset = 0;
    while let Some(start_relative) = text[offset..].find("[[") {
        let start = offset + start_relative;
        let target_start = start + 2;
        let Some(end_relative) = text[target_start..].find("]]") else {
            if strict {
                bail!("invalid link macro");
            }
            break;
        };
        let target_end = target_start + end_relative;
        let target = &text[target_start..target_end];
        if strict
            && !crate::is_valid_location(target)
            && crate::literature::validate_citation_key(target).is_err()
        {
            bail!("invalid link macro target `{target}`");
        }
        occurrences.push(LinkOccurrence {
            target,
            macro_range: start..target_end + 2,
            target_range: target_start..target_end,
        });
        offset = target_end + 2;
    }
    if strict && text[offset..].contains("]]") {
        bail!("invalid link macro");
    }
    Ok(occurrences)
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

    pub(crate) fn validate_edit<'a>(
        &self,
        body: &'a str,
        old_body: Option<&str>,
    ) -> Result<LinkBody<'a>> {
        let old_broken: BTreeSet<String> = match old_body {
            Some(old_body) => LinkBody::parse(old_body)?
                .occurrences()
                .iter()
                .filter(|occurrence| !self.addresses.contains(occurrence.target))
                .map(|occurrence| occurrence.target.to_string())
                .collect(),
            None => BTreeSet::new(),
        };
        let body = LinkBody::parse(body)?;
        for occurrence in body.occurrences() {
            if !self.addresses.contains(occurrence.target)
                && !old_broken.contains(occurrence.target)
            {
                bail!("link target `{}` does not exist", occurrence.target);
            }
        }
        Ok(body)
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
                let reverse = inbound
                    .get(card.address.as_str())
                    .into_iter()
                    .flat_map(|sources| sources.iter())
                    .map(|source| {
                        format!(
                            "This note has been referred by note [[{source}]] {}",
                            titles.get(*source).copied().unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (card.address.clone(), reverse)
            })
            .collect())
    }

    pub(crate) fn broken_links(&self) -> Result<Vec<BrokenLink>> {
        let mut broken = Vec::new();
        for card in &self.cards {
            for line in card.body.lines() {
                for occurrence in LinkBody::parse(line)?.occurrences() {
                    if !self.addresses.contains(occurrence.target) {
                        broken.push(BrokenLink {
                            source_address: card.address.clone(),
                            source_title: card.title.clone(),
                            target: occurrence.target.to_string(),
                            source_line: line.to_string(),
                        });
                    }
                }
            }
        }
        Ok(broken)
    }
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
                    macro_range: 4..11,
                    target_range: 6..9,
                },
                LinkOccurrence {
                    target: "LitA",
                    macro_range: 16..24,
                    target_range: 18..22,
                },
            ]
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
            "This note has been referred by note [[0/0]] Earlier Source\n\
             This note has been referred by note [[0/2]] Later Source"
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
