//! The parts of `scripts/book-digest/book-of.ps1` the audit uses: the category of each Card and
//! the book a Card belongs to (inferred attribution). Comparisons are ordinal, as in the script.

use crate::ps::field;
use regex::Regex;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static ANCHOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+/\d+$").unwrap());

/// `Get-LinkBook`: the book a Link target is in (its tree), or None.
pub fn link_book<'a>(target: &'a str, books: &HashSet<String>) -> Option<&'a str> {
    let head = target.split('/').next().unwrap_or_default();
    books.contains(head).then_some(head)
}

/// `Test-SourceTarget`: a Link to a book's Literature Card or one of its anchors (a source line).
pub fn source_target(target: &str, books: &HashSet<String>) -> bool {
    link_book(target, books).is_some() && (!target.contains('/') || ANCHOR.is_match(target))
}

pub struct BookIndex {
    /// address -> category (ordinal keys).
    pub category: HashMap<String, String>,
    /// address -> book key, for the agent Cards of a book.
    pub book_of: HashMap<String, String>,
    /// The agent Literature trees, in ordinal order.
    #[allow(dead_code)]
    pub books: Vec<String>,
    pub book_set: HashSet<String>,
}

/// `Get-BookIndex`.
/// The index without a register read: no Card is a register part.
pub fn book_index(trees: &[Value], cards: &[Value]) -> BookIndex {
    book_index_with_parts(trees, cards, &HashSet::new())
}

/// The same index, with the addresses a register root declares under 词条分册 (§8.5, D188).
pub fn book_index_with_parts(trees: &[Value], cards: &[Value], register_parts: &HashSet<String>) -> BookIndex {
    let mut books = HashSet::new();
    let mut literature = HashSet::new();
    for tree in trees {
        if field(tree, "kind") == "literature" {
            literature.insert(field(tree, "tree"));
            if field(tree, "workspace") == "agent" {
                books.insert(field(tree, "tree"));
            }
        }
    }
    let mut anchors: HashMap<String, usize> = HashMap::new();
    for card in cards {
        if literature.contains(&field(card, "tree")) && ANCHOR.is_match(&field(card, "address")) {
            *anchors.entry(field(card, "tree")).or_default() += 1;
        }
    }
    let mut category_of = HashMap::new();
    let mut book_of = HashMap::new();
    for card in cards {
        let address = field(card, "address");
        category_of.insert(address.clone(), zt_check::graph::category_with_parts(card, &literature, &anchors, register_parts));
        if field(card, "workspace") != "agent" {
            continue;
        }
        // docs/literature-only-digest-design.md: a Card belongs to a book only when it is in that
        // book's Literature tree. A Topic-tree Card is idea-grow's and belongs to no book, however
        // many Links it has into one, so no digest step reviews, edits or bridges from it.
        let tree = field(card, "tree");
        if books.contains(&tree) {
            book_of.insert(address, tree);
        }
    }
    let mut ordered: Vec<String> = books.iter().cloned().collect();
    ordered.sort();
    BookIndex { category: category_of, book_of, books: ordered, book_set: books }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // The assertions of scripts/book-digest/tests/book-of.tests.ps1, on the Rust port.
    #[test]
    fn categories_and_attribution() {
        let mut trees = vec![json!({ "tree": "Alpha", "kind": "literature", "workspace": "agent" })];
        let mut cards = vec![json!({ "address": "Alpha/1", "tree": "Alpha", "kind": "regular", "type": "finding", "workspace": "agent", "links": [] })];
        let index = book_index(&trees, &cards);
        assert_eq!(index.category["Alpha/1"], "finding");
        trees.push(json!({ "tree": "Beta", "kind": "literature", "workspace": "agent" }));
        cards.extend([
            json!({ "address": "4/1", "tree": "4", "kind": "regular", "type": "idea", "workspace": "agent", "links": ["Beta", "Alpha/1|a", "Alpha/1|b"] }),
            json!({ "address": "4/2", "tree": "4", "kind": "regular", "type": "idea", "workspace": "agent", "links": ["Beta/1|a", "Alpha/1|a", "Beta/1|a"] }),
            json!({ "address": "4/3", "tree": "4", "kind": "regular", "type": "idea", "workspace": "agent", "links": [] }),
        ]);
        let index = book_index(&trees, &cards);
        assert_eq!(index.book_of["Alpha/1"], "Alpha");
        // A Topic-tree Card belongs to no book, whatever it links.
        assert!(!index.book_of.contains_key("4/1") && !index.book_of.contains_key("4/2"));
        assert!(!index.book_of.contains_key("4/3"));
        trees.push(json!({ "tree": "Human", "kind": "literature", "workspace": "human" }));
        cards.extend([
            json!({ "address": "Alpha", "tree": "Alpha", "kind": "literature", "type": "idea", "workspace": "agent", "links": [] }),
            json!({ "address": "Alpha/2", "tree": "Alpha", "kind": "regular", "type": "idea", "workspace": "agent", "links": [] }),
            json!({ "address": "Alpha/3", "tree": "Alpha", "kind": "regular", "type": "idea", "workspace": "agent", "links": [] }),
            json!({ "address": "Alpha/1|m", "tree": "Alpha", "kind": "regular", "type": "model", "facet": "设定", "workspace": "agent", "links": [] }),
            json!({ "address": "Alpha/1|t", "tree": "Alpha", "kind": "regular", "type": "idea", "title": "术语表", "workspace": "agent", "links": [] }),
            json!({ "address": "Alpha/1|s", "tree": "Alpha", "kind": "regular", "type": "idea", "parent": "Alpha/1", "edge": "side", "workspace": "agent", "links": ["Alpha/1", "Alpha/2", "Alpha/3"] }),
            json!({ "address": "4/4", "tree": "4", "kind": "regular", "type": "idea", "workspace": "human", "links": ["Alpha"] }),
            json!({ "address": "4/5", "tree": "4", "kind": "regular", "type": "idea", "workspace": "agent", "links": ["alpha"] }),
            json!({ "address": "Case/1", "tree": "4", "kind": "regular", "type": "model", "workspace": "agent", "links": ["Beta"] }),
            json!({ "address": "case/1", "tree": "4", "kind": "regular", "type": "hub", "workspace": "agent", "links": ["Alpha"] }),
        ]);
        let index = book_index(&trees, &cards);
        assert_eq!(index.category["Alpha"], "root");
        assert_eq!(index.category["Alpha/2"], "anchor");
        assert_eq!(index.category["Alpha/1|m"], "part");
        assert_eq!(index.category["Alpha/1|t"], "terms");
        assert_eq!(index.category["Alpha/1|s"], "structure");
        assert!(!index.book_of.contains_key("4/4") && !index.book_of.contains_key("4/5"));
        assert!(!index.books.contains(&"Human".to_string()));
        assert!(!index.book_of.contains_key("Case/1") && !index.book_of.contains_key("case/1"));
        let index = book_index(&trees, &cards);
        let mut rt = trees.clone();
        rt.reverse();
        let mut rc = cards.clone();
        rc.reverse();
        let reordered = book_index(&rt, &rc);
        assert_eq!(index.category, reordered.category);
        assert_eq!(index.book_of, reordered.book_of);
        // A Card the register root declares under 词条分册 is a navigation Card, not an Idea
        // (§8.5, D188): the audit reads that category from the root declaration.
        let declared = HashSet::from(["4/5".to_string()]);
        let with_part = book_index_with_parts(&trees, &cards, &declared);
        assert_eq!(with_part.category["4/5"], "register-part");
        assert!(!zt_check::graph::SUBSTANTIVE.contains(&"register-part"));
        assert!(zt_check::graph::EXCLUDED.contains(&"register-part"));
    }
}
