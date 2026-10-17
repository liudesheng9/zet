//! `zt-digest hub -Step precheck`: judges one DeepSeek file (out\hub-<tree>.json or
//! out\fields.json) before its writer replies — the members apply would drop, and what get_hub
//! and zt-check would report on the hubs apply builds from it. Read-only: it loads only the agent
//! trees the draft names and writes no Card and no state.

use super::text::{self, FIELD_REF, LINE_REF, TREE_REF, st, tree_id};
use super::{Hub, gt_limit, lt_limit, read_json};
use crate::check::Check;
use crate::ps::{self, CiMap};
use crate::util;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// zt-check's Card language rules the precheck reports on bodies and group names.
const STYLE_RULES: &[&str] = &["2", "3", "4", "5", "7", "9"];

static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[\[").unwrap());
static LINE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^line:").unwrap());
static FIELD_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^field:").unwrap());
static TREE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^tree:").unwrap());
static REF_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(line|tree|field):").unwrap());
static NEAR_GROUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(邻近|neighbours$)").unwrap());
static HUB_LEAF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^hub-(.+)\.json$").unwrap());
static CHECK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(e\d+) R(\d+) (.*)$").unwrap());

/// `$HubRank`: the level of a hub (0 when it is none of these).
fn hub_rank(level: &str) -> i64 {
    match ps::key(level).as_str() {
        "line" => 1,
        "topic" => 2,
        "field" => 3,
        "register" => 4,
        _ => 0,
    }
}

/// `$a -ge $b` on two JSON values: null is below every number and equal to null.
fn ge_value(a: &Value, b: &Value) -> bool {
    match (a.as_f64(), b.as_f64()) {
        (Some(x), Some(y)) => x >= y,
        (Some(_), None) => b.is_null(),
        (None, _) => a.is_null() && b.is_null(),
    }
}

/// How a draft's refs resolve, as apply will resolve them.
enum Refs {
    /// A tree's hub file: `line:<anchor>` (the state's line hubs and the draft's anchors), `tree:<n>`.
    Hub { line_hubs: CiMap<String>, topic_hub: CiMap<String> },
    /// fields.json: `tree:<n>`, `field:<name>` (the state's fields and the file's).
    Field { topic_hub: CiMap<String>, field_names: CiMap<String> },
}

/// The context of one hub of a draft.
struct Ctx {
    at: String,
    level: String,
    anchor: String,
    refs: Refs,
    line_titles: CiMap<String>,
}

/// One hub of a draft as apply builds it: its resolved members, body, and group names.
struct Draft {
    at: String,
    members: Vec<(String, String)>,
    body: String,
    names: Vec<String>,
}

/// `Get-DraftRefs`: every ref, anchor, and tree a draft names, at any depth.
fn draft_refs(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, x) in map {
                if ps::is_in(key, &["ref", "anchor", "tree"]) && !(x.is_array() || x.is_object()) {
                    out.push(st(x));
                } else if ps::eq(key, "refs") && x.is_array() {
                    out.extend(ps::each(x).into_iter().map(st));
                } else {
                    draft_refs(x, out);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|x| draft_refs(x, out)),
        _ => {}
    }
}

/// `Get-Reach`: the hubs and Cards a member list reaches, through the line hubs it lists.
fn reach(edges: &CiMap<Vec<String>>, start: &str) -> HashSet<String> {
    let mut seen = HashSet::new();
    let mut stack: Vec<String> = edges.get(start).cloned().unwrap_or_default();
    while let Some(t) = stack.pop() {
        if !seen.insert(t.clone()) {
            continue;
        }
        if let Some(next) = edges.get(&t) {
            stack.extend(next.iter().cloned());
        }
    }
    seen
}

impl Hub {
    fn add_pre(&mut self, at: &str, code: &str, text: impl AsRef<str>) {
        self.pre.push(format!("{at} {code} {}", text.as_ref()));
    }

    fn limit(&self, name: &str) -> Value {
        self.pre_limits[name].clone()
    }

    /// `Import-Cards`: the Card cache holds only the agent trees these refs name.
    fn import_cards(&mut self, refs: &[String]) -> Result<()> {
        self.cards = Some(HashMap::new());
        let listed = self.read_one("list_trees", json!({ "workspace": "agent" }))?;
        let known: HashSet<String> = ps::each(&listed["trees"]).into_iter().map(|t| ps::key(&st(&t["tree"]))).collect();
        let ids = ps::select_unique(
            refs.iter().map(|r| tree_id(&REF_PREFIX.replace(r, ""))).filter(|id| !id.is_empty() && known.contains(&ps::key(id))),
        );
        if ids.is_empty() {
            return Ok(());
        }
        let calls: Vec<(&str, Value)> = ids.iter().map(|id| ("get_tree", json!({ "tree": id }))).collect();
        let trees = self.reads(&calls)?;
        let cards = self.cards.as_mut().unwrap();
        for t in &trees {
            for c in ps::each(&t["cards"]) {
                cards.insert(ps::key(&st(&c["address"])), c.clone());
            }
        }
        Ok(())
    }

    fn resolve_pre(&mut self, refs: &Refs, r: &str) -> Result<Option<String>> {
        let (topic_hub, line_hubs, field_names) = match refs {
            Refs::Hub { line_hubs, topic_hub } => (topic_hub, Some(line_hubs), None),
            Refs::Field { topic_hub, field_names } => (topic_hub, None, Some(field_names)),
        };
        if let Some(line_hubs) = line_hubs
            && let Some(c) = LINE_REF.captures(r)
        {
            return Ok(line_hubs.get(&c[1]).cloned().filter(|a| !a.is_empty()));
        }
        if let Some(c) = TREE_REF.captures(r) {
            return self.resolve_tree(&c[1], topic_hub);
        }
        if let Some(field_names) = field_names
            && let Some(c) = FIELD_REF.captures(r)
        {
            return Ok(field_names.get(&c[1]).cloned().filter(|a| !a.is_empty()));
        }
        Ok(if self.exists(r)? { Some(r.to_string()) } else { None })
    }

    fn member_title(&self, ctx: &Ctx, survey: &Value, r: &str, address: Option<&str>) -> String {
        if let Some(c) = LINE_REF.captures(r) {
            return ctx.line_titles.get(&c[1]).cloned().unwrap_or_default();
        }
        if let Some(c) = TREE_REF.captures(r)
            && let Some(s) = survey["trees"].as_object().and_then(|m| m.get(&c[1]))
        {
            return st(&s["title"]);
        }
        match address.filter(|a| !a.is_empty()).and_then(|a| self.card(a)) {
            Some(card) => st(&card["title"]),
            None => String::new(),
        }
    }

    /// `Get-MemberRank`: the level of a member that is a hub (0 when it is not one).
    fn member_rank(&self, r: &str, address: &str) -> i64 {
        if LINE_PREFIX.is_match(r) {
            return 1;
        }
        if let Some(card) = self.card(address)
            && ps::eq(&st(&card["type"]), "hub")
        {
            return hub_rank(&st(&card["level"]));
        }
        if FIELD_PREFIX.is_match(r) {
            return 3;
        }
        if TREE_PREFIX.is_match(r) && !address.ends_with("/0") {
            return 2;
        }
        0
    }

    /// `Test-Member`: one line of a hub. Its ref resolves as apply resolves it, and its reason is
    /// long enough and is not the member's title. The resolved address, or None.
    fn test_member(&mut self, ctx: &Ctx, survey: &Value, item: &Value, neighbour: bool) -> Result<Option<String>> {
        let at = ctx.at.clone();
        if !item.is_object() || st(&item["ref"]).is_empty() {
            self.add_pre(&at, "shape", format!("每行要有 ref 和 reason：{}", text::piped_json(item, 4)));
            return Ok(None);
        }
        let r = st(&item["ref"]);
        let address = self.resolve_pre(&ctx.refs, &r)?;
        if address.is_none() {
            let hint = if BRACKETS.is_match(&r) {
                "ref 只写地址，不加 [[ ]]。".to_string()
            } else if let Some(c) = LINE_REF.captures(&r) {
                format!("lines 里没有 anchor 为 {} 的线索枢纽，或这张分支卡不存在。", &c[1])
            } else if let Some(c) = TREE_REF.captures(&r) {
                format!("主题 {} 不存在。", &c[1])
            } else if FIELD_PREFIX.is_match(&r) {
                "这里不能列领域。".to_string()
            } else {
                "这张卡不存在。".to_string()
            };
            self.add_pre(&at, "broken", format!("{r} 解析不出来，脚本会丢掉这一行。{hint}"));
        }
        let words = text::words(&st(&item["reason"]));
        let min = ps::int(&self.limit("min_reason"))?;
        let count = words.chars().count() as i64;
        if count < min {
            self.add_pre(&at, "bare", format!("{r} 的理由只有 {count} 个字（公式和链接不算），至少要 {min} 个字。"));
        } else {
            let title = text::words(&self.member_title(ctx, survey, &r, address.as_deref()));
            if !title.is_empty() && title.contains(&words) {
                self.add_pre(&at, "bare", format!("{r} 的理由重复了卡的标题。写这张卡贡献了什么。"));
            }
        }
        let kind = st(&item["kind"]);
        if neighbour && !kind.is_empty() && !text::is_kind(&kind) {
            self.add_pre(&at, "kind", format!("{r} 的关系词 {kind} 不可用。用 依赖、例示、对照、放宽、对应、反驳 之一，或留空。"));
        }
        if let Some(address) = &address {
            let rank = self.member_rank(&r, address);
            let nested = ps::eq(&ctx.level, "line") && LINE_REF.captures(&r).is_some_and(|c| ps::starts_with(&c[1], &format!("{}|", ctx.anchor)));
            if rank != 0 && rank >= hub_rank(&ctx.level) && !nested {
                self.add_pre(&at, "upward", format!("{r} 是同级或更高一级的枢纽。枢纽只列更低一级的枢纽。"));
            }
        }
        Ok(address)
    }

    /// `Test-Draft`: one hub of a draft — its shape, its lines, and its size (邻近 does not count).
    fn test_draft(&mut self, ctx: &Ctx, survey: &Value, hub: &Value) -> Result<Draft> {
        let at = ctx.at.clone();
        let mut result = Draft { at: at.clone(), members: Vec::new(), body: String::new(), names: Vec::new() };
        if !hub.is_object() {
            self.add_pre(&at, "shape", "应当是一个对象。");
            return Ok(result);
        }
        if st(&hub["title"]).trim().is_empty() {
            self.add_pre(&at, "shape", "缺少 title。");
        }
        if st(&hub["scope"]).trim().is_empty() {
            self.add_pre(&at, "shape", "缺少 scope。");
        }
        let groups: Vec<Value> = match &hub["groups"] {
            Value::Array(items) => items.clone(),
            _ => {
                self.add_pre(&at, "shape", "groups 应当是数组。");
                Vec::new()
            }
        };
        let mut near: Vec<Value> = Vec::new();
        if hub.as_object().is_some_and(|m| m.contains_key("neighbours")) && !hub["neighbours"].is_null() {
            match &hub["neighbours"] {
                Value::Array(items) => near = items.clone(),
                _ => self.add_pre(&at, "shape", "neighbours 应当是数组。"),
            }
        }
        let (mut lines, mut count) = (0i64, 0i64);
        for g in &groups {
            if !g.is_object() || st(&g["name"]).trim().is_empty() || !g["items"].is_array() {
                self.add_pre(&at, "shape", format!("每组要有 name 和 items 数组：{}", text::piped_json(g, 2)));
                continue;
            }
            result.names.push(st(&g["name"]));
            let items = ps::each(&g["items"]);
            for item in &items {
                if let Some(address) = self.test_member(ctx, survey, item, false)? {
                    result.members.push((st(&item["ref"]), address));
                }
            }
            // get_hub does not count a group named 邻近 either.
            if !items.is_empty() && !NEAR_GROUP.is_match(st(&g["name"]).trim()) {
                count += 1;
                lines += items.len() as i64;
            }
        }
        for item in &near {
            if let Some(address) = self.test_member(ctx, survey, item, true)? {
                result.members.push((st(&item["ref"]), address));
            }
        }
        let (max_lines, max_groups) = (self.limit("max_lines"), self.limit("max_groups"));
        if gt_limit(lines, &max_lines) {
            self.add_pre(&at, "oversize", format!("groups 共 {lines} 行，最多 {} 行（neighbours 不算）。只留读者应当从这里进入的卡。", st(&max_lines)));
        }
        if gt_limit(count, &max_groups) {
            self.add_pre(&at, "oversize", format!("groups 有 {count} 组，最多 {} 组（neighbours 不算）。合并相近的组。", st(&max_groups)));
        }
        if count < 2 && lines > 8 {
            self.add_pre(&at, "flat", format!("{lines} 行只分了 {count} 组。分成两组或更多组。"));
        }
        // The body as apply writes it; an unresolved ref keeps its line so its reason is checked too.
        let mut dropped = Vec::new();
        result.body = text::format_body(
            &st(&hub["scope"]),
            &Value::Array(groups),
            &Value::Array(near),
            &mut |r: &str| Ok(Some(self.resolve_pre(&ctx.refs, r)?.unwrap_or_else(|| r.to_string()))),
            &mut dropped,
        )?;
        Ok(result)
    }

    /// `Test-HubFile`: a tree's hub file — the topic hub and its line hubs, and coverage (H9, H10).
    fn test_hub_file(&mut self, leaf: &str, plan: &Value, survey: &Value) -> Result<Vec<Draft>> {
        let tree = st(&plan["tree"]);
        if tree.is_empty() {
            self.add_pre(leaf, "shape", "缺少 tree。");
            return Ok(Vec::new());
        }
        if let Some(c) = HUB_LEAF.captures(leaf)
            && !ps::eq(&c[1], &tree)
        {
            self.add_pre(leaf, "tree", format!("文件名是 {leaf}，tree 却是 {tree}。"));
        }
        let Some(s) = survey["trees"].as_object().and_then(|m| m.get(&tree)).cloned() else {
            self.add_pre(leaf, "tree", format!("主题 {tree} 不在 survey.json 里，脚本会跳过整个文件。"));
            return Ok(Vec::new());
        };
        if !plan["topic"].is_object() {
            self.add_pre(leaf, "shape", "缺少 topic 对象。");
            return Ok(Vec::new());
        }
        let mut lines: Vec<Value> = Vec::new();
        match &plan["lines"] {
            Value::Array(items) => {
                for line in items {
                    if line.is_object() {
                        lines.push(line.clone());
                    } else {
                        self.add_pre(leaf, "shape", "lines 的每一项应当是一个对象。");
                    }
                }
            }
            _ => self.add_pre(leaf, "shape", "lines 应当是数组。没有线索枢纽时写 []。"),
        }
        let mut refs = vec![tree.clone()];
        draft_refs(plan, &mut refs);
        self.import_cards(&refs)?;
        // Line hubs as apply makes them: the state's, and one for each anchor of the draft that exists.
        let mut line_hubs: CiMap<String> = CiMap::new();
        let mut line_titles: CiMap<String> = CiMap::new();
        let mut drafted: Vec<String> = Vec::new();
        if let Some(state_lines) = self.state["trees"].as_object().and_then(|m| m.get(&tree)).and_then(|e| e["lines"].as_object()) {
            for (k, v) in state_lines {
                line_hubs.insert(k, st(v));
            }
        }
        for line in &lines {
            let anchor = st(&line["anchor"]);
            if anchor.is_empty() || !self.exists(&anchor)? {
                self.add_pre(&format!("line:{anchor}"), "broken", format!("anchor {anchor} 不存在，脚本会丢掉这张线索枢纽。"));
                continue;
            }
            if !line_hubs.contains(&anchor) {
                line_hubs.insert(&anchor, format!("line:{anchor}"));
            }
            line_titles.insert(&anchor, st(&line["title"]));
            drafted.push(anchor);
        }
        let topic_hub = self.topic_hubs(survey);
        let at = format!("topic:{tree}");
        let ctx = |at: String, level: &str, anchor: String, line_hubs: &CiMap<String>, line_titles: &CiMap<String>| Ctx {
            at,
            level: level.to_string(),
            anchor,
            refs: Refs::Hub { line_hubs: line_hubs.clone(), topic_hub: topic_hub.clone() },
            line_titles: line_titles.clone(),
        };
        let mut drafts: Vec<Draft> = Vec::new();
        let topic_ctx = ctx(at.clone(), "topic", String::new(), &line_hubs, &line_titles);
        drafts.push(self.test_draft(&topic_ctx, survey, &plan["topic"])?);
        for line in &lines {
            let anchor = st(&line["anchor"]);
            let key = format!("line:{anchor}");
            if !ps::is_in(&anchor, &drafted) || drafts.iter().any(|d| ps::key(&d.at) == ps::key(&key)) {
                continue;
            }
            let line_ctx = ctx(key, "line", anchor, &line_hubs, &line_titles);
            drafts.push(self.test_draft(&line_ctx, survey, line)?);
        }
        // Coverage (H9): a line hub is `line:<anchor>` whether the draft names it by line: or by address.
        let mut by_address: CiMap<String> = CiMap::new();
        for (k, v) in line_hubs.iter() {
            by_address.insert(v, format!("line:{k}"));
        }
        let mut edges: CiMap<Vec<String>> = CiMap::new();
        for d in &drafts {
            let targets = d.members.iter().map(|(_, a)| by_address.get(a).cloned().unwrap_or_else(|| a.clone())).collect();
            edges.insert(&d.at, targets);
        }
        let mut models: Vec<(String, String)> = self
            .cards
            .as_ref()
            .unwrap()
            .values()
            .filter(|c| ps::eq(&tree_id(&st(&c["address"])), &tree) && ps::eq(&st(&c["type"]), "model") && !ps::truthy(&c["facet"]))
            .map(|c| (st(&c["address"]), st(&c["title"])))
            .collect();
        ps::ps_sort(&mut models, |a, b| ps::culture_cmp(&a.0, &b.0));
        let covered = reach(&edges, &at);
        for (m, title) in &models {
            if !covered.contains(m) {
                self.add_pre(&at, "unlisted", format!("模型卡 [[{m}]] {title} 不在主题枢纽里，也不在它列出的线索枢纽里。"));
            }
        }
        let line_min = self.limit("line_min");
        for b in ps::each(&s["branches"]) {
            let required = ge_value(&b["cards"], &line_min) || ge_value(&b["models"], &json!(2));
            let anchor = st(&b["anchor"]);
            if required && !ps::is_in(&anchor, &drafted) {
                self.add_pre(&at, "unlisted", format!("分支 [[{anchor}]] 必须有线索枢纽。在 lines 里加一项，anchor 写 {anchor}。"));
            }
        }
        for anchor in drafted.clone() {
            let key = format!("line:{anchor}");
            if !covered.contains(&key) {
                self.add_pre(&at, "unlisted", format!("线索枢纽 line:{anchor} 不在主题枢纽里。用 line:{anchor} 列出它，或由外层分支的线索枢纽列出它。"));
            }
            if !drafts.iter().any(|d| ps::key(&d.at) == ps::key(&key)) {
                continue;
            }
            let reached = reach(&edges, &key);
            let prefix = format!("{anchor}|");
            for (m, _) in models.iter().filter(|(m, _)| ps::eq(m, &anchor) || ps::starts_with(m, &prefix)) {
                if !reached.contains(m) {
                    self.add_pre(&key, "unlisted", format!("模型卡 [[{m}]] 在这一支里，这张线索枢纽没有列出它。"));
                }
            }
            for inner in drafted.iter().filter(|d| ps::starts_with(d, &prefix)) {
                if !reached.contains(&format!("line:{inner}")) {
                    self.add_pre(&key, "unlisted", format!("内层的线索枢纽 line:{inner} 在这一支里。用 line:{inner} 列出它。"));
                }
            }
        }
        // Neighbours (H10): a tree that shares neighbour_links Links with another tree lists a Card outside it.
        let neighbour_links = self.limit("neighbour_links");
        let mut near = Vec::new();
        for n in ps::pipe(&s["neighbours"]) {
            if !lt_limit(ps::int(&n["links"])?, &neighbour_links) {
                near.push(st(&n["tree"]));
            }
        }
        if !near.is_empty() {
            let outside = drafts[0].members.iter().any(|(r, a)| !LINE_PREFIX.is_match(r) && !ps::eq(&tree_id(a), &tree));
            if !outside {
                let first: Vec<String> = near.iter().take(5).cloned().collect();
                self.add_pre(&at, "isolated", format!("主题 {tree} 与主题 {} 有链接，主题枢纽却没有列出本树以外的卡。在 neighbours 里写一到五行。", first.join("、")));
            }
        }
        Ok(drafts)
    }

    /// `Test-FieldsFile`: fields.json — each field hub (Topics as `tree:<n>`) and the register's terms.
    fn test_fields_file(&mut self, leaf: &str, plan: &Value, survey: &Value) -> Result<Vec<Draft>> {
        let fields: Vec<Value> = match &plan["fields"] {
            Value::Array(items) => items.clone(),
            _ => {
                self.add_pre(leaf, "shape", "fields 应当是数组。");
                Vec::new()
            }
        };
        let register: Vec<Value> = match &plan["register"] {
            Value::Array(items) => items.clone(),
            _ => {
                self.add_pre(leaf, "shape", "register 应当是数组。");
                Vec::new()
            }
        };
        let mut refs = Vec::new();
        draft_refs(plan, &mut refs);
        self.import_cards(&refs)?;
        let topic_hub = self.topic_hubs(survey);
        // A field: ref names a field of this file or one the state already has.
        let mut field_names: CiMap<String> = CiMap::new();
        if let Some(state_fields) = self.state["fields"].as_object() {
            for (name, address) in state_fields {
                field_names.insert(name, st(address));
            }
        }
        for field in &fields {
            let name = st(&field["name"]);
            if field.is_object() && !name.is_empty() {
                field_names.insert(&name, format!("field:{name}"));
            }
        }
        let refs = Refs::Field { topic_hub: topic_hub.clone(), field_names: field_names.clone() };
        let (field_min, field_max) = (self.limit("field_min"), self.limit("field_max"));
        let mut drafts = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            if !field.is_object() || st(&field["name"]).trim().is_empty() {
                self.add_pre(leaf, "shape", format!("第 {} 个领域缺少 name。", i + 1));
                continue;
            }
            let at = format!("field:{}", st(&field["name"]));
            // apply lists each Topic of a field as tree:<n>.
            let groups = match &field["groups"] {
                Value::Array(items) => Value::Array(
                    items
                        .iter()
                        .map(|g| {
                            if g.is_object() && g["items"].is_array() {
                                let items: Vec<Value> = ps::each(&g["items"])
                                    .into_iter()
                                    .map(|x| if x.is_object() { json!({ "ref": format!("tree:{}", st(&x["tree"])), "reason": x["reason"] }) } else { x.clone() })
                                    .collect();
                                json!({ "name": g["name"], "items": items })
                            } else {
                                g.clone()
                            }
                        })
                        .collect(),
                ),
                other => other.clone(),
            };
            let ctx = Ctx {
                at: at.clone(),
                level: "field".into(),
                anchor: String::new(),
                refs: Refs::Field { topic_hub: topic_hub.clone(), field_names: field_names.clone() },
                line_titles: CiMap::new(),
            };
            let draft = self.test_draft(&ctx, survey, &json!({ "title": field["title"], "scope": field["scope"], "groups": groups }))?;
            let topics = ps::select_unique(draft.members.iter().map(|(r, _)| r.clone())).len() as i64;
            if lt_limit(topics, &field_min) || gt_limit(topics, &field_max) {
                self.add_pre(&at, "size", format!("领域有 {topics} 个可用的主题，应当在 {} 到 {} 个之间。", st(&field_min), st(&field_max)));
            }
            drafts.push(draft);
        }
        for (j, t) in register.iter().enumerate() {
            let j = j + 1;
            if !t.is_object() {
                self.add_pre("register", "shape", format!("第 {j} 个关键词应当是一个对象。"));
                continue;
            }
            let term = st(&t["term"]).trim().to_string();
            if term.is_empty() {
                self.add_pre("register", "bare", format!("第 {j} 个关键词缺少 term。"));
            }
            let Some(list) = t["refs"].as_array().filter(|l| !l.is_empty()) else {
                self.add_pre("register", "shape", format!("关键词「{term}」要有 refs 数组，一到三项。"));
                continue;
            };
            let mut ok = 0;
            for r in list {
                if self.resolve_pre(&refs, &st(r))?.is_some() {
                    ok += 1;
                } else {
                    self.add_pre("register", "broken", format!("关键词「{term}」的 {} 解析不出来，脚本会丢掉它。", st(r)));
                }
            }
            if list.len() > 3 {
                self.add_pre("register", "oversize", format!("关键词「{term}」有 {} 个入口，最多 3 个。", list.len()));
            }
            if ok == 0 {
                self.add_pre("register", "broken", format!("关键词「{term}」没有可用的入口，脚本会丢掉这个关键词。"));
            }
        }
        Ok(drafts)
    }

    /// `Invoke-StyleCheck`: zt-check on the bodies and the group names, as plain entries (no
    /// category): R2, R3, R4, R5, R7, and R9.
    fn style_check(&mut self, drafts: &[Draft]) -> Result<()> {
        let mut entries = Vec::new();
        let mut owners: HashMap<String, (String, &str)> = HashMap::new();
        for d in drafts {
            if !d.body.is_empty() {
                let id = format!("e{}", entries.len());
                owners.insert(id.clone(), (d.at.clone(), ""));
                entries.push(json!({ "address": id, "title": "", "body": d.body, "facet": null }));
            }
            let names: Vec<&String> = d.names.iter().filter(|n| !n.is_empty()).collect();
            if !names.is_empty() {
                let id = format!("e{}", entries.len());
                owners.insert(id.clone(), (d.at.clone(), "组名："));
                entries.push(json!({ "address": id, "title": "", "body": names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join("\n"), "facet": null }));
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        let tmp = std::env::temp_dir().join(format!("hub-precheck-{}-{:08x}.json", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0)));
        let tmp = tmp.to_string_lossy().into_owned();
        util::write_text(&tmp, &ps::json_pretty(&Value::Array(entries)))?;
        let result = match self.zt() {
            Ok(zt) => Check { json: tmp.clone(), ..Check::default() }.run(zt),
            Err(e) => Err(e),
        };
        let _ = std::fs::remove_file(&tmp);
        let out = match result {
            Ok(out) => out,
            Err(e) => {
                self.add_pre("style", "error", format!("zt-check 失败：{e:#}"));
                return Ok(());
            }
        };
        for line in out {
            let Some(c) = CHECK_LINE.captures(&line) else { continue };
            let Some((at, prefix)) = owners.get(&ps::key(&c[1])).cloned() else { continue };
            if STYLE_RULES.contains(&&c[2]) {
                self.add_pre(&at, &format!("R{}", &c[2]), format!("{prefix}{}", &c[3]));
            }
        }
        Ok(())
    }

    pub(super) fn step_precheck(&mut self) -> Result<()> {
        if self.p.file.is_empty() {
            bail!("-Step precheck needs -File <hub-<tree>.json or fields.json>");
        }
        let path = super::full_path(&self.p.file)?;
        let leaf = ps::leaf(&path);
        self.pre_limits = self.read_limits()?;
        let survey = read_json(&ps::join(&self.work, "survey.json"))?;
        let mut plan = Value::Null;
        if !std::path::Path::new(&path).is_file() {
            self.add_pre(&leaf, "json", "文件不存在。");
        } else {
            match read_json(&path) {
                Ok(v) => plan = v,
                Err(e) => {
                    let message = format!("{e:#}").split_whitespace().collect::<Vec<_>>().join(" ");
                    self.add_pre(&leaf, "json", format!("不是合法 JSON：{message}"));
                }
            }
            if plan.is_null() && self.pre.is_empty() {
                self.add_pre(&leaf, "json", "文件是空的。");
            } else if !plan.is_null() && !plan.is_object() {
                self.add_pre(&leaf, "shape", "顶层应当是一个对象。");
                plan = Value::Null;
            }
        }
        if !plan.is_null() {
            let fields = ps::eq(&leaf, "fields.json") || plan.as_object().is_some_and(|m| m.contains_key("fields") || m.contains_key("register"));
            let drafts = if fields { self.test_fields_file(&leaf, &plan, &survey)? } else { self.test_hub_file(&leaf, &plan, &survey)? };
            self.style_check(&drafts)?;
        }
        let mut lines = std::mem::take(&mut self.pre);
        lines.push(format!("faults: {}", lines.len()));
        util::print_lines(&lines)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draft_refs_find_every_ref_anchor_and_tree() {
        let plan = json!({ "tree": "3", "topic": { "groups": [{ "items": [{ "ref": "3/1" }, { "Ref": "line:3/2" }] }] },
            "lines": [{ "anchor": "3/2", "neighbours": [{ "ref": "4/1" }] }], "register": [{ "refs": ["tree:5", "field:x"] }], "x": { "tree": ["no"] } });
        let mut refs = Vec::new();
        draft_refs(&plan, &mut refs);
        assert_eq!(refs, ["3", "3/1", "line:3/2", "3/2", "4/1", "tree:5", "field:x"]);
    }

    #[test]
    fn reach_follows_line_hubs() {
        let mut edges = CiMap::new();
        edges.insert("topic:3", vec!["3/1".to_string(), "line:3/2".to_string()]);
        edges.insert("line:3/2", vec!["3/2|b".to_string(), "line:3/2|a".to_string()]);
        edges.insert("line:3/2|a", vec!["3/2|a|c".to_string()]);
        let got = reach(&edges, "topic:3");
        for x in ["3/1", "line:3/2", "3/2|b", "line:3/2|a", "3/2|a|c"] {
            assert!(got.contains(x), "{x}");
        }
        assert!(!reach(&edges, "line:3/2").contains("3/1"));
        assert_eq!(hub_rank("Topic"), 2);
        assert!(NEAR_GROUP.is_match("邻近主题") && NEAR_GROUP.is_match("Neighbours") && !NEAR_GROUP.is_match("neighbours x"));
    }
}
