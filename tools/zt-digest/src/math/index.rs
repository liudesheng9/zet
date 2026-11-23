//! The statement finder's job text (owner, 2026-11-21: DeepSeek finds the statements, no pattern
//! does). A DeepSeek job reads each prepared section with its line numbers and answers with the
//! fragments of answer schema 2; `source.rs` checks each answer against its packet and joins them
//! into the ownership projection (docs/math-digest-p1a-design.md §2). This file holds the prepared
//! section type, the kind words and the job text.

/// The six unit forms: a definition or a result that becomes a unit head.
pub const UNIT_FORMS: &[&str] = &["definition", "axiom", "theorem", "proposition", "lemma", "corollary"];
/// The fewest characters of a `quote` that tie a span to its text.
pub const MIN_QUOTE: usize = 4;

/// One prepared section a job reads: its file name, group, chapter and lines.
#[derive(Clone)]
pub struct Section {
    pub file: String,
    pub group: i64,
    pub chapter: String,
    pub lines: Vec<String>,
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
        "remark" => "注",
        "notation" => "记号",
        _ => "",
    }
}

/// The job text of one finder job: each section's lines, numbered as the answer must cite them.
pub fn job_text(title: &str, id: &str, out: &str, check: &str, sections: &[Section]) -> String {
    let mut sb = format!("# 找出数学对象：任务 {id}，《{title}》\n\n");
    sb.push_str(&format!("把结果写入 `{out}`。只读本文件，不读其他文件。\n\n"));
    sb.push_str(&format!("SELF-CHECK: {check}\n\n"));
    sb.push_str("下面每一节是一个原文文件。每行开头是行号和 `│`，行号不属于原文。`section` 写文件名，`line`、`end` 写这里的行号；`col`、`end_col` 从行首数字符，从 0 开始。\n\n");
    for sec in sections {
        sb.push_str(&format!("## 文件 `{}`（第 {} 组，{}）\n\n", sec.file, sec.group, sec.chapter));
        for (i, l) in sec.lines.iter().enumerate() {
            sb.push_str(&format!("{}│ {}\n", i + 1, l));
        }
        sb.push('\n');
    }
    sb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sec(file: &str, group: i64, text: &str) -> Section {
        Section { file: file.into(), group, chapter: format!("第 {group} 章"), lines: text.split('\n').map(String::from).collect() }
    }

    #[test]
    fn the_job_text_numbers_every_line_and_names_the_self_check() {
        let s = [sec("s01.md", 1, "# 论微小量\n\n我们把 x 的一小部分记作 dx，读作“x 的微分”。")];
        let t = job_text("微积分", "ix-01", "D:\\o\\ix-01.json", "zt-digest math -Step index-check -Job ix-01", &s);
        assert!(t.contains("SELF-CHECK: zt-digest math -Step index-check -Job ix-01"));
        assert!(t.contains("## 文件 `s01.md`（第 1 组，第 1 章）"));
        assert!(t.contains("3│ 我们把 x 的一小部分记作 dx"));
    }

    #[test]
    fn every_kind_has_its_word() {
        assert_eq!(kind_zh("theorem"), "定理");
        assert_eq!(kind_zh("exercise"), "习题");
        assert_eq!(kind_zh("remark"), "注");
    }
}
