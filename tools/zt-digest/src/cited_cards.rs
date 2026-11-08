//! `zt-digest cited-cards`: the Cited Cards of a digest (#220, docs/cited-literature-cards-proposal.md).
//! Every row of the book's Cited list gets one Cited Card. DeepSeek writes each work's content as
//! JSON after it reviews how the work is used in the WHOLE source (the digest's notes often leave
//! citations out); the tool renders, checks and publishes the Cards as the list's immediate Side
//! successors in row order.
//!
//!   zt-digest cited-cards -Step plan|jobs|precheck|check|apply -Staging <folder> [-PerJob 12]
//!                         [-Tokens <job:token,...>] [-Draft <file>] [-Token <n>] [-Apply]
//!
//!   plan      groups the list's rows into writer jobs (-PerJob, 1–12), freezes cited-cards\plan.json
//!             and writes each job's file; in a prepared run it registers `citedcards/tNNN@<rev>`
//!             and `citedcards/book@<rev>`. Writes no Card. Prints {"ok","jobs","revision","rows"}.
//!   jobs      lists the writer jobs (only those -Tokens names, when given). Prints {"ok","jobs":[…]}.
//!   precheck  renders one draft (-Draft) and checks it: C3 (every location is a source line that
//!             names the work; a work with no location takes only 背景) and the zt-check rules of a
//!             Cited Card, R6 included. One fault per line, then `faults: N`.
//!   check     the readiness of every row (cited-cards\check\readiness.json). Prints {"ok","ready","not_ready"}.
//!   apply     with -Apply publishes the ready prefix of rows in order, updates a changed Card, fills
//!             the list's `引用处` and `关系` cells, and writes cited-cards\report.json. Prints
//!             {"ok","rows","committed","complete"}.

use crate::args::Args;
use crate::cited;
use crate::organize::common::{self, pstr};
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;

const INSTRUCTION: &str = "dsh-cited-card-instructions.zh.md";
const PROMPT: &str = "按以上说明，先在全书原文里查清每篇文献被怎样使用，再写出 JSON 文件并运行自查。最后只回复一行 JSON。";
/// The prompt of a later message of a writer conversation (`then`).
const NEXT_PROMPT: &str = "继续：按第一条消息的说明处理本条任务文件里的文献，先在全书原文里查清每篇被怎样使用，再写出本条指定的 JSON 文件并运行它的自查。最后只回复一行 JSON。";
/// Works of one job, and one job is one message of a writer conversation. The owner's order of 2026-11-08
/// (docs/digest-job-messages-proposal.md): 6 works a message, 4 messages a conversation, so one DeepSeek
/// subagent writes 24 works. A turn thinks only about what its user message gave it.
const DEFAULT_PER_JOB: usize = 6;
/// The writer jobs one conversation holds, one message each.
const MESSAGES_PER_CONVERSATION: usize = 4;
/// The most locations one Cited Card lists.
const MAX_LOCATIONS: usize = 12;
const RELATIONS: &[&str] = zt_check::cited::RELATIONS;

struct Params {
    step: String,
    staging: String,
    per_job: usize,
    tokens: String,
    draft: String,
    token: String,
    apply: bool,
}

/// What every step reads: the book, its source lines and their headings, the Cited plan.
struct Book {
    key: String,
    title: String,
    source_path: String,
    lines: Vec<(usize, String, bool)>,
    headings: Vec<(Option<String>, Option<String>)>,
    cited: Value,
    work: String,
    /// The section anchors that exist in the box, when apply has checked them: a reorganized tree moves its
    /// section Cards, and the manifest's anchors then name Cards that are gone (acemoglu2007equilibrium/10,
    /// live backfill 2026-11-07). `None` keeps every anchor (plan and precheck write no Card).
    live: std::cell::RefCell<Option<std::collections::HashSet<String>>>,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Staging", "PerJob", "Tokens", "Draft", "Token"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["Apply"])?;
    let per_job = a.int_or("PerJob", DEFAULT_PER_JOB as i64)?;
    if !(1..=12).contains(&per_job) {
        bail!("-PerJob is 1 to 12");
    }
    let p = Params {
        step: a.choice("Step", &["plan", "jobs", "precheck", "check", "apply"])?,
        staging: common::mandatory(&a, "Staging")?,
        per_job: per_job as usize,
        tokens: a.str("Tokens"),
        draft: a.str("Draft"),
        token: a.str("Token"),
        apply: a.flag("Apply"),
    };
    let book = read_book(&p.staging)?;
    match p.step.as_str() {
        "jobs" => jobs_step(&p, &book),
        "precheck" => {
            let mut zt = crate::zt::start()?;
            precheck_step(&p, &book, &mut zt)
        }
        "check" => {
            let mut zt = crate::zt::start()?;
            check_step(&book, &mut zt)
        }
        step => {
            let mut zt = crate::zt::start()?;
            if step == "plan" || p.apply {
                if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "cited-cards", step, &book.key)? {
                    return crate::lockcheck::stop("cited-cards", &refusal);
                }
            }
            if step == "plan" { plan_step(&p, &book, &mut zt) } else { apply_step(&p, &book, &mut zt) }
        }
    }
}

fn read_book(staging: &str) -> Result<Book> {
    let manifest = common::manifest(staging)?;
    let key = pstr(&manifest, "citation_key");
    let mut cited = util::read_json(ps::join(&ps::join(staging, "cited"), "plan.json"))
        .context("no Cited plan; run `zt-digest cited -Step plan` first")?;
    // The Cited apply's merged rows carry what the normalize jobs split; they replace the plan's rows.
    if let Ok(merged) = util::read_json(ps::join(&ps::join(staging, "cited"), "merged.json")) {
        if merged["plan_hash"] == cited["plan_hash"] && merged["rows"].is_array() {
            cited["rows"] = merged["rows"].clone();
        }
    }
    // The file the Cited plan read (its -Source, or the manifest's source): the line numbers are its.
    let source_path = match pstr(&cited, "source") {
        frozen if !frozen.is_empty() => frozen,
        _ => pstr(&manifest, "source"),
    };
    let bytes = std::fs::read(&source_path).with_context(|| format!("failed to read the source {source_path}"))?;
    let source = String::from_utf8_lossy(&bytes).trim_start_matches('\u{FEFF}').to_string();
    let lines = cited::reference_lines(&source, &pstr(&cited, "heading"));
    let headings = cited::line_headings(&lines, &manifest["groups"]);
    let work = ps::join(staging, "cited-cards");
    std::fs::create_dir_all(ps::join(&work, "drafts"))?;
    Ok(Book { key, title: pstr(&manifest, "book_title"), source_path, lines, headings, cited, work, live: std::cell::RefCell::new(None) })
}

fn plan_path(book: &Book) -> String {
    ps::join(&book.work, "plan.json")
}

fn draft_path(book: &Book, job: &str) -> String {
    ps::join(&ps::join(&book.work, "drafts"), &format!("{job}.json"))
}

fn read_plan(book: &Book) -> Result<Value> {
    let plan = util::read_json(plan_path(book)).context("no Cited Card plan; run -Step plan first")?;
    if plan["kind"] != "cited_cards_plan" {
        bail!("cited-cards\\plan.json is not a Cited Card plan");
    }
    Ok(plan)
}

/// The Cited plan's row `n`.
fn cited_row(book: &Book, n: u64) -> Option<&Value> {
    book.cited["rows"].as_array().and_then(|rows| rows.iter().find(|row| row["row"].as_u64() == Some(n)))
}

/// The book's Term list body (for R6 and the writer's spellings), if it has one.
fn term_body(zt: &mut Zt, key: &str) -> Option<String> {
    let reply = zt.request("get_terms", json!({ "at": key })).ok()?;
    if reply["result"]["isError"] == true {
        return None;
    }
    let text = reply["result"]["content"][0]["text"].as_str()?;
    let terms: Value = serde_json::from_str(text).ok()?;
    let list = terms["list"].as_str()?;
    zt.call("get_card", json!({ "at": list })).ok().map(|card| pstr(&card, "body"))
}

// --- plan ----------------------------------------------------------------------------------------

fn plan_step(p: &Params, book: &Book, zt: &mut Zt) -> Result<i32> {
    let list = cited::existing_list(zt, &book.key)?
        .context("the book has no Cited list; run the cited stage first")?;
    let rows: Vec<Value> = list["rows"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|row| {
            let n = row["row"].as_u64().unwrap_or(0);
            let planned = cited_row(book, n).cloned().unwrap_or(Value::Null);
            json!({
                "row": n,
                "entry": row["entry"],
                "id": planned["id"],
                "surname": planned["surname"],
                "year": planned["year"],
                "title": planned["title"],
                "box": planned["box"],
            })
        })
        .collect();
    if rows.is_empty() {
        bail!("the Cited list has no row");
    }
    let list_address = pstr(&list, "list");
    let terms = term_body(zt, &book.key);
    // The same list keeps its jobs and drafts. A changed list (a new reading of the reference material) moves
    // the old drafts aside, carries every live Cited Card whose work the new list still holds, and gives only
    // the other rows to writers (2026-11-07: the agentic Cited list replaced the parser on every finished book).
    let same = util::read_json(plan_path(book))
        .ok()
        .is_some_and(|old| pstr(&old, "list") == list_address && old["rows"].as_array().map(Vec::as_slice) == Some(rows.as_slice()));
    let (jobs, carried) = if same {
        let old = util::read_json(plan_path(book))?;
        (job_list(p, book, &rows, &list_address), old["carried"].as_array().cloned().unwrap_or_default())
    } else {
        carry(p, book, zt, &list, &rows, terms.as_deref())?
    };
    let plan_hash = crate::canon::hash_value(&json!({ "list": list_address, "rows": rows, "jobs": jobs }));
    let old = util::read_json(plan_path(book)).ok();
    let revision = match &old {
        Some(old) if old["plan_hash"] == json!(plan_hash) => old["revision"].as_i64().unwrap_or(1),
        Some(old) => old["revision"].as_i64().unwrap_or(0) + 1,
        None => 1,
    };
    let plan = json!({
        "schema_version": 1,
        "kind": "cited_cards_plan",
        "book": book.key,
        "list": list_address,
        "revision": revision,
        "plan_hash": plan_hash,
        "per_job": p.per_job,
        "jobs": jobs,
        "carried": carried,
        "rows": rows,
    });
    util::write_atomic(plan_path(book), &ps::json_pretty(&plan))?;

    let doc = util::read_text(ps::join(&common::scripts_dir(), INSTRUCTION))?.replace("{{KEY}}", &book.key);
    util::write_atomic(ps::join(&book.work, INSTRUCTION), &doc)?;
    std::fs::create_dir_all(ps::join(&book.work, "jobs"))?;
    for job in &jobs {
        let id = pstr(job, "id");
        util::write_atomic(
            ps::join(&ps::join(&book.work, "jobs"), &format!("{id}.md")),
            &job_file(p, book, &plan, job, terms.as_deref(), false),
        )?;
        util::write_atomic(next_file(book, &id), &job_file(p, book, &plan, job, terms.as_deref(), true))?;
    }
    if std::path::Path::new(&ps::join(&p.staging, "ledger.sqlite3")).is_file() {
        let prepared = crate::ledger::Prepared::read(&p.staging, zt)?;
        crate::ledger::cited_cards_register(&prepared, &plan)?;
    }
    util::print_line(&util::compact(&json!({ "ok": true, "jobs": jobs.len(), "carried": carried.len(), "revision": revision, "rows": rows.len() })))?;
    Ok(0)
}

/// A published Cited Card read back into its draft item (the inverse of [`render`]): its entry and the item,
/// or `None` when the body is not a rendered Cited Card.
fn parse_card(title: &str, body: &str) -> Option<(String, Value)> {
    let entry = body.lines().find_map(|l| l.strip_prefix("文献："))?.trim().to_string();
    let relations: Vec<String> = body
        .lines()
        .find_map(|l| l.strip_prefix("关系："))
        .map(|r| r.split('、').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();
    let section = |name: &str| -> Option<String> {
        let start = body.find(&format!("## {name}\n"))? + name.len() + 4;
        let rest = &body[start..];
        let end = ["\n## ", "\n本库中的这篇文献：", "\n来源：[["].iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
        Some(rest[..end].trim().to_string())
    };
    let why = section("为什么引用")?;
    let role = section("在本书研究中的作用")?;
    let relationship = section("本书与该文献的关系")?;
    static LOCATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^- .*?（第 (\d+) 行）：(.*?)(?: 另见 \[\[([^\]]+)\]\])?$").unwrap());
    let locations: Vec<Value> = section("引用处")
        .unwrap_or_default()
        .lines()
        .filter_map(|l| LOCATION.captures(l.trim()))
        .map(|c| {
            let mut l = json!({ "line": c[1].parse::<u64>().unwrap_or(0), "use": c[2].trim() });
            if let Some(see) = c.get(3) {
                l["see"] = json!(see.as_str());
            }
            l
        })
        .collect();
    let short = title.split_once('：').map(|(_, s)| s.trim().to_string())?;
    Some((entry, json!({ "short_title": short, "why": why, "role": role, "relationship": relationship, "relations": relations, "locations": locations })))
}

/// A changed list: move the old drafts aside, read the live Cited Cards back, and carry each one whose entry
/// the new list holds (same letters and digits) and whose item passes the checks against its new row. Carried
/// rows fill their own jobs, whose drafts are written now; the other rows go to writer jobs after them.
/// Returns the jobs and the carried job ids.
fn carry(p: &Params, book: &Book, zt: &mut Zt, list: &Value, rows: &[Value], terms: Option<&str>) -> Result<(Vec<Value>, Vec<Value>)> {
    let drafts = ps::join(&book.work, "drafts");
    // The drafts of the previous plan are carried too, by the entry of the row they were written for: a list
    // read again after its writers ran (morony2005iraq, 2026-11-07: authors filled in, 1,549 drafts ready)
    // keeps that work. They come before the live Cards, being newer.
    let mut old: HashMap<String, Value> = HashMap::new();
    if let Ok(previous) = util::read_json(plan_path(book)) {
        let entry_of: HashMap<u64, String> = previous["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|r| r["row"].as_u64().map(|n| (n, pstr(r, "entry"))))
            .collect();
        for job in previous["jobs"].as_array().into_iter().flatten() {
            for item in read_items(&draft_path(book, &pstr(job, "id"))) {
                if let Some(entry) = item["row"].as_u64().and_then(|n| entry_of.get(&n)) {
                    old.entry(cited::fold(entry)).or_insert(item);
                }
            }
        }
    }
    if std::path::Path::new(&drafts).is_dir() && std::fs::read_dir(&drafts)?.next().is_some() {
        let mut n = 1;
        while std::path::Path::new(&ps::join(&book.work, &format!("drafts-old-{n}"))).exists() {
            n += 1;
        }
        std::fs::rename(&drafts, ps::join(&book.work, &format!("drafts-old-{n}")))?;
    }
    std::fs::create_dir_all(&drafts)?;
    let addresses: Vec<String> = list["cards"].as_array().into_iter().flatten().map(|c| pstr(c, "address")).collect();
    for chunk in addresses.chunks(100) {
        let reply = zt.call("get_cards", json!({ "addresses": chunk }))?;
        for card in reply["cards"].as_array().into_iter().flatten() {
            if let Some((entry, item)) = parse_card(&pstr(card, "title"), &pstr(card, "body")) {
                old.entry(cited::fold(&entry)).or_insert(item);
            }
        }
    }
    let provisional = json!({ "rows": rows });
    let mut carried_items: Vec<Value> = Vec::new();
    let mut open: Vec<Value> = Vec::new();
    for row in rows {
        let mut item = old.remove(&cited::fold(&pstr(row, "entry"))).unwrap_or(Value::Null);
        if item.is_object() {
            item["row"] = row["row"].clone();
            if item_faults(book, &provisional, &item, terms).iter().all(|(_, blocking)| !blocking) {
                carried_items.push(item);
                continue;
            }
        }
        open.push(row.clone());
    }
    let mut jobs = Vec::new();
    let mut carried = Vec::new();
    for chunk in carried_items.chunks(p.per_job) {
        let id = format!("t{:03}", jobs.len() + 1);
        util::write_atomic(draft_path(book, &id), &ps::json_pretty(&json!(chunk)))?;
        jobs.push(json!({ "id": id, "rows": chunk.iter().map(|i| i["row"].clone()).collect::<Vec<_>>() }));
        carried.push(json!(id));
    }
    for chunk in open.chunks(p.per_job) {
        let id = format!("t{:03}", jobs.len() + 1);
        jobs.push(json!({ "id": id, "rows": chunk.iter().map(|r| r["row"].clone()).collect::<Vec<_>>() }));
    }
    Ok((jobs, carried))
}

/// The writer jobs of a plan. The same list at the same size keeps its jobs. At a new size a job with a
/// draft on disk keeps its rows (morony2005iraq, 2026-11-06: the size doubled at job 145 of 396), and the
/// other rows are grouped at the new size under ids no job used, so no written draft is lost.
fn job_list(p: &Params, book: &Book, rows: &[Value], list_address: &str) -> Vec<Value> {
    let group = |rows: &[Value], first: usize| -> Vec<Value> {
        rows.chunks(p.per_job)
            .enumerate()
            .map(|(n, chunk)| json!({ "id": format!("t{:03}", first + n), "rows": chunk.iter().map(|r| r["row"].clone()).collect::<Vec<_>>() }))
            .collect()
    };
    let old = util::read_json(plan_path(book)).ok().filter(|old| pstr(old, "list") == list_address && old["rows"].as_array().map(Vec::as_slice) == Some(rows));
    let Some(old) = old else { return group(rows, 1) };
    let old_jobs = old["jobs"].as_array().cloned().unwrap_or_default();
    if old["per_job"].as_u64() == Some(p.per_job as u64) {
        return old_jobs;
    }
    let (written, open): (Vec<Value>, Vec<Value>) = old_jobs.iter().cloned().partition(|job| std::path::Path::new(&draft_path(book, &pstr(job, "id"))).is_file());
    let number = |job: &Value| pstr(job, "id").trim_start_matches('t').parse::<usize>().unwrap_or(0);
    let last = old_jobs.iter().map(number).max().unwrap_or(0);
    let open_rows: std::collections::HashSet<u64> = open.iter().flat_map(|job| job["rows"].as_array().cloned().unwrap_or_default()).filter_map(|n| n.as_u64()).collect();
    let rest: Vec<Value> = rows.iter().filter(|row| row["row"].as_u64().is_some_and(|n| open_rows.contains(&n))).cloned().collect();
    written.into_iter().chain(group(&rest, last + 1)).collect()
}

/// The follow-up text of a writer job: the job as a later message of a conversation.
fn next_file(book: &Book, id: &str) -> String {
    ps::join(&ps::join(&book.work, "jobs"), &format!("{id}.next.md"))
}

/// One writer job's file: the works, the whole source, the candidate lines, the Term spellings. A
/// follow-up (a later message of a writer conversation) leaves out the Term spellings and the relation
/// words, which the conversation's first message already holds.
fn job_file(p: &Params, book: &Book, plan: &Value, job: &Value, terms: Option<&str>, follow_up: bool) -> String {
    let id = pstr(job, "id");
    let draft = draft_path(book, &id);
    let mut text = format!(
        "# 文献引用卡任务 {id}\n\n本书：《{}》（文献卡 [[{}]]）\n参考文献表卡：[[{}]]\n全书原文：{}（中英对照的 Markdown；行号从第 1 行起数）\n输出文件：{draft}\n\n",
        book.title, book.key, pstr(plan, "list"), book.source_path
    );
    if follow_up {
        text.push_str("这是同一次对话里的下一条消息。共享说明、本书的术语和关系词都在第一条消息里，照第一条消息的做法写。只写本条列出的文献，要写得和只有这几篇文献时一样细，不因为前面写过别的文献而压缩。\n\n## 文献\n\n");
    }
    if let Some(body) = terms.filter(|_| !follow_up) {
        text.push_str("## 本书的术语（写左列，不用右列的词）\n\n");
        for row in zt_check::text::term_table(body) {
            text.push_str(&format!("- {}：不用 {}\n", row.term, if row.avoid.is_empty() { "—".to_string() } else { row.avoid.join("、") }));
        }
        text.push('\n');
    }
    if !follow_up {
        text.push_str(&format!("## 关系词\n\n{}\n\n## 文献\n\n", RELATIONS.join("、")));
    }
    for n in job["rows"].as_array().into_iter().flatten() {
        let n = n.as_u64().unwrap_or(0);
        let row = plan["rows"].as_array().and_then(|rows| rows.iter().find(|r| r["row"].as_u64() == Some(n))).cloned().unwrap_or(Value::Null);
        text.push_str(&format!("### 第 {n} 条（{}）\n\n文献：{}\n", pstr(&row, "id"), pstr(&row, "entry")));
        text.push_str(&format!("第一作者的姓：{}；年份：{}；题名：{}\n", pstr(&row, "surname"), pstr(&row, "year"), pstr(&row, "title")));
        if let Some(key) = row["box"].as_str() {
            text.push_str(&format!("本库中已有这篇文献：[[{key}]]\n"));
        }
        let candidates = cited_row(book, n).and_then(|r| r["candidates"].as_array().cloned()).unwrap_or_default();
        if candidates.is_empty() {
            text.push_str("脚本没有找到候选行。请自己在全书原文里搜索。\n\n");
        } else {
            text.push_str("脚本找到的候选行（只是起点，不是全部）：\n");
            for c in candidates {
                text.push_str(&format!("- 第 {} 行（{}）：{}\n", c["line"], pstr(&c, "heading"), pstr(&c, "text")));
            }
            text.push('\n');
        }
    }
    let first = job["rows"][0].as_u64().unwrap_or(1);
    text.push_str(&format!(
        "## 输出样例\n\n`[{{\"row\":{first},\"short_title\":\"…\",\"why\":\"…\",\"role\":\"…\",\"relationship\":\"…\",\"relations\":[\"依赖\"],\"locations\":[{{\"line\":12,\"use\":\"…\"}}]}}]`\n\nSELF-CHECK: zt-digest cited-cards -Step precheck -Staging \"{}\" -Draft \"{draft}\"\n",
        p.staging
    ));
    text
}

// --- jobs ----------------------------------------------------------------------------------------

fn jobs_step(p: &Params, book: &Book) -> Result<i32> {
    let plan = read_plan(book)?;
    let wanted: Vec<String> = p
        .tokens
        .split(',')
        .filter_map(|pair| pair.split(':').next())
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect();
    let jobs: Vec<Value> = plan["jobs"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|job| wanted.is_empty() || wanted.contains(&pstr(job, "id")))
        // A carried job holds the live Cards read back at plan time: nothing for a writer.
        .filter(|job| !plan["carried"].as_array().is_some_and(|c| c.contains(&job["id"])))
        .map(|job| {
            let id = pstr(job, "id");
            let next = next_file(book, &id);
            if !std::path::Path::new(&next).is_file() {
                bail!("the follow-up file `{next}` is missing; rerun -Step plan");
            }
            Ok(json!({
                "id": id,
                "files": [ps::join(&book.work, INSTRUCTION), ps::join(&ps::join(&book.work, "jobs"), &format!("{id}.md"))],
                "prompt": PROMPT,
                "skills": ["asd-ste100-zh"],
                "model": cited::MODEL,
                "effort": "high",
                "answer": draft_path(book, &id),
                "next_file": next,
            }))
        })
        .collect::<Result<Vec<_>>>()?;
    let jobs = conversations(jobs);
    util::print_line(&util::compact(&json!({ "ok": true, "jobs": jobs })))?;
    Ok(0)
}

/// The writer jobs as conversations (2026-11-08, docs/digest-job-messages-proposal.md): the jobs in their
/// order, `MESSAGES_PER_CONVERSATION` to a conversation. The first job is the entry with its full files; each
/// later job is an item of its `then` with only its follow-up text, its own draft and SELF-CHECK. Each job keeps
/// its own ledger unit, so a message is checked and completed like a job.
fn conversations(jobs: Vec<Value>) -> Vec<Value> {
    jobs.chunks(MESSAGES_PER_CONVERSATION)
        .map(|chunk| {
            let mut first = chunk[0].clone();
            first.as_object_mut().map(|map| map.remove("next_file"));
            if chunk.len() > 1 {
                first["then"] = json!(chunk[1..]
                    .iter()
                    .map(|job| json!({ "id": job["id"], "files": [job["next_file"]], "prompt": NEXT_PROMPT, "answer": job["answer"] }))
                    .collect::<Vec<_>>());
            }
            first
        })
        .collect()
}

// --- render and check ----------------------------------------------------------------------------

/// One location of a draft, resolved against the source.
struct Location {
    line: usize,
    use_text: String,
    heading: Option<String>,
    anchor: Option<String>,
}

fn locations(book: &Book, item: &Value) -> Vec<Location> {
    let mut found: Vec<Location> = item["locations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|l| {
            let line = l["line"].as_u64()? as usize;
            let (heading, anchor) = book.headings.get(line.checked_sub(1)?).cloned().unwrap_or((None, None));
            // A carried Card keeps the anchor it already names (the box moved it with the section); a new one takes
            // the manifest's anchor only while that Card exists.
            let anchor = match l["see"].as_str().filter(|a| !a.is_empty()) {
                Some(see) => Some(see.to_string()),
                None => anchor.filter(|a| book.live.borrow().as_ref().is_none_or(|live| live.contains(a))),
            };
            Some(Location { line, use_text: pstr(l, "use"), heading, anchor })
        })
        .collect();
    found.sort_by_key(|l| l.line);
    found.dedup_by_key(|l| l.line);
    found
}

/// `1 引言、2 模型（3 处）`, or `仅参考文献` when the book never cites the work in its text.
fn cited_at(found: &[Location]) -> String {
    if found.is_empty() {
        return "仅参考文献".into();
    }
    let mut headings: Vec<String> = Vec::new();
    for l in found {
        let h = l.heading.clone().unwrap_or_else(|| "正文".into());
        if !headings.contains(&h) {
            headings.push(h);
        }
    }
    format!("{}（{} 处）", headings.join("、"), found.len())
}

fn relations(item: &Value) -> Vec<String> {
    item["relations"].as_array().into_iter().flatten().filter_map(|r| r.as_str().map(|s| s.trim().to_string())).collect()
}

/// A Cited Card's title and body from one draft item and its plan row.
fn render(book: &Book, row: &Value, item: &Value) -> (String, String) {
    let found = locations(book, item);
    let surname = pstr(row, "surname");
    let year = pstr(row, "year");
    let short = pstr(item, "short_title");
    let head = if surname.is_empty() {
        pstr(row, "entry").chars().take(20).collect::<String>()
    } else if year.is_empty() {
        surname
    } else {
        format!("{surname}（{year}）")
    };
    let title = format!("{head}：{}", short.trim());
    let summary = cited_at(&found);
    let at_line = if found.is_empty() { summary.clone() } else { summary.replacen("（", "（共 ", 1) };
    let mut body = format!(
        "文献：{}\n引用处：{at_line}\n关系：{}\n\n## 为什么引用\n\n{}\n\n## 在本书研究中的作用\n\n{}\n\n## 本书与该文献的关系\n\n{}\n",
        pstr(row, "entry"),
        relations(item).join("、"),
        pstr(item, "why").trim(),
        pstr(item, "role").trim(),
        pstr(item, "relationship").trim(),
    );
    if !found.is_empty() {
        body.push_str("\n## 引用处\n\n");
        for l in &found {
            let heading = l.heading.clone().unwrap_or_else(|| "正文".into());
            let see = l.anchor.as_ref().map(|a| format!(" 另见 [[{a}]]")).unwrap_or_default();
            body.push_str(&format!("- {heading}（第 {} 行）：{}{see}\n", l.line, l.use_text.trim()));
        }
    }
    if let Some(key) = row["box"].as_str() {
        body.push_str(&format!("\n本库中的这篇文献：[[{key}]]\n"));
    }
    body.push_str(&format!("\n来源：[[{}]] 参考文献第 {} 条", book.key, row["row"]));
    (title, body)
}

/// Whether an R6 hit (`uses `X` for `Y`: <line>`) stands only in a section title the tool rendered: the
/// `引用处：` line is all titles and counts, and a location line is `<title>（第 N 行）：<the writer's use>`.
/// The book's own titles are never reworded (atkinson2015lectures: `公司税的影响`, `资本课税的归宿`, where the
/// Term list avoids 公司税 and 资本课税), so only the writer's text counts.
fn only_in_section_title(message: &str) -> bool {
    let Some(word) = message.split('`').nth(1) else {
        return false;
    };
    let Some((_, line)) = message.split_once("`: ") else {
        return false;
    };
    if line.starts_with("引用处：") {
        return true;
    }
    match line.split_once("行）：") {
        Some((_, written)) => !written.contains(word),
        None => false,
    }
}

/// The faults of one draft item: C3 against the source, then the zt-check rules on the render.
fn item_faults(book: &Book, plan: &Value, item: &Value, terms: Option<&str>) -> Vec<(String, bool)> {
    let n = item["row"].as_u64().unwrap_or(0);
    let tag = format!("row{n}");
    let Some(row) = plan["rows"].as_array().and_then(|rows| rows.iter().find(|r| r["row"].as_u64() == Some(n))) else {
        return vec![(format!("{tag} C0 no row {n} in the Cited Card plan"), true)];
    };
    let mut faults = Vec::new();
    if pstr(item, "short_title").trim().is_empty() {
        faults.push((format!("{tag} C1 the short_title is empty"), true));
    }
    // Names are compared folded, so `Jaszi` in the text names `Jászi` in the list (live backfill, 2026-10-31).
    let surname = cited::fold(&pstr(row, "surname"));
    // An Arabic surname listed with its article is cited without it (`al-Sharaf` in the list, `Sharaf` in the
    // notes; scheiner2022baghdad, live backfill 2026-11-07).
    let bare = {
        let raw = pstr(row, "surname");
        let lower = raw.to_lowercase();
        ["al-", "el-", "ad-", "an-", "ar-", "as-", "at-", "az-", "ash-", "al ", "el "]
            .iter()
            .find_map(|article| lower.starts_with(article).then(|| cited::fold(&raw[article.len()..])))
            .filter(|b| b.len() >= 4)
    };
    let words: Vec<String> = pstr(row, "title")
        .split(|c: char| !c.is_alphanumeric())
        .map(cited::fold)
        // A number of three digits counts as a title word: an anonymous work is named by it (`Chronicle of 741`,
        // hoyland2015path, live backfill 2026-11-07).
        .filter(|w| w.len() >= 4 || (w.len() == 3 && w.chars().all(|c| c.is_ascii_digit())))
        .collect();
    let year: String = pstr(row, "year").chars().take(4).collect();
    // A surname written without a comma before the initials (`Salter W. E. G. 1966.`, the JPE list of
    // acemoglu2010labor) parses whole; its first word of four letters or more names the work too.
    let first_name = pstr(row, "surname")
        .split_whitespace()
        .next()
        .map(cited::fold)
        .filter(|name| name.len() >= 4 && *name != surname);
    // An Arabic name of three words or more that opens on Ibn or Abū is cited by its first two words
    // (`Ibn Yūnus al-Miṣrī` in the list, `Ibn Yūnus, Ta'rīkh` in the notes; sijpesteijn2013shaping, 2026-11-08).
    let lead = {
        let parts: Vec<String> = pstr(row, "surname").split_whitespace().map(cited::fold).collect();
        (parts.len() >= 3 && ["ibn", "abu", "umm", "bint"].contains(&parts[0].as_str()) && parts[1].len() >= 3)
            .then(|| format!("{}{}", parts[0], parts[1]))
    };
    let raw = item["locations"].as_array().cloned().unwrap_or_default();
    if raw.len() > MAX_LOCATIONS {
        faults.push((format!("{tag} C3 {} locations; list at most {MAX_LOCATIONS}", raw.len()), true));
    }
    for l in &raw {
        let line = l["line"].as_u64().unwrap_or(0) as usize;
        if line == 0 || line > book.lines.len() {
            faults.push((format!("{tag} C3 line {line} is not a line of the source"), true));
            continue;
        }
        if book.lines[line - 1].2 {
            faults.push((format!("{tag} C3 line {line} is the reference list itself, not a use"), true));
            continue;
        }
        let near: Vec<String> = (line.saturating_sub(1)..=line + 1)
            .filter_map(|i| i.checked_sub(1).and_then(|i| book.lines.get(i)))
            .map(|(_, text, _)| cited::fold(text))
            .collect();
        let near = near.join(" ");
        // `fold` drops spaces, so the words of the nearby lines are split before folding.
        let near_words: Vec<String> = (line.saturating_sub(1)..=line + 1)
            .filter_map(|i| i.checked_sub(1).and_then(|i| book.lines.get(i)))
            .flat_map(|(_, text, _)| text.split(|c: char| !c.is_alphanumeric()).map(cited::fold).collect::<Vec<_>>())
            .filter(|w| w.len() >= 6)
            .collect();
        let alias = pstr(l, "as");
        if !alias.is_empty() {
            // A work cited by its authors' initials (`AJR (2001, 2012)`): the line itself holds the initials
            // and the year, and the initials start like the surname.
            let own = &book.lines[line - 1].1;
            let shaped = (2..=8).contains(&alias.chars().count()) && alias.chars().all(|c| c.is_ascii_uppercase());
            let starts = cited::fold(&alias).chars().next() == surname.chars().next();
            if !(shaped && starts && own.contains(&alias) && (year.is_empty() || own.contains(&year))) {
                faults.push((format!("{tag} C3 line {line} does not hold the initials `{alias}` and the year {year} of the work"), true));
            }
            continue;
        }
        // The book may spell a name one letter apart from its list (`Boycko` in the text, `Boyco` in the list
        // of hart1995firms): that counts when the line itself gives the work's year.
        let own_line = &book.lines[line - 1].1;
        let near_spelling = surname.len() >= 5
            && !year.is_empty()
            && own_line.contains(&year)
            && own_line
                .split(|c: char| !c.is_alphanumeric())
                .map(cited::fold)
                .any(|word| one_edit(&word, &surname));
        let names = (!surname.is_empty() && cited::squeeze(&near).contains(&cited::squeeze(&surname)))
            || bare.as_ref().is_some_and(|b| cited::squeeze(&near).contains(&cited::squeeze(b)))
            || near_spelling
            || first_name.as_ref().is_some_and(|name| near.contains(name.as_str()))
            || lead.as_ref().is_some_and(|l| cited::squeeze(&near).contains(&cited::squeeze(l)))
            // A work without a surname whose title has one long word (an author index's `Kharāj`, morony2005iraq,
            // live backfill 2026-11-07) is named by that word alone. A title word of seven letters or more also
            // counts when the line spells it one letter apart (`Eutychios` in the list, `Eutychius` in the notes;
            // sijpesteijn2013shaping, 2026-11-08).
            || words
                .iter()
                .filter(|w| near.contains(w.as_str()) || (w.chars().count() >= 7 && near_words.iter().any(|x| one_edit(x, w.as_str()))))
                .count()
                >= if surname.is_empty() { words.len().clamp(1, 2) } else { 2 };
        if !names {
            faults.push((format!("{tag} C3 line {line} does not name the work (its surname or two title words)"), true));
        }
    }
    if raw.is_empty() && relations(item) != ["背景"] {
        faults.push((format!("{tag} C3 a work with no in-text location takes only 背景"), true));
    }
    let (title, body) = render(book, row, item);
    let entry = json!({
        "address": format!("{}|{}", pstr(plan, "list"), n),
        "type": "cited",
        "category": "cited",
        "tree": book.key,
        "book": book.key,
        "title": title,
        "body": body,
        "cited_entry": pstr(row, "entry"),
    });
    let report = zt_check::core::Check::new(zt_check::core::Limits::default(), false, "")
        .with_terms(terms)
        .report(&[entry]);
    for d in report {
        // The `文献：` line quotes the reference verbatim; it is never reworded, so R6 does not count there
        // (ebrill2001modern: `CVAT, VIVAT, and All That`, where the Term list avoids CVAT).
        if d.rule == "R6" && (d.message.contains(": 文献：") || cited::quotes_the_reference(&d.message, &pstr(row, "entry")) || only_in_section_title(&d.message)) {
            continue;
        }
        // R6 counts here: a Cited Card keeps the book's Term spellings.
        let blocking = d.blocking() || d.rule == "R6";
        let message = format!("{tag} {} {}{}", d.rule, if blocking { "" } else { "(advisory) " }, d.message);
        faults.push((message, blocking));
    }
    faults
}

/// Whether two words differ by at most one inserted, removed or changed letter.
fn one_edit(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (&a, &b) } else { (&b, &a) };
    match long.len() - short.len() {
        0 => short.iter().zip(long.iter()).filter(|(x, y)| x != y).count() == 1,
        1 => {
            let at = short.iter().zip(long.iter()).position(|(x, y)| x != y).unwrap_or(short.len());
            short[at..] == long[at + 1..]
        }
        _ => false,
    }
}

fn read_items(path: &str) -> Vec<Value> {
    util::read_json(path).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default()
}

fn precheck_step(p: &Params, book: &Book, zt: &mut Zt) -> Result<i32> {
    if p.draft.is_empty() {
        bail!("-Draft is required for -Step precheck");
    }
    let plan = read_plan(book)?;
    let terms = term_body(zt, &book.key);
    let items = match util::read_json(&p.draft) {
        Ok(Value::Array(items)) => items,
        _ => {
            util::print_lines(&["draft C0 the draft is not a JSON array".into(), "faults: 1".into()])?;
            return Ok(0);
        }
    };
    let mut lines = Vec::new();
    let mut count = 0;
    for item in &items {
        for (line, blocking) in item_faults(book, &plan, item, terms.as_deref()) {
            count += usize::from(blocking);
            lines.push(line);
        }
    }
    lines.push(format!("faults: {count}"));
    util::print_lines(&lines)?;
    Ok(0)
}

/// Every plan row's readiness: its draft item and its blocking faults.
fn readiness(book: &Book, plan: &Value, terms: Option<&str>) -> (Vec<Value>, HashMap<u64, Value>) {
    let mut items: HashMap<u64, Value> = HashMap::new();
    let mut rows = Vec::new();
    for job in plan["jobs"].as_array().into_iter().flatten() {
        let id = pstr(job, "id");
        let drafted: HashMap<u64, Value> = read_items(&draft_path(book, &id))
            .into_iter()
            .filter_map(|item| item["row"].as_u64().map(|n| (n, item)))
            .collect();
        for n in job["rows"].as_array().into_iter().flatten().filter_map(Value::as_u64) {
            let (status, faults) = match drafted.get(&n) {
                None => ("missing", Vec::new()),
                Some(item) => {
                    let faults: Vec<String> = item_faults(book, plan, item, terms).into_iter().filter(|(_, b)| *b).map(|(l, _)| l).collect();
                    if faults.is_empty() {
                        items.insert(n, item.clone());
                        ("ready", faults)
                    } else {
                        ("faults", faults)
                    }
                }
            };
            rows.push(json!({ "row": n, "job": id, "status": status, "faults": faults }));
        }
    }
    rows.sort_by_key(|r| r["row"].as_u64());
    (rows, items)
}

fn check_step(book: &Book, zt: &mut Zt) -> Result<i32> {
    let plan = read_plan(book)?;
    let terms = term_body(zt, &book.key);
    let (rows, _) = readiness(book, &plan, terms.as_deref());
    let ready = rows.iter().filter(|r| r["status"] == "ready").count();
    let artifact = json!({
        "schema_version": 1,
        "kind": "cited_cards_readiness",
        "book": book.key,
        "plan_revision": plan["revision"],
        "plan_hash": plan["plan_hash"],
        "rows": rows,
    });
    let dir = ps::join(&book.work, "check");
    std::fs::create_dir_all(&dir)?;
    util::write_atomic(ps::join(&dir, "readiness.json"), &ps::json_pretty(&artifact))?;
    util::print_line(&util::compact(&json!({ "ok": true, "ready": ready, "not_ready": rows.len() - ready })))?;
    Ok(0)
}

// --- apply ---------------------------------------------------------------------------------------

/// The most items and authored bytes one identified call carries (the archive's own real-batch bound).
const BATCH_ITEMS: usize = 64;
const BATCH_BYTES: usize = 1024 * 1024;

/// Write Cards (edits or creates) in identified `apply_batch` calls of at most [`BATCH_ITEMS`] items and
/// [`BATCH_BYTES`] authored bytes, in row order, so each is appended after the last and nothing moves.
/// atkinson2015lectures (2026-11-02): 1,589 rows, and one `create_successor` call per Card cost about
/// 5 s of fixed archive time each. Answers each call's slots with the addresses it created; the
/// operation keys make a retry replay the archive's own receipts.
fn write_in_batches(zt: &mut Zt, cards: &[(usize, Value)]) -> Result<Vec<(Vec<usize>, Vec<String>)>> {
    let bytes = |item: &Value| pstr(&item["arguments"], "title").len() + pstr(&item["arguments"], "body").len();
    let mut calls: Vec<Vec<&(usize, Value)>> = Vec::new();
    let mut size = 0usize;
    for card in cards {
        let fits = calls
            .last()
            .is_some_and(|call| call.len() < BATCH_ITEMS && size + bytes(&card.1) <= BATCH_BYTES);
        if !fits {
            calls.push(Vec::new());
            size = 0;
        }
        size += bytes(&card.1);
        calls.last_mut().expect("a call").push(card);
    }
    let mut answered = Vec::new();
    for call in calls {
        let items: Vec<&Value> = call.iter().map(|(_, item)| item).collect();
        let reply = zt.request("apply_batch", json!({ "items": items }))?;
        if let Some(message) = zt_check::mcp::reply_error(&reply) {
            bail!("apply_batch refused the Cited Cards {}: {message}", pstr(items[0], "client_ref"));
        }
        let value: Value = serde_json::from_str(&zt_check::mcp::reply_text(&reply))
            .context("the apply_batch answer is not JSON")?;
        let results = value["results"].as_array().cloned().unwrap_or_default();
        if results.len() != items.len() {
            bail!("apply_batch answered {} of {} Cited Cards", results.len(), items.len());
        }
        let mut addresses = Vec::new();
        for (item, result) in items.iter().zip(&results) {
            let address = pstr(&result["result"], "address");
            if result["client_ref"] != item["client_ref"] || address.is_empty() {
                bail!("apply_batch answered another item than {}", pstr(item, "client_ref"));
            }
            addresses.push(address);
        }
        answered.push((call.iter().map(|(slot, _)| *slot).collect(), addresses));
    }
    Ok(answered)
}

fn apply_step(p: &Params, book: &Book, zt: &mut Zt) -> Result<i32> {
    let plan = read_plan(book)?;
    let hash = pstr(&plan, "plan_hash");
    let run = format!("cited-cards-{}", &hash[..16.min(hash.len())]);
    let terms = term_body(zt, &book.key);
    let (states, items) = readiness(book, &plan, terms.as_deref());
    let list = cited::existing_list(zt, &book.key)?.context("the book has no Cited list")?;
    let anchors: Vec<String> = book.headings.iter().filter_map(|(_, a)| a.clone()).collect::<std::collections::BTreeSet<_>>().into_iter().collect();
    let mut live = std::collections::HashSet::new();
    for chunk in anchors.chunks(200) {
        let reply = zt.call("get_cards", json!({ "addresses": chunk, "view": "text" }))?;
        live.extend(reply["cards"].as_array().into_iter().flatten().map(|c| pstr(c, "address")));
    }
    *book.live.borrow_mut() = Some(live);
    let list_address = pstr(&list, "list");
    if list_address != pstr(&plan, "list") {
        bail!("the Cited list moved from {} to {list_address}; rerun -Step plan", pstr(&plan, "list"));
    }
    let existing = list["cards"].as_array().cloned().unwrap_or_default();
    let plan_rows = plan["rows"].as_array().cloned().unwrap_or_default();
    let mut published = Vec::new();
    // The new Cards, gathered in row order and created in identified batches after the loop.
    let mut new_cards: Vec<(usize, Value)> = Vec::new();
    let mut cells: HashMap<u64, (String, String)> = HashMap::new();
    let mut stop: Option<String> = None;
    // The current text of every published Card, read in bulk: a backfill compares about 1,500 Cards, and a read
    // and an edit per Card took about half a second each (live backfill 2026-11-07).
    let mut current: HashMap<String, (String, String, String)> = HashMap::new();
    // Every Link into a Cited Card from outside the list, with the work its Card holds now: the Cards are written in
    // row order, so a row that moves takes another address, and the Link must follow its work (live backfill
    // 2026-11-07: removing repeats moved later rows).
    let mut inbound: Vec<(String, String, String)> = Vec::new();
    if p.apply {
        let addresses: Vec<String> = existing.iter().map(|c| pstr(c, "address")).collect();
        for chunk in addresses.chunks(200) {
            let reply = zt.call("get_cards", json!({ "addresses": chunk }))?;
            for card in reply["cards"].as_array().into_iter().flatten() {
                let address = pstr(card, "address");
                let entry = pstr(card, "body").lines().find_map(|l| l.strip_prefix("文献：").map(str::to_string)).unwrap_or_default();
                for source in card["inbound"].as_array().into_iter().flatten().map(|x| pstr(x, "address")) {
                    if !source.starts_with(&list_address) {
                        inbound.push((source, address.clone(), cited::fold(&entry)));
                    }
                }
                current.insert(address, (pstr(card, "title"), pstr(card, "body"), crate::calls::authored_fingerprint(card)));
            }
        }
    }
    let mut edits: Vec<(usize, Value)> = Vec::new();
    for (index, row) in plan_rows.iter().enumerate() {
        let n = row["row"].as_u64().unwrap_or(0);
        let Some(item) = items.get(&n) else {
            stop = Some(format!("row {n} is not ready"));
            break;
        };
        let (title, body) = render(book, row, item);
        let address = match existing.get(index) {
            Some(card) => {
                let address = pstr(card, "address");
                if card["names_row"].as_u64().is_some_and(|named| named != n) {
                    stop = Some(format!("the Card {address} names another row"));
                    break;
                }
                let base = current.get(&address).map(|(_, _, f)| f.clone()).unwrap_or_default();
                let changed = current.get(&address).is_none_or(|(t, b, _)| *t != title || b.trim_end() != body.trim_end());
                if p.apply && changed {
                    edits.push((
                        published.len(),
                        json!({
                            "op_key": format!("{run}/{}/{}/apply/e{n:04}", book.key, plan["revision"]),
                            "client_ref": format!("e{n:04}"),
                            "tool": "edit_card",
                            "arguments": { "at": address, "title": title, "body": body, "base_fingerprint": base },
                        }),
                    ));
                }
                address
            }
            None if p.apply => {
                new_cards.push((
                    published.len(),
                    json!({
                        "op_key": format!("{run}/{}/{}/apply/c{n:04}", book.key, plan["revision"]),
                        "client_ref": format!("c{n:04}"),
                        "tool": "create_successor",
                        "arguments": { "at": list_address, "kind": "side", "type": "cited", "title": title, "body": body },
                    }),
                ));
                String::new()
            }
            None => String::new(),
        };
        let found = locations(book, item);
        cells.insert(n, (cited_at(&found), relations(item).join("、")));
        published.push(json!({ "row": n, "card": address, "status": "committed" }));
    }
    // Edits first, in identified batches: they keep their addresses, so the new Cards still append after them.
    write_in_batches(zt, &edits)?;
    for (slots, created) in write_in_batches(zt, &new_cards)? {
        for (slot, address) in slots.into_iter().zip(created) {
            published[slot]["card"] = json!(address);
        }
    }
    // A complete publication owns every Cited Card of the list: one filed beyond the rows is stale (the list
    // got shorter after a parser fix; live backfill of grossman2002integration) and is removed.
    let mut removed = 0usize;
    let mut relinked = 0usize;
    if p.apply && stop.is_none() && published.len() == plan_rows.len() {
        // Each Link goes to the Card that now holds its work, before any stale Card is deleted.
        let home: HashMap<String, String> = plan_rows
            .iter()
            .zip(&published)
            .map(|(row, done)| (cited::fold(&pstr(row, "entry")), pstr(done, "card")))
            .collect();
        for (source, old, entry) in &inbound {
            match home.get(entry) {
                Some(target) if target != old && !target.is_empty() => {
                    zt.call("relink", json!({ "from": old, "to": target, "sources": [source] }))?;
                    relinked += 1;
                }
                _ => {}
            }
        }
        // From the last one back, in start-state batches of BATCH_ITEMS: deleting a Side successor renumbers only
        // the later ones, so each batch names addresses no earlier batch moved. A tree batch previews and commits
        // under its own plan hash (zt 0.51, PRD #228).
        let stale: Vec<String> = existing.iter().skip(plan_rows.len()).map(|c| pstr(c, "address")).collect();
        for chunk in stale.rchunks(BATCH_ITEMS) {
            let items: Vec<Value> = chunk
                .iter()
                .rev()
                .map(|a| json!({ "op_key": format!("{run}/{}/{}/delete/{}", book.key, plan["revision"], a.replace(['/', '|'], "-")), "client_ref": format!("d-{}", a.replace(['/', '|'], "-")), "tool": "delete_card", "arguments": { "at": a } }))
                .collect();
            let planned = zt.request("apply_batch", json!({ "preview": true, "items": items }))?;
            if let Some(message) = zt_check::mcp::reply_error(&planned) {
                bail!("apply_batch refused deleting {}: {message}", chunk[0]);
            }
            let plan: Value = serde_json::from_str(&zt_check::mcp::reply_text(&planned)).context("the apply_batch preview is not JSON")?;
            if plan["complete"] != true {
                bail!("deleting {} is blocked: {}", chunk[0], plan["refusals"]);
            }
            let done = zt.request("apply_batch", json!({ "items": items, "confirm": plan["plan_hash"] }))?;
            if let Some(message) = zt_check::mcp::reply_error(&done) {
                bail!("apply_batch did not delete {}: {message}", chunk[0]);
            }
            removed += chunk.len();
        }
    }
    // The list's 引用处 and 关系 cells follow the published Cards.
    if p.apply && !cells.is_empty() {
        let table: Vec<(String, String, String, String)> = list["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| {
                let n = row["row"].as_u64().unwrap_or(0);
                let (at, relation) = cells
                    .get(&n)
                    .cloned()
                    .unwrap_or_else(|| (pstr(row, "cited_at"), pstr(row, "relation")));
                (pstr(row, "entry"), pstr(row, "year"), at, relation)
            })
            .collect();
        let (title, body) = cited::list_card(&book.title, &book.key, &table);
        let current = zt.call("get_card", json!({ "at": list_address }))?;
        if pstr(&current, "title") != title || pstr(&current, "body").trim_end() != body.trim_end() {
            zt.call("edit_card", json!({ "at": list_address, "title": title, "body": body }))?;
        }
    }
    let complete = stop.is_none() && published.len() == plan_rows.len();
    let report = json!({
        "schema_version": 1,
        "kind": "cited_cards_report",
        "claim_token": p.token.parse::<i64>().ok(),
        "book": book.key,
        "plan_revision": plan["revision"],
        "plan_hash": plan["plan_hash"],
        "applied": p.apply,
        "complete": complete,
        "stop": stop,
        "rows": published,
        "removed": removed,
        "relinked": relinked,
        "readiness": states,
    });
    util::write_atomic(ps::join(&book.work, "report.json"), &ps::json_pretty(&report))?;
    let mut line = json!({ "ok": true, "rows": plan_rows.len(), "committed": published.len() });
    if complete {
        line["complete"] = json!(true);
    }
    if removed > 0 {
        line["removed"] = json!(removed);
    }
    if relinked > 0 {
        line["relinked"] = json!(relinked);
    }
    util::print_line(&util::compact(&line))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-11-08 (docs/digest-job-messages-proposal.md): nine writer jobs of six works are three conversations
    /// of four, four and one messages; a later message reads only its follow-up text and keeps its own draft.
    #[test]
    fn writer_jobs_run_as_conversations_of_four_messages() {
        assert_eq!(DEFAULT_PER_JOB, 6);
        let jobs: Vec<Value> = (1..=9)
            .map(|n| json!({ "id": format!("t{n:03}"), "files": ["I.md", format!("jobs\\t{n:03}.md")], "prompt": PROMPT, "answer": format!("drafts\\t{n:03}.json"), "next_file": format!("jobs\\t{n:03}.next.md") }))
            .collect();
        let entries = conversations(jobs);
        let shape: Vec<(String, usize)> = entries.iter().map(|e| (pstr(e, "id"), e["then"].as_array().map_or(0, Vec::len))).collect();
        assert_eq!(shape, vec![("t001".into(), 3), ("t005".into(), 3), ("t009".into(), 0)]);
        assert_eq!(entries[0]["files"], json!(["I.md", "jobs\\t001.md"]));
        assert_eq!(entries[0]["then"][2], json!({ "id": "t004", "files": ["jobs\\t004.next.md"], "prompt": NEXT_PROMPT, "answer": "drafts\\t004.json" }));
        assert!(entries.iter().all(|e| e.get("next_file").is_none()));
        assert!(entries[2].get("then").is_none(), "a lone job is one message");
    }

    /// Live backfill 2026-11-07: a reorganized tree moved its section Cards, so the manifest's anchor
    /// (`acemoglu2007equilibrium/10`) named no Card and the publication was refused. A carried Card keeps the
    /// anchor its own body names; the parse reads it back with the rest of the item.
    #[test]
    fn a_published_card_reads_back_into_its_item_with_its_anchors() {
        let body = "文献：Smith, J. (1999). Bargaining.\n引用处：1 引言（共 1 处）\n关系：依赖、对照\n\n## 为什么引用\n\n说明谈判。\n\n## 在本书研究中的作用\n\n给出起点。\n\n## 本书与该文献的关系\n\n依赖它。\n\n## 引用处\n\n- 1 引言（第 3 行）：本书用它说明谈判。 另见 [[tiny2026/7|a]]\n- 2 模型（第 9 行）：再次引用。\n\n来源：[[tiny2026]] 参考文献第 1 条";
        let (entry, item) = parse_card("Smith（1999）：谈判的作用", body).expect("a rendered Card");
        assert_eq!(entry, "Smith, J. (1999). Bargaining.");
        assert_eq!(item["short_title"], json!("谈判的作用"));
        assert_eq!(item["relations"], json!(["依赖", "对照"]));
        assert_eq!(item["relationship"], json!("依赖它。"));
        assert_eq!(item["locations"][0], json!({ "line": 3, "use": "本书用它说明谈判。", "see": "tiny2026/7|a" }));
        assert_eq!(item["locations"][1], json!({ "line": 9, "use": "再次引用。" }));
        assert!(parse_card("标题", "不是文献引用卡。").is_none());
    }
}
