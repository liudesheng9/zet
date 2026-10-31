//! The pure facts the Write scope reads (CONTEXT.md, docs/agent-write-scope-proposal.md): which
//! addresses are in a Topic tree, which Topic-tree Cards authored text links, and how alike two
//! Topic titles are. The zt server's Write gate and the `zt mcp` client both use them, so the rule
//! has one implementation. A Topic tree is a tree whose id is a Topic number (all ASCII digits),
//! so none of this needs an archive read.

use crate::text::{LinkForm, link_occurrences};
use std::collections::HashSet;

/// The generated section a Card read ends with; its Links are the archive's, not authored.
const REVERSE_LINKS: &str = "<!-- zt:reverse-links -->";

/// Whether an address (a Location, a Citation key, or a Link target) is in a Topic tree.
pub fn is_topic_address(address: &str) -> bool {
    let tree = address.trim().split('/').next().unwrap_or_default();
    !tree.is_empty() && tree.bytes().all(|b| b.is_ascii_digit())
}

/// The Topic-tree Link targets of authored text, ignoring the generated Reverse links section.
/// R0272 (S12, #154): each macro's actual target comes from the shared Link scan, so a Term use
/// names the address after `@`, never the `label@target` text the old bracket regex reported. A
/// malformed Term-like macro is no Link; the strict write path refuses it on its own.
pub fn topic_links(text: &str) -> Vec<String> {
    let authored = text.split(REVERSE_LINKS).next().unwrap_or_default();
    link_occurrences(authored)
        .into_iter()
        .filter(|occurrence| match occurrence.form {
            LinkForm::Ordinary => true,
            LinkForm::Term => occurrence.is_valid_term_macro(),
        })
        .map(|occurrence| occurrence.target.trim().to_string())
        .filter(|target| is_topic_address(target))
        .collect()
}

/// A title without whitespace, punctuation and symbols, in lower case: Topic titles compare by it.
pub fn title_key(title: &str) -> String {
    title
        .chars()
        .filter(|c| !c.is_whitespace() && !is_punctuation_or_symbol(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Unicode punctuation (P*) and symbol (S*) categories, as the regex class `[\p{P}\p{S}]` reads them.
fn is_punctuation_or_symbol(c: char) -> bool {
    use regex::Regex;
    use std::sync::LazyLock;
    static CLASS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\p{P}\p{S}]$").unwrap());
    let mut buffer = [0u8; 4];
    CLASS.is_match(c.encode_utf8(&mut buffer))
}

/// The Dice coefficient of the character bigrams of two title keys (1.0 for equal one-character keys).
pub fn similarity(a: &str, b: &str) -> f64 {
    let grams = |t: &str| {
        let chars: Vec<char> = t.chars().collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect::<HashSet<_>>()
    };
    let (x, y) = (grams(a), grams(b));
    if x.is_empty() || y.is_empty() {
        return if a == b { 1.0 } else { 0.0 };
    }
    let common = x.iter().filter(|g| y.contains(g)).count();
    2.0 * common as f64 / (x.len() + y.len()) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topic_addresses_are_numbered_trees() {
        assert!(is_topic_address("408/1|a"));
        assert!(is_topic_address("12/0"));
        assert!(!is_topic_address("grossman2002integration/1|d"));
        assert!(!is_topic_address("Smith2024"));
        assert!(!is_topic_address(""));
    }

    #[test]
    fn titles_compare_without_punctuation_and_case() {
        assert_eq!(title_key("Price, Theory!"), "pricetheory");
        assert_eq!(title_key("价格 理论：导论"), "价格理论导论");
        assert!(similarity("价格理论导论", "价格理论导引") >= 0.8);
        assert!(similarity("价格理论", "货币银行") < 0.8);
        assert_eq!(similarity("a", "a"), 1.0);
    }
}
