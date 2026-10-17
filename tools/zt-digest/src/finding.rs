//! `zt-digest finding`: the port of `scripts/book-digest/finding-compiler.ps1`, the
//! empirical-results layer of the economics digest: the Finding Cards of a book, one per empirical
//! result, written after the digest. A compiler cycle: the tool plans, DeepSeek writes a JSON file
//! per job and checks it with the precheck step, the tool writes the Cards.
//!
//! -Staging is the digest's staging folder (manifest.json with each group's section files, the
//! owned-gNN.txt logs of the digest runs, sections\). -Work (default <Staging>\findings) holds the
//! job files, out\<job>.json (DeepSeek's answers), state.json, and created.txt.
//!
//!   plan      finds the result candidates of each group: the lines of its sections that name a
//!             number and a cue of empirical work, at most -MaxCandidates per group; lists each
//!             group's own Idea Cards and anchor; writes one job file per -GroupsPerJob groups with
//!             its SELF-CHECK line (`zt-digest finding -Step precheck …`).
//!   precheck  read-only: checks out\<Job>.json as apply would and runs zt-check on the Finding
//!             Cards as drafts (F1, F2, F3, the Card language).
//!   apply     creates each Finding Card without a fault as a Side successor of its group's anchor
//!             and links it from every supported Idea Card ("印证 [[finding]]：title。"). Without
//!             -Apply it only validates. Two write runs, as `event`.
//!   report    the counts of state.json.
//!
//! The shared cycle is `event::layer`.

use crate::args::Args;
use crate::event::layer::{self, Ctx, First, Layer, Spec, get_field, int_of, prop, supports};
use crate::ps::{self, CiMap};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

static ROLE_START: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^这一结果(印证|支持|例示)").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]").unwrap());
static SKIP: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(!\[|\||#)").unwrap());
/// A cue of empirical work; a candidate line has one and a number.
static CUE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(数据|样本|调查|问卷|访谈|受访|回归|估计|系数|显著|实验|统计|观测|面板|普查|抽样|比例|百分之|[%％])").unwrap());
static DIGIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

static SPEC: Spec = Spec {
    tool: "finding",
    list: "findings",
    prefix: "fd",
    code0: "FD0",
    work_dir: "findings",
    doc: "dsh-finding-instructions.zh.md",
    job_title: "实证结果任务",
    anchor_note: "（结果卡放在它的侧后继上，来源行链接它）",
    skip_types: &["model", "event", "hub", "finding"],
    candidates_heading: "结果候选：",
    prompt: "按以上说明写出这些组的实证结果卡 JSON 文件，运行 SELF-CHECK 改到 faults: 0。最后只回复一行 JSON。",
    kind: "印证",
    merges: false,
};

const FIELDS: &[&str] = &["title", "measure", "data", "method", "result", "limits", "section", "role"];

/// `Get-FindingBody`: the six labels the F1 rule checks, then the role.
pub(crate) fn finding_body(key: &str, e: &Value) -> Result<String> {
    let n = int_of(&get_field(e, "group"))?;
    Ok([
        format!("测量：{}", get_field(e, "measure")),
        format!("数据：{}", get_field(e, "data")),
        format!("方法：{}", get_field(e, "method")),
        format!("结果：{}", get_field(e, "result")),
        format!("局限：{}", get_field(e, "limits")),
        format!("来源：[[{key}/{n}]] {}", get_field(e, "section")),
        String::new(),
        get_field(e, "role"),
    ]
    .join("\n"))
}

/// A section line as a result candidate: Han text, not a figure, table, or heading, with a cue of
/// empirical work and a number; white space runs made one space, cut to 140 UTF-16 units and `…`.
pub(crate) fn candidate(line: &str) -> Option<String> {
    if !HAN.is_match(line) || SKIP.is_match(line) || !CUE.is_match(line) || !DIGIT.is_match(line) {
        return None;
    }
    let clip = SPACES.replace_all(line, " ").trim().to_string();
    Some(if ps::len16(&clip) > 140 { format!("{}…", ps::cut16(&clip, 140)) } else { clip })
}

struct Findings {
    max: i64,
}

impl Layer for Findings {
    fn spec(&self) -> &'static Spec {
        &SPEC
    }

    fn candidates(&self, ctx: &Ctx) -> Result<(usize, Vec<(i64, Vec<(String, String)>)>)> {
        let mut by_group: Vec<(i64, Vec<(String, String)>)> = Vec::new();
        let mut total = 0usize;
        for g in ctx.groups().into_iter().filter(|g| !g.is_null()) {
            let mut list: Vec<(String, String)> = Vec::new();
            for name in ps::list(prop(g, "sections")) {
                let name = ps::s(name);
                let path = ps::join_path(&ctx.staging, &format!("sections\\{name}"));
                if !ps::exists(&path) {
                    continue;
                }
                for (no, line) in ps::read_lines(&path)?.iter().enumerate() {
                    if list.len() as i64 >= self.max {
                        break;
                    }
                    if let Some(clip) = candidate(line) {
                        list.push((format!("sections/{name}:{}", no + 1), clip));
                    }
                }
            }
            if !list.is_empty() {
                let n = ps::int(prop(g, "n"))?;
                total += list.len();
                match by_group.iter_mut().find(|(k, _)| *k == n) {
                    Some((_, old)) => *old = list,
                    None => by_group.push((n, list)),
                }
            }
        }
        Ok((total, by_group))
    }

    fn test(&self, ctx: &mut Ctx, items: &[Value]) -> Result<Vec<String>> {
        let mut faults: Vec<String> = Vec::new();
        let mut titles: CiMap<usize> = CiMap::new();
        for (index, e) in items.iter().enumerate() {
            let i = index + 1;
            let id = format!("finding:{i}");
            if !e.is_object() {
                faults.push(format!("{id} FD1 not an object"));
                continue;
            }
            for f in FIELDS {
                if get_field(e, f).trim().is_empty() {
                    faults.push(format!("{id} FD1 missing field: {f}"));
                }
            }
            for f in FIELDS {
                if get_field(e, f).contains('\n') {
                    faults.push(format!("{id} FD1 {f} has a line break: one line per label"));
                }
            }
            let n = int_of(&get_field(e, "group"))?;
            if ctx.group(n)?.is_none() {
                faults.push(format!("{id} FD2 no group {n} in the book"));
                continue;
            }
            let owned = ctx.owned(n)?;
            let supports = supports(e);
            if supports.is_empty() {
                faults.push(format!("{id} FD2 supports is empty"));
            }
            for s in &supports {
                if !ps::is_in(s, &owned) {
                    faults.push(format!("{id} FD2 {s} is not a Card of group {n}"));
                }
            }
            let role = get_field(e, "role");
            if !supports.is_empty() && !supports.iter().any(|s| role.contains(&format!("[[{s}]]"))) {
                faults.push(format!("{id} FD3 role links no Card of supports"));
            }
            if !role.is_empty() && !ROLE_START.is_match(&role) {
                faults.push(format!("{id} FD3 role does not start with 这一结果印证"));
            }
            let t = get_field(e, "title").trim().to_string();
            if !t.is_empty() {
                match titles.get(&t) {
                    Some(first) => faults.push(format!("{id} FD4 the same title as finding:{first}")),
                    None => titles.insert(&t, i),
                }
            }
        }
        Ok(faults)
    }

    fn drafts(&self, ctx: &Ctx, items: &[Value]) -> Result<Vec<Value>> {
        let mut drafts = Vec::new();
        for (index, e) in items.iter().enumerate() {
            if e.is_object() {
                drafts.push(json!({
                    "address": format!("finding:{}", index + 1), "title": get_field(e, "title"), "body": finding_body(&ctx.key, e)?,
                    "type": "finding", "category": "finding", "book": ctx.key
                }));
            }
        }
        Ok(drafts)
    }

    fn first(&self, ctx: &mut Ctx, items: &[Value], bad: &[usize], made: &CiMap<String>) -> Result<First> {
        let mut first = First::default();
        for (index, e) in items.iter().enumerate() {
            let i = index + 1;
            if bad.contains(&i) || made.contains(&i.to_string()) {
                continue;
            }
            let n = int_of(&get_field(e, "group"))?;
            first.calls.push(json!({ "tool": "create_successor", "arguments": {
                "at": format!("{}/{n}", ctx.key), "kind": "side", "type": "finding",
                "title": get_field(e, "title").trim(), "body": finding_body(&ctx.key, e)?, "src": get_field(e, "src")
            } }));
            first.created_by.push((first.calls.len() - 1, i));
        }
        Ok(first)
    }
}

/// `zt-digest finding`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Staging", "Work", "Job", "GroupsPerJob", "MaxCandidates"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["Apply"])?;
    let step = a.choice("Step", &["plan", "precheck", "apply", "report"])?;
    let staging = a.required("Staging")?;
    let per_job = a.int_or("GroupsPerJob", 3)?;
    let layer = Findings { max: a.int_or("MaxCandidates", 16)? };
    layer::run(&layer, &step, &staging, &a.str("Work"), &a.str("Job"), per_job, a.flag("Apply"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_finding_body_has_the_six_labels() {
        let e = json!({ "group": "2", "measure": "m", "data": "d", "method": "x", "result": "r", "limits": "l", "section": "§2", "role": "这一结果印证 [[K/2]]。" });
        assert_eq!(finding_body("K", &e).unwrap(), "测量：m\n数据：d\n方法：x\n结果：r\n局限：l\n来源：[[K/2]] §2\n\n这一结果印证 [[K/2]]。");
        assert!(finding_body("K", &json!({ "group": "two" })).is_err());
    }

    #[test]
    fn candidates_are_lines_with_a_cue_and_a_number() {
        assert_eq!(candidate("作者访谈了 48 家企业。").as_deref(), Some("作者访谈了 48 家企业。"));
        assert_eq!(candidate("  42%  的企业\t放弃了扩张。 ").as_deref(), Some("42% 的企业 放弃了扩张。"));
        assert_eq!(candidate("这是一个模型的假设：企业是风险中性的。"), None);
        assert_eq!(candidate("# 第 1 章 调查"), None);
        assert_eq!(candidate("| 样本 | 12 |"), None);
        assert_eq!(candidate("Survey data of 1200 firms"), None);
        assert_eq!(candidate("全角数字的样本：１２"), Some("全角数字的样本：１２".to_string()));
        let long = format!("调查 1：{}", "长".repeat(200));
        let clip = candidate(&long).unwrap();
        assert_eq!(ps::len16(&clip), 141);
        assert!(clip.ends_with('…'));
    }

    #[test]
    fn the_job_file_names_the_anchor_note() {
        let block = layer::GroupBlock { n: 2, chapter: "第二章".into(), headings: "调查".into(), cards: vec![], candidates: vec![] };
        let text = layer::job_text(&SPEC, "fd-01", "书", "o.json", "SELF-CHECK: x", "K", &[block]);
        assert!(text.contains("锚点：`K/2`（结果卡放在它的侧后继上，来源行链接它）"));
        assert!(text.starts_with("# 实证结果任务 fd-01：《书》"));
        assert!(text.contains("结果候选："));
    }
}
