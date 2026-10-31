//! `zt-digest cited-cards`: the Cited Cards of a digest (#220, docs/cited-literature-cards-proposal.md).
//! Every row of the book's Cited list gets one Cited Card. DeepSeek writes each work's content as
//! JSON after it reviews how the work is used in the WHOLE source (the digest's notes often leave
//! citations out); the tool renders, checks and publishes the Cards as the list's immediate Side
//! successors in row order.
//!
//!   zt-digest cited-cards -Step plan|jobs|precheck|check|apply -Staging <folder> [-PerJob 4]
//!                         [-Tokens <job:token,...>] [-Draft <file>] [-Token <n>] [-Apply]
//!
//!   plan      groups the list's rows into writer jobs (-PerJob, 1–8), freezes cited-cards\plan.json
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
use std::collections::HashMap;

const INSTRUCTION: &str = "dsh-cited-card-instructions.zh.md";
const PROMPT: &str = "按以上说明，先在全书原文里查清每篇文献被怎样使用，再写出 JSON 文件并运行自查。最后只回复一行 JSON。";
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
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Staging", "PerJob", "Tokens", "Draft", "Token"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["Apply"])?;
    let per_job = a.int_or("PerJob", 4)?;
    if !(1..=8).contains(&per_job) {
        bail!("-PerJob is 1 to 8");
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
    let lines = cited::reference_lines(&source, "");
    let headings = cited::line_headings(&lines, &manifest["groups"]);
    let work = ps::join(staging, "cited-cards");
    std::fs::create_dir_all(ps::join(&work, "drafts"))?;
    Ok(Book { key, title: pstr(&manifest, "book_title"), source_path, lines, headings, cited, work })
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
    let jobs: Vec<Value> = rows
        .chunks(p.per_job)
        .enumerate()
        .map(|(n, chunk)| json!({ "id": format!("t{:03}", n + 1), "rows": chunk.iter().map(|r| r["row"].clone()).collect::<Vec<_>>() }))
        .collect();
    let list_address = pstr(&list, "list");
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
        "rows": rows,
    });
    util::write_atomic(plan_path(book), &ps::json_pretty(&plan))?;

    let doc = util::read_text(ps::join(&common::scripts_dir(), INSTRUCTION))?.replace("{{KEY}}", &book.key);
    util::write_atomic(ps::join(&book.work, INSTRUCTION), &doc)?;
    let terms = term_body(zt, &book.key);
    std::fs::create_dir_all(ps::join(&book.work, "jobs"))?;
    for job in &jobs {
        let id = pstr(job, "id");
        util::write_atomic(
            ps::join(&ps::join(&book.work, "jobs"), &format!("{id}.md")),
            &job_file(p, book, &plan, job, terms.as_deref()),
        )?;
    }
    if std::path::Path::new(&ps::join(&p.staging, "ledger.sqlite3")).is_file() {
        let prepared = crate::ledger::Prepared::read(&p.staging, zt)?;
        crate::ledger::cited_cards_register(&prepared, &plan)?;
    }
    util::print_line(&util::compact(&json!({ "ok": true, "jobs": jobs.len(), "revision": revision, "rows": rows.len() })))?;
    Ok(0)
}

/// One writer job's file: the works, the whole source, the candidate lines, the Term spellings.
fn job_file(p: &Params, book: &Book, plan: &Value, job: &Value, terms: Option<&str>) -> String {
    let id = pstr(job, "id");
    let draft = draft_path(book, &id);
    let mut text = format!(
        "# 文献引用卡任务 {id}\n\n本书：《{}》（文献卡 [[{}]]）\n参考文献表卡：[[{}]]\n全书原文：{}（中英对照的 Markdown；行号从第 1 行起数）\n输出文件：{draft}\n\n",
        book.title, book.key, pstr(plan, "list"), book.source_path
    );
    if let Some(body) = terms {
        text.push_str("## 本书的术语（写左列，不用右列的词）\n\n");
        for row in zt_check::text::term_table(body) {
            text.push_str(&format!("- {}：不用 {}\n", row.term, if row.avoid.is_empty() { "—".to_string() } else { row.avoid.join("、") }));
        }
        text.push('\n');
    }
    text.push_str(&format!("## 关系词\n\n{}\n\n## 文献\n\n", RELATIONS.join("、")));
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
        .map(|job| {
            let id = pstr(job, "id");
            json!({
                "id": id,
                "files": [ps::join(&book.work, INSTRUCTION), ps::join(&ps::join(&book.work, "jobs"), &format!("{id}.md"))],
                "prompt": PROMPT,
                "skills": ["asd-ste100-zh"],
                "model": cited::MODEL,
                "effort": "high",
                "answer": draft_path(book, &id),
            })
        })
        .collect();
    util::print_line(&util::compact(&json!({ "ok": true, "jobs": jobs })))?;
    Ok(0)
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
    let words: Vec<String> = pstr(row, "title")
        .split(|c: char| !c.is_alphanumeric())
        .map(cited::fold)
        .filter(|w| w.len() >= 4)
        .collect();
    let year: String = pstr(row, "year").chars().take(4).collect();
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
        let names = (!surname.is_empty() && near.contains(&surname))
            || words.iter().filter(|w| near.contains(w.as_str())).count() >= 2;
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
        // R6 counts here: a Cited Card keeps the book's Term spellings.
        let blocking = d.blocking() || d.rule == "R6";
        let message = format!("{tag} {} {}{}", d.rule, if blocking { "" } else { "(advisory) " }, d.message);
        faults.push((message, blocking));
    }
    faults
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

fn apply_step(p: &Params, book: &Book, zt: &mut Zt) -> Result<i32> {
    let plan = read_plan(book)?;
    let terms = term_body(zt, &book.key);
    let (states, items) = readiness(book, &plan, terms.as_deref());
    let list = cited::existing_list(zt, &book.key)?.context("the book has no Cited list")?;
    let list_address = pstr(&list, "list");
    if list_address != pstr(&plan, "list") {
        bail!("the Cited list moved from {} to {list_address}; rerun -Step plan", pstr(&plan, "list"));
    }
    let existing = list["cards"].as_array().cloned().unwrap_or_default();
    let plan_rows = plan["rows"].as_array().cloned().unwrap_or_default();
    let mut published = Vec::new();
    let mut cells: HashMap<u64, (String, String)> = HashMap::new();
    let mut stop: Option<String> = None;
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
                if p.apply {
                    let current = zt.call("get_card", json!({ "at": address }))?;
                    if pstr(&current, "title") != title || pstr(&current, "body").trim_end() != body.trim_end() {
                        zt.call("edit_card", json!({ "at": address, "title": title, "body": body }))?;
                    }
                }
                address
            }
            None if p.apply => {
                let created = zt.call(
                    "create_successor",
                    json!({ "at": list_address, "kind": "side", "type": "cited", "title": title, "body": body }),
                )?;
                pstr(&created, "address")
            }
            None => String::new(),
        };
        let found = locations(book, item);
        cells.insert(n, (cited_at(&found), relations(item).join("、")));
        published.push(json!({ "row": n, "card": address, "status": "committed" }));
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
        "readiness": states,
    });
    util::write_atomic(ps::join(&book.work, "report.json"), &ps::json_pretty(&report))?;
    let mut line = json!({ "ok": true, "rows": plan_rows.len(), "committed": published.len() });
    if complete {
        line["complete"] = json!(true);
    }
    util::print_line(&util::compact(&line))?;
    Ok(0)
}
