//! `zt-digest model-context` (model-context.ps1): the context pack one model
//! elaborator starts from. Reads only.
//!
//!   zt-digest model-context -Staging <folder> -Model <address> -Out <file.md>
//!
//! The pack holds the Model Card, its parts and relations, its neighbours, the
//! Cards that link it and that it links, the book group that wrote it (from
//! owned-gNN.txt in -Staging, else the anchor it is filed under) with the
//! section files and their headings, the anchor Card, and the book's other
//! models. Prints one compact JSON line:
//! {"model","group","anchor","sections","parts","context"}.

use crate::ps::{self, each, pipe, s, truthy};
use super::read_calls;
use crate::args::Args;
use crate::util;
use crate::zt::Zt;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

static GROUP_OF_ADDRESS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[^/]+/(\d+)(\||$)").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,4}\s").unwrap());

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Staging", "Model", "Out"], &[])?;
    let staging = a.required("Staging")?;
    let model = a.required("Model")?;
    let out = a.required("Out")?;
    let mut zt = crate::zt::start()?;
    let summary = build(&mut zt, &staging, &model, &out)?;
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

/// `$group -eq $n`-style match of a manifest group number.
fn same_number(v: &Value, n: i64) -> bool {
    match v {
        Value::Number(x) => x.as_f64() == Some(n as f64),
        Value::String(text) => ps::eq(text, &n.to_string()),
        Value::Bool(flag) => i64::from(*flag) == n,
        _ => false,
    }
}

/// Writes the pack to `out` and returns the summary line's object.
pub fn build(zt: &mut Zt, staging: &str, model: &str, out: &str) -> Result<Value> {
    // Mandatory parameters refuse an empty string, as PowerShell's binder does.
    for (name, value) in [("Staging", staging), ("Model", model), ("Out", out)] {
        if value.is_empty() {
            bail!("Cannot bind argument to parameter '{name}' because it is an empty string.");
        }
    }
    let key = model.split('/').next().unwrap_or_default().to_string();
    let answers = read_calls(
        zt,
        &[
            ("get_card", json!({ "at": model })),
            ("get_model", json!({ "at": model })),
            // The book's own tree, not search_cards: a search stops at 200 results.
            ("get_tree", json!({ "tree": key })),
        ],
    )?;
    let card = answers.first().cloned().unwrap_or(Value::Null);
    let structure = answers.get(1).cloned().unwrap_or(Value::Null);
    let found = answers.get(2).cloned().unwrap_or(Value::Null);

    // Which group wrote the Model Card.
    let mut group: Option<i64> = None;
    for name in ps::child_files(staging, "owned-g", ".txt") {
        let path = ps::join(staging, &name);
        let owned: Vec<String> = ps::read_lines(&path)?.iter().map(|l| l.trim().to_string()).collect();
        if ps::is_in(model, &owned) {
            let stem = &name[..name.len() - 4];
            group = Some(ps::int(&Value::String(Regex::new("(?i)^owned-g").unwrap().replace(stem, "").into_owned()))?);
            break;
        }
    }
    if group.unwrap_or(0) == 0
        && let Some(c) = GROUP_OF_ADDRESS.captures(model)
    {
        group = Some(ps::int(&Value::String(c[1].to_string()))?);
    }
    let anchor = group.filter(|g| *g != 0).map(|g| format!("{key}/{g}"));

    let manifest_file = ps::join(staging, "manifest.json");
    let manifest = if ps::exists(&manifest_file) { Some(util::read_json(&manifest_file)?) } else { None };
    let entry = match (&manifest, group.filter(|g| *g != 0)) {
        (Some(m), Some(g)) => pipe(&m["groups"]).into_iter().find(|e| same_number(&e["n"], g)).cloned(),
        _ => None,
    };
    let anchor_card = match &anchor {
        Some(anchor) => read_calls(zt, &[("get_card", json!({ "at": anchor }))]).ok().and_then(|v| v.into_iter().next()),
        None => None,
    };

    let mut md = String::new();
    let mut line = |text: &str| {
        md.push_str(text);
        md.push_str("\r\n");
    };
    line(&format!("# 模型上下文：[[{model}]]"));
    line("");
    let level = s(&structure["level"]);
    let level_name = if ps::eq(&level, "concept") && !structure["level"].is_null() {
        "第 1 层，概念模型（concept）"
    } else if ps::eq(&level, "formal") && !structure["level"].is_null() {
        "第 2 层，形式模型（formal）"
    } else {
        "未定（按模型说明第 0 步决定）"
    };
    line(&format!("细化层级：{level_name}"));
    line("");
    line("## 模型卡（DeepSeek 写的原稿，需要对照原文核实）");
    line("");
    line(&format!("**{}**", s(&card["title"])));
    line("");
    line(s(&card["body"]).trim());
    line("");
    if !pipe(&structure["parts"]).is_empty() {
        line("## 已有的模型分卡");
        line("");
        for part in each(&structure["parts"]) {
            let filed_elsewhere = ps::is_false(&part["filed"]);
            let place = if filed_elsewhere { "（归档在别处，由模型卡的 Link 纳入）" } else { "" };
            let also = pipe(&part["also"]).iter().map(|x| format!("[[{}]]", s(&x["address"]))).collect::<Vec<_>>().join("、");
            let emphasis = if truthy(&part["emphasis"]) { format!(" ({})", s(&part["emphasis"])) } else { String::new() };
            let also = if also.is_empty() { String::new() } else { format!("（也属于 {also}）") };
            line(&format!("- [[{}]] `{}`{emphasis}{place}{also}：{}", s(&part["address"]), s(&part["facet"]), s(&part["title"])));
        }
        line("");
        if !pipe(&structure["relations"]).is_empty() {
            line("已有的关系（类型取自 Link 紧前的词；refers 表示没有关系词）：");
            line("");
            for relation in each(&structure["relations"]) {
                line(&format!(
                    "- [[{}]] → [[{}]] `{}`：{}",
                    s(&relation["from"]),
                    s(&relation["to"]),
                    s(&relation["kind"]),
                    s(&relation["note"])
                ));
            }
            line("");
        }
    }
    let neighbours = pipe(&structure["neighbours"]);
    if !neighbours.is_empty() {
        line("## 模型外的邻居（与模型的卡互相链接的卡，带关系类型）");
        line("");
        for n in neighbours {
            line(&format!(
                "- [[{}]] → [[{}]] `{}`（{}）：{}",
                s(&n["from"]),
                s(&n["to"]),
                s(&n["kind"]),
                s(&n["card"]["title"]),
                s(&n["note"])
            ));
        }
        line("");
    }
    let part_addresses: Vec<String> = pipe(&structure["parts"]).iter().map(|p| s(&p["address"])).collect();
    let inbound: Vec<&Value> =
        pipe(&card["inbound"]).into_iter().filter(|r| !ps::is_in(&s(&r["address"]), &part_addresses)).filter(|r| !r.is_null()).collect();
    if !inbound.is_empty() {
        line("## 讨论这个模型的卡片（链接到它）");
        line("");
        for r in inbound {
            line(&format!("- [[{}]]（{}）{}", s(&r["address"]), s(&r["workspace"]), s(&r["title"])));
        }
        line("");
    }
    let outbound: Vec<&Value> = pipe(&card["outbound"]).into_iter().filter(|r| !truthy(&r["broken"])).filter(|r| !r.is_null()).collect();
    if !outbound.is_empty() {
        line("## 模型卡链接的卡片（改写时保留这些链接）");
        line("");
        for r in outbound {
            line(&format!("- [[{}]]（{}）{}", s(&r["address"]), s(&r["workspace"]), s(&r["title"])));
        }
        line("");
    }

    line("## 来源");
    line("");
    let body = s(&card["body"]);
    let sources: Vec<&str> = body.split('\n').filter(|l| l.contains("来源")).collect();
    if !sources.is_empty() {
        line(&format!("模型卡的来源行：{}", sources.join(" / ")));
        line("");
    }
    let g = group.unwrap_or(0);
    let anchor_text = anchor.clone().unwrap_or_default();
    if let Some(entry) = &entry {
        let headings = pipe(&entry["headings"]).iter().map(|h| s(h)).collect::<Vec<_>>().join("；");
        line(&format!("第 {g} 组（锚点 [[{anchor_text}]]），{}；小节：{headings}。", s(&entry["chapter"])));
        line("");
        line(&format!("DeepSeek 写这张模型卡时读的就是这一组的原文：`{}`。小节文件与其中的标题（行号: 标题）：", s(&entry["file"])));
        line("");
        for name in pipe(&entry["sections"]) {
            let path = ps::join(staging, &format!("sections\\{}", s(name)));
            line(&format!("- `{path}`"));
            if ps::exists(&path) {
                for (n, text) in ps::read_lines(&path)?.iter().enumerate() {
                    if HEADING.is_match(text) {
                        line(&format!("  - {}: {}", n + 1, text.trim()));
                    }
                }
            }
        }
        line("");
    } else if g != 0 {
        line(&format!(
            "第 {g} 组（锚点 [[{anchor_text}]]）。暂存目录里没有 manifest.json：用 Select-String 在 `{staging}\\sections\\*.md` 或 `{staging}\\book.zh.md` 里找来源小节；若暂存目录不在，用 scholar_archive_* 工具读原书。"
        ));
        line("");
    } else {
        line(&format!("找不到这张模型卡所属的组：按来源行在 `{staging}\\book.zh.md` 里检索，或用 scholar_archive_* 工具读原书。"));
        line("");
    }
    let index = match &manifest {
        Some(m) => s(&m["index"]),
        None => ps::join(staging, "sections\\index.md"),
    };
    let source = match &manifest {
        Some(m) => s(&m["source"]),
        None => String::new(),
    };
    for (label, path) in [("全书索引", index), ("全书合并文本", ps::join(staging, "book.zh.md")), ("双语原文（只读）", source.clone())] {
        if !path.is_empty() && ps::exists(&path) {
            line(&format!("- {label}：`{path}`"));
        }
    }
    line("");
    // The chapter's endnotes: the definitions a model may need but the digest range leaves out
    // (R310). Read-only reference text, with the chapter's missing notes kept visible.
    let chapter = entry.as_ref().map(|e| s(&e["chapter"])).unwrap_or_default();
    let notes_block = if chapter.is_empty() {
        None
    } else {
        manifest
            .as_ref()
            .and_then(|m| m["notes"]["chapters"].as_array())
            .and_then(|list| list.iter().find(|c| ps::eq(&s(&c["chapter"]), &chapter) && c["present"] == true))
            .and_then(|record| crate::prepare::notes::reference_block(record, &source))
    };
    if let Some(block) = notes_block {
        for text in block.lines() {
            line(text);
        }
    }

    if let Some(anchor_card) = &anchor_card {
        line(&format!("## 本组锚点卡 [[{anchor_text}]]：这一部分在全书中的位置"));
        line("");
        line(&format!("**{}**", s(&anchor_card["title"])));
        line("");
        let mut body = s(&anchor_card["body"]).trim().to_string();
        if ps::len16(&body) > 1600 {
            body = ps::cut16(&body, 1600) + "……";
        }
        line(&body);
        line("");
    }

    let others: Vec<&Value> = pipe(&found["cards"])
        .into_iter()
        .filter(|c| ps::eq(&s(&c["type"]), "model") && !c["type"].is_null() && !truthy(&c["facet"]) && !ps::eq(&s(&c["address"]), model))
        .collect();
    if !others.is_empty() {
        line("## 本书的其他模型（可链接；不要重复它们的内容）");
        line("");
        for other in others {
            let level = if truthy(&other["level"]) { format!(" ({})", s(&other["level"])) } else { String::new() };
            line(&format!("- [[{}]] {}{level}", s(&other["address"]), s(&other["title"])));
        }
        line("");
    }

    util::write_text(out, &md)?;
    let sections = match &entry {
        Some(e) => pipe(&e["sections"]).into_iter().cloned().collect::<Vec<_>>(),
        None => Vec::new(),
    };
    Ok(json!({
        "model": model,
        "group": group,
        "anchor": anchor,
        "sections": sections,
        "parts": pipe(&structure["parts"]).len(),
        "context": out,
    }))
}
