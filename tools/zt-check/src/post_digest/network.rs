//! Model network audit (`zt-check --model <head> --network [--format json]`).
//!
//! Pure Rust port of `skills/zt-model-restructure/scripts/check-network.mjs` and the three
//! pure layout functions (`blockTree`, `blockLayout`, `treeLayout`) from `src/gui/layout.js`,
//! removing any HTTP dependency on `/layout.js`.

use crate::mcp::Zt;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;

const NODE_W: f64 = 124.0;
const NODE_H: f64 = 66.0;
const LEAD_MIN: usize = 20;

static CHAPTER_NUM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^第\s*(\d+)\s*章$").expect("chapter num regex"));

#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetworkReport {
    pub parent: BTreeMap<String, Option<String>>,
    pub lines: Vec<String>,
    pub forward: Vec<Value>,
    pub moves: Vec<Value>,
    pub unanchored: Vec<Value>,
    pub parts: Vec<Value>,
    pub leads: Vec<Value>,
    pub empty: Vec<Value>,
    pub formal: Vec<Value>,
    pub shape: Value,
    pub threads: Value,
    pub ok: bool,
}

impl NetworkReport {
    pub fn to_json(&self) -> Value {
        json!({
            "parent": self.parent,
            "lines": self.lines,
            "forward": self.forward,
            "moves": self.moves,
            "unanchored": self.unanchored,
            "parts": self.parts,
            "leads": self.leads,
            "empty": self.empty,
            "formal": self.formal,
            "shape": self.shape,
            "threads": self.threads,
            "ok": self.ok,
        })
    }
}

/// Pure Rust port of `src/gui/layout.js::blockTree`.
struct BlockTree {
    parent: BTreeMap<String, Option<String>>,
    children: HashMap<String, Vec<String>>,
    roots: Vec<String>,
}

fn block_tree(blocks: &[Value], edges: &[Value]) -> BlockTree {
    let at: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.get("id").and_then(Value::as_str).map(|id| (id, i)))
        .collect();

    // Best earlier block rested on most (ties go to the later `to_block`).
    let mut best: HashMap<&str, (&str, i64, usize)> = HashMap::new();
    for e in edges {
        if e.get("kind").and_then(Value::as_str) != Some("rests-on") {
            continue;
        }
        let Some(from) = e.get("from_block").and_then(Value::as_str) else {
            continue;
        };
        let Some(to) = e.get("to_block").and_then(Value::as_str) else {
            continue;
        };
        let (Some(&fi), Some(&ti)) = (at.get(from), at.get(to)) else {
            continue;
        };
        if ti >= fi {
            continue;
        }
        let count = e.get("count").and_then(Value::as_i64).unwrap_or(0);
        let replace = match best.get(from) {
            None => true,
            Some(&(_, prev_count, prev_ti)) => {
                count > prev_count || (count == prev_count && ti > prev_ti)
            }
        };
        if replace {
            best.insert(from, (to, count, ti));
        }
    }

    let mut parent: BTreeMap<String, Option<String>> = BTreeMap::new();
    let mut children: HashMap<String, Vec<String>> = HashMap::new();
    let mut roots: Vec<String> = Vec::new();

    for b in blocks {
        let Some(id) = b.get("id").and_then(Value::as_str) else {
            continue;
        };
        if let Some(&(to, _, _)) = best.get(id) {
            parent.insert(id.to_string(), Some(to.to_string()));
            children
                .entry(to.to_string())
                .or_default()
                .push(id.to_string());
        } else {
            parent.insert(id.to_string(), None);
            roots.push(id.to_string());
        }
    }
    BlockTree {
        parent,
        children,
        roots,
    }
}

/// Pure Rust port of `src/gui/layout.js::blockLayout` (focus=null, backbone=true).
struct BlockLayoutSummary {
    ribbons_len: usize,
    unanchored: Vec<String>,
}

fn block_layout_backbone(blocks: &[Value], edges: &[Value]) -> BlockLayoutSummary {
    let at: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.get("id").and_then(Value::as_str).map(|id| (id, i)))
        .collect();

    let shown: Vec<&Value> = edges
        .iter()
        .filter(|e| {
            let from = e.get("from_block").and_then(Value::as_str).unwrap_or("");
            let to = e.get("to_block").and_then(Value::as_str).unwrap_or("");
            let kind = e.get("kind").and_then(Value::as_str).unwrap_or("");
            at.contains_key(from) && at.contains_key(to) && kind != "refers"
        })
        .collect();

    let is_up = |e: &Value| -> bool {
        if e.get("kind").and_then(Value::as_str) != Some("rests-on") {
            return false;
        }
        let from = e.get("from_block").and_then(Value::as_str).unwrap_or("");
        let to = e.get("to_block").and_then(Value::as_str).unwrap_or("");
        match (at.get(from), at.get(to)) {
            (Some(&fi), Some(&ti)) => fi < ti,
            _ => false,
        }
    };

    // Best downward rests-on edge per `from_block` (by index into `shown`).
    let mut best_idx: HashMap<&str, (usize, i64, usize)> = HashMap::new();
    for (idx, e) in shown.iter().enumerate() {
        if e.get("kind").and_then(Value::as_str) != Some("rests-on") || is_up(e) {
            continue;
        }
        let from = e.get("from_block").and_then(Value::as_str).unwrap_or("");
        let to = e.get("to_block").and_then(Value::as_str).unwrap_or("");
        let Some(&ti) = at.get(to) else {
            continue;
        };
        let count = e.get("count").and_then(Value::as_i64).unwrap_or(0);
        let replace = match best_idx.get(from) {
            None => true,
            Some(&(_, prev_count, prev_ti)) => {
                count > prev_count || (count == prev_count && ti > prev_ti)
            }
        };
        if replace {
            best_idx.insert(from, (idx, count, ti));
        }
    }
    let keep: HashSet<usize> = best_idx.values().map(|&(idx, _, _)| idx).collect();
    let ribbons_len = shown
        .iter()
        .enumerate()
        .filter(|(idx, e)| keep.contains(idx) || is_up(e))
        .count();

    let mut anchored: HashSet<&str> = HashSet::new();
    for e in edges {
        if e.get("kind").and_then(Value::as_str) != Some("rests-on") {
            continue;
        }
        let from = e.get("from_block").and_then(Value::as_str).unwrap_or("");
        let to = e.get("to_block").and_then(Value::as_str).unwrap_or("");
        if let (Some(&fi), Some(&ti)) = (at.get(from), at.get(to))
            && ti < fi
            && from != to
        {
            anchored.insert(from);
        }
    }
    let unanchored = blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| {
            let id = b.get("id").and_then(Value::as_str)?;
            let kind = b.get("kind").and_then(Value::as_str).unwrap_or("");
            if i > 0 && kind == "chapter" && !anchored.contains(id) {
                Some(id.to_string())
            } else {
                None
            }
        })
        .collect();

    BlockLayoutSummary {
        ribbons_len,
        unanchored,
    }
}

/// Pure Rust port of `src/gui/layout.js::treeLayout` (computing max depth and contour width).
struct TreeLayoutMetrics {
    depth: usize,
    width: i64,
}

struct SubContour {
    left: Vec<f64>,
    right: Vec<f64>,
    cx: f64,
    max_depth: usize,
}

fn tree_layout_metrics(
    roots: &[String],
    children: &HashMap<String, Vec<String>>,
    gap_x: f64,
) -> TreeLayoutMetrics {
    fn sub(
        id: &str,
        d: usize,
        children: &HashMap<String, Vec<String>>,
        gap_x: f64,
    ) -> SubContour {
        let kids = children.get(id).map(Vec::as_slice).unwrap_or(&[]);
        if kids.is_empty() {
            return SubContour {
                left: vec![0.0],
                right: vec![NODE_W],
                cx: NODE_W / 2.0,
                max_depth: d,
            };
        }
        let mut sub_list = Vec::with_capacity(kids.len());
        let mut max_depth = d;
        for c in kids {
            let sc = sub(c, d + 1, children, gap_x);
            max_depth = max_depth.max(sc.max_depth);
            sub_list.push(sc);
        }
        let mut left: Vec<f64> = Vec::new();
        let mut right: Vec<f64> = Vec::new();
        let mut offsets: Vec<f64> = Vec::with_capacity(sub_list.len());
        for (i, t) in sub_list.iter().enumerate() {
            let mut off = 0.0_f64;
            if i > 0 {
                let overlap = right.len().min(t.left.len());
                for k in 0..overlap {
                    off = off.max(right[k] + gap_x - t.left[k]);
                }
            }
            offsets.push(off);
            for k in 0..t.left.len() {
                if k < left.len() {
                    left[k] = left[k].min(t.left[k] + off);
                    right[k] = right[k].max(t.right[k] + off);
                } else {
                    left.push(t.left[k] + off);
                    right.push(t.right[k] + off);
                }
            }
        }
        let cx = (sub_list[0].cx + offsets[0]
            + sub_list[sub_list.len() - 1].cx
            + offsets[offsets.len() - 1])
            / 2.0;
        let x_self = cx - NODE_W / 2.0;
        let mut full_left = Vec::with_capacity(left.len() + 1);
        full_left.push(x_self);
        full_left.extend(left);
        let mut full_right = Vec::with_capacity(right.len() + 1);
        full_right.push(x_self + NODE_W);
        full_right.extend(right);
        let min_x = full_left
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let mut cx_norm = cx;
        if min_x != 0.0 && min_x.is_finite() {
            for v in &mut full_left {
                *v -= min_x;
            }
            for v in &mut full_right {
                *v -= min_x;
            }
            cx_norm -= min_x;
        }
        SubContour {
            left: full_left,
            right: full_right,
            cx: cx_norm,
            max_depth,
        }
    }

    let _ = NODE_H;
    let mut forest_left: Vec<f64> = Vec::new();
    let mut forest_right: Vec<f64> = Vec::new();
    let mut max_depth = 0usize;
    let mut has_any = false;
    for (i, r) in roots.iter().enumerate() {
        has_any = true;
        let t = sub(r, 0, children, gap_x);
        max_depth = max_depth.max(t.max_depth);
        let mut off = 0.0_f64;
        if i > 0 {
            let overlap = forest_right.len().min(t.left.len());
            for k in 0..overlap {
                off = off.max(forest_right[k] + gap_x * 2.0 - t.left[k]);
            }
        }
        for k in 0..t.left.len() {
            if k < forest_left.len() {
                forest_left[k] = forest_left[k].min(t.left[k] + off);
                forest_right[k] = forest_right[k].max(t.right[k] + off);
            } else {
                forest_left.push(t.left[k] + off);
                forest_right.push(t.right[k] + off);
            }
        }
    }
    let width = if forest_right.is_empty() {
        0
    } else {
        let max_r = forest_right.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let min_l = forest_left.iter().copied().fold(0.0_f64, f64::min);
        (max_r - min_l).round() as i64
    };
    let depth = if has_any { max_depth + 1 } else { 1 };
    TreeLayoutMetrics { depth, width }
}

fn is_result_child(c: &Value) -> bool {
    let facet = c.get("facet").and_then(Value::as_str).unwrap_or("");
    facet.starts_with("结果")
        || facet.starts_with("基准")
        || facet.starts_with("校准")
        || facet.starts_with("数值实验")
}

fn block_name(b: Option<&Value>, fallback: &str) -> String {
    let Some(b) = b else {
        return fallback.to_string();
    };
    let kind = b.get("kind").and_then(Value::as_str).unwrap_or("");
    if kind == "aside" {
        return "旁注".to_string();
    }
    if kind == "shared" {
        return "共用".to_string();
    }
    let facet = b.get("facet").and_then(Value::as_str).unwrap_or("");
    let label = facet.split(['：', ':']).next().unwrap_or("").trim();
    if let Some(cap) = CHAPTER_NUM_RE.captures(label) {
        return cap.get(1).map_or("", |m| m.as_str()).to_string();
    }
    if !label.is_empty() {
        label.to_string()
    } else {
        b.get("head")
            .and_then(Value::as_str)
            .unwrap_or(fallback)
            .to_string()
    }
}

/// Analyse one `get_model` response as the GUI's model network draws it.
pub fn analyse(m: &Value) -> NetworkReport {
    let blocks = m
        .get("blocks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let edges = m
        .get("block_edges")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    let level = m.get("level").and_then(Value::as_str).unwrap_or("");
    let has_form = m
        .get("form")
        .is_some_and(|v| !v.is_null() && v.as_str() != Some(""));

    let at: HashMap<&str, usize> = blocks
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.get("id").and_then(Value::as_str).map(|id| (id, i)))
        .collect();
    let by_id: HashMap<&str, &Value> = blocks
        .iter()
        .filter_map(|b| b.get("id").and_then(Value::as_str).map(|id| (id, b)))
        .collect();

    let mut facet_of: HashMap<&str, &str> = HashMap::new();
    for o in m["build"]["order"].as_array().into_iter().flatten() {
        if let (Some(addr), Some(facet)) = (
            o.get("address").and_then(Value::as_str),
            o.get("facet").and_then(Value::as_str),
        ) {
            facet_of.insert(addr, facet);
        }
    }
    for b in blocks {
        if let (Some(head), Some(facet)) = (
            b.get("head").and_then(Value::as_str),
            b.get("facet").and_then(Value::as_str),
        ) {
            facet_of.insert(head, facet);
        }
        for c in b["children"].as_array().into_iter().flatten() {
            if let (Some(addr), Some(facet)) = (
                c.get("address").and_then(Value::as_str),
                c.get("facet").and_then(Value::as_str),
            ) {
                facet_of.insert(addr, facet);
            }
        }
    }

    let tree = block_tree(blocks, edges);
    let layout = block_layout_backbone(blocks, edges);

    // Forward edges: rests-on from an earlier block to a later block.
    let mut forward = Vec::new();
    let mut forward_blocks: HashSet<String> = HashSet::new();
    let mut moves: Vec<Value> = Vec::new();
    let mut seen_move_cards: HashSet<String> = HashSet::new();

    for e in edges {
        if e.get("kind").and_then(Value::as_str) != Some("rests-on") {
            continue;
        }
        let from_block = e.get("from_block").and_then(Value::as_str).unwrap_or("");
        let to_block = e.get("to_block").and_then(Value::as_str).unwrap_or("");
        let (Some(&fi), Some(&ti)) = (at.get(from_block), at.get(to_block)) else {
            continue;
        };
        if !(fi < ti) {
            continue;
        }
        forward_blocks.insert(from_block.to_string());
        let pairs = e.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
        let mut cards: Vec<String> = Vec::new();
        for p in &pairs {
            if let Some(from) = p.get("from").and_then(Value::as_str)
                && !cards.iter().any(|c| c == from)
            {
                cards.push(from.to_string());
            }
        }
        let in_setting_block = by_id
            .get(from_block)
            .and_then(|b| b.get("kind"))
            .and_then(Value::as_str)
            == Some("setting");

        let mut fixes: Vec<String> = Vec::new();
        for card in &cards {
            let mut premises: Vec<&str> = Vec::new();
            for x in edges {
                if x.get("kind").and_then(Value::as_str) != Some("rests-on")
                    || x.get("from_block").and_then(Value::as_str) != Some(from_block)
                {
                    continue;
                }
                let x_to = x.get("to_block").and_then(Value::as_str).unwrap_or("");
                for p in x.get("pairs").and_then(Value::as_array).into_iter().flatten() {
                    if p.get("from").and_then(Value::as_str) == Some(card.as_str()) {
                        premises.push(x_to);
                    }
                }
            }
            premises.sort_by(|a, b| {
                let ia = at.get(a).copied().unwrap_or(0);
                let ib = at.get(b).copied().unwrap_or(0);
                ib.cmp(&ia)
            });
            let latest = premises.first().copied().unwrap_or(to_block);
            let latest_head = by_id
                .get(latest)
                .and_then(|b| b.get("head"))
                .and_then(Value::as_str)
                .unwrap_or(latest);
            let latest_name = block_name(by_id.get(latest).copied(), latest);
            let card_facet = facet_of.get(card.as_str()).copied().unwrap_or("");
            let is_maintained_primitive = card_facet.starts_with("设定")
                || card_facet.starts_with("假设")
                || card_facet.starts_with("假定")
                || (in_setting_block
                    && (card_facet.starts_with("定义")
                        || card_facet.starts_with("判据")
                        || card_facet.starts_with("均衡定义")
                        || card_facet.starts_with("解概念")));

            let fix = if is_maintained_primitive {
                format!(
                    "{card} is a setting: a setting rests on nothing derived later, so this relation is reversed. If the later card (in {latest_head}, chapter {latest_name}) works with the setting's variable or definition, let it rest on {card} and let the setting only point to it; keep it in the setting part. A derived fact the setting states moves to the card that derives it"
                )
            } else {
                format!(
                    "move {card} after its latest premise: file it under {latest_head} (chapter {latest_name}) or later, with every card that rests on it"
                )
            };
            if seen_move_cards.insert(card.clone()) {
                moves.push(json!({ "card": card, "fix": fix }));
            }
            fixes.push(fix);
        }

        let joined_fix = if fixes.is_empty() {
            format!(
                "block {} rests on the later block {}",
                block_name(by_id.get(from_block).copied(), from_block),
                block_name(by_id.get(to_block).copied(), to_block)
            )
        } else {
            fixes.join(" | ")
        };
        forward.push(json!({
            "from_block": from_block,
            "to_block": to_block,
            "count": e.get("count").and_then(Value::as_i64).unwrap_or(0),
            "cards": cards,
            "pairs": pairs,
            "fixes": fixes,
            "fix": joined_fix,
        }));
    }

    let is_primitive = |id: &str| -> bool {
        level == "empirical"
            && by_id
                .get(id)
                .and_then(|b| b.get("facet"))
                .and_then(Value::as_str)
                .is_some_and(|f| f.starts_with("数据"))
    };

    let mut unanchored_ids: HashSet<String> = HashSet::new();
    let unanchored: Vec<Value> = layout
        .unanchored
        .into_iter()
        .filter(|id| !is_primitive(id))
        .map(|id| {
            unanchored_ids.insert(id.clone());
            let head = by_id
                .get(id.as_str())
                .and_then(|b| b.get("head"))
                .and_then(Value::as_str)
                .unwrap_or(&id);
            let nm = block_name(by_id.get(id.as_str()).copied(), &id);
            json!({
                "block": id,
                "fix": format!("chapter {nm} rests on no earlier block: add a 依赖 line from {head} to the chapter it builds on"),
            })
        })
        .collect();

    let chapters: Vec<&Value> = blocks
        .iter()
        .filter(|b| b.get("kind").and_then(Value::as_str) == Some("chapter"))
        .collect();

    let mut leads = Vec::new();
    for b in &chapters {
        let id = b.get("id").and_then(Value::as_str).unwrap_or("");
        let head = b.get("head").and_then(Value::as_str).unwrap_or(id);
        let lead = b.get("lead").and_then(Value::as_str).unwrap_or("");
        if lead.is_empty() {
            leads.push(json!({
                "block": id,
                "why": "missing",
                "fix": format!("give {head} a **直觉** section: its first sentence opens the block"),
            }));
        } else if lead.chars().count() < LEAD_MIN {
            leads.push(json!({
                "block": id,
                "why": "short",
                "lead": lead,
                "fix": format!("the 直觉 of {head} opens with \"{lead}\"; state the mechanism in its first sentence ({LEAD_MIN}+ characters)"),
            }));
        }
    }

    let mut empty_ids: HashSet<String> = HashSet::new();
    let empty: Vec<Value> = if level == "empirical" {
        Vec::new()
    } else {
        chapters
            .iter()
            .filter(|b| {
                !b["children"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(is_result_child)
            })
            .map(|b| {
                let id = b.get("id").and_then(Value::as_str).unwrap_or("");
                empty_ids.insert(id.to_string());
                let nm = block_name(Some(b), id);
                json!({
                    "block": id,
                    "fix": format!("chapter {nm} has no result card (facet 结果：…): its block shows no result"),
                })
            })
            .collect()
    };

    let parts = m["build"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();

    let metrics = tree_layout_metrics(&tree.roots, &tree.children, 14.0);
    let mut lines = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn walk(
        id: &str,
        d: usize,
        by_id: &HashMap<&str, &Value>,
        children: &HashMap<String, Vec<String>>,
        forward_blocks: &HashSet<String>,
        unanchored_ids: &HashSet<String>,
        empty_ids: &HashSet<String>,
        lines: &mut Vec<String>,
    ) {
        if let Some(&b) = by_id.get(id) {
            let mut marks = Vec::new();
            if forward_blocks.contains(id) {
                marks.push("FORWARD");
            }
            if unanchored_ids.contains(id) {
                marks.push("UNANCHORED");
            }
            if empty_ids.contains(id) {
                marks.push("no result");
            }
            let kind = b.get("kind").and_then(Value::as_str).unwrap_or("");
            let title = if kind == "aside" {
                "旁注"
            } else {
                b.get("title").and_then(Value::as_str).unwrap_or("")
            };
            let members = b.get("members").and_then(Value::as_u64).unwrap_or(0);
            let results = b["children"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|c| is_result_child(c))
                .count();
            let suffix = if marks.is_empty() {
                String::new()
            } else {
                format!(" <- {}", marks.join(", "))
            };
            lines.push(format!(
                "{}{} {title} [{members} 张, {results} 结果]{suffix}",
                "  ".repeat(d),
                block_name(Some(b), id)
            ));
        }
        if let Some(kids) = children.get(id) {
            for c in kids {
                walk(c, d + 1, by_id, children, forward_blocks, unanchored_ids, empty_ids, lines);
            }
        }
    }
    for r in &tree.roots {
        walk(
            r,
            0,
            &by_id,
            &tree.children,
            &forward_blocks,
            &unanchored_ids,
            &empty_ids,
            &mut lines,
        );
    }

    // Level 2 formal models: check formal Layer 1 faults (matching check-model --stage restructure).
    let mut formal = Vec::new();
    let pending_criterion_import = m["build"]["closure"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|f| {
            let Some(c) = f.get("card").filter(|v| v.is_object()) else {
                return false;
            };
            c.get("type").and_then(Value::as_str) == Some("model")
                && c.get("models").and_then(Value::as_array).is_some_and(|a| !a.is_empty())
                && crate::post_digest::stage::is_criterion_card(c)
        });

    if level == "formal" && !has_form && m.get("build").is_some_and(Value::is_object) {
        for k in [
            "formal_shape",
            "setting_dependency",
            "criterion_unbound",
            "result_unbound",
            "proof_unbound",
            "experiment_unbound",
            "symbol_uncovered",
        ] {
            if k == "criterion_unbound" && pending_criterion_import {
                continue;
            }
            for f in m["build"][k].as_array().into_iter().flatten() {
                formal.push(json!({
                    "kind": k,
                    "card": f.get("address").cloned().unwrap_or(Value::Null),
                    "fix": f.get("fix").cloned().unwrap_or(Value::Null),
                }));
            }
        }
        let part_by_addr: HashMap<&str, &Value> = m["parts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.get("address").and_then(Value::as_str).map(|a| (a, p)))
            .collect();
        let head_addr = m["head"]["address"].as_str().unwrap_or("<head>");
        for b in &chapters {
            for c in b["children"].as_array().into_iter().flatten() {
                let facet = c.get("facet").and_then(Value::as_str).unwrap_or("");
                if !facet.starts_with("结果") {
                    continue;
                }
                let addr = c.get("address").and_then(Value::as_str).unwrap_or("");
                let Some(&part) = part_by_addr.get(addr) else {
                    continue;
                };
                let Some(body) = part.get("body").and_then(Value::as_str) else {
                    continue;
                };
                let missing: Vec<&str> = ["陈述", "为什么", "依据"]
                    .into_iter()
                    .filter(|sec| {
                        let tag = format!("**{sec}**");
                        !body.lines().any(|l| l.starts_with(&tag))
                    })
                    .collect();
                if !missing.is_empty() {
                    let missing_s = missing
                        .iter()
                        .map(|s| format!("**{s}**"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    formal.push(json!({
                        "kind": "result_sections",
                        "card": addr,
                        "fix": format!("{addr} ({facet}) lacks required sections: {missing_s} (run zt-check --model {head_addr} --skip-shared for full FM1-FM6)"),
                    }));
                }
            }
        }
    }

    let threads_head = layout.ribbons_len * 2 + 3;
    let ok = forward.is_empty()
        && unanchored.is_empty()
        && parts.is_empty()
        && leads.is_empty()
        && formal.is_empty();

    NetworkReport {
        parent: tree.parent,
        lines,
        forward,
        moves,
        unanchored,
        parts,
        leads,
        empty,
        formal,
        shape: json!({
            "blocks": blocks.len(),
            "depth": metrics.depth,
            "width": metrics.width,
            "fits": metrics.width <= 520,
        }),
        threads: json!({ "head": threads_head }),
        ok,
    }
}

pub fn format_text(head: &str, r: &NetworkReport) -> Vec<String> {
    let fits_note = if r.shape["fits"].as_bool() == Some(true) {
        ""
    } else {
        " (scaled down on the board)"
    };
    let mut out = vec![format!(
        "network of {head}: {} blocks, {} layers, {}px wide{fits_note}, {} paths with the Model Card open",
        r.shape["blocks"].as_u64().unwrap_or(0),
        r.shape["depth"].as_u64().unwrap_or(0),
        r.shape["width"].as_i64().unwrap_or(0),
        r.threads["head"].as_u64().unwrap_or(0),
    )];
    for l in &r.lines {
        out.push(format!("  {l}"));
    }
    for (k, list) in [
        ("forward", &r.moves),
        ("unanchored", &r.unanchored),
        ("parts", &r.parts),
        ("lead", &r.leads),
        ("formal", &r.formal),
        ("advice", &r.empty),
    ] {
        for f in list {
            let fix = f.get("fix").and_then(Value::as_str).unwrap_or("");
            out.push(format!("{k}: {fix}"));
        }
    }
    out.push(if r.ok {
        "network: clean".to_string()
    } else {
        "network: faults above".to_string()
    });
    out
}

/// Validates required fields and row shapes in a `get_model` payload for network analysis (H8).
pub fn validate_network_payload(model: &Value, head: &str) -> Result<()> {
    let Some(blocks) = model.get("blocks").and_then(Value::as_array) else {
        bail!("{head}: no blocks (zt 0.43+ and a model with a main line)");
    };
    if blocks.len() < 2 {
        bail!("{head}: no blocks (zt 0.43+ and a model with a main line)");
    }
    let mut block_ids: HashSet<&str> = HashSet::new();
    for (idx, b) in blocks.iter().enumerate() {
        let Some(obj) = b.as_object() else {
            bail!("get_model answer for {head} has non-object blocks[{idx}]");
        };
        let Some(id) = obj.get("id").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} blocks[{idx}] lacks non-empty string `id`");
        };
        if !block_ids.insert(id) {
            bail!("get_model answer for {head} has duplicate block id `{id}`");
        }
        let Some(kind) = obj.get("kind").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} blocks[{idx}] lacks non-empty string `kind`");
        };
        if !matches!(kind, "setting" | "chapter" | "aside" | "shared") {
            bail!("get_model answer for {head} blocks[{idx}] has unknown `kind` `{kind}`");
        }
        if !obj.get("children").is_some_and(Value::is_array) {
            bail!("get_model answer for {head} blocks[{idx}] lacks required `children` array");
        }
    }
    let Some(edges) = model.get("block_edges").and_then(Value::as_array) else {
        bail!("get_model answer for {head} lacks `block_edges` array");
    };
    for (idx, e) in edges.iter().enumerate() {
        let Some(obj) = e.as_object() else {
            bail!("get_model answer for {head} has non-object block_edges[{idx}]");
        };
        let Some(from) = obj.get("from_block").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} block_edges[{idx}] lacks string `from_block`");
        };
        let Some(to) = obj.get("to_block").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} block_edges[{idx}] lacks string `to_block`");
        };
        let Some(kind) = obj.get("kind").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
            bail!("get_model answer for {head} block_edges[{idx}] lacks string `kind`");
        };
        if !matches!(
            kind,
            "rests-on"
                | "illustrates"
                | "contrasts"
                | "relaxes"
                | "corresponds"
                | "objects"
                | "specializes"
                | "refers"
                | "term"
        ) {
            bail!("get_model answer for {head} block_edges[{idx}] has unknown `kind` `{kind}`");
        }
        if !block_ids.contains(from) {
            bail!("get_model answer for {head} block_edges[{idx}] references unknown `from_block` `{from}`");
        }
        if !block_ids.contains(to) {
            bail!("get_model answer for {head} block_edges[{idx}] references unknown `to_block` `{to}`");
        }
    }
    let Some(build) = model.get("build").and_then(Value::as_object) else {
        bail!("get_model answer for {head} lacks `build` object");
    };
    if !build.get("parts").is_some_and(Value::is_array) {
        bail!("get_model answer for {head} lacks required `build.parts` array");
    }
    let (level, has_form) = crate::post_digest::stage::validate_level_and_form(model, build, head)?;
    if level == "formal" && !has_form {
        if !model.get("parts").is_some_and(Value::is_array) {
            bail!("get_model answer for {head} lacks `parts` array for formal result check");
        }
        for k in [
            "closure",
            "formal_shape",
            "setting_dependency",
            "criterion_unbound",
            "result_unbound",
            "proof_unbound",
            "experiment_unbound",
            "symbol_uncovered",
        ] {
            if !build.get(k).is_some_and(Value::is_array) {
                bail!("get_model answer for {head} lacks required formal `build.{k}` array");
            }
        }
    }
    Ok(())
}

pub fn run_live(zt: &mut Zt, head: &str, json_out: bool) -> Result<(Vec<String>, u8, usize)> {
    let model = zt.call("get_model", json!({ "at": head }))?;
    let got_head = model["head"]["address"].as_str().unwrap_or("");
    if got_head != head {
        if got_head.is_empty() {
            bail!("{head} is not a Model Card");
        }
        bail!("{head} is not a Model Card: it belongs to the model {got_head}; give that address");
    }
    validate_network_payload(&model, head)?;
    let report = analyse(&model);
    let fault_count = report.forward.len()
        + report.unanchored.len()
        + report.parts.len()
        + report.leads.len()
        + report.formal.len();
    let exit = if report.ok { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report.to_json())?]
    } else {
        format_text(head, &report)
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const H: &str = "K/4|a";

    fn child(address: &str, facet: &str, proof: Option<&[&str]>) -> Value {
        let proof_val = match proof {
            Some(list) => json!({ "first": list[0], "steps": list.len(), "addresses": list }),
            None => Value::Null,
        };
        json!({ "address": address, "facet": facet, "title": format!("{facet}：x"), "proof": proof_val })
    }

    fn block(i: usize, head: &str, kind: &str, facet: &str, lead: Option<&str>, children: Vec<Value>) -> Value {
        json!({
            "id": format!("b{i}"),
            "head": head,
            "kind": kind,
            "facet": facet,
            "title": format!("{facet}：标题"),
            "lead": lead,
            "members": 1 + children.len(),
            "part": null,
            "chaptered": true,
            "children": children,
        })
    }

    fn fixture() -> Value {
        json!({
            "head": { "address": H },
            "level": null,
            "form": null,
            "build": { "parts": [] },
            "blocks": [
                block(0, "K/4|a|a", "setting", "设定", None, vec![
                    child("K/4|a|a|a", "符号表与读法", None),
                    child("K/4|a|a|b", "设定：信息", None),
                    child("K/4|a|a|c", "推导：管理跨度项", None),
                ]),
                block(1, "K/4|a|a|1", "chapter", "第 1 章：直觉", Some("很短。"), vec![
                    child("K/4|a|a|1|a", "结果：效率", Some(&["K/4|a|a|1|a|a"])),
                ]),
                block(2, "K/4|a|a|2", "chapter", "第 2 章：生产", Some("多一个职业，边际成本降一点，管理跨度成本升一点。"), vec![
                    child("K/4|a|a|2|a", "结果：离散度", Some(&["K/4|a|a|2|a|a"])),
                ]),
                block(3, "K/4|a|a|3", "chapter", "第 3 章：工资", None, vec![]),
                block(4, "K/4|a|a|4", "chapter", "第 4 章：开放", Some("出口把市场放大，同一家工厂卖得更多，于是愿意多设职业。"), vec![
                    child("K/4|a|a|4|a", "局限", None),
                ]),
            ],
            "block_edges": [
                { "from_block": "b1", "to_block": "b0", "kind": "rests-on", "count": 2, "pairs": [] },
                { "from_block": "b2", "to_block": "b1", "kind": "rests-on", "count": 3, "pairs": [] },
                { "from_block": "b0", "to_block": "b2", "kind": "rests-on", "count": 1, "pairs": [
                    { "from": "K/4|a|a|b", "to": "K/4|a|a|2|a|a" },
                    { "from": "K/4|a|a|c", "to": "K/4|a|a|2|a" },
                ]},
                { "from_block": "b4", "to_block": "b2", "kind": "rests-on", "count": 1, "pairs": [] },
                { "from_block": "b4", "to_block": "b1", "kind": "refers", "count": 5, "pairs": [] },
            ],
        })
    }

    #[test]
    fn analyses_tree_forward_unanchored_leads_and_empty_matching_gui_layout() {
        let r = analyse(&fixture());
        assert_eq!(r.parent.get("b0"), Some(&None));
        assert_eq!(r.parent.get("b1"), Some(&Some("b0".to_string())));
        assert_eq!(r.parent.get("b2"), Some(&Some("b1".to_string())));
        assert_eq!(r.parent.get("b3"), Some(&None));
        assert_eq!(r.parent.get("b4"), Some(&Some("b2".to_string())));
        assert_eq!(r.moves.len(), 2);
        assert!(r.moves[0]["fix"].as_str().unwrap().contains("is a setting"));
        assert!(r.moves[1]["fix"].as_str().unwrap().contains("move K/4|a|a|c after its latest premise"));
        assert_eq!(r.unanchored.len(), 1);
        assert_eq!(r.unanchored[0]["block"], "b3");
        assert_eq!(r.leads.len(), 2);
        assert_eq!(r.empty.len(), 2);
        assert_eq!(r.shape["depth"], 4);
        assert_eq!(r.shape["width"], 276); // two roots side-by-side: 124 + 2*14 + 124 = 276
        assert_eq!(r.threads["head"], 11); // 4 ribbons * 2 + 3 = 11
        assert!(!r.ok);
    }

    #[test]
    fn clean_spine_and_empirical_and_formal_v2_chapter_rules() {
        let mut m = fixture();
        m["blocks"][1]["lead"] = json!("错配越少，效率越高，代价是更多的管理跨度成本。");
        m["blocks"][3]["lead"] = json!("工资随效率走，离散度取决于绩效敏感度和监督的强弱。");
        m["blocks"][3]["children"] = json!([child("K/4|a|a|3|a", "结果：工资", Some(&["K/4|a|a|3|a|a"]))]);
        m["block_edges"] = json!([
            { "from_block": "b1", "to_block": "b0", "kind": "rests-on", "count": 2, "pairs": [] },
            { "from_block": "b2", "to_block": "b1", "kind": "rests-on", "count": 3, "pairs": [] },
            { "from_block": "b3", "to_block": "b2", "kind": "rests-on", "count": 1, "pairs": [] },
            { "from_block": "b4", "to_block": "b2", "kind": "rests-on", "count": 1, "pairs": [] },
        ]);
        let r = analyse(&m);
        assert!(r.ok, "{r:?}");

        let mut emp = fixture();
        emp["level"] = json!("empirical");
        emp["blocks"][3]["facet"] = json!("数据");
        let re = analyse(&emp);
        assert!(re.empty.is_empty());
        assert!(re.unanchored.is_empty());

        let mut f2 = fixture();
        f2["level"] = json!("formal");
        f2["blocks"][1]["lead"] = json!("错配越少，效率越高，代价是更多的管理跨度成本。");
        f2["blocks"][2]["children"] = json!([
            child("K/4|a|a|2|a", "判据：派生均衡定义", None),
            child("K/4|a|a|2|b", "校准：稳态参数", None),
        ]);
        f2["blocks"][3]["lead"] = json!("数值实验比较基准与固定供给体制的脉冲响应。");
        f2["blocks"][3]["children"] = json!([child("K/4|a|a|3|a", "数值实验：外逃冲击", None)]);
        f2["blocks"][4]["children"] = json!([child("K/4|a|a|4|a", "基准：无信号混同工资", None)]);
        f2["block_edges"] = json!([
            { "from_block": "b1", "to_block": "b0", "kind": "rests-on", "count": 1, "pairs": [] },
            { "from_block": "b2", "to_block": "b1", "kind": "rests-on", "count": 1, "pairs": [] },
            { "from_block": "b2", "to_block": "b3", "kind": "rests-on", "count": 1, "pairs": [{ "from": "K/4|a|a|2|a", "to": "K/4|a|a|3|a" }] },
            { "from_block": "b3", "to_block": "b2", "kind": "rests-on", "count": 1, "pairs": [] },
            { "from_block": "b4", "to_block": "b3", "kind": "rests-on", "count": 1, "pairs": [] },
        ]);
        let rf = analyse(&f2);
        assert!(rf.empty.is_empty());
        assert_eq!(rf.moves.len(), 1);
        assert!(rf.moves[0]["fix"].as_str().unwrap().contains("move K/4|a|a|2|a after its latest premise"));
    }

    #[test]
    fn rust_layout_matches_gui_layout_js_exact_numbers() {
        // Cross-check against `src/gui/layout.js` when running inside the repo with `node` available.
        let layout_js = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("src")
            .join("gui")
            .join("layout.js");
        assert!(
            layout_js.is_file(),
            "missing GUI layout reference at {}",
            layout_js.display()
        );
        let multi_root_tie = json!({
            "head": { "address": H },
            "build": { "parts": [] },
            "blocks": [
                block(0, "K/4|a|a", "setting", "设定", None, vec![]),
                block(1, "K/4|a|a|1", "chapter", "第 1 章", Some("这是足够长的一句机制直觉。"), vec![]),
                block(2, "K/4|a|a|2", "chapter", "第 2 章", Some("这是足够长的一句机制直觉。"), vec![]),
                block(3, "K/4|a|a|3", "chapter", "第 3 章", Some("这是足够长的一句机制直觉。"), vec![]),
                block(4, "K/4|a|a|4", "chapter", "第 4 章", Some("这是足够长的一句机制直觉。"), vec![]),
            ],
            "block_edges": [
                { "from_block": "b1", "to_block": "b0", "kind": "rests-on", "count": 2, "pairs": [] },
                { "from_block": "b2", "to_block": "b0", "kind": "rests-on", "count": 2, "pairs": [] },
                // Tie-breaking: b3 rests on b1 (count 2) and b2 (count 2) -> later block b2 wins!
                { "from_block": "b3", "to_block": "b1", "kind": "rests-on", "count": 2, "pairs": [] },
                { "from_block": "b3", "to_block": "b2", "kind": "rests-on", "count": 2, "pairs": [] },
                // b4 is a separate root (unanchored).
            ],
        });
        for (name, f) in [("fixture", fixture()), ("multi_root_tie", multi_root_tie)] {
            let script = format!(
                "const L=require({});const m={};const t=L.blockTree(m.blocks,m.block_edges);const bl=L.blockLayout(m.blocks,m.block_edges,null,{{focusOnly:true,backbone:true}});const tl=L.treeLayout(t.roots,t.children,()=>({{w:124,h:66}}),{{gapX:14,gapY:34}});console.log(JSON.stringify({{parent:t.parent,unanchored:bl.unanchored,ribbons:bl.ribbons.length,width:Math.round(tl.width),depth:Math.max(0,...Object.values(tl.pos).map(p=>p.depth))+1}}));",
                serde_json::to_string(&layout_js.display().to_string()).unwrap(),
                serde_json::to_string(&f).unwrap(),
            );
            let out = std::process::Command::new("node")
                .arg("-e")
                .arg(&script)
                .output()
                .expect("node must be runnable for layout.js parity test");
            assert!(
                out.status.success(),
                "node layout.js parity script failed on {name}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            let js: Value = serde_json::from_slice(&out.stdout).expect("valid JSON from layout.js");
            let rust = analyse(&f);
            assert_eq!(serde_json::to_value(&rust.parent).unwrap(), js["parent"], "{name}: parent");
            assert_eq!(rust.shape["width"], js["width"], "{name}: width");
            assert_eq!(rust.shape["depth"], js["depth"], "{name}: depth");
            assert_eq!(rust.threads["head"], js["ribbons"].as_u64().unwrap() * 2 + 3, "{name}: threads");
            let rust_unanch: Vec<&str> = rust
                .unanchored
                .iter()
                .filter_map(|u| u["block"].as_str())
                .collect();
            let js_unanch: Vec<&str> = js["unanchored"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .collect();
            assert_eq!(rust_unanch, js_unanch, "{name}: unanchored");
        }

        // P1-1 & H8: missing/non-array block_edges, missing block children, duplicate block IDs,
        // dangling edge endpoints, and missing level must all fail payload validation cleanly.
        let mut missing_edges = fixture();
        missing_edges.as_object_mut().unwrap().remove("block_edges");
        assert!(validate_network_payload(&missing_edges, H).is_err());

        let mut missing_children = fixture();
        missing_children["blocks"][0].as_object_mut().unwrap().remove("children");
        assert!(validate_network_payload(&missing_children, H).is_err());

        let mut dup_id = fixture();
        dup_id["blocks"][1]["id"] = json!("b0");
        assert!(validate_network_payload(&dup_id, H).is_err());

        let mut dangling_edge = fixture();
        dangling_edge["block_edges"][0]["to_block"] = json!("b999");
        assert!(validate_network_payload(&dangling_edge, H).is_err());

        let mut missing_level = fixture();
        missing_level.as_object_mut().unwrap().remove("level");
        assert!(validate_network_payload(&missing_level, H).is_err());

        let mut bad_kind = fixture();
        bad_kind["blocks"][1]["kind"] = json!("chaptr");
        assert!(validate_network_payload(&bad_kind, H).is_err());

        let mut bad_edge_kind = fixture();
        bad_edge_kind["block_edges"][0]["kind"] = json!("rests-onn");
        assert!(validate_network_payload(&bad_edge_kind, H).is_err());

        assert!(validate_network_payload(&fixture(), H).is_ok());
    }
}
