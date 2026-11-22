//! The statement index of a mathematics book, found by DeepSeek (owner, 2026-11-21): a book states
//! its definitions and results in more ways than any pattern can list (numbered environments,
//! a rule in a sentence, "this is called …", a worked derivation with no label), so a DeepSeek job
//! reads each prepared section with its line numbers and lists every statement it finds. This file
//! holds the pure half: the job text, the check of one answer against the numbered lines it was
//! given, and the index built from all answers in book order. The index has the record shape the
//! skeleton, notation, boundary, proof and theory steps already read.

use crate::ps::{self, CiMap, field, s};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

/// The kinds an answer may give: the six unit forms, then the two kinds that go beside a unit.
pub const UNIT_FORMS: &[&str] = &["definition", "axiom", "theorem", "proposition", "lemma", "corollary"];
pub const OTHER_KINDS: &[&str] = &["example", "exercise"];
const RESULTS: &[&str] = &["theorem", "proposition", "lemma", "corollary"];
/// The fewest characters of a `quote` that tie a statement to its line.
pub const MIN_QUOTE: usize = 4;

static SPACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
/// A book's own numbered label, kept as the unit's id in the form the old index used (`定理 3.2`).
static NUMBERED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(定义|公理|定理|命题|引理|推论|例子|例|习题|练习|Definition|Axiom|Theorem|Proposition|Lemma|Corollary|Example|Exercise)\s*([0-9]+(?:[.．][0-9A-Za-z]+)*)\s*$").unwrap()
});

/// One prepared section a job reads: its file name, group, chapter and lines.
#[derive(Clone)]
pub struct Section {
    pub file: String,
    pub group: i64,
    pub chapter: String,
    pub lines: Vec<String>,
}

fn squeeze(text: &str) -> String {
    SPACE.replace_all(text, "").to_string()
}

/// The Chinese word of a kind, for an id the book gives no label.
pub fn kind_zh(kind: &str) -> &'static str {
    match kind {
        "definition" => "定义",
        "axiom" => "公理",
        "theorem" => "定理",
        "proposition" => "命题",
        "lemma" => "引理",
        "corollary" => "推论",
        "example" => "例",
        "exercise" => "习题",
        _ => "",
    }
}

/// The job text of one finder job: each section's lines, numbered as the answer must cite them.
pub fn job_text(title: &str, id: &str, out: &str, check: &str, sections: &[Section]) -> String {
    let mut sb = format!("# 找出数学陈述：任务 {id}，《{title}》\n\n");
    sb.push_str(&format!("把结果写入 `{out}`。只读本文件，不读其他文件。\n\n"));
    sb.push_str(&format!("SELF-CHECK: {check}\n\n"));
    sb.push_str("下面每一节是一个原文文件。每行开头是行号和 `│`，行号不属于原文。`section` 写文件名，`line`、`end`、`proof` 写这里的行号。\n\n");
    for sec in sections {
        sb.push_str(&format!("## 文件 `{}`（第 {} 组，{}）\n\n", sec.file, sec.group, sec.chapter));
        for (i, l) in sec.lines.iter().enumerate() {
            sb.push_str(&format!("{}│ {}\n", i + 1, l));
        }
        sb.push('\n');
    }
    sb
}

/// The faults of one answer against the sections its job gave. Each fault is one line that
/// names the entry and what to change. An empty list is a passing answer.
pub fn check_answer(answer: &Value, sections: &[Section]) -> Vec<String> {
    let mut faults = Vec::new();
    let Some(list) = answer.get("statements").and_then(Value::as_array) else {
        return vec!["the answer has no `statements` array".into()];
    };
    let by_file: CiMap<&Section> = {
        let mut m = CiMap::new();
        for sec in sections {
            m.insert(&sec.file, sec);
        }
        m
    };
    let mut starts: CiMap<usize> = CiMap::new();
    let mut spans: Vec<(String, i64, i64, usize, bool)> = Vec::new();
    for (k, e) in list.iter().enumerate() {
        let n = k + 1;
        let kind = field(e, "kind").to_lowercase();
        let unit = UNIT_FORMS.contains(&kind.as_str());
        if !unit && !OTHER_KINDS.contains(&kind.as_str()) {
            faults.push(format!("statement {n}: kind `{kind}` is not one of {}", [UNIT_FORMS, OTHER_KINDS].concat().join(", ")));
            continue;
        }
        let name = field(e, "name").trim().to_string();
        if name.is_empty() {
            faults.push(format!("statement {n}: `name` is empty"));
        }
        let file = field(e, "section");
        let Some(sec) = by_file.get(&file) else {
            faults.push(format!("statement {n}: section `{file}` is not a file of this job"));
            continue;
        };
        let total = sec.lines.len() as i64;
        let line = e["line"].as_i64().unwrap_or(0);
        let end = e["end"].as_i64().unwrap_or(0);
        let proof = e["proof"].as_i64().unwrap_or(0);
        if line < 1 || line > total {
            faults.push(format!("statement {n} ({name}): line {line} is outside {file} (1-{total})"));
            continue;
        }
        if end < line || end > total {
            faults.push(format!("statement {n} ({name}): end {end} must be from line {line} to {total}"));
            continue;
        }
        if proof != 0 && (proof <= line || proof > end) {
            faults.push(format!("statement {n} ({name}): proof {proof} must be 0 or a line after {line} and not after {end}"));
        }
        if proof != 0 && !unit {
            faults.push(format!("statement {n} ({name}): an {kind} has no proof line; write proof 0"));
        }
        let quote = squeeze(&field(e, "quote"));
        let text = squeeze(&sec.lines[(line - 1) as usize]);
        if quote.chars().count() < MIN_QUOTE {
            faults.push(format!("statement {n} ({name}): quote must copy at least {MIN_QUOTE} characters of line {line}"));
        } else if !text.contains(&quote) {
            faults.push(format!("statement {n} ({name}): quote is not on line {line} of {file}; give the line where the statement starts"));
        }
        let key = format!("{file}:{line}");
        if let Some(first) = starts.get(&key) {
            faults.push(format!("statement {n} ({name}): statement {first} starts on the same line {line} of {file}"));
        } else {
            starts.insert(&key, n);
        }
        spans.push((file, line, end, n, unit));
    }
    // Two units never overlap: each unit is one block of the book. An example or exercise may sit
    // inside the block of the unit it serves.
    let units: Vec<&(String, i64, i64, usize, bool)> = spans.iter().filter(|x| x.4).collect();
    for (i, a) in units.iter().enumerate() {
        for b in units.iter().skip(i + 1) {
            if ps::eq(&a.0, &b.0) && a.1 <= b.2 && b.1 <= a.2 {
                faults.push(format!("statements {} and {} overlap in {} (lines {}-{} and {}-{}); end the first before the second starts", a.3, b.3, a.0, a.1, a.2, b.1, b.2));
            }
        }
    }
    faults
}

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = Map::new();
    for (k, v) in pairs {
        map.insert(k.to_string(), v);
    }
    Value::Object(map)
}

/// The index from every job's checked answer: units and others in book order (group, then the
/// section's place in the prepared order, then line), each with the record shape the later steps
/// read. A labelled statement keeps the book's label as its id; an unlabelled one gets its kind
/// word and its name (`定理「乘积的导数」`). The cites resolve by id, label or name.
pub fn build(answers: &[Value], sections: &[Section]) -> (Vec<Value>, Vec<Value>) {
    let place: CiMap<(i64, usize, String)> = {
        let mut m = CiMap::new();
        for (i, sec) in sections.iter().enumerate() {
            m.insert(&sec.file, (sec.group, i, sec.chapter.clone()));
        }
        m
    };
    let mut all: Vec<(i64, usize, i64, Value)> = Vec::new();
    for answer in answers {
        for e in ps::pipe(&answer["statements"]) {
            let file = field(e, "section");
            let Some((group, order, _)) = place.get(&file) else { continue };
            all.push((*group, *order, e["line"].as_i64().unwrap_or(0), e.clone()));
        }
    }
    all.sort_by(|a, b| (a.0, a.1, a.2).cmp(&(b.0, b.1, b.2)));
    let mut used: CiMap<usize> = CiMap::new();
    let mut units: Vec<Value> = Vec::new();
    let mut others: Vec<Value> = Vec::new();
    let mut aliases: CiMap<String> = CiMap::new();
    let mut raw_cites: Vec<Vec<String>> = Vec::new();
    for (group, _, _, e) in &all {
        let file = field(e, "section");
        let chapter = place.get(&file).map(|p| p.2.clone()).unwrap_or_default();
        let kind = field(e, "kind").to_lowercase();
        let label = field(e, "label").trim().to_string();
        let name = field(e, "name").trim().to_string();
        let (word, number) = match NUMBERED.captures(&label) {
            Some(m) => (m[1].to_string(), m[2].replace('．', ".")),
            None => (String::new(), String::new()),
        };
        let base = if !number.is_empty() {
            format!("{word} {number}")
        } else if !label.is_empty() {
            label.clone()
        } else {
            format!("{}「{}」", kind_zh(&kind), name)
        };
        let seen = used.get(&base).copied().unwrap_or(0);
        used.insert(&base, seen + 1);
        let id = if seen == 0 { base.clone() } else { format!("{base}（{}）", seen + 1) };
        let proof = e["proof"].as_i64().unwrap_or(0);
        let mut entry = obj(vec![
            ("id", json!(id)),
            ("label", json!(label)),
            ("word", json!(word)),
            ("number", json!(number)),
            ("name", json!(name)),
            ("group", json!(group)),
            ("chapter", json!(chapter)),
            ("section", json!(file)),
            ("line", e["line"].clone()),
            ("end", e["end"].clone()),
            ("proof", json!(proof)),
        ]);
        let map = entry.as_object_mut().unwrap();
        if UNIT_FORMS.contains(&kind.as_str()) {
            for alias in [&id, &label, &name, &base] {
                if !alias.is_empty() && !aliases.contains(alias) {
                    aliases.insert(alias, id.clone());
                }
            }
            map.insert("form".into(), json!(kind));
            let level = if RESULTS.contains(&kind.as_str()) && proof > 0 { "formal" } else { "concept" };
            map.insert("level".into(), json!(level));
            raw_cites.push(ps::pipe(&e["cites"]).into_iter().map(s).collect());
            units.push(entry);
        } else {
            map.insert("kind".into(), json!(kind));
            // An example or exercise belongs to the unit the answer names, else to the last unit
            // before it in the same section.
            let named = field(e, "belongs_to");
            let after = match units.last() {
                Some(u) if ps::eq(&field(u, "section"), &file) => field(u, "id"),
                _ => String::new(),
            };
            map.insert("belongs_to".into(), json!(named));
            map.insert("after".into(), json!(after));
            others.push(entry);
        }
    }
    for o in others.iter_mut() {
        let named = field(o, "belongs_to");
        if let Some(id) = aliases.get(&named) {
            o.as_object_mut().unwrap().insert("after".into(), json!(id));
        }
    }
    for (u, cites) in units.iter_mut().zip(raw_cites) {
        let own = field(u, "id");
        let resolved = ps::select_unique(cites.iter().filter_map(|c| aliases.get(c.trim()).cloned()).filter(|c| !ps::eq(c, &own)));
        u.as_object_mut().unwrap().insert("cites".into(), json!(resolved));
    }
    (units, others)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(file: &str, group: i64, text: &str) -> Section {
        Section { file: file.into(), group, chapter: format!("第 {group} 章"), lines: text.split('\n').map(String::from).collect() }
    }

    fn book() -> Vec<Section> {
        vec![
            sec("s01.md", 1, "# 论微小量\n\n我们把 x 的一小部分记作 dx，读作“x 的微分”。\n\n例如 x=10 时 dx 可取 0.1。"),
            sec("s02.md", 2, "# 乘积\n\n两个函数乘积的导数等于第一个乘第二个的导数，加上第二个乘第一个的导数。\n\n设 u、v 增加 du、dv，展开 (u+du)(v+dv)。\n\n略去 du·dv，得结论。\n\n**定理 2.1**（商法则）商的导数。\n\n练习 III\n1. 求导。"),
        ]
    }

    #[test]
    fn a_good_answer_passes_and_builds_the_index_in_book_order() {
        let a2 = json!({"statements": [
            {"kind": "theorem", "label": "定理 2.1", "name": "商法则", "section": "s02.md", "line": 9, "end": 9, "proof": 0, "quote": "定理 2.1", "cites": ["乘积的导数"]},
            {"kind": "theorem", "label": "", "name": "乘积的导数", "section": "s02.md", "line": 3, "end": 7, "proof": 5, "quote": "两个函数乘积的导数", "cites": ["微分 dx"]},
            {"kind": "exercise", "label": "练习 III", "name": "求导练习", "section": "s02.md", "line": 11, "end": 12, "proof": 0, "quote": "练习 III", "belongs_to": "乘积的导数"}
        ]});
        let a1 = json!({"statements": [
            {"kind": "definition", "label": "", "name": "微分 dx", "section": "s01.md", "line": 3, "end": 3, "proof": 0, "quote": "我们把 x 的一小部分记作 dx"},
            {"kind": "example", "label": "", "name": "dx 的取值", "section": "s01.md", "line": 5, "end": 5, "proof": 0, "quote": "例如 x=10"}
        ]});
        let b = book();
        assert!(check_answer(&a1, &b[..1]).is_empty());
        assert!(check_answer(&a2, &b[1..]).is_empty(), "{:?}", check_answer(&a2, &b[1..]));
        let (units, others) = build(&[a2, a1], &b);
        let ids: Vec<String> = units.iter().map(|u| field(u, "id")).collect();
        assert_eq!(ids, ["定义「微分 dx」", "定理「乘积的导数」", "定理 2.1"]);
        assert_eq!(field(&units[1], "level"), "formal");
        assert_eq!(field(&units[2], "level"), "concept");
        assert_eq!(units[1]["cites"], json!(["定义「微分 dx」"]));
        assert_eq!(units[2]["cites"], json!(["定理「乘积的导数」"]));
        assert_eq!(units[0]["group"], json!(1));
        assert_eq!(field(&units[0], "chapter"), "第 1 章");
        assert_eq!(others.len(), 2);
        assert_eq!(field(&others[0], "after"), "定义「微分 dx」");
        assert_eq!(field(&others[1], "id"), "练习 III");
        assert_eq!(field(&others[1], "after"), "定理「乘积的导数」", "the named unit wins over the last unit");
    }

    #[test]
    fn a_wrong_line_kind_overlap_or_proof_is_a_fault() {
        let b = book();
        let bad = json!({"statements": [
            {"kind": "rule", "name": "x", "section": "s02.md", "line": 3, "end": 3, "quote": "两个函数"},
            {"kind": "theorem", "name": "乘积", "section": "s02.md", "line": 4, "end": 7, "proof": 0, "quote": "两个函数乘积"},
            {"kind": "theorem", "name": "乘积 2", "section": "s02.md", "line": 3, "end": 8, "proof": 9, "quote": "两个函数乘积"},
            {"kind": "lemma", "name": "y", "section": "s09.md", "line": 1, "end": 1, "quote": "abcd"},
            {"kind": "definition", "name": "z", "section": "s02.md", "line": 9, "end": 20, "quote": "定理 2.1"}
        ]});
        let f = check_answer(&bad, &b[1..]).join("\n");
        assert!(f.contains("statement 1: kind `rule`"));
        assert!(f.contains("statement 2 (乘积): quote is not on line 4"));
        assert!(f.contains("statement 3 (乘积 2): proof 9 must be 0"));
        assert!(f.contains("statement 4: section `s09.md` is not a file of this job"));
        assert!(f.contains("statement 5 (z): end 20 must be from line 9 to 12"));
        assert!(f.contains("statements 2 and 3 overlap"));
        assert_eq!(check_answer(&json!({}), &b), ["the answer has no `statements` array"]);
    }

    #[test]
    fn the_job_text_numbers_every_line_and_names_the_self_check() {
        let t = job_text("微积分", "ix-01", "D:\\o\\ix-01.json", "zt-digest math -Step index-check -Job ix-01", &book()[..1]);
        assert!(t.contains("SELF-CHECK: zt-digest math -Step index-check -Job ix-01"));
        assert!(t.contains("## 文件 `s01.md`（第 1 组，第 1 章）"));
        assert!(t.contains("3│ 我们把 x 的一小部分记作 dx"));
    }

    #[test]
    fn a_repeated_id_gets_a_number() {
        let b = book();
        let a = json!({"statements": [
            {"kind": "definition", "name": "微分", "section": "s01.md", "line": 3, "end": 3, "quote": "我们把"},
            {"kind": "definition", "name": "微分", "section": "s01.md", "line": 5, "end": 5, "quote": "例如"}
        ]});
        let (units, _) = build(&[a], &b);
        assert_eq!(units.iter().map(|u| field(u, "id")).collect::<Vec<_>>(), ["定义「微分」", "定义「微分」（2）"]);
    }
}
