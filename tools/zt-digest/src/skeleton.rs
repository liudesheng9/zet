//! `zt-digest skeleton`: the port of `scripts/book-digest/book-skeleton.ps1`, the book's skeleton
//! (docs/digest-card-layers-proposal.md, proposal 7): the Literature Card index, the structure Card,
//! and the bodies of the book's Topics. The tool builds each frame from the manifest and the
//! trees; one DeepSeek job writes the short texts as JSON; Opus gives only the thesis.
//!
//!   zt-digest skeleton -Step plan|apply -Staging <folder> -Work <folder> [-Audit <audit.json>]
//!                      [-Thesis <text>] [-Decisions <decisions.json>] [-Apply]
//!
//! -Audit names an audit.json of the book; without it the tool runs the audit in-process into
//! <Work>\audit (the script ran digest-audit.ps1). Every step prints compact JSON as its last line.
//!
//!   plan   writes job.md (one section per group, the book's Model Cards with their levels, its
//!          Event Cards, the Topics where the book holds most Cards, and the SELF-CHECK line
//!          `zt-digest skeleton -Step apply …` without -Apply) and the instruction file.
//!   apply  reads out\skeleton.json and checks it; with -Apply writes the structure Card, the
//!          Literature Card index, and each Topic body (keeping every line that links a Hub Card).

use crate::args::Args;
use crate::organize::common::{self, Lazy, Sb, prop, pstr};
use crate::ps::{self, CiMap, eq, field, is_in};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::LazyLock;

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
static SEMICOLON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[；;]").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static BAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)does not exist|semicolon").unwrap());
static STRUCTURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^本卡是《.*》的结构卡").unwrap());
static TERM_TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
static DOCUMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\s*原书文档").unwrap());

const PROMPT: &str = "按以上说明写出骨架 JSON 文件。最后只回复一行 JSON。";
const DOC: &str = "dsh-skeleton-instructions.zh.md";

struct Params {
    step: String,
    staging: String,
    work: String,
    audit: String,
    thesis: String,
    decisions: String,
    apply: bool,
}

/// What the book holds, from the manifest and the audit.
struct Book {
    key: String,
    title: String,
    groups: Vec<Value>,
    anchors: Vec<String>,
    known: HashSet<String>,
    cards: Vec<Value>,
    models: Vec<Value>,
    events: Vec<Value>,
    topic_rows: Vec<Value>,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Staging", "Work", "Audit", "Thesis", "Decisions"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["Apply"])?;
    let step = a.choice("Step", &["plan", "apply"])?;
    let mut p = Params {
        step,
        staging: common::mandatory(&a, "Staging")?,
        work: common::mandatory(&a, "Work")?,
        audit: a.str("Audit"),
        thesis: a.str("Thesis"),
        decisions: a.str("Decisions"),
        apply: a.flag("Apply"),
    };
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    let title = pstr(&manifest, "book_title");
    let out = ps::join(&p.work, "out");
    std::fs::create_dir_all(&p.work)?;
    std::fs::create_dir_all(&out)?;
    let mut zt = Lazy::new();

    // --- what the book holds ---
    if p.audit.is_empty() {
        p.audit = ps::join(&p.work, "audit\\audit.json");
        common::run_audit(zt.get()?, &key, &ps::parent(&p.audit))?;
    }
    let audit_dir = ps::parent(&p.audit);
    let audit = common::read_json(&p.audit)?;
    let book = ps::at(&ps::pipe(&audit["books"]), 0);
    let cards = ps::ps_items(&common::read_json(&ps::join(&audit_dir, "audit-cards.json"))?);
    let mut known = HashSet::new();
    for line in ps::read_lines(&ps::join(&audit_dir, "audit-addresses.txt"))? {
        if !line.is_empty() {
            known.insert(line.trim().to_string());
        }
    }
    let groups: Vec<Value> = ps::pipe(prop(&manifest, "groups")).into_iter().cloned().collect();
    let anchors: Vec<String> = groups.iter().map(|g| format!("{key}/{}", pstr(g, "n"))).collect();
    let models: Vec<Value> = ps::ps_items(&book["models"]["list"]).into_iter().filter(|m| ps::truthy(&m["address"])).collect();
    let events: Vec<Value> = cards.iter().filter(|c| eq(&field(c, "category"), "event") && eq(&field(c, "book"), &key)).cloned().collect();
    let mut topic_rows = Vec::new();
    for t in ps::ps_items(&book["topics"]["list"]) {
        if ps::int(&t["cards"])? * 2 > ps::int(&t["size"])? {
            topic_rows.push(t);
        }
    }
    let b = Book { key, title, groups, anchors, known, cards, models, events, topic_rows };
    if p.step == "plan" { plan(&p, &b, &out, &mut zt) } else { apply(&mut p, &b, &out, &mut zt) }
}

fn level_name(level: &Value, other: &str) -> String {
    let level = ps::s(level);
    if eq(&level, "concept") {
        "概念模型".into()
    } else if eq(&level, "formal") {
        "形式模型".into()
    } else {
        other.into()
    }
}

/// `($text -replace '\s+', ' ').Trim()`.
fn one_line(text: &str) -> String {
    SPACES.replace_all(text, " ").trim().to_string()
}

/// The SELF-CHECK line of the job: apply without -Apply checks the JSON and writes nothing.
fn self_check(staging: &str, work: &str) -> String {
    format!("SELF-CHECK: `zt-digest skeleton -Step apply -Staging \"{staging}\" -Work \"{work}\"`（不要加 -Apply）")
}

fn plan(p: &Params, b: &Book, out: &str, zt: &mut Lazy) -> Result<i32> {
    let key = &b.key;
    let addresses: Vec<String> = b.anchors.iter().cloned().chain(b.topic_rows.iter().map(|t| format!("{}/0", field(t, "tree")))).collect();
    let calls: Vec<(&str, Value)> = addresses.iter().map(|a| ("get_card", json!({ "at": a }))).collect();
    let read = if calls.is_empty() { Vec::new() } else { common::reads(zt.get()?, &calls)? };
    let mut by_address: CiMap<Value> = CiMap::new();
    for c in read {
        by_address.insert(&pstr(&c, "address"), c);
    }
    let none = Value::Null;
    let mut sb = Sb::new();
    sb.line(format!("# 骨架任务：《{}》", b.title)).blank().line(format!("把结果写入 `{}`。", ps::join(out, "skeleton.json"))).blank();
    sb.line("## 各组（锚点卡）").blank();
    for g in &b.groups {
        let a = format!("{key}/{}", pstr(g, "n"));
        let c = by_address.get(&a).unwrap_or(&none);
        let mut body = one_line(&pstr(c, "body"));
        if ps::len16(&body) > 600 {
            body = ps::cut16(&body, 600) + "…";
        }
        let headings: Vec<String> = ps::pipe(prop(g, "headings")).into_iter().map(ps::s).collect();
        sb.line(format!("### `{a}`（{}：{}）", pstr(g, "chapter"), headings.join("；")))
            .blank()
            .line(format!("标题：{}", pstr(c, "title")))
            .blank()
            .line(format!("正文：{body}"))
            .blank();
    }
    sb.line("## 模型卡").blank();
    if !b.models.is_empty() {
        for m in &b.models {
            sb.line(format!("- `{}`（{}）{}", field(m, "address"), level_name(&m["level"], "未定层级"), field(m, "title")));
        }
    } else {
        sb.line("（没有）");
    }
    sb.blank().line("## 事件卡").blank();
    if !b.events.is_empty() {
        for e in &b.events {
            let targets: Vec<String> = ps::ps_items(&e["links"]).iter().take(6).map(|l| format!("`{}`", ps::s(l))).collect();
            sb.line(format!("- `{}` {}；它链接：{}", field(e, "address"), field(e, "title"), targets.join("、")));
        }
    } else {
        sb.line("（没有）");
    }
    sb.blank().line("## 本书为主的主题").blank();
    for t in &b.topic_rows {
        let root = format!("{}/0", field(t, "tree"));
        let c = by_address.get(&root).unwrap_or(&none);
        sb.line(format!("### `{root}` {}（{} 张卡）", field(t, "title"), field(t, "size")))
            .blank()
            .line(format!("现在的正文：{}", one_line(&pstr(c, "body"))))
            .blank()
            .line("树中的卡片：")
            .blank();
        let mut in_tree: Vec<&Value> = b
            .cards
            .iter()
            .filter(|x| eq(&field(x, "tree"), &field(t, "tree")) && is_in(&field(x, "category"), &["idea", "event", "model"]))
            .collect();
        ps::ps_sort(&mut in_tree, |x, y| ps::culture_cmp(&field(x, "address"), &field(y, "address")));
        for x in in_tree.into_iter().take(12) {
            sb.line(format!("- `{}` {}", field(x, "address"), field(x, "title")));
        }
        sb.blank();
    }
    sb.line(self_check(&p.staging, &p.work));
    let job = ps::join(&p.work, "job.md");
    util::write_text(&job, &sb.0)?;
    let doc = util::read_text(ps::join(&common::scripts_dir(), DOC))?;
    let doc_copy = ps::join(&p.work, DOC);
    util::write_text(&doc_copy, &doc.replace("{{KEY}}", key))?;
    let last = json!({
        "groups": b.groups.len(), "models": b.models.len(), "events": b.events.len(), "topics": b.topic_rows.len(),
        "jobs": [{ "id": "skeleton", "files": [doc_copy, job], "prompt": PROMPT }]
    });
    util::print_line(&ps::json_compact(&last))?;
    Ok(0)
}

/// `Get-Links`: the Link targets of a text, trimmed.
fn links(text: &str) -> Vec<String> {
    LINK.captures_iter(text).map(|c| c[1].trim().to_string()).collect()
}

/// An argument line of the structure Card.
struct Argument {
    anchor: String,
    adds: String,
    key: Vec<String>,
}

/// What `apply` builds from skeleton.json: the checked parts and the problems.
struct Checked {
    says: CiMap<String>,
    argument: Vec<Argument>,
    model_lines: Vec<String>,
    event_lines: Vec<String>,
    /// Topic root → its tree.
    topic_set: CiMap<String>,
    topic_bodies: CiMap<String>,
    problems: Vec<String>,
}

fn check(sk: &Value, b: &Book) -> Checked {
    let mut problems: Vec<String> = Vec::new();
    let test_text = |problems: &mut Vec<String>, at: &str, text: &str| {
        for l in links(text) {
            if !b.known.contains(&l) {
                problems.push(format!("{at}: [[{l}]] does not exist"));
            }
        }
        if SEMICOLON.is_match(text) {
            problems.push(format!("{at}: a semicolon"));
        }
    };
    let strings = |v: &Value| -> Vec<String> { ps::pipe(v).into_iter().map(ps::s).collect() };

    let mut says: CiMap<String> = CiMap::new();
    for c in ps::pipe(&sk["chapters"]) {
        let anchor = field(c, "anchor");
        let text = field(c, "says");
        if is_in(&anchor, &b.anchors) && !text.trim().is_empty() {
            says.insert(&anchor, text.trim().to_string());
            test_text(&mut problems, &anchor, &text);
            // The index line puts the group label before the text (`- 第 N 组（chapter）：…`), so its first sentence
            // is measured with the label: on grossman2002integration trial 11 one line came to 41 characters, an R2 fault
            // in the Literature Card that the bounded correction may not reopen.
            if let Some(g) = b.groups.iter().find(|g| ps::eq(&format!("{}/{}", b.key, pstr(g, "n")), &anchor)) {
                let first = text.trim().split('。').next().unwrap_or_default();
                let line = format!("第 {} 组（{}）：{first}。", pstr(g, "n"), pstr(g, "chapter"));
                if !zt_check::check::sentence_fits(&line) {
                    problems.push(format!("{anchor}: the index line's first sentence is longer than 40 characters with its group label (`{line}`); shorten says"));
                }
            }
        }
    }
    let mut argument = Vec::new();
    for a in ps::pipe(&sk["argument"]) {
        let anchor = field(a, "anchor");
        if !is_in(&anchor, &b.anchors) {
            problems.push(format!("argument: `{anchor}` is no anchor"));
            continue;
        }
        let key: Vec<String> = strings(a.get("key_cards").unwrap_or(&Value::Null)).into_iter().filter(|k| b.known.contains(k)).take(3).collect();
        test_text(&mut problems, &anchor, &field(a, "adds"));
        argument.push(Argument { anchor, adds: field(a, "adds").trim().to_string(), key });
    }
    let mut model_set: CiMap<&Value> = CiMap::new();
    for m in &b.models {
        model_set.insert(&field(m, "address"), m);
    }
    let mut model_lines = Vec::new();
    for m in ps::pipe(&sk["models"]) {
        let address = field(m, "address");
        let Some(model) = model_set.get(&address) else {
            problems.push(format!("models: `{address}` is no Model Card of the book"));
            continue;
        };
        test_text(&mut problems, &address, &field(m, "shows"));
        let level = level_name(&model["level"], "模型");
        let mut both = strings(m.get("built_on_by").unwrap_or(&Value::Null));
        both.extend(strings(m.get("builds_on").unwrap_or(&Value::Null)));
        let later = ps::select_unique(both.into_iter().filter(|x| model_set.contains(x)));
        let tail = if later.is_empty() {
            String::new()
        } else {
            format!(" 依赖它的模型：{}。", later.iter().map(|x| format!("[[{x}]]")).collect::<Vec<_>>().join("、"))
        };
        model_lines.push(format!("- [[{address}]]（{level}）：{}{tail}", field(m, "shows").trim()));
    }
    let event_set: HashSet<String> = b.events.iter().map(|e| ps::key(&field(e, "address"))).collect();
    let mut event_lines = Vec::new();
    for e in ps::pipe(&sk["events"]) {
        let address = field(e, "address");
        if !event_set.contains(&ps::key(&address)) {
            problems.push(format!("events: `{address}` is no Event Card of the book"));
            continue;
        }
        let supports: Vec<String> = strings(e.get("supports").unwrap_or(&Value::Null)).into_iter().filter(|k| b.known.contains(k)).take(3).collect();
        let what = if supports.is_empty() {
            "本书的论证".to_string()
        } else {
            supports.iter().map(|x| format!("[[{x}]]")).collect::<Vec<_>>().join("、")
        };
        event_lines.push(format!("- [[{address}]]：例示 {what} 的观点。"));
    }
    let mut topic_set: CiMap<String> = CiMap::new();
    for t in &b.topic_rows {
        topic_set.insert(&format!("{}/0", field(t, "tree")), field(t, "tree"));
    }
    let mut topic_bodies: CiMap<String> = CiMap::new();
    for t in ps::pipe(&sk["topics"]) {
        let root = field(t, "root");
        let Some(tree) = topic_set.get(&root).cloned() else {
            problems.push(format!("topics: `{root}` is no Topic of the book"));
            continue;
        };
        let body = field(t, "body").trim().to_string();
        if !links(&body).iter().any(|l| eq(l.split('/').next().unwrap_or_default(), &tree)) {
            problems.push(format!("topics: `{root}` links no entry Card of its tree"));
            continue;
        }
        test_text(&mut problems, &root, &body);
        topic_bodies.insert(&root, body);
    }
    Checked { says, argument, model_lines, event_lines, topic_set, topic_bodies, problems }
}

/// The structure Card's body.
fn structure_body(marker: &str, key: &str, c: &Checked) -> Result<String> {
    let mut sb = Sb::new();
    sb.line(marker).blank().line("## 论证").blank();
    let mut keyed: Vec<(i64, &Argument)> = Vec::new();
    for a in &c.argument {
        let last = a.anchor.split('/').next_back().unwrap_or_default().to_string();
        keyed.push((ps::int(&Value::String(last))?, a));
    }
    ps::ps_sort(&mut keyed, |x, y| x.0.cmp(&y.0));
    for (_, a) in keyed {
        let keys = if a.key.is_empty() {
            String::new()
        } else {
            format!(" 关键卡片：{}。", a.key.iter().map(|k| format!("[[{k}]]")).collect::<Vec<_>>().join("、"))
        };
        sb.line(format!("- [[{}]]：{}{keys}", a.anchor, a.adds));
    }
    if !c.model_lines.is_empty() {
        sb.blank().line("## 模型").blank();
        for l in &c.model_lines {
            sb.line(l);
        }
    }
    if !c.event_lines.is_empty() {
        sb.blank().line("## 事件").blank();
        for l in &c.event_lines {
            sb.line(l);
        }
    }
    sb.blank().line(format!("来源：[[{key}]]"));
    Ok(sb.0.trim_end().to_string())
}

fn apply(p: &mut Params, b: &Book, out: &str, zt: &mut Lazy) -> Result<i32> {
    let key = &b.key;
    let file = ps::join(out, "skeleton.json");
    if !ps::exists(&file) {
        util::print_line(&ps::json_compact(&json!({ "ok": false, "error": "no skeleton.json" })))?;
        return Ok(0);
    }
    let sk = common::read_json(&file)?;
    let c = check(&sk, b);
    if c.problems.iter().any(|x| BAD.is_match(x)) {
        util::print_line(&ps::json_compact(&json!({ "ok": false, "problems": c.problems })))?;
        return Ok(0);
    }

    // The thesis: -Thesis, else decisions.json, else the title the structure Card has.
    if p.thesis.is_empty() && !p.decisions.is_empty() && ps::exists(&p.decisions) {
        p.thesis = field(&common::read_json(&p.decisions)?, "thesis");
    }
    let tree = common::read_first(zt.get()?, "get_tree", json!({ "tree": key }))?;
    let tree_cards = ps::each(prop(&tree, "cards"));
    let mut structure = tree_cards.iter().find(|x| STRUCTURE.is_match(&pstr(x, "excerpt"))).copied();
    // A structure Card of an older digest has no marker line: a Side successor of <key>/1 that links
    // at least half of the anchors (and is no anchor, model, event, or term table).
    if structure.is_none() {
        structure = tree_cards
            .iter()
            .find(|x| {
                eq(&pstr(x, "parent"), &format!("{key}/1"))
                    && eq(&pstr(x, "edge"), "side")
                    && eq(&pstr(x, "type"), "idea")
                    && !TERM_TABLE.is_match(&pstr(x, "title"))
                    && ps::pipe(prop(x, "links")).iter().filter(|l| is_in(&ps::s(l), &b.anchors)).count() * 2 >= b.anchors.len()
            })
            .copied();
    }
    if p.thesis.trim().is_empty() {
        p.thesis = match structure {
            Some(s) => pstr(s, "title"),
            None => format!("《{}》的论证结构", b.title),
        };
    }
    let marker = format!("本卡是《{}》的结构卡：按阅读顺序列出全书的论证、模型与事件。", b.title);
    let body = structure_body(&marker, key, &c)?;
    let mut written = Map::new();
    if p.apply {
        let call = match structure {
            Some(s) => json!({ "tool": "edit_card", "arguments": { "at": pstr(s, "address"), "title": p.thesis, "body": body } }),
            None => json!({ "tool": "create_successor", "arguments": { "at": format!("{key}/1"), "kind": "side", "title": p.thesis, "body": body } }),
        };
        let lines = common::write_all(zt.get()?, &[call])?;
        let structure_at = common::first_ok(&lines);
        written.insert("structure".into(), json!(structure_at));
        // The Literature Card: the index, the structure Card, and the document line it had.
        let root = common::read_first(zt.get()?, "get_card", json!({ "at": key }))?;
        let root_body = pstr(&root, "body");
        let docs: Vec<&str> = root_body.split('\n').filter(|l| DOCUMENT.is_match(l)).collect();
        let mut index = Sb::new();
        index.line(format!("《{}》的索引：每组一行，写出它讲了什么，再链接它的锚点卡。", b.title)).blank();
        for g in &b.groups {
            let a = format!("{key}/{}", pstr(g, "n"));
            let text = match c.says.get(&a) {
                Some(t) => t.clone(),
                None => ps::pipe(prop(g, "headings")).into_iter().map(ps::s).collect::<Vec<_>>().join("；"),
            };
            index.line(format!("- 第 {} 组（{}）：{text} → [[{a}]]", pstr(g, "n"), pstr(g, "chapter")));
        }
        index.blank().line(format!("全书的论证结构见 [[{structure_at}]]。"));
        if let Some(t) = tree_cards.iter().find(|x| TERM_TABLE.is_match(&pstr(x, "title"))) {
            index.line(format!("本书的术语见 [[{}]]。", pstr(t, "address")));
        }
        if !docs.is_empty() {
            index.blank().line(docs.join("\n"));
        }
        let mut calls = vec![json!({ "tool": "edit_card", "arguments": { "at": key, "body": index.0.trim_end() } })];
        // Topic bodies: keep every line that links a Hub Card of the tree.
        for (r, new_body) in c.topic_bodies.iter() {
            let tree_of = c.topic_set.get(r).cloned().unwrap_or_default();
            let tc = common::read_first(zt.get()?, "get_tree", json!({ "tree": tree_of }))?;
            let hubs: Vec<String> = ps::each(prop(&tc, "cards")).into_iter().filter(|x| eq(&pstr(x, "type"), "hub")).map(|x| pstr(x, "address")).collect();
            let root_card = common::read_first(zt.get()?, "get_card", json!({ "at": r }))?;
            let root_text = pstr(&root_card, "body");
            let keep: Vec<&str> = root_text.split('\n').filter(|line| links(line).iter().any(|l| is_in(l, &hubs))).collect();
            let body = if keep.is_empty() { new_body.clone() } else { format!("{new_body}\n\n{}", keep.join("\n")) };
            calls.push(json!({ "tool": "edit_card", "arguments": { "at": r, "body": body } }));
        }
        common::write_all(zt.get()?, &calls)?;
        written.insert("literature".into(), json!(key));
        written.insert("topics".into(), json!(c.topic_bodies.keys()));
    }
    let last = json!({
        "ok": true, "thesis": p.thesis, "argument": c.argument.len(), "models": c.model_lines.len(), "events": c.event_lines.len(),
        "topics": c.topic_bodies.len(), "problems": c.problems, "written": written
    });
    util::print_line(&ps::json_compact(&last))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> Book {
        Book {
            key: "K".into(),
            title: "书".into(),
            groups: vec![json!({ "n": 1, "chapter": "一", "headings": ["甲"] }), json!({ "n": 2, "chapter": "二", "headings": ["乙", "丙"] })],
            anchors: vec!["K/1".into(), "K/2".into()],
            known: ["K/1", "K/2", "K/1|a", "K/2|a", "5/1", "5/2"].iter().map(|s| s.to_string()).collect(),
            cards: Vec::new(),
            models: vec![json!({ "address": "K/1|a", "level": "concept", "title": "模型" })],
            events: vec![json!({ "address": "K/2|a", "category": "event", "book": "K" })],
            topic_rows: vec![json!({ "tree": "5", "title": "主题", "size": 3, "cards": 2 })],
        }
    }

    // The index line's first sentence is measured with its group label, so the writer shortens says itself.
    #[test]
    fn an_index_line_is_measured_with_its_group_label() {
        let fits = check(&json!({ "chapters": [{ "anchor": "K/1", "says": "简单模型决定企业边界。第二句可以很长很长很长很长很长很长很长很长很长很长很长。" }] }), &book());
        assert!(!fits.problems.iter().any(|p| p.contains("group label")), "{:?}", fits.problems);
        let long = check(&json!({ "chapters": [{ "anchor": "K/2", "says": "简单模型用敲竹杠扭曲、搜寻摩擦与纵向一体化的治理成本共同决定企业的边界与组织模式。" }] }), &book());
        assert!(long.problems.iter().any(|p| p.starts_with("K/2: the index line's first sentence")), "{:?}", long.problems);
    }

    #[test]
    fn skeleton_json_is_checked_as_the_script_checked_it() {
        let sk = json!({
            "chapters": [{ "anchor": "K/1", "says": "第一组。" }, { "anchor": "K/9", "says": "不是锚点。" }],
            "argument": [{ "anchor": "K/2", "adds": "其次。", "key_cards": ["5/2", "99/9"] }, { "anchor": "K/1", "adds": "首先。", "key_cards": ["5/1"] }, { "anchor": "X/1", "adds": "x" }],
            "models": [{ "address": "K/1|a", "shows": "说明。", "built_on_by": ["K/1|a", "K/1|a"] }, { "address": "K/7", "shows": "x" }],
            "events": [{ "address": "k/2|a", "supports": [] }],
            "topics": [{ "root": "5/0", "body": "依赖 [[5/1]]。" }, { "root": "6/0", "body": "x" }]
        });
        let c = check(&sk, &book());
        assert_eq!(c.problems, ["argument: `X/1` is no anchor", "models: `K/7` is no Model Card of the book", "topics: `6/0` is no Topic of the book"]);
        assert_eq!(c.model_lines, ["- [[K/1|a]]（概念模型）：说明。 依赖它的模型：[[K/1|a]]。"]);
        assert_eq!(c.event_lines, ["- [[k/2|a]]：例示 本书的论证 的观点。"]);
        assert_eq!(c.topic_bodies.keys(), ["5/0"]);
        let body = structure_body("标记", "K", &c).unwrap();
        let nl = ps::NL;
        assert!(body.starts_with(&format!("标记{nl}{nl}## 论证{nl}{nl}- [[K/1]]：首先。 关键卡片：[[5/1]]。{nl}- [[K/2]]：其次。 关键卡片：[[5/2]]。{nl}{nl}## 模型")));
        assert!(body.ends_with(&format!("{nl}{nl}来源：[[K]]")));
        // A Link to an unknown Card and a semicolon are the problems that stop apply. A list that
        // is missing is `@($null)`, one empty entry, as in the script.
        let bad = check(&json!({ "topics": [{ "root": "5/0", "body": "依赖 [[5/1]]；见 [[5/9]]。" }] }), &book());
        assert_eq!(
            bad.problems,
            [
                "argument: `` is no anchor",
                "models: `` is no Model Card of the book",
                "events: `` is no Event Card of the book",
                "5/0: [[5/9]] does not exist",
                "5/0: a semicolon"
            ]
        );
        assert_eq!(self_check("D:\\s", "D:\\s\\skeleton"), "SELF-CHECK: `zt-digest skeleton -Step apply -Staging \"D:\\s\" -Work \"D:\\s\\skeleton\"`（不要加 -Apply）");
    }
}
