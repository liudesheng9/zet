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

/// Link macros are recognized in Markdown prose only: text inside inline code
/// spans and fenced code blocks is never a Link.
fn scan_occurrences(text: &str, strict: bool) -> Result<Vec<LinkOccurrence<'_>>> {
    let code = code_ranges(text);
    let in_code = |index: usize| code.iter().find(|range| range.contains(&index));
    let mut occurrences = Vec::new();
    let mut offset = 0;
    while let Some(start_relative) = text[offset..].find("[[") {
        let start = offset + start_relative;
        if let Some(range) = in_code(start) {
            offset = range.end;
            continue;
        }
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
    if strict {
        let mut tail = offset;
        while let Some(relative) = text[tail..].find("]]") {
            let index = tail + relative;
            match in_code(index) {
                Some(range) => tail = range.end.max(index + 1),
                None => bail!("invalid link macro"),
            }
        }
    }
    Ok(occurrences)
}

/// Byte ranges of fenced code blocks and inline code spans in Markdown text.
pub(crate) fn code_ranges(text: &str) -> Vec<Range<usize>> {
    let (mut fenced, inline) = scan_code(text);
    fenced.extend(inline);
    fenced.sort_by_key(|range| range.start);
    fenced
}

/// Byte ranges of fenced code blocks, each from its opening fence line
/// through its closing fence line.
pub(crate) fn fenced_code_ranges(text: &str) -> Vec<Range<usize>> {
    scan_code(text).0
}

fn scan_code(text: &str) -> (Vec<Range<usize>>, Vec<Range<usize>>) {
    let mut ranges = Vec::new();
    let mut inline = Vec::new();
    let mut fence: Option<(u8, usize, usize)> = None;
    let mut prose_start = 0;
    let mut line_start = 0;
    for line in text.split_inclusive('\n') {
        let line_end = line_start + line.len();
        let content = line.trim_end_matches(['\n', '\r']);
        let indent = content.len() - content.trim_start_matches(' ').len();
        let trimmed = &content[indent..];
        let run = |ch: u8| trimmed.bytes().take_while(|byte| *byte == ch).count();
        match fence {
            None if indent <= 3 && (run(b'`') >= 3 || run(b'~') >= 3) => {
                let ch = trimmed.as_bytes()[0];
                inline_code_ranges(text, prose_start..line_start, &mut inline);
                fence = Some((ch, run(ch), line_start));
            }
            Some((ch, count, start))
                if indent <= 3 && run(ch) >= count && trimmed[run(ch)..].trim().is_empty() =>
            {
                ranges.push(start..line_end);
                fence = None;
                prose_start = line_end;
            }
            _ => {}
        }
        line_start = line_end;
    }
    match fence {
        Some((_, _, start)) => ranges.push(start..text.len()),
        None => inline_code_ranges(text, prose_start..text.len(), &mut inline),
    }
    (ranges, inline)
}

fn inline_code_ranges(text: &str, within: Range<usize>, ranges: &mut Vec<Range<usize>>) {
    let bytes = text.as_bytes();
    let run_at = |index: usize| {
        bytes[index..within.end]
            .iter()
            .take_while(|byte| **byte == b'`')
            .count()
    };
    let mut index = within.start;
    while index < within.end {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }
        let open = run_at(index);
        let mut search = index + open;
        let mut close = None;
        while search < within.end {
            if bytes[search] == b'`' {
                let run = run_at(search);
                if run == open {
                    close = Some(search + run);
                    break;
                }
                search += run;
            } else {
                search += 1;
            }
        }
        match close {
            Some(end) => {
                ranges.push(index..end);
                index = end;
            }
            None => index += open,
        }
    }
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

fn line_around(text: &str, index: usize) -> &str {
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
