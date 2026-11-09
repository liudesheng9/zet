//! `zt-digest evidence-link`: the Finding Cards of a book linked to the estimates of its empirical
//! models (docs/empirical-models-proposal.md §8.2 step 9, §7). Deterministic, no agent.
//!
//!   zt-digest evidence-link -CitationKey <key> [-Out <folder>] [-DryRun]
//!
//! A Finding Card matches an estimate part when both stand in the same group of the book (the
//! number after `<key>/`) and the Finding's `结果` line holds the estimate's value (the `值` line
//! of its `**估计**` section). A Finding that matches exactly one estimate gets
//! `依赖 [[estimate]]：…` in its body, and the estimate gets `对应 [[finding]]：…` in its
//! `**对应**` section. A Finding that matches none, or several, is reported, never forced. A Link
//! already there is kept, so a second run writes nothing. Prints one JSON line:
//! `{"ok","findings","estimates","linked","unmatched","ambiguous","file"}`.

use super::tables;
use crate::args::Args;
use crate::ps::{self, s, truthy};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::LazyLock;
use zt_check::empirical::{EmpiricalRole, role_of, section};

static GROUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+/(\d+)").unwrap());
static RESULT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*[-*]?\s*(?:\*\*)?结果(?:\*\*)?\s*[：:](.*)$").unwrap());
static PLAIN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-−–]?\d[\d,]*(?:\.\d+)?\s*%?\s*$").unwrap());
static UNCERTAIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(标准误|t\s*值|t\s*统计量|z\s*值|p\s*值)\s*(为|是|：|:|=)?\s*[-−–]?\d[\d.,]*|[（(]\s*[-−–]?\d[\d.,]*\s*[)）]").unwrap()
});
static VALUE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*-\s*值[：:]\s*([^。\n]*)").unwrap());

/// The group a Card of the book stands in.
fn group(address: &str) -> Option<String> {
    GROUP.captures(address).map(|c| c[1].to_string())
}

/// The Card a Location is filed under inside its tree: the address without its last `|` segment.
fn parent(at: &str) -> Option<String> {
    at.rsplit_once('|').map(|(up, _)| up.to_string())
}

/// One estimate part of an empirical model: its address, its group and its value.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Estimate {
    pub(crate) address: String,
    pub(crate) group: String,
    /// The value, when the 值 line is one plain number; a prose value line gives magnitudes instead.
    pub(crate) value: Option<String>,
    /// The magnitudes the estimate reads in its title and **量级** (`3.5 个百分点`), without a small integer.
    pub(crate) magnitudes: Vec<String>,
}

/// A number that can tell estimates apart: a decimal, or an integer above 12 (`3 个百分点`, `10% 水平` cannot).
fn telling(number: &str) -> bool {
    number.contains('.') || number.trim_start_matches('-').parse::<u64>().is_ok_and(|n| n > 12)
}

/// The estimate parts of the book's empirical models, from the tree's cards (each with address,
/// type, facet, level and body).
pub(crate) fn estimates(cards: &[Value]) -> Vec<Estimate> {
    let by_address: BTreeMap<String, &Value> = cards.iter().map(|c| (s(&c["address"]), c)).collect();
    let head_level = |at: &str| -> Option<String> {
        let mut up = parent(at);
        while let Some(above) = up {
            if let Some(card) = by_address.get(&above)
                && s(&card["type"]) == "model"
                && !truthy(&card["facet"])
            {
                return Some(s(&card["level"]));
            }
            up = parent(&above);
        }
        None
    };
    let mut out = Vec::new();
    for card in cards {
        let at = s(&card["address"]);
        if s(&card["type"]) != "model" || role_of(&s(&card["facet"])) != Some(EmpiricalRole::Estimate) {
            continue;
        }
        if head_level(&at).as_deref() != Some("empirical") {
            continue;
        }
        let body = s(&card["body"]);
        let line = section(&body, "估计").and_then(|estimate| VALUE.captures(&estimate).map(|c| c[1].trim().to_string()));
        // A value is one plain number; a prose line (「99 国中 55 国……」) states magnitudes.
        let value = line.as_deref().filter(|l| PLAIN.is_match(l)).and_then(|l| tables::numbers(l).into_iter().next());
        let prose = if value.is_none() { line.unwrap_or_default() } else { String::new() };
        let read = format!("{}\n{}\n{prose}", s(&card["title"]), section(&body, "量级").unwrap_or_default());
        let mut magnitudes: Vec<String> = Vec::new();
        for n in tables::numbers(&read).into_iter().filter(|n| telling(n)) {
            if !magnitudes.contains(&n) {
                magnitudes.push(n);
            }
        }
        if let Some(group) = group(&at)
            && (value.is_some() || !magnitudes.is_empty())
        {
            out.push(Estimate { address: at, group, value, magnitudes });
        }
    }
    out
}

/// What a Finding Card matches: the estimates of its group whose value its 结果 line holds, else those
/// whose stated magnitude it holds.
pub(crate) fn matches<'a>(finding: &Value, estimates: &'a [Estimate]) -> Vec<&'a Estimate> {
    let at = s(&finding["address"]);
    let Some(g) = group(&at) else { return Vec::new() };
    let body = s(&finding["body"]);
    // A t value, a standard error or a number in parentheses is the uncertainty of a result, not its value.
    let numbers: Vec<String> = RESULT
        .captures_iter(&body)
        .flat_map(|c| tables::numbers(&UNCERTAIN.replace_all(&c[1], " ")))
        .collect();
    let by_value: Vec<&Estimate> =
        estimates.iter().filter(|e| e.group == g && e.value.as_ref().is_some_and(|v| numbers.contains(v))).collect();
    if !by_value.is_empty() {
        return by_value;
    }
    // A Finding often states the text's magnitude, which the estimate reads in its title and 量级. An estimate
    // that states several numbers needs two of them, so one shared count (99 countries) tells nothing.
    let score = |e: &Estimate| e.magnitudes.iter().filter(|m| numbers.contains(m)).count();
    let need = |e: &Estimate| if e.magnitudes.len() >= 2 { 2 } else { 1 };
    let best = estimates.iter().filter(|e| e.group == g && score(e) >= need(e)).map(score).max().unwrap_or(0);
    estimates.iter().filter(|e| e.group == g && best > 0 && score(e) >= need(e) && score(e) == best).collect()
}

/// The Finding's new body: its Link to the estimate, before its source line.
pub(crate) fn finding_body(body: &str, estimate: &str) -> Option<String> {
    if body.contains(&format!("[[{estimate}]]")) {
        return None;
    }
    let line = format!("依赖 [[{estimate}]]：模型中的这个估计给出本结果的规格、表格和不确定性。");
    let lines: Vec<&str> = body.lines().collect();
    let at = lines.iter().rposition(|l| {
        let t = l.trim_start();
        t.starts_with("来源：") || t.starts_with("来源:") || t.starts_with("- 来源") || t.starts_with("**来源**")
    });
    Some(match at {
        Some(k) => {
            let mut out = lines[..k].join("\n").trim_end().to_string();
            out.push_str(&format!("\n\n{line}\n\n"));
            out.push_str(&lines[k..].join("\n"));
            out
        }
        None => format!("{}\n\n{line}", body.trim_end()),
    })
}

/// The estimate's new body: `对应 [[finding]]：…` in its `**对应**` section.
pub(crate) fn estimate_body(body: &str, finding: &str) -> Option<String> {
    if body.contains(&format!("[[{finding}]]")) {
        return None;
    }
    let line = format!("对应 [[{finding}]]：本书对这个结果的陈述。");
    let current = super::text::get_section(body, "对应");
    let content = match current {
        Some(section) => format!("{}\n{line}", section.trim_end()),
        None => format!("**对应**\n{line}"),
    };
    Some(super::text::set_section(body, "对应", &content))
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["CitationKey", "Out"], &["DryRun"])?;
    let key = a.required("CitationKey")?;
    let mut zt = crate::zt::start()?;
    if !a.flag("DryRun")
        && let Some(refusal) = crate::lockcheck::preflight(&mut zt, "evidence-link", "link", &key)?
    {
        return crate::lockcheck::stop("evidence-link", &refusal);
    }
    let tree = super::read_one(&mut zt, "get_tree", json!({ "tree": key, "bodies": true }))?;
    let cards: Vec<Value> = ps::pipe(&tree["cards"]).into_iter().cloned().collect();
    let found = estimates(&cards);
    let mut linked = Vec::new();
    let mut unmatched = Vec::new();
    let mut ambiguous = Vec::new();
    let mut calls = Vec::new();
    let mut estimate_bodies: BTreeMap<String, String> =
        cards.iter().filter(|c| found.iter().any(|e| e.address == s(&c["address"]))).map(|c| (s(&c["address"]), s(&c["body"]))).collect();
    let findings: Vec<&Value> = cards.iter().filter(|c| s(&c["type"]) == "finding").collect();
    for finding in &findings {
        let at = s(&finding["address"]);
        let hits = matches(finding, &found);
        match hits.as_slice() {
            [] => unmatched.push(json!(at)),
            [one] => {
                if let Some(body) = finding_body(&s(&finding["body"]), &one.address) {
                    calls.push(json!({ "tool": "edit_card", "arguments": { "at": at, "body": body } }));
                }
                let current = estimate_bodies.get(&one.address).cloned().unwrap_or_default();
                if let Some(body) = estimate_body(&current, &at) {
                    estimate_bodies.insert(one.address.clone(), body);
                }
                linked.push(json!({ "finding": at, "estimate": one.address }));
            }
            many => ambiguous.push(json!({ "finding": at, "estimates": many.iter().map(|e| e.address.clone()).collect::<Vec<_>>() })),
        }
    }
    for card in &cards {
        let at = s(&card["address"]);
        if let Some(body) = estimate_bodies.get(&at)
            && *body != s(&card["body"])
        {
            calls.push(json!({ "tool": "edit_card", "arguments": { "at": at, "body": body } }));
        }
    }
    let writes = calls.len();
    if !a.flag("DryRun") && !calls.is_empty() {
        let mut options = crate::calls::Options::new();
        let (_, failure) = crate::calls::run(&mut zt, &calls, &mut options).map_err(|e| anyhow::anyhow!("zt helper failed: {e:#}"))?;
        if let Some(failure) = failure {
            anyhow::bail!("evidence-link: a write failed: {}", failure.lines().join(" | "));
        }
    }
    let report = json!({ "key": key, "linked": linked, "unmatched": unmatched, "ambiguous": ambiguous, "writes": writes });
    let out = a.str("Out");
    let file = if out.is_empty() {
        String::new()
    } else {
        let path = ps::join(&out, "evidence-link.json");
        util::write_text(&path, &serde_json::to_string_pretty(&report)?)?;
        path
    };
    util::print_line(&util::compact(&json!({
        "ok": true, "findings": findings.len(), "estimates": found.len(), "linked": linked.len(),
        "unmatched": unmatched.len(), "ambiguous": ambiguous.len(), "writes": writes, "file": file,
    })))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards() -> Vec<Value> {
        vec![
            json!({"address": "K/5|b", "type": "model", "level": "empirical", "body": "问题"}),
            json!({"address": "K/5|b|a", "type": "model", "facet": "数据", "body": ""}),
            json!({"address": "K/5|b|a|1|1|a", "type": "model", "facet": "估计：企业规模", "body": "规模大。\n\n**估计**\n- 估计量：[[K/5|b|a|1]] 中 β，表 2 (1)。\n- 值：0.014。\n- 标准误：0.003。\n\n**量级**\n待写\n\n**依据**\n依赖 [[K/5|b|a|1]]。"}),
            json!({"address": "K/5|b|a|1|1|b", "type": "model", "facet": "估计：跨国公司", "body": "**估计**\n- 值：0.047。\n- 标准误：0.017。\n\n**量级**\n待写\n\n**依据**\n依赖 [[K/5|b|a|1]]。"}),
            json!({"address": "K/6|a", "type": "model", "level": "formal", "body": ""}),
            json!({"address": "K/6|a|a", "type": "model", "facet": "估计：别的", "body": "**估计**\n- 值：0.014。"}),
            json!({"address": "K/5|c|a", "type": "finding", "body": "测量：规模。\n数据：搜寻。\n方法：OLS。\n结果：系数 0.014，标准误 0.003。\n局限：横截面。\n来源：[[K]] §5.2"}),
            json!({"address": "K/5|c|b", "type": "finding", "body": "结果：跨国公司的系数是 0.047。\n来源：[[K]] §5.2"}),
            json!({"address": "K/5|c|c", "type": "finding", "body": "结果：上市公司系数 0.030。\n来源：[[K]] §5.2"}),
            json!({"address": "K/7|c|a", "type": "finding", "body": "结果：系数 0.014。\n来源：[[K]] §7"}),
        ]
    }

    #[test]
    fn only_the_estimates_of_an_empirical_model_are_read() {
        let found = estimates(&cards());
        assert_eq!(found.iter().map(|e| e.address.as_str()).collect::<Vec<_>>(), ["K/5|b|a|1|1|a", "K/5|b|a|1|1|b"]);
        assert_eq!(found[0].value.as_deref(), Some(".014"));
        assert_eq!(found[0].group, "5");
    }

    #[test]
    fn a_finding_matches_the_estimate_of_its_group_whose_value_it_states() {
        let cards = cards();
        let found = estimates(&cards);
        let hit = |at: &str| matches(cards.iter().find(|c| c["address"] == at).unwrap(), &found).iter().map(|e| e.address.clone()).collect::<Vec<_>>();
        assert_eq!(hit("K/5|c|a"), ["K/5|b|a|1|1|a"]);
        assert_eq!(hit("K/5|c|b"), ["K/5|b|a|1|1|b"]);
        assert!(hit("K/5|c|c").is_empty(), "no estimate holds 0.030");
        assert!(hit("K/7|c|a").is_empty(), "another group");
    }

    /// hansen2021demand pilot (2026-11-09): a Finding states the text's magnitude ("3.5 个百分点"), which
    /// the estimate holds in its title and **量级**; the value wins over a magnitude, and a small integer
    /// ("3 个百分点", "10% 水平") is no evidence.
    #[test]
    fn a_finding_that_states_the_magnitude_matches_the_estimate_that_reads_it() {
        let mut cards = cards();
        cards[2]["title"] = json!("估计：规模大一个标准差，概率高 3.5 个百分点");
        cards[2]["body"] = json!(format!("{}\n\n**量级**\n对数就业一个标准差对应概率高 3.5 个百分点。", cards[2]["body"].as_str().unwrap().replace("\n\n**量级**\n待写", "")));
        cards[3]["body"] = json!(format!("{}\n\n**量级**\n跨国企业的概率高 4.7 个百分点，在 1% 水平上显著。", cards[3]["body"].as_str().unwrap().replace("\n\n**量级**\n待写", "")));
        cards.push(json!({"address": "K/5|c|d", "type": "finding", "body": "结果：对数就业每提高一个标准差，社交技能要求的概率高 3.5 个百分点。\n来源：[[K]] §5.2"}));
        cards.push(json!({"address": "K/5|c|f", "type": "finding", "body": "结果：并购活动对应 3 个百分点的更高概率，在 10% 水平显著。\n来源：[[K]] §5.2"}));
        let found = estimates(&cards);
        let hit = |at: &str| matches(cards.iter().find(|c| c["address"] == at).unwrap(), &found).iter().map(|e| e.address.clone()).collect::<Vec<_>>();
        assert_eq!(hit("K/5|c|d"), ["K/5|b|a|1|1|a"]);
        assert!(hit("K/5|c|f").is_empty(), "3 and 10 are small integers");
        assert_eq!(hit("K/5|c|a"), ["K/5|b|a|1|1|a"], "the value still matches");
    }

    /// ebrill2001modern pilot (2026-11-09): a value is read only from a plain number; a Finding's
    /// t value or standard error is no value; a prose estimate that states several numbers needs two.
    #[test]
    fn uncertainty_numbers_and_prose_values_do_not_make_a_match() {
        let est = |address: &str, value: &str| json!({"address": address, "type": "model", "facet": "估计：x", "title": "估计", "body": format!("**估计**\n- 值：{value}。\n\n**量级**\n待写")});
        let cards = vec![
            json!({"address": "K/4|d|a", "type": "model", "level": "empirical", "body": ""}),
            est("K/4|d|a|a|6|c", "-0.21"),
            est("K/4|d|a|a|7|a", "99 国中 55 国预测收入更高，这 55 国的平均收入增长约为 GDP 的 0.4%"),
            json!({"address": "K/4|e|e", "type": "finding", "body": "结果：交互项系数为 -0.06，t 值为 -0.21。\n来源：[[K]]"}),
            json!({"address": "K/4|d|i", "type": "finding", "body": "结果：99 个增值税国家中 55 个的预测收入更高，平均高约 GDP 的 0.4%。\n来源：[[K]]"}),
            json!({"address": "K/4|d|j", "type": "finding", "body": "结果：99 个增值税国家中 93 个的预测收入更高，平均高约 GDP 的 4%。\n来源：[[K]]"}),
            json!({"address": "K/4|d|k", "type": "finding", "body": "结果：系数为 -0.21（0.09）。\n来源：[[K]]"}),
        ];
        let found = estimates(&cards);
        let hit = |at: &str| matches(cards.iter().find(|c| c["address"] == at).unwrap(), &found).iter().map(|e| e.address.clone()).collect::<Vec<_>>();
        assert!(hit("K/4|e|e").is_empty(), "-0.21 is the Finding's t value");
        assert_eq!(hit("K/4|d|i"), ["K/4|d|a|a|7|a"]);
        assert!(hit("K/4|d|j").is_empty(), "99 alone does not tell the 55-country estimate");
        assert_eq!(hit("K/4|d|k"), ["K/4|d|a|a|6|c"], "a coefficient with its error in parentheses");
    }

    #[test]
    fn the_links_go_before_the_source_line_and_into_the_correspondence_section() {
        let finding = "结果：系数 0.014。\n来源：[[K]] §5.2";
        let body = finding_body(finding, "K/5|b|a|1|1|a").unwrap();
        assert_eq!(body, "结果：系数 0.014。\n\n依赖 [[K/5|b|a|1|1|a]]：模型中的这个估计给出本结果的规格、表格和不确定性。\n\n来源：[[K]] §5.2");
        assert!(finding_body(&body, "K/5|b|a|1|1|a").is_none(), "a second run writes nothing");
        let estimate = "**估计**\n- 值：0.014。\n\n**依据**\n依赖 [[K/5|b|a|1]]。";
        let with = estimate_body(estimate, "K/5|c|a").unwrap();
        assert!(with.contains("**对应**\n对应 [[K/5|c|a]]：本书对这个结果的陈述。"), "{with}");
        let twice = estimate_body(&with, "K/5|c|z").unwrap();
        assert!(twice.contains("对应 [[K/5|c|a]]：本书对这个结果的陈述。\n对应 [[K/5|c|z]]"), "{twice}");
        assert!(estimate_body(&twice, "K/5|c|a").is_none());
    }
}
