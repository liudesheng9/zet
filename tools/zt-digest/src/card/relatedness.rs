//! The port of `scripts/book-digest/relatedness.ps1`: a small TF-IDF over Chinese character
//! bigrams and Latin words, to rank the Cards most likely related to one Card.
//!
//! PowerShell enumerated hashtables in a per-process random order, so it broke ties (equal
//! weights at the `keep` cut, equal scores) at random. This port breaks them by first
//! appearance: terms in text order, documents in input order.

use crate::ps::{lower_invariant, ps_sort};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

static CLEAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[^$]*\$|\[\[[^\]]*\]\]|`[^`]*`").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]+").unwrap());
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-z][a-z\-]{3,}").unwrap());

/// `Get-Terms`: term -> count, in order of first appearance.
fn terms(text: &str) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    let mut add = |term: String| match at.get(&term) {
        Some(&i) => out[i].1 += 1,
        None => {
            at.insert(term.clone(), out.len());
            out.push((term, 1));
        }
    };
    let clean = CLEAN.replace_all(text, " ");
    for run in HAN.find_iter(&clean) {
        let chars: Vec<char> = run.as_str().chars().collect();
        for pair in chars.windows(2) {
            add(pair.iter().collect());
        }
    }
    for word in WORD.find_iter(&lower_invariant(&clean)) {
        add(word.as_str().to_string());
    }
    out
}

pub struct Relatedness {
    ids: Vec<String>,
    vectors: HashMap<String, Vec<(String, f64)>>,
    lookup: HashMap<String, HashMap<String, f64>>,
    postings: HashMap<String, Vec<String>>,
    /// The document frequency of every term of the corpus, and the corpus size: a query that is not
    /// one of the documents is weighted by the same rule, so the query never changes the weights.
    df: HashMap<String, usize>,
    count: f64,
    keep: usize,
    /// Every term of every document, weighting aside: the raw sets the overlap fallback needs when
    /// a shared term was universal and carries no weight.
    termsets: HashMap<String, Vec<String>>,
}

impl Relatedness {
    /// `New-Relatedness`: weighted vectors for documents (id, text), the `keep` heaviest terms each.
    pub fn new(documents: &[(String, String)], keep: usize) -> Self {
        let mut df: HashMap<String, usize> = HashMap::new();
        let mut raw: Vec<(String, Vec<(String, u32)>)> = Vec::new();
        for (id, text) in documents {
            let t = terms(text);
            for (term, _) in &t {
                *df.entry(term.clone()).or_default() += 1;
            }
            match raw.iter_mut().find(|(k, _)| k == id) {
                Some(entry) => entry.1 = t,
                None => raw.push((id.clone(), t)),
            }
        }
        let count = documents.len().max(1) as f64;
        let mut model = Self {
            ids: Vec::new(),
            vectors: HashMap::new(),
            lookup: HashMap::new(),
            postings: HashMap::new(),
            df: df.clone(),
            count,
            keep,
            termsets: HashMap::new(),
        };
        for (id, t) in raw {
            let mut weights: Vec<(String, f64)> = Vec::new();
            for (term, tf) in &t {
                let d = df[term] as f64;
                if d >= count {
                    continue;
                }
                weights.push((term.clone(), (1.0 + (*tf as f64).ln()) * (count / d).ln()));
            }
            ps_sort(&mut weights, |a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            weights.truncate(keep);
            let mut norm = weights.iter().map(|(_, w)| w * w).sum::<f64>().sqrt();
            if norm == 0.0 {
                norm = 1.0;
            }
            let vector: Vec<(String, f64)> = weights.into_iter().map(|(t, w)| (t, w / norm)).collect();
            for (term, _) in &vector {
                model.postings.entry(term.clone()).or_default().push(id.clone());
            }
            model.lookup.insert(id.clone(), vector.iter().cloned().collect());
            model.vectors.insert(id.clone(), vector);
            model.termsets.insert(id.clone(), t.into_iter().map(|(term, _)| term).collect());
            model.ids.push(id);
        }
        model
    }

    /// Every other id that shares a term with `id`, most similar first.
    fn rank(&self, id: &str) -> Vec<String> {
        let Some(vector) = self.vectors.get(id) else { return Vec::new() };
        self.score(vector, usize::MAX, |other| other == id)
    }

    /// One weighted vector scored against every other document, most similar first.
    fn score(&self, vector: &[(String, f64)], top: usize, exclude: impl Fn(&str) -> bool) -> Vec<String> {
        let mut scores: Vec<(String, f64)> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        for (term, weight) in vector {
            for other in self.postings.get(term).into_iter().flatten() {
                let add = weight * self.lookup.get(other).and_then(|lookup| lookup.get(term)).copied().unwrap_or(0.0);
                if add == 0.0 || exclude(other) {
                    continue;
                }
                match at.get(other) {
                    Some(&i) => scores[i].1 += add,
                    None => {
                        at.insert(other.clone(), scores.len());
                        scores.push((other.clone(), add));
                    }
                }
            }
        }
        // OrderByDescending: stable.
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scores.into_iter().take(top).map(|(k, _)| k).collect()
    }

    /// The documents most similar to one text that is not itself a document: the same weighted
    /// terms and cosine scoring, so a caller may rank a question, a section or a whole group
    /// against the corpus without adding the text to it (S05, #98).
    pub fn rank_text(&self, text: &str, top: usize, exclude: impl Fn(&str) -> bool) -> Vec<String> {
        let mut weights: Vec<(String, f64)> = Vec::new();
        for (term, tf) in terms(text) {
            // A term no document uses matches nothing, and one every document uses carries no
            // weight; neither enters the query vector, exactly as for a document.
            let Some(&d) = self.df.get(&term) else { continue };
            if (d as f64) >= self.count {
                continue;
            }
            weights.push((term, (1.0 + (tf as f64).ln()) * (self.count / d as f64).ln()));
        }
        ps_sort(&mut weights, |a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        weights.truncate(self.keep);
        let mut norm = weights.iter().map(|(_, w)| w * w).sum::<f64>().sqrt();
        if norm == 0.0 {
            norm = 1.0;
        }
        let vector: Vec<(String, f64)> = weights.into_iter().map(|(t, w)| (t, w / norm)).collect();
        self.score(&vector, top, exclude)
    }

    /// `Find-Related`: the `top` ids most similar to `id`, without those `exclude` refuses.
    pub fn find(&self, id: &str, top: usize, exclude: impl Fn(&str) -> bool) -> Vec<String> {
        let mut found = Vec::new();
        for other in self.rank(id) {
            if exclude(&other) {
                continue;
            }
            found.push(other);
            if found.len() >= top {
                break;
            }
        }
        found
    }

    /// The documents that share at least one term with `text`, most shared terms first. A corpus
    /// that is too uniform for the weighted ranking - a term every document uses carries no weight
    /// and is dropped - still answers by raw overlap, so a small or homogeneous archive offers its
    /// matching Cards instead of nothing (S05, #98).
    pub fn overlap(&self, text: &str, top: usize, exclude: impl Fn(&str) -> bool) -> Vec<String> {
        let wanted: HashMap<String, ()> = terms(text).into_iter().map(|(term, _)| (term, ())).collect();
        let mut scores: Vec<(String, usize)> = Vec::new();
        for id in &self.ids {
            if exclude(id) {
                continue;
            }
            let shared = self
                .termsets
                .get(id)
                .map(|terms| terms.iter().filter(|term| wanted.contains_key(*term)).count())
                .unwrap_or(0);
            if shared > 0 {
                scores.push((id.clone(), shared));
            }
        }
        // OrderByDescending: stable, so equal overlaps keep the corpus order.
        scores.sort_by(|a, b| b.1.cmp(&a.1));
        scores.into_iter().take(top).map(|(id, _)| id).collect()
    }

    #[allow(dead_code)]
    pub fn ids(&self) -> &[String] {
        &self.ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn related_cards_share_rare_terms() {
        let docs: Vec<(String, String)> = [
            ("a", "所有权决定剩余控制权 ownership"),
            ("b", "剩余控制权给所有者谈判地位 ownership"),
            ("c", "债务把控制权交给债权人"),
            ("d", "通用汽车收购费雪车身公司"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let model = Relatedness::new(&docs, 80);
        let found = model.find("a", 5, |_| false);
        assert_eq!(found.first().map(String::as_str), Some("b"));
        assert!(!found.contains(&"d".to_string()));
        assert_eq!(model.find("a", 5, |id| id == "b").first().map(String::as_str), Some("c"));
        assert_eq!(terms("[[3/1]] 控制权 $x$ Ownership"), vec![("控制".into(), 1), ("制权".into(), 1), ("ownership".into(), 1)]);
    }

    #[test]
    fn a_query_outside_the_corpus_ranks_without_changing_the_weights() {
        let docs: Vec<(String, String)> = [
            ("a", "所有权决定剩余控制权"),
            ("b", "剩余控制权带来谈判地位"),
            ("c", "债务与债权人"),
            ("d", "通用汽车收购费雪车身公司"),
        ]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
        let model = Relatedness::new(&docs, 80);
        let ranked = model.rank_text("剩余控制权的谈判地位", 5, |_| false);
        assert_eq!(ranked, vec!["b".to_string(), "a".to_string()]);
        // A term every document shares cannot separate them, and an unused term matches nothing.
        let same: Vec<(String, String)> = (0..3).map(|i| (i.to_string(), "同一个词".to_string())).collect();
        let flat = Relatedness::new(&same, 80);
        assert!(flat.rank_text("同一个词", 5, |_| false).is_empty());
        assert!(model.rank_text("无关的词", 5, |_| false).is_empty());
        // The exclude rule is the caller's: an address already offered is never repeated.
        assert_eq!(model.rank_text("剩余控制权", 5, |id| id == "a"), vec!["b".to_string()]);
    }
}
