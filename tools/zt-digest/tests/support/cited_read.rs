//! A stand-in for the DeepSeek reading jobs of `zt-digest cited` (2026-11-07): it answers every part of a
//! Cited plan with one entry per material line, split by a small rule, so a test can reach `apply` and the
//! Cited Cards without a model. A test passes its own rule for lines the default does not split.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// The fields of one line: (authors, year, title), or `None` to skip the line.
pub type Fields = dyn Fn(&str) -> Option<(String, String, String)>;

/// The default rule: the authors stand before the first year, the title after it up to the next full stop
/// or closing quote. A line without a year is skipped.
pub fn default_fields(text: &str) -> Option<(String, String, String)> {
    let year_at = text.char_indices().find_map(|(i, _)| {
        let digits: String = text[i..].chars().take(4).collect();
        let before = text[..i].chars().next_back();
        let after = text[i..].chars().nth(4);
        (digits.len() == 4
            && digits.chars().all(|c| c.is_ascii_digit())
            && (digits.starts_with("19") || digits.starts_with("20"))
            && !before.is_some_and(|c| c.is_ascii_digit())
            && !after.is_some_and(|c| c.is_ascii_digit()))
        .then_some(i)
    })?;
    let mut year: String = text[year_at..].chars().take(4).collect();
    if let Some(letter) = text[year_at + 4..].chars().next().filter(|c| c.is_ascii_lowercase()) {
        year.push(letter);
    }
    let authors = text[..year_at].trim().trim_end_matches(['(', ',', '.', ' ']).trim().to_string();
    let rest = text[year_at + year.len()..].trim_start_matches([')', '.', ',', ':', ' ', '“', '"', '‘']);
    let end = rest.find(['.', '”', '"', '’']).unwrap_or(rest.len());
    let title = rest[..end].trim().to_string();
    (!title.is_empty()).then_some((authors, year, title))
}

/// Answer every part of the staging's Cited plan with `fields`. Returns the answer files.
pub fn answer_parts(staging: &Path, fields: &Fields) -> Vec<PathBuf> {
    let read = staging.join("cited").join("read");
    let plan: Value = serde_json::from_str(&std::fs::read_to_string(staging.join("cited").join("plan.json")).expect("cited plan")).expect("plan JSON");
    let mut written = Vec::new();
    for part in plan["parts"].as_array().expect("parts") {
        let id = part["id"].as_str().unwrap();
        let text = std::fs::read_to_string(read.join(format!("part-{id}.md"))).expect("part file");
        let mut entries = Vec::new();
        let mut skip = Vec::new();
        for line in text.lines().filter(|l| l.starts_with('L')) {
            let Some((number, body)) = line[1..].split_once(": ") else { continue };
            let Ok(n) = number.parse::<u64>() else { continue };
            match fields(body) {
                Some((authors, year, title)) => entries.push(json!({ "lines": [n], "entry": body, "authors": authors, "year": year, "title": title, "venue": "" })),
                None => skip.push(n),
            }
        }
        let path = read.join("out").join(format!("{id}.json"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, json!({ "entries": entries, "skip": skip }).to_string()).unwrap();
        written.push(path);
    }
    written
}
