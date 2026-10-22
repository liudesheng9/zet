//! `zt-digest grow`: the idea-grow compiler (docs/idea-grow-design.md). It reads a finished
//! Literature tree and the whole box, and grows the agent Topic trees with the reader's own ideas,
//! as Luhmann wrote his main-box Cards from his bibliographic notes. A compiler cycle: the tool
//! plans, DeepSeek writes one JSON answer per section and checks it with `precheck`, the tool writes
//! the Cards, checks them, and plans one bounded fix round.
//!
//!   zt-digest grow -Step plan     -Book <key> -Work <folder> [-Instructions <file>] [-From N] [-To M]
//!   zt-digest grow -Step precheck -Work <folder> -Job <gw-NNN|fxR-NNN>
//!   zt-digest grow -Step apply    -Work <folder> [-Apply]
//!   zt-digest grow -Step check    -Work <folder> -Round R
//!   zt-digest grow -Step report   -Work <folder>
//!
//! plan      reads the Literature tree (either Workspace) and every title of the box, and writes
//!           snapshot.md, topics.md, titles.json, sections\sNNN.md, jobs\gw-NNN.md (one per
//!           section not applied yet), instructions.zh.md, read-only.allow.txt (empty: the helper
//!           then refuses every write), baseline.json and plan.json. It prints a short line; the
//!           job ids are gw-001 … gw-<jobs>, so no id list passes through a relay.
//! precheck  read-only: the answer's faults (G1–G13, docs/idea-grow-design.md) and zt-check on the
//!           drafts, one line each, then `faults: N`.
//! apply     for every answered job not applied yet, in job order: refuses each idea with a fault,
//!           makes the new Topics through the topic gate (a near-name refusal files the Cards under
//!           the existing Topic), drops a run-level duplicate, creates each Card at its parent (a
//!           Direct successor whose slot is taken becomes a Side successor), appends the source line
//!           when it is missing; a `same` idea appends its sentence to an agent Card or is recorded
//!           for a human one. Then the answered fix jobs: each edit with the Card's text as `base`.
//!           Without -Apply it only validates.
//! check     zt-check on every Card the run wrote; each hard fault becomes a fix job of round R
//!           (jobs\fxR-NNN.md, one per section). Prints `{"fix":M,...}`.
//! report    grow-report.json and grow-report.md: ideas per kind and section, Cards, Topics, same,
//!           refused, duplicates, faults left, and the Topic share before and after (P5).
//!
//! It writes only agent Topic trees: it never edits a Literature-tree Card, never moves or deletes a
//! Card, and never writes a human Card.

use crate::args::Args;
use crate::calls;
use crate::check::Check;
use crate::lockcheck;
use crate::ps::join_path;
use crate::snapshot;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;

/// The ideas one section may keep (the prompt asks for about 3–10).
pub const MAX_IDEAS: usize = 12;
/// The lines a doorway `n/1` keeps beside it (the zettelkasten skill's about seven Side successors per
/// Card); a later root placement goes beside the last line instead (`line_parent`).
pub const MAX_LINES: usize = 7;
/// The jobs one run may plan (Haiku managers run 8 each, 8 at once).
pub const MAX_JOBS: usize = 120;
/// One Literature Card's body in a section file, in characters.
const BODY_CHARS: usize = 1200;
/// Two titles are near at this Dice coefficient of their character bigrams (the topic gate's 0.8).
const NEAR: f64 = 0.8;
/// The existing Topic Cards a section lists as closest by wording.
const CLOSEST: usize = 15;
const KINDS: [&str; 5] = ["核心", "关联", "有用", "有趣", "直觉"];
/// The Link kinds zt reads, with the words that announce them: a port of `RELATION_KINDS` in
/// src/model.rs (the zt binary), so a check here reads a Link as the archive will. `引出` announces a
/// plain reference (the linked Card builds on this one).
const RELATION_CUES: [(&str, &[&str]); 8] = [
    ("rests-on", &["依赖", "依据", "基于", "建立在", "取自", "沿用", "承接", "用到", "借用", "代入", "推导自", "由此推出", "按定义", "rests on", "builds on", "depends on", "relies on", "derived from", "follows from"]),
    ("illustrates", &["例示", "举例", "例子", "示例", "演示", "例证", "实例", "案例", "印证", "illustrat", "for example", "e.g."]),
    ("contrasts", &["对照", "对比", "相比", "不同于", "区别", "相反", "逆命题"]),
    ("relaxes", &["放宽", "放松", "推广", "扩展", "拓展", "一般化"]),
    ("corresponds", &["对应", "类比", "同构", "相当于", "等价", "参照"]),
    ("objects", &["反驳", "批评", "质疑", "反例", "否定", "挑战"]),
    ("specializes", &["特例", "特殊情形", "special case", "specializ"]),
    ("refers", &["引出"]),
];
/// The kinds a Topic Card may cite its own book with (never rests-on: I7; never a plain reference).
const CITE_KINDS: [&str; 6] = ["illustrates", "contrasts", "relaxes", "corresponds", "objects", "specializes"];
const CITE_WORDS: &str = "例示/印证/对应/对照/放宽/推广/反驳";
const KIND_WORDS: &str = "依赖/依据/承接/例示/印证/对应/类比/对照/区别/放宽/推广/反驳/特例";
const REVERSE_MARK: &str = "<!-- zt:reverse-links -->";

static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]\n]+)\]\]").unwrap());
static REVERSE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^- \[\[([^\]]+)\]\]\s*(.*)$").unwrap());
static FAULT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\S+) ([A-Z]+\d+) (.*)$").unwrap());
static FAULTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^faults: \d+$").unwrap());
static TOPIC_EXISTS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"topic exists: `([^`]+)`").unwrap());
static SOURCE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^来源：").unwrap());

// --- small text helpers ------------------------------------------------------------------------

fn s(v: &Value) -> String {
    match v {
        Value::String(t) => t.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn cut(text: &str, chars: usize) -> String {
    let t = text.trim();
    if t.chars().count() <= chars {
        return t.to_string();
    }
    format!("{}……", t.chars().take(chars).collect::<String>())
}

/// The tree part of an address: `12` of `12/3|a`, `Key` of `Key/3`, `Key` of `Key`.
fn tree_of(address: &str) -> &str {
    address.split('/').next().unwrap_or(address)
}

/// A Topic-tree address: its tree part is a number.
fn in_topic_tree(address: &str) -> bool {
    let tree = tree_of(address);
    !tree.is_empty() && tree.chars().all(|c| c.is_ascii_digit()) && address.contains('/')
}

fn is_root(address: &str) -> bool {
    address.ends_with("/0") && !address.contains('|')
}

/// The Direct successor's address: a trailing number goes up by one, after a letter a line starts at 1.
fn next_direct(address: &str) -> String {
    let (head, last) = match address.rfind(['|', '/']) {
        Some(i) => (&address[..=i], &address[i + 1..]),
        None => ("", address),
    };
    match last.parse::<u64>() {
        Ok(n) => format!("{head}{}", n + 1),
        Err(_) => format!("{address}|1"),
    }
}

fn near(a: &str, b: &str) -> bool {
    let (x, y) = (calls::title_key(a), calls::title_key(b));
    !x.is_empty() && (x == y || calls::similarity(&x, &y) >= NEAR)
}

fn bigrams(text: &str) -> HashSet<(char, char)> {
    let chars: Vec<char> = text.chars().filter(|c| c.is_alphanumeric()).collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

/// The first sentence of a body (up to the first 。！？ or line end).
fn first_sentence(body: &str) -> String {
    let t = body.trim_start();
    let end = t.find(['。', '！', '？', '\n']).map(|i| i + t[i..].chars().next().map_or(1, char::len_utf8)).unwrap_or(t.len());
    t[..end].to_string()
}

/// The kind announced last in `words` (zt's `announced_in`): the cue that ends last, the longer on a tie.
fn announced_in(words: &str) -> Option<&'static str> {
    let lower = words.to_lowercase();
    let mut best: Option<(usize, usize, &'static str)> = None;
    for (kind, cues) in RELATION_CUES {
        for cue in cues {
            let word = cue.starts_with(|ch: char| ch.is_ascii_alphabetic());
            for (at, _) in lower.match_indices(cue) {
                if word && lower[..at].chars().next_back().is_some_and(|ch| ch.is_ascii_alphanumeric()) {
                    continue;
                }
                let found = (at + cue.len(), cue.len(), kind);
                if best.is_none_or(|(end, len, _)| (found.0, found.1) > (end, len)) {
                    best = Some(found);
                }
            }
        }
    }
    best.map(|(_, _, kind)| kind)
}

fn ends_sentence(body: &str, index: usize, ch: char) -> bool {
    ['。', '！', '？', '!', '?', '；', ';', '\n'].contains(&ch) || (ch == '.' && body[index + 1..].starts_with(char::is_whitespace))
}

fn sentence_start(body: &str, at: usize) -> usize {
    body[..at].char_indices().rev().find(|&(index, ch)| ends_sentence(body, index, ch)).map_or(0, |(index, ch)| index + ch.len_utf8())
}

/// Every Link of `body` with its kind, read as zt's `read_links` reads it: the words since the
/// sentence start (or since an earlier Link of the same sentence) announce it; with none, a Link
/// shares the kind of the earlier Link of its sentence, else it is a plain reference.
fn link_kinds(body: &str) -> Vec<(String, usize, &'static str)> {
    let mut out = Vec::new();
    let mut previous: Option<(usize, &'static str)> = None;
    for m in LINK.captures_iter(body) {
        let range = m.get(0).unwrap().range();
        let start = sentence_start(body, range.start);
        let shared = previous.filter(|&(end, _)| end > start);
        let from = shared.map_or(start, |(end, _)| end);
        let kind = announced_in(&body[from..range.start]).or(shared.map(|(_, kind)| kind)).unwrap_or("refers");
        previous = Some((range.end, kind));
        out.push((m[1].trim().to_string(), range.start, kind));
    }
    out
}

/// Whether `body` links one of `targets` with one of `kinds`.
fn cites_with_kind(body: &str, targets: &[String], kinds: &[&str]) -> bool {
    link_kinds(body).iter().any(|(target, _, kind)| targets.iter().any(|t| t.eq_ignore_ascii_case(target)) && kinds.contains(kind))
}

/// The Links of `body` into the book `key` that zt reads as rests-on (I7: an own-source premise).
fn rests_on_book(body: &str, key: &str) -> Vec<String> {
    link_kinds(body).into_iter().filter(|(target, _, kind)| *kind == "rests-on" && tree_of(target).eq_ignore_ascii_case(key)).map(|(t, _, _)| t).collect()
}

/// The Links of `body` outside its source line that zt reads as plain references (G10: a Link
/// carries its reason, Luhmann; zt-check L1).
fn plain_links(body: &str) -> Vec<String> {
    link_kinds(body)
        .into_iter()
        .filter(|(_, at, kind)| *kind == "refers" && !body[sentence_start(body, *at)..*at].trim_start().starts_with("来源"))
        .map(|(t, _, _)| t)
        .collect()
}

fn job_id(prefix: &str, n: usize) -> String {
    format!("{prefix}-{n:03}")
}

// --- the work folder --------------------------------------------------------------------------

struct Work {
    dir: String,
}

impl Work {
    fn open(dir: &str) -> Result<Self> {
        if dir.trim().is_empty() {
            bail!("-Work is required");
        }
        let dir = dir.trim_end_matches(['\\', '/']).to_string();
        for sub in ["", "out", "jobs", "sections"] {
            std::fs::create_dir_all(if sub.is_empty() { dir.clone() } else { join_path(&dir, sub) })?;
        }
        Ok(Self { dir })
    }
    fn path(&self, name: &str) -> String {
        join_path(&self.dir, name)
    }
    fn read(&self, name: &str) -> Result<Value> {
        util::read_json(self.path(name)).with_context(|| format!("{} (run -Step plan first)", self.path(name)))
    }
    fn write(&self, name: &str, value: &Value) -> Result<()> {
        util::write_text(self.path(name), &serde_json::to_string_pretty(value)?)
    }
    fn state(&self) -> Result<Value> {
        let path = self.path("state.json");
        if std::path::Path::new(&path).exists() {
            util::read_json(&path)
        } else {
            Ok(json!({ "sections": {}, "cards": {}, "fixes": {}, "fixed": {} }))
        }
    }
}

// --- plan ---------------------------------------------------------------------------------------

/// The reverse-link sources of a Card's stored text: `(address, title)` per line.
fn reverse_links(text: &str) -> Vec<(String, String)> {
    let Some(at) = text.find(REVERSE_MARK) else { return Vec::new() };
    text[at..]
        .lines()
        .filter_map(|line| REVERSE_LINE.captures(line.trim()).map(|c| (c[1].trim().to_string(), c[2].trim().to_string())))
        .collect()
}

fn type_label(card: &Value) -> String {
    match s(&card["type"]).as_str() {
        "model" if !s(&card["facet"]).is_empty() => format!("模型分卡·{}", s(&card["facet"])),
        "model" => "模型卡".into(),
        "event" => "事件卡".into(),
        "finding" => "结果卡".into(),
        _ => "观点卡".into(),
    }
}

/// The Topic share of a book (P5): the distinct Topic-tree Cards (either Workspace) that link a Card of its
/// Literature tree, against its Literature-tree Idea Cards (root and anchors left out).
fn share_of(cards: &[Value], key: &str) -> Value {
    let anchor = Regex::new(&format!(r"^(?i){}/\d+$", regex::escape(key))).unwrap();
    let mut topic: HashSet<String> = HashSet::new();
    let mut literature = 0;
    for card in cards {
        let at = s(&card["address"]);
        if at != key && !anchor.is_match(&at) && s(&card["type"]) == "idea" {
            literature += 1;
        }
        for (source, _) in reverse_links(&s(&card["text"])) {
            if in_topic_tree(&source) && !is_root(&source) {
                topic.insert(source);
            }
        }
    }
    let total = topic.len() + literature;
    let share = if total == 0 { Value::Null } else { json!(((topic.len() as f64 / total as f64) * 1000.0).round() / 1000.0) };
    json!({ "topic": topic.len(), "literature": literature, "share": share, "topic_cards": topic.into_iter().collect::<std::collections::BTreeSet<_>>() })
}

/// The two closest-Card blocks of one section file (Tree Lock S15): the Cards an idea may continue
/// (an agent Card, or a human Card to link), and the Locked ones. A Locked Card stays related context
/// and a Link target, and is never offered as a Card to continue.
fn closest_blocks(open: &[(f64, Value)], locked: &[(f64, Value)]) -> String {
    let mut md = String::new();
    if !open.is_empty() {
        md.push_str("## 用词最接近本节的已有主题卡\n\n");
        for (_, t) in open.iter().take(CLOSEST) {
            md.push_str(&format!("- [[{}]] {}（{}）\n", s(&t["address"]), s(&t["title"]), if s(&t["workspace"]) == "agent" { "代理，可接续" } else { "人类，只读，可链接" }));
        }
        md.push('\n');
    }
    if !locked.is_empty() {
        md.push_str("## 用词最接近的已锁定主题卡（只读：可作相关语境，可链接，不能写入）\n\n");
        for (_, t) in locked.iter().take(CLOSEST) {
            md.push_str(&format!("- [[{}]] {}（已锁定：只读，可链接）\n", s(&t["address"]), s(&t["title"])));
        }
        md.push('\n');
    }
    md
}

fn read_book(zt: &mut Zt, key: &str) -> Result<(Value, Vec<Value>)> {
    let tree = zt.call("get_tree", json!({ "tree": key, "bodies": true }))?;
    if tree["complete"] != true {
        bail!("the tree `{key}` was not read completely");
    }
    if s(&tree["zone"]) != "literature" {
        bail!("`{key}` is not a Literature tree (zone {})", s(&tree["zone"]));
    }
    let cards = tree["cards"].as_array().cloned().context("get_tree gave no cards")?;
    Ok((tree, cards))
}

/// The job files' commands for this run: `zt-digest` on the PATH by default, else the given binary
/// (`-Tool`), and the zt config of the box the helper reads (`-ZtConfig`: a copy of the box runs its
/// own zt service). The SELF-CHECK reads no Card, so it needs no config.
fn retool(plan: &Value, text: &str) -> String {
    let tool = s(&plan["tool"]);
    let exe = if tool.is_empty() { "zt-digest".to_string() } else { format!("& '{tool}'") };
    let env = s(&plan["zt_config"]);
    let text = text.replace("SELF-CHECK: zt-digest grow ", &format!("SELF-CHECK: {exe} grow "));
    text.replace("{{CALLENV}}", &if env.is_empty() { String::new() } else { format!("$env:ZT_CONFIG_DIR='{env}'\n") }).replace("{{TOOL}}", &exe)
}

fn instructions_file(given: &str) -> Result<String> {
    if !given.is_empty() {
        return Ok(given.to_string());
    }
    let dir = match std::env::var("ZT_DIGEST_SCRIPTS") {
        Ok(dir) if !dir.is_empty() => dir,
        _ => crate::model::Scripts::find().dir,
    };
    // scripts\book-digest → scripts\idea-grow
    let repo = crate::ps::parent(&dir.trim_end_matches(['\\', '/']).to_string());
    Ok(join_path(&join_path(&repo, "idea-grow"), "dsh-grow-instructions.zh.md"))
}

/// The books of a run: `-Books a,b` or `-Book a`, else every Literature tree of the box (both
/// Workspaces). The box is the unit: the more Literature trees it holds, the more each section's
/// ideas can relate to (Luhmann's main box grew from all his reading, not from one book at a time).
fn books_of(zt: &mut Zt, a: &Args) -> Result<Vec<String>> {
    let mut named: Vec<String> = a.str("Books").split([',', ';']).chain(std::iter::once(a.str("Book").as_str())).map(|k| k.trim().to_string()).filter(|k| !k.is_empty()).collect();
    named.dedup();
    if !named.is_empty() {
        return Ok(named);
    }
    let listed = zt.call("list_trees", json!({ "zone": "literature" }))?;
    Ok(listed["trees"].as_array().into_iter().flatten().filter(|t| s(&t["kind"]) == "literature").map(|t| s(&t["root"])).collect())
}

/// The Topic share of the run's books together: the distinct agent Topic Cards that link any of
/// them, against all their Literature-tree Idea Cards, and the share of each book.
fn box_share(books: &[(String, Vec<Value>)]) -> Value {
    let mut topic: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut literature = 0u64;
    let mut each = Map::new();
    for (key, cards) in books {
        let one = share_of(cards, key);
        literature += one["literature"].as_u64().unwrap_or(0);
        for t in one["topic_cards"].as_array().into_iter().flatten() {
            topic.insert(s(t));
        }
        each.insert(key.clone(), json!({ "topic": one["topic"], "literature": one["literature"], "share": one["share"] }));
    }
    let total = topic.len() as u64 + literature;
    let share = if total == 0 { Value::Null } else { json!(((topic.len() as f64 / total as f64) * 1000.0).round() / 1000.0) };
    json!({ "topic": topic.len(), "literature": literature, "share": share, "books": each })
}

fn plan(a: &Args) -> Result<()> {
    let work = Work::open(&a.required("Work")?)?;
    let from = a.int_or("From", 1)?;
    let to = a.int_or("To", i64::MAX)?;
    let template = util::read_text(instructions_file(&a.str("Instructions"))?)?;
    let mut zt = crate::zt::start()?;
    let keys = books_of(&mut zt, a)?;
    if keys.is_empty() {
        bail!("the box holds no Literature tree to grow from");
    }
    let mut books: Vec<(String, String, Value, Vec<Value>)> = Vec::new();
    for k in &keys {
        let (tree, cards) = read_book(&mut zt, k)?;
        books.push((s(&tree["root"]), s(&tree["title"]), tree["workspace"].clone(), cards));
    }

    // The whole box: the snapshot DeepSeek greps, and the titles precheck compares with.
    let details = snapshot::read_title_trees(&mut zt)?;
    let (snapshot_text, count) = snapshot::render(&details, 60);
    util::write_text(work.path("snapshot.md"), &snapshot_text)?;
    let mut titles = Vec::new();
    for t in &details {
        for card in t["cards"].as_array().into_iter().flatten() {
            let at = s(&card["address"]);
            titles.push(json!({ "address": at, "title": card["title"], "workspace": t["workspace"], "type": card["type"], "facet": card["facet"], "lock": card["lock"] }));
        }
    }
    work.write("titles.json", &Value::Array(titles.clone()))?;

    let listed = zt.call("list_trees", json!({}))?;
    let mut topics_md = String::from("# 主题目录（代理与人类工作区；人类主题只读，可链接）\n\n");
    let mut literature_md = String::from("\n# 文献目录（卡片盒里的全部文献树）\n\n");
    let mut topic_count = 0;
    let mut locked_topics = 0;
    for t in listed["trees"].as_array().into_iter().flatten() {
        let side = if s(&t["workspace"]) == "agent" { "代理" } else { "人类" };
        if s(&t["kind"]) == "topic" {
            topic_count += 1;
            // A Locked Topic is readable context and a Link target only (Tree Lock S15).
            let locked = if t["locked"] == true {
                locked_topics += 1;
                "，已锁定：只读，可链接，不能写入"
            } else {
                ""
            };
            topics_md.push_str(&format!("- [[{}]] {}（{side}，{} 张{locked}）：{}\n", s(&t["root"]), s(&t["title"]), s(&t["count"]), cut(&s(&t["excerpt"]), 80)));
        } else {
            literature_md.push_str(&format!("- [[{}]] 《{}》（{side}，{} 张）\n", s(&t["root"]), s(&t["title"]), s(&t["count"])));
        }
    }
    topics_md.push_str(&literature_md);
    util::write_text(work.path("topics.md"), &topics_md)?;

    let topic_cards: Vec<&Value> = titles.iter().filter(|t| in_topic_tree(&s(&t["address"])) && !is_root(&s(&t["address"]))).collect();
    let literature_cards: Vec<&Value> = titles.iter().filter(|t| !in_topic_tree(&s(&t["address"])) && s(&t["address"]).contains('/')).collect();
    let state = work.state()?;
    let mut sections = Vec::new();
    let mut jobs = Vec::new();
    let mut n = 0i64;
    for (key, btitle, _, cards) in &books {
        // Sections: each anchor `Key/n` with its subtree, in the anchors' order; a tree with no
        // anchor is one section of its own (its root and every Card).
        let anchor = Regex::new(&format!(r"^(?i){}/(\d+)$", regex::escape(key))).unwrap();
        let mut anchors: Vec<(i64, Value)> = cards
            .iter()
            .filter_map(|c| anchor.captures(&s(&c["address"])).and_then(|m| m[1].parse::<i64>().ok()).map(|k| (k, c.clone())))
            .collect();
        anchors.sort_by_key(|(k, _)| *k);
        let whole = anchors.is_empty();
        if whole {
            if let Some(root) = cards.iter().find(|c| s(&c["address"]) == *key) {
                anchors.push((1, root.clone()));
            }
        }
        for (k, head) in &anchors {
            if *k < from || *k > to {
                continue;
            }
            n += 1;
            let at = s(&head["address"]);
            let prefix = format!("{at}|");
            let members: Vec<&Value> = cards.iter().filter(|c| whole || s(&c["address"]) == at || s(&c["address"]).starts_with(&prefix)).collect();
            let mut md = format!("# 第 {n} 节：《{btitle}》（[[{key}]]）的 [[{at}]] {}\n\n本节属于《{btitle}》，引用键 `{key}`：来源行写 `来源：[[{key}]]`，本书的文献卡用 `[[{key}/…]]`。\n\n", s(&head["title"]));
            let mut text_all = String::new();
            let mut linked: HashSet<String> = HashSet::new();
            for card in &members {
                let address = s(&card["address"]);
                let label = if address == at && !whole { "锚点".to_string() } else if address == *key { "文献卡".to_string() } else { type_label(card) };
                md.push_str(&format!("## [[{address}]] {}（{label}）\n\n{}\n\n", s(&card["title"]), cut(&s(&card["body"]), BODY_CHARS)));
                let topic_links: Vec<(String, String)> = reverse_links(&s(&card["text"])).into_iter().filter(|(a, _)| in_topic_tree(a)).collect();
                if !topic_links.is_empty() {
                    md.push_str("已链接本卡的主题卡：");
                    md.push_str(&topic_links.iter().map(|(a, t)| format!("[[{a}]] {t}")).collect::<Vec<_>>().join("；"));
                    md.push_str("\n\n");
                    linked.extend(topic_links.into_iter().map(|(a, _)| a));
                }
                text_all.push_str(&s(&card["title"]));
                text_all.push_str(&s(&card["body"]));
            }
            let grams = bigrams(&text_all);
            let score = |t: &&Value| -> Option<f64> {
                let g = bigrams(&s(&t["title"]));
                if g.len() < 3 {
                    return None;
                }
                let shared = g.iter().filter(|x| grams.contains(x)).count();
                let score = shared as f64 / g.len() as f64;
                (shared >= 2 && score >= 0.2).then_some(score)
            };
            let rank = |list: &[&Value], skip: &dyn Fn(&Value) -> bool| -> Vec<(f64, Value)> {
                let mut scored: Vec<(f64, Value)> = list.iter().filter(|t| !skip(t)).filter_map(|t| score(t).map(|x| (x, (*t).clone()))).collect();
                scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal).then_with(|| s(&x.1["address"]).cmp(&s(&y.1["address"]))));
                scored
            };
            let closest = rank(&topic_cards, &|t| linked.contains(&s(&t["address"])));
            // The Locked Cards are context and Link targets only: they are listed apart from the Cards
            // an idea may continue, so a planner never reads one as a destination (S15, AC01/AC02).
            let (open_cards, locked_cards): (Vec<(f64, Value)>, Vec<(f64, Value)>) = closest.into_iter().partition(|(_, t)| t["lock"].is_null());
            md.push_str(&closest_blocks(&open_cards, &locked_cards));
            // Other books: the notes of other Literature trees closest to this section, so an idea
            // can contrast, confirm or generalize what another source says.
            let others = rank(&literature_cards, &|t| tree_of(&s(&t["address"])).eq_ignore_ascii_case(key));
            if !others.is_empty() {
                md.push_str("## 其他文献里用词接近的笔记（可以对照、印证、推广）\n\n");
                for (_, t) in others.iter().take(CLOSEST) {
                    md.push_str(&format!("- [[{}]] {}（{}）\n", s(&t["address"]), s(&t["title"]), tree_of(&s(&t["address"]))));
                }
                md.push('\n');
            }
            let file = work.path(&format!("sections\\s{n:03}.md"));
            util::write_text(&file, &md)?;
            let addresses: Vec<String> = members.iter().map(|c| s(&c["address"])).collect();
            sections.push(json!({ "n": n, "anchor": at, "book": key, "book_title": btitle, "file": file, "cards": addresses }));
            if state["sections"][&at]["applied"] == true {
                continue;
            }
            jobs.push(at);
        }
    }
    // At most MAX_JOBS sections per run, in book order: a rerun with the same folder takes the rest.
    let left = jobs.len().saturating_sub(MAX_JOBS);
    jobs.truncate(MAX_JOBS);
    let snapshot_path = work.path("snapshot.md");
    let topics_path = work.path("topics.md");
    let allow = work.path("read-only.allow.txt");
    util::write_text(&allow, "")?;
    let (key, title) = if books.len() == 1 { (books[0].0.clone(), format!("《{}》", books[0].1)) } else { ("Key".to_string(), format!("卡片盒里的 {} 部文献", books.len())) };
    let instructions = template
        .replace("{{TITLE}}", &title)
        .replace("{{KEY}}", &key)
        .replace("{{WORK}}", &work.dir)
        .replace("{{SNAPSHOT}}", &snapshot_path)
        .replace("{{TOPICS}}", &topics_path)
        .replace("{{ALLOW}}", &allow)
        .replace("{{MAX}}", &MAX_IDEAS.to_string());
    let tools = json!({ "tool": a.str("Tool"), "zt_config": a.str("ZtConfig") });
    let instructions = retool(&tools, &instructions);
    let instructions_path = work.path("instructions.zh.md");
    util::write_text(&instructions_path, &instructions)?;
    // The run's Topics are planned first, over every section of every book (gt-001): the sections
    // then file under them, so parallel sections do not each open a Topic for one concept (pilot 1:
    // eight Topics, four of them on the organization mode), and one concept gathers its ideas from
    // several sources. The section jobs are written by -Step topics.
    let pending: Vec<Value> = jobs.iter().map(|at| json!(at)).collect();
    let topic_jobs = if pending.is_empty() || state["run_topics"].is_array() { 0 } else { 1 };
    if topic_jobs == 1 {
        let answer = work.path("out\\gt-001.json");
        let _ = std::fs::remove_file(&answer);
        let mut index = String::new();
        let mut current = String::new();
        for x in &sections {
            if !pending.contains(&x["anchor"]) {
                continue;
            }
            if s(&x["book"]) != current {
                current = s(&x["book"]);
                index.push_str(&format!("\n### 《{}》[[{current}]]\n\n", s(&x["book_title"])));
            }
            let head = books.iter().flat_map(|b| b.3.iter()).find(|c| s(&c["address"]) == s(&x["anchor"])).cloned().unwrap_or(Value::Null);
            let lines: Vec<String> = s(&head["body"]).lines().filter(|l| { let t = l.trim_start(); t.starts_with('§') || t.starts_with('第') }).take(8).map(|l| format!("  {}", cut(l, 80))).collect();
            index.push_str(&format!("- 第 {} 节 [[{}]] {}\n{}\n", s(&x["n"]), s(&x["anchor"]), s(&head["title"]), lines.join("\n")));
        }
        let text = format!(
            "# 任务 gt-001：为{title}规划本次运行的主题\n\n先读任务说明 `{instructions_path}` 中的\"规划主题\"一节。\n\n- 答案文件：`{answer}`\n- 新建主题最多 {} 个\n\nSELF-CHECK: zt-digest grow -Step precheck -Work \"{}\" -Job gt-001\n\n## 各文献的各节\n{index}\n## 已有主题与文献\n\n{}",
            max_new_topics(pending.len()),
            work.dir,
            util::read_text(&topics_path)?
        );
        util::write_text(work.path("jobs\\gt-001.md"), &retool(&tools, &text))?;
    }
    let planned: Vec<Value> = Vec::new();
    // The share before the run: kept from the first plan once a section was applied.
    let applied = state["sections"].as_object().is_some_and(|m| m.values().any(|v| v["applied"] == true));
    let shares: Vec<(String, Vec<Value>)> = books.iter().map(|b| (b.0.clone(), b.3.clone())).collect();
    let baseline = match work.read("baseline.json") {
        Ok(old) if applied => old,
        _ => box_share(&shares),
    };
    work.write("baseline.json", &baseline)?;
    let book_rows: Vec<Value> = books.iter().map(|b| json!({ "key": b.0, "title": b.1, "workspace": b.2 })).collect();
    work.write(
        "plan.json",
        &json!({ "key": key, "title": title, "tool": tools["tool"], "zt_config": tools["zt_config"], "books": book_rows, "sections": sections, "pending": pending, "left": left, "jobs": planned, "instructions": instructions_path, "snapshot": snapshot_path, "topics": topics_path, "allow": allow }),
    )?;
    util::print_last(&json!({
        "topic_jobs": topic_jobs, "pending": pending.len(), "left": left, "books": books.len(), "sections": sections.len(), "cards": books.iter().map(|b| b.3.len()).sum::<usize>(), "box": count, "topics": topic_count, "locked": locked_topics,
        "baseline": { "topic": baseline["topic"], "literature": baseline["literature"], "share": baseline["share"] },
    }))
}

// --- validation ---------------------------------------------------------------------------------

struct Ctx {
    key: String,
    plan: Value,
    titles: Vec<Value>,
    state: Value,
}

impl Ctx {
    fn load(work: &Work) -> Result<Self> {
        let plan = work.read("plan.json")?;
        let titles = work.read("titles.json")?.as_array().cloned().unwrap_or_default();
        let state = work.state()?;
        Ok(Self { key: s(&plan["key"]), plan, titles, state })
    }
    fn job(&self, id: &str) -> Option<&Value> {
        self.plan["jobs"].as_array()?.iter().find(|j| s(&j["id"]) == id)
    }
    fn section(&self, anchor: &str) -> Option<&Value> {
        self.plan["sections"].as_array()?.iter().find(|x| s(&x["anchor"]) == anchor)
    }
    /// The Citation key of a section's book (the run may read many books).
    fn book_of(&self, anchor: &str) -> String {
        self.section(anchor).map(|x| s(&x["book"])).filter(|k| !k.is_empty()).unwrap_or_else(|| self.key.clone())
    }
    /// The book of a Card this run wrote (its section's book).
    fn book_of_card(&self, address: &str) -> String {
        self.book_of(&s(&self.state["cards"][address]["section"]))
    }
    fn title_of(&self, address: &str) -> Option<&Value> {
        self.titles.iter().find(|t| s(&t["address"]) == address)
    }
    /// The Topics this run planned (-Step topics): `{id, root, title, body, sections, new}`.
    fn run_topics(&self) -> Vec<&Value> {
        self.state["run_topics"].as_array().map(|l| l.iter().collect()).unwrap_or_default()
    }
    /// A Card of the box (at plan time), a run Topic, or a Card this run wrote.
    fn exists(&self, address: &str) -> bool {
        self.title_of(address).is_some() || self.state["cards"].get(address).is_some() || self.run_topics().iter().any(|t| s(&t["root"]) == address)
    }
    /// The active Lock of a Card's tree, as the plan's title export recorded it (Tree Lock S15): a
    /// Locked Card stays readable context and a Link target, and is never a destination. A Card this
    /// run wrote, or a Card with no Lock at plan time, answers `None`; the storage gate still decides
    /// at the write.
    fn lock_of(&self, address: &str) -> Option<lockcheck::Lock> {
        self.title_of(address).and_then(lockcheck::lock_fact)
    }
    fn workspace(&self, address: &str) -> String {
        if self.state["cards"].get(address).is_some() || self.run_topics().iter().any(|t| s(&t["root"]) == address) {
            return "agent".into();
        }
        self.title_of(address).map(|t| s(&t["workspace"])).unwrap_or_default()
    }
}

/// One fault: the idea or topic id, the code, the message.
type Fault = (String, String, String);

/// The G12 message of a Locked destination (Tree Lock S15, R330/R335): the Card, its Lock, and the
/// ways out. It names the Lock, so no caller reads a Locked destination as a free one.
fn locked_destination(at: &str, same: bool, lock: &lockcheck::Lock) -> String {
    if same {
        format!(
            "`same`: `{at}` is locked by the owner (Lock {}, tree `{}`): it takes no appended sentence. Write the idea as a new Card under an unlocked agent Card instead",
            lock.lock_id, lock.tree
        )
    } else {
        format!(
            "`{at}` is inside the locked tree `{}` (Lock {}): a locked Card stays readable and linkable but takes no write. Put the idea under an unlocked agent Card, or declare a new Topic",
            lock.tree, lock.lock_id
        )
    }
}

/// The G12 message of a new Topic whose name would be filed in a Locked Topic.
fn locked_topic(root: &str, title: &str, lock: &lockcheck::Lock) -> String {
    let name = if title.trim().is_empty() { String::new() } else { format!(" {title}") };
    format!(
        "the Topic `{root}`{name} is locked by the owner (Lock {}, tree `{}`): it takes no new Cards. Name this concept differently, or plan an unlocked Topic",
        lock.lock_id, lock.tree
    )
}

fn answer_of(path: &str) -> std::result::Result<Value, String> {
    let raw = util::read_text(path).map_err(|e| format!("{e:#}"))?;
    let raw = raw.trim_start_matches('\u{feff}');
    if raw.trim().is_empty() {
        return Err("the file is empty".into());
    }
    serde_json::from_str::<Value>(raw).map_err(|e| e.to_string())
}

/// The drafts zt-check reads for the ideas: category idea, so the Card layer rules apply.
fn drafts_of(key: &str, ideas: &[&Value]) -> Vec<Value> {
    ideas
        .iter()
        .filter(|i| s(&i["same"]).is_empty())
        .map(|i| json!({ "address": format!("idea:{}", s(&i["id"])), "title": s(&i["title"]), "body": with_source(key, &s(&i["body"]), &[]), "type": "idea", "category": "idea", "book": key, "workspace": "agent" }))
        .collect()
}

/// The body with its source line: `来源：[[Key]]` appended when the body has none.
fn with_source(key: &str, body: &str, _sources: &[String]) -> String {
    if SOURCE_LINE.is_match(body) {
        body.trim_end().to_string()
    } else {
        format!("{}\n\n来源：[[{key}]]", body.trim_end())
    }
}

fn zt_check_drafts(work: &Work, tag: &str, drafts: Vec<Value>) -> Result<Vec<String>> {
    if drafts.is_empty() {
        return Ok(Vec::new());
    }
    let file = work.path(&format!("drafts-{tag}.json"));
    util::write_text(&file, &serde_json::to_string_pretty(&Value::Array(drafts))?)?;
    let check = Check { json: file, ..Check::default() };
    let lines = zt_check::check::run_lines(&check.args(), None).map_err(|e| anyhow::anyhow!("zt-check did not run: {e:#}"))?;
    Ok(lines.into_iter().filter(|l| !FAULTS.is_match(l) && !l.contains("(advisory)")).collect())
}

/// The faults of one growth answer (G1–G13). zt-check runs separately on the drafts.
fn validate(ctx: &Ctx, section: &str, answer: &Value) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut add = |id: &str, code: &str, message: String| faults.push((id.to_string(), code.to_string(), message));
    let Some(map) = answer.as_object() else {
        add("answer", "G1", "the answer is not a JSON object".into());
        return faults;
    };
    if s(&answer["section"]) != section {
        add("answer", "G1", format!("\"section\" must be `{section}`"));
    }
    let ideas: Vec<&Value> = answer["ideas"].as_array().map(|l| l.iter().collect()).unwrap_or_default();
    if ideas.is_empty() && s(&answer["none"]).trim().is_empty() {
        add("answer", "G1", "no ideas and no \"none\" reason".into());
    }
    if ideas.len() > MAX_IDEAS {
        add("answer", "G1", format!("{} ideas; at most {MAX_IDEAS}: keep the ones worth keeping", ideas.len()));
    }
    let _ = map;
    let members: Vec<String> = ctx.section(section).and_then(|x| x["cards"].as_array()).map(|l| l.iter().map(s).collect()).unwrap_or_default();
    let key = ctx.book_of(section);
    let topics: Vec<&Value> = answer["topics"].as_array().map(|l| l.iter().collect()).unwrap_or_default();
    let mut ids: HashSet<String> = HashSet::new();
    let run_topics: Vec<Value> = ctx.run_topics().iter().map(|t| json!({ "address": t["root"], "title": t["title"] })).collect();
    let agent_topics: Vec<&Value> = ctx.titles.iter().filter(|t| s(&t["workspace"]) == "agent" && is_root(&s(&t["address"]))).chain(run_topics.iter()).collect();
    for t in &topics {
        let id = s(&t["id"]);
        if !id.starts_with('T') || !ids.insert(id.clone()) {
            add(&id, "G1", "a topic id is T1, T2 …, once each".into());
        }
        if s(&t["title"]).trim().is_empty() || s(&t["body"]).trim().is_empty() {
            add(&id, "G1", "a new Topic needs title and body (2–3 sentences: the question it asks)".into());
        }
        if let Some(same) = agent_topics.iter().find(|x| near(&s(&x["title"]), &s(&t["title"]))) {
            let root = s(&same["address"]);
            match ctx.lock_of(&root) {
                Some(lock) => add(&id, "G12", locked_topic(&root, &s(&same["title"]), &lock)),
                None => add(&id, "G8", format!("the Topic `{root}` {} already exists: file the ideas under it (at its Cards) instead", s(&same["title"]))),
            }
        }
    }
    let mut earlier: Vec<String> = Vec::new();
    let mut titles_here: Vec<(String, String)> = Vec::new();
    // The idea each Topic root takes in this answer: one per Topic, so a section opens one line there.
    let mut at_root: HashMap<String, String> = HashMap::new();
    for idea in &ideas {
        let id = s(&idea["id"]);
        if !id.starts_with('I') || !ids.insert(id.clone()) {
            add(&id, "G1", "an idea id is I1, I2 …, once each".into());
        }
        let kind = s(&idea["kind"]);
        if !KINDS.contains(&kind.as_str()) {
            add(&id, "G1", format!("kind `{kind}` is not one of {}", KINDS.join("/")));
        }
        let sources: Vec<String> = idea["sources"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
        if sources.is_empty() {
            add(&id, "G2", "name the section's Cards the idea comes from in \"sources\"".into());
        }
        for src in &sources {
            if !members.iter().any(|m| m.eq_ignore_ascii_case(src)) {
                add(&id, "G2", format!("`{src}` is not a Card of this section"));
            }
        }
        let body = s(&idea["body"]);
        let same = s(&idea["same"]);
        if !cites_with_kind(&body, &sources, &CITE_KINDS) {
            add(&id, "G3", format!("cite a source note in the sentence that uses it, with a real relation before the Link ({CITE_WORDS}), e.g. `例示 [[{}]]：本书……`", sources.first().cloned().unwrap_or_else(|| format!("{key}/n"))));
        }
        for target in plain_links(&body) {
            add(&id, "G10", format!("`[[{target}]]` reads as a plain reference: put the relation's kind word before it in its sentence ({KIND_WORDS}; `引出`, `见`, `由…可得` are no kind)"));
        }
        for target in rests_on_book(&body, &key) {
            add(&id, "G4", format!("`[[{target}]]` reads as rests-on into its own book: write the assumption into the Card, and cite the note with 例示/对应"));
        }
        for m in LINK.captures_iter(&body) {
            let target = m[1].trim();
            if let Some(r) = target.strip_prefix('@') {
                if !earlier.contains(&r.to_string()) && !topics.iter().any(|t| s(&t["id"]) == r) {
                    add(&id, "G5", format!("`[[@{r}]]` names no earlier idea or declared Topic"));
                }
            } else if !ctx.exists(target) {
                add(&id, "G5", format!("`[[{target}]]` is not a Card of the box"));
            }
        }
        if !same.is_empty() {
            if !ctx.exists(&same) {
                add(&id, "G9", format!("`same`: `{same}` is not a Card of the box"));
            } else if !in_topic_tree(&same) {
                add(&id, "G9", format!("`same`: `{same}` is not a Topic-tree Card; a Literature note is the source, not the same idea"));
            } else if let Some(lock) = ctx.lock_of(&same) {
                add(&id, "G12", locked_destination(&same, true, &lock));
            }
            earlier.push(id);
            continue;
        }
        let title = s(&idea["title"]);
        if title.trim().is_empty() || body.trim().is_empty() {
            add(&id, "G1", "an idea needs title and body".into());
        }
        let at = s(&idea["at"]);
        let side = s(&idea["as"]);
        if !side.is_empty() && side != "direct" && side != "side" {
            add(&id, "G5", "\"as\" is direct or side".into());
        }
        if (at.starts_with('T') && !at.contains('/')) || is_root(&at) {
            match at_root.get(&at) {
                Some(first) => add(&id, "G13", format!("`{at}` already takes {first} in this answer: a section opens one line in a Topic, so chain this idea from it (`\"at\": \"{first}\"`, or a later idea of that line) or continue a named Card of the Topic")),
                None => {
                    at_root.insert(at.clone(), id.clone());
                }
            }
        }
        if at.starts_with('T') && !at.contains('/') {
            if !topics.iter().any(|t| s(&t["id"]) == at) {
                add(&id, "G5", format!("`{at}` is not a declared new Topic"));
            }
        } else if at.starts_with('I') && !at.contains('/') {
            if !earlier.contains(&at) {
                add(&id, "G5", format!("`{at}` is not an earlier idea of this answer"));
            }
        } else if at.is_empty() {
            add(&id, "G5", "\"at\": the Card this idea continues (an agent Topic Card), an earlier idea, or a new Topic".into());
        } else if !ctx.exists(&at) {
            add(&id, "G5", format!("`{at}` is not a Card of the box"));
        } else if !in_topic_tree(&at) {
            add(&id, "G5", format!("`{at}` is a Literature-tree Card: this workflow files ideas in Topic trees only"));
        } else if ctx.workspace(&at) != "agent" {
            add(&id, "G5", format!("`{at}` is a human Card and takes no agent successor: file the idea in an agent Topic (a new one if needed) and open it with `延续 [[{at}]]：…`"));
        } else if let Some(lock) = ctx.lock_of(&at) {
            add(&id, "G12", locked_destination(&at, false, &lock));
        } else if !is_root(&at) && !link_kinds(&first_sentence(&body)).iter().any(|(t, _, k)| t == &at && *k != "refers") {
            add(&id, "G6", format!("the idea continues `{at}`: its first sentence says how, with a kind word before the Link ({KIND_WORDS}), e.g. `本卡放宽 [[{at}]] 的……假设。`"));
        }
        for src in &sources {
            if let Some(t) = ctx.title_of(src)
                && near(&s(&t["title"]), &title)
            {
                add(&id, "G7", format!("the title repeats its source note `{src}`: state the general claim in your own words"));
            }
        }
        if let Some(t) = ctx.titles.iter().find(|t| !sources.contains(&s(&t["address"])) && near(&s(&t["title"]), &title)) {
            add(&id, "G7", format!("the title is near `{}` {}: if it is the same claim use \"same\", else say the difference in the title", s(&t["address"]), s(&t["title"])));
        }
        if let Some((other, _)) = titles_here.iter().find(|(_, t)| near(t, &title)) {
            add(&id, "G7", format!("the title is near idea {other} of this answer: one claim, one Card"));
        }
        titles_here.push((id.clone(), title));
        earlier.push(id);
    }
    faults
}

fn print_faults(faults: &[Fault], checked: &[String]) -> Result<()> {
    let mut lines: Vec<String> = faults.iter().map(|(id, code, message)| format!("{id} {code} {message}")).collect();
    lines.extend(checked.iter().cloned());
    let n = lines.len();
    lines.push(format!("faults: {n}"));
    util::print_lines(&lines)
}

fn precheck(a: &Args) -> Result<()> {
    let work = Work::open(&a.required("Work")?)?;
    let id = a.required("Job")?;
    let ctx = Ctx::load(&work)?;
    let path = work.path(&format!("out\\{id}.json"));
    let answer = match answer_of(&path) {
        Ok(v) => v,
        Err(e) => return print_faults(&[("answer".into(), "G1".into(), format!("{path}: {e}"))], &[]),
    };
    if id == "gt-001" {
        let faults = validate_topics(&ctx, &answer);
        let drafts: Vec<Value> = answer["topics"].as_array().into_iter().flatten().filter(|t| s(&t["existing"]).is_empty()).map(|t| json!({ "address": format!("topic:{}", s(&t["id"])), "title": s(&t["title"]), "body": s(&t["body"]), "type": "idea" })).collect();
        let checked = zt_check_drafts(&work, &id, drafts)?;
        return print_faults(&faults, &checked);
    }
    if id.starts_with("gl") {
        let faults = validate_links(&ctx, &id, &answer);
        let drafts: Vec<Value> = answer["links"].as_array().into_iter().flatten().enumerate().map(|(i, l)| json!({ "address": format!("link:{}", i + 1), "title": "跨树链接", "body": s(&l["sentence"]), "type": "idea" })).collect();
        let checked = zt_check_drafts(&work, &id, drafts)?;
        return print_faults(&faults, &checked);
    }
    if id.starts_with("fx") {
        let (faults, drafts) = validate_fix(&ctx, &id, &answer);
        let checked = zt_check_drafts(&work, &id, drafts)?;
        return print_faults(&faults, &checked);
    }
    let Some(job) = ctx.job(&id) else { bail!("no job `{id}` in plan.json") };
    let faults = validate(&ctx, &s(&job["section"]), &answer);
    let ideas: Vec<&Value> = answer["ideas"].as_array().map(|l| l.iter().collect()).unwrap_or_default();
    let checked = zt_check_drafts(&work, &id, drafts_of(&ctx.book_of(&s(&job["section"])), &ideas))?;
    print_faults(&faults, &checked)
}

/// The faults of a fix answer and its drafts: every edit names a Card of the fix job.
fn validate_fix(ctx: &Ctx, id: &str, answer: &Value) -> (Vec<Fault>, Vec<Value>) {
    let mut faults = Vec::new();
    let mut drafts = Vec::new();
    let cards: Vec<String> = ctx.state["fixes"][id]["cards"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    let key = ctx.book_of(&s(&ctx.state["fixes"][id]["section"]));
    let edits = answer["edits"].as_array().cloned().unwrap_or_default();
    if edits.is_empty() {
        faults.push(("answer".into(), "G1".into(), "no \"edits\"".into()));
    }
    for e in &edits {
        let at = s(&e["at"]);
        if !cards.contains(&at) {
            faults.push((at.clone(), "G5".into(), "not a Card of this fix job".into()));
            continue;
        }
        let body = s(&e["body"]);
        for target in rests_on_book(&body, &key) {
            faults.push((at.clone(), "G4".into(), format!("`[[{target}]]` reads as rests-on into its own book: write the assumption in, cite with 例示/对应")));
        }
        for target in plain_links(&body) {
            faults.push((at.clone(), "G10".into(), format!("`[[{target}]]` reads as a plain reference: put a kind word before it ({KIND_WORDS})")));
        }
        drafts.push(json!({ "address": at, "title": s(&e["title"]), "body": with_source(&key, &body, &[]), "type": "idea", "category": "idea", "book": key, "workspace": "agent" }));
    }
    (faults, drafts)
}

// --- the run's Topics -----------------------------------------------------------------------

/// The new Topics one run may open: about one per two sections, at least 2, at most 8.
fn max_new_topics(sections: usize) -> usize {
    sections.div_ceil(2).clamp(2, 8)
}

/// The faults of the topic answer (gt-001).
fn validate_topics(ctx: &Ctx, answer: &Value) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut add = |id: &str, code: &str, message: String| faults.push((id.to_string(), code.to_string(), message));
    let Some(list) = answer["topics"].as_array() else {
        add("answer", "G1", "the answer needs \"topics\": [...] (an empty list when the box already has every concept)".into());
        return faults;
    };
    let pending: Vec<String> = ctx.plan["pending"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    let numbers: Vec<i64> = ctx.plan["sections"].as_array().into_iter().flatten().filter(|x| pending.contains(&s(&x["anchor"]))).filter_map(|x| x["n"].as_i64()).collect();
    let agent_topics: Vec<&Value> = ctx.titles.iter().filter(|t| s(&t["workspace"]) == "agent" && is_root(&s(&t["address"]))).collect();
    let mut ids: HashSet<String> = HashSet::new();
    let mut new_titles: Vec<(String, String)> = Vec::new();
    for t in list {
        let id = s(&t["id"]);
        if !id.starts_with('T') || !ids.insert(id.clone()) {
            add(&id, "G1", "a topic id is T1, T2 …, once each".into());
        }
        let existing = s(&t["existing"]);
        if !existing.is_empty() {
            if !agent_topics.iter().any(|x| s(&x["address"]) == existing) {
                add(&id, "G5", format!("`existing`: `{existing}` is not an agent Topic root (n/0); a human Topic takes no agent Card"));
            } else if let Some(lock) = ctx.lock_of(&existing) {
                add(&id, "G12", locked_topic(&existing, "", &lock));
            }
        } else {
            let title = s(&t["title"]);
            if title.trim().is_empty() || s(&t["body"]).trim().is_empty() {
                add(&id, "G1", "a new Topic needs title (the concept's name) and body (2–3 sentences: the question it asks)".into());
            }
            if let Some(same) = agent_topics.iter().find(|x| near(&s(&x["title"]), &title)) {
                let root = s(&same["address"]);
                match ctx.lock_of(&root) {
                    Some(lock) => add(&id, "G12", locked_topic(&root, &s(&same["title"]), &lock)),
                    None => add(&id, "G8", format!("the Topic `{root}` {} already exists: give \"existing\": \"{root}\" instead", s(&same["title"]))),
                }
            }
            if let Some((other, _)) = new_titles.iter().find(|(_, x)| near(x, &title)) {
                add(&id, "G8", format!("the new Topic is near {other} of this answer: one concept, one Topic"));
            }
            new_titles.push((id.clone(), title));
        }
        let sections: Vec<i64> = t["sections"].as_array().map(|l| l.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        if sections.is_empty() {
            add(&id, "G2", "\"sections\": the numbers of the sections whose ideas belong in this Topic".into());
        }
        for n in sections {
            if !numbers.contains(&n) {
                add(&id, "G2", format!("section {n} is not one of this run's sections"));
            }
        }
    }
    let bound = max_new_topics(pending.len());
    if new_titles.len() > bound {
        add("answer", "G1", format!("{} new Topics; at most {bound}: keep the concepts few and file the rest under them", new_titles.len()));
    }
    faults
}

/// The section jobs, with the run's Topics; returns their count.
fn write_section_jobs(work: &Work, plan: &mut Value, run_topics: &[Value]) -> Result<usize> {
    let title = s(&plan["title"]);
    let instructions = s(&plan["instructions"]);
    let pending: Vec<String> = plan["pending"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    let mut block = String::new();
    if !run_topics.is_empty() {
        block.push_str("## 本次运行的主题（先放进这些主题）\n\n");
        for t in run_topics {
            let n: Vec<String> = t["sections"].as_array().into_iter().flatten().map(|v| format!("第 {} 节", s(v))).collect();
            block.push_str(&format!("- [[{}]] {}（{}）：{}（{}）\n", s(&t["root"]), s(&t["title"]), if t["new"] == true { "本次新建" } else { "已有" }, cut(&s(&t["body"]), 120), n.join("、")));
        }
        block.push_str("\n放置时优先用这些主题：接在主题里最贴切的卡后面，或写 `\"at\": \"<主题地址>\"`：主题还没有卡时，这张卡成为入口卡；否则脚本把它放在入口卡的旁支上，开出本节在这个主题里的一条思路。一节在一个主题的根上只放一个想法：本节放进这个主题的其余想法接在它后面（`\"at\": \"I1\"`），或接在主题里一张点名的卡后面（第二个也写主题地址时自查报 G13）。本次运行的其他小节也往这些主题里放卡，所以同一概念不要另建主题。只有这些主题和主题目录里都没有这个概念时，才在 `topics` 里新建。\n\n");
    }
    let mut planned = Vec::new();
    for (i, at) in pending.iter().enumerate() {
        let section = plan["sections"].as_array().into_iter().flatten().find(|x| s(&x["anchor"]) == *at).cloned().unwrap_or(Value::Null);
        let md = util::read_text(s(&section["file"]))?;
        let id = job_id("gw", i + 1);
        let answer = work.path(&format!("out\\{id}.json"));
        let _ = std::fs::remove_file(&answer);
        let text = format!(
            "# 任务 {id}：《{title}》的 [[{at}]] 一节\n\n先读任务说明 `{instructions}`（它在本文件之前已给出时不必再读）。\n\n- 本节：`{at}`（答案里的 \"section\" 写这个地址）\n- 答案文件：`{answer}`\n\nSELF-CHECK: zt-digest grow -Step precheck -Work \"{}\" -Job {id}\n\n{block}{md}",
            work.dir
        );
        util::write_text(work.path(&format!("jobs\\{id}.md")), &retool(plan, &text))?;
        planned.push(json!({ "id": id, "section": at, "answer": answer }));
    }
    let n = planned.len();
    plan["jobs"] = Value::Array(planned);
    work.write("plan.json", plan)?;
    Ok(n)
}

/// `-Step topics`: the topic answer validated, its new Topics made through the topic gate (a
/// near-name refusal reuses the existing Topic), the run's Topics recorded, and the section jobs
/// written. A rerun keeps the recorded Topics. A gate that reuses a Topic another Lock froze stops
/// the step with the typed refusal: it writes no Card inside the Locked tree.
fn topics(a: &Args) -> Result<i32> {
    let work = Work::open(&a.required("Work")?)?;
    let write = a.flag("Apply");
    let mut ctx = Ctx::load(&work)?;
    let mut created = 0;
    let mut refused: Vec<Value> = Vec::new();
    if !ctx.state["run_topics"].is_array() {
        let mut run: Vec<Value> = Vec::new();
        if let Ok(answer) = answer_of(&work.path("out\\gt-001.json")) {
            let faults = validate_topics(&ctx, &answer);
            let bad: HashSet<String> = faults.iter().map(|f| f.0.clone()).collect();
            if bad.contains("answer") {
                refused.push(json!({ "id": "answer", "faults": faults.iter().map(|f| format!("{} {}", f.1, f.2)).collect::<Vec<_>>() }));
            } else {
                let mut zt: Option<Zt> = None;
                for t in answer["topics"].as_array().into_iter().flatten() {
                    let id = s(&t["id"]);
                    if bad.contains(&id) {
                        refused.push(json!({ "id": id, "faults": faults.iter().filter(|f| f.0 == id).map(|f| format!("{} {}", f.1, f.2)).collect::<Vec<_>>() }));
                        continue;
                    }
                    let existing = s(&t["existing"]);
                    let (root, title, body, new) = if !existing.is_empty() {
                        let row = ctx.title_of(&existing).cloned().unwrap_or(Value::Null);
                        (existing, s(&row["title"]), String::new(), false)
                    } else if !write {
                        continue;
                    } else {
                        if zt.is_none() {
                            zt = Some(crate::zt::start()?);
                        }
                        let zt = zt.as_mut().unwrap();
                        match write_one(zt, json!({ "tool": "create_topic", "arguments": { "title": s(&t["title"]), "body": s(&t["body"]) } }), &work.path("topic.gate"))? {
                            Written::Made(root) => {
                                created += 1;
                                (root, s(&t["title"]), s(&t["body"]), true)
                            }
                            Written::Refused { error, refusal } => {
                                if let Some(lock) = refusal.as_ref().filter(|r| r["error"] == "card_locked") {
                                    let totals = json!({ "jobs": 0, "topics": run.len(), "created": created, "refused": refused.len() });
                                    return stop_stage(&totals, &archive_refusal("topics", "", lock));
                                }
                                match TOPIC_EXISTS.captures(&error) {
                                    Some(c) => {
                                        let root = c[1].to_string();
                                        // The gate reuses an existing Topic: the plan's title facts may be
                                        // older than a Lock the owner took since, so read the fresh fact.
                                        if let Some(lock) = lockcheck::lock_of(zt, &root)? {
                                            let message = format!("the Topic `{root}` is locked by the owner (Lock {}): it takes no new Cards", lock.lock_id);
                                            let refusal = locked_refusal("topics", &root, &lock.tree, lock.lock_id, &message, None);
                                            let totals = json!({ "jobs": 0, "topics": run.len(), "created": created, "refused": refused.len() });
                                            return stop_stage(&totals, &refusal);
                                        }
                                        (root, s(&t["title"]), s(&t["body"]), false)
                                    }
                                    None => {
                                        refused.push(json!({ "id": id, "faults": [format!("create_topic failed: {error}")] }));
                                        continue;
                                    }
                                }
                            }
                        }
                    };
                    run.push(json!({ "id": id, "root": root, "title": title, "body": body, "sections": t["sections"], "new": new }));
                }
            }
        }
        if !write {
            util::print_last(&json!({ "jobs": 0, "topics": run.len(), "created": 0, "refused": refused.len() }))?;
            return Ok(0);
        }
        ctx.state["run_topics"] = Value::Array(run);
        ctx.state["topics_refused"] = Value::Array(refused.clone());
        work.write("state.json", &ctx.state)?;
    }
    let run: Vec<Value> = ctx.state["run_topics"].as_array().cloned().unwrap_or_default();
    let mut plan = ctx.plan.clone();
    let jobs = write_section_jobs(&work, &mut plan, &run)?;
    util::print_last(&json!({ "jobs": jobs, "topics": run.len(), "created": created, "refused": refused.len() }))?;
    Ok(0)
}

// --- apply --------------------------------------------------------------------------------------

// --- Locked destinations (Tree Lock S15) ---------------------------------------------------------

/// The Lock facts of the archive's own `card_locked` refusal: its blocking tree and its first Lock id.
fn archive_lock(refusal: &Value) -> Option<(String, i64)> {
    let tree = refusal["tree"].as_str()?.to_string();
    let lock_id = refusal["locks"]
        .as_array()
        .and_then(|locks| locks.first())
        .and_then(|lock| lock["lock_id"].as_i64())
        .or_else(|| refusal["lock_id"].as_i64())?;
    Some((tree, lock_id))
}

/// The typed refusal a grow stage answers when its destination is Locked (S15, AC03/AC04): the S14
/// preflight line's vocabulary (`ok`, `code`, `tree`, `lock_id`), so a caller recognizes a Lock
/// without matching English text; the archive's own facts when its storage gate answered (`reason`,
/// `cards`, `locks`, `sources`); and the grow context (`mode`, `step`, `at`). It names no bypass: the
/// owner unlocks the tree in the GUI, and the Locked Cards stay readable as sources.
fn locked_refusal(step: &str, at: &str, tree: &str, lock_id: i64, message: &str, archive: Option<&Value>) -> Value {
    let mut value = json!({
        "ok": false, "code": "card_locked", "error": "card_locked", "message": message,
        "tree": tree, "lock_id": lock_id, "mode": "grow", "step": step, "at": at,
        "hint": "unlock the tree in the GUI first; its Cards stay readable as sources",
    });
    if let Some(archive) = archive {
        for key in ["reason", "cards", "locks", "sources"] {
            if let Some(fact) = archive.get(key) {
                value[key] = fact.clone();
            }
        }
    }
    value
}

/// The grow typed refusal built from the archive's own `card_locked` refusal: the archive sends its
/// blocking tree and Lock ids, so this never loses them.
fn archive_refusal(step: &str, at: &str, archive: &Value) -> Value {
    let (tree, lock_id) = archive_lock(archive).unwrap_or_default();
    let message = archive["message"].as_str().unwrap_or("the destination is locked by the owner");
    locked_refusal(step, at, &tree, lock_id, message, Some(archive))
}

/// The `stopped` fact of a job the archive refused because its destination is Locked (S15, AC04):
/// the address it tried, the Lock that stopped it and the archive's own sentence. A job with this
/// fact wrote nothing after the refusal and keeps its answers, so a rerun after the unlock resumes.
fn stopped_fact(at: &str, archive: &Value) -> Value {
    let (tree, lock_id) = archive_lock(archive).unwrap_or_default();
    json!({ "code": "card_locked", "at": at, "tree": tree, "lock_id": lock_id, "message": archive["message"].clone() })
}

/// The counters one job's partial record adds to the run totals (created, topics, same, refused,
/// duplicates), so a stopped stage still reports what it already made.
fn add_partial(totals: &mut Value, record: &Value) {
    for (key, field) in [("created", "created"), ("topics", "topics"), ("same", "same"), ("refused", "refused"), ("duplicates", "duplicates")] {
        let n = record[field].as_array().map_or(0, Vec::len);
        totals[key] = json!(totals[key].as_u64().unwrap_or(0) + n as u64);
    }
}

/// Stops a grow stage on a Locked destination: the totals line, then the typed refusal as the last
/// stdout line, the sentence on stderr, and the refusal exit code. It writes no further Card and
/// retries nothing.
fn stop_stage(totals: &Value, refusal: &Value) -> Result<i32> {
    util::print_line(&util::compact(totals))?;
    util::print_line(&util::compact(refusal))?;
    eprintln!("zt-digest grow: {}", refusal["message"].as_str().unwrap_or_default());
    Ok(lockcheck::LOCKED_EXIT)
}

/// One write through the helper; the address it made, or the refusal the archive answered.
enum Written {
    Made(String),
    Refused { error: String, refusal: Option<Value> },
}

fn write_one(zt: &mut Zt, call: Value, gate: &str) -> Result<Written> {
    let mut options = calls::Options::new();
    options.topic_gate = gate.to_string();
    let (done, failure) = calls::run(zt, &[call], &mut options)?;
    if let Some(f) = failure {
        return Ok(Written::Refused { error: f.lines().first().cloned().unwrap_or_default(), refusal: f.refusal.clone() });
    }
    let line = done.first().map(|d| d.line.clone()).unwrap_or_default();
    Ok(Written::Made(line.split_once(" ok ").map(|(_, a)| a.trim().to_string()).unwrap_or_default()))
}

/// The Card a root placement is filed beside: the doorway while it holds fewer than `MAX_LINES`
/// lines (Side successors `n/1|a` …), else its last line, and so on down while that one is full too.
/// The new line then branches off the doorway's last line instead of widening the doorway's fan.
fn line_parent(door: &str, exists: &mut dyn FnMut(&str) -> bool) -> String {
    let mut at = door.to_string();
    loop {
        let lines: Vec<String> = ('a'..='z').map(|c| format!("{at}|{c}")).take_while(|a| exists(a)).collect();
        match lines.last() {
            Some(last) if lines.len() >= MAX_LINES => at = last.clone(),
            _ => return at,
        }
    }
}

fn card_exists(zt: &mut Zt, address: &str) -> bool {
    calls::read(zt, "get_card", json!({ "at": address })).is_ok()
}

/// `-Step apply`: writes the answered jobs in job order. `Written::Refused` with the archive's
/// `card_locked` refusal stops the stage: the affected job keeps its answers and the receipts of the
/// Cards it already made, and the run answers the typed refusal with the refusal exit code. No Lock
/// is bypassed, no Card is placed elsewhere instead, and nothing is retried.
fn apply(a: &Args) -> Result<i32> {
    let work = Work::open(&a.required("Work")?)?;
    let write = a.flag("Apply");
    let mut ctx = Ctx::load(&work)?;
    let gate = work.path("topic.gate");
    let mut zt: Option<Zt> = None;
    let mut totals = json!({ "applied": 0, "created": 0, "topics": 0, "same": 0, "refused": 0, "duplicates": 0, "edited": 0, "unanswered": 0, "stopped": 0 });
    let bump = |t: &mut Value, k: &str, n: usize| t[k] = json!(t[k].as_u64().unwrap_or(0) + n as u64);
    let jobs = ctx.plan["jobs"].as_array().cloned().unwrap_or_default();
    for job in &jobs {
        let id = s(&job["id"]);
        let section = s(&job["section"]);
        if ctx.state["sections"][&section]["applied"] == true {
            continue;
        }
        let key = ctx.book_of(&section);
        let answer = match answer_of(&s(&job["answer"])) {
            Ok(v) => v,
            Err(_) => {
                bump(&mut totals, "unanswered", 1);
                continue;
            }
        };
        let faults = validate(&ctx, &section, &answer);
        let ideas: Vec<Value> = answer["ideas"].as_array().cloned().unwrap_or_default();
        let refs: Vec<&Value> = ideas.iter().collect();
        let checked = zt_check_drafts(&work, &id, drafts_of(&key, &refs))?;
        let mut bad: HashMap<String, Vec<String>> = HashMap::new();
        for (fid, code, message) in &faults {
            bad.entry(fid.clone()).or_default().push(format!("{code} {message}"));
        }
        for line in &checked {
            if let Some(c) = FAULT_LINE.captures(line) {
                let fid = c[1].trim_start_matches("idea:").to_string();
                bad.entry(fid).or_default().push(format!("{} {}", &c[2], &c[3]));
            }
        }
        if bad.contains_key("answer") {
            // The whole answer is unusable: nothing of it is written.
            ctx.state["sections"][&section] = json!({ "job": id, "applied": false, "refused_answer": bad["answer"] });
            bump(&mut totals, "refused", ideas.len().max(1));
            continue;
        }
        if !write {
            continue;
        }
        if zt.is_none() {
            zt = Some(crate::zt::start()?);
        }
        let zt = zt.as_mut().unwrap();
        let mut record = json!({ "job": id, "applied": true, "created": [], "topics": [], "same": [], "refused": [], "duplicates": [], "kinds": {} });
        // Topics: made only for the ideas that use them, through the topic gate.
        let mut topic_root: HashMap<String, String> = HashMap::new();
        for t in answer["topics"].as_array().into_iter().flatten() {
            let tid = s(&t["id"]);
            let used = ideas.iter().any(|i| s(&i["at"]) == tid && !bad.contains_key(&s(&i["id"])));
            if !used || bad.contains_key(&tid) {
                continue;
            }
            match write_one(zt, json!({ "tool": "create_topic", "arguments": { "title": s(&t["title"]), "body": s(&t["body"]) } }), &gate)? {
                Written::Made(root) => {
                    record["topics"].as_array_mut().unwrap().push(json!(root));
                    topic_root.insert(tid, root);
                }
                Written::Refused { error, refusal } => {
                    if let Some(archive) = refusal.as_ref().filter(|r| r["error"] == "card_locked") {
                        add_partial(&mut totals, &record);
                        record["applied"] = json!(false);
                        record["stopped"] = stopped_fact(&tid, archive);
                        ctx.state["sections"][&section] = record;
                        work.write("state.json", &ctx.state)?;
                        bump(&mut totals, "stopped", 1);
                        return stop_stage(&totals, &archive_refusal("apply", &tid, archive));
                    }
                    match TOPIC_EXISTS.captures(&error) {
                        Some(c) => {
                            let root = c[1].to_string();
                            // The gate reuses an existing Topic: the plan's facts may be older than a
                            // Lock, so read the fresh fact before filing the ideas under it.
                            if let Some(lock) = lockcheck::lock_of(zt, &root)? {
                                let message = format!("the Topic `{root}` is locked by the owner (Lock {}): it takes no new Cards", lock.lock_id);
                                add_partial(&mut totals, &record);
                                record["applied"] = json!(false);
                                record["stopped"] = json!({ "code": "card_locked", "at": root, "tree": lock.tree, "lock_id": lock.lock_id, "message": message });
                                ctx.state["sections"][&section] = record;
                                work.write("state.json", &ctx.state)?;
                                bump(&mut totals, "stopped", 1);
                                let refusal = locked_refusal("apply", &root, &lock.tree, lock.lock_id, &message, None);
                                return stop_stage(&totals, &refusal);
                            }
                            topic_root.insert(tid, root);
                        }
                        None => {
                            bad.entry(tid.clone()).or_default().push(format!("G8 create_topic failed: {error}"));
                        }
                    }
                }
            }
        }
        let mut made: HashMap<String, String> = HashMap::new();
        let mut parent_of: HashMap<String, (String, String)> = HashMap::new();
        for idea in &ideas {
            let iid = s(&idea["id"]);
            if let Some(why) = bad.get(&iid) {
                record["refused"].as_array_mut().unwrap().push(json!({ "id": iid, "faults": why }));
                continue;
            }
            let sources: Vec<String> = idea["sources"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
            let resolve = |body: &str, made: &HashMap<String, String>, roots: &HashMap<String, String>| -> String {
                LINK.replace_all(body, |c: &regex::Captures| {
                    let target = c[1].trim();
                    match target.strip_prefix('@') {
                        Some(r) => match made.get(r).or_else(|| roots.get(r)) {
                            Some(addr) => format!("[[{addr}]]"),
                            None => String::new(),
                        },
                        None => c[0].to_string(),
                    }
                })
                .into_owned()
            };
            let body = with_source(&key, &resolve(&s(&idea["body"]), &made, &topic_root), &sources);
            let same = s(&idea["same"]);
            if !same.is_empty() {
                if ctx.workspace(&same) == "agent" {
                    match write_one(zt, json!({ "tool": "append_card", "arguments": { "at": same, "text": resolve(&s(&idea["body"]), &made, &topic_root) } }), &gate)? {
                        Written::Made(_) => record["same"].as_array_mut().unwrap().push(json!({ "id": iid, "card": same, "appended": true })),
                        Written::Refused { error, refusal } => {
                            if let Some(archive) = refusal.as_ref().filter(|r| r["error"] == "card_locked") {
                                add_partial(&mut totals, &record);
                                record["applied"] = json!(false);
                                record["stopped"] = stopped_fact(&same, archive);
                                ctx.state["sections"][&section] = record;
                                work.write("state.json", &ctx.state)?;
                                bump(&mut totals, "stopped", 1);
                                return stop_stage(&totals, &archive_refusal("apply", &same, archive));
                            }
                            record["refused"].as_array_mut().unwrap().push(json!({ "id": iid, "faults": [format!("append failed: {error}")] }));
                        }
                    }
                } else {
                    record["same"].as_array_mut().unwrap().push(json!({ "id": iid, "card": same, "appended": false }));
                }
                continue;
            }
            let title = s(&idea["title"]);
            // A run-level duplicate: a title near one this run already wrote.
            let dup = ctx.state["cards"].as_object().and_then(|m| m.iter().find(|(_, c)| near(&s(&c["title"]), &title)).map(|(a, _)| a.clone()));
            if let Some(dup) = dup {
                record["duplicates"].as_array_mut().unwrap().push(json!({ "id": iid, "card": dup }));
                made.insert(iid.clone(), dup);
                continue;
            }
            // The parent and the kind.
            let at = s(&idea["at"]);
            let want = if s(&idea["as"]) == "side" { "side" } else { "direct" };
            let mut full: Option<String> = None;
            let (parent, kind) = if let Some(root) = topic_root.get(&at).cloned().or_else(|| is_root(&at).then(|| at.clone())) {
                // A Topic root: the first Card is the doorway n/1; any later root placement opens a new
                // line of the Topic beside the doorway (n/1|a, n/1|b …), and the section's ideas chain in
                // it. A parallel section cannot see where another section's line ends: in pilot 2 the
                // main-line append joined five sections into one blind 20-step chain. A doorway with
                // MAX_LINES lines takes no more: the new line branches off its last line (live run
                // 2026-10-22: 412/1 gathered nine lines), and the report names the full doorway.
                let door = next_direct(&root);
                if !card_exists(zt, &door) {
                    (root, "direct")
                } else {
                    let beside = line_parent(&door, &mut |a| card_exists(zt, a));
                    if beside != door {
                        full = Some(door);
                    }
                    (beside, "side")
                }
            } else if let Some(p) = made.get(&at) {
                (p.clone(), want)
            } else if at.starts_with('I') && !at.contains('/') {
                // The idea it continues was refused: file it beside that idea's own parent.
                match parent_of.get(&at) {
                    Some((p, _)) => (p.clone(), "side"),
                    None => {
                        record["refused"].as_array_mut().unwrap().push(json!({ "id": iid, "faults": [format!("G5 the idea {at} it continues was not written")] }));
                        continue;
                    }
                }
            } else {
                (at.clone(), want)
            };
            let kind = if kind == "direct" && !is_root(&parent) && card_exists(zt, &next_direct(&parent)) { "side" } else { kind };
            match write_one(zt, json!({ "tool": "create_successor", "arguments": { "at": parent, "kind": kind, "type": "idea", "title": title, "body": body } }), &gate)? {
                Written::Made(address) => {
                    parent_of.insert(iid.clone(), (parent.clone(), kind.to_string()));
                    made.insert(iid.clone(), address.clone());
                    let k = s(&idea["kind"]);
                    record["kinds"][&k] = json!(record["kinds"][&k].as_u64().unwrap_or(0) + 1);
                    let mut receipt = json!({ "id": iid, "card": address, "parent": parent, "as": kind, "kind": k });
                    if let Some(door) = &full {
                        receipt["full"] = json!(door);
                    }
                    record["created"].as_array_mut().unwrap().push(receipt);
                    ctx.state["cards"][&address] = json!({ "job": id, "section": section, "book": key, "idea": iid, "title": title, "kind": k });
                }
                Written::Refused { error, refusal } => {
                    if let Some(archive) = refusal.as_ref().filter(|r| r["error"] == "card_locked") {
                        add_partial(&mut totals, &record);
                        record["applied"] = json!(false);
                        record["stopped"] = stopped_fact(&parent, archive);
                        ctx.state["sections"][&section] = record;
                        work.write("state.json", &ctx.state)?;
                        bump(&mut totals, "stopped", 1);
                        return stop_stage(&totals, &archive_refusal("apply", &parent, archive));
                    }
                    record["refused"].as_array_mut().unwrap().push(json!({ "id": iid, "faults": [format!("create failed: {error}")] }));
                }
            }
        }
        add_partial(&mut totals, &record);
        bump(&mut totals, "applied", 1);
        ctx.state["sections"][&section] = record;
        work.write("state.json", &ctx.state)?;
    }
    // The answered fix jobs.
    let fixes: Vec<(String, Value)> = ctx.state["fixes"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    for (fid, fix) in fixes {
        if fix["applied"] == true {
            continue;
        }
        let Ok(answer) = answer_of(&work.path(&format!("out\\{fid}.json"))) else { continue };
        let (faults, drafts) = validate_fix(&ctx, &fid, &answer);
        let checked = zt_check_drafts(&work, &fid, drafts)?;
        if !write {
            continue;
        }
        let bad: HashSet<String> = faults.iter().map(|f| f.0.clone()).chain(checked.iter().filter_map(|l| FAULT_LINE.captures(l).map(|c| c[1].to_string()))).collect();
        if zt.is_none() {
            zt = Some(crate::zt::start()?);
        }
        let zt = zt.as_mut().unwrap();
        let mut edited = Vec::new();
        let mut refused = Vec::new();
        for e in answer["edits"].as_array().into_iter().flatten() {
            let at = s(&e["at"]);
            if bad.contains(&at) || bad.contains("answer") {
                refused.push(json!(at));
                continue;
            }
            let current = calls::read(zt, "get_card", json!({ "at": at }))?;
            let key = ctx.book_of_card(&at);
            let call = json!({ "tool": "edit_card", "arguments": { "at": at, "title": s(&e["title"]), "body": with_source(&key, &s(&e["body"]), &[]), "base": current["text"] } });
            match write_one(zt, call, &gate)? {
                Written::Made(_) => {
                    edited.push(json!(at));
                    ctx.state["cards"][&at]["title"] = json!(s(&e["title"]));
                }
                Written::Refused { error, refusal } => {
                    if let Some(archive) = refusal.as_ref().filter(|r| r["error"] == "card_locked") {
                        bump(&mut totals, "edited", edited.len());
                        let mut stopped = ctx.state["fixes"][&fid].clone();
                        stopped["applied"] = json!(false);
                        stopped["stopped"] = stopped_fact(&at, archive);
                        stopped["edited"] = json!(edited);
                        ctx.state["fixes"][&fid] = stopped;
                        work.write("state.json", &ctx.state)?;
                        bump(&mut totals, "stopped", 1);
                        return stop_stage(&totals, &archive_refusal("apply", &at, archive));
                    }
                    refused.push(json!(format!("{at}: {error}")));
                }
            }
        }
        bump(&mut totals, "edited", edited.len());
        ctx.state["fixes"][&fid]["applied"] = json!(true);
        ctx.state["fixes"][&fid]["edited"] = json!(edited);
        ctx.state["fixes"][&fid]["refused"] = json!(refused);
        work.write("state.json", &ctx.state)?;
    }
    // The answered link jobs: each valid Link sentence appended to its Card.
    let links: Vec<(String, Value)> = ctx.state["links"].as_object().map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
    for (lid, job) in links {
        if job["applied"] == true {
            continue;
        }
        let Ok(answer) = answer_of(&work.path(&format!("out\\{lid}.json"))) else { continue };
        let faults = validate_links(&ctx, &lid, &answer);
        if !write {
            continue;
        }
        let bad: HashSet<String> = faults.iter().map(|f| f.0.clone()).collect();
        if zt.is_none() {
            zt = Some(crate::zt::start()?);
        }
        let zt = zt.as_mut().unwrap();
        let mut added = Vec::new();
        let mut refused = Vec::new();
        for (i, l) in answer["links"].as_array().into_iter().flatten().enumerate() {
            let tag = format!("L{}", i + 1);
            let (from, to) = (s(&l["from"]), s(&l["to"]));
            if bad.contains(&tag) || bad.contains("answer") {
                refused.push(json!(format!("{from} -> {to}")));
                continue;
            }
            let current = calls::read(zt, "get_card", json!({ "at": from }))?;
            if s(&current["body"]).contains(&format!("[[{to}]]")) {
                continue;
            }
            // The source line stays the Card's last line (pilot 4: appended Links landed after it).
            let body = before_source(&s(&current["body"]), &s(&l["sentence"]));
            match write_one(zt, json!({ "tool": "edit_card", "arguments": { "at": from, "body": body, "base": current["text"] } }), &work.path("topic.gate"))? {
                Written::Made(_) => added.push(json!({ "from": from, "to": to })),
                Written::Refused { error, refusal } => {
                    if let Some(archive) = refusal.as_ref().filter(|r| r["error"] == "card_locked") {
                        bump(&mut totals, "linked", added.len());
                        let mut stopped = ctx.state["links"][&lid].clone();
                        stopped["applied"] = json!(false);
                        stopped["stopped"] = stopped_fact(&from, archive);
                        stopped["added"] = json!(added);
                        ctx.state["links"][&lid] = stopped;
                        work.write("state.json", &ctx.state)?;
                        bump(&mut totals, "stopped", 1);
                        return stop_stage(&totals, &archive_refusal("apply", &from, archive));
                    }
                    refused.push(json!(format!("{from} -> {to}: {error}")));
                }
            }
        }
        bump(&mut totals, "linked", added.len());
        ctx.state["links"][&lid]["applied"] = json!(true);
        ctx.state["links"][&lid]["added"] = json!(added);
        ctx.state["links"][&lid]["refused"] = json!(refused);
        work.write("state.json", &ctx.state)?;
    }
    if write {
        work.write("state.json", &ctx.state)?;
    }
    util::print_last(&totals)?;
    Ok(0)
}

// --- the link round -----------------------------------------------------------------------------

/// `body` with `sentence` as a paragraph before its trailing source line (`来源：…`), or at the end
/// when it has none.
fn before_source(body: &str, sentence: &str) -> String {
    let body = body.trim_end();
    let lines: Vec<&str> = body.split('\n').collect();
    match lines.iter().rposition(|l| l.trim_start().starts_with("来源：")) {
        Some(i) if lines[i + 1..].iter().all(|l| l.trim().is_empty()) => {
            let head = lines[..i].join("\n");
            format!("{}\n\n{}\n\n{}", head.trim_end(), sentence.trim(), lines[i..].join("\n").trim_end())
        }
        _ => format!("{body}\n\n{}", sentence.trim()),
    }
}

/// The Links one new Card may get in the link round.
const LINKS_PER_CARD: usize = 2;

/// `-Step links`: one job per Topic tree the run wrote in (gl-NNN): that tree's new Cards, the run's
/// new Cards of the other trees, and the box's Topic Cards closest by wording. DeepSeek proposes
/// Links to Cards of other trees, each as one sentence with its relation (Luhmann: the valuable
/// Link joins distant trees; parallel section jobs could not see each other's Cards).
fn links(a: &Args) -> Result<()> {
    let work = Work::open(&a.required("Work")?)?;
    let mut state = work.state()?;
    if state["links"].is_object() {
        let n = state["links"].as_object().map_or(0, |m| m.values().filter(|v| v["applied"] != true).count());
        return util::print_last(&json!({ "link_jobs": n, "kept": true }));
    }
    let titles = work.read("titles.json")?.as_array().cloned().unwrap_or_default();
    let plan = work.read("plan.json")?;
    let mut by_tree: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for at in state["cards"].as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default() {
        by_tree.entry(tree_of(&at).to_string()).or_default().push(at);
    }
    let all: Vec<String> = by_tree.values().flatten().cloned().collect();
    let mut zt = crate::zt::start()?;
    let read = zt.cards(&all, "text")?;
    let card_of = |at: &str| read.cards.iter().find(|c| s(&c["address"]) == at).cloned().unwrap_or(Value::Null);
    let mut jobs: Map<String, Value> = Map::new();
    let mut n = 0;
    for (tree, cards) in &by_tree {
        n += 1;
        let id = job_id("gl", n);
        let answer = work.path(&format!("out\\{id}.json"));
        let _ = std::fs::remove_file(&answer);
        let mut md = format!("# 任务 {id}：主题树 {tree} 本次新写的卡的跨树链接\n\n先读任务说明中的\"跨树链接\"一节。\n\n- 答案文件：`{answer}`\n- 每张卡最多 {LINKS_PER_CARD} 个新 Link\n\nSELF-CHECK: zt-digest grow -Step precheck -Work \"{}\" -Job {id}\n\n## 本主题本次新写的卡（from）\n\n", work.dir);
        let mut text_all = String::new();
        for at in cards {
            let c = card_of(at);
            md.push_str(&format!("### [[{at}]] {}\n\n{}\n\n", s(&c["title"]), cut(&s(&c["body"]), 260)));
            text_all.push_str(&s(&c["title"]));
        }
        let grams = bigrams(&text_all);
        // Only Cards that share wording with this tree's new Cards are offered: in pilot 4 a Topic on
        // public credit, offered only Cards on outsourcing, wrote forced analogies.
        let shares_words = |title: &str| {
            let g = bigrams(title);
            g.len() >= 3 && g.iter().filter(|x| grams.contains(x)).count() >= 2
        };
        let mut run_candidates = Vec::new();
        for (other, list) in &by_tree {
            if other == tree {
                continue;
            }
            for at in list {
                let title = s(&card_of(at)["title"]);
                if shares_words(&title) {
                    run_candidates.push(format!("- [[{at}]] {title}\n"));
                }
            }
        }
        if !run_candidates.is_empty() {
            md.push_str("## 其他主题本次新写的卡（to 的候选）\n\n");
            md.push_str(&run_candidates.concat());
        }
        let mut scored: Vec<(f64, &Value)> = titles
            .iter()
            .filter(|t| {
                let at = s(&t["address"]);
                in_topic_tree(&at) && !is_root(&at) && tree_of(&at) != tree && !all.contains(&at)
            })
            .filter_map(|t| {
                let g = bigrams(&s(&t["title"]));
                let shared = g.iter().filter(|x| grams.contains(x)).count();
                (g.len() >= 3 && shared >= 2).then(|| (shared as f64 / g.len() as f64, t))
            })
            .collect();
        scored.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap_or(std::cmp::Ordering::Equal));
        if run_candidates.is_empty() && scored.is_empty() {
            // Nothing in the box shares this tree's wording: no job (an empty answer would be the
            // honest one, and a forced analogy the likely one).
            n -= 1;
            continue;
        }
        if !scored.is_empty() {
            md.push_str("\n## 盒子里用词接近的其他主题卡（to 的候选，也可以自己搜索）\n\n");
            for (_, t) in scored.iter().take(20) {
                md.push_str(&format!("- [[{}]] {}（{}）\n", s(&t["address"]), s(&t["title"]), if s(&t["workspace"]) == "agent" { "代理" } else { "人类" }));
            }
        }
        util::write_text(work.path(&format!("jobs\\{id}.md")), &retool(&plan, &md))?;
        jobs.insert(id, json!({ "tree": tree, "cards": cards, "applied": false }));
    }
    state["links"] = Value::Object(jobs);
    work.write("state.json", &state)?;
    util::print_last(&json!({ "link_jobs": n }))
}

/// The faults of a link answer (G11): each Link starts at a new Card of the job, ends at a Card of
/// another tree (not a root), and is one sentence that carries its relation.
fn validate_links(ctx: &Ctx, id: &str, answer: &Value) -> Vec<Fault> {
    let mut faults: Vec<Fault> = Vec::new();
    let mut add = |tag: &str, message: String| faults.push((tag.to_string(), "G11".to_string(), message));
    let Some(list) = answer["links"].as_array() else {
        add("answer", "the answer needs \"links\": [...] (an empty list when no real relation exists)".into());
        return faults;
    };
    let cards: Vec<String> = ctx.state["links"][id]["cards"].as_array().map(|l| l.iter().map(s).collect()).unwrap_or_default();
    let mut per: HashMap<String, usize> = HashMap::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();
    for (i, l) in list.iter().enumerate() {
        let tag = format!("L{}", i + 1);
        let (from, to, sentence) = (s(&l["from"]), s(&l["to"]), s(&l["sentence"]));
        if !cards.contains(&from) {
            add(&tag, format!("`from` `{from}` is not a new Card of this job"));
        }
        if !ctx.exists(&to) || is_root(&to) {
            add(&tag, format!("`to` `{to}` is not a Card of the box (a Topic root is no target: link its most specific Card)"));
        } else if tree_of(&to) == tree_of(&from) {
            add(&tag, format!("`{to}` is in the same tree as `{from}`: this round links other trees"));
        }
        let kinds = link_kinds(&sentence);
        if kinds.len() != 1 || kinds[0].0 != to {
            add(&tag, format!("the sentence holds exactly one Link, `[[{to}]]`"));
        } else if kinds[0].2 == "refers" {
            add(&tag, format!("`[[{to}]]` reads as a plain reference: put the relation's kind word before it ({KIND_WORDS})"));
        }
        if !rests_on_book(&sentence, &ctx.book_of_card(&from)).is_empty() {
            add(&tag, "a Topic Card does not rest on its own book".into());
        }
        if sentence.contains('；') || sentence.chars().count() > 120 || sentence.trim().is_empty() {
            add(&tag, "one or two short sentences (at most 120 characters, no `；`)".into());
        }
        if !seen.insert((from.clone(), to.clone())) {
            add(&tag, "the same Link twice".into());
        }
        let n = per.entry(from.clone()).or_default();
        *n += 1;
        if *n > LINKS_PER_CARD {
            add(&tag, format!("more than {LINKS_PER_CARD} new Links from `{from}`: keep the strongest"));
        }
    }
    faults
}

// --- check --------------------------------------------------------------------------------------

/// zt-check on the run's Cards, one run per book (`--book` names each Card's own source).
fn faults_by_book(work: &Work, zt: &mut Zt, ctx: &Ctx, cards: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut by_book: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for at in cards {
        by_book.entry(ctx.book_of_card(at)).or_default().push(at.clone());
    }
    let mut out = BTreeMap::new();
    for (key, list) in by_book {
        out.extend(run_faults(work, zt, &key, &list)?);
    }
    Ok(out)
}

/// zt-check's hard faults on the run's Cards, by address.
fn run_faults(work: &Work, zt: &mut Zt, key: &str, cards: &[String]) -> Result<BTreeMap<String, Vec<String>>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    if cards.is_empty() {
        return Ok(out);
    }
    let file = work.path("run-cards.txt");
    util::write_text(&file, &cards.join("\n"))?;
    // --cards with the book: the Card layer rules (I1–I7, L1, L2) as well as the prose rules.
    let lines = Check { log: file, book: key.to_string(), ..Check::default() }.run(zt)?;
    for line in lines.iter().filter(|l| !FAULTS.is_match(l) && !l.contains("(advisory)")) {
        if let Some(c) = FAULT_LINE.captures(line)
            && cards.contains(&c[1].to_string())
        {
            out.entry(c[1].to_string()).or_default().push(format!("{} {}", &c[2], &c[3]));
        }
    }
    Ok(out)
}

const FIX_ORDER: &str = "按这个顺序修改：先把缺的条件或假设写进正文；需要本书的推导时，用真实的关系链接文献卡（`例示`、`对应`、`对照`），不要用 `依赖`；不要把这张卡改成只依附本书的笔记，也不要删除它。只改下面列出的问题，保留其余内容和全部 Link。";

fn check(a: &Args) -> Result<()> {
    let work = Work::open(&a.required("Work")?)?;
    let round = a.int_or("Round", 1)?;
    let mut state = work.state()?;
    let cards: Vec<String> = state["cards"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let mut zt = crate::zt::start()?;
    let ctx = Ctx::load(&work)?;
    let faults = faults_by_book(&work, &mut zt, &ctx, &cards)?;
    let mut by_section: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for at in faults.keys() {
        by_section.entry(s(&state["cards"][at]["section"])).or_default().push(at.clone());
    }
    let mut n = 0;
    for (section, list) in &by_section {
        n += 1;
        let id = format!("fx{round}-{n:03}");
        let read = zt.cards(list, "text")?;
        let mut md = format!("# 修正任务 {id}：[[{section}]] 一节新写的主题卡\n\n{FIX_ORDER}\n\n答案文件：`{}`，格式：`{{\"edits\":[{{\"at\":\"地址\",\"title\":\"标题\",\"body\":\"正文\"}}]}}`（只写要改的卡；body 是完整的新正文，不含 `# 标题`）。\n\nSELF-CHECK: zt-digest grow -Step precheck -Work \"{}\" -Job {id}\n\n", work.path(&format!("out\\{id}.json")), work.dir);
        for card in &read.cards {
            let at = s(&card["address"]);
            md.push_str(&format!("## [[{at}]] {}\n\n问题：\n{}\n\n正文：\n\n{}\n\n", s(&card["title"]), faults[&at].iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n"), s(&card["body"])));
        }
        util::write_text(work.path(&format!("jobs\\{id}.md")), &retool(&ctx.plan, &md))?;
        let _ = std::fs::remove_file(work.path(&format!("out\\{id}.json")));
        state["fixes"][&id] = json!({ "section": section, "cards": list, "round": round, "applied": false });
    }
    state["checked"] = json!({ "round": round, "faults": faults.values().map(Vec::len).sum::<usize>(), "cards": faults.len() });
    work.write("state.json", &state)?;
    util::print_last(&json!({ "fix": n, "round": round, "faults": faults.values().map(Vec::len).sum::<usize>(), "cards": faults.len() }))
}

// --- report -------------------------------------------------------------------------------------

fn report(a: &Args) -> Result<()> {
    let work = Work::open(&a.required("Work")?)?;
    let plan = work.read("plan.json")?;
    let state = work.state()?;
    let baseline = work.read("baseline.json").unwrap_or(Value::Null);
    let key = s(&plan["key"]);
    let mut zt = crate::zt::start()?;
    let mut books: Vec<(String, Vec<Value>)> = Vec::new();
    for b in plan["books"].as_array().into_iter().flatten() {
        let k = s(&b["key"]);
        let (_, cards) = read_book(&mut zt, &k)?;
        books.push((k, cards));
    }
    let after = box_share(&books);
    let run_cards: Vec<String> = state["cards"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default();
    let ctx = Ctx::load(&work)?;
    let left = faults_by_book(&work, &mut zt, &ctx, &run_cards)?;
    // Bridges: the run's Cards that cite two or more Literature trees (an idea read across sources).
    let read = if run_cards.is_empty() { Vec::new() } else { zt.cards(&run_cards, "text")?.cards };
    let bridges: Vec<String> = read
        .iter()
        .filter(|c| {
            let trees: HashSet<String> = LINK.captures_iter(&s(&c["body"])).map(|m| tree_of(m[1].trim()).to_lowercase()).filter(|t| !t.chars().all(|ch| ch.is_ascii_digit())).collect();
            trees.len() >= 2
        })
        .map(|c| s(&c["address"]))
        .collect();
    let mut kinds: BTreeMap<String, u64> = BTreeMap::new();
    let mut rows = Vec::new();
    let mut totals: Map<String, Value> = Map::new();
    for section in plan["sections"].as_array().into_iter().flatten() {
        let at = s(&section["anchor"]);
        let r = &state["sections"][&at];
        let count = |f: &str| r[f].as_array().map_or(0, Vec::len);
        for (k, v) in r["kinds"].as_object().into_iter().flatten() {
            *kinds.entry(k.clone()).or_default() += v.as_u64().unwrap_or(0);
        }
        for f in ["created", "topics", "same", "refused", "duplicates"] {
            let t = totals.entry(f.to_string()).or_insert(json!(0));
            *t = json!(t.as_u64().unwrap_or(0) + count(f) as u64);
        }
        rows.push(json!({ "section": at, "applied": r["applied"] == true, "created": count("created"), "topics": count("topics"), "same": count("same"), "refused": count("refused"), "duplicates": count("duplicates") }));
    }
    let linked: usize = state["links"].as_object().map_or(0, |m| m.values().map(|v| v["added"].as_array().map_or(0, Vec::len)).sum());
    totals.insert("linked".into(), json!(linked));
    // The jobs a Locked destination stopped (S15): each names the tree, the Lock and the address it
    // tried, so the owner reads which destination is frozen and what to unlock.
    let mut stopped: Vec<Value> = Vec::new();
    let mut take_stopped = |record: &Value| {
        if !record["stopped"].is_null() {
            stopped.push(record["stopped"].clone());
        }
    };
    for section in plan["sections"].as_array().into_iter().flatten() {
        take_stopped(&state["sections"][&s(&section["anchor"])]);
    }
    for field in ["fixes", "links"] {
        for record in state[field].as_object().into_iter().flatten().map(|(_, v)| v) {
            take_stopped(record);
        }
    }
    totals.insert("stopped".into(), json!(stopped.len()));
    // The root placements a full doorway sent one level down (MAX_LINES): each names the doorway, so
    // the reader sees where a structure Card or a new Topic would help.
    let deeper: Vec<Value> = plan["sections"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|x| state["sections"][&s(&x["anchor"])]["created"].as_array().cloned().unwrap_or_default())
        .filter(|c| !c["full"].is_null())
        .map(|c| json!({ "card": c["card"], "parent": c["parent"], "door": c["full"] }))
        .collect();
    totals.insert("deeper".into(), json!(deeper.len()));
    let mut topics_made: Vec<Value> = state["run_topics"].as_array().into_iter().flatten().filter(|t| t["new"] == true).map(|t| t["root"].clone()).collect();
    topics_made.extend(plan["sections"].as_array().into_iter().flatten().flat_map(|x| state["sections"][&s(&x["anchor"])]["topics"].as_array().cloned().unwrap_or_default()));
    let run_topics: Vec<Value> = state["run_topics"].as_array().cloned().unwrap_or_default();
    totals.insert("topics".into(), json!(topics_made.len()));
    let report = json!({
        "key": key, "title": plan["title"], "sections": rows, "totals": totals, "kinds": kinds,
        "topics_made": topics_made, "run_topics": run_topics, "faults_left": left, "bridges": bridges, "stopped": stopped, "deeper": deeper,
        "share": { "before": baseline, "after": after },
    });
    work.write("grow-report.json", &report)?;
    let pct = |v: &Value| v.as_f64().map(|x| format!("{:.0}%", x * 100.0)).unwrap_or_else(|| "—".into());
    let mut md = if books.len() == 1 { format!("# 主题树生长报告：{}（[[{key}]]）\n\n", s(&plan["title"])) } else { format!("# 主题树生长报告：{}\n\n", s(&plan["title"])) };
    md.push_str(&format!(
        "- 主题份额（链接这些文献的主题卡，两个工作区 / 这些文献树的观点卡）：之前 {}（{} / {}），之后 {}（{} / {}）。\n",
        pct(&baseline["share"]), s(&baseline["topic"]), s(&baseline["literature"]), pct(&after["share"]), s(&after["topic"]), s(&after["literature"])
    ));
    md.push_str(&format!("- 跨文献的主题卡（引用两部或更多文献）：{} 张。\n", bridges.len()));
    if books.len() > 1 {
        md.push_str("\n| 文献 | 之前 | 之后 |\n| --- | --- | --- |\n");
        for (k, _) in &books {
            md.push_str(&format!("| [[{k}]] | {} | {} |\n", pct(&baseline["books"][k]["share"]), pct(&after["books"][k]["share"])));
        }
        md.push('\n');
    }
    md.push_str(&format!(
        "- 新写主题卡 {}，新建主题 {}，跨树链接 {}，并入已有卡（same）{}，拒绝 {}，重复 {}；剩余问题 {} 张卡。\n",
        s(&report["totals"]["created"]), s(&report["totals"]["topics"]), s(&report["totals"]["linked"]), s(&report["totals"]["same"]), s(&report["totals"]["refused"]), s(&report["totals"]["duplicates"]), left.len()
    ));
    md.push_str(&format!("- 按种类：{}。\n\n", kinds.iter().map(|(k, v)| format!("{k} {v}")).collect::<Vec<_>>().join("，")));
    md.push_str("| 小节 | 新卡 | 新主题 | same | 拒绝 | 重复 |\n| --- | --- | --- | --- | --- | --- |\n");
    for r in report["sections"].as_array().into_iter().flatten() {
        md.push_str(&format!("| [[{}]] | {} | {} | {} | {} | {} |\n", s(&r["section"]), s(&r["created"]), s(&r["topics"]), s(&r["same"]), s(&r["refused"]), s(&r["duplicates"])));
    }
    if !left.is_empty() {
        md.push_str("\n## 剩余问题\n\n");
        for (at, f) in &left {
            md.push_str(&format!("- [[{at}]]：{}\n", f.join("；")));
        }
    }
    if !deeper.is_empty() {
        md.push_str(&format!("\n## 入口卡已满（已有 {MAX_LINES} 条思路：新思路放在最后一条思路的旁支上）\n\n"));
        for d in &deeper {
            md.push_str(&format!("- [[{}]]：入口卡 [[{}]] 已满，放在 [[{}]] 的旁支上\n", s(&d["card"]), s(&d["door"]), s(&d["parent"])));
        }
    }
    if !stopped.is_empty() {
        md.push_str("\n## 已锁定而停下的写入（解锁后重跑同一个 out 继续）\n\n");
        for stop in &stopped {
            md.push_str(&format!("- [[{}]]：{}（Lock {}，树 `{}`）\n", s(&stop["at"]), s(&stop["message"]), s(&stop["lock_id"]), s(&stop["tree"])));
        }
    }
    util::write_text(work.path("grow-report.md"), &md)?;
    util::print_last(&json!({
        "created": report["totals"]["created"], "topics": report["totals"]["topics"], "linked": report["totals"]["linked"], "same": report["totals"]["same"],
        "refused": report["totals"]["refused"], "duplicates": report["totals"]["duplicates"], "stopped": report["totals"]["stopped"], "deeper": report["totals"]["deeper"], "faults_left": left.len(),
        "share_before": baseline["share"], "share_after": after["share"], "books": books.len(), "bridges": bridges.len(), "report": work.path("grow-report.md"),
    }))
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Step", "Book", "Books", "Work", "Instructions", "From", "To", "Job", "Round", "Tool", "ZtConfig"], &["Apply"])?;
    let code = match a.choice("Step", &["plan", "topics", "links", "precheck", "apply", "check", "report"])?.as_str() {
        "plan" => {
            plan(&a)?;
            0
        }
        "topics" => topics(&a)?,
        "links" => {
            links(&a)?;
            0
        }
        "precheck" => {
            precheck(&a)?;
            0
        }
        "apply" => apply(&a)?,
        "check" => {
            check(&a)?;
            0
        }
        _ => {
            report(&a)?;
            0
        }
    };
    Ok(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_read_as_zt_numbers_them() {
        assert_eq!(next_direct("12/3"), "12/4");
        assert_eq!(next_direct("12/3|a"), "12/3|a|1");
        assert_eq!(next_direct("12/3|a|2"), "12/3|a|3");
        assert_eq!(next_direct("12/0"), "12/1");
        assert!(in_topic_tree("12/3|a") && !in_topic_tree("Key/3") && !in_topic_tree("Key"));
        assert!(is_root("12/0") && !is_root("12/0|a") && !is_root("12/1"));
    }

    #[test]
    fn a_source_is_cited_with_a_real_relation() {
        let src = vec!["K/3|a".to_string()];
        assert!(cites_with_kind("一般地说，事后议价压低投资。例示 [[K/3|a]]：本书……", &src, &CITE_KINDS));
        assert!(!cites_with_kind("见 [[K/3|a]]。", &src, &CITE_KINDS));
        assert!(!cites_with_kind("依赖 [[K/3|a]] 的设定。", &src, &CITE_KINDS));
        assert_eq!(rests_on_book("依赖 [[K/3|a]] 的设定。对照 [[K/4]]。", "K"), vec!["K/3|a".to_string()]);
        assert!(rests_on_book("依赖 [[12/3]] 的结论。", "K").is_empty());
    }

    #[test]
    fn reverse_links_name_the_cards_that_link_a_note() {
        let text = "# t\n\nbody\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[0/1|a]] 信用\n- [[K/2]] 锚点\n";
        let links = reverse_links(text);
        assert_eq!(links, vec![("0/1|a".to_string(), "信用".to_string()), ("K/2".to_string(), "锚点".to_string())]);
        let cards = vec![
            json!({ "address": "K", "type": "idea", "text": "" }),
            json!({ "address": "K/1", "type": "idea", "text": "" }),
            json!({ "address": "K/1|a", "type": "idea", "text": text }),
            json!({ "address": "K/1|b", "type": "model", "text": "" }),
        ];
        let share = share_of(&cards, "K");
        assert_eq!(share["topic"], json!(1));
        assert_eq!(share["literature"], json!(1));
        assert_eq!(share["share"], json!(0.5));
    }

    // The Link kinds read as zt reads them (src/model.rs read_links).
    #[test]
    fn links_read_their_kind_as_zt_does() {
        let kinds = |b: &str| link_kinds(b).into_iter().map(|(t, _, k)| format!("{t}={k}")).collect::<Vec<_>>();
        assert_eq!(kinds("本卡承接 [[380/1]] 的判断。"), vec!["380/1=rests-on"]);
        assert_eq!(kinds("本卡引出 [[381/1]] 的后果。由 [[380/1]] 可得：……"), vec!["381/1=refers", "380/1=refers"]);
        assert_eq!(kinds("对照 [[a/1]] 与 [[a/2]]：两者不同。"), vec!["a/1=contrasts", "a/2=contrasts"]);
        assert_eq!(kinds("例示 [[K/1]]：……依赖 [[12/3]] 的条件。"), vec!["K/1=illustrates", "12/3=rests-on"]);
        assert_eq!(plain_links("见 [[12/3]]。

来源：[[K]]"), vec!["12/3".to_string()]);
        assert!(plain_links("放宽 [[12/3]] 的假设。

来源：[[K]] §2").is_empty());
    }

    #[test]
    fn the_run_topics_are_few_and_new() {
        let mut c = ctx();
        c.plan["pending"] = json!(["K/3"]);
        let ok = json!({ "topics": [{ "id": "T1", "existing": "12/0", "sections": [3] }, { "id": "T2", "title": "外包合同的定价", "body": "本主题追问：外包价格怎样形成。", "sections": [3] }] });
        assert!(validate_topics(&c, &ok).is_empty(), "{:?}", validate_topics(&c, &ok));
        let bad = json!({ "topics": [
            { "id": "T1", "existing": "0/0", "sections": [3] },
            { "id": "T2", "title": "敲竹杠。", "body": "问题。", "sections": [9] },
            { "id": "T3", "title": "外包合同的定价", "body": "问题。", "sections": [3] },
            { "id": "T4", "title": "外包合同的定价！", "body": "问题。", "sections": [3] },
        ] });
        let got = codes(&validate_topics(&c, &bad));
        for want in ["T1 G5", "T2 G8", "T2 G2", "T4 G8", "answer G1"] {
            assert!(got.contains(&want.to_string()), "{want} missing in {got:?}");
        }
        assert_eq!(max_new_topics(1), 2);
        assert_eq!(max_new_topics(8), 4);
        assert_eq!(max_new_topics(67), 8);
    }

    #[test]
    fn the_job_commands_name_the_tool_and_the_box() {
        let text = "SELF-CHECK: zt-digest grow -Step precheck -Job gw-001\n{{CALLENV}}@'\n[]\n'@ | {{TOOL}} call - -Allow x";
        assert_eq!(retool(&json!({}), text), "SELF-CHECK: zt-digest grow -Step precheck -Job gw-001\n@'\n[]\n'@ | zt-digest call - -Allow x");
        let copy = retool(&json!({ "tool": "D:\\t\\zt-digest.exe", "zt_config": "D:\\box\\config" }), text);
        assert!(copy.starts_with("SELF-CHECK: & 'D:\\t\\zt-digest.exe' grow -Step precheck"));
        assert!(copy.contains("$env:ZT_CONFIG_DIR='D:\\box\\config'\n@'") && copy.ends_with("'@ | & 'D:\\t\\zt-digest.exe' call - -Allow x"));
    }

    #[test]
    fn a_link_sentence_goes_before_the_source_line() {
        assert_eq!(before_source("主张。\n\n来源：[[K]]\n", "对照 [[12/3]]：x。"), "主张。\n\n对照 [[12/3]]：x。\n\n来源：[[K]]");
        assert_eq!(before_source("主张。", "对照 [[12/3]]：x。"), "主张。\n\n对照 [[12/3]]：x。");
        assert_eq!(before_source("来源：[[K]]\n\n后记。", "对照 [[a/1]]。"), "来源：[[K]]\n\n后记。\n\n对照 [[a/1]]。");
    }

    #[test]
    fn the_first_sentence_ends_at_the_full_stop() {
        assert_eq!(first_sentence("本卡放宽 [[12/3]] 的假设。其余。"), "本卡放宽 [[12/3]] 的假设。");
        assert_eq!(first_sentence("没有句号"), "没有句号");
    }

    fn ctx() -> Ctx {
        let plan = json!({ "key": "K", "sections": [{ "n": 3, "anchor": "K/3", "cards": ["K/3", "K/3|a", "K/3|b"] }], "jobs": [{ "id": "gw-001", "section": "K/3" }] });
        let titles = vec![
            json!({ "address": "K/3|a", "title": "专用投资完成后供应商被敲竹杠", "workspace": "agent" }),
            json!({ "address": "K/3|b", "title": "外包价格由纳什议价决定", "workspace": "agent" }),
            json!({ "address": "12/0", "title": "敲竹杠", "workspace": "agent" }),
            json!({ "address": "12/1", "title": "事后议价让投资方得不到全部回报", "workspace": "agent" }),
            json!({ "address": "0/1", "title": "信用是银行的起点", "workspace": "human" }),
        ];
        Ctx { key: "K".into(), plan, titles, state: json!({ "cards": {} }) }
    }

    fn codes(faults: &[Fault]) -> Vec<String> {
        faults.iter().map(|f| format!("{} {}", f.0, f.1)).collect()
    }

    #[test]
    fn a_locked_topic_is_read_only_context_and_never_a_destination() {
        let mut c = ctx();
        // `12` is one whole Topic tree the owner has locked: 12/0 is its root, 12/1 a Card in it.
        c.titles[2]["lock"] = json!({ "lock_id": 7, "tree": "12", "since": "2026-10-21T10:00:00.000Z" });
        c.titles[3]["lock"] = json!({ "lock_id": 7, "tree": "12", "since": "2026-10-21T10:00:00.000Z" });
        let idea = |at: &str, same: &str| {
            json!({ "section": "K/3", "ideas": [{ "id": "I1", "kind": "关联", "title": "沉没投资削弱议价地位",
                "body": "本卡推广 [[12/1]]：外包的供应商也先投资。例示 [[K/3|a]]：本书。", "sources": ["K/3|a"],
                "at": at, "same": same }] })
        };
        // The Card is Locked: a successor would write inside the Locked tree.
        let got = codes(&validate(&c, "K/3", &idea("12/1", "")));
        assert!(got.contains(&"I1 G12".to_string()), "{got:?}");
        // Its root is Locked too.
        let got = codes(&validate(&c, "K/3", &idea("12/0", "")));
        assert!(got.contains(&"I1 G12".to_string()), "{got:?}");
        // A `same` sentence would append to a Locked Card.
        let got = codes(&validate(&c, "K/3", &idea("", "12/1")));
        assert!(got.contains(&"I1 G12".to_string()), "{got:?}");
        // An unlocked Card in the same box stays a destination.
        let got = codes(&validate(&c, "K/3", &idea("0/1", "")));
        assert!(!got.contains(&"I1 G12".to_string()), "{got:?}");
    }

    #[test]
    fn a_locked_topic_refuses_a_new_topic_that_would_be_filed_in_it() {
        let mut c = ctx();
        c.titles[2]["lock"] = json!({ "lock_id": 7, "tree": "12", "since": "2026-10-21T10:00:00.000Z" });
        // `12/0` is titled 敲竹杠: a new Topic of that name is G12, never G8's "file under it".
        let answer = json!({ "section": "K/3", "topics": [{ "id": "T1", "title": "敲竹杠", "body": "本主题追问。" }], "ideas": [
            { "id": "I1", "kind": "核心", "title": "专用投资以后供应商被敲竹杠", "body": "例示 [[K/3|a]]：本书。", "sources": ["K/3|a"], "at": "T1" }
        ] });
        let got = codes(&validate(&c, "K/3", &answer));
        assert!(got.contains(&"T1 G12".to_string()), "{got:?}");
        assert!(!got.contains(&"T1 G8".to_string()), "{got:?}");
        // The Topic plan: `existing` may name an unlocked agent Topic root only.
        c.plan["pending"] = json!(["K/3"]);
        let topics = json!({ "topics": [{ "id": "T1", "existing": "12/0", "sections": [3] }] });
        let got = codes(&validate_topics(&c, &topics));
        assert!(got.contains(&"T1 G12".to_string()), "{got:?}");
    }

    #[test]
    fn the_archive_refusal_becomes_the_grow_typed_refusal() {
        let archive = json!({
            "error": "card_locked",
            "message": "card `1/1` is inside locked tree `1`; a locked tree takes no new Cards",
            "tree": "1", "cards": ["1/1"], "reason": "create-inside", "hint": "unlock the tree in the GUI first",
            "locks": [{ "lock_id": 3, "tree": "1" }]
        });
        assert_eq!(archive_lock(&archive), Some(("1".to_string(), 3)));
        assert_eq!(archive_lock(&json!({ "error": "card_locked" })), None);
        let refusal = locked_refusal("apply", "1/1", "1", 3, "the destination is locked", Some(&archive));
        assert_eq!(refusal["ok"], json!(false));
        assert_eq!(refusal["code"], json!("card_locked"));
        assert_eq!(refusal["mode"], json!("grow"));
        assert_eq!(refusal["step"], json!("apply"));
        assert_eq!(refusal["at"], json!("1/1"));
        assert_eq!(refusal["tree"], json!("1"));
        assert_eq!(refusal["lock_id"], json!(3));
        assert_eq!(refusal["reason"], json!("create-inside"));
        assert_eq!(refusal["cards"], json!(["1/1"]));
        // A Lock read fresh (not a refused write) answers the same typed shape without archive facts.
        let plain = locked_refusal("topics", "0/0", "0", 9, "the destination is locked", None);
        assert_eq!(plain["code"], json!("card_locked"));
        assert_eq!(plain["lock_id"], json!(9));
        assert!(plain.get("reason").is_none(), "{plain}");
        // The hint is an owner action: no route, no token, no Access bypass.
        let hint = plain["hint"].as_str().unwrap();
        assert!(hint.contains("unlock the tree in the GUI"), "{hint}");
        assert!(!hint.contains("/api/") && !hint.contains("token") && !hint.contains("Access"), "{hint}");
    }

    #[test]
    fn the_section_lists_locked_cards_as_context_only() {
        let open = vec![
            (0.9, json!({ "address": "12/1", "title": "事后议价", "workspace": "agent" })),
            (0.5, json!({ "address": "0/1", "title": "信用", "workspace": "human" })),
        ];
        let locked = vec![(0.8, json!({ "address": "7/1", "title": "沉没投资", "workspace": "agent", "lock": { "lock_id": 4, "tree": "7" } }))];
        let text = closest_blocks(&open, &locked);
        assert!(text.contains("## 用词最接近本节的已有主题卡"), "{text}");
        assert!(text.contains("- [[12/1]] 事后议价（代理，可接续）"), "{text}");
        assert!(text.contains("- [[0/1]] 信用（人类，只读，可链接）"), "{text}");
        assert!(text.contains("## 用词最接近的已锁定主题卡（只读：可作相关语境，可链接，不能写入）"), "{text}");
        assert!(text.contains("- [[7/1]] 沉没投资（已锁定：只读，可链接）"), "{text}");
        // A Locked Card is never offered as a Card to continue.
        assert!(!text.contains("[[7/1]] 沉没投资（代理，可接续）"), "{text}");
        // No Locked Card, no Locked block.
        assert!(!closest_blocks(&open, &[]).contains("已锁定"), "{}", closest_blocks(&open, &[]));
    }

    #[test]
    fn a_good_answer_has_no_fault() {
        let answer = json!({ "section": "K/3", "topics": [{ "id": "T1", "title": "外包合同的定价", "body": "本主题追问：外包价格怎样形成。" }], "ideas": [
            { "id": "I1", "kind": "核心", "title": "议价能力决定外包合同的价格", "body": "价格在议价中形成。例示 [[K/3|b]]：本书用纳什议价。", "sources": ["K/3|b"], "at": "T1" },
            { "id": "I2", "kind": "直觉", "title": "先投资的一方在议价中处于弱势", "body": "投资已经沉没。例示 [[K/3|a]]：本书的供应商先投资。", "sources": ["K/3|a"], "at": "I1", "as": "direct" },
            { "id": "I3", "kind": "关联", "title": "外包合同的不完全性放大了事后议价", "body": "本卡推广 [[12/1]]：把它用到外包。对应 [[K/3|a]]：本书的模型。", "sources": ["K/3|a"], "at": "12/1", "as": "side" },
        ] });
        assert!(validate(&ctx(), "K/3", &answer).is_empty(), "{:?}", validate(&ctx(), "K/3", &answer));
    }

    #[test]
    fn the_checks_name_each_fault() {
        let answer = json!({ "section": "K/3", "topics": [{ "id": "T1", "title": "敲竹杠", "body": "本主题追问。" }], "ideas": [
            { "id": "I1", "kind": "核心", "title": "专用投资完成后供应商被敲竹杠", "body": "依赖 [[K/3|a]] 的设定。", "sources": ["K/3|a"], "at": "T1" },
            { "id": "I2", "kind": "奇怪", "title": "一个新想法", "body": "例示 [[K/9]]：……", "sources": ["K/9"], "at": "0/1" },
            { "id": "I3", "kind": "关联", "title": "另一个想法完全不同", "body": "例示 [[K/3|a]]：……", "sources": ["K/3|a"], "at": "12/1" },
            { "id": "I4", "kind": "关联", "title": "同一主张", "body": "例示 [[K/3|a]]：本书也这样说。", "sources": ["K/3|a"], "same": "K/3|b" },
            { "id": "I5", "kind": "有用", "title": "第五个想法", "body": "例示 [[K/3|a]]：……", "sources": ["K/3|a"], "at": "K/3|a" },
        ] });
        let got = codes(&validate(&ctx(), "K/3", &answer));
        for want in ["T1 G8", "I1 G3", "I1 G4", "I1 G7", "I2 G1", "I2 G2", "I2 G5", "I3 G6", "I4 G9", "I5 G5"] {
            assert!(got.contains(&want.to_string()), "{want} missing in {got:?}");
        }
    }

    #[test]
    fn a_section_places_one_idea_at_a_topic_root() {
        let idea = |id: &str, title: &str, at: &str| {
            json!({ "id": id, "kind": "核心", "title": title, "body": "外包要先找到伙伴。例示 [[K/3|a]]：本书。", "sources": ["K/3|a"], "at": at })
        };
        let topics = json!([{ "id": "T1", "title": "外包合同的定价", "body": "本主题追问：外包价格怎样形成。" }]);
        // Two ideas at one new Topic: the second opens a second line, so it is G13.
        let answer = json!({ "section": "K/3", "topics": topics, "ideas": [idea("I1", "议价能力决定外包合同的价格", "T1"), idea("I2", "搜寻摩擦让企业留在自制", "T1")] });
        let got = codes(&validate(&ctx(), "K/3", &answer));
        assert!(got.contains(&"I2 G13".to_string()) && !got.contains(&"I1 G13".to_string()), "{got:?}");
        // The same at an existing Topic root.
        let answer = json!({ "section": "K/3", "ideas": [idea("I1", "议价能力决定外包合同的价格", "12/0"), idea("I2", "搜寻摩擦让企业留在自制", "12/0"), idea("I3", "厚市场里外包更快成交", "12/0")] });
        let got = codes(&validate(&ctx(), "K/3", &answer));
        assert!(got.contains(&"I2 G13".to_string()) && got.contains(&"I3 G13".to_string()) && !got.contains(&"I1 G13".to_string()), "{got:?}");
        let message = validate(&ctx(), "K/3", &answer).into_iter().find(|f| f.0 == "I2" && f.1 == "G13").unwrap().2;
        assert!(message.contains("I1") && message.contains("\"at\": \"I1\""), "{message}");
        // One idea at each of two Topics, the rest chained: no G13.
        let mut chained = idea("I3", "厚市场里外包更快成交", "I1");
        chained["as"] = json!("direct");
        let answer = json!({ "section": "K/3", "topics": topics, "ideas": [idea("I1", "议价能力决定外包合同的价格", "T1"), idea("I2", "搜寻摩擦让企业留在自制", "12/0"), chained] });
        let got = codes(&validate(&ctx(), "K/3", &answer));
        assert!(!got.iter().any(|c| c.ends_with("G13")), "{got:?}");
    }

    #[test]
    fn a_full_doorway_files_the_next_line_beside_its_last_line() {
        let have = |cards: &[&str]| cards.iter().map(|c| c.to_string()).collect::<HashSet<String>>();
        // Fewer than seven lines: the new line goes beside the doorway.
        let box_ = have(&["9/1", "9/1|a", "9/1|b"]);
        assert_eq!(line_parent("9/1", &mut |a| box_.contains(a)), "9/1");
        // Seven lines: beside the last one.
        let box_ = have(&["9/1", "9/1|a", "9/1|b", "9/1|c", "9/1|d", "9/1|e", "9/1|f", "9/1|g"]);
        assert_eq!(line_parent("9/1", &mut |a| box_.contains(a)), "9/1|g");
        // An older doorway with nine lines: its last line, never a tenth beside the doorway.
        let box_ = have(&["9/1", "9/1|a", "9/1|b", "9/1|c", "9/1|d", "9/1|e", "9/1|f", "9/1|g", "9/1|h", "9/1|i"]);
        assert_eq!(line_parent("9/1", &mut |a| box_.contains(a)), "9/1|i");
        // The last line full too: one level further down.
        let mut cards = vec!["9/1".to_string()];
        cards.extend(('a'..='g').map(|c| format!("9/1|{c}")));
        cards.extend(('a'..='g').map(|c| format!("9/1|g|{c}")));
        let box_: HashSet<String> = cards.into_iter().collect();
        assert_eq!(line_parent("9/1", &mut |a| box_.contains(a)), "9/1|g|g");
    }
}
