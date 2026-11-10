//! `zt-digest prepare ... -Extend <earlier staging>`: a prepared run that digests only the part of a book
//! an earlier digest left out, into the same Literature tree, without touching the earlier groups.
//!
//! The owner (2026-11-10): every digest of a work includes its appendices. Three economics books were
//! digested before that rule and stopped at their reference list (brandimarti2026reorganizing,
//! becker2025division, hansen2021demand). Their appendices are digested by an extension:
//!
//! - the earlier range is the earlier run's frozen `start_heading`, `end_heading` and `skip_headings`
//!   (its run contract); the new range is the one the current rule resolves. The extension stages exactly
//!   the units of the new range after the earlier range, which must be a prefix of the new one;
//! - its groups are numbered from F = (the largest n with a top-line Card `key/n` in the live tree) + 1, so a
//!   group n is still anchored at `key/n` (a reorganized tree may have fewer top-line Cards than groups);
//! - its setup writes no Literature root: the group anchors continue the top line from `key/(F-1)`;
//! - the book's Term list, Term Cards and Cited list are reused: the run freezes `terms` and `termCards`
//!   off, and the earlier staging's `terms.md` is copied so the writers use the book's words.

use super::{Unit, split_units};
use crate::ps::{eq, join_path, s};
use crate::util;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// The range an earlier digest of the same book covered, read from its staging folder.
#[derive(Clone, Debug)]
pub(crate) struct Earlier {
    pub(crate) staging: String,
    pub(crate) start: String,
    pub(crate) end: String,
    pub(crate) skip: Vec<String>,
}

impl Earlier {
    /// The earlier staging's frozen range. Only a managed run (one with a run contract) names it.
    pub(crate) fn read(staging: &str, key: &str) -> Result<Self> {
        let path = join_path(staging, "manifest.json");
        let manifest = util::read_json(&path).with_context(|| format!("-Extend: no manifest.json in {staging}"))?;
        if !eq(&s(&manifest["citation_key"]), key) {
            bail!("-Extend: {staging} holds the digest of `{}`, not of `{key}`", s(&manifest["citation_key"]));
        }
        if manifest.get("extends").is_some_and(|v| !v.is_null()) {
            bail!("-Extend: {staging} is itself an extension; extend from the book's first digest");
        }
        let options = &manifest["run_contract"]["options"];
        let (Some(start), Some(end), Some(skip)) =
            (options["start_heading"].as_str(), options["end_heading"].as_str(), options["skip_headings"].as_str())
        else {
            bail!("-Extend: {path} names no frozen range (run_contract.options start_heading, end_heading, skip_headings)");
        };
        Ok(Self {
            staging: crate::ps::full_name(staging),
            start: start.to_string(),
            end: end.to_string(),
            skip: skip.split(',').map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect(),
        })
    }

    /// The manifest's record of the extension (`extends`): later tools and the run report read it.
    pub(crate) fn record(&self, first_group: usize) -> Value {
        json!({
            "staging": self.staging,
            "first_group": first_group,
            "start_heading": self.start,
            "end_heading": self.end,
            "skip_headings": self.skip.join(","),
        })
    }
}

/// The units of the new range that the earlier range did not cover: those after the earlier end heading. The
/// earlier range must be a prefix of the new one: up to the earlier end heading, the new range selects the same
/// units, by heading and chapter, in the same order (the text may differ, because a source's translation is
/// repaired after a digest). Refused when it is not, or when nothing is left.
pub(crate) fn units(kept: &[String], earlier: &Earlier, start: &str, end: &str, skip: &[String]) -> Result<Vec<Unit>> {
    if earlier.end.is_empty() {
        bail!("-Extend: the earlier digest ran to the end of the source (no end heading); nothing is left to digest");
    }
    let old = split_units(kept, &earlier.start, &earlier.end, &earlier.skip);
    if old.is_empty() {
        bail!("-Extend: the earlier range (\"{}\" to \"{}\") selects no text of this source", earlier.start, earlier.end);
    }
    // The new range cut at the earlier end heading: a prefix of the new range, and it must be the earlier range.
    let head = split_units(kept, start, &earlier.end, skip);
    let same = |a: &Unit, b: &Unit| eq(&a.heading, &b.heading) && eq(&a.chapter, &b.chapter);
    if let Some(i) = (0..old.len().max(head.len())).find(|&i| !matches!((old.get(i), head.get(i)), (Some(a), Some(b)) if same(a, b))) {
        let name = |unit: Option<&Unit>| unit.map(|u| format!("\"{}\"", u.heading)).unwrap_or_else(|| "nothing".into());
        bail!(
            "-Extend: the earlier range is not a prefix of the new range: before \"{}\" its unit {} is {} and the new range's is {} (new range \"{start}\" to {}, skipping \"{}\")",
            earlier.end,
            i + 1,
            name(old.get(i)),
            name(head.get(i)),
            if end.is_empty() { "the end".to_string() } else { format!("\"{end}\"") },
            skip.join(","),
        );
    }
    let mut new = split_units(kept, start, end, skip);
    let rest = new.split_off(head.len().min(new.len()));
    if rest.is_empty() {
        bail!("-Extend: the new range ends where the earlier digest ended (\"{}\"); nothing is left to digest", earlier.end);
    }
    Ok(rest)
}

/// The first group number of an extension: one past the largest top-line Card `key/n` of the live tree
/// (`get_tree` of the book). An empty tree is refused: an extension continues an existing tree.
pub(crate) fn first_group(tree: &Value, key: &str) -> Result<usize> {
    let cards = tree["cards"].as_array().context("-Extend: the Literature tree read has no card list")?;
    if cards.is_empty() {
        bail!("-Extend: the Literature tree {key} does not exist; an extension continues an existing tree");
    }
    let prefix = format!("{}/", key.to_lowercase());
    let top = cards
        .iter()
        .filter_map(|card| {
            let address = s(&card["address"]);
            let rest = address.to_lowercase().strip_prefix(&prefix)?.to_string();
            (!rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit())).then(|| rest.parse::<usize>().ok()).flatten()
        })
        .max()
        .unwrap_or(0);
    Ok(top + 1)
}

/// The extension's run switches: the declared ones, with the book's Term list and Term Cards reused
/// (`terms` and `termCards` frozen off). A declared `true` for either is refused.
pub(crate) fn run_options(declared: Option<&Value>) -> Result<Value> {
    let mut options = match declared {
        Some(value) => value.as_object().context("run options must be a JSON object")?.clone(),
        None => serde_json::Map::new(),
    };
    for name in ["terms", "termCards"] {
        if options.get(name) == Some(&json!(true)) {
            bail!("-Extend reuses the book's Term list and Term Cards: run option `{name}` must be false");
        }
        options.insert(name.into(), json!(false));
    }
    Ok(Value::Object(options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kept(raw: &str) -> Vec<String> {
        raw.split("\n\n").map(str::to_string).collect()
    }

    fn earlier(end: &str, skip: &[&str]) -> Earlier {
        Earlier { staging: "D:\\old".into(), start: "1. 引言".into(), end: end.into(), skip: skip.iter().map(|h| h.to_string()).collect() }
    }

    // brandimarti2026reorganizing: the first digest stopped at 参考文献; the new rule skips the list and reads
    // the figures, tables and the model appendix after it.
    const PAPER: &str = "# 1. 引言\n\n引言正文。\n\n# 2. 模型\n\n模型正文。\n\n# 参考文献\n\n文献甲。\n\n# A. 补充图表\n\n图表正文。\n\n# B. 模型附录\n\n附录正文。\n\n## B.1. 证明\n\n证明正文。";

    #[test]
    fn the_extension_stages_only_the_units_after_the_earlier_range() {
        let rest = units(&kept(PAPER), &earlier("参考文献", &[]), "1. 引言", "", &["参考文献".to_string()]).unwrap();
        let headings: Vec<&str> = rest.iter().map(|u| u.heading.as_str()).collect();
        assert_eq!(headings, ["A. 补充图表", "B. 模型附录", "B.1. 证明"]);
        assert!(rest.iter().all(|u| !u.text.contains("文献甲") && !u.text.contains("引言正文")));
    }

    #[test]
    fn an_earlier_range_that_is_not_a_prefix_is_refused() {
        // The earlier digest skipped 2. 模型, which the new range reads: that unit lies inside the old range.
        let error = units(&kept(PAPER), &earlier("参考文献", &["2. 模型"]), "1. 引言", "", &["参考文献".to_string()]).unwrap_err().to_string();
        assert!(error.contains("not a prefix"), "{error}");
    }

    #[test]
    fn nothing_left_to_digest_is_refused() {
        let raw = "# 1. 引言\n\n引言正文。\n\n# 参考文献\n\n文献甲。\n\n# 索引\n\n索引正文。";
        let error = units(&kept(raw), &earlier("参考文献", &[]), "1. 引言", "参考文献", &[]).unwrap_err().to_string();
        assert!(error.contains("nothing is left"), "{error}");
        let error = units(&kept(raw), &earlier("", &[]), "1. 引言", "", &[]).unwrap_err().to_string();
        assert!(error.contains("nothing is left"), "{error}");
    }

    #[test]
    fn the_first_group_is_one_past_the_largest_top_line_card() {
        // becker2025division: 13 groups were digested, but the reorganized top line ends at the Cited list key/9.
        let cards = ["K", "K/1", "K/1|a", "K/7", "K/8", "K/9", "K/9|a", "K/10a", "K2/30"];
        let tree = json!({"cards": cards.iter().map(|a| json!({"address": a})).collect::<Vec<_>>()});
        assert_eq!(first_group(&tree, "K").unwrap(), 10);
        let error = first_group(&json!({"cards": []}), "K").unwrap_err().to_string();
        assert!(error.contains("does not exist"), "{error}");
    }

    #[test]
    fn an_extension_setup_creates_no_root_and_chains_from_the_last_top_line_card() {
        let groups = [
            json!({"n": 11, "headings": ["A. 补充图表"], "chapter": "A. 补充图表"}),
            json!({"n": 12, "headings": ["B. 模型附录", "B.1. 证明"], "chapter": "B. 模型附录"}),
        ];
        let calls = super::super::setup_calls("K", None, &groups.iter().collect::<Vec<_>>());
        assert!(calls.iter().all(|call| call["tool"] == "create_successor" && call["arguments"]["kind"] == "direct"));
        let at: Vec<&str> = calls.iter().map(|call| call["arguments"]["at"].as_str().unwrap()).collect();
        assert_eq!(at, ["K/10", "K/11"], "group 11 follows the top-line Card K/10, group 12 follows K/11");
        assert!(calls[0]["arguments"]["title"].as_str().unwrap().starts_with("第 11 组"));
        // A fresh preparation keeps its root and its first anchor under the root.
        let fresh = super::super::setup_calls("K", Some(json!({"workspace": "agent"})), &[&json!({"n": 1, "headings": ["导论"], "chapter": "导论"})]);
        assert_eq!(fresh[0]["tool"], "create_literature");
        assert_eq!(fresh[1]["arguments"]["at"], "K");
    }

    #[test]
    fn the_extension_reuses_the_term_list() {
        let options = run_options(Some(&json!({"models": true}))).unwrap();
        assert_eq!(options, json!({"models": true, "terms": false, "termCards": false}));
        assert!(run_options(Some(&json!({"terms": true}))).is_err());
    }
}
