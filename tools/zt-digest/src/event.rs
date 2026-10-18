//! `zt-digest event`: the port of `scripts/book-digest/event-compiler.ps1`, the event layer of the
//! economics digest: the Event Cards of a book, written after the digest runs. A compiler cycle:
//! the tool plans, DeepSeek writes a JSON file per job and checks it with the precheck step, the
//! tool writes the Cards.
//!
//! -Staging is the digest's staging folder (manifest.json, events.json from prepare-book-digest,
//! the owned-gNN.txt logs of the digest runs, sections\). -Work (default <Staging>\events) holds
//! the job files, out\<job>.json (DeepSeek's answers), state.json, and created.txt.
//!
//!   plan      groups the event candidates by group, lists each group's own Idea Cards and anchor,
//!             and writes one job file per -GroupsPerJob groups with its SELF-CHECK line
//!             (`zt-digest event -Step precheck …`). Jobs already applied are left out.
//!   precheck  read-only: checks out\<Job>.json as apply would (fields, supports, role Link,
//!             existing Event Card) and runs zt-check on the new Event Cards as drafts.
//!   apply     for each answered job not yet applied: refuses an event with a fault, merges into an
//!             existing Event Card or creates the Event Card as a Side successor of its first
//!             supported Idea Card, then links it from every supported Idea Card ("案例 [[event]]：
//!             title。"). Without -Apply it only validates. Two write runs (the Cards, then the
//!             Link edits with base); a job whose run fails part way stays under partial.
//!   report    the counts of state.json.
//!
//! The shared cycle is `layer` (also used by `finding`).

pub(crate) mod layer;

use crate::args::Args;
use crate::ps::{self, CiMap};
use crate::util;
use anyhow::Result;
use layer::{Ctx, First, Layer, Spec, get_field, int_of, prop, supports};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

static TITLE_BAD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)[；\n]").unwrap());
static ROLE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^本事件(例示|是|作为)").unwrap());
static SOURCE_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^来源：").unwrap());

static SPEC: Spec = Spec {
    tool: "event",
    list: "events",
    prefix: "ev",
    code0: "EV0",
    candidate_code: "EV6",
    work_dir: "events",
    doc: "dsh-event-instructions.zh.md",
    job_title: "事件任务",
    anchor_note: "",
    skip_types: &["model", "event", "hub"],
    candidates_heading: "事件候选：",
    prompt: "按以上说明写出这些组的事件卡 JSON 文件，运行 SELF-CHECK 改到 faults: 0。最后只回复一行 JSON。",
    kind: "案例",
    merges: true,
};

const FIELDS: &[&str] = &["title", "time", "place", "actors", "course", "section", "role"];

/// `Get-EventBody`: the fixed labels the E2 rule checks, then the evidence role.
pub(crate) fn event_body(key: &str, e: &Value) -> String {
    [
        format!("时间：{}", get_field(e, "time")),
        format!("地点：{}", get_field(e, "place")),
        format!("参与者：{}", get_field(e, "actors")),
        format!("经过：{}", get_field(e, "course")),
        format!("来源：[[{key}]] {}", get_field(e, "section")),
        String::new(),
        format!("证据作用：{}", get_field(e, "role")),
    ]
    .join("\n")
}

/// The body of an existing Event Card with this book's source after its last source line and the
/// role at the end; None when the body has both already (a failed run merged it).
pub(crate) fn merged_body(body: &str, source: &str, role: &str) -> Option<String> {
    if body.contains(source) && body.contains(role) {
        return None;
    }
    let mut lines: Vec<String> = body.split('\n').map(str::to_string).collect();
    match lines.iter().rposition(|l| SOURCE_LINE.is_match(l)) {
        Some(at) => lines.insert(at + 1, source.to_string()),
        None => lines.push(source.to_string()),
    }
    lines.push(role.to_string());
    Some(lines.join("\n"))
}

struct Events;

impl Layer for Events {
    fn spec(&self) -> &'static Spec {
        &SPEC
    }

    fn candidates(&self, ctx: &Ctx) -> Result<(usize, Vec<(i64, Vec<(String, String)>)>)> {
        let file = ps::join_path(&ctx.staging, "events.json");
        let candidates = if ps::exists(&file) { ps::ps_items(&ps::parse_json(&util::read_text(&file)?)?) } else { Vec::new() };
        let mut by_group: Vec<(i64, Vec<(String, String)>)> = Vec::new();
        for c in &candidates {
            let n = ps::int(prop(c, "group"))?;
            let item = (ps::s(prop(c, "src")), ps::s(prop(c, "text")));
            match by_group.iter_mut().find(|(g, _)| *g == n) {
                Some((_, list)) => list.push(item),
                None => by_group.push((n, vec![item])),
            }
        }
        Ok((candidates.len(), by_group))
    }

    fn candidate_source(&self, ctx: &Ctx) -> Result<Value> {
        let file = ps::join_path(&ctx.staging, "events.json");
        Ok(if ps::exists(&file) {
            let bytes = std::fs::read(&file)?;
            json!({"kind":"artifact","path":std::fs::canonicalize(&file)?.to_string_lossy(),"sha256":crate::canon::sha256_hex(&bytes)})
        } else {
            json!({"kind":"absent","reason":"the staging folder holds no events.json"})
        })
    }

    fn test(&self, ctx: &mut Ctx, items: &[Value]) -> Result<Vec<String>> {
        let mut faults: Vec<String> = Vec::new();
        let mut titles: CiMap<usize> = CiMap::new();
        let spans = layer::declared_spans(self, ctx)?;
        for (index, e) in items.iter().enumerate() {
            let i = index + 1;
            let id = format!("event:{i}");
            if !e.is_object() {
                faults.push(format!("{id} EV1 not an object"));
                continue;
            }
            for f in FIELDS {
                if get_field(e, f).trim().is_empty() {
                    faults.push(format!("{id} EV1 missing field: {f}"));
                }
            }
            if TITLE_BAD.is_match(&get_field(e, "title")) {
                faults.push(format!("{id} EV1 the title has ； or a line break"));
            }
            let n = int_of(&get_field(e, "group"))?;
            if ctx.group(n)?.is_none() {
                faults.push(format!("{id} EV2 no group {n} in the book"));
                continue;
            }
            let owned = ctx.owned(n)?;
            let supports = supports(e);
            if supports.is_empty() {
                faults.push(format!("{id} EV2 supports is empty"));
            }
            for s in &supports {
                if !ps::is_in(s, &owned) {
                    faults.push(format!("{id} EV2 {s} is not a Card of group {n}"));
                }
            }
            // The entry names the candidate sentence it writes: no Event Card comes from
            // a sentence the plan never enumerated.
            let src = get_field(e, "src").trim().to_string();
            if src.is_empty() {
                faults.push(format!("{id} {} src names no candidate of group {n}", self.spec().candidate_code));
            } else if layer::candidate_at(&spans, n, &src).is_none() {
                faults.push(format!("{id} {} src `{src}` is not a candidate of group {n}", self.spec().candidate_code));
            }
            let role = get_field(e, "role");
            if !supports.is_empty() && !supports.iter().any(|s| role.contains(&format!("[[{s}]]"))) {
                faults.push(format!("{id} EV3 role links no Card of supports"));
            }
            if !role.is_empty() && !ROLE_START.is_match(&role) {
                faults.push(format!("{id} EV3 role does not start with 本事件例示"));
            }
            let t = get_field(e, "title").trim().to_string();
            if !t.is_empty() {
                match titles.get(&t) {
                    Some(first) => faults.push(format!("{id} EV5 the same title as event:{first}")),
                    None => titles.insert(&t, i),
                }
            }
            let existing = get_field(e, "existing").trim().to_string();
            if !existing.is_empty() {
                // A failed read (no such Card) is no Card.
                let card = ctx.get_cards(std::slice::from_ref(&existing)).ok().and_then(|v| v.into_iter().next()).unwrap_or(Value::Null);
                if !ps::truthy(&card) {
                    faults.push(format!("{id} EV4 existing {existing} is not a Card"));
                } else if !ps::eq(&ps::s(prop(&card, "type")), "event") {
                    faults.push(format!("{id} EV4 existing {existing} is not an Event Card"));
                }
            }
        }
        Ok(faults)
    }

    fn drafts(&self, ctx: &Ctx, items: &[Value]) -> Result<Vec<Value>> {
        let mut drafts = Vec::new();
        for (index, e) in items.iter().enumerate() {
            if e.is_object() && get_field(e, "existing").trim().is_empty() {
                drafts.push(json!({
                    "address": format!("event:{}", index + 1), "title": get_field(e, "title"), "body": event_body(&ctx.key, e),
                    "type": "event", "category": "event", "book": ctx.key
                }));
            }
        }
        Ok(drafts)
    }

    fn first(&self, ctx: &mut Ctx, items: &[Value], bad: &[usize], made: &CiMap<String>) -> Result<First> {
        let mut first = First::default();
        for (index, e) in items.iter().enumerate() {
            let i = index + 1;
            if bad.contains(&i) {
                continue;
            }
            let supports = supports(e);
            let existing = get_field(e, "existing").trim().to_string();
            if !existing.is_empty() {
                let card = ctx.get_cards(std::slice::from_ref(&existing))?.into_iter().next().unwrap_or(Value::Null);
                let source = format!("来源：[[{}]] {}", ctx.key, get_field(e, "section"));
                let role = format!("证据作用：{}", get_field(e, "role"));
                if let Some(body) = merged_body(&ps::s(prop(&card, "body")), &source, &role) {
                    first.calls.push(json!({ "tool": "edit_card", "arguments": { "at": existing, "body": body, "base": ps::s(prop(&card, "text")) } }));
                }
                first.refs.insert(&i.to_string(), existing.clone());
                first.merged.push(existing);
            } else if !made.contains(&i.to_string()) {
                first.calls.push(json!({ "tool": "create_successor", "arguments": {
                    "at": supports.first().cloned().unwrap_or_default(), "kind": "side", "type": "event",
                    "title": get_field(e, "title").trim(), "body": event_body(&ctx.key, e), "src": get_field(e, "src")
                } }));
                first.created_by.push((first.calls.len() - 1, i));
            }
        }
        Ok(first)
    }
}

/// `zt-digest event`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Staging", "Work", "Job", "GroupsPerJob"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["Apply"])?;
    let step = a.choice("Step", &["plan", "precheck", "apply", "report"])?;
    let staging = a.required("Staging")?;
    let per_job = a.int_or("GroupsPerJob", 3)?;
    layer::run(&Events, &step, &staging, &a.str("Work"), &a.str("Job"), per_job, a.flag("Apply"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_event_body_has_the_labels() {
        let e = json!({ "time": "1926 年", "place": "美国", "actors": "通用汽车", "course": "收购。", "section": "§1", "role": "本事件例示 [[K/1]]。" });
        assert_eq!(event_body("K", &e), "时间：1926 年\n地点：美国\n参与者：通用汽车\n经过：收购。\n来源：[[K]] §1\n\n证据作用：本事件例示 [[K/1]]。");
    }

    #[test]
    fn a_merge_goes_after_the_last_source_line() {
        let body = "时间：1919 年\n来源：[[A]] §4\n\n证据作用：甲。";
        assert_eq!(
            merged_body(body, "来源：[[K]] §2", "证据作用：乙。").unwrap(),
            "时间：1919 年\n来源：[[A]] §4\n来源：[[K]] §2\n\n证据作用：甲。\n证据作用：乙。"
        );
        assert_eq!(merged_body("无来源", "来源：[[K]] §2", "证据作用：乙。").unwrap(), "无来源\n来源：[[K]] §2\n证据作用：乙。");
        assert!(merged_body("来源：[[K]] §2\n证据作用：乙。", "来源：[[K]] §2", "证据作用：乙。").is_none());
    }

    #[test]
    fn the_job_file_names_the_tool() {
        let block = layer::GroupBlock {
            n: 1,
            chapter: "第一章".into(),
            headings: "一体化；契约".into(),
            cards: vec![("T/1".into(), "标题".into()), ("K/1".into(), String::new())],
            candidates: vec![("sections/s01.md:3".into(), "1926 年，收购。".into())],
        };
        let check = layer::self_check_line(&SPEC, "D:\\s", "D:\\s\\events", "ev-01");
        assert_eq!(check, "SELF-CHECK: zt-digest event -Step precheck -Staging \"D:\\s\" -Work \"D:\\s\\events\" -Job ev-01");
        let text = layer::job_text(&SPEC, "ev-01", "书", "D:\\s\\events\\out\\ev-01.json", &check, "K", &[block]);
        let expected = [
            "# 事件任务 ev-01：《书》", "", "把结果写入 `D:\\s\\events\\out\\ev-01.json`。", "", &check, "",
            "## 第 1 组：第一章：一体化；契约", "", "锚点：`K/1`", "", "本组的观点卡（supports 只能取这些地址）：", "",
            "- `T/1` 标题", "- `K/1` ", "", "事件候选：", "", "- `sections/s01.md:3` 1926 年，收购。", "", "",
        ]
        .join(ps::NL);
        assert_eq!(text, expected);
    }
}
