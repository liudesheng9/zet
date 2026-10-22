//! The process-wide write scope (docs/literature-only-digest-design.md). A digest builds the book's
//! Literature tree and nothing else, so a digest process sets the Literature scope once, before its
//! first archive call; from then on the client refuses, before the call reaches the archive, every
//! write that would create a Topic tree, change a Card of a Topic tree, make a Hub Card, or put a
//! Link to a Topic-tree Card into authored text. The idea-grow, hub and register tools keep the open
//! scope. A Topic tree is a tree whose id is a Topic number (all ASCII digits), so the check needs
//! no archive read.

use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

static LITERATURE_ONLY: AtomicBool = AtomicBool::new(false);

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\[\]]+)\]\]").unwrap());

/// The generated section a Card read ends with; its Links are the archive's, not authored.
const REVERSE_LINKS: &str = "<!-- zt:reverse-links -->";

/// Puts this process in the Literature scope (true) or back in the open scope (false).
pub fn set_literature_only(on: bool) {
    LITERATURE_ONLY.store(on, Ordering::SeqCst);
}

/// Whether this process is in the Literature scope.
pub fn literature_only() -> bool {
    LITERATURE_ONLY.load(Ordering::SeqCst)
}

/// Whether an address (a Location, a Citation key, or a Link target) is in a Topic tree.
pub fn is_topic_address(address: &str) -> bool {
    let tree = address.trim().split('/').next().unwrap_or_default();
    !tree.is_empty() && tree.bytes().all(|b| b.is_ascii_digit())
}

/// The Topic-tree Link targets of authored text, ignoring the generated Reverse links section.
pub fn topic_links(text: &str) -> Vec<String> {
    let authored = text.split(REVERSE_LINKS).next().unwrap_or_default();
    LINK.captures_iter(authored).map(|c| c[1].trim().to_string()).filter(|t| is_topic_address(t)).collect()
}

/// Why the Literature scope refuses this call, or None when it may go to the archive. Reads pass.
pub fn literature_refusal(tool: &str, arguments: &Value) -> Option<String> {
    let text = |name: &str| arguments.get(name).and_then(Value::as_str).unwrap_or_default();
    let topic_arg = |names: &[&str]| names.iter().map(|n| text(n)).find(|a| is_topic_address(a)).map(str::to_string);
    let authored = || {
        let mut found = Vec::new();
        for name in ["title", "body", "text"] {
            found.extend(topic_links(text(name)));
        }
        found
    };
    let hub = || text("type").eq_ignore_ascii_case("hub");
    let scope = "the digest writes only the book's Literature tree";
    match tool {
        "create_topic" => Some(format!("{scope}: it creates no Topic tree; Topic trees belong to zt-idea-grow")),
        "apply_batch" => {
            let items = arguments.get("items").or_else(|| arguments.get("preview_items")).and_then(Value::as_array);
            for (i, item) in items.into_iter().flatten().enumerate() {
                let tool = item.get("tool").and_then(Value::as_str).unwrap_or_default();
                let args = item.get("arguments").cloned().unwrap_or(Value::Null);
                if let Some(why) = literature_refusal(tool, &args) {
                    return Some(format!("batch item {i} ({tool}): {why}"));
                }
            }
            None
        }
        "create_successor" | "edit_card" | "insert_card" | "move_card" | "delete_card" | "edit_literature_metadata" => {
            if let Some(at) = topic_arg(&["at", "before", "to", "into"]) {
                return Some(format!("{scope}: `{at}` is a Topic-tree Card; Topic trees belong to zt-idea-grow"));
            }
            if hub() {
                return Some(format!("{scope}: it makes no Hub Card; hubs belong to zt-idea-grow"));
            }
            let links = authored();
            if !links.is_empty() {
                return Some(format!("{scope}: the text links Topic-tree Cards ({}); link Literature Cards only", links.join(", ")));
            }
            None
        }
        "create_literature" => {
            let links = authored();
            (!links.is_empty()).then(|| format!("{scope}: the text links Topic-tree Cards ({}); link Literature Cards only", links.join(", ")))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn topic_addresses_are_numbered_trees() {
        assert!(is_topic_address("408/1|a"));
        assert!(is_topic_address("12/0"));
        assert!(is_topic_address("7"));
        assert!(!is_topic_address("grossman2002integration/1|d"));
        assert!(!is_topic_address("Smith2024"));
        assert!(!is_topic_address("2002grossman/1"));
        assert!(!is_topic_address(""));
    }

    #[test]
    fn authored_topic_links_are_found_outside_the_reverse_links() {
        let body = "依赖 [[Smith2024/1]]。对照 [[408/1]]。\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[411/1]] 索引";
        assert_eq!(topic_links(body), vec!["408/1".to_string()]);
        assert!(topic_links("only [[Smith2024/1|a]] here").is_empty());
    }

    #[test]
    fn the_literature_scope_refuses_topic_and_hub_writes() {
        assert!(literature_refusal("create_topic", &json!({ "title": "企业边界" })).is_some());
        assert!(literature_refusal("create_successor", &json!({ "at": "408/1", "title": "x" })).is_some());
        assert!(literature_refusal("edit_card", &json!({ "at": "409/1|a", "body": "x" })).is_some());
        assert!(literature_refusal("move_card", &json!({ "at": "Smith2024/1|a", "to": "12/1|b" })).is_some());
        assert!(literature_refusal("delete_card", &json!({ "at": "Smith2024/3", "into": "12/1" })).is_some());
        assert!(literature_refusal("insert_card", &json!({ "before": "12/2", "title": "x" })).is_some());
        assert!(literature_refusal("create_successor", &json!({ "at": "Smith2024/1", "title": "x", "type": "hub" })).is_some());
        assert!(literature_refusal("edit_card", &json!({ "at": "Smith2024/1|a", "body": "对照 [[408/1]]。" })).is_some());
        assert!(literature_refusal("edit_card", &json!({ "at": "Smith2024/1|a", "text": "# t\n\n[[3/1]]" })).is_some());
        let batch = json!({ "items": [
            { "op_key": "r/u/1/s/a", "client_ref": "a", "tool": "edit_card", "arguments": { "at": "Smith2024/1", "body": "ok" } },
            { "op_key": "r/u/1/s/b", "client_ref": "b", "tool": "create_topic", "arguments": { "title": "x" } },
        ] });
        assert!(literature_refusal("apply_batch", &batch).unwrap().starts_with("batch item 1 (create_topic)"));
    }

    #[test]
    fn the_literature_scope_lets_literature_work_and_reads_pass() {
        assert!(literature_refusal("create_successor", &json!({ "at": "Smith2024/1", "kind": "side", "title": "x", "body": "依赖 [[Smith2024/1]]，对照 [[Jones2020/2|a]]。" })).is_none());
        assert!(literature_refusal("edit_card", &json!({ "at": "Smith2024/1|a", "body": "x\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[408/1]] t" })).is_none());
        assert!(literature_refusal("create_literature", &json!({ "bibtex": "@book{Smith2024, title={T}}" })).is_none());
        assert!(literature_refusal("get_card", &json!({ "at": "408/1" })).is_none());
        assert!(literature_refusal("get_tree", &json!({ "tree": "408" })).is_none());
        let batch = json!({ "items": [{ "op_key": "r/u/1/s/a", "client_ref": "a", "tool": "create_successor", "arguments": { "at": "Smith2024/1", "title": "x" } }] });
        assert!(literature_refusal("apply_batch", &batch).is_none());
    }
}
