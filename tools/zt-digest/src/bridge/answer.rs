//! `Test-BridgeAnswer` and `Sentence-Faults`: the BR1–BR9 checks of one judge answer (or of an
//! accepted selection) against a frozen plan or published report.

use super::{ceq, ceqv, cin, field_of, get, zt_check_json};
use crate::ps::{self, ps_array, ps_count, ps_items, s, truthy};
use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::LazyLock;

pub static NOTATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(符号表|notation)").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]\r\n]+)\]\]").unwrap());
static LINE_BREAK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\r\n]").unwrap());
static NOT_PLAIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[\r\n]|(?m)^\s*#|<!--|```|~~~|(?i:<(?:figure|div|script|svg|table)\b)").unwrap());
static DRAFT_FAULT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^bridge-draft (L1|L2|R2|R3|R4|R5|R6|R7|R9)\b").unwrap());

const UNIT_FORMS: &[&str] = &["definition", "axiom", "theorem", "proposition", "lemma", "corollary"];

/// `$kindWords`: the kind word of a Bridge sentence and its Relation kind.
pub fn kind_of(word: &str) -> &'static str {
    match word {
        "对应" | "类比" => "corresponds",
        "对照" | "区别" => "contrasts",
        "例示" | "印证" | "案例" => "illustrates",
        "放宽" | "推广" => "relaxes",
        "反驳" | "批评" => "objects",
        "依赖" | "基于" => "rests-on",
        "特例" => "specializes",
        _ => "",
    }
}

/// `Unit-Head`: the Model Card of a mathematical unit.
pub fn unit_head(card: &Value) -> bool {
    ceq(get(card, "type"), "model") && !truthy(get(card, "facet")) && cin(get(card, "form"), UNIT_FORMS)
}

/// `Carrier-Allowed`.
fn carrier_allowed(card: &Value) -> bool {
    ceq(get(card, "workspace"), "agent")
        && ceq(get(card, "kind"), "regular")
        && cin(get(card, "category"), &["idea", "event", "finding", "model"])
        && !truthy(get(card, "facet"))
        && !ceq(get(card, "form"), "theory")
        && !NOTATION.is_match(&s(get(card, "title")))
}

/// What `Test-BridgeAnswer` returns.
pub struct Checked {
    pub faults: Vec<String>,
    pub proposals: Vec<Value>,
    pub none: Vec<Value>,
}

fn contains(list: &[Value], value: &str) -> bool {
    list.iter().any(|item| ceq(item, value))
}

/// `Sentence-Faults`: the Card checks of the Bridge sentence as a draft Card (with the term
/// table of the carrier's model setting, else of the carrier's book), as `bridge-draft` fault
/// lines. Sets `term_check` on the Bridge.
pub fn sentence_faults(work: &str, bridge: &mut Map<String, Value>, plan: &Value) -> Result<Vec<String>> {
    let sentence = bridge.get("sentence").cloned().unwrap_or(Value::Null);
    let target = bridge.get("target").cloned().unwrap_or(Value::Null);
    let mut entries = vec![json!({
        "address": "bridge-draft", "title": "跨书关联", "body": sentence, "category": "idea", "parent": "",
        "links": [target.clone()], "kinds": [target]
    })];
    let cards = get(plan, "cards");
    let carrier = cards.get(s(bridge.get("carrier").unwrap_or(&Value::Null))).unwrap_or(&Value::Null);
    let has_terms = |at: &str| !at.is_empty() && cards.get(at).is_some_and(|c| s(get(c, "body")).contains("术语表"));
    let mut at = s(&carrier["model"]["build"]["setting"]);
    if !has_terms(&at) {
        at = String::new();
    }
    let book = get(carrier, "book");
    if at.is_empty() && truthy(book) && get(plan, "term_tables").get(s(book)).is_some() {
        at = s(&plan["term_tables"][s(book)]);
    }
    let mut setting = String::new();
    if has_terms(&at) {
        setting = at.clone();
        let card = &cards[&at];
        entries.push(json!({ "address": at, "title": get(card, "title"), "body": get(card, "body"), "category": "terms" }));
    }
    bridge.insert("term_check".into(), json!(if setting.is_empty() { "unavailable" } else { "passed" }));
    let draft = ps::join_path(work, &format!("tmp\\precheck-draft-{}.json", std::process::id()));
    std::fs::create_dir_all(ps::parent(&draft))?;
    super::canon::save_json(&draft, &Value::Array(entries))?;
    let lines = zt_check_json(&draft, &setting)?;
    Ok(lines.into_iter().filter(|l| DRAFT_FAULT.is_match(l)).collect())
}

/// `Test-BridgeAnswer`: the faults of one answer, and its proposals and none reasons when its
/// envelope holds.
pub fn test_answer(work: &str, answer: &Value, plan: &Value, assignment: &Value) -> Result<Checked> {
    let mut faults: Vec<String> = Vec::new();
    let mut proposals: Vec<Map<String, Value>> = Vec::new();
    let mut none: Vec<Value> = Vec::new();
    let envelope = match answer.as_object() {
        None => false,
        Some(m) => {
            ["schema", "plan_id", "job", "sources"].iter().all(|k| m.contains_key(*k))
                && (m["schema"].is_i64() || m["schema"].is_u64())
                && m["schema"].as_i64() == Some(1)
                && ceqv(&m["plan_id"], get(plan, "plan_id"))
                && ceqv(&m["job"], get(assignment, "id"))
                && m["sources"].is_array()
        }
    };
    if !envelope {
        return Ok(Checked { faults: vec!["BR1 invalid answer envelope, schema, plan, or job".into()], proposals: Vec::new(), none: Vec::new() });
    }
    let assigned = ps_array(get(assignment, "sources"));
    let cards = get(plan, "cards");
    let key = get(plan, "citation_key");
    let books = ps_array(&plan["options"]["books"]);
    let mut seen: HashSet<String> = HashSet::new();
    let mut pairs: HashSet<String> = HashSet::new();
    let mut envelope_ok = true;
    for result in ps_items(&answer["sources"]) {
        let valid = match result.as_object() {
            None => false,
            Some(m) => {
                ["source", "bridges", "none"].iter().all(|k| m.contains_key(*k))
                    && m["source"].is_string()
                    && assigned.iter().any(|a| ceqv(&m["source"], a))
                    && seen.insert(s(&m["source"]))
                    && m["bridges"].is_array()
                    && m["none"].is_string()
            }
        };
        if !valid {
            faults.push("BR1 invalid, unassigned, or repeated source result".into());
            envelope_ok = false;
            continue;
        }
        let source = s(&result["source"]);
        let reason = s(&result["none"]);
        let bridges = ps_items(&result["bridges"]);
        if bridges.is_empty() {
            if reason.trim().is_empty() || LINE_BREAK.is_match(&reason) {
                faults.push(format!("BR8 {source} needs a one-line none reason"));
            } else {
                none.push(json!({ "source": source, "why": reason }));
            }
            continue;
        }
        if !reason.is_empty() || bridges.len() > 2 {
            faults.push(format!("BR6 {source} needs one or two Bridges and an empty none"));
            continue;
        }
        let source_plan = ps_items(get(plan, "sources")).into_iter().find(|p| ceq(get(p, "address"), &source)).unwrap_or(Value::Null);
        let targets: Vec<Value> = ps_items(get(&source_plan, "candidates")).iter().map(|c| get(c, "address").clone()).collect();
        for answer_bridge in bridges {
            let start = faults.len();
            let Some(m) = answer_bridge.as_object() else {
                faults.push(format!("BR1 {source} Bridge is not an object"));
                continue;
            };
            let mut fields_ok = true;
            for name in ["carrier", "target", "word", "sentence", "why"] {
                if !matches!(m.get(name), Some(Value::String(t)) if !t.trim().is_empty()) {
                    faults.push(format!("BR8 {source} invalid {name}"));
                    fields_ok = false;
                }
            }
            if !fields_ok {
                continue;
            }
            let text = |name: &str| s(&m[name]);
            let (carrier_at, target_at, word, sentence, why) = (text("carrier"), text("target"), text("word"), text("sentence"), text("why"));
            let mut bridge = Map::new();
            for (k, v) in [("source", &source), ("carrier", &carrier_at), ("target", &target_at), ("word", &word), ("sentence", &sentence), ("why", &why)] {
                bridge.insert(k.into(), json!(v));
            }
            let new = carrier_at == source && contains(&targets, &target_at);
            let earlier = target_at == source && contains(&targets, &carrier_at);
            if !new && !earlier {
                faults.push(format!("BR2 {source} pair is not a supplied candidate in either direction"));
            }
            let (Some(carrier), Some(target)) = (cards.get(&carrier_at), cards.get(&target_at)) else {
                faults.push(format!("BR3 {source} endpoint snapshot missing"));
                continue;
            };
            if !carrier_allowed(carrier)
                || !cin(get(target, "category"), &["idea", "event", "finding", "model", "part"])
                || !ceq(get(target, "kind"), "regular")
                || ceq(get(target, "form"), "theory")
                || NOTATION.is_match(&s(get(target, "title")))
            {
                faults.push(format!("BR3 {source} ineligible carrier or target"));
            }
            let source_card = cards.get(&source).unwrap_or(&Value::Null);
            if !ceq(get(source_card, "workspace"), "agent") || !ceqv(get(source_card, "book"), key) || (new && ceqv(get(target, "book"), key)) {
                faults.push(format!("BR3 {source} endpoint is not outside the selected source book"));
            }
            let other = if new { target } else { carrier };
            if truthy(get(other, "book")) && ps_count(&plan["options"]["books"]) > 0 && !books.iter().any(|b| ceqv(get(other, "book"), b)) {
                faults.push(format!("BR3 {source} endpoint is outside the target-book filter"));
            }
            bridge.insert("section".into(), json!(if new { "new" } else { "earlier" }));
            let kind = kind_of(&word);
            bridge.insert("kind".into(), json!(kind));
            if kind.is_empty() {
                faults.push(format!("BR4 {source} unknown kind word"));
            }
            let carrier_model = ceq(get(carrier, "type"), "model");
            if carrier_model && truthy(get(target, "facet")) {
                faults.push(format!("BR4 {source} a Model Card cannot take in a Model part through a Bridge"));
            }
            if carrier_model && (word == "印证" || word == "案例") {
                faults.push(format!("BR4 {source} a Model Card carrier takes no evidence word (印证, 案例)"));
            }
            if kind == "rests-on" && (truthy(get(target, "facet")) || (carrier_model && !unit_head(target))) {
                faults.push(format!("BR4 {source} invalid rests-on boundary"));
            }
            if kind == "specializes" && (!ceq(get(carrier, "form"), "definition") || !ceq(get(target, "form"), "definition")) {
                faults.push(format!("BR4 {source} specialization requires two Definition heads"));
            }
            if earlier
                && (!unit_head(target)
                    || !cin(get(carrier, "type"), &["idea", "model"])
                    || !truthy(get(carrier, "book"))
                    || ceqv(get(carrier, "book"), key)
                    || !(word == "依赖" || word == "基于"))
            {
                faults.push(format!("BR4 {source} invalid earlier-carrier exception"));
            }
            let links: Vec<_> = LINK.captures_iter(&sentence).collect();
            let placed = Regex::new(&format!(r"{}\s*\[\[{}\]\]", regex::escape(&word), regex::escape(&target_at))).map(|r| r.is_match(&sentence)).unwrap_or(false);
            if links.len() != 1 || links[0][1] != *target_at || !placed {
                faults.push(format!("BR5 {source} needs exactly one correctly targeted Link immediately after its kind word"));
            }
            let pair = format!("{carrier_at}\0{target_at}");
            if pairs.contains(&format!("{target_at}\0{carrier_at}")) {
                faults.push(format!("BR6 {source} the reverse pair is already proposed: one relation per pair of Cards"));
            }
            if !pairs.insert(pair) {
                faults.push(format!("BR6 {source} repeated directed pair"));
                // A conflicting second sentence cannot silently leave the first executable.
                let mut i = proposals.len();
                while i > 0 {
                    i -= 1;
                    let prior = Value::Object(proposals[i].clone());
                    if field_of(&prior, "carrier") == carrier_at
                        && field_of(&prior, "target") == target_at
                        && (field_of(&prior, "word") != word || field_of(&prior, "sentence") != sentence)
                    {
                        proposals.remove(i);
                    }
                }
            }
            if contains(&ps_array(get(carrier, "links")), &target_at) || contains(&ps_array(get(target, "links")), &carrier_at) {
                faults.push(format!("BR7 {source} pair is already linked"));
            }
            if NOT_PLAIN.is_match(&sentence) || NOT_PLAIN.is_match(&why) {
                faults.push(format!("BR8 {source} sentence and why must be plain single lines"));
            }
            if faults.len() == start {
                for fault in sentence_faults(work, &mut bridge, plan)? {
                    faults.push(format!("BR9 {source} {fault}"));
                }
            }
            if faults.len() == start {
                proposals.push(bridge);
            }
        }
    }
    if seen.len() != ps_count(get(assignment, "sources")) {
        faults.push("BR1 every assigned source must appear exactly once".into());
        envelope_ok = false;
    }
    Ok(Checked {
        faults,
        proposals: if envelope_ok { proposals.into_iter().map(Value::Object).collect() } else { Vec::new() },
        none: if envelope_ok { none } else { Vec::new() },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(at: &str, book: &str, kind_type: &str, category: &str, form: &str, facet: &str) -> Value {
        json!({ "address": at, "workspace": "agent", "kind": "regular", "type": kind_type, "category": category, "book": book,
                "form": form, "facet": facet, "title": "可独立理解的卡片", "body": "卡片正文。", "links": [] })
    }

    fn plan() -> Value {
        let mut cards = Map::new();
        cards.insert("0/1".into(), card("0/1", "New2026", "idea", "idea", "", ""));
        cards.insert("0/2".into(), card("0/2", "New2026", "model", "model", "", ""));
        cards.insert("Old/1|a".into(), card("Old/1|a", "Old", "idea", "idea", "", ""));
        cards.insert("Old/1|c".into(), card("Old/1|c", "Old", "model", "part", "", "推导"));
        json!({ "plan_id": "p-1", "citation_key": "New2026", "options": { "books": [] }, "cards": cards, "term_tables": {},
                "sources": [{ "address": "0/1", "candidates": [{ "address": "Old/1|a" }, { "address": "Old/1|c" }] },
                            { "address": "0/2", "candidates": [{ "address": "Old/1|c" }] }] })
    }

    fn one(source: &str, carrier: &str, target: &str, word: &str, sentence: &str) -> Value {
        json!({ "schema": 1, "plan_id": "p-1", "job": "br-001", "sources": [{ "source": source, "none": "",
                "bridges": [{ "carrier": carrier, "target": target, "word": word, "sentence": sentence, "why": "两张卡讨论相同的机制。" }] }] })
    }

    fn faults(answer: &Value, sources: &[&str]) -> Vec<String> {
        let assignment = json!({ "id": "br-001", "sources": sources });
        test_answer("unused", answer, &plan(), &assignment).unwrap().faults
    }

    #[test]
    fn envelopes_and_rules_before_the_card_check() {
        assert_eq!(faults(&json!({ "schema": 1.0, "plan_id": "p-1", "job": "br-001", "sources": [] }), &["0/1"]), vec!["BR1 invalid answer envelope, schema, plan, or job"]);
        assert_eq!(faults(&json!({ "schema": 1, "plan_id": "p-1", "job": "br-001", "sources": [] }), &["0/1"]), vec!["BR1 every assigned source must appear exactly once"]);
        let no_reason = json!({ "schema": 1, "plan_id": "p-1", "job": "br-001", "sources": [{ "source": "0/1", "bridges": [], "none": " " }] });
        assert_eq!(faults(&no_reason, &["0/1"]), vec!["BR8 0/1 needs a one-line none reason"]);
        let f = faults(&one("0/1", "0/1", "Old/1|c", "依赖", "依赖 [[Old/1|c]]：理由。"), &["0/1"]);
        assert_eq!(f, vec!["BR4 0/1 invalid rests-on boundary"]);
        let f = faults(&one("0/2", "0/2", "Old/1|c", "对应", "对应 [[Old/1|c]]：理由。"), &["0/2"]);
        assert_eq!(f, vec!["BR4 0/2 a Model Card cannot take in a Model part through a Bridge"]);
        let f = faults(&one("0/1", "0/1", "Old/1|a", "引出", "对应 [[Old/1|a]] [[0/2]]"), &["0/1"]);
        assert_eq!(
            f,
            vec![
                "BR4 0/1 unknown kind word",
                "BR5 0/1 needs exactly one correctly targeted Link immediately after its kind word"
            ]
        );
        let f = faults(&one("0/1", "0/1", "9/9", "对应", "对应 [[9/9]]：理由。"), &["0/1"]);
        assert_eq!(f, vec!["BR2 0/1 pair is not a supplied candidate in either direction", "BR3 0/1 endpoint snapshot missing"]);
        let f = faults(&one("0/1", "0/1", "Old/1|a", "对应", "对应 [[Old/1|a]]：<div>理由</div>"), &["0/1"]);
        assert_eq!(f, vec!["BR8 0/1 sentence and why must be plain single lines"]);
    }
}
