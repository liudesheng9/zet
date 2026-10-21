//! The model compiler's text functions, without the archive: Derivation part
//! sections, chain position lines, the Model Card's sections, the setting's
//! tables, and the explain step's insertion and core. Each is the function of
//! model-compiler.ps1 it names.

use crate::ps::{self, list, pipe, s, truthy};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

macro_rules! pattern {
    ($name:ident, $re:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($re).unwrap());
    };
}

pattern!(DISPLAY, r"\$\$[\s\S]+?\$\$");
pattern!(INLINE, r"\$[^$\n]+\$");
pattern!(SENTENCE_END, r"(?i)[。！？\n]");
pattern!(DISPLAY_WHOLE, r"(?i)^\$\$([\s\S]*)\$\$$");
pattern!(SECTION_HEAD, r"(?i)^\s*\*\*(思路|起点|步骤|结果|解读)\*\*\s*$");
pattern!(POSITION, r"(?i)^第 \d+ 张，共 \d+ 张");
pattern!(SOURCE, r"(?i)^来源\s*[：:]");
pattern!(NOTE, r"(?i)^说明\s*[：:]");
pattern!(NOTE_LABEL, r"(?i)^说明\s*[：:]\s*");
pattern!(STEP_NUMBER, r"(?i)^\s*(\d+)[.、)]");
pattern!(LINKS_OR_SOURCE, r"(?i)^\s*(\*\*(关联|来源)\*\*|#+\s*(关联|来源)|(关联|来源)[：:])");
pattern!(NEXT_HEAD, r"(?i)^\s*(\*\*[^*]+\*\*\s*$|#+\s|\S{2,6}[：:]\s*$)");
pattern!(SETTING_FACET, r"(?i)设定");

/// The link words of the relation kinds (`$Words`; its keys ignore case).
pub fn link_word(kind: &str) -> String {
    const WORDS: [(&str, &str); 8] = [
        ("rests", "依赖"),
        ("substitutes", "代入"),
        ("illustrates", "例示"),
        ("contrasts", "对照"),
        ("relaxes", "放宽"),
        ("corresponds", "对应"),
        ("objects", "反驳"),
        ("refers", "见"),
    ];
    WORDS.iter().find(|(k, _)| ps::key(k) == ps::key(kind)).map_or_else(|| kind.to_string(), |(_, w)| w.to_string())
}

/// `Get-Line`: shared for a take, else the plan's line, else main.
pub fn line_of(part: &Value) -> String {
    if truthy(&part["take"]) {
        "shared".into()
    } else if truthy(&part["line"]) {
        s(&part["line"])
    } else {
        "main".into()
    }
}

/// `Get-Ref`: existing, take, move, or new.
pub fn ref_of(part: &Value) -> &'static str {
    if truthy(&part["existing"]) {
        "existing"
    } else if truthy(&part["take"]) {
        "take"
    } else if truthy(&part["move"]) {
        "move"
    } else {
        "new"
    }
}

pub fn derives(part: &Value) -> bool {
    truthy(&part["derive"])
}

/// `Get-Tables`: the setting's symbol and term tables (StringBuilder lines end in CRLF).
pub fn tables(plan: &Value) -> String {
    let mut t = String::new();
    let mut line = |text: &str| {
        t.push_str(text);
        t.push_str("\r\n");
    };
    line("符号表");
    line("");
    line("| 符号 | 含义 | 首次出现 |");
    line("| --- | --- | --- |");
    for row in list(&plan["symbols"]) {
        let row = pipe(row);
        let first = if row.len() > 2 && truthy(row[2]) { s(row[2]) } else { "本卡".to_string() };
        line(&format!("| {} | {} | {first} |", s(&ps::at(&row, 0)), s(&ps::at(&row, 1))));
    }
    line("");
    line("术语表");
    line("");
    line("| 术语 | 不用 |");
    line("| --- | --- |");
    for row in list(&plan["terms"]) {
        let row = pipe(row);
        let avoid = row.iter().skip(1).map(|v| s(v)).collect::<Vec<_>>().join("、");
        line(&format!("| {} | {avoid} |", s(&ps::at(&row, 0))));
    }
    t.trim_end().to_string()
}

/// `Get-ResultFormula`: the last display formula, else the first inline one,
/// else the first sentence (cut to 30 characters).
pub fn result_formula(result: &str) -> String {
    if let Some(last) = DISPLAY.find_iter(result).last() {
        return last.as_str().to_string();
    }
    if let Some(inline) = INLINE.find(result) {
        return inline.as_str().to_string();
    }
    let first = SENTENCE_END.split(result).next().unwrap_or_default().trim().to_string();
    if ps::len16(&first) > 30 { ps::cut16(&first, 30) + "…" } else { first }
}

/// `Get-Inline`: a display formula as an inline one.
pub fn inline(formula: &str) -> String {
    let f = formula.trim();
    match DISPLAY_WHOLE.captures(f) {
        Some(c) => format!("${}$", c[1].trim()),
        None => f.to_string(),
    }
}

/// `Format-ChainLine`: the first line of a chain member's 思路.
pub fn chain_line(k: usize, n: usize, tos: &[String]) -> String {
    if n < 2 {
        return String::new();
    }
    let tos: Vec<String> = tos.iter().map(|t| result_formula(t)).collect();
    if k == 1 {
        let all = tos.iter().map(|t| inline(t)).collect::<Vec<_>>().join("、");
        return format!("第 1 张，共 {n} 张。全链依次推出 {all}。");
    }
    let get = |i: usize| tos.get(i).map(|t| inline(t)).unwrap_or_default();
    format!("第 {k} 张，共 {n} 张：从 {} 推到 {}。", get(k - 2), get(k - 1))
}

#[derive(Clone, Debug)]
pub struct Section {
    pub head: String,
    pub lines: Vec<String>,
}

/// `Split-Sections`: text before the first heading has head ''.
pub fn split_sections(body: &str) -> Vec<Section> {
    let mut out = Vec::new();
    let mut cur = Section { head: String::new(), lines: Vec::new() };
    for line in body.replace('\r', "").split('\n') {
        if let Some(c) = SECTION_HEAD.captures(line) {
            out.push(std::mem::replace(&mut cur, Section { head: c[1].to_string(), lines: Vec::new() }));
            continue;
        }
        cur.lines.push(line.to_string());
    }
    out.push(cur);
    out
}

pub fn join_sections(sections: &[Section]) -> String {
    let mut parts = Vec::new();
    for sec in sections {
        let text = sec.lines.join("\n").trim().to_string();
        if !sec.head.is_empty() {
            parts.push(if text.is_empty() { format!("**{}**", sec.head) } else { format!("**{}**\n{text}", sec.head) });
        } else if !text.is_empty() {
            parts.push(text);
        }
    }
    parts.join("\n\n").trim().to_string()
}

/// `Set-ChainLine`: the position line at the top of 思路; a body without 思路 stays.
pub fn set_chain_line(body: &str, line: &str) -> String {
    let mut sections = split_sections(body);
    let Some(idea) = sections.iter_mut().find(|sec| ps::eq(&sec.head, "思路")) else { return body.to_string() };
    let mut rest: Vec<String> = idea.lines.iter().filter(|l| !l.trim().is_empty()).cloned().collect();
    if rest.first().is_some_and(|r| POSITION.is_match(r.trim())) {
        rest.remove(0);
    }
    let mut lines = Vec::new();
    if !line.is_empty() {
        lines.push(line.to_string());
    }
    lines.extend(rest);
    idea.lines = lines;
    join_sections(&sections)
}

/// `Get-Result`: the 结果 section without a 来源 line.
pub fn result_of(body: &str) -> String {
    match split_sections(body).into_iter().find(|sec| ps::eq(&sec.head, "结果")) {
        Some(r) => r.lines.iter().filter(|l| !SOURCE.is_match(l.trim())).cloned().collect::<Vec<_>>().join("\n").trim().to_string(),
        None => String::new(),
    }
}

/// `Set-Section`: replaces (or adds, before 关联 or 来源) one section of a Card body.
pub fn set_section(body: &str, heading: &str, content: &str) -> String {
    let mut lines: Vec<String> = body.replace('\r', "").split('\n').map(str::to_string).collect();
    let h = regex::escape(heading);
    let start_re = Regex::new(&format!(r"(?i)^\s*(\*\*{h}\*\*|#+\s*{h}|{h}[：:])")).unwrap();
    let mut new: Vec<String> = content.split('\n').map(str::to_string).collect();
    new.push(String::new());
    let Some(start) = lines.iter().position(|l| start_re.is_match(l)) else {
        let at = lines.iter().position(|l| LINKS_OR_SOURCE.is_match(l)).unwrap_or(lines.len());
        lines.splice(at..at, new);
        return lines.join("\n").trim().to_string();
    };
    let end = (start + 1..lines.len()).find(|&i| NEXT_HEAD.is_match(&lines[i])).unwrap_or(lines.len());
    lines.splice(start..end, new);
    lines.join("\n").trim().to_string()
}

/// `Get-Section`: one section of a Card body (its heading line through the line before the next
/// heading), as `set_section` finds it; `None` when the body has no such section.
pub fn get_section(body: &str, heading: &str) -> Option<String> {
    let lines: Vec<String> = body.replace('\r', "").split('\n').map(str::to_string).collect();
    let h = regex::escape(heading);
    let start_re = Regex::new(&format!(r"(?i)^\s*(\*\*{h}\*\*|#+\s*{h}|{h}[：:])")).unwrap();
    let start = lines.iter().position(|l| start_re.is_match(l))?;
    let end = (start + 1..lines.len()).find(|&i| NEXT_HEAD.is_match(&lines[i])).unwrap_or(lines.len());
    Some(lines[start..end].join("\n").trim().to_string())
}

pub fn is_setting_facet(facet: &str) -> bool {
    SETTING_FACET.is_match(facet)
}

// --- explain ------------------------------------------------------------------------------------

/// `Get-SectionText`: a section without the position line and 待写.
pub fn section_text(sections: &[Section], head: &str) -> String {
    let Some(sec) = sections.iter().find(|x| ps::eq(&x.head, head)) else { return String::new() };
    sec.lines
        .iter()
        .filter(|l| !l.trim().is_empty() && !ps::eq(l.trim(), "待写") && !POSITION.is_match(l.trim()))
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// `Test-Explained`: both 思路 and 解读 say something.
pub fn explained(body: &str) -> bool {
    let sections = split_sections(body);
    !section_text(&sections, "思路").is_empty() && !section_text(&sections, "解读").is_empty()
}

/// The first authored line of 思路, position line included: `section_text` drops that line, so
/// a stale chain placement is only visible here.
pub fn position_of(body: &str) -> String {
    let sections = split_sections(body);
    sections
        .iter()
        .find(|sec| ps::eq(&sec.head, "思路"))
        .and_then(|sec| {
            sec.lines
                .iter()
                .map(|line| line.trim())
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default()
}

/// `Get-Core`: what explain-apply must never change, line by line.
pub fn core(body: &str) -> String {
    let mut lines = Vec::new();
    for sec in split_sections(body) {
        let explanation = ps::is_in(&sec.head, &["思路", "解读"]);
        for l in &sec.lines {
            let t = l.trim();
            if t.is_empty() {
                continue;
            }
            if explanation {
                if SOURCE.is_match(t) {
                    lines.push(t.to_string());
                }
                continue;
            }
            if !NOTE.is_match(t) {
                lines.push(t.to_string());
            }
        }
    }
    lines.join("\n")
}

/// `$notes[$n]`: an object by key, an array by index.
fn note_at(notes: &Value, n: &str) -> String {
    match notes {
        Value::Object(map) => map.get(n).map(s).unwrap_or_default(),
        Value::Array(items) => n.parse::<usize>().ok().and_then(|i| items.get(i)).map(s).unwrap_or_default(),
        _ => String::new(),
    }
}

/// `Add-Explanation`: the body with the new 思路 (after the position line),
/// 说明 lines, and 解读; a 来源 line at the end of 结果 stays last.
pub fn add_explanation(body: &str, idea: &str, notes: &Value, meaning: &str, position: &str) -> String {
    let mut sections: Vec<Section> = split_sections(body).into_iter().filter(|sec| !ps::is_in(&sec.head, &["思路", "解读"])).collect();
    if let Some(steps) = sections.iter_mut().find(|sec| ps::eq(&sec.head, "步骤")) {
        let mut out = Vec::new();
        for line in &steps.lines {
            if NOTE.is_match(line.trim()) {
                continue;
            }
            out.push(line.clone());
            if truthy(notes)
                && let Some(c) = STEP_NUMBER.captures(line)
            {
                let note = note_at(notes, &c[1]);
                if !note.trim().is_empty() {
                    out.push(format!("   说明：{}", NOTE_LABEL.replace(note.trim(), "")));
                }
            }
        }
        steps.lines = out;
    }
    let idea_lines: Vec<String> = [position, idea.trim()].iter().filter(|x| !x.is_empty()).map(|x| x.to_string()).collect();
    let mut at = sections.iter().position(|sec| ps::eq(&sec.head, "起点")).unwrap_or(0);
    if at == 0 && !sections.is_empty() && sections[0].head.is_empty() {
        at = 1;
    }
    sections.insert(at, Section { head: "思路".into(), lines: idea_lines });
    let result = sections.iter().rposition(|sec| ps::eq(&sec.head, "结果"));
    let mut tail = Vec::new();
    if let Some(r) = result {
        let keep = &mut sections[r].lines;
        while keep.last().is_some_and(|l| l.trim().is_empty()) {
            keep.pop();
        }
        if keep.last().is_some_and(|l| SOURCE.is_match(l.trim())) {
            tail.push(keep.pop().unwrap_or_default());
        }
    }
    let at = result.map_or(sections.len(), |r| r + 1);
    sections.insert(at, Section { head: "解读".into(), lines: vec![meaning.trim().to_string()] });
    if !tail.is_empty() {
        sections.insert(at + 1, Section { head: String::new(), lines: tail });
    }
    join_sections(&sections)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn chain_lines_name_the_results() {
        let tos = vec!["$$P\\le L$$".to_string(), "$$L\\ge K-w \\quad (5.10)$$".to_string()];
        assert_eq!(chain_line(1, 2, &tos), "第 1 张，共 2 张。全链依次推出 $P\\le L$、$L\\ge K-w \\quad (5.10)$。");
        assert_eq!(chain_line(2, 2, &tos), "第 2 张，共 2 张：从 $P\\le L$ 推到 $L\\ge K-w \\quad (5.10)$。");
        assert_eq!(chain_line(1, 1, &tos), "");
        assert_eq!(result_formula("先 $a$ 再 $$b$$ 和 $$c$$"), "$$c$$");
        assert_eq!(result_formula("没有式子的一句话。第二句"), "没有式子的一句话");
        assert_eq!(result_formula(&"长".repeat(40)), format!("{}…", "长".repeat(30)));
    }

    #[test]
    fn chain_line_replaces_the_old_position() {
        let body = "**思路**\n第 1 张，共 2 张。旧。\n先求还款。\n\n**起点**\n- a\n\n**结果**\n$$x$$\n来源：书";
        let new = set_chain_line(body, "第 1 张，共 3 张。新。");
        assert_eq!(new, "**思路**\n第 1 张，共 3 张。新。\n先求还款。\n\n**起点**\n- a\n\n**结果**\n$$x$$\n来源：书");
        assert_eq!(result_of(body), "$$x$$");
        assert_eq!(set_chain_line("没有思路", "x"), "没有思路");
    }

    #[test]
    fn the_position_line_stays_visible_even_when_the_explanation_is_written() {
        let body = "**思路**\n第 2 张，共 3 张：从 a 推到 b。\n把上界代入。\n\n**起点**\n- 依赖上一步。\n\n**结果**\n$$L$$\n\n**解读**\n够大才借得到钱。";
        assert_eq!(position_of(body), "第 2 张，共 3 张：从 a 推到 b。");
        assert!(explained(body));
        // A part without 思路 or with only 待写 has no authored position line.
        assert_eq!(position_of("**起点**\n- a\n\n**结果**\n$$L$$"), "");
        assert_eq!(position_of("**思路**\n待写\n\n**起点**\n- a"), "待写");
        assert!(!explained("**思路**\n待写\n\n**起点**\n- a\n\n**结果**\n$$L$$\n\n**解读**\n待写"));
    }

    #[test]
    fn set_section_replaces_or_inserts_before_links() {
        let body = "问题：x\n\n来源：书";
        assert_eq!(set_section(body, "结构", "**结构**\n\n- a"), "问题：x\n\n**结构**\n\n- a\n\n来源：书");
        let again = set_section("问题：x\n\n**结构**\n\n- old\n\n**关联**\n- y", "结构", "**结构**\n\n- new");
        assert_eq!(again, "问题：x\n\n**结构**\n\n- new\n\n**关联**\n- y");
        // A 来源 line with text after the colon is no heading: the section runs to the end.
        assert_eq!(set_section("问题\n\n**结构**\n- a\n\n来源：书", "结构", ""), "问题");
        assert_eq!(set_section("问题\n\n**结构**\n- a\n\n来源：\n书", "结构", ""), "问题\n\n\n\n来源：\n书");
    }

    #[test]
    fn explanations_go_in_without_touching_the_core() {
        let body = "**起点**\n- 依赖上一步。\n\n**步骤**\n1. c\n   说明：旧\n\n**结果**\n$$L$$\n\n来源：原书";
        let new = add_explanation(body, "把上界代入。", &json!({ "1": "说明：代入上一张的结果。" }), "够大才借得到钱。", "第 2 张，共 2 张：从 a 推到 b。");
        assert_eq!(
            new,
            "**思路**\n第 2 张，共 2 张：从 a 推到 b。\n把上界代入。\n\n**起点**\n- 依赖上一步。\n\n**步骤**\n1. c\n   说明：代入上一张的结果。\n\n**结果**\n$$L$$\n\n**解读**\n够大才借得到钱。\n\n来源：原书"
        );
        assert_eq!(core(&new), core(body));
        assert!(explained(&new));
        assert!(!explained(body));
    }

    #[test]
    fn tables_and_words() {
        let plan = json!({ "symbols": [["$L$", "清算价值"]], "terms": [["清算", "卖资产", "变卖资产"]] });
        assert_eq!(
            tables(&plan),
            "符号表\r\n\r\n| 符号 | 含义 | 首次出现 |\r\n| --- | --- | --- |\r\n| $L$ | 清算价值 | 本卡 |\r\n\r\n术语表\r\n\r\n| 术语 | 不用 |\r\n| --- | --- |\r\n| 清算 | 卖资产、变卖资产 |"
        );
        assert_eq!(link_word("Rests"), "依赖");
        assert_eq!(link_word("特例"), "特例");
    }
}
