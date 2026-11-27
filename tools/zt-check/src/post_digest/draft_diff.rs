//! Draft-vs-original preservation check (`zt-check --draft-diff <orig.json> <draft.json> [--chapters a,b,...]`).
//!
//! Mirrors `skills/zt-model-restructure/scripts/check-drafts.mjs`, with strict JSON schema checks,
//! Term-link target head verification (`LinkForm::Term`), optional missing-target lookup, and
//! revision coherence across reads.

use crate::mcp::Zt;
use crate::text::{LinkForm, link_occurrences};
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const EQ_ID_SRC: &str = r"[A-Za-z]\.\d{1,2}[a-c]?|[A-Za-z]?\d{1,2}(?:\.\d{1,2})?[a-c]?";

static DISPLAY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\$\$[\s\S]*?\$\$").expect("display regex"));
static HSPACE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[ \t]+").expect("hspace regex"));
static TAG_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"[（(]({EQ_ID_SRC})[)）]")).expect("tag regex"));
static PREMISE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(依赖|用到|沿用|基于|代入|依据|承接|建立在|由此推出|取自|借用)\s*\[\[([^\]@]+)\]\]")
        .expect("premise regex")
});

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DraftFinding {
    pub address: String,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct DraftDiffReport {
    pub count: usize,
    pub count_mismatch: Option<String>,
    pub findings: Vec<DraftFinding>,
    pub ok: bool,
}

impl DraftDiffReport {
    pub fn to_json(&self) -> Value {
        json!({
            "ok": self.ok,
            "drafts": self.count,
            "count_mismatch": self.count_mismatch,
            "findings": self.findings.iter().map(|f| json!({
                "address": f.address,
                "issues": f.issues,
            })).collect::<Vec<_>>(),
        })
    }
}

fn display_formulas(body: &str) -> Vec<String> {
    DISPLAY_RE
        .find_iter(body)
        .map(|m| HSPACE_RE.replace_all(m.as_str().trim(), " ").into_owned())
        .collect()
}

fn eq_tags(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for cap in TAG_RE.captures_iter(text) {
        let id = cap.get(1).map_or("", |m| m.as_str());
        let norm = format!("({id})");
        if seen.insert(norm.clone()) {
            out.push(norm);
        }
    }
    out
}

fn count_map(items: impl IntoIterator<Item = String>) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for x in items {
        *map.entry(x).or_insert(0) += 1;
    }
    map
}

fn is_term_head_card(card: &Value) -> bool {
    card.get("type").and_then(Value::as_str) == Some("term")
        && card.get("term_role").and_then(Value::as_str) == Some("card")
}

/// Validates that every entry in a draft/original array is an object with unique non-empty string
/// `address`, plus string `title` and `body`.
pub fn validate_entries(items: &[Value], label: &str) -> Result<()> {
    let mut seen: HashSet<&str> = HashSet::new();
    for (idx, item) in items.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            bail!("{label}[{idx}] must be a JSON object");
        };
        let Some(addr) = obj
            .get("address")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        else {
            bail!("{label}[{idx}] lacks a non-empty string `address`");
        };
        if !seen.insert(addr) {
            bail!("{label} contains duplicate address `{addr}`");
        }
        if !obj.get("title").is_some_and(Value::is_string) {
            bail!("{label}[{idx}] lacks a string `title`");
        }
        if !obj.get("body").is_some_and(Value::is_string) {
            bail!("{label}[{idx}] lacks a string `body`");
        }
    }
    Ok(())
}

/// Validates that `orig` is non-empty and every entry in `orig` and `drafts` belongs to the same book key.
pub fn validate_batch_book_key(orig: &[Value], drafts: &[Value]) -> Result<String> {
    let Some(first_addr) = orig
        .first()
        .and_then(|o| o.get("address"))
        .and_then(Value::as_str)
    else {
        bail!("original card list contains no cards with an address");
    };
    let key = first_addr.split('/').next().unwrap_or(first_addr);
    let prefix = format!("{key}/");
    for (label, list) in [("orig", orig), ("drafts", drafts)] {
        for item in list {
            let addr = item.get("address").and_then(Value::as_str).unwrap_or("");
            if addr != key && !addr.starts_with(&prefix) {
                bail!("mixed book keys in {label}: expected `{key}`, got `{addr}`");
            }
        }
    }
    Ok(key.to_string())
}

pub fn check_drafts(
    orig: &[Value],
    drafts: &[Value],
    chapters: &[String],
    valid_in_tree: &HashSet<String>,
    valid_ext: &HashSet<String>,
    term_heads: &HashSet<String>,
) -> DraftDiffReport {
    let key = orig
        .first()
        .and_then(|o| o.get("address"))
        .and_then(Value::as_str)
        .and_then(|a| a.split('/').next())
        .unwrap_or("");
    let key_prefix = format!("{key}/");

    let count_mismatch = (drafts.len() != orig.len())
        .then(|| format!("count {} != {}", drafts.len(), orig.len()));

    let orig_batch_counts = count_map(
        orig.iter()
            .flat_map(|o| display_formulas(o.get("body").and_then(Value::as_str).unwrap_or(""))),
    );
    let draft_batch_counts = count_map(
        drafts
            .iter()
            .flat_map(|d| display_formulas(d.get("body").and_then(Value::as_str).unwrap_or(""))),
    );

    let mut draft_by_addr: HashMap<&str, &Value> = HashMap::new();
    let mut findings = Vec::new();
    for d in drafts {
        let addr = d.get("address").and_then(Value::as_str).unwrap_or("");
        if draft_by_addr.insert(addr, d).is_some() {
            findings.push(DraftFinding {
                address: addr.to_string(),
                issues: vec!["duplicate draft address".to_string()],
            });
        }
    }

    let mut orig_addrs: HashSet<&str> = HashSet::new();
    for o in orig {
        let addr = o.get("address").and_then(Value::as_str).unwrap_or("");
        if !orig_addrs.insert(addr) {
            findings.push(DraftFinding {
                address: addr.to_string(),
                issues: vec!["duplicate original address".to_string()],
            });
            continue;
        }
        let Some(&d) = draft_by_addr.get(addr) else {
            findings.push(DraftFinding {
                address: addr.to_string(),
                issues: vec!["missing".to_string()],
            });
            continue;
        };

        let mut f = Vec::new();
        let o_body = o.get("body").and_then(Value::as_str).unwrap_or("");
        let d_body = d.get("body").and_then(Value::as_str).unwrap_or("");
        let d_title = d.get("title").and_then(Value::as_str).unwrap_or("");
        let is_chapter = chapters.iter().any(|c| c == addr);
        let orig_facet = o.get("facet").and_then(Value::as_str).unwrap_or("");
        let draft_facet_opt = d.get("facet").and_then(Value::as_str);
        if !orig_facet.trim().is_empty() && draft_facet_opt.is_some_and(|s| s.trim().is_empty()) {
            f.push(format!("cleared original facet `{orig_facet}`"));
        }
        let effective_facet = draft_facet_opt
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(orig_facet);

        if !is_chapter {
            let have = count_map(display_formulas(d_body));
            let want = count_map(display_formulas(o_body));
            let mut lost = 0usize;
            for (eq, n) in &want {
                let got = have.get(eq).copied().unwrap_or(0);
                if got < *n {
                    lost += n - got;
                }
            }
            if lost > 0 {
                f.push(format!("lost {lost} display formula occurrence(s)"));
            }
            let combined = format!("{d_body}{d_title}");
            let draft_tags: HashSet<String> = eq_tags(&combined).into_iter().collect();
            let lost_tags: Vec<String> = eq_tags(o_body)
                .into_iter()
                .filter(|t| !draft_tags.contains(t))
                .collect();
            if !lost_tags.is_empty() {
                f.push(format!(
                    "equation numbers no longer named: {} (fine if only a mention was merged)",
                    lost_tags.join(" ")
                ));
            }
        } else {
            let mut seen_eq = HashSet::new();
            let mut dropped = 0usize;
            for eq in display_formulas(o_body) {
                if !seen_eq.insert(eq.clone()) {
                    continue;
                }
                if draft_batch_counts.get(&eq).copied().unwrap_or(0)
                    < orig_batch_counts.get(&eq).copied().unwrap_or(0)
                {
                    dropped += 1;
                }
            }
            if dropped > 0 {
                f.push(format!(
                    "chapter dropped {dropped} display formula occurrence(s) not preserved in batch multiplicity"
                ));
            }
            let self_idx = chapters.iter().position(|c| c == addr);
            for cap in PREMISE_RE.captures_iter(d_body) {
                let target = cap.get(2).map_or("", |m| m.as_str());
                if let (Some(ti), Some(si)) = (chapters.iter().position(|c| c == target), self_idx)
                    && ti >= si
                {
                    f.push(format!("rests on {target}, which is not an earlier chapter"));
                }
            }
        }

        if let Err(link_err) = crate::text::parse_links(d_body) {
            f.push(format!("malformed Link: {link_err}"));
        }
        for occ in link_occurrences(d_body) {
            let target = occ.target;
            let exists = if target == key || target.starts_with(&key_prefix) {
                if !valid_in_tree.contains(target) {
                    f.push(format!("Link to {target}, not in the tree"));
                    false
                } else {
                    true
                }
            } else if !valid_ext.contains(target) {
                f.push(format!("external Link to {target}, not found in archive"));
                false
            } else {
                true
            };
            if exists && occ.form == LinkForm::Term && !term_heads.contains(target) {
                f.push(format!("Term Link to {target}, not a Term head"));
            }
        }

        if d_body.contains("本模型") || d_body.contains("该模型") {
            f.push("says 本模型/该模型".to_string());
        }
        let max_title = if is_chapter { 80 } else { 40 };
        let title_len = d_title.chars().count();
        if title_len > max_title {
            f.push(format!("title {title_len} characters (max {max_title})"));
        }

        let required: &[&str] = match crate::formal::role_of(effective_facet.trim()) {
            Some(crate::formal::FormalRole::Derivation) => {
                &["目的：", "**思路**", "**起点**", "**步骤**", "**结果**", "**解读**"]
            }
            Some(crate::formal::FormalRole::Argument) => {
                &["目标：", "**依据**", "**论证**", "**结论**"]
            }
            Some(crate::formal::FormalRole::Result) => &["**陈述**", "**为什么**", "**依据**"],
            Some(crate::formal::FormalRole::Calibration) => &["**起点**", "**参数与出处**"],
            Some(crate::formal::FormalRole::Experiment) => {
                &["**实验设定**", "**数值结果**", "**边界**"]
            }
            Some(crate::formal::FormalRole::Criterion) => &["**起点**", "**作用域**"],
            _ => &[],
        };
        for h in required {
            if !d_body.lines().any(|l| l.trim_start().starts_with(h)) {
                f.push(format!("no {h}"));
            }
        }

        if !f.is_empty() {
            findings.push(DraftFinding {
                address: addr.to_string(),
                issues: f,
            });
        }
    }

    for d in drafts {
        let addr = d.get("address").and_then(Value::as_str).unwrap_or("");
        if !orig_addrs.contains(addr) {
            findings.push(DraftFinding {
                address: addr.to_string(),
                issues: vec!["extra draft not in original".to_string()],
            });
        }
    }

    let ok = count_mismatch.is_none() && findings.is_empty();
    DraftDiffReport {
        count: drafts.len(),
        count_mismatch,
        findings,
        ok,
    }
}

pub fn format_text(r: &DraftDiffReport) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(m) = &r.count_mismatch {
        lines.push(m.clone());
    }
    for f in &r.findings {
        lines.push(format!("{}: {}", f.address, f.issues.join("; ")));
    }
    let bad = r.findings.len() + usize::from(r.count_mismatch.is_some());
    lines.push(format!("{} draft(s), {bad} with findings", r.count));
    lines
}

fn read_json_array(path: &str) -> Result<Vec<Value>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
    let val: Value = serde_json::from_str(raw).with_context(|| format!("{path} is not valid JSON"))?;
    val.as_array()
        .cloned()
        .with_context(|| format!("{path} must be a JSON array of cards"))
}

pub fn run_live(
    zt: &mut Zt,
    orig_path: &str,
    draft_path: &str,
    chapters: &[String],
    json_out: bool,
) -> Result<(Vec<String>, u8, usize)> {
    let orig = read_json_array(orig_path)?;
    let drafts = read_json_array(draft_path)?;
    validate_entries(&orig, orig_path)?;
    validate_entries(&drafts, draft_path)?;
    let key = validate_batch_book_key(&orig, &drafts)?;
    let key = key.as_str();
    let key_prefix = format!("{key}/");

    let tree_reply = zt.call("get_tree", json!({ "tree": key }))?;
    let rev_before = tree_reply["archive_revision"]
        .as_u64()
        .context("get_tree answer lacks `archive_revision`")?;
    let tree_cards = tree_reply["cards"]
        .as_array()
        .context("get_tree answer lacks `cards` array")?;
    let mut valid_in_tree: HashSet<String> = HashSet::from([key.to_string()]);
    let mut term_heads: HashSet<String> = HashSet::new();
    for c in tree_cards {
        if let Some(a) = c.get("address").and_then(Value::as_str) {
            valid_in_tree.insert(a.to_string());
            if is_term_head_card(c) {
                term_heads.insert(a.to_string());
            }
        }
    }

    let mut ext_targets: Vec<String> = Vec::new();
    let mut seen_ext: HashSet<String> = HashSet::new();
    for d in &drafts {
        let body = d.get("body").and_then(Value::as_str).unwrap_or("");
        for occ in link_occurrences(body) {
            let target = occ.target;
            if target != key
                && !target.starts_with(&key_prefix)
                && seen_ext.insert(target.to_string())
            {
                ext_targets.push(target.to_string());
            }
        }
    }
    let mut valid_ext: HashSet<String> = HashSet::new();
    if !ext_targets.is_empty() {
        let fetched = zt.cards_optional(&ext_targets, "text")?;
        if fetched.archive_revision != rev_before {
            bail!("incomplete checker inputs: changing read view");
        }
        for (req, c) in fetched.resolved {
            valid_ext.insert(req.clone());
            if let Some(a) = c.get("address").and_then(Value::as_str) {
                valid_ext.insert(a.to_string());
                if is_term_head_card(&c) {
                    term_heads.insert(a.to_string());
                    term_heads.insert(req);
                }
            }
        }
    }
    if zt.cards(&[], "text")?.archive_revision != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }

    let report = check_drafts(&orig, &drafts, chapters, &valid_in_tree, &valid_ext, &term_heads);
    let fault_count = report.findings.len() + usize::from(report.count_mismatch.is_some());
    let exit = if report.ok { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report.to_json())?]
    } else {
        format_text(&report)
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_dropped_formula_forward_chapter_premise_bad_link_term_not_head_and_missing_role_sections() {
        let orig = vec![
            json!({
                "address": "K/1|a|1",
                "title": "第 1 章",
                "facet": "第 1 章",
                "body": "**直觉**\n直觉。\n$$x = 1$$"
            }),
            json!({
                "address": "K/1|a|1|a",
                "title": "结果：命题 1",
                "facet": "结果：命题 1",
                "body": "**陈述**\n由式 (1)：$$y = 2$$\n**为什么**\n原因。\n**依据**\n局部证明。"
            }),
        ];
        let drafts = vec![
            json!({
                "address": "K/1|a|1",
                "title": "第 1 章",
                "facet": "第 1 章",
                "body": "**直觉**\n依赖 [[K/1|a|2]]。本模型如此。"
            }),
            json!({
                "address": "K/1|a|1|a",
                "title": "结果：命题 1",
                "facet": "结果：命题 1",
                "body": "**陈述**\n见 [[K/missing]]、[[Other/9]] 与 [[假术语@K/1|a|2]]。"
            }),
        ];
        let chapters = vec!["K/1|a|1".to_string(), "K/1|a|2".to_string()];
        let valid_tree = HashSet::from(["K".into(), "K/1|a|1".into(), "K/1|a|2".into(), "K/1|a|1|a".into()]);
        let report = check_drafts(&orig, &drafts, &chapters, &valid_tree, &HashSet::new(), &HashSet::new());
        assert!(!report.ok);
        assert_eq!(report.findings.len(), 2);
        let ch_issues = &report.findings[0].issues;
        assert!(ch_issues.iter().any(|s| s.contains("display formula")), "{ch_issues:?}");
        assert!(ch_issues.iter().any(|s| s.contains("not an earlier chapter")), "{ch_issues:?}");
        assert!(ch_issues.iter().any(|s| s.contains("本模型/该模型")), "{ch_issues:?}");
        let res_issues = &report.findings[1].issues;
        assert!(res_issues.iter().any(|s| s.contains("lost 1 display formula")), "{res_issues:?}");
        assert!(res_issues.iter().any(|s| s.contains("K/missing")), "{res_issues:?}");
        assert!(res_issues.iter().any(|s| s.contains("Other/9")), "{res_issues:?}");
        assert!(res_issues.iter().any(|s| s.contains("not a Term head")), "{res_issues:?}");
        assert!(res_issues.iter().any(|s| s.contains("no **为什么**")), "{res_issues:?}");
    }

    #[test]
    fn chapter_moving_formula_to_side_card_in_same_batch_preserves_multiplicity_and_validates_entries() {
        let orig = vec![
            json!({ "address": "K/1|a|1", "title": "第 1 章", "facet": "第 1 章", "body": "$$x = 1$$" }),
            json!({ "address": "K/1|a|1|a", "title": "结果：A", "facet": "结果：A", "body": "**陈述**\nA\n**为什么**\nB\n**依据**\nC" }),
        ];
        let drafts = vec![
            json!({ "address": "K/1|a|1", "title": "第 1 章", "facet": "第 1 章", "body": "摘要。" }),
            json!({ "address": "K/1|a|1|a", "title": "结果：A", "facet": "结果：A", "body": "**陈述**\n$$x = 1$$\n**为什么**\nB\n**依据**\nC" }),
        ];
        let chapters = vec!["K/1|a|1".to_string()];
        let valid_tree = HashSet::from(["K".into(), "K/1|a|1".into(), "K/1|a|1|a".into()]);
        let report = check_drafts(&orig, &drafts, &chapters, &valid_tree, &HashSet::new(), &HashSet::new());
        assert!(report.ok, "{:?}", report.findings);

        // Missing or non-string body/title must fail validation cleanly.
        assert!(validate_entries(&[json!({ "address": "K/1", "title": "A" })], "draft").is_err());
        assert!(validate_entries(&[json!({ "address": "K/1", "title": 123, "body": "" })], "draft").is_err());

        // P0-7: duplicate addresses and mixed book keys must fail validation cleanly, and extra
        // draft addresses not in orig must be reported as findings.
        assert!(validate_entries(
            &[
                json!({ "address": "K/1", "title": "A", "body": "" }),
                json!({ "address": "K/1", "title": "A2", "body": "" }),
            ],
            "draft"
        )
        .is_err());
        assert!(validate_batch_book_key(
            &[json!({ "address": "K/1", "title": "A", "body": "" })],
            &[json!({ "address": "Other/1", "title": "A", "body": "" })],
        )
        .is_err());
        let extra_report = check_drafts(
            &[json!({ "address": "K/1", "title": "A", "body": "ok" })],
            &[
                json!({ "address": "K/1", "title": "A", "body": "ok" }),
                json!({ "address": "K/2", "title": "B", "body": "extra" }),
            ],
            &[],
            &HashSet::from(["K".into(), "K/1".into(), "K/2".into()]),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(!extra_report.ok);
        assert!(
            extra_report
                .findings
                .iter()
                .any(|f| f.address == "K/2" && f.issues.iter().any(|i| i.contains("extra"))),
            "{:?}",
            extra_report.findings
        );

        // P1-4: full-width `（1）` tags are tracked and required role headings must start a line
        // (not appear inline mid-sentence).
        let p14_orig = vec![json!({
            "address": "K/1|a|1|a",
            "title": "结果：命题 1",
            "facet": "结果：命题 1",
            "body": "**陈述**\n由式（1）成立。\n**为什么**\n原因。\n**依据**\n局部证明。"
        })];
        let p14_draft = vec![json!({
            "address": "K/1|a|1|a",
            "title": "结果：命题 1",
            "facet": "结果：命题 1",
            "body": "**陈述**\n式子省略了编号，这里顺便提到**为什么**和**依据**但不是标题。"
        })];
        let p14 = check_drafts(
            &p14_orig,
            &p14_draft,
            &[],
            &HashSet::from(["K".into(), "K/1|a|1|a".into()]),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(!p14.ok);
        let issues = &p14.findings[0].issues;
        assert!(issues.iter().any(|s| s.contains("(1)")), "dropped full-width （1） must be flagged: {issues:?}");
        assert!(issues.iter().any(|s| s.contains("no **为什么**")), "inline **为什么** must not satisfy heading: {issues:?}");
        assert!(issues.iter().any(|s| s.contains("no **依据**")), "inline **依据** must not satisfy heading: {issues:?}");

        // M9: empty draft facet must NOT suppress original Result headings, and unclosed `[[` must be caught.
        let m9_draft = vec![json!({
            "address": "K/1|a|1|a",
            "title": "结果：命题 1",
            "facet": "",
            "body": "**陈述**\n由式（1）与未闭合 [[K/1。"
        })];
        let m9 = check_drafts(
            &p14_orig,
            &m9_draft,
            &[],
            &HashSet::from(["K".into(), "K/1|a|1|a".into()]),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(!m9.ok);
        let m9_issues = &m9.findings[0].issues;
        assert!(m9_issues.iter().any(|s| s.contains("cleared original facet")), "{m9_issues:?}");
        assert!(m9_issues.iter().any(|s| s.contains("no **为什么**")), "{m9_issues:?}");
        assert!(m9_issues.iter().any(|s| s.contains("malformed Link")), "{m9_issues:?}");

        // M9: leading whitespace in draft facet (e.g. " 结果") must NOT disable required role headings.
        let m9_ws = check_drafts(
            &p14_orig,
            &[json!({
                "address": "K/1|a|1|a",
                "title": "结果：命题 1",
                "facet": " 结果",
                "body": "只保留式（1），没有任何必填标题。"
            })],
            &[],
            &HashSet::from(["K".into(), "K/1|a|1|a".into()]),
            &HashSet::new(),
            &HashSet::new(),
        );
        assert!(!m9_ws.ok);
        let ws_issues = &m9_ws.findings[0].issues;
        assert!(ws_issues.iter().any(|s| s.contains("no **陈述**")), "{ws_issues:?}");
        assert!(ws_issues.iter().any(|s| s.contains("no **为什么**")), "{ws_issues:?}");
        assert!(ws_issues.iter().any(|s| s.contains("no **依据**")), "{ws_issues:?}");
    }
}
