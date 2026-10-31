//! The Cited Card rules (#218): C1, C2, C4, C5 and C7.
//!
//! A Card of type `cited` states the cited literature of one book. A Cited list Card holds the
//! five-column table of the works the book cites (`编号`, `文献`, `年份`, `引用处`, `关系`); a Cited
//! Card states one work: its `文献：` line, three written sections, its `引用处` lines and its
//! source line. Neither is a claim, so the Idea-card title, size and sentence rules leave them
//! alone (a Cited Card reads the Literature-note rows); R6, R22 and these rules apply. C3 reads
//! the source file and C6 is R6, so both live elsewhere.

use serde_json::Value;

use crate::core::Faults;

/// The relation words a Cited Card may name in its `关系：` line (proposal D10).
pub const RELATIONS: &[&str] = &["依赖", "推广", "对照", "反驳", "印证", "参照", "背景"];

/// The three written sections, in order.
pub const SECTIONS: &[&str] = &["为什么引用", "在本书研究中的作用", "本书与该文献的关系"];

/// The most sentences one written section may hold.
pub const MAX_SECTION_SENTENCES: usize = 4;

/// The columns of a Cited list table, in order.
pub const COLUMNS: &[&str] = &["编号", "文献", "年份", "引用处", "关系"];

/// Whether a `cited` Card's body is a Cited list: it holds a table whose header names `编号`
/// and `文献`.
pub fn is_list_body(body: &str) -> bool {
    body.lines().any(|line| {
        let line = line.trim();
        line.starts_with('|') && cells(line).iter().any(|c| c == "编号") && cells(line).iter().any(|c| c == "文献")
    })
}

fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|cell| cell.trim().to_string()).collect()
}

/// The script-written lines that close a Cited Card body: they end the last written section.
const CLOSING: &[&str] = &["来源：", "本库中的这篇文献："];

/// The text of one `## <name>` section: the lines after its heading up to the next heading or a
/// closing line the script writes (`来源：`, `本库中的这篇文献：`).
pub fn section<'a>(body: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let mut lines = body.lines();
    lines.find(|line| line.trim().trim_start_matches('#').trim() == name && line.trim().starts_with("## "))?;
    Some(
        lines
            .take_while(|line| {
                let line = line.trim_start();
                !line.starts_with("## ") && !CLOSING.iter().any(|label| line.starts_with(label))
            })
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect(),
    )
}

/// The value of one head line `<label>：<value>` at the start of the body.
pub fn head_line<'a>(body: &'a str, label: &str) -> Option<&'a str> {
    body.lines()
        .map(str::trim)
        .find_map(|line| line.strip_prefix(label).and_then(|rest| rest.strip_prefix('：')))
        .map(str::trim)
}

/// The sentences of a section: split after 。！？ (a last piece without one counts too).
fn sentences(lines: &[&str]) -> usize {
    let text = lines.join("");
    text.split(['。', '！', '？'])
        .filter(|piece| !piece.trim().is_empty())
        .count()
}

/// Every `[[…]]` macro in a text that is not a Term link (`[[label@address]]`).
fn plain_links(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("[[") {
        let after = &rest[start + 2..];
        let Some(end) = after.find("]]") else { break };
        let target = &after[..end];
        if !target.contains('@') {
            found.push(target.to_string());
        }
        rest = &after[end + 2..];
    }
    found
}

/// The C rules for one `cited` Card entry. `cited_entry`, when the caller supplies it, is the
/// list row's `文献` cell that the Card's `文献：` line must equal (C4).
pub(crate) fn rules(card: &Value, address: &str, faults: &mut Faults) {
    let body = card.get("body").and_then(Value::as_str).unwrap_or_default();
    if is_list_body(body) {
        list_rules(body, address, faults);
        return;
    }
    // C1: the three sections, each 1–4 sentences.
    for name in SECTIONS {
        match section(body, name) {
            None => faults.add(
                address,
                "C1",
                "missing_section",
                name,
                &format!("the Cited Card has no `## {name}` section"),
            ),
            Some(lines) => {
                let count = sentences(&lines);
                if count == 0 || count > MAX_SECTION_SENTENCES {
                    faults.add(
                        address,
                        "C1",
                        "section_size",
                        &format!("{name}:{count}"),
                        &format!("`## {name}` holds {count} sentence(s); write 1–{MAX_SECTION_SENTENCES}"),
                    );
                }
            }
        }
    }
    // C2: the relation words come from the fixed set and each one is named in the relation section.
    match head_line(body, "关系") {
        None => faults.add(address, "C2", "missing_relation", "关系", "the Cited Card has no `关系：` line"),
        Some(value) => {
            let named = section(body, "本书与该文献的关系").unwrap_or_default().join("");
            for word in value.split(['、', ',', '，', ' ']).map(str::trim).filter(|w| !w.is_empty()) {
                if !RELATIONS.contains(&word) {
                    faults.add(
                        address,
                        "C2",
                        "unknown_relation",
                        word,
                        &format!("`{word}` is not a relation word; use {}", RELATIONS.join("、")),
                    );
                } else if !named.contains(word) {
                    faults.add(
                        address,
                        "C2",
                        "relation_unstated",
                        word,
                        &format!("`## 本书与该文献的关系` does not name the relation `{word}`"),
                    );
                }
            }
        }
    }
    // C4: the reference line, equal to the list row's entry when the caller supplies it.
    match head_line(body, "文献") {
        None => faults.add(address, "C4", "missing_reference", "文献", "the Cited Card has no `文献：` line"),
        Some(value) => {
            if let Some(entry) = card.get("cited_entry").and_then(Value::as_str)
                && value != entry.trim()
            {
                faults.add(
                    address,
                    "C4",
                    "reference_changed",
                    value,
                    &format!("the `文献：` line differs from the list row `{entry}`"),
                );
            }
        }
    }
    // C5: the written sections carry no Link of their own; the script adds every Link.
    for name in SECTIONS {
        let text = section(body, name).unwrap_or_default().join("\n");
        for target in plain_links(&text) {
            faults.add(
                address,
                "C5",
                "written_link",
                &format!("{name}:{target}"),
                &format!("`## {name}` holds the Link [[{target}]]; the script adds Links"),
            );
        }
    }
}

/// C7: the Cited list table has the five columns and rows numbered 1..N in order.
fn list_rules(body: &str, address: &str, faults: &mut Faults) {
    let mut lines = body.lines().map(str::trim).filter(|line| line.starts_with('|'));
    let Some(header) = lines.next() else { return };
    let names = cells(header);
    if names != COLUMNS {
        faults.add(
            address,
            "C7",
            "list_columns",
            &names.join("|"),
            &format!("the Cited list columns are `{}`; write `{}`", names.join(" | "), COLUMNS.join(" | ")),
        );
        return;
    }
    let mut expected = 1usize;
    for line in lines {
        let row = cells(line);
        if row.iter().all(|cell| cell.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
            continue;
        }
        if row.len() != COLUMNS.len() {
            faults.add(
                address,
                "C7",
                "row_width",
                line,
                &format!("a Cited list row has {} cells, not {}: {line}", row.len(), COLUMNS.len()),
            );
        }
        if row.first().and_then(|cell| cell.parse::<usize>().ok()) != Some(expected) {
            faults.add(
                address,
                "C7",
                "row_number",
                &format!("{expected}:{line}"),
                &format!("row {expected} is numbered `{}`", row.first().cloned().unwrap_or_default()),
            );
        }
        expected += 1;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::core::{Check, Limits};

    const GOOD: &str = "文献：Deming, D. J. (2017). The Growing Importance of Social Skills in the Labor Market. QJE.\n引用处：§1、§5（共 2 处）\n关系：依赖、推广\n\n## 为什么引用\n\n本书在引言中引用这篇文献，用来说明社交技能在劳动力市场中的回报上升，而高管层面缺少同样直接的证据；这一点是本书要补上的空白。\n\n## 在本书研究中的作用\n\n这篇文献给出了社交技能降低协调成本的思路。本书的模型依赖这一思路。\n\n## 本书与该文献的关系\n\n本书依赖它的任务交换思想，并把它推广到高管与工人之间的纵向沟通。\n\n## 引用处\n\n- §1（第 52 行）：本书用它说明社交技能回报上升。另见 [[Test2026/1|f]]\n\n来源：[[Test2026]] 参考文献第 14 条";

    const LIST: &str = "本卡是《Test》的参考文献表。\n\n| 编号 | 文献 | 年份 | 引用处 | 关系 |\n| --- | --- | --- | --- | --- |\n| 1 | A. Title. | 2017 | §1 | 依赖 |\n| 2 | B. Other. | 2009 | 仅参考文献 | 背景 |\n\n来源：[[Test2026]] 参考文献";

    fn entry(body: &str) -> Value {
        json!({
            "address": "Test2026/1|b|n",
            "type": "cited",
            "category": "cited",
            "tree": "Test2026",
            "title": "Deming（2017）：劳动力市场中社交技能的重要性上升",
            "body": body,
        })
    }

    fn lines(cards: &[Value]) -> Vec<String> {
        Check::new(Limits::default(), false, "").run(cards)
    }

    fn of(lines: &[String], rule: &str) -> Vec<String> {
        lines.iter().filter(|line| line.split(' ').nth(1) == Some(rule)).cloned().collect()
    }

    #[test]
    fn a_well_formed_cited_card_has_no_fault_even_with_long_sentences_and_a_semicolon() {
        let found = lines(&[entry(GOOD)]);
        assert!(found.is_empty(), "{found:#?}");
    }

    #[test]
    fn c1_names_a_missing_section_and_a_section_of_five_sentences() {
        let body = GOOD.replace("## 为什么引用", "## 引用原因");
        let found = lines(&[entry(&body)]);
        assert_eq!(of(&found, "C1").len(), 1, "{found:#?}");
        assert!(found[0].contains("## 为什么引用"), "{found:#?}");
        let body = GOOD.replace("这篇文献给出了社交技能降低协调成本的思路。", "一。二。三。四。");
        let found = lines(&[entry(&body)]);
        assert!(of(&found, "C1").iter().any(|line| line.contains("5 sentence")), "{found:#?}");
    }

    #[test]
    fn c2_refuses_a_word_outside_the_set_and_a_word_the_section_does_not_name() {
        let found = lines(&[entry(&GOOD.replace("关系：依赖、推广", "关系：依赖、补充"))]);
        assert!(of(&found, "C2").iter().any(|line| line.contains("`补充` is not a relation word")), "{found:#?}");
        let found = lines(&[entry(&GOOD.replace("关系：依赖、推广", "关系：依赖、对照"))]);
        assert!(of(&found, "C2").iter().any(|line| line.contains("`对照`")), "{found:#?}");
    }

    #[test]
    fn c4_needs_the_reference_line_and_keeps_it_equal_to_the_supplied_row() {
        let found = lines(&[entry(&GOOD.replace("文献：", "出处："))]);
        assert_eq!(of(&found, "C4").len(), 1, "{found:#?}");
        let mut card = entry(GOOD);
        card["cited_entry"] = json!("Deming, D. J. (2017). The Growing Importance of Social Skills in the Labor Market. QJE, 132(4).");
        let found = lines(&[card]);
        assert!(of(&found, "C4").iter().any(|line| line.contains("differs from the list row")), "{found:#?}");
        let mut card = entry(GOOD);
        card["cited_entry"] = json!("Deming, D. J. (2017). The Growing Importance of Social Skills in the Labor Market. QJE.");
        assert!(of(&lines(&[card]), "C4").is_empty());
    }

    #[test]
    fn c5_refuses_a_written_link_but_keeps_a_term_link_and_the_script_links() {
        let body = GOOD.replace("本书依赖它的任务交换思想", "本书依赖 [[Test2026/9|a]] 的任务交换思想");
        let found = lines(&[entry(&body)]);
        assert_eq!(of(&found, "C5").len(), 1, "{found:#?}");
        let body = GOOD.replace("本书依赖它的任务交换思想", "本书依赖它的[[任务交换@Test2026/1|a|c]]思想");
        assert!(of(&lines(&[entry(&body)]), "C5").is_empty());
    }

    #[test]
    fn the_last_section_ends_before_the_script_lines_that_follow_it() {
        let body = GOOD.replace("## 引用处

- §1（第 52 行）：本书用它说明社交技能回报上升。另见 [[Test2026/1|f]]

", "")
            + "
本库中的这篇文献：[[Deming2017]]";
        assert!(!body.contains("## 引用处"));
        let found = lines(&[entry(&body)]);
        assert!(of(&found, "C5").is_empty(), "{found:#?}");
    }

    #[test]
    fn c7_reads_the_list_columns_and_the_row_numbers() {
        let mut list = entry(LIST);
        list["title"] = json!("参考文献：《Test》引用的 2 篇文献");
        let found = lines(&[list.clone()]);
        assert!(found.is_empty(), "a well-formed list has no fault: {found:#?}");
        list["body"] = json!(LIST.replace("| 编号 | 文献 | 年份 | 引用处 | 关系 |", "| 编号 | 文献 | 年份 |"));
        assert_eq!(of(&lines(&[list.clone()]), "C7").len(), 1);
        list["body"] = json!(LIST.replace("| 2 | B. Other.", "| 3 | B. Other."));
        let found = lines(&[list]);
        assert!(of(&found, "C7").iter().any(|line| line.contains("row 2 is numbered `3`")), "{found:#?}");
    }
}
