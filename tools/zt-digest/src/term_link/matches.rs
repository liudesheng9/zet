//! The bounded literal matcher of the deterministic Term-link pass (design 13.5, S22 / #160).
//!
//! There are at most the default 60 table terms, so one sorted literal scan is easier to inspect
//! than an automaton and keeps leftmost-longest semantics exactly. At each position the longest
//! word that starts there wins; a blocker wins an exact tie with a link word. Link words are exact
//! term names with one valid head and automatic linking enabled. Blockers are every avoid word,
//! every name without a usable head or with automatic linking off, the conservative short names,
//! the ambiguous bare form of qualified homonyms, and the names that publish no table row.

use crate::canon;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// The occurrence policy of one pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Policy {
    /// The first eligible unblocked occurrence of each exact term in each body.
    First,
    /// Every eligible unblocked occurrence.
    All,
}

impl Policy {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Policy::First => "first",
            Policy::All => "all",
        }
    }
}

/// One table row, joined to the exact names that publish it and the head's auto-link choice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Term {
    /// The 1-based table row.
    pub(crate) row: usize,
    /// The exact spelling the table and its head share.
    pub(crate) term: String,
    /// The row's `不用` words.
    pub(crate) avoid: Vec<String>,
    /// Every valid Term head that publishes this exact spelling.
    pub(crate) heads: Vec<String>,
    /// Whether the head leaves automatic linking enabled (absence of `自动链接：否`).
    pub(crate) auto_link: bool,
}

impl Term {
    /// Why this exact name cannot be a link word; `None` when it can.
    pub(crate) fn blocker(&self) -> Option<&'static str> {
        if self.heads.is_empty() {
            Some("no_head")
        } else if self.heads.len() > 1 {
            Some("duplicate_head")
        } else if !self.auto_link {
            Some("auto_link_off")
        } else if short_name(&self.term) {
            Some("short")
        } else {
            None
        }
    }

    /// The one head a link word names.
    pub(crate) fn link_head(&self) -> Option<&str> {
        self.blocker()
            .is_none()
            .then(|| self.heads.first().map(String::as_str))
            .flatten()
    }
}

/// A word the scan can meet: a link word naming one term row, or a blocker that consumes the text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Word {
    pub(crate) text: String,
    /// The index into [`Vocabulary::terms`] of the link word.
    pub(crate) link: Option<usize>,
    /// The skip reason of a blocker.
    pub(crate) blocker: Option<&'static str>,
}

/// The vocabulary of one book pass.
#[derive(Debug)]
pub(crate) struct Vocabulary {
    pub(crate) terms: Vec<Term>,
    /// Every word, in construction order.
    words: Vec<Word>,
    /// Candidate word indices by first byte, each list longest first.
    by_first: BTreeMap<u8, Vec<usize>>,
    /// The canonical term→head mapping and the canonical auto-link choices (R0359).
    pub(crate) mapping_hash: String,
    pub(crate) autolink_hash: String,
}

impl Vocabulary {
    /// Build the vocabulary from the book's rows, its head names, and the extra named Term Cards
    /// that publish no row (a part or an unfiled Card still protects its own spelling).
    pub(crate) fn build(terms: &[Term], unrowed: &[String]) -> Vocabulary {
        let mut words: Vec<Word> = Vec::new();
        let mut insert = |text: &str, link: Option<usize>, blocker: Option<&'static str>| {
            if text.trim().is_empty() {
                return;
            }
            if let Some(existing) = words.iter_mut().find(|word| word.text == text) {
                // A blocker always wins its exact tie with a link word, and the first row order
                // decides between two blockers. A link word never displaces a blocker.
                if existing.blocker.is_none() && blocker.is_some() {
                    existing.link = None;
                    existing.blocker = blocker;
                } else if existing.link.is_none() && existing.blocker.is_none() {
                    existing.link = link;
                }
                return;
            }
            words.push(Word {
                text: text.to_string(),
                link,
                blocker,
            });
        };
        for (index, term) in terms.iter().enumerate() {
            match term.blocker() {
                None => insert(&term.term, Some(index), None),
                Some(reason) => insert(&term.term, None, Some(reason)),
            }
        }
        for term in terms {
            for avoid in &term.avoid {
                insert(avoid, None, Some("avoid_word"));
            }
        }
        // Ambiguous unqualified forms of qualified homonyms: 学习（个体行为） and 学习（产业生产率）
        // keep both full forms linkable, and the bare 学习 becomes a blocker.
        let mut bases: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for term in terms {
            if let Some(base) = qualified_base(&term.term) {
                bases.entry(base).or_default().push(&term.term);
            }
        }
        for (base, spellings) in &bases {
            if spellings.len() > 1 {
                insert(base, None, Some("ambiguous"));
            }
        }
        // A named Term Card that publishes no exact row is still a spelling a shorter link word
        // must not link inside.
        for name in unrowed {
            insert(name, None, Some("no_row"));
        }

        let mut by_first: BTreeMap<u8, Vec<usize>> = BTreeMap::new();
        for (index, word) in words.iter().enumerate() {
            if let Some(first) = word.text.as_bytes().first() {
                by_first.entry(*first).or_default().push(index);
            }
        }
        for candidates in by_first.values_mut() {
            candidates.sort_by(|a, b| {
                words[*b]
                    .text
                    .len()
                    .cmp(&words[*a].text.len())
                    .then_with(|| a.cmp(b))
            });
        }
        let mapping = Value::Array(
            terms
                .iter()
                .map(|term| {
                    json!({
                        "row": term.row,
                        "term": term.term,
                        "head": term.heads.first(),
                        "heads": term.heads,
                    })
                })
                .collect(),
        );
        let autolink = Value::Array(
            terms
                .iter()
                .map(|term| {
                    json!({
                        "row": term.row,
                        "term": term.term,
                        "auto_link": term.auto_link,
                    })
                })
                .collect(),
        );
        Vocabulary {
            terms: terms.to_vec(),
            words,
            by_first,
            mapping_hash: canon::hash_value(&mapping),
            autolink_hash: canon::hash_value(&autolink),
        }
    }

    /// The longest word matching at `index`, with its end. A Latin edge must have the ASCII word
    /// boundaries the shared rule names; a blocker wins an exact tie (`GDP` vs an avoid word
    /// `GDP`, or the same name both linkable and avoided).
    pub(crate) fn longest_at(&self, body: &str, index: usize) -> Option<(usize, &Word)> {
        let first = *body.as_bytes().get(index)?;
        let mut best: Option<usize> = None;
        for &candidate in self.by_first.get(&first)? {
            let word = &self.words[candidate];
            if !body[index..].starts_with(&word.text) {
                continue;
            }
            let end = index + word.text.len();
            if !super::spans::boundaries_ok(body, index, end) {
                continue;
            }
            let replace = match best {
                None => true,
                Some(held) => {
                    let held = &self.words[held];
                    word.text.len() > held.text.len()
                        || (word.text.len() == held.text.len()
                            && word.blocker.is_some()
                            && held.blocker.is_none())
                }
            };
            if replace {
                best = Some(candidate);
            }
        }
        best.map(|candidate| (index + self.words[candidate].text.len(), &self.words[candidate]))
    }

    /// The link-word occurrences inside one protected span, leftmost-longest, as `(start, end)`.
    /// A blocker still consumes its text, so a link word inside a longer avoid word is not
    /// reported as a potential use. They are reported skips: the span's own reason is why no link
    /// is made.
    pub(crate) fn link_occurrences_in(&self, body: &str, range: &std::ops::Range<usize>) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        let mut index = range.start;
        while index < range.end {
            if let Some((end, word)) = self.longest_at(body, index)
                && end <= range.end
            {
                if word.blocker.is_none() {
                    found.push((index, end));
                }
                index = end;
                continue;
            }
            index += char_len(body, index);
        }
        found
    }
}

/// The Literature tree a Term-link target names: the Citation key before the first `/` (or the
/// whole target), and `None` for a numbered Topic address. This is the same tree test the digest's
/// Literature scope uses, so a Term head at `key/1|b|a` belongs to the book `key`, while `12/3` is
/// no book at all.
pub(crate) fn literature_owner(target: &str) -> Option<&str> {
    let tree = target.split_once('/').map_or(target, |(tree, _)| tree);
    (!tree.is_empty() && !tree.bytes().all(|byte| byte.is_ascii_digit())).then_some(tree)
}

/// The exact link word for one spelling, when it has one valid head and automatic linking on.
pub(crate) fn link_word<'a>(vocabulary: &'a Vocabulary, text: &str) -> Option<(&'a str, usize)> {
    let (index, term) = vocabulary
        .terms
        .iter()
        .enumerate()
        .find(|(_, term)| term.term == text)?;
    let head = term.link_head()?;
    Some((head, index))
}

/// The bare form of a qualified term: `学习（个体行为）` → `学习`.
fn qualified_base(term: &str) -> Option<&str> {
    let trimmed = term.trim_end();
    let (open, close) = if trimmed.ends_with('）') {
        ('（', '）')
    } else if trimmed.ends_with(')') {
        ('(', ')')
    } else {
        return None;
    };
    let at = trimmed.rfind(open)?;
    let base = &trimmed[..at];
    (!base.is_empty() && trimmed[at..].starts_with(open) && trimmed[at..].ends_with(close)).then_some(base)
}

/// The conservative short-name default: no one-Han-character term and no Latin name under three
/// letters. `GDP` already has three letters, so it links; a one-letter symbol does not.
fn short_name(term: &str) -> bool {
    let han = term.chars().filter(|ch| is_han(*ch)).count();
    if han == 1 && term.chars().count() == 1 {
        return true;
    }
    han == 0 && term.chars().filter(|ch| ch.is_ascii_alphabetic()).count() < 3
}

fn is_han(ch: char) -> bool {
    matches!(ch, '\u{3400}'..='\u{9FFF}')
}

fn char_len(body: &str, index: usize) -> usize {
    body[index..].chars().next().map_or(1, char::len_utf8)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn term(row: usize, name: &str, avoid: &[&str], heads: &[&str], auto_link: bool) -> Term {
        Term {
            row,
            term: name.to_string(),
            avoid: avoid.iter().map(|word| word.to_string()).collect(),
            heads: heads.iter().map(|head| head.to_string()).collect(),
            auto_link,
        }
    }

    fn book() -> Vocabulary {
        Vocabulary::build(
            &[
                term(1, "替代弹性", &["替代弹性系数"], &["K/3"], true),
                term(2, "净生产函数", &[], &["K/4"], true),
                term(3, "生产函数", &[], &["K/5"], true),
                term(4, "相对要素价格", &[], &[], true),
                term(5, "要素价格", &[], &["K/6"], true),
                term(6, "学习（个体行为）", &[], &["K/7"], true),
                term(7, "学习（产业生产率）", &[], &["K/8"], true),
                term(8, "GDP", &[], &["K/9"], true),
                term(9, "学", &[], &["K/10"], true),
                term(10, "ABC", &[], &["K/11"], false),
            ],
            &["XYZ".to_string()],
        )
    }

    fn word<'a>(vocabulary: &'a Vocabulary, text: &str) -> (usize, &'a Word) {
        let (end, found) = vocabulary
            .longest_at(text, 0)
            .unwrap_or_else(|| panic!("no word at the start of {text:?}"));
        (end, found)
    }

    #[test]
    fn link_words_and_blockers_are_built_from_heads_avoid_words_and_the_short_rule() {
        let vocabulary = book();
        assert_eq!(word(&vocabulary, "替代弹性").1.link, Some(0));
        assert_eq!(word(&vocabulary, "替代弹性系数").1.blocker, Some("avoid_word"));
        assert_eq!(word(&vocabulary, "相对要素价格").1.blocker, Some("no_head"));
        assert_eq!(word(&vocabulary, "要素价格").1.link, Some(4));
        assert_eq!(word(&vocabulary, "学").1.blocker, Some("short"));
        assert_eq!(word(&vocabulary, "ABC").1.blocker, Some("auto_link_off"));
        assert_eq!(word(&vocabulary, "XYZ").1.blocker, Some("no_row"));
        // A duplicated head makes the spelling ambiguous, not linkable.
        let duplicated = Vocabulary::build(&[term(1, "均衡", &[], &["K/3", "K/4"], true)], &[]);
        assert_eq!(word(&duplicated, "均衡").1.blocker, Some("duplicate_head"));
    }

    #[test]
    fn qualified_homonyms_block_only_the_bare_form() {
        let vocabulary = book();
        assert_eq!(word(&vocabulary, "学习（个体行为）").1.link, Some(5));
        assert_eq!(word(&vocabulary, "学习（产业生产率）").1.link, Some(6));
        assert_eq!(word(&vocabulary, "学习").1.blocker, Some("ambiguous"));
        // A qualified spelling is no prefix of the other one.
        assert_eq!(word(&vocabulary, "学习（个体行为）").0, "学习（个体行为）".len());
    }

    #[test]
    fn the_longest_word_wins_and_latin_boundaries_and_case_are_exact() {
        let vocabulary = book();
        assert_eq!(word(&vocabulary, "净生产函数").1.text, "净生产函数");
        assert_eq!(word(&vocabulary, "生产函数").1.text, "生产函数");
        assert_eq!(word(&vocabulary, "GDP 增长").1.text, "GDP");
        assert_eq!(
            vocabulary.longest_at("GDPR", 0).map(|(_, word)| word.text.clone()),
            None,
            "GDP inside GDPR is no match"
        );
        assert!(vocabulary.longest_at("GDP_2024", 0).is_none());
        assert!(
            vocabulary.longest_at("gdp", 0).is_none(),
            "Latin matching is case-sensitive"
        );
        // A shorter word inside a longer one is consumed together with it.
        let (end, found) = word(&vocabulary, "相对要素价格与要素价格");
        assert_eq!(found.blocker, Some("no_head"));
        assert_eq!(end, "相对要素价格".len());
    }

    #[test]
    fn a_blocker_wins_the_exact_tie_and_hashes_freeze_mapping_and_choices() {
        // `均衡` is linkable for its own row and an avoid word of another: it stays plain text.
        let vocabulary = Vocabulary::build(
            &[
                term(1, "均衡", &[], &["K/3"], true),
                term(2, "多重性", &["均衡"], &["K/4"], true),
            ],
            &[],
        );
        let (_, found) = word(&vocabulary, "均衡");
        assert_eq!(found.blocker, Some("avoid_word"));
        assert_eq!(found.link, None);
        // The same rows build the same exact fingerprints; a changed auto-link choice changes only
        // the choice fingerprint.
        let rows = || {
            vec![
                term(1, "均衡", &[], &["K/3"], true),
                term(2, "多重性", &["均衡"], &["K/4"], true),
            ]
        };
        let again = Vocabulary::build(&rows(), &[]);
        assert_eq!(vocabulary.mapping_hash, again.mapping_hash);
        assert_eq!(vocabulary.autolink_hash, again.autolink_hash);
        let mut changed_rows = rows();
        changed_rows[0].auto_link = false;
        let changed = Vocabulary::build(&changed_rows, &[]);
        // The same rows keep the mapping fingerprint when only the auto-link choice changes.
        assert_eq!(changed.mapping_hash, vocabulary.mapping_hash);
        assert_ne!(changed.autolink_hash, vocabulary.autolink_hash);
    }

    #[test]
    fn protected_span_occurrences_report_each_link_word_once() {
        let vocabulary = book();
        let text = "净生产函数 与 生产函数 与 替代弹性系数 与 学习（个体行为）";
        let found: Vec<&str> = vocabulary
            .link_occurrences_in(text, &(0..text.len()))
            .into_iter()
            .map(|(start, end)| &text[start..end])
            .collect();
        assert_eq!(found, ["净生产函数", "生产函数", "学习（个体行为）"]);
    }
}
