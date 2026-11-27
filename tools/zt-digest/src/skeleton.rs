//! `zt-digest skeleton`: the port of `scripts/book-digest/book-skeleton.ps1`, the book's skeleton
//! (docs/digest-card-layers-proposal.md, proposal 7): the Literature Card index and the structure
//! Card. A digest writes only the Literature tree (docs/literature-only-digest-design.md); the
//! Topic bodies are zt-idea-grow's. The tool builds each frame from the manifest and the
//! trees; one DeepSeek job writes the short texts as JSON; Opus gives only the thesis.
//!
//!   zt-digest skeleton -Step plan|apply -Staging <folder> -Work <folder> [-Audit <audit.json>]
//!                      [-Thesis <text>] [-Decisions <decisions.json>] [-Apply]
//!
//! -Audit names an audit.json of the book; without it the tool runs the audit in-process into
//! <Work>\audit (the script ran digest-audit.ps1). Every step prints compact JSON as its last line.
//!
//!   plan   writes job.md (one section per group, the book's Model Cards with their levels, its
//!          Event Cards, and the SELF-CHECK line
//!          `zt-digest skeleton -Step apply …` without -Apply) and the instruction file.
//!   apply  reads out\skeleton.json and checks it; with -Apply writes the structure Card, the
//!          Literature Card index. An answer that still carries `topics` is a problem.

use crate::args::Args;
use crate::organize::common::{self, Lazy, Sb, prop, pstr};
use crate::ps::{self, CiMap, eq, field, is_in};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::LazyLock;

static SEMICOLON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[；;]").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
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
    models: Vec<Value>,
    events: Vec<Value>,
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
    let mut zt = Lazy::new();
    // R328 (§15.2): the plan allocates the one skeleton job and the apply writes the structure Card,
    // the Literature Card index and the Topic bodies. Both read the locked tree normally, and both
    // stop before their first file or Card when the book's Literature tree is locked by the owner.
    if p.step == "plan" || p.apply {
        if let Some(refusal) = crate::lockcheck::preflight(zt.get()?, "skeleton", &p.step, &key)? {
            return crate::lockcheck::stop("skeleton", &refusal);
        }
    }
    std::fs::create_dir_all(&p.work)?;
    std::fs::create_dir_all(&out)?;

    // --- what the book holds ---
    let work_audit = ps::join(&p.work, "audit\\audit.json");
    let candidate = if p.audit.is_empty() { work_audit.clone() } else { p.audit.clone() };
    if usable_audit(&mut zt, &key, &candidate)? {
        p.audit = candidate;
    } else {
        if !p.audit.is_empty() {
            crate::util::emit_err(&format!("zt-digest skeleton: -Audit {} is stale or mismatched; rerunning audit", p.audit));
        }
        p.audit = work_audit;
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
    let b = Book { key, title, groups, anchors, known, models, events };
    if p.step == "plan" { plan(&p, &b, &out, &mut zt) } else { apply(&mut p, &b, &out, &mut zt) }
}

fn usable_audit(zt: &mut Lazy, key: &str, audit_path: &str) -> Result<bool> {
    if !ps::exists(audit_path) {
        return Ok(false);
    }
    let dir = ps::parent(audit_path);
    let cards_path = ps::join(&dir, "audit-cards.json");
    let addr_path = ps::join(&dir, "audit-addresses.txt");
    if !ps::exists(&cards_path) || !ps::exists(&addr_path) {
        return Ok(false);
    }
    let Ok(audit) = common::read_json(audit_path) else {
        return Ok(false);
    };
    if audit["complete"] != json!(true) || audit["skip_models"] != json!(false) {
        return Ok(false);
    }
    let Some(exp_cards_sha) = audit["sidecars"]["cards_sha256"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(false);
    };
    let Some(exp_addr_sha) = audit["sidecars"]["addresses_sha256"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(false);
    };
    let Ok(cards_bytes) = std::fs::read(&cards_path) else {
        return Ok(false);
    };
    let Ok(addr_bytes) = std::fs::read(&addr_path) else {
        return Ok(false);
    };
    if crate::canon::sha256_hex(&cards_bytes) != exp_cards_sha || crate::canon::sha256_hex(&addr_bytes) != exp_addr_sha {
        return Ok(false);
    }
    let books = ps::pipe(&audit["books"]);
    if books.len() != 1 || !eq(&field(books[0], "key"), key) {
        return Ok(false);
    }
    let Some(audit_rev) = audit["archive_revision"].as_u64() else {
        return Ok(false);
    };
    let Some(audit_id) = audit["baseline"]["evidence"]["archive"]["id"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(false);
    };
    let client = zt.get()?;
    let contract_view = client.call("get_cards", json!({ "addresses": [], "view": "text" }))?;
    let Some(live_rev) = contract_view["archive_revision"].as_u64() else {
        return Ok(false);
    };
    let identity_view = client.archive_view()?;
    let id_rev = identity_view["archive_revision"].as_u64();
    let live_id = identity_view["archive_identity"]["id"].as_str();
    Ok(live_rev == audit_rev && id_rev == Some(audit_rev) && live_id == Some(audit_id))
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
    let addresses: Vec<String> = b.anchors.clone();
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
    sb.blank();
    sb.line(self_check(&p.staging, &p.work));
    let job = ps::join(&p.work, "job.md");
    util::write_text(&job, &sb.0)?;
    let doc = util::read_text(ps::join(&common::scripts_dir(), DOC))?;
    let doc_copy = ps::join(&p.work, DOC);
    util::write_text(&doc_copy, &doc.replace("{{KEY}}", key))?;
    let last = json!({
        "groups": b.groups.len(), "models": b.models.len(), "events": b.events.len(),
        "jobs": [{ "id": "skeleton", "files": [doc_copy, job], "prompt": PROMPT }]
    });
    util::print_line(&ps::json_compact(&last))?;
    Ok(0)
}

/// `Get-Links`: resolves every `[[addr]]` or Term-link `[[label@addr]]` target address.
fn links(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for occ in zt_check::text::link_occurrences(text) {
        let raw = text.get(occ.macro_range).unwrap_or_default().to_string();
        out.push((raw, occ.target.to_string()));
    }
    out
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
    problems: Vec<String>,
}

fn check(sk: &Value, b: &Book) -> Checked {
    let mut problems: Vec<String> = Vec::new();
    let test_text = |problems: &mut Vec<String>, at: &str, text: &str| {
        if let Err(fault) = zt_check::text::parse_links(text) {
            problems.push(format!("{at}: invalid link syntax ({fault:?})"));
        }
        for (raw, target) in links(text) {
            if !b.known.contains(&target) {
                problems.push(format!("{at}: {raw} does not exist"));
            }
        }
        if SEMICOLON.is_match(text) {
            problems.push(format!("{at}: a semicolon"));
        }
    };
    let strings = |v: &Value| -> Vec<String> { ps::pipe(v).into_iter().map(ps::s).collect() };

    let mut says: CiMap<String> = CiMap::new();
    let mut seen_chapters: HashSet<String> = HashSet::new();
    for c in ps::pipe(&sk["chapters"]) {
        let anchor = field(c, "anchor");
        if !is_in(&anchor, &b.anchors) {
            problems.push(format!("chapters: `{anchor}` is no anchor"));
            continue;
        }
        if !seen_chapters.insert(ps::key(&anchor)) {
            problems.push(format!("chapters: duplicate anchor `{anchor}`"));
            continue;
        }
        let text = field(c, "says");
        let trimmed = text.trim();
        if trimmed.is_empty() {
            problems.push(format!("{anchor}: says is empty"));
            continue;
        }
        says.insert(&anchor, trimmed.to_string());
        test_text(&mut problems, &anchor, &text);
        if let Some(g) = b.groups.iter().find(|g| ps::eq(&format!("{}/{}", b.key, pstr(g, "n")), &anchor)) {
            let line = format!("- 第 {} 组（{}）：{trimmed} → [[{anchor}]]", pstr(g, "n"), pstr(g, "chapter"));
            for fault in zt_check::check::line_r2_faults(&line) {
                problems.push(format!("{anchor}: R2 on index line {fault}"));
            }
        }
    }
    for anchor in &b.anchors {
        if !seen_chapters.contains(&ps::key(anchor)) {
            problems.push(format!("chapters: missing anchor `{anchor}`"));
        }
    }

    let mut argument = Vec::new();
    let mut seen_args: HashSet<String> = HashSet::new();
    for a in ps::pipe(&sk["argument"]) {
        let anchor = field(a, "anchor");
        if !is_in(&anchor, &b.anchors) {
            problems.push(format!("argument: `{anchor}` is no anchor"));
            continue;
        }
        if !seen_args.insert(ps::key(&anchor)) {
            problems.push(format!("argument: duplicate anchor `{anchor}`"));
            continue;
        }
        let adds = field(a, "adds").trim().to_string();
        if adds.is_empty() {
            problems.push(format!("{anchor}: argument adds is empty"));
        }
        let raw_keys: Vec<String> = if a.get("key_cards").is_some_and(Value::is_array) {
            strings(a.get("key_cards").unwrap_or(&Value::Null))
                .into_iter()
                .filter(|k| !k.trim().is_empty())
                .collect()
        } else {
            Vec::new()
        };
        if raw_keys.len() > 3 {
            problems.push(format!("{anchor}: more than 3 key_cards ({})", raw_keys.len()));
        }
        let mut key = Vec::new();
        for k in raw_keys {
            if zt_check::mcp::scope::is_topic_address(&k) || !b.known.contains(&k) {
                problems.push(format!("{anchor}: key_card `[[{k}]]` is invalid or unknown"));
            } else if key.len() < 3 {
                key.push(k);
            }
        }
        test_text(&mut problems, &anchor, &adds);
        let keys_tail = if key.is_empty() {
            String::new()
        } else {
            format!(" 关键卡片：{}。", key.iter().map(|k| format!("[[{k}]]")).collect::<Vec<_>>().join("、"))
        };
        let rendered = format!("- [[{anchor}]]：{adds}{keys_tail}");
        for fault in zt_check::check::line_r2_faults(&rendered) {
            problems.push(format!("{anchor}: R2 on argument line {fault}"));
        }
        argument.push(Argument { anchor, adds, key });
    }
    for anchor in &b.anchors {
        if !seen_args.contains(&ps::key(anchor)) {
            problems.push(format!("argument: missing anchor `{anchor}`"));
        }
    }

    let mut model_set: CiMap<&Value> = CiMap::new();
    for m in &b.models {
        model_set.insert(&field(m, "address"), m);
    }
    let mut model_lines = Vec::new();
    let mut seen_models: HashSet<String> = HashSet::new();
    if !(b.models.is_empty() && sk["models"].as_array().is_some_and(Vec::is_empty)) {
        for m in ps::pipe(&sk["models"]) {
            let address = field(m, "address");
            let Some(model) = model_set.get(&address) else {
                problems.push(format!("models: `{address}` is no Model Card of the book"));
                continue;
            };
            if !seen_models.insert(ps::key(&address)) {
                problems.push(format!("models: duplicate model `{address}`"));
                continue;
            }
            let shows = field(m, "shows").trim().to_string();
            if shows.is_empty() {
                problems.push(format!("{address}: shows is empty"));
            }
            test_text(&mut problems, &address, &shows);
            let level = level_name(&model["level"], "模型");
            let mut both = if m.get("built_on_by").is_some_and(Value::is_array) {
                strings(m.get("built_on_by").unwrap_or(&Value::Null))
            } else {
                Vec::new()
            };
            if m.get("builds_on").is_some_and(Value::is_array) {
                both.extend(strings(m.get("builds_on").unwrap_or(&Value::Null)));
            }
            both.retain(|x| !x.trim().is_empty());
            for x in &both {
                if !model_set.contains(x) {
                    problems.push(format!("{address}: model reference `{x}` is no Model Card of the book"));
                }
            }
            let later = ps::select_unique(both.into_iter().filter(|x| model_set.contains(x)));
            let tail = if later.is_empty() {
                String::new()
            } else {
                format!(" 依赖它的模型：{}。", later.iter().map(|x| format!("[[{x}]]")).collect::<Vec<_>>().join("、"))
            };
            let rendered = format!("- [[{address}]]（{level}）：{shows}{tail}");
            for fault in zt_check::check::line_r2_faults(&rendered) {
                problems.push(format!("{address}: R2 on model line {fault}"));
            }
            model_lines.push(rendered);
        }
    }
    for m in &b.models {
        let addr = field(m, "address");
        if !seen_models.contains(&ps::key(&addr)) {
            problems.push(format!("models: missing model `{addr}`"));
        }
    }

    let event_set: HashSet<String> = b.events.iter().map(|e| ps::key(&field(e, "address"))).collect();
    let mut event_lines = Vec::new();
    let mut seen_events: HashSet<String> = HashSet::new();
    if !(b.events.is_empty() && sk["events"].as_array().is_some_and(Vec::is_empty)) {
        for e in ps::pipe(&sk["events"]) {
            let address = field(e, "address");
            if !event_set.contains(&ps::key(&address)) {
                problems.push(format!("events: `{address}` is no Event Card of the book"));
                continue;
            }
            if !seen_events.insert(ps::key(&address)) {
                problems.push(format!("events: duplicate event `{address}`"));
                continue;
            }
            let raw_supports: Vec<String> = if e.get("supports").is_some_and(Value::is_array) {
                strings(e.get("supports").unwrap_or(&Value::Null))
                    .into_iter()
                    .filter(|k| !k.trim().is_empty())
                    .collect()
            } else {
                Vec::new()
            };
            if raw_supports.len() > 3 {
                problems.push(format!("{address}: more than 3 supports ({})", raw_supports.len()));
            }
            let mut supports = Vec::new();
            for s_addr in raw_supports {
                if zt_check::mcp::scope::is_topic_address(&s_addr) || !b.known.contains(&s_addr) {
                    problems.push(format!("{address}: support `[[{s_addr}]]` is invalid or unknown"));
                } else if supports.len() < 3 {
                    supports.push(s_addr);
                }
            }
            let what = if supports.is_empty() {
                "本书的论证".to_string()
            } else {
                supports.iter().map(|x| format!("[[{x}]]")).collect::<Vec<_>>().join("、")
            };
            let rendered = format!("- [[{address}]]：例示 {what} 的观点。");
            for fault in zt_check::check::line_r2_faults(&rendered) {
                problems.push(format!("{address}: R2 on event line {fault}"));
            }
            event_lines.push(rendered);
        }
    }
    for e in &b.events {
        let addr = field(e, "address");
        if !seen_events.contains(&ps::key(&addr)) {
            problems.push(format!("events: missing event `{addr}`"));
        }
    }

    if ps::pipe(&sk["topics"]).iter().any(|t| ps::truthy(t)) {
        problems.push("topics: a digest writes no Topic body; Topic trees are zt-idea-grow's".to_string());
    }
    Checked { says, argument, model_lines, event_lines, problems }
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
    if !c.problems.is_empty() {
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
        let calls = vec![json!({ "tool": "edit_card", "arguments": { "at": key, "body": index.0.trim_end() } })];
        common::write_all(zt.get()?, &calls)?;
        written.insert("literature".into(), json!(key));
    }
    let last = json!({
        "ok": true, "thesis": p.thesis, "argument": c.argument.len(), "models": c.model_lines.len(), "events": c.event_lines.len(),
        "problems": c.problems, "written": written
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
            known: ["K/1", "K/2", "K/1|a", "K/2|a", "K/1|b", "K/2|b", "5/1", "5/2"].iter().map(|s| s.to_string()).collect(),
            models: vec![json!({ "address": "K/1|a", "level": "concept", "title": "模型" })],
            events: vec![json!({ "address": "K/2|a", "category": "event", "book": "K" })],
        }
    }

    // The index line's sentences are measured with its group label and arrow link, so the writer shortens says itself.
    #[test]
    fn an_index_line_is_measured_with_its_group_label() {
        let fits = check(&json!({ "chapters": [{ "anchor": "K/1", "says": "简单模型决定企业边界。第二句可以很长很长很长很长很长很长很长很长很长很长很长。" }] }), &book());
        assert!(!fits.problems.iter().any(|p| p.contains("R2 on index line")), "{:?}", fits.problems);
        let long = check(&json!({ "chapters": [{ "anchor": "K/2", "says": "简单模型用敲竹杠扭曲、搜寻摩擦与纵向一体化的治理成本共同决定企业的边界与组织模式。" }] }), &book());
        assert!(long.problems.iter().any(|p| p.starts_with("K/2: R2 on index line")), "{:?}", long.problems);
        // A long second sentence on an index line is also caught.
        let long_second = check(&json!({ "chapters": [{ "anchor": "K/1", "says": "简单模型决定企业边界。第二句把敲竹杠扭曲、搜寻摩擦与纵向一体化的治理成本以及事后分配机制全部展开并逐一比较和检验。" }] }), &book());
        assert!(long_second.problems.iter().any(|p| p.starts_with("K/1: R2 on index line")), "{:?}", long_second.problems);
    }

    #[test]
    fn skeleton_json_is_checked_for_completeness_r2_and_valid_links() {
        let clean = json!({
            "chapters": [{ "anchor": "K/1", "says": "第一组。" }, { "anchor": "K/2", "says": "第二组。" }],
            "argument": [{ "anchor": "K/2", "adds": "其次[[均衡@K/1|b]]。", "key_cards": ["K/2|b"] }, { "anchor": "K/1", "adds": "首先。", "key_cards": ["K/1|b"] }],
            "models": [{ "address": "K/1|a", "shows": "说明企业边界由剩余控制权决定。", "built_on_by": ["K/1|a"] }],
            "events": [{ "address": "k/2|a", "supports": [] }]
        });
        let ok_res = check(&clean, &book());
        assert!(ok_res.problems.is_empty(), "{:?}", ok_res.problems);
        assert_eq!(ok_res.model_lines, ["- [[K/1|a]]（概念模型）：说明企业边界由剩余控制权决定。 依赖它的模型：[[K/1|a]]。"]);
        assert_eq!(ok_res.event_lines, ["- [[k/2|a]]：例示 本书的论证 的观点。"]);
        let body = structure_body("标记", "K", &ok_res).unwrap();
        let nl = ps::NL;
        assert!(body.starts_with(&format!("标记{nl}{nl}## 论证{nl}{nl}- [[K/1]]：首先。 关键卡片：[[K/1|b]]。{nl}- [[K/2]]：其次[[均衡@K/1|b]]。 关键卡片：[[K/2|b]]。{nl}{nl}## 模型")));
        assert!(body.ends_with(&format!("{nl}{nl}来源：[[K]]")));

        // Invalid key_cards, topic links, duplicate/missing rows, and R2 length faults all fail validation.
        let dirty = json!({
            "chapters": [{ "anchor": "K/1", "says": "第一组。" }, { "anchor": "K/1", "says": "重复。" }, { "anchor": "K/9", "says": "不是锚点。" }],
            "argument": [{ "anchor": "K/1", "adds": "首先。", "key_cards": ["5/1", "99/9"] }, { "anchor": "X/1", "adds": "x" }],
            "models": [{ "address": "K/1|a", "shows": "这个模型详细分析了不完全契约下的专用性投资、事后谈判摩擦以及纵向一体化对剩余控制权分配的综合影响。", "built_on_by": ["K/9|z"] }],
            "events": [{ "address": "k/2|a", "supports": ["5/2"] }],
            "topics": [{ "root": "5/0", "body": "依赖 [[5/1]]。" }]
        });
        let c = check(&dirty, &book());
        assert!(c.problems.iter().any(|p| p == "chapters: `K/9` is no anchor"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p == "chapters: duplicate anchor `K/1`"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p == "chapters: missing anchor `K/2`"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p == "K/1: key_card `[[5/1]]` is invalid or unknown"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p == "K/1: key_card `[[99/9]]` is invalid or unknown"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p == "argument: missing anchor `K/2`"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p.starts_with("K/1|a: R2 on model line")), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p.contains("model reference `K/9|z`")), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p == "k/2|a: support `[[5/2]]` is invalid or unknown"), "{:?}", c.problems);
        assert!(c.problems.iter().any(|p| p.starts_with("topics:")), "{:?}", c.problems);
        assert_eq!(self_check("D:\\s", "D:\\s\\skeleton"), "SELF-CHECK: `zt-digest skeleton -Step apply -Staging \"D:\\s\" -Work \"D:\\s\\skeleton\"`（不要加 -Apply）");
    }
}
