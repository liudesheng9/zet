//! The pure parts of `zt-digest math`: a Card body with one
//! section replaced (`Set-Section`), the dependency map (`Get-Map`), the Folgezettel order key
//! (`Get-OrderKey`), and PowerShell's array slice `$lines[a..b]`.

use crate::ps::{self, CiMap, field, s};
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static IS_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*(\*\*([^*]+)\*\*|#{1,6}\s*(\S+))").unwrap());
static BOLD_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*\*\*([^*]+)\*\*").unwrap());
static HEAD_LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*#{1,6}\s*(.+)$").unwrap());
static SOURCE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*来源[：:]").unwrap());
static BLANKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d+$").unwrap());
static MAP_DROP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"[<>&"]"#).unwrap());

pub const RESULTS: &[&str] = &["theorem", "proposition", "lemma", "corollary"];

/// `$FormZh`: the Chinese word of a form ('' for an unknown form).
pub fn form_zh(form: &str) -> &'static str {
    match form.to_lowercase().as_str() {
        "theory" => "理论",
        "definition" => "定义",
        "axiom" => "公理",
        "theorem" => "定理",
        "proposition" => "命题",
        "lemma" => "引理",
        "corollary" => "推论",
        _ => "",
    }
}

/// `$lines[($a)..($b)]`: the range runs down when b < a; a negative index counts from the end;
/// an index out of range gives nothing.
pub fn ps_slice(lines: &[String], a: i64, b: i64) -> Vec<String> {
    let n = lines.len() as i64;
    let step = if b >= a { 1 } else { -1 };
    let mut out = Vec::new();
    let mut i = a;
    loop {
        let at = if i < 0 { n + i } else { i };
        if (0..n).contains(&at) {
            out.push(lines[at as usize].clone());
        }
        if i == b {
            break;
        }
        i += step;
    }
    out
}

/// `Get-Section` on lines already read: lines `from` to `to` (1-based), at most `max` of them, joined by `\n`.
pub fn section_of(lines: &[String], from: i64, to: i64, max: i64) -> String {
    let mut to = to.min(lines.len() as i64);
    let from = from.max(1);
    if to - from + 1 > max {
        to = from + max - 1;
    }
    ps_slice(lines, from - 1, to - 1).join("\n")
}

/// `Set-Section`: a Card body with one section (a bold label or heading such as **结构**) replaced,
/// removed (empty `text`), or added before 来源.
/// The current text of one labelled section of a Card body, found exactly as
/// `Set-Section` finds it: the label line through the line before the next label or
/// source line. The ledger's `theory` check re-reads the membership and dependency
/// summary a Theory Card holds (R241).
pub fn section_text(body: &str, name: &str) -> String {
    let lines: Vec<&str> = body.split('\n').collect();
    let label_of = |l: &str| -> String {
        let cut = |t: &str| t.trim().trim_end_matches(['：', ':']).to_string();
        if let Some(m) = BOLD_LABEL.captures(l) {
            cut(&m[1])
        } else if let Some(m) = HEAD_LABEL.captures(l) {
            cut(&m[1])
        } else {
            String::new()
        }
    };
    let Some(start) = lines
        .iter()
        .position(|l| IS_LABEL.is_match(l) && ps::eq(&label_of(l), name))
    else {
        return String::new();
    };
    let mut end = lines.len();
    for (j, l) in lines.iter().enumerate().skip(start + 1) {
        if IS_LABEL.is_match(l) || SOURCE_LINE.is_match(l) {
            end = j;
            break;
        }
    }
    lines[start..end].join("\n").trim().to_string()
}

/// `Set-Section`: the body with one labelled section replaced or inserted before its source
/// line. `name` of the new section is `text`'s own first label.
pub fn set_section(body: &str, name: &str, text: &str) -> String {
    let mut lines: Vec<String> = body.split('\n').map(str::to_string).collect();
    let is_label = |l: &str| IS_LABEL.is_match(l);
    let label_of = |l: &str| -> String {
        let cut = |t: &str| t.trim().trim_end_matches(['：', ':']).to_string();
        if let Some(m) = BOLD_LABEL.captures(l) {
            cut(&m[1])
        } else if let Some(m) = HEAD_LABEL.captures(l) {
            cut(&m[1])
        } else {
            String::new()
        }
    };
    let start = lines.iter().position(|l| is_label(l) && ps::eq(&label_of(l), name));
    if let Some(start) = start {
        let mut end = lines.len();
        for (j, l) in lines.iter().enumerate().skip(start + 1) {
            if is_label(l) || SOURCE_LINE.is_match(l) {
                end = j;
                break;
            }
        }
        lines.drain(start..end);
        if !text.is_empty() {
            lines.insert(start, format!("{}\n", text.trim_end()));
        }
    } else if !text.is_empty() {
        let at = lines.iter().position(|l| SOURCE_LINE.is_match(l)).unwrap_or(lines.len());
        lines.insert(at, format!("{}\n", text.trim_end()));
    }
    BLANKS.replace_all(&lines.join("\n"), "\n\n").trim().to_string()
}

/// `Get-OrderKey`: Folgezettel order as a sortable key: numbers by value, side letters by length
/// then alphabet.
pub fn order_key(address: &str) -> String {
    address
        .split(['/', '|'])
        .map(|part| {
            if DIGITS.is_match(part) {
                format!("N{:08}", part.parse::<i64>().unwrap_or(0))
            } else {
                format!("L{:03}{part}", ps::len16(part))
            }
        })
        .collect::<Vec<_>>()
        .join("/")
}

/// `Get-Map`: the dependency map, one column per depth, each unit a box that opens its Card.
pub fn dependency_map(order: &[&Value]) -> Result<String> {
    if order.is_empty() {
        return Ok(String::new());
    }
    let mut depth: CiMap<i64> = CiMap::new();
    for u in order {
        let mut d = 0;
        for c in ps::pipe(&u["cites"]) {
            if c.is_null() {
                bail!("Exception calling \"ContainsKey\" with \"1\" argument(s): \"Value cannot be null. (Parameter 'key')\"");
            }
            if let Some(x) = depth.get(&s(c)) {
                d = d.max(x + 1);
            }
        }
        depth.insert(&field(u, "address"), d);
    }
    let mut columns: Vec<(i64, i64)> = Vec::new();
    let mut pos: CiMap<(i64, i64)> = CiMap::new();
    for u in order {
        let d = *depth.get(&field(u, "address")).unwrap_or(&0);
        let row = match columns.iter_mut().find(|(k, _)| *k == d) {
            Some(slot) => {
                let row = slot.1;
                slot.1 += 1;
                row
            }
            None => {
                columns.push((d, 1));
                0
            }
        };
        pos.insert(&field(u, "address"), (d, row));
    }
    let (w, h, bw, bh) = (170i64, 46i64, 140i64, 34i64);
    let cols = depth.iter().map(|(_, v)| *v).max().unwrap_or(0) + 1;
    let rows = columns.iter().map(|(_, n)| *n).max().unwrap_or(0);
    let mut svg = format!(
        "<svg viewBox=\"0 0 {} {}\" width=\"100%\" xmlns=\"http://www.w3.org/2000/svg\" font-size=\"11\" font-family=\"sans-serif\">",
        cols * w,
        rows * h + 8
    );
    for u in order {
        let to = *pos.get(&field(u, "address")).unwrap_or(&(0, 0));
        for c in ps::pipe(&u["cites"]) {
            let Some(from) = pos.get(&s(c)) else { continue };
            svg.push_str(&format!(
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#999\" stroke-width=\"1\"/>",
                from.0 * w + bw + 4,
                from.1 * h + 4 + bh / 2,
                to.0 * w + 4,
                to.1 * h + 4 + bh / 2
            ));
        }
    }
    for u in order {
        let p = *pos.get(&field(u, "address")).unwrap_or(&(0, 0));
        let (x, y) = (p.0 * w + 4, p.1 * h + 4);
        let weight = field(u, "weight");
        let stroke = if ps::eq(&weight, "focus") {
            "#333"
        } else if ps::eq(&weight, "support") {
            "#888"
        } else {
            "#bbb"
        };
        let fill = if ps::eq(&weight, "focus") { "#fffbe6" } else { "#fff" };
        let mut label = MAP_DROP.replace_all(&field(u, "title"), "").into_owned();
        if ps::len16(&label) > 11 {
            label = format!("{}…", ps::cut16(&label, 11));
        }
        let zh = form_zh(&field(u, "form"));
        svg.push_str(&format!(
            "<g data-zt=\"{}\" style=\"cursor:pointer\"><rect x=\"{x}\" y=\"{y}\" width=\"{bw}\" height=\"{bh}\" rx=\"3\" fill=\"{fill}\" stroke=\"{stroke}\"/><text x=\"{}\" y=\"{}\" fill=\"#666\">{zh}</text><text x=\"{}\" y=\"{}\" fill=\"#222\">{label}</text></g>",
            field(u, "address"),
            x + 6,
            y + 14,
            x + 6,
            y + 28
        ));
    }
    svg.push_str("</svg>");
    Ok(format!("```chart\n<figure>{svg}<figcaption>依赖图：左边的单元先学，线指向引用它的单元。</figcaption></figure>\n```"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn slices_as_powershell() {
        let l: Vec<String> = ["a", "b", "c"].iter().map(|x| x.to_string()).collect();
        assert_eq!(ps_slice(&l, 0, -1), ["a", "c"]);
        assert_eq!(ps_slice(&l, 4, 2), ["c"]);
        assert_eq!(ps_slice(&l, 1, 2), ["b", "c"]);
        assert_eq!(section_of(&l, 2, 9, 80), "b\nc");
        assert_eq!(section_of(&l, 1, 9, 2), "a\nb");
        assert_eq!(section_of(&[], 1, 5, 6), "");
    }

    #[test]
    fn a_section_is_replaced_removed_or_added_before_the_source() {
        let body = "**问题**：待写。\n\n**结构**：\n- a\n\n来源：[[K]] 第 1 章";
        assert_eq!(set_section(body, "问题", "**问题**：为何？"), "**问题**：为何？\n\n**结构**：\n- a\n\n来源：[[K]] 第 1 章");
        assert_eq!(set_section(body, "结构", ""), "**问题**：待写。\n\n来源：[[K]] 第 1 章");
        assert_eq!(set_section(body, "图", "**图**：\n\nx"), "**问题**：待写。\n\n**结构**：\n- a\n\n**图**：\n\nx\n\n来源：[[K]] 第 1 章");
        assert_eq!(set_section("# 结构：\nold\n## 后续\nz", "结构", "**结构**（依赖顺序）：\n- b"), "**结构**（依赖顺序）：\n- b\n\n## 后续\nz");
    }

    #[test]
    fn order_keys_sort_side_successors_last_first() {
        assert_eq!(order_key("K/2|a"), "L001K/N00000002/L001a");
        assert_eq!(order_key("3/10|ab"), "N00000003/N00000010/L002ab");
    }

    #[test]
    fn the_map_puts_each_unit_right_of_what_it_cites() {
        let order = [
            json!({ "address": "K/1|a", "form": "definition", "title": "连续<x>", "cites": [], "weight": "focus" }),
            json!({ "address": "K/1|b", "form": "theorem", "title": "介值定理：连续函数取遍中间值", "cites": ["k/1|A"] }),
        ];
        let refs: Vec<&Value> = order.iter().collect();
        let map = dependency_map(&refs).unwrap();
        assert!(map.starts_with("```chart\n<figure><svg viewBox=\"0 0 340 54\""));
        assert!(map.contains("<line x1=\"144\" y1=\"21\" x2=\"174\" y2=\"21\""));
        assert!(map.contains("fill=\"#fffbe6\" stroke=\"#333\"/><text x=\"10\" y=\"18\" fill=\"#666\">定义</text><text x=\"10\" y=\"32\" fill=\"#222\">连续x</text>"));
        assert!(map.contains(">介值定理：连续函数取遍…</text>"));
        assert_eq!(dependency_map(&[]).unwrap(), "");
    }
}
