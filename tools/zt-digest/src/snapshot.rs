//! `zt-digest snapshot-topics`: the port of `scripts/book-digest/snapshot-topics.ps1`. Writes a
//! compact snapshot of every tree (address and title of each Card) for a DeepSeek digest run, so
//! it can place and link Cards without spending turns on searches. Read-only calls through one
//! `zt mcp`: one fresh `export_titles`. Agent trees first, then human trees (marked
//! read-only); titles are cut to -TitleChars characters. Prints `<N> cards -> <Out>`.
//!
//! `cut` and `mark` serve `prepare-link` (prepare-link-pass.ps1) too, which wrote the same lines.

use crate::args::Args;
use crate::ps::{self, CiMap, NL, eq, s};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A fresh title export per invocation. Keep the existing rendering helpers
/// and group shape so preparation can use the same compact title catalogue.
pub(crate) fn read_title_trees(zt:&mut Zt)->Result<Vec<Value>> {
    let export=zt.call("export_titles",json!({}))?;
    let titles=export["titles"].as_array().context("incomplete title export: missing titles array")?;
    if export["complete"]!=true || export["count"].as_u64()!=Some(titles.len() as u64) || !export["archive_revision"].is_u64() {
        bail!("incomplete title export: completeness, count or archive revision is missing");
    }
    let mut indices=BTreeMap::new();let mut trees:Vec<Value>=Vec::new();
    for card in titles {
        let at=card["address"].as_str().context("incomplete title export: missing address")?;
        let tree=card["tree"].as_str().context("incomplete title export: missing tree")?;
        let workspace=card["workspace"].as_str().context("incomplete title export: missing Workspace")?;
        let index=*indices.entry(tree.to_string()).or_insert_with(||{
            let index=trees.len();trees.push(json!({"tree":tree,"workspace":workspace,"cards":[]}));index
        });
        let mut row=card.clone();
        let root=at==tree || at==format!("{tree}/0");
        row["edge"]=json!(if root{"root"}else{"direct"});
        row["kind"]=json!(if at==tree{"literature"}else if root{"topic"}else{"regular"});
        trees[index]["archive_revision"]=export["archive_revision"].clone();
        if root {
            let kind=if at==tree{"literature"}else{"topic"};
            trees[index]["zone"]=json!(kind);trees[index]["kind"]=json!(kind);
        }
        trees[index]["cards"].as_array_mut().unwrap().push(row);
    }
    // The former list_trees/get_tree input followed the archive's binary
    // address order. Preserve that input order before the existing renderers
    // and ranking sorts, whose equal-score ordering depends on it.
    trees.sort_by(|a,b|s(&a["tree"]).cmp(&s(&b["tree"])));
    for tree in &mut trees {
        tree["cards"].as_array_mut().unwrap().sort_by(|a,b|s(&a["address"]).cmp(&s(&b["address"])));
    }
    Ok(trees)
}

/// `Cut`: a title of at most `chars` UTF-16 units, else cut and `…` added.
pub(crate) fn cut(text: &str, chars: usize) -> String {
    if ps::len16(text) <= chars { text.to_string() } else { format!("{}…", ps::cut16(text, chars)) }
}

/// `Mark`: ` [模型·<facet>]` for a Model part, ` [模型]` for a Model Card, ` [事件]` for an Event
/// Card. Common facets read in Chinese; any other facet is the label its author wrote.
pub(crate) fn mark(card: &Value) -> String {
    let card_type = s(&card["type"]);
    let facet = s(&card["facet"]);
    if eq(&card_type, "model") && !facet.is_empty() {
        let mut names: CiMap<&str> = CiMap::new();
        for (k, v) in [
            ("setting", "设定"),
            ("assumptions", "假设"),
            ("timing", "时序"),
            ("dynamics", "动态"),
            ("solution", "求解"),
            ("results", "结论"),
            ("intuition", "直觉"),
            ("limits", "局限"),
        ] {
            names.insert(k, v);
        }
        let name = names.get(&facet).map(|n| n.to_string()).unwrap_or(facet);
        return format!(" [模型·{name}]");
    }
    if eq(&card_type, "model") {
        " [模型]".to_string()
    } else if eq(&card_type, "event") {
        " [事件]".to_string()
    } else {
        String::new()
    }
}

/// One snapshot line: `- `<address>`<mark> <title>` (a root), else indented.
pub(crate) fn card_line(card: &Value, title_chars: usize) -> String {
    let indent = if eq(&s(&card["edge"]), "root") { "- " } else { "  - " };
    format!("{indent}`{}`{} {}", s(&card["address"]), mark(card), cut(&s(&card["title"]), title_chars))
}

/// The cards of a tree in `Sort-Object { $_.address }` order.
pub(crate) fn sorted_cards(tree: &Value) -> Vec<Value> {
    let mut cards = ps::ps_items(&tree["cards"]);
    ps::ps_sort(&mut cards, |a, b| ps::culture_cmp(&s(&a["address"]), &s(&b["address"])));
    cards
}

/// `zt-digest snapshot-topics -Out <file> [-TitleChars 40]`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Out", "TitleChars"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let out = a.required("Out")?;
    if out.is_empty() {
        bail!("Cannot bind argument to parameter 'Out' because it is an empty string.");
    }
    let title_chars = a.int_or("TitleChars", 40)?.max(0) as usize;
    let mut zt = Zt::start()?;
    let details = read_title_trees(&mut zt)?;

    let mut text = String::new();
    text.push_str("# 现有卡片快照（本组启动时生成；之后其他组可能新增卡片；[模型] 为模型卡，[模型·假设] 等为模型分卡，[事件] 为事件卡）");
    text.push_str(NL);
    text.push_str(NL);
    let mut count = 0;
    for workspace in ["agent", "human"] {
        text.push_str(if workspace == "agent" { "## 代理工作区" } else { "## 人类工作区（只读，可链接）" });
        text.push_str(NL);
        for tree in details.iter().filter(|t| eq(&s(&t["workspace"]), workspace)) {
            for card in sorted_cards(tree) {
                text.push_str(&card_line(&card, title_chars));
                text.push_str(NL);
                count += 1;
            }
        }
        text.push_str(NL);
    }
    std::fs::write(&out, text).with_context(|| format!("failed to write {out}"))?;
    util::print_line(&format!("{count} cards -> {out}"))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_and_cuts() {
        assert_eq!(mark(&json!({ "type": "model", "facet": "Setting" })), " [模型·设定]");
        assert_eq!(mark(&json!({ "type": "model", "facet": "均衡" })), " [模型·均衡]");
        assert_eq!(mark(&json!({ "type": "model", "facet": null })), " [模型]");
        assert_eq!(mark(&json!({ "type": "event" })), " [事件]");
        assert_eq!(mark(&json!({ "type": "idea" })), "");
        assert_eq!(cut("一二三", 3), "一二三");
        assert_eq!(cut("一二三四", 3), "一二三…");
        assert_eq!(card_line(&json!({ "address": "3/1", "edge": "direct", "title": "标题" }), 40), "  - `3/1` 标题");
        assert_eq!(card_line(&json!({ "address": "3/0", "edge": "root", "title": "T" }), 40), "- `3/0` T");
    }
}
