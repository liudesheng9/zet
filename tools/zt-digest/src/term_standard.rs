//! The standard terminology list of one Literature type (docs/term-align-design.md): the shelf's index
//! over its books' Term lists. A new book is matched against it, so a job holds the new book's rows and a
//! few candidates each, and the list can grow without entering a job.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

/// The index weights, as measured on the three history books: all 42 known non-exact pairs at 5 candidates.
/// A candidate below this share of the best score is left out: 42 of 42 stay, at 4.7 candidates a row and not 5.9.
const KEEP_SHARE: f64 = 0.3;
const TERM_WEIGHT: f64 = 3.0;
const AVOID_WEIGHT: f64 = 2.0;
const SINGLE_WEIGHT: f64 = 0.3;

/// One standard term.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Entry {
    pub id: String,
    pub term: String,
    pub variants: Vec<String>,
    pub meaning: String,
    pub example: String,
    /// What each book holds. A spelling that differs from `term` means the book must follow a respell.
    pub books: BTreeMap<String, String>,
}

/// A Term row as the index reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Probe {
    pub term: String,
    pub avoid: Vec<String>,
    pub meaning: String,
}

/// One Term row of a book.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Row {
    pub id: String,
    pub term: String,
    pub avoid: Vec<String>,
    pub meaning: String,
    pub head: String,
    pub uses: i64,
}

/// What a book's rows need. Only `open` rows reach an agent.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Triage {
    /// A row whose term is already a standard term: (row, entry).
    pub same: Vec<(String, String)>,
    /// A row with candidates: (row, candidate entries).
    pub open: Vec<(String, Vec<String>)>,
    /// A row with no candidate: it joins the list by script.
    pub new: Vec<String>,
    /// A row whose entry was respelled since this book joined: (row, entry). It follows by script.
    pub known: Vec<(String, String)>,
}

/// The standard list.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Standard {
    pub kind: String,
    pub entries: Vec<Entry>,
    /// Each aligned book's rows hash.
    pub aligned: BTreeMap<String, String>,
}

/// The candidate entries of a row, best first, at most `limit`. An entry shares at least one character
/// pair with the row; a row that shares none has no candidate.
#[cfg(test)]
pub(crate) fn candidates(standard: &Standard, row: &Probe, limit: usize) -> Vec<String> {
    Index::new(standard).candidates(row, limit)
}

/// The candidate index over a standard list. Built once a run, so a book's rows do not rebuild it.
struct Index<'a> {
    entries: Vec<(&'a Entry, Grams)>,
    seen: HashMap<String, usize>,
}

impl<'a> Index<'a> {
    fn new(standard: &'a Standard) -> Self {
        let entries: Vec<(&Entry, Grams)> = standard.entries.iter().map(|entry| (entry, grams_of(&entry.term, &entry.variants, &entry.meaning))).collect();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for gram in entries.iter().flat_map(|(_, grams)| grams.keys()) {
            *seen.entry(gram.clone()).or_default() += 1;
        }
        Self { entries, seen }
    }

    fn candidates(&self, row: &Probe, limit: usize) -> Vec<String> {
        let total = self.entries.len() as f64;
        let rarity = |gram: &str| (1.0 + total / (self.seen.get(gram).copied().unwrap_or(0) as f64 + 1.0)).ln();
        let wanted = grams_of(&row.term, &row.avoid, &row.meaning);
        let mut ranked: Vec<(f64, &str)> = Vec::new();
        for (entry, grams) in &self.entries {
            let mut pair = false;
            let mut score = 0.0;
            for (gram, weight) in &wanted {
                if let Some(other) = grams.get(gram) {
                    pair |= gram.chars().count() > 1;
                    score += weight * other * rarity(gram).powi(2);
                }
            }
            if pair {
                ranked.push((score, &entry.id));
            }
        }
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(b.1)));
        let floor = ranked.first().map_or(0.0, |(best, _)| best * KEEP_SHARE);
        ranked.into_iter().take_while(|(score, _)| *score >= floor).take(limit).map(|(_, id)| id.to_string()).collect()
    }
}

/// Sort a book's rows by what they need.
pub(crate) fn triage(standard: &Standard, key: &str, rows: &[Row], limit: usize) -> Triage {
    let index = Index::new(standard);
    let by_term: HashMap<&str, &str> = standard.entries.iter().map(|entry| (entry.term.as_str(), entry.id.as_str())).collect();
    let mut sorted = Triage::default();
    for row in rows {
        if let Some(id) = by_term.get(row.term.as_str()) {
            sorted.same.push((row.id.clone(), (*id).to_string()));
        }
    }
    let same: HashSet<&str> = sorted.same.iter().map(|(row, _)| row.as_str()).collect();
    for row in rows.iter().filter(|row| !same.contains(row.id.as_str())) {
        if let Some(entry) = standard.entries.iter().find(|entry| entry.books.get(key) == Some(&row.term)) {
            sorted.known.push((row.id.clone(), entry.id.clone()));
        }
    }
    let held: HashSet<&str> = sorted.same.iter().chain(&sorted.known).map(|(_, id)| id.as_str()).collect();
    let settled: HashSet<&str> = sorted.same.iter().chain(&sorted.known).map(|(row, _)| row.as_str()).collect();
    for row in rows.iter().filter(|row| !settled.contains(row.id.as_str())) {
        let probe = Probe { term: row.term.clone(), avoid: row.avoid.clone(), meaning: row.meaning.clone() };
        let offered: Vec<String> = index.candidates(&probe, limit + held.len()).into_iter().filter(|id| !held.contains(id.as_str())).take(limit).collect();
        if offered.is_empty() {
            sorted.new.push(row.id.clone());
        } else {
            sorted.open.push((row.id.clone(), offered));
        }
    }
    sorted
}

/// What is wrong with an align answer: (row, entry) pairs. An empty list is an answer `apply` takes.
pub(crate) fn answer_faults(triage: &Triage, answer: &[(String, String)]) -> Vec<String> {
    let offered: HashMap<&str, &Vec<String>> = triage.open.iter().map(|(row, ids)| (row.as_str(), ids)).collect();
    let mut faults = Vec::new();
    let (mut rows, mut entries) = (HashSet::new(), HashSet::new());
    for (row, to) in answer {
        match offered.get(row.as_str()) {
            None => faults.push(format!("{row} is no row of this job")),
            Some(ids) if !ids.contains(to) => faults.push(format!("{to} is no candidate of {row}")),
            Some(_) => {}
        }
        if !rows.insert(row) {
            faults.push(format!("{row} stands twice"));
        }
        if !entries.insert(to) {
            faults.push(format!("{to} takes two rows of one book"));
        }
    }
    faults
}

/// One Term of a book that takes the standard name.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Rename {
    pub old: String,
    pub canon: String,
    pub head: String,
}

/// Enter a book into the list: its aligned rows join their entries, and every other row becomes a new
/// entry. Answers the renames this book needs. An earlier book's rows are never touched.
pub(crate) fn merge(standard: &mut Standard, key: &str, rows: &[Row], triage: &Triage, answer: &[(String, String)], examples: &HashMap<String, String>) -> Vec<Rename> {
    let terms: HashSet<String> = standard.entries.iter().map(|entry| entry.term.clone()).collect();
    let by_row: HashMap<&str, &Row> = rows.iter().map(|row| (row.id.as_str(), row)).collect();
    let mut join = |standard: &mut Standard, row: &Row, id: &str| -> Option<String> {
        let entry = standard.entries.iter_mut().find(|entry| entry.id == id)?;
        entry.books.insert(key.to_string(), entry.term.clone());
        for word in row.avoid.iter().chain(std::iter::once(&row.term)) {
            if !terms.contains(word) && *word != entry.term && !entry.variants.contains(word) {
                entry.variants.push(word.clone());
            }
        }
        Some(entry.term.clone())
    };
    let mut renames = Vec::new();
    for (row, id) in &triage.same {
        if let Some(row) = by_row.get(row.as_str()) {
            join(standard, row, id);
        }
    }
    let mut joined: HashSet<&str> = HashSet::new();
    let pairs = answer.iter().chain(&triage.known);
    for (row_id, id) in pairs {
        if let Some(row) = by_row.get(row_id.as_str())
            && let Some(canon) = join(standard, row, id)
        {
            joined.insert(row_id);
            renames.push(Rename { old: row.term.clone(), canon, head: row.head.clone() });
        }
    }
    // Every other row is a term the list does not know yet.
    let settled: HashSet<&str> = triage.same.iter().map(|(row, _)| row.as_str()).collect();
    for row in rows.iter().filter(|row| !settled.contains(row.id.as_str()) && !joined.contains(row.id.as_str())) {
        let number = standard.entries.iter().filter_map(|entry| entry.id.trim_start_matches('s').parse::<u64>().ok()).max().unwrap_or(0) + 1;
        standard.entries.push(Entry {
            id: format!("s{number}"),
            term: row.term.clone(),
            variants: row.avoid.clone(),
            meaning: row.meaning.clone(),
            example: examples.get(&row.id).cloned().unwrap_or_default(),
            books: BTreeMap::from([(key.to_string(), row.term.clone())]),
        });
    }
    let after: Vec<Row> = rows
        .iter()
        .map(|row| {
            let term = renames.iter().find(|rename| rename.old == row.term).map_or_else(|| row.term.clone(), |rename| rename.canon.clone());
            Row { term, ..row.clone() }
        })
        .collect();
    standard.aligned.insert(key.to_string(), rows_hash(&after));
    renames
}

/// Give a standard entry a better spelling. The old one becomes a variant.
pub(crate) fn respell(standard: &mut Standard, id: &str, term: &str) -> Result<()> {
    if standard.entries.iter().any(|entry| entry.term == term && entry.id != id) {
        bail!("{term} is already another standard term");
    }
    let entry = standard.entries.iter_mut().find(|entry| entry.id == id).with_context(|| format!("{id} is no entry of the list"))?;
    let old = std::mem::replace(&mut entry.term, term.to_string());
    entry.variants.retain(|word| word != term);
    if old != term && !entry.variants.contains(&old) {
        entry.variants.push(old);
    }
    Ok(())
}

/// Whether a book holds a spelling the list has since changed.
pub(crate) fn must_follow(standard: &Standard, key: &str) -> bool {
    standard.entries.iter().any(|entry| entry.books.get(key).is_some_and(|spelling| *spelling != entry.term))
}

/// The hash that says a book's rows are the ones the list knows.
pub(crate) fn rows_hash(rows: &[Row]) -> String {
    let mut terms: Vec<&str> = rows.iter().map(|row| row.term.as_str()).collect();
    terms.sort_unstable();
    crate::canon::sha256_hex(terms.join("\n").as_bytes())
}

/// The list's file under an archive root.
pub(crate) fn store_path(root: &Path, kind: &str) -> PathBuf {
    root.join("terms").join(format!("{kind}.json"))
}

/// Read a list; a shelf with no file has an empty list.
pub(crate) fn load(path: &Path, kind: &str) -> Result<Standard> {
    if !path.exists() {
        return Ok(Standard { kind: kind.to_string(), ..Standard::default() });
    }
    let value = crate::util::read_json(path)?;
    let text = |item: &Value, name: &str| item[name].as_str().unwrap_or_default().to_string();
    let entries = value["entries"]
        .as_array()
        .context("the standard list has no entries")?
        .iter()
        .map(|item| Entry {
            id: text(item, "id"),
            term: text(item, "term"),
            variants: item["variants"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect(),
            meaning: text(item, "meaning"),
            example: text(item, "example"),
            books: item["books"].as_object().into_iter().flatten().map(|(key, spelling)| (key.clone(), spelling.as_str().unwrap_or_default().to_string())).collect(),
        })
        .collect();
    let aligned = value["aligned"].as_object().into_iter().flatten().map(|(key, hash)| (key.clone(), hash.as_str().unwrap_or_default().to_string())).collect();
    Ok(Standard { kind: text(&value, "type"), entries, aligned })
}

/// Write a list, keeping the previous version beside it.
pub(crate) fn save(path: &Path, standard: &Standard) -> Result<()> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    if path.exists() {
        std::fs::copy(path, path.with_extension("json.bak"))?;
    }
    let entries: Vec<Value> = standard
        .entries
        .iter()
        .map(|entry| json!({"id": entry.id, "term": entry.term, "variants": entry.variants, "meaning": entry.meaning, "example": entry.example, "books": entry.books}))
        .collect();
    let value = json!({"schema_version": 1, "type": standard.kind, "entries": entries, "aligned": standard.aligned});
    crate::util::write_atomic(path, &crate::ps::json_pretty(&value))
}

/// A row's weighted Han characters and character pairs. The term counts most, a 不用 word next, and the
/// meaning least: it is what finds a pair that shares no character (改宗/皈依).
type Grams = HashMap<String, f64>;

fn add_grams(text: &str, weight: f64, grams: &mut Grams) {
    let chars: Vec<char> = text.chars().filter(|c| ('\u{3400}'..='\u{9FFF}').contains(c)).collect();
    for (index, ch) in chars.iter().enumerate() {
        *grams.entry(ch.to_string()).or_default() += weight * SINGLE_WEIGHT;
        if let Some(next) = chars.get(index + 1) {
            *grams.entry(format!("{ch}{next}")).or_default() += weight;
        }
    }
}

fn grams_of(term: &str, avoid: &[String], meaning: &str) -> Grams {
    let mut grams = Grams::new();
    add_grams(term, TERM_WEIGHT, &mut grams);
    for word in avoid {
        // Several 不用 words often repeat one character pair; it still counts once.
        let mut own = Grams::new();
        add_grams(word, AVOID_WEIGHT, &mut own);
        for (gram, weight) in own {
            let held = grams.entry(gram).or_default();
            *held = held.max(weight);
        }
    }
    add_grams(meaning, 1.0, &mut grams);
    grams
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, term: &str, variants: &[&str], meaning: &str) -> Entry {
        Entry { id: id.into(), term: term.into(), variants: variants.iter().map(ToString::to_string).collect(), meaning: meaning.into(), ..Entry::default() }
    }

    fn probe(term: &str, avoid: &[&str], meaning: &str) -> Probe {
        Probe { term: term.into(), avoid: avoid.iter().map(ToString::to_string).collect(), meaning: meaning.into() }
    }

    fn shelf() -> Standard {
        Standard {
            kind: "history".into(),
            entries: vec![
                entry("s1", "撒拉森人", &["撒拉逊人"], "拜占庭史料对穆斯林的通称"),
                entry("s2", "改宗", &["改信"], "放弃原有信仰转信伊斯兰教"),
                entry("s3", "哈拉吉", &[], "对伊斯兰土地征收的土地税"),
                entry("s4", "清真寺", &[], "穆斯林礼拜的场所"),
                entry("s5", "哈里发", &[], "穆罕默德的继任者"),
            ],
            aligned: BTreeMap::new(),
        }
    }

    fn row(id: &str, term: &str, meaning: &str) -> Row {
        Row { id: id.into(), term: term.into(), meaning: meaning.into(), head: format!("k/{id}"), ..Row::default() }
    }

    #[test]
    fn an_exact_term_settles_without_a_job() {
        let sorted = triage(&shelf(), "k", &[row("D1", "哈里发", "继任者")], 6);
        assert_eq!(sorted.same, vec![("D1".to_string(), "s5".to_string())]);
        assert!(sorted.open.is_empty() && sorted.new.is_empty(), "{sorted:?}");
    }

    #[test]
    fn a_row_with_no_candidate_joins_by_script_and_the_rest_reach_a_job() {
        let sorted = triage(&shelf(), "k", &[row("D1", "琐罗亚斯德", "古代波斯宗教"), row("D2", "萨拉森人", "罗马人的称呼")], 6);
        assert_eq!(sorted.new, vec!["D1".to_string()]);
        assert_eq!(sorted.open.len(), 1, "{sorted:?}");
        assert_eq!(sorted.open[0].0, "D2");
        assert!(sorted.open[0].1.contains(&"s1".to_string()));
    }

    #[test]
    fn an_entry_this_book_already_holds_is_no_candidate() {
        // 哈里发 settled on s5, so 哈里发国 must not be offered s5: a book gives one row to an entry.
        let sorted = triage(&shelf(), "k", &[row("D1", "哈里发", "继任者"), row("D2", "哈里发国", "哈里发统治的国家")], 6);
        assert!(sorted.open.iter().all(|(_, offered)| !offered.contains(&"s5".to_string())), "{sorted:?}");
    }

    #[test]
    fn an_answer_stays_inside_its_rows_and_their_candidates() {
        let sorted = Triage { open: vec![("D1".into(), vec!["s1".into()]), ("D2".into(), vec!["s1".into(), "s2".into()])], ..Triage::default() };
        let pair = |row: &str, to: &str| (row.to_string(), to.to_string());
        assert!(answer_faults(&sorted, &[pair("D1", "s1"), pair("D2", "s2")]).is_empty());
        assert!(answer_faults(&sorted, &[pair("D9", "s1")])[0].contains("D9"));
        assert!(answer_faults(&sorted, &[pair("D1", "s2")])[0].contains("s2"));
        assert!(answer_faults(&sorted, &[pair("D1", "s1"), pair("D2", "s1")])[0].contains("two rows"));
        assert!(answer_faults(&sorted, &[pair("D1", "s1"), pair("D1", "s1")])[0].contains("twice"));
    }

    fn seeded() -> Standard {
        let mut list = Standard { kind: "history".into(), ..Standard::default() };
        let rows = [row("C1", "撒拉森人", "拜占庭史料对穆斯林的通称"), row("C2", "哈里发", "穆罕默德的继任者")];
        let sorted = triage(&list, "k", &rows, 6);
        merge(&mut list, "first", &rows, &sorted, &[], &HashMap::new());
        list
    }

    #[test]
    fn the_first_book_seeds_the_list_and_renames_nothing() {
        let mut list = Standard { kind: "history".into(), ..Standard::default() };
        let rows = [row("C1", "撒拉森人", "通称")];
        let sorted = triage(&list, "k", &rows, 6);
        let examples = HashMap::from([("C1".to_string(), "一句原句。".to_string())]);
        assert!(merge(&mut list, "first", &rows, &sorted, &[], &examples).is_empty());
        assert_eq!(list.entries[0].id, "s1");
        assert_eq!(list.entries[0].example, "一句原句。");
        assert_eq!(list.entries[0].books["first"], "撒拉森人");
        assert!(list.aligned.contains_key("first"));
    }

    #[test]
    fn an_aligned_row_renames_this_book_and_leaves_the_earlier_book_alone() {
        let mut list = seeded();
        let mut rows = vec![row("D1", "萨拉森人", "罗马人的称呼")];
        rows[0].avoid = vec!["撒拉逊人".into()];
        let sorted = triage(&list, "k", &rows, 6);
        let renames = merge(&mut list, "second", &rows, &sorted, &[("D1".to_string(), "s1".to_string())], &HashMap::new());
        assert_eq!(renames, vec![Rename { old: "萨拉森人".into(), canon: "撒拉森人".into(), head: "k/D1".into() }]);
        let entry = &list.entries[0];
        assert_eq!(entry.term, "撒拉森人", "a standard term does not change");
        assert_eq!(entry.books["first"], "撒拉森人", "the earlier book stays as it was");
        assert_eq!(entry.books["second"], "撒拉森人", "the list records what the book holds after the rename");
        assert!(entry.variants.contains(&"萨拉森人".to_string()) && entry.variants.contains(&"撒拉逊人".to_string()), "{entry:?}");
    }

    #[test]
    fn a_row_the_answer_leaves_out_becomes_a_new_entry() {
        let mut list = seeded();
        let rows = [row("D1", "哈里发国", "哈里发统治的国家")];
        let sorted = triage(&list, "k", &rows, 6);
        assert!(merge(&mut list, "second", &rows, &sorted, &[], &HashMap::new()).is_empty());
        let added = list.entries.last().unwrap();
        assert_eq!((added.id.as_str(), added.term.as_str()), ("s3", "哈里发国"));
    }

    #[test]
    fn a_book_merged_again_after_its_renames_changes_nothing() {
        let mut list = seeded();
        let rows = [row("D1", "萨拉森人", "罗马人的称呼")];
        let sorted = triage(&list, "k", &rows, 6);
        merge(&mut list, "second", &rows, &sorted, &[("D1".to_string(), "s1".to_string())], &HashMap::new());
        let before = list.entries.clone();
        let renamed = [row("D1", "撒拉森人", "罗马人的称呼")];
        let again = triage(&list, "second", &renamed, 6);
        assert!(merge(&mut list, "second", &renamed, &again, &[], &HashMap::new()).is_empty());
        assert_eq!(list.entries[0].books["second"], "撒拉森人");
        assert_eq!(list.entries.len(), before.len(), "no entry is added");
        assert_eq!(list.entries[0].variants, before[0].variants);
    }

    #[test]
    fn a_rejected_word_that_is_a_standard_term_stays_out_of_the_variants() {
        let mut list = seeded();
        let mut rows = vec![row("D1", "萨拉森人", "称呼")];
        rows[0].avoid = vec!["哈里发".into()];
        let sorted = triage(&list, "k", &rows, 6);
        merge(&mut list, "second", &rows, &sorted, &[("D1".to_string(), "s1".to_string())], &HashMap::new());
        assert!(!list.entries[0].variants.contains(&"哈里发".to_string()), "{:?}", list.entries[0]);
    }

    #[test]
    fn a_respelled_entry_keeps_the_old_spelling_as_a_variant() {
        let mut list = seeded();
        respell(&mut list, "s1", "萨拉森人").unwrap();
        assert_eq!(list.entries[0].term, "萨拉森人");
        assert!(list.entries[0].variants.contains(&"撒拉森人".to_string()));
        assert!(respell(&mut list, "s1", "哈里发").is_err(), "the spelling of another entry is refused");
        assert!(respell(&mut list, "s9", "甲").is_err(), "an unknown entry is refused");
    }

    #[test]
    fn a_book_follows_a_respelled_entry_with_no_agent() {
        let mut list = seeded();
        respell(&mut list, "s1", "萨拉森人").unwrap();
        let rows = [row("C1", "撒拉森人", "拜占庭史料对穆斯林的通称")];
        let sorted = triage(&list, "first", &rows, 6);
        assert_eq!(sorted.known, vec![("C1".to_string(), "s1".to_string())]);
        assert!(sorted.open.is_empty(), "no agent reads this row: {sorted:?}");
        let renames = merge(&mut list, "first", &rows, &sorted, &[], &HashMap::new());
        assert_eq!(renames, vec![Rename { old: "撒拉森人".into(), canon: "萨拉森人".into(), head: "k/C1".into() }]);
        assert_eq!(list.entries.len(), 2, "no entry is added");
    }

    #[test]
    fn a_book_that_was_renamed_follows_a_later_respell() {
        let mut list = seeded();
        let rows = [row("D1", "萨拉森人", "罗马人的称呼")];
        let sorted = triage(&list, "second", &rows, 6);
        merge(&mut list, "second", &rows, &sorted, &[("D1".to_string(), "s1".to_string())], &HashMap::new());
        assert!(!must_follow(&list, "second"));
        respell(&mut list, "s1", "撒拉逊人").unwrap();
        assert!(must_follow(&list, "second") && must_follow(&list, "first"));
        let renamed = [row("D1", "撒拉森人", "罗马人的称呼")];
        let again = triage(&list, "second", &renamed, 6);
        assert_eq!(again.known, vec![("D1".to_string(), "s1".to_string())], "{again:?}");
        let renames = merge(&mut list, "second", &renamed, &again, &[], &HashMap::new());
        assert_eq!(renames[0].canon, "撒拉逊人");
        assert!(!must_follow(&list, "second"));
    }

    #[test]
    fn a_new_book_with_the_old_spelling_still_goes_to_a_job() {
        // The automatic path follows a book's own record, never a word that merely matches.
        let mut list = seeded();
        respell(&mut list, "s1", "萨拉森人").unwrap();
        let sorted = triage(&list, "another", &[row("D1", "撒拉森人", "通称")], 6);
        assert!(sorted.known.is_empty() && sorted.open.len() == 1, "{sorted:?}");
    }

    #[test]
    fn the_rows_hash_follows_the_terms_and_not_the_order_of_the_table() {
        let (a, b) = (row("D1", "甲", "一"), row("D2", "乙", "二"));
        assert_eq!(rows_hash(&[a.clone(), b.clone()]), rows_hash(&[b.clone(), a.clone()]));
        assert_ne!(rows_hash(&[a.clone(), b]), rows_hash(&[a, row("D2", "丙", "二")]));
    }

    #[test]
    fn a_saved_list_reads_back_and_keeps_the_previous_version() {
        let folder = tempfile::tempdir().unwrap();
        let path = store_path(folder.path(), "history");
        assert!(path.ends_with("terms/history.json"), "{path:?}");
        assert_eq!(load(&path, "history").unwrap().kind, "history", "no file is an empty list");
        let mut list = seeded();
        save(&path, &list).unwrap();
        assert_eq!(load(&path, "history").unwrap(), list);
        list.entries.pop();
        save(&path, &list).unwrap();
        let kept = path.with_extension("json.bak");
        assert_eq!(load(&kept, "history").unwrap().entries.len(), 2);
        assert_eq!(load(&path, "history").unwrap().entries.len(), 1);
    }

    #[test]
    fn a_spelling_variant_finds_its_standard_term() {
        let found = candidates(&shelf(), &probe("萨拉森人", &[], "罗马人对阿拉伯人的称呼"), 6);
        assert_eq!(found.first().map(String::as_str), Some("s1"), "{found:?}");
    }

    #[test]
    fn a_term_that_shares_no_character_is_found_through_its_meaning() {
        let found = candidates(&shelf(), &probe("皈依", &[], "放弃原有信仰转信伊斯兰教的过程"), 6);
        assert!(found.contains(&"s2".to_string()), "{found:?}");
        let tax = candidates(&shelf(), &probe("土地税", &[], "向耕地持有者征收的税"), 6);
        assert!(tax.contains(&"s3".to_string()), "{tax:?}");
    }

    #[test]
    fn many_rejected_words_sharing_a_pair_count_once() {
        // 言行 weighs 3 on each side, so 9 once; three 不用 words added up would make it 27. The true entry
        // shares 15 pairs of its meaning, so it leads only when the repeated pair counts once.
        let meaning = "归于先知的话语与默认的传述材料汇集";
        let standard = Standard {
            entries: vec![entry("s1", "圣训", &[], meaning), entry("s2", "言行录", &[], "别的文集")],
            ..Standard::default()
        };
        let found = candidates(&standard, &probe("穆罕默德言行", &["言行甲", "言行乙", "言行丙"], meaning), 1);
        assert_eq!(found, vec!["s1".to_string()]);
    }

    #[test]
    fn a_candidate_far_below_the_best_is_not_shown() {
        let standard = Standard {
            entries: vec![entry("s1", "撒拉森人", &[], "拜占庭史料对穆斯林的通称"), entry("s2", "无关", &[], "拜占庭的一座城市")],
            ..Standard::default()
        };
        let found = candidates(&standard, &probe("撒拉森人", &[], "拜占庭史料对穆斯林的通称"), 6);
        assert_eq!(found, vec!["s1".to_string()], "the weak entry shares only 拜占庭");
    }

    #[test]
    fn a_rejected_word_reaches_the_entry_that_lists_it() {
        let found = candidates(&shelf(), &probe("撒拉逊", &[], "中世纪的称呼"), 6);
        assert_eq!(found.first().map(String::as_str), Some("s1"), "{found:?}");
    }

    #[test]
    fn a_row_sharing_no_character_pair_has_no_candidate() {
        assert!(candidates(&shelf(), &probe("琐罗亚斯德", &[], "古代波斯宗教"), 6).is_empty());
    }

    #[test]
    fn the_limit_holds() {
        let many = Standard {
            entries: (1..=20).map(|n| entry(&format!("s{n}"), &format!("税制{n}"), &[], "一种税制")).collect(),
            ..Standard::default()
        };
        assert_eq!(candidates(&many, &probe("税制", &[], "一种税制"), 6).len(), 6);
    }
}
