//! The pure facts the Write scope reads (CONTEXT.md, docs/agent-write-scope-proposal.md): which
//! addresses are in a Topic tree, which Topic-tree Cards authored text links, and how alike two
//! Topic titles are. The zt server's Write gate and the `zt mcp` client both use them, so the rule
//! has one implementation. A Topic tree is a tree whose id is a Topic number (all ASCII digits),
//! so none of this needs an archive read.

use crate::text::{LinkForm, link_occurrences};
use serde_json::Value;
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

/// A write the Literature scope refuses: the stable reason, the address it names, and the sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Violation {
    /// `topic_tree`, `hub` or `topic_link`.
    pub reason: &'static str,
    pub at: String,
    pub message: String,
}

/// Why the Literature scope refuses this one call (never a batch: each item is checked as its own
/// call), or None when it may write. Reads pass. A Literature-scope connection writes only
/// Literature trees: it creates no Topic tree, names no Topic-tree Card as a destination, makes no
/// Hub Card, and puts no Link to a Topic-tree Card into authored text. Whether an existing Card is a
/// Hub Card needs an archive read; the server adds that check.
pub fn literature_violation(tool: &str, arguments: &Value) -> Option<Violation> {
    let text = |name: &str| arguments.get(name).and_then(Value::as_str).unwrap_or_default();
    let scope = "this connection writes only Literature trees";
    let violation = |reason, at: &str, message: String| Some(Violation { reason, at: at.to_string(), message });
    match tool {
        "create_topic" => violation(
            "topic_tree",
            "",
            format!("{scope}: it creates no Topic tree; Topic trees belong to zt-idea-grow"),
        ),
        "create_successor" | "edit_card" | "insert_card" | "move_card" | "delete_card" | "edit_literature_metadata" | "set_literature_type" => {
            for name in ["at", "before", "to", "into"] {
                let address = text(name);
                if is_topic_address(address) {
                    return violation(
                        "topic_tree",
                        address,
                        format!("{scope}: `{address}` is a Topic-tree Card; Topic trees belong to zt-idea-grow"),
                    );
                }
            }
            if text("type").eq_ignore_ascii_case("hub") {
                return violation("hub", text("at"), format!("{scope}: it makes or changes no Hub Card; hubs belong to zt-idea-grow"));
            }
            authored_topic_links(arguments, text("at"))
        }
        "create_literature" => authored_topic_links(arguments, ""),
        _ => None,
    }
}

fn authored_topic_links(arguments: &Value, at: &str) -> Option<Violation> {
    let text = |name: &str| arguments.get(name).and_then(Value::as_str).unwrap_or_default();
    let mut links = Vec::new();
    for name in ["title", "body", "text"] {
        links.extend(topic_links(text(name)));
    }
    if let Some(paragraph) = arguments["owned_paragraph"]["text"].as_str() {
        links.extend(topic_links(paragraph));
    }
    (!links.is_empty()).then(|| Violation {
        reason: "topic_link",
        at: at.to_string(),
        message: format!(
            "this connection writes only Literature trees: the text links Topic-tree Cards ({}); link Literature Cards only",
            links.join(", ")
        ),
    })
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
    use serde_json::json;

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

    #[test]
    fn authored_topic_links_are_found_outside_the_reverse_links() {
        let body = "依赖 [[Smith2024/1]]。对照 [[408/1]]。\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[411/1]] 索引";
        assert_eq!(topic_links(body), vec!["408/1".to_string()]);
        assert!(topic_links("only [[Smith2024/1|a]] here").is_empty());
    }

    /// R0272/S12-AC02 (S12, #154): a Term use of a Topic-tree Card is refused by the address after
    /// `@`; malformed Term-like markup is no Link and never one concatenated address.
    #[test]
    fn a_term_use_of_a_topic_card_is_read_by_its_target() {
        assert_eq!(topic_links("对照 [[均衡@408/1|a]] 与 [[Smith2024/1]]。"), vec!["408/1|a".to_string()]);
        let refusal = literature_violation("edit_card", &json!({ "at": "Smith2024/1|a", "body": "对照 [[均衡@408/1|a]]。" }))
            .expect("a Term use of a Topic Card is refused");
        assert_eq!(refusal.reason, "topic_link");
        assert!(refusal.message.contains("408/1|a"), "{refusal:?}");
        assert!(literature_violation("create_successor", &json!({ "at": "Smith2024/1", "title": "x", "body": "依赖 [[边际成本@Smith2024/2|a]]。" })).is_none());
        assert!(topic_links("[[a@b@408/1]]").is_empty());
        assert!(topic_links("[[@408/1]]").is_empty());
    }

    #[test]
    fn the_literature_scope_names_its_reason_and_lets_literature_work_pass() {
        let reason = |tool: &str, arguments: Value| literature_violation(tool, &arguments).map(|v| v.reason);
        assert_eq!(reason("create_topic", json!({ "title": "企业边界" })), Some("topic_tree"));
        assert_eq!(reason("move_card", json!({ "at": "Smith2024/1|a", "to": "12/1|b" })), Some("topic_tree"));
        assert_eq!(reason("delete_card", json!({ "at": "Smith2024/3", "into": "12/1" })), Some("topic_tree"));
        assert_eq!(reason("create_successor", json!({ "at": "Smith2024/1", "title": "x", "type": "hub" })), Some("hub"));
        assert_eq!(reason("edit_card", json!({ "at": "Smith2024/1|a", "text": "# t\n\n[[3/1]]" })), Some("topic_link"));
        assert_eq!(reason("edit_card", json!({ "at": "Smith2024/1|a", "owned_paragraph": { "text": "对照 [[3/1]]。" } })), Some("topic_link"));
        assert_eq!(reason("create_successor", json!({ "at": "Smith2024/1", "kind": "side", "title": "x", "body": "依赖 [[Smith2024/1]]，对照 [[Jones2020/2|a]]。" })), None);
        assert_eq!(reason("edit_card", json!({ "at": "Smith2024/1|a", "body": "x\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[408/1]] t" })), None);
        assert_eq!(reason("create_literature", json!({ "bibtex": "@book{Smith2024, title={T}}" })), None);
        assert_eq!(reason("get_card", json!({ "at": "408/1" })), None);
    }
}
