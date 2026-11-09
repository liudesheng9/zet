//! `tables.json` of an empirical model (docs/empirical-models-proposal.md §8.2 step 1): the
//! source's tables, read by script from the section files of the model's group, so every number
//! of an estimate comes from its cell and never from a writer's copy.
//!
//! A table is an HTML `<table>` (MinerU writes one line per table) or a Markdown pipe table.
//! Its caption is the nearest `表 n` / `Table n` line above it, its notes the `注` / `Note` lines
//! below it. Each row keeps its label (the first cell), its cells and the line it stands on.
//!
//! `cell` resolves an estimate's `{table, row, col}`: the value, its stars, its standard error
//! (in the same cell as `0.014***(0.003)`, or in the parenthesized row below), the significance
//! level the notes give the stars, and the sample size of the same column. `numbers` reads the
//! numbers of a text the way the evidence check compares them.

use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

// A caption: 表 2, Table 3.1, A1, or a Roman numeral (表 VI, autor2004will).
static CAPTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*(?:\*\*)?(表|Table|TABLE)\s*([A-Za-z]?\d+(?:[.\-]\d+)*[A-Za-z]?|[IVXLC]+\b)").unwrap()
});
// A notes line: a 注/Note label, or the star legend itself (`\* p<0.1，…`, hansen2021demand Table 5).
static NOTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*((注|附注|说明|资料来源|数据来源|来源|Notes?|Source)\s*[：:.]|\\?\*+\s*[pP]\s*[<＜])").unwrap()
});
static ROW: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<tr[^>]*>(.*?)</tr>").unwrap());
static CELL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?is)<t([dh])([^>]*)>(.*?)</t[dh]>"#).unwrap());
static COLSPAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)colspan\s*=\s*"?(\d+)"#).unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\s*([-−–]?\s*\d[\d,]*(?:\.\d+)?|[-−–]?\.\d+)\s*(%?)\s*(\\?\*(?:\\?\*)*)?\s*(?:[\(\[]\s*([-−–]?\d[\d,]*(?:\.\d+)?|[-−–]?\.\d+)\s*[\)\]])?\s*(\\?\*(?:\\?\*)*)?\s*$").unwrap()
});
static PAREN_ONLY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[\(\[]\s*([-−–]?\d[\d,]*(?:\.\d+)?|[-−–]?\.\d+)\s*[\)\]]\s*$").unwrap());
static NUMBER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[-−–]?\d[\d,]*(?:\.\d+)?|[-−–]?\.\d+").unwrap());
static OBSERVATIONS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(observations|obs\.?|n|no\. of obs|number of observations|样本量|观测值|观测数|观测值数|样本数)\s*$").unwrap()
});

fn decode(text: &str) -> String {
    let plain = TAG.replace_all(text, " ");
    plain
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'")
        .replace("&quot;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// One table's rows from its HTML: each `<td>`/`<th>` text, a `colspan` cell followed by empty
/// cells so the columns stay aligned.
fn html_rows(html: &str) -> Vec<Vec<String>> {
    ROW.captures_iter(html)
        .map(|row| {
            let mut cells = Vec::new();
            for cell in CELL.captures_iter(&row[1]) {
                cells.push(decode(&cell[3]));
                let span = COLSPAN.captures(&cell[2]).and_then(|c| c[1].parse::<usize>().ok()).unwrap_or(1);
                for _ in 1..span.min(64) {
                    cells.push(String::new());
                }
            }
            cells
        })
        .collect()
}

fn pipe_rows(lines: &[&str]) -> Vec<Vec<String>> {
    lines
        .iter()
        .map(|line| line.trim().trim_start_matches('|').trim_end_matches('|'))
        .filter(|line| !line.chars().all(|c| matches!(c, '-' | ':' | '|' | ' ')))
        .map(|line| line.split('|').map(|cell| decode(cell.trim())).collect())
        .collect()
}

/// A short form of a label or a column head for matching: no spaces, no brackets, lower case.
fn norm(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && !"()（）[]【】".contains(*c))
        .collect::<String>()
        .to_lowercase()
}

/// The canonical form of one number: no thousands separators, an ASCII minus, no leading zero
/// before the decimal point (`0.014` and `.014` match).
pub(crate) fn canonical(number: &str) -> String {
    let mut text = number.replace([',', ' '], "").replace(['−', '–'], "-");
    let negative = text.starts_with('-');
    if negative {
        text.remove(0);
    }
    let trimmed = text.trim_start_matches('0');
    let body = if trimmed.is_empty() || trimmed.starts_with('.') {
        if trimmed.is_empty() { "0".to_string() } else { trimmed.to_string() }
    } else {
        trimmed.to_string()
    };
    let body = if body.contains('.') { body.trim_end_matches('0').trim_end_matches('.').to_string() } else { body };
    let body = if body.is_empty() { "0".to_string() } else { body };
    if negative && body != "0" { format!("-{body}") } else { body }
}

/// The numbers of a text, in canonical form.
pub(crate) fn numbers(text: &str) -> Vec<String> {
    NUMBER.find_iter(text).map(|m| canonical(m.as_str())).collect()
}

/// Every table in one section file. `file` is how a source reference names it (`sections/s12.md`).
pub(crate) fn extract(text: &str, file: &str) -> Vec<Value> {
    let lines = text.lines().collect::<Vec<_>>();
    let mut tables = Vec::new();
    let mut i = 0;
    let mut untitled = 0;
    while i < lines.len() {
        let line = lines[i];
        let (rows, end) = if line.contains("<table") {
            // A table may span lines; it ends on the line that closes it.
            let mut end = i;
            let mut html = String::new();
            while end < lines.len() {
                html.push_str(lines[end]);
                html.push('\n');
                if lines[end].contains("</table>") {
                    break;
                }
                end += 1;
            }
            (html_rows(&html), end)
        } else if line.trim_start().starts_with('|') {
            let mut end = i;
            while end + 1 < lines.len() && lines[end + 1].trim_start().starts_with('|') {
                end += 1;
            }
            if end == i {
                i += 1;
                continue;
            }
            (pipe_rows(&lines[i..=end]), end)
        } else {
            i += 1;
            continue;
        };
        // The caption: the nearest 表 n line within four lines above.
        let caption = (i.saturating_sub(4)..i)
            .rev()
            .map(|k| lines[k])
            .find(|text| CAPTION.is_match(text))
            .unwrap_or("");
        let label = match CAPTION.captures(caption) {
            Some(c) => format!("表 {}", &c[2]),
            None => {
                untitled += 1;
                let stem = file.rsplit(['/', '\\']).next().unwrap_or(file).trim_end_matches(".md");
                format!("未命名表 {stem}-{untitled}")
            }
        };
        let notes = lines[end + 1..]
            .iter()
            .take(4)
            .filter(|text| NOTE.is_match(text))
            .map(|text| text.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let width = rows.iter().map(Vec::len).max().unwrap_or(0);
        // The head row: the first row that fills the width and holds no estimate.
        let head = rows.iter().position(|row| {
            row.len() == width && row.iter().skip(1).filter(|c| !c.is_empty()).count() >= 1 && !row.iter().skip(1).any(|c| VALUE.is_match(c) && c.contains('*'))
        });
        let header = head.map(|k| rows[k].clone()).unwrap_or_default();
        // Rows above the head are the table's titles (the dependent variable, a panel name).
        let titles = head
            .map(|h| rows[..h].iter().map(|row| row.iter().filter(|c| !c.is_empty()).cloned().collect::<Vec<_>>().join(" ")).filter(|t| !t.is_empty()).collect::<Vec<_>>())
            .unwrap_or_default();
        let body = rows
            .iter()
            .enumerate()
            .filter(|(k, _)| Some(*k) != head && head.is_none_or(|h| *k > h))
            .map(|(_, row)| json!({ "label": row.first().cloned().unwrap_or_default(), "cells": row.iter().skip(1).cloned().collect::<Vec<_>>() }))
            .collect::<Vec<_>>();
        tables.push(json!({
            "label": label,
            "caption": caption.trim(),
            "file": file,
            "line": i + 1,
            "notes": notes,
            "titles": titles,
            "header": header.iter().skip(1).cloned().collect::<Vec<_>>(),
            "rows": body,
        }));
        i = end + 1;
    }
    tables
}

/// The table a plan names (`表 2`, `Table 2`, `2`).
pub(crate) fn find<'a>(tables: &'a Value, label: &str) -> Option<&'a Value> {
    let wanted = norm(label).trim_start_matches("表").trim_start_matches("table").to_string();
    tables["tables"].as_array()?.iter().find(|table| {
        let have = norm(table["label"].as_str().unwrap_or_default());
        have.trim_start_matches("表") == wanted || have == norm(label)
    })
}

/// The index of a column in a table's data cells: a head that matches (`1` matches `(1)`), or a
/// 1-based position.
fn column(table: &Value, col: &str) -> Option<usize> {
    let wanted = norm(col);
    let header = table["header"].as_array()?;
    if let Some(k) = header.iter().position(|h| norm(h.as_str().unwrap_or_default()) == wanted) {
        return Some(k);
    }
    let n = wanted.trim_start_matches("第").trim_end_matches("列").parse::<usize>().ok()?;
    let width = table["rows"].as_array()?.iter().map(|r| r["cells"].as_array().map_or(0, Vec::len)).max().unwrap_or(0);
    (n >= 1 && n <= width.max(header.len())).then(|| n - 1)
}

fn row_index(table: &Value, row: &str) -> Option<usize> {
    let rows = table["rows"].as_array()?;
    let wanted = norm(row);
    rows.iter()
        .position(|r| norm(r["label"].as_str().unwrap_or_default()) == wanted)
        .or_else(|| {
            rows.iter().position(|r| {
                let have = norm(r["label"].as_str().unwrap_or_default());
                !have.is_empty() && (have.contains(&wanted) || wanted.contains(&have))
            })
        })
}

fn stars_in(text: &str) -> usize {
    text.matches('*').count()
}

/// The significance level the notes give `stars` stars (`*** p<0.01`), if they give one.
fn significance(notes: &str, stars: usize) -> Option<String> {
    if stars == 0 {
        return None;
    }
    // TeX marks in the notes (`$*$`, `$^{*}$`) are the same stars.
    let plain = notes.replace("\\*", "*").replace("^{", "").replace(['$', '}'], "");
    let pattern = Regex::new(&format!(r"(?:^|[^*])\*{{{stars}}}\s*(?:表示|denotes|:|：)?\s*[pP]\s*([<＜≤])\s*(\d*\.?\d+)")).ok()?;
    if let Some(c) = pattern.captures(&plain) {
        return Some(format!("p {} {}", if &c[1] == "＜" { "<" } else { &c[1] }, &c[2]));
    }
    // 「\* 表示在 5% 水平上显著」(ebrill2001modern).
    let words = Regex::new(&format!(r"(?:^|[^*])\*{{{stars}}}\s*(?:表示|代表)\s*(?:在)?\s*(\d+(?:\.\d+)?)\s*%\s*(?:的)?\s*水平")).ok()?;
    words.captures(&plain).map(|c| format!("{}% 水平", &c[1]))
}

/// What the parentheses of a table hold, as its notes say: `t` (t statistics), `z`, or `se`
/// (standard errors, the default).
fn paren_kind(notes: &str) -> &'static str {
    let lower = notes.to_lowercase();
    if Regex::new(r"t\s*(统计量|值|比率)|t-stat|t stat|t ratio|t-ratio").is_ok_and(|r| r.is_match(&lower)) {
        "t"
    } else if Regex::new(r"z\s*(统计量|值)|z-stat|z stat").is_ok_and(|r| r.is_match(&lower)) {
        "z"
    } else {
        "se"
    }
}

/// One cell of an estimate: `{text, value, stars, se, significance, n, table, row, col, file, line}`,
/// or none when the table, the row or the column is not there.
pub(crate) fn cell(tables: &Value, table: &str, row: &str, col: &str) -> Option<Value> {
    let found = find(tables, table)?;
    let r = row_index(found, row)?;
    let c = column(found, col)?;
    let rows = found["rows"].as_array()?;
    let text = rows[r]["cells"].get(c).and_then(Value::as_str).unwrap_or_default().to_string();
    let parsed = VALUE.captures(&text);
    let mut value = parsed.as_ref().map(|p| format!("{}{}", p[1].replace(' ', ""), &p[2]));
    let stars = stars_in(&text);
    let mut se = parsed.as_ref().and_then(|p| p.get(4)).map(|m| m.as_str().to_string());
    // A standard error in the row below: an empty label and a parenthesized number.
    if se.is_none()
        && let Some(next) = rows.get(r + 1)
        && next["label"].as_str().unwrap_or_default().trim().is_empty()
        && let Some(below) = next["cells"].get(c).and_then(Value::as_str)
        && let Some(p) = PAREN_ONLY.captures(below)
    {
        se = Some(p[1].to_string());
    }
    if value.is_none() && !text.trim().is_empty() {
        value = NUMBER.find(&text).map(|m| m.as_str().to_string());
    }
    let n = rows
        .iter()
        .find(|row| OBSERVATIONS.is_match(row["label"].as_str().unwrap_or_default()))
        .and_then(|row| row["cells"].get(c).and_then(Value::as_str))
        .filter(|n| !n.trim().is_empty())
        .map(str::to_string);
    let head = found["header"].get(c).and_then(Value::as_str).unwrap_or_default().to_string();
    Some(json!({
        "text": text,
        "value": value,
        "stars": stars,
        "se": se,
        "paren": paren_kind(found["notes"].as_str().unwrap_or_default()),
        "significance": significance(found["notes"].as_str().unwrap_or_default(), stars),
        "n": n,
        "table": found["label"],
        "row": rows[r]["label"],
        "col": if head.is_empty() { format!("第 {} 列", c + 1) } else { head },
        "file": found["file"],
        "line": found["line"],
    }))
}

/// Every number the model's tables hold, canonical: cells, heads, row labels and notes.
pub(crate) fn table_numbers(tables: &Value) -> Vec<String> {
    let mut out = Vec::new();
    for table in tables["tables"].as_array().into_iter().flatten() {
        let mut text = String::new();
        for field in ["caption", "notes"] {
            text.push_str(table[field].as_str().unwrap_or_default());
            text.push('\n');
        }
        for head in table["header"].as_array().into_iter().flatten() {
            text.push_str(head.as_str().unwrap_or_default());
            text.push('\n');
        }
        for row in table["rows"].as_array().into_iter().flatten() {
            text.push_str(row["label"].as_str().unwrap_or_default());
            text.push('\n');
            for cell in row["cells"].as_array().into_iter().flatten() {
                text.push_str(cell.as_str().unwrap_or_default());
                text.push(' ');
            }
        }
        out.extend(numbers(&text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HANSEN: &str = "表 2 展示了社交技能与这些代理变量之间的横截面关系。\n\n表 2：社交集群与企业特征\n\n<table><tr><td colspan=\"6\">Dependent Variable: Social Skill Cluster</td></tr><tr><td></td><td>(1)</td><td>(2)</td><td>(3)</td><td>(4)</td><td>(5)</td></tr><tr><td>Log(Employment)</td><td>0.014***(0.003)</td><td>0.012***(0.003)</td><td>0.015***(0.003)</td><td>0.012***(0.003)</td><td>0.012***(0.003)</td></tr><tr><td>MNE</td><td></td><td>0.047***(0.017)</td><td></td><td></td><td></td></tr><tr><td>Diversified</td><td></td><td></td><td>-0.004(0.017)</td><td></td><td></td></tr><tr><td>M&amp;A activity</td><td></td><td></td><td></td><td>0.030*(0.016)</td><td></td></tr><tr><td>Observations</td><td>4,618</td><td>4,618</td><td>4,618</td><td>4,618</td><td>4,618</td></tr></table>\n\n注：\\* p<0.1，\\*\\* p<0.05，\\*\\*\\* p<0.01。所有列均以 OLS 估计。\n\n我们先看第 (1) 列。";

    const EBRILL: &str = "表 4.2 增值税收入的回归\n\n| 变量 | A | B |\n| --- | --- | --- |\n| ln(SR) | 0.70** | 0.62* |\n| | (0.21) | (0.30) |\n| N | 69 | 89 |\n\n注：括号内为 White 稳健标准误。";

    #[test]
    fn an_html_table_reads_its_caption_head_rows_and_notes() {
        let tables = extract(HANSEN, "sections/s12.md");
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!(t["label"], "表 2");
        assert_eq!(t["line"], 5);
        assert_eq!(t["header"], json!(["(1)", "(2)", "(3)", "(4)", "(5)"]));
        assert_eq!(t["titles"], json!(["Dependent Variable: Social Skill Cluster"]));
        assert_eq!(t["rows"][0]["label"], "Log(Employment)");
        assert!(t["notes"].as_str().unwrap().starts_with("注："));
        assert_eq!(t["rows"].as_array().unwrap().iter().find(|r| r["label"] == "M&A activity").unwrap()["cells"][3], "0.030*(0.016)");
    }

    #[test]
    fn a_cell_gives_its_value_stars_error_significance_and_n() {
        let tables = json!({ "tables": extract(HANSEN, "sections/s12.md") });
        let c = cell(&tables, "表 2", "Log(Employment)", "1").expect("a cell");
        assert_eq!(c["value"], "0.014");
        assert_eq!(c["stars"], 3);
        assert_eq!(c["se"], "0.003");
        assert_eq!(c["significance"], "p < 0.01");
        assert_eq!(c["n"], "4,618");
        assert_eq!(c["col"], "(1)");
        let d = cell(&tables, "Table 2", "diversified", "(3)").expect("a cell");
        assert_eq!(d["value"], "-0.004");
        assert_eq!(d["stars"], 0);
        assert_eq!(d["significance"], Value::Null);
        assert!(cell(&tables, "表 3", "MNE", "2").is_none());
        assert!(cell(&tables, "表 2", "Exports", "2").is_none());
    }

    #[test]
    fn a_pipe_table_takes_its_standard_error_from_the_row_below() {
        let tables = json!({ "tables": extract(EBRILL, "sections/s05.md") });
        assert_eq!(tables["tables"][0]["label"], "表 4.2");
        let c = cell(&tables, "表 4.2", "ln(SR)", "A").expect("a cell");
        assert_eq!(c["value"], "0.70");
        assert_eq!(c["se"], "0.21");
        assert_eq!(c["n"], "69");
        let b = cell(&tables, "4.2", "ln(SR)", "2").expect("column by position");
        assert_eq!(b["value"], "0.62");
    }

    /// ebrill2001modern Table 4.2 (pilot 2026-11-09): the parentheses hold t statistics, as the notes say,
    /// and the legend reads 「\* 表示在 5% 水平上显著」.
    #[test]
    fn the_notes_say_what_the_parentheses_hold_and_what_a_star_means() {
        let text = "表 4.2. 增值税收入的建模\n\n| 变量 | A |\n| --- | --- |\n| ln(SR) | 0.70* |\n| | (2.81) |\n| N | 69 |\n\n注：括号内为 t 统计量（由 White 标准误计算得出）；$*$ 表示在 5% 水平上显著，$\\dagger$ 表示在 10% 水平上显著。";
        let tables = json!({ "tables": extract(text, "sections/s18.md") });
        let c = cell(&tables, "表 4.2", "ln(SR)", "A").expect("a cell");
        assert_eq!(c["se"], "2.81");
        assert_eq!(c["paren"], "t");
        assert_eq!(c["significance"], "5% 水平");
        let plain = json!({ "tables": extract(HANSEN, "sections/s12.md") });
        assert_eq!(cell(&plain, "表 2", "MNE", "2").unwrap()["paren"], "se");
    }

    /// A table without a caption is named by its file, so two files' first untitled tables differ
    /// (hansen2021demand: s07.md and s08.md each began with 未命名表 1).
    #[test]
    fn an_untitled_table_is_named_by_its_file() {
        let text = "正文。\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n";
        assert_eq!(extract(text, "sections/s07.md")[0]["label"], "未命名表 s07-1");
        assert_eq!(extract(text, "sections/s08.md")[0]["label"], "未命名表 s08-1");
    }

    /// autor2004will numbers its tables in Roman numerals (表 I. … 表 VII.); a plan names them so.
    #[test]
    fn a_roman_numeral_caption_names_the_table() {
        let text = "表 VI. 测试对任职期的影响\n\n<table><tr><td></td><td>(1)</td></tr><tr><td>测试</td><td>0.10</td></tr></table>\n\nTable IV: Hiring\n\n| a | b |\n| --- | --- |\n| 1 | 2 |\n\n表注：以下为说明。\n";
        let found = extract(text, "back-matter.zh.md");
        assert_eq!(found[0]["label"], "表 VI");
        assert_eq!(found[1]["label"], "表 IV", "a label is written as 表 whatever the caption's language");
        let tables = json!({ "tables": found });
        assert_eq!(cell(&tables, "表 VI", "测试", "1").expect("a cell")["value"], "0.10");
    }

    /// hansen2021demand Table 5: the notes line is the star legend itself, with no 注 label.
    #[test]
    fn a_star_legend_line_is_the_tables_notes() {
        let text = "表 5：稳健性检验\n\n<table><tr><td>Dependent Variable: Social Cluster</td><td>(1) Baseline</td><td>(2) Probit</td></tr><tr><td>Log(Employment)</td><td>0.014***(0.003)</td><td>0.059***(0.013)</td></tr></table>\n\n\\* p<0.1，\\*\\* p<0.05，\\*\\*\\* p<0.01。除第 (2) 列外，所有列均以 OLS 估计。";
        let tables = json!({ "tables": extract(text, "sections/s15.md") });
        let c = cell(&tables, "表 5", "Log(Employment)", "2").expect("a cell by position");
        assert_eq!(c["value"], "0.059");
        assert_eq!(c["significance"], "p < 0.01");
    }

    #[test]
    fn numbers_compare_in_canonical_form() {
        assert_eq!(numbers("值 0.014，N = 4,618，−0.004 与 .50"), [".014", "4618", "-.004", ".5"]);
        assert_eq!(canonical("0.70"), ".7");
        assert_eq!(canonical("-0"), "0");
        assert!(table_numbers(&json!({ "tables": extract(HANSEN, "s") })).contains(&".047".to_string()));
    }
}
