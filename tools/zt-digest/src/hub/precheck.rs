//! `zt-digest hub -Step precheck`: judges one DeepSeek file (out\hub-<tree>.json or
//! out\fields.json) before its writer replies — the members apply would drop, and what get_hub
//! and zt-check would report on the hubs apply builds from it. Read-only: it loads only the agent
//! trees the draft names and writes no Card and no state.

use super::text::{self, FIELD_REF, LINE_REF, TREE_REF, st, tree_id};
use super::{Hub, gt_limit, lt_limit, read_json};
use crate::check::Check;
use crate::ps::{self, CiMap};
use crate::util;

/// A title for exact comparison: whitespace collapsed, case ignored (D169).
fn normalize_title(title: &str) -> String {
    title.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// zt-check's Card language rules the precheck reports on bodies and group names.
const STYLE_RULES: &[&str] = &["2", "3", "4", "5", "7", "9"];

/// The most entry Cards a Topic body's `## 入口` section may name (§8.2, D167).
const ENTRY_MAX: usize = 3;

static BRACKETS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\[\[").unwrap());
static LINE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^line:").unwrap());
static FIELD_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^field:").unwrap());
static TREE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^tree:").unwrap());
static REF_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(line|tree|field):").unwrap());
static NEAR_GROUP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(邻近|neighbours$)").unwrap());
static HUB_LEAF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^hub-(.+)\.json$").unwrap());
static CHECK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(e\d+) R(\d+) (.*)$").unwrap());

/// Folgezettel order as `get_hub` computes it (`model::address_order` in the zt binary):
/// numbers by value, side letters by length then alphabet, so `3/2|b` precedes `3/2|aa` and
/// `3/10` follows `3/9`. The precheck's `mirror` test needs the same order the report uses.
fn address_order(a: &str, b: &str) -> std::cmp::Ordering {
    fn segments(address: &str) -> Vec<(u8, u64, &str)> {
        address
            .split(['/', '|'])
            .map(|segment| match segment.parse::<u64>() {
                Ok(number) => (0, number, ""),
                Err(_) => (1, segment.len() as u64, segment),
            })
            .collect()
    }
    segments(a).cmp(&segments(b)).then_with(|| a.cmp(b))
}

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

/// The Card a hub address hangs under, as zt's `parent_location` reads it: `3/1|a` → `3/1`,
/// `3/2` → `3/1`, `3/1` → `3/0`. None for a Topic root or a string that is no address.
fn address_parent(location: &str) -> Option<String> {
    let (prefix, last) = location.rsplit_once(['/', '|'])?;
    let side = location.as_bytes().get(prefix.len()) == Some(&b'|');
    if last.chars().all(|c| c.is_ascii_alphabetic()) {
        return Some(prefix.to_string());
    }
    let number: u64 = last.parse().ok()?;
    if number > 1 {
        return Some(format!("{prefix}{}{}", if side { '|' } else { '/' }, number - 1));
    }
    Some(if side { prefix.to_string() } else { format!("{prefix}/0") })
}

/// The address apply rewrites or creates for a tree's topic hub: the state's or survey's hub,
/// else the tree's legacy hub it rewrites, else `<tree>/1` where it creates one.
fn topic_hub_address(known: Option<&str>, legacy: Option<&str>, tree: &str) -> String {
    match known.filter(|address| !address.is_empty()).or_else(|| legacy.filter(|address| !address.is_empty())) {
        Some(address) => address.to_string(),
        None => format!("{tree}/1"),
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

    /// The title of the Card `anchor`, normalized; a Card outside the imported trees falls
    /// back to the survey's title of `tree` (a Topic Card's own tree).
    fn anchor_title(&self, survey: &Value, anchor: &str, tree: &str) -> String {
        match self.card(anchor) {
            Some(card) => normalize_title(&st(&card["title"])),
            None => normalize_title(
                &survey["trees"].as_object().and_then(|m| m.get(tree)).map(|s| st(&s["title"])).unwrap_or_default(),
            ),
        }
    }

    /// `anchor-title` (D130, D169): a new line or topic hub may not repeat the title of the
    /// Card it hangs under — the reader learns nothing about the way in. A topic hub also
    /// names the tree's Topic Card: that is the anchor `get_hub` reports the fault against.
    /// Whitespace and case are normalized; a differently worded title is no fault.
    fn test_anchor_title(&mut self, ctx: &Ctx, survey: &Value, hub: &Value) {
        let title = normalize_title(&st(&hub["title"]));
        if title.is_empty() {
            return;
        }
        let (anchors, tree) = if ps::eq(&ctx.level, "line") {
            (vec![ctx.anchor.clone()], String::new())
        } else if ps::eq(&ctx.level, "topic") {
            let tree = ctx.at.trim_start_matches("topic:").to_string();
            // A topic hub is a Side successor: apply rewrites the tree's hub (or legacy hub)
            // and creates a new one under `<tree>/1`, so that Card is the one it hangs under.
            let tree_entry = survey["trees"].as_object().and_then(|m| m.get(&tree));
            let legacy = ps::each(&tree_entry.map(|s| s["legacy"].clone()).unwrap_or(Value::Null))
                .first()
                .map(|at| st(at))
                .unwrap_or_default();
            let known = self.topic_hubs(survey).get(&tree).cloned().unwrap_or_default();
            let parent = address_parent(&topic_hub_address(Some(&known), Some(&legacy), &tree))
                .unwrap_or_else(|| format!("{tree}/1"));
            let topic = format!("{tree}/0");
            let anchors = if parent == topic { vec![topic] } else { vec![topic, parent] };
            (anchors, tree)
        } else {
            return;
        };
        let copied = anchors.into_iter().find(|anchor| {
            let upper = self.anchor_title(survey, anchor, &tree);
            !upper.is_empty() && title == upper
        });
        let Some(anchor) = copied else { return };
        self.add_pre(
            &ctx.at,
            "anchor-title",
            format!("标题和锚点 `{anchor}` 的标题相同。改写成说明入口作用的标题，例如 消费需求的分解入口。"),
        );
    }

    /// The entry Cards a draft writes into the Topic body instead of a topic hub (§8.2, D167,
    /// D168): one to three Cards of the tree, each saying what it adds. Returns true when the
    /// draft takes that alternative, so no topic hub is judged for the tree.
    fn test_entries(&mut self, at: &str, tree: &str, s: &Value, survey: &Value, plan: &Value) -> Result<bool> {
        let entries = &plan["topic"]["entries"];
        // `ps_count` counts null as one; a draft with no `entries` key has no entry section.
        if !ps::truthy(entries) || ps::each(entries).is_empty() {
            return Ok(false);
        }
        // A tree with line hubs needs a topic hub to list them (H3: a line hub no higher hub
        // lists is unanchored), so the entry section cannot replace one there.
        let line_min = self.limit("line_min");
        let branched = ps::each(&s["branches"]).into_iter().any(|b| {
            ge_value(&b["cards"], &line_min) || ge_value(&b["models"], &json!(2))
        });
        let mut existing: Vec<String> = Vec::new();
        let known = self.topic_hubs(survey).get(tree).cloned().unwrap_or_default();
        if !known.is_empty() {
            existing.push(known);
        }
        if let Some(legacy) = ps::each(&s["legacy"]).into_iter().map(st).next()
            && !legacy.is_empty()
        {
            existing.push(legacy);
        }
        if let Some(hub) = existing.first() {
            self.add_pre(at, "entry", format!("主题 {tree} 已经有主题枢纽 [[{hub}]]，entries 会被丢掉。写主题枢纽的 groups。"));
            return Ok(false);
        }
        if branched {
            self.add_pre(
                at,
                "entry",
                format!("主题 {tree} 有需要线索枢纽的分支，线索枢纽要由主题枢纽列出。写主题枢纽的 groups，不要写 entries。"),
            );
            return Ok(false);
        }
        let items = ps::each(entries);
        if items.len() > ENTRY_MAX {
            self.add_pre(at, "oversize", format!("entries 有 {} 项，最多 {ENTRY_MAX} 项。只留读者应当从这里进入的卡。", items.len()));
        }
        let mut seen: Vec<String> = Vec::new();
        for item in &items {
            if !item.is_object() || st(&item["ref"]).is_empty() {
                self.add_pre(at, "shape", format!("entries 的每一项要有 ref 和 reason：{}", text::piped_json(item, 4)));
                continue;
            }
            let r = st(&item["ref"]);
            if REF_PREFIX.is_match(&r) {
                self.add_pre(at, "broken", format!("entries 只写主题里的卡片地址：{r}"));
                continue;
            }
            if !self.exists(&r)? {
                self.add_pre(at, "broken", format!("entries 里的 {r} 不存在，脚本会丢掉这一项。"));
                continue;
            }
            let tree_of = tree_id(&r);
            if !ps::eq(&tree_of, tree) || ps::eq(&r, &format!("{tree}/0")) {
                self.add_pre(at, "entry", format!("entries 里的 {r} 不是主题 {tree} 的卡。入口只能是本树的卡。"));
                continue;
            }
            if self.card(&r).is_some_and(|c| ps::eq(&st(&c["type"]), "hub")) {
                self.add_pre(at, "entry", format!("entries 里的 {r} 是枢纽卡。入口写读者要进入的实质卡。"));
                continue;
            }
            if ps::is_in(&r, &seen) {
                self.add_pre(at, "entry", format!("entries 把 {r} 写了两遍。每张卡只写一次。"));
                continue;
            }
            seen.push(r.clone());
            if st(&item["reason"]).trim().is_empty() {
                self.add_pre(at, "bare", format!("entries 里的 {r} 没有理由。写一句话，说这张卡贡献了什么。"));
            }
        }
        Ok(true)
    }

    /// `mirror` as `get_hub` computes it (§8.2, D167): a topic hub of four or more lines whose
    /// every line lies in its own tree in non-decreasing address order repeats the tree map. Such
    /// a draft is refused; the writer lists one to three entry Cards in the Topic body instead.
    fn test_mirror(&mut self, at: &str, tree: &str, hub: &Value) {
        let mut lines: Vec<String> = Vec::new();
        let mut outside = false;
        let push = |this: &Self, item: &Value, lines: &mut Vec<String>, outside: &mut bool| {
            if !item.is_object() {
                return;
            }
            let r = st(&item["ref"]);
            let address = if let Some(c) = LINE_REF.captures(&r) { c[1].to_string() } else { r };
            if address.is_empty() || (!LINE_PREFIX.is_match(&st(&item["ref"])) && this.card(&address).is_none()) {
                return;
            }
            if !ps::eq(&tree_id(&address), tree) {
                *outside = true;
            }
            lines.push(address);
        };
        for g in ps::pipe(&hub["groups"]) {
            for item in ps::pipe(&g["items"]) {
                push(self, item, &mut lines, &mut outside);
            }
        }
        for item in ps::pipe(&hub["neighbours"]) {
            push(self, item, &mut lines, &mut outside);
        }
        if outside || lines.len() < 4 {
            return;
        }
        let ordered = lines.windows(2).all(|pair| address_order(&pair[0], &pair[1]) != std::cmp::Ordering::Greater);
        if ordered {
            self.add_pre(
                at,
                "mirror",
                format!(
                    "主题枢纽按地址顺序抄了主题 {tree} 的图，读者在树图里已经看得到。不要写这张枢纽：在 topic 里写 entries（一到三项，每项 ref 和 reason），脚本把它们写进主题卡的“## 入口”一节。"
                ),
            );
        }
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
        self.test_anchor_title(ctx, survey, hub);
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
        // §8.2 D167: a topic hub whose only useful lines would repeat the tree map is not written.
        // The writer lists one to three entry Cards in the Topic body instead, and a tree that
        // takes that alternative needs no topic hub here (D168).
        let entries = self.test_entries(&at, &tree, &s, survey, plan)?;
        let topic_ctx = ctx(at.clone(), "topic", String::new(), &line_hubs, &line_titles);
        if entries && lines.is_empty() {
            drafts.push(Draft {
                at: at.clone(),
                members: Vec::new(),
                body: text::entries_body(&plan["topic"]["entries"], &mut |r: &str| Ok(Some(r.to_string())))?,
                names: Vec::new(),
            });
            // The entry section is the whole answer: no topic hub exists to list model Cards,
            // line hubs, or a neighbour, so the hub coverage checks do not apply (D168).
            return Ok(drafts);
        }
        if entries {
            self.add_pre(&at, "entry", "entries 和 lines 只能选一种：有线索枢纽就要写主题枢纽。");
        }
        drafts.push(self.test_draft(&topic_ctx, survey, &plan["topic"])?);
        self.test_mirror(&at, &tree, &plan["topic"]);
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
    fn an_address_parent_is_the_card_it_hangs_under() {
        assert_eq!(address_parent("3/1|a").as_deref(), Some("3/1"));
        assert_eq!(address_parent("3/1|a|2").as_deref(), Some("3/1|a|1"));
        assert_eq!(address_parent("3/2").as_deref(), Some("3/1"));
        assert_eq!(address_parent("3/1").as_deref(), Some("3/0"));
        assert_eq!(address_parent("3"), None);
        assert_eq!(address_parent(""), None);
    }

    #[test]
    fn a_topic_hub_address_follows_what_apply_writes() {
        // Apply rewrites the tree's known hub, else its legacy hub, else creates `<tree>/1`.
        assert_eq!(topic_hub_address(Some("3/1|a"), Some("3/2|b"), "3"), "3/1|a");
        assert_eq!(topic_hub_address(None, Some("3/2|b"), "3"), "3/2|b");
        assert_eq!(topic_hub_address(Some(""), Some(""), "3"), "3/1");
        assert_eq!(address_parent("3/2|b").as_deref(), Some("3/2"));
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

    /// A precheck with the named Cards imported, as `test_hub_file` leaves it.
    fn hub_with(addresses: &[&str]) -> crate::hub::Hub {
        let mut cards = HashMap::new();
        for address in addresses {
            cards.insert(ps::key(address), json!({"address": address, "title": format!("卡 {address}"), "type": "idea"}));
        }
        crate::hub::Hub {
            p: crate::hub::Params {
                step: "precheck".into(),
                file: String::new(),
                limits: String::new(),
                trees: String::new(),
                per_job: 0,
                rebuild: false,
            },
            work: String::new(),
            out: String::new(),
            state_file: String::new(),
            state: json!({}),
            doc: String::new(),
            zt: None,
            cards: Some(cards),
            pre: Vec::new(),
            pre_limits: json!({ "topic_min": 7, "line_min": 12, "max_lines": 25, "max_groups": 7, "min_reason": 8, "neighbour_links": 3 }),
            stage: None,
        }
    }

    /// Four or more lines of one tree in address order repeat the tree map; a line from another
    /// tree, or an order that is not the map's, is a hub that adds a way in (D167).
    #[test]
    fn a_topic_hub_in_address_order_is_a_mirror() {
        let mut hub = hub_with(&["3/1", "3/2", "3/3", "3/4", "3/10", "4/1"]);
        let map = json!({"groups": [{"name": "主线", "items": [
            {"ref": "3/1", "reason": "第一步。"}, {"ref": "3/2", "reason": "第二步。"},
            {"ref": "3/3", "reason": "第三步。"}, {"ref": "3/10", "reason": "第十步。"}]}], "neighbours": []});
        hub.test_mirror("topic:3", "3", &map);
        assert!(hub.pre.iter().any(|line| line.contains(" mirror ")), "{:?}", hub.pre);
        // Numbers compare by value, so 3/10 after 3/3 is still the map's order.
        assert_eq!(address_order("3/10", "3/9"), std::cmp::Ordering::Greater);
        // A Card from another tree is a way out of the tree: no mirror.
        let mut hub = hub_with(&["3/1", "3/2", "3/3", "3/4", "4/1"]);
        let neighbour = json!({"groups": [{"name": "主线", "items": [
            {"ref": "3/1", "reason": "第一步。"}, {"ref": "3/2", "reason": "第二步。"},
            {"ref": "3/3", "reason": "第三步。"}]}],
            "neighbours": [{"kind": "对照", "ref": "4/1", "reason": "另一本书的讲法。"}]});
        hub.test_mirror("topic:3", "3", &neighbour);
        assert!(hub.pre.is_empty(), "{:?}", hub.pre);
        // A hub that groups by concept is not in address order: no mirror.
        let mut hub = hub_with(&["3/1", "3/2", "3/3", "3/4"]);
        let grouped = json!({"groups": [{"name": "后果", "items": [
            {"ref": "3/3", "reason": "第三步。"}, {"ref": "3/1", "reason": "第一步。"},
            {"ref": "3/4", "reason": "第四步。"}, {"ref": "3/2", "reason": "第二步。"}]}], "neighbours": []});
        hub.test_mirror("topic:3", "3", &grouped);
        assert!(hub.pre.is_empty(), "{:?}", hub.pre);
        // Three lines are too few to repeat the map.
        let mut hub = hub_with(&["3/1", "3/2", "3/3"]);
        let short = json!({"groups": [{"name": "主线", "items": [
            {"ref": "3/1", "reason": "第一步。"}, {"ref": "3/2", "reason": "第二步。"},
            {"ref": "3/3", "reason": "第三步。"}]}], "neighbours": []});
        hub.test_mirror("topic:3", "3", &short);
        assert!(hub.pre.is_empty(), "{:?}", hub.pre);
    }
}
