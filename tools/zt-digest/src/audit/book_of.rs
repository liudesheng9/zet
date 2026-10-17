//! The parts of `scripts/book-digest/book-of.ps1` the audit uses: the category of each Card and
//! the book a Card belongs to (inferred attribution). Comparisons are ordinal, as in the script.

use super::ps::{culture_cmp, ps_sort, s};
use regex::Regex;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static ANCHOR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+/\d+$").unwrap());
static TERMS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
static STRUCTURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^本卡是《.*》的结构卡").unwrap());

/// `Get-LinkBook`: the book a Link target is in (its tree), or None.
pub fn link_book<'a>(target: &'a str, books: &HashSet<String>) -> Option<&'a str> {
    let head = target.split('/').next().unwrap_or_default();
    books.contains(head).then_some(head)
}

/// `Test-SourceTarget`: a Link to a book's Literature Card or one of its anchors (a source line).
pub fn source_target(target: &str, books: &HashSet<String>) -> bool {
    link_book(target, books).is_some() && (!target.contains('/') || ANCHOR.is_match(target))
}

fn links(card: &Value) -> Vec<String> {
    super::ps::ps_items(&card["links"]).iter().map(super::ps::ps_str).collect()
}

/// `Get-Category`.
fn category(card: &Value, literature: &HashSet<String>, anchor_count: &HashMap<String, usize>) -> String {
    let kind = s(card, "kind");
    if kind == "topic" || kind == "literature" {
        return "root".into();
    }
    match s(card, "type").as_str() {
        "finding" => return "finding".into(),
        "hub" => return "hub".into(),
        "event" => return "event".into(),
        "model" => return if super::ps::truthy(&card["facet"]) { "part" } else { "model" }.into(),
        _ => {}
    }
    let tree = s(card, "tree");
    let in_book = literature.contains(&tree);
    if in_book && ANCHOR.is_match(&s(card, "address")) {
        return "anchor".into();
    }
    if in_book && TERMS.is_match(&s(card, "title")) {
        return "terms".into();
    }
    if in_book && STRUCTURE.is_match(&s(card, "excerpt")) {
        return "structure".into();
    }
    if in_book && s(card, "parent") == format!("{tree}/1") && s(card, "edge") == "side" && s(card, "type") == "idea" {
        let own = Regex::new(&format!("^{}/\\d+$", regex::escape(&tree))).unwrap();
        let anchors: HashSet<String> = links(card).into_iter().filter(|t| own.is_match(t)).collect();
        // A missing count reads as $null, and any count is -ge $null.
        let enough = anchor_count.get(&tree).is_none_or(|&n| anchors.len() * 2 >= n);
        if anchors.len() >= 3 && enough {
            return "structure".into();
        }
    }
    "idea".into()
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
pub fn book_index(trees: &[Value], cards: &[Value]) -> BookIndex {
    let mut books = HashSet::new();
    let mut literature = HashSet::new();
    for tree in trees {
        if s(tree, "kind") == "literature" {
            literature.insert(s(tree, "tree"));
            if s(tree, "workspace") == "agent" {
                books.insert(s(tree, "tree"));
            }
        }
    }
    let mut anchors: HashMap<String, usize> = HashMap::new();
    for card in cards {
        if literature.contains(&s(card, "tree")) && ANCHOR.is_match(&s(card, "address")) {
            *anchors.entry(s(card, "tree")).or_default() += 1;
        }
    }
    let mut category_of = HashMap::new();
    let mut book_of = HashMap::new();
    for card in cards {
        let address = s(card, "address");
        category_of.insert(address.clone(), category(card, &literature, &anchors));
        if s(card, "workspace") != "agent" {
            continue;
        }
        let tree = s(card, "tree");
        if books.contains(&tree) {
            book_of.insert(address, tree);
            continue;
        }
        let mut votes: Vec<(String, i64)> = Vec::new();
        let mut seen = HashSet::new();
        for target in links(card) {
            if !seen.insert(target.clone()) {
                continue;
            }
            if let Some(book) = link_book(&target, &books) {
                let add = if source_target(&target, &books) { 10 } else { 1 };
                match votes.iter_mut().find(|(k, _)| k == book) {
                    Some((_, n)) => *n += add,
                    None => votes.push((book.to_string(), add)),
                }
            }
        }
        // The highest vote wins; a tie goes to the first key in Sort-Object order.
        if !votes.is_empty() {
            ps_sort(&mut votes, |a, b| match b.1.cmp(&a.1) {
                Ordering::Equal => culture_cmp(&a.0, &b.0),
                other => other,
            });
            book_of.insert(address, votes[0].0.clone());
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
        assert_eq!(index.book_of["4/1"], "Beta");
        assert_eq!(index.book_of["4/2"], "Alpha");
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
        assert_eq!(index.book_of["Case/1"], "Beta");
        assert_eq!(index.book_of["case/1"], "Alpha");
        let mut rt = trees.clone();
        rt.reverse();
        let mut rc = cards.clone();
        rc.reverse();
        let reordered = book_index(&rt, &rc);
        assert_eq!(index.category, reordered.category);
        assert_eq!(index.book_of, reordered.book_of);
    }
}
