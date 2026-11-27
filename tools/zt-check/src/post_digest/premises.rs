//! Hidden equation-number premise scanner (`zt-check --model <head> --premises`).
//!
//! Mirrors `skills/zt-model-restructure/scripts/check-premises.mjs` and reuses
//! `crate::formal::is_local_route_clause` directly instead of duplicating it.

use crate::formal::{FormalRole, is_local_route_clause, role_of, section};
use crate::mcp::Zt;
use crate::text::{LinkForm, link_occurrences};
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

const EQ_ID_SRC: &str = r"[A-Za-z]\.\d{1,2}[a-c]?|[A-Za-z]?\d{1,2}(?:\.\d{1,2})?[a-c]?";

static EQ_NUM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"[（(]({EQ_ID_SRC})[)）]")).expect("eq num regex"));
static DISPLAY_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)\$\$(.*?)\$\$").expect("display math regex"));
static TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"\\tag\s*\{{\s*({EQ_ID_SRC})\s*\}}")).expect("tag regex")
});
static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\*\*([^*]+)\*\*(.*)$").expect("heading regex"));
static SOURCE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*来源[：:]").expect("source regex"));
static DIRECT_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.*)\|(\d+)$").expect("direct num regex"));

#[derive(Clone, Debug, Default)]
pub struct CardText {
    pub title: String,
    pub body: String,
    pub facet: String,
    pub derivation: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PremiseHit {
    pub card: String,
    pub eq: String,
    pub derived_by: String,
    pub linked: bool,
    pub line: String,
}

impl PremiseHit {
    pub fn to_json(&self) -> Value {
        json!({
            "card": self.card,
            "eq": self.eq,
            "derived_by": self.derived_by,
            "linked": self.linked,
            "line": self.line,
        })
    }
}

fn eq_in(text: &str) -> Vec<String> {
    EQ_NUM
        .captures_iter(text)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn displays(body: &str) -> Vec<&str> {
    DISPLAY_RE
        .captures_iter(body)
        .filter_map(|c| c.get(1).map(|m| m.as_str()))
        .collect()
}

fn tags(tex: &str) -> Vec<String> {
    TAG_RE
        .captures_iter(tex)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn is_criterion_facet(facet: &str) -> bool {
    role_of(facet) == Some(FormalRole::Criterion)
}

/// Remove pointer-only sections individually (up to the next `**Heading**` — standalone or inline —
/// or `来源：`), never discarding later substantive sections. On Criterion cards, `**说明**` is a
/// forward navigation pointer (proposal §5.4).
pub fn strip_pointer_sections(body: &str, facet: &str) -> String {
    let mut skip_names = HashSet::from(["为什么需要它", "解读", "承上启下"]);
    if is_criterion_facet(facet) {
        skip_names.insert("说明");
    }
    let clean = body.replace('\r', "");
    let mut out = Vec::new();
    let mut skipping = false;
    for line in clean.split('\n') {
        if let Some(cap) = HEADING_RE.captures(line) {
            let name = cap.get(1).map_or("", |m| m.as_str().trim());
            skipping = skip_names.contains(name);
            if !skipping {
                out.push(line);
            }
            continue;
        }
        if SOURCE_RE.is_match(line) {
            skipping = false;
        }
        if !skipping {
            out.push(line);
        }
    }
    out.join("\n")
}

fn is_direct_side_child(addr: &str, parent: &str) -> bool {
    let Some(rest) = addr.strip_prefix(parent).and_then(|s| s.strip_prefix('|')) else {
        return false;
    };
    !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_lowercase())
}

fn next_direct_address(addr: &str) -> String {
    if let Some(cap) = DIRECT_NUM_RE.captures(addr) {
        let prefix = cap.get(1).map_or("", |m| m.as_str());
        let n: u64 = cap.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        format!("{prefix}|{}", n + 1)
    } else {
        format!("{addr}|1")
    }
}

fn card_label<'a>(card: &'a CardText) -> &'a str {
    if !card.facet.is_empty() {
        &card.facet
    } else {
        &card.title
    }
}

/// Exact bound proof-route addresses matching Rust `src/model/formal.rs::proof_route_addresses`:
/// - Result/Benchmark with `**依据**`: only direct Side proof roots linked via ordinary links in
///   non-external, non-inline local clauses (`is_local_route_clause`) of the first `**依据**`
///   section (`formal::section`), plus contiguous Direct proof-step continuations.
/// - Result without `**依据**`: NO bound proof route (matching core `src/model/formal.rs:487-513`).
/// - Benchmark without `**依据**`: all direct Side proof children + contiguous Direct continuations.
fn bound_proof_route(
    at: &str,
    card: &CardText,
    order: &[String],
    cards: &HashMap<String, CardText>,
) -> HashSet<String> {
    let role = role_of(card_label(card));
    let is_result = role == Some(FormalRole::Result);
    let is_benchmark = role == Some(FormalRole::Benchmark);
    if !is_result && !is_benchmark {
        return HashSet::new();
    }
    let is_proof_node = |addr: &str| -> bool {
        cards.get(addr).is_some_and(|c| {
            c.derivation || role_of(card_label(c)).is_some_and(FormalRole::is_proof_step)
        })
    };

    let mut roots: Vec<String> = Vec::new();
    if let Some(just) = section(&card.body, "依据") {
        for line in just.lines() {
            for clause in line.split(['；', ';']) {
                if !is_local_route_clause(clause) {
                    continue;
                }
                for occ in link_occurrences(clause) {
                    if occ.form == LinkForm::Ordinary
                        && is_direct_side_child(occ.target, at)
                        && is_proof_node(occ.target)
                        && !roots.iter().any(|r| r == occ.target)
                    {
                        roots.push(occ.target.to_string());
                    }
                }
            }
        }
    } else if is_benchmark {
        for addr in order {
            if is_direct_side_child(addr, at)
                && is_proof_node(addr)
                && !roots.iter().any(|r| r == addr)
            {
                roots.push(addr.clone());
            }
        }
    }

    let mut out = HashSet::new();
    for root in roots {
        let mut cur = root;
        while !out.contains(&cur) && is_proof_node(&cur) {
            let next = next_direct_address(&cur);
            out.insert(cur);
            cur = next;
        }
    }
    out
}

/// Scan a model in build order for equation numbers named before the card that derives them.
pub fn scan(
    order: &[String],
    cards: &HashMap<String, CardText>,
    relations: &[Value],
    foreign: &HashSet<String>,
) -> Vec<PremiseHit> {
    let pos: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, a)| (a.as_str(), i))
        .collect();
    let mut defs: HashMap<String, String> = HashMap::new();

    for at in order {
        if foreign.contains(at) {
            continue;
        }
        let Some(c) = cards.get(at) else {
            continue;
        };
        let mut named = eq_in(&c.title);
        for d in displays(&c.body) {
            named.extend(eq_in(d));
            named.extend(tags(d));
        }
        for n in named {
            defs.entry(n).or_insert_with(|| at.clone());
        }
    }

    let rests: HashSet<(String, String)> = relations
        .iter()
        .filter(|r| r.get("kind").and_then(Value::as_str) == Some("rests-on"))
        .filter_map(|r| {
            let from = r.get("from").and_then(Value::as_str)?;
            let to = r.get("to").and_then(Value::as_str)?;
            Some((from.to_string(), to.to_string()))
        })
        .collect();

    let mut hits = Vec::new();
    for at in order {
        if foreign.contains(at) {
            continue;
        }
        let Some(card) = cards.get(at) else {
            continue;
        };
        if card.facet.starts_with("符号") {
            continue;
        }
        let body = strip_pointer_sections(&card.body, &card.facet);
        let route = bound_proof_route(at, card, order, cards);
        let mut seen = HashSet::new();
        for n in eq_in(&body) {
            if !seen.insert(n.clone()) {
                continue;
            }
            let Some(derived) = defs.get(&n) else {
                continue;
            };
            if derived == at {
                continue;
            }
            let Some(&p_derived) = pos.get(derived.as_str()) else {
                continue;
            };
            let Some(&p_at) = pos.get(at.as_str()) else {
                continue;
            };
            if p_derived <= p_at || route.contains(derived) {
                continue;
            }
            let needle_ascii = format!("({n})");
            let needle_full = format!("（{n}）");
            let line = body
                .split('\n')
                .find(|l| l.contains(&needle_ascii) || l.contains(&needle_full))
                .unwrap_or("")
                .trim()
                .to_string();
            hits.push(PremiseHit {
                card: at.clone(),
                eq: n,
                derived_by: derived.clone(),
                linked: rests.contains(&(at.clone(), derived.clone())),
                line,
            });
        }
    }
    hits
}

pub fn format_text(head: &str, hits: &[PremiseHit]) -> Vec<String> {
    let sh = |a: &str| -> String {
        a.strip_prefix(head)
            .map(|rest| format!("H{rest}"))
            .unwrap_or_else(|| a.to_string())
    };
    let mut lines: Vec<String> = hits
        .iter()
        .map(|h| {
            let snippet: String = h.line.chars().take(140).collect();
            format!(
                "{} names ({}), derived later by {}: {}",
                sh(&h.card),
                h.eq,
                sh(&h.derived_by),
                snippet
            )
        })
        .collect();
    if hits.is_empty() {
        lines.push("premises: none used before their card".to_string());
    } else {
        lines.push(format!(
            "{} hits: read each; a use is fixed by moving cards, a pointer stays",
            hits.len()
        ));
    }
    lines
}

pub fn validate_relations(relations: &[Value], head: &str) -> Result<()> {
    for (idx, r) in relations.iter().enumerate() {
        let Some(obj) = r.as_object() else {
            bail!("get_model answer for {head} relations[{idx}] must be a JSON object");
        };
        for field in ["kind", "from", "to"] {
            if obj.get(field).and_then(Value::as_str).is_none_or(str::is_empty) {
                bail!("get_model answer for {head} relations[{idx}] lacks non-empty string `{field}`");
            }
        }
    }
    Ok(())
}

pub fn run_live(zt: &mut Zt, head: &str, json_out: bool) -> Result<(Vec<String>, u8, usize)> {
    let rev_before = zt.cards(&[], "text")?.archive_revision;
    let model = zt.call("get_model", json!({ "at": head }))?;
    let got_head = model
        .get("head")
        .and_then(|h| h.get("address"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if got_head != head {
        if got_head.is_empty() {
            bail!("{head} is not a Model Card");
        }
        bail!("{head} is not a Model Card: it belongs to the model {got_head}; give that address");
    }
    let Some(order_arr) = model["build"]["order"].as_array() else {
        bail!("get_model answer for {head} lacks `build.order` array");
    };
    let Some(parts_arr) = model.get("parts").and_then(Value::as_array) else {
        bail!("get_model answer for {head} lacks `parts` array");
    };
    let Some(relations) = model.get("relations").and_then(Value::as_array).cloned() else {
        bail!("get_model answer for {head} lacks `relations` array");
    };
    validate_relations(&relations, head)?;
    let mut order = Vec::with_capacity(order_arr.len());
    for (idx, o) in order_arr.iter().enumerate() {
        let Some(addr) = o.get("address").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} build.order[{idx}] lacks non-empty string `address`");
        };
        order.push(addr.to_string());
    }
    let foreign: HashSet<String> = order_arr
        .iter()
        .filter(|o| {
            o.get("line").and_then(Value::as_str) == Some("shared")
                && !o
                    .get("import")
                    .is_some_and(|v| !v.is_null() && v.as_bool() != Some(false) && v.as_str() != Some(""))
        })
        .filter_map(|o| o.get("address").and_then(Value::as_str).map(str::to_string))
        .collect();

    let order_derivation: HashSet<String> = order_arr
        .iter()
        .filter(|o| o.get("derivation").and_then(Value::as_bool) == Some(true))
        .filter_map(|o| o.get("address").and_then(Value::as_str).map(str::to_string))
        .collect();

    let mut cards: HashMap<String, CardText> = HashMap::new();
    for p in parts_arr {
        if let (Some(addr), Some(body)) = (
            p.get("address").and_then(Value::as_str),
            p.get("body").and_then(Value::as_str),
        ) {
            let deriv = p.get("derivation").and_then(Value::as_bool) == Some(true)
                || order_derivation.contains(addr);
            cards.insert(
                addr.to_string(),
                CardText {
                    title: p.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
                    body: body.to_string(),
                    facet: p.get("facet").and_then(Value::as_str).unwrap_or("").to_string(),
                    derivation: deriv,
                },
            );
        }
    }
    let missing_bodies: Vec<String> = order
        .iter()
        .filter(|a| !cards.contains_key(a.as_str()))
        .cloned()
        .collect();
    if !missing_bodies.is_empty() {
        let fetched = zt.cards(&missing_bodies, "text")?;
        if fetched.archive_revision != rev_before {
            bail!("incomplete checker inputs: changing read view");
        }
        for c in fetched.cards {
            let addr = c.get("address").and_then(Value::as_str).unwrap_or("").to_string();
            if addr.is_empty() {
                continue;
            }
            let deriv = order_derivation.contains(&addr);
            cards.insert(
                addr,
                CardText {
                    title: c.get("title").and_then(Value::as_str).unwrap_or("").to_string(),
                    body: c.get("body").and_then(Value::as_str).unwrap_or("").to_string(),
                    facet: c.get("facet").and_then(Value::as_str).unwrap_or("").to_string(),
                    derivation: deriv,
                },
            );
        }
    }
    let rev_after = zt.cards(&[], "text")?.archive_revision;
    if rev_after != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }
    let hits = scan(&order, &cards, &relations, &foreign);
    let fault_count = hits.len();
    let exit = if hits.is_empty() { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&json!({
            "head": head,
            "ok": hits.is_empty(),
            "hits": hits.iter().map(PremiseHit::to_json).collect::<Vec<_>>(),
        }))?]
    } else {
        format_text(head, &hits)
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn card(title: &str, body: &str) -> CardText {
        CardText { title: title.into(), body: body.into(), facet: String::new(), derivation: false }
    }

    fn card_f(title: &str, facet: &str, body: &str) -> CardText {
        CardText { title: title.into(), body: body.into(), facet: facet.into(), derivation: false }
    }

    #[test]
    fn catches_forward_equation_reference_while_exempting_pointers_and_own_proof() {
        let order: Vec<String> = ["K/s", "K/s|b", "K/2|a", "K/2|a|a", "K/3|a"]
            .into_iter().map(String::from).collect();
        let cards = HashMap::from([
            ("K/s".into(), card("设定", "一句话：设定。\n\n**内容**\n1. 由式 (2)，参数 β 可以忽略。\n\n**为什么需要它**\n[[K/3|a]] 用这里的 (9) 求工资。")),
            ("K/s|b".into(), card("设定：时序", "**内容**\n1. 三个阶段。")),
            ("K/2|a".into(), card_f("结果：任务范围 (2)", "结果", "**结果**\n$$b=1/(n+1)\\quad(2)$$\n\n**依据**\n- 局部证明：见 [[K/2|a|a]]，它推出 (4)。")),
            ("K/2|a|a".into(), card("推导：效率 (4)", "**起点**\n- 依赖 [[K/2|a]] 的 (2)。\n\n**结果**\n$$\\lambda\\quad(4)$$")),
            ("K/3|a".into(), card("推导：工资 (9)", "**起点**\n- 依赖 (4)。\n\n**结果**\n$$w\\quad(9)$$")),
        ]);
        let hits = scan(&order, &cards, &[], &HashSet::new());
        let got: Vec<_> = hits.iter().map(|h| (h.card.as_str(), h.eq.as_str(), h.derived_by.as_str())).collect();
        assert_eq!(got, vec![("K/s", "2", "K/2|a")]);
        assert!(hits[0].line.contains("由式 (2)"));
    }

    #[test]
    fn legacy_proof_heading_exempts_benchmark_only_not_result() {
        let order: Vec<String> = ["K/2|a", "K/2|a|a", "K/3|a", "K/3|a|a"]
            .into_iter()
            .map(String::from)
            .collect();
        let cards = HashMap::from([
            // Result with legacy **证明** (no **依据**): NOT exempted (matches core proof_route_addresses).
            ("K/2|a".into(), card_f("结果：命题 1", "结果", "**结果**\n结论见 (4)。\n\n**证明**\n见 [[K/2|a|a]]。")),
            ("K/2|a|a".into(), card_f("推导：引理 (4)", "推导", "**结果**\n$$x=1\\quad(4)$$")),
            // Benchmark without **依据**: direct Side proof branch IS exempted.
            ("K/3|a".into(), card_f("基准：完全信息", "基准", "**结论**\n解由 (8) 给出。\n\n**证明**\n见 [[K/3|a|a]]。")),
            ("K/3|a|a".into(), card_f("推导：基准解 (8)", "推导", "**结果**\n$$y=2\\quad(8)$$")),
        ]);
        let hits = scan(&order, &cards, &[], &HashSet::new());
        let got: Vec<_> = hits.iter().map(|h| (h.card.as_str(), h.eq.as_str(), h.derived_by.as_str())).collect();
        assert_eq!(got, vec![("K/2|a", "4", "K/2|a|a")]);
    }

    #[test]
    fn skips_notation_card_and_ignores_legacy_shares() {
        let order: Vec<String> = ["K/n", "K/9|e", "K/10|c", "K/10|b"].into_iter().map(String::from).collect();
        let cards = HashMap::from([
            ("K/n".into(), card_f("符号表", "符号表与读法", "| G | 辅助函数 (33) |")),
            ("K/9|e".into(), card("对照：完整模型", "**内容**\n$$\\Lambda=G(\\theta) \\tag{33}$$")),
            ("K/10|c".into(), card("结果：反事实", "**结果**\n用式 (33) 算出效应。")),
            ("K/10|b".into(), card("推导：矩条件", "**起点**\n- 由式 (33) 解出 K。")),
        ]);
        assert!(scan(&order, &cards, &[], &HashSet::new()).is_empty());

        let order2: Vec<String> = ["K/9|x", "K/a", "K/b"].into_iter().map(String::from).collect();
        let cards2 = HashMap::from([
            ("K/9|x".into(), card("推导：他模型的式 (2)", "**结果**\n$$y\\quad(2)$$\n\n用 (5)。")),
            ("K/a".into(), card("推导：使用", "**起点**\n- 由式 (2) 得到。")),
            ("K/b".into(), card("推导：本模型的式 (2)", "**结果**\n$$z\\quad(2)$$")),
        ]);
        let foreign = HashSet::from(["K/9|x".to_string()]);
        let hits = scan(&order2, &cards2, &[json!({"from": "K/a", "to": "K/b", "kind": "rests-on"})], &foreign);
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].card.as_str(), hits[0].eq.as_str(), hits[0].derived_by.as_str(), hits[0].linked), ("K/a", "2", "K/b", true));
    }

    #[test]
    fn formal_v2_criterion_pointer_appendix_tags_and_local_route_parity() {
        let order: Vec<String> = ["K/s", "K/s|b", "K/s|c", "K/1|a", "K/1|a|a", "K/1|a|a|1"]
            .into_iter().map(String::from).collect();
        let cards = HashMap::from([
            ("K/s".into(), card_f("设定：环境", "设定", "**起点**\n原始设定。\n\n**设定**\n先引用 (2.1) 与 (A.1)。")),
            ("K/s|b".into(), card_f("设定：次级块", "设定：次级块", "**起点**\n依赖 [[K/s]]。\n\n**设定**\n$$x = 1 \\tag{2.1}$$")),
            ("K/s|c".into(), card_f("均衡定义：竞争均衡", "均衡定义：竞争均衡", "**起点**\n依赖 [[K/s]]。\n\n**均衡定义**\n价格出清。\n\n**说明**\n存在性见后文式 (A.1)。\n\n**作用域** 在式 (B.3) 的约束下成立。")),
            ("K/1|a".into(), card_f("结果：命题 1", "结果：命题 1", "**陈述**\n满足 (A.1)。\n\n**依据**\n- 局部证明：见 [[K/1|a|a]]。")),
            ("K/1|a|a".into(), card_f("推导：附录 A", "推导：附录 A", "**起点**\n先用了后一步的 (B.3)。\n\n**结果**\n$$y = 2 \\tag{A.1}$$")),
            ("K/1|a|a|1".into(), card_f("推导：附录 B", "推导：附录 B", "**结果**\n$$z = 3 \\tag{B.3}$$")),
        ]);
        let hits = scan(&order, &cards, &[], &HashSet::new());
        let got: Vec<_> = hits.iter().map(|h| (h.card.as_str(), h.eq.as_str(), h.derived_by.as_str())).collect();
        assert_eq!(got, vec![
            ("K/s", "2.1", "K/s|b"),
            ("K/s", "A.1", "K/1|a|a"),
            ("K/s|c", "B.3", "K/1|a|a|1"),
            ("K/1|a|a", "B.3", "K/1|a|a|1"),
        ]);

        let bench_order = vec!["K/2|a".into(), "K/2|a|a".into()];
        let bench_cards = HashMap::from([
            ("K/2|a".into(), card_f("基准：无信号混同工资", "基准：无信号混同工资", "**基准**\n由式 (B.2) 给出。")),
            ("K/2|a|a".into(), card_f("推导：混同工资", "推导：混同工资", "**结果**\n$$w = 2 - q_1 \\tag{B.2}$$")),
        ]);
        assert!(scan(&bench_order, &bench_cards, &[], &HashSet::new()).is_empty());

        for (clause, want) in [
            ("见 [[K/3|a|a]]。", 1usize),
            ("- inline: 局部证明 [[K/3|a|a]]。", 1usize),
            ("- 局部例示：[[K/3|a|a]]。", 0usize),
        ] {
            let p_cards = HashMap::from([
                ("K/3|a".into(), card_f("结果：命题 3", "结果：命题 3", &format!("**陈述**\n满足 (C.1)。\n\n**依据**\n{clause}"))),
                ("K/3|a|a".into(), card_f("推导：证明", "推导：证明", "**结果**\n$$v = 1 \\tag{C.1}$$")),
            ]);
            assert_eq!(scan(&["K/3|a".into(), "K/3|a|a".into()], &p_cards, &[], &HashSet::new()).len(), want, "clause: {clause}");
        }

        // P1-2: core proof_route_addresses parity:
        // (1) derivation:true English facet is recognized as a proof node;
        // (2) repeated **依据** blocks are NOT concatenated (only first **依据** per formal::section);
        // (3) Term links [[label@K/4|a|a]] in **依据** are ignored (LinkForm::Ordinary only).
        let mut eng_proof = card_f("Derivation step (D.1)", "Derivation: step", "**结果**\n$$w = 1 \\tag{D.1}$$");
        eng_proof.derivation = true;
        let p2_order = vec!["K/4|a".into(), "K/4|a|a".into(), "K/4|a|b".into()];
        let p2_cards = HashMap::from([
            (
                "K/4|a".into(),
                card_f(
                    "结果：命题 4",
                    "结果：命题 4",
                    "**陈述**\n见 (D.1) 与 (D.2)。\n\n**依据**\n- 局部证明：见 [[K/4|a|a]] 与 [[术语@K/4|a|b]]。\n\n**说明**\n注。\n\n**依据**\n- 局部证明：见 [[K/4|a|b]]。",
                ),
            ),
            ("K/4|a|a".into(), eng_proof),
            (
                "K/4|a|b".into(),
                card_f("推导：第二块 (D.2)", "推导：第二块", "**结果**\n$$u = 2 \\tag{D.2}$$"),
            ),
        ]);
        let p2_hits = scan(&p2_order, &p2_cards, &[], &HashSet::new());
        let p2_got: Vec<_> = p2_hits.iter().map(|h| (h.card.as_str(), h.eq.as_str())).collect();
        // D.1 is exempted (via derivation:true), D.2 is NOT exempted (Term link ignored, 2nd **依据** ignored).
        assert_eq!(p2_got, vec![("K/4|a", "D.2")]);

        // M7: malformed relation entries must fail validation instead of being dropped silently.
        assert!(validate_relations(&[json!({ "kind": "rests-on", "from": "K/1", "to": "K/2" })], "K/4|a").is_ok());
        assert!(validate_relations(&[json!({ "kind": "rests-on", "from": "K/1" })], "K/4|a").is_err());
        assert!(validate_relations(&[json!("bad")], "K/4|a").is_err());
    }
}
