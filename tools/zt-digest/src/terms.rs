//! `zt-digest terms`: the port of `scripts/book-digest/book-terms.ps1`, the book term table of a
//! digest (docs/digest-card-layers-proposal.md, proposal 2). The tool finds the candidate terms;
//! DeepSeek chooses the terms and the words not to use as a JSON file; the tool writes the table.
//! Rule R6 (one term, one meaning) then checks every Card of the book against it (the audit reads
//! a Card of the Literature tree whose title starts with 术语表).
//!
//!   zt-digest terms -Step plan|apply -Staging <folder> [-MaxTerms 60] [-MaxCandidates 160] [-Apply]
//!
//! -Staging is the digest's staging folder (manifest.json, sections\). The job and its output live
//! in <Staging>\terms\. Every step prints compact JSON as its last line.
//!
//!   plan   writes terms\job.md (the section headings, the words with their original in brackets,
//!          the frequent word groups, the bold words, and the sentences that define a term, each
//!          with its section file and line) and terms\dsh-terms-instructions.zh.md. Prints
//!          {"bold","definitions","jobs":[{"id","files","prompt"}]}.
//!   apply  reads terms\out\terms.json, keeps at most -MaxTerms valid entries, writes
//!          <Staging>\terms.md, and with -Apply creates or rewrites the Card 术语表 (a Side
//!          successor of <key>/1). Prints {"ok","terms","with_avoid","card","file"}.

use crate::args::Args;
use crate::organize::common::{self, Sb, prop, pstr};
use crate::ps::{self, CiMap};
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use std::io::Write;
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*([^*\n]{2,16})\*\*").unwrap());
static HAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]").unwrap());
static DEFINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(称为|叫做|定义为|是指|即所谓|我们把)").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static GLOSSED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([\x{3400}-\x{9FFF}]{2,10})\s*[（(]\s*([A-Za-z][A-Za-z \-]{2,40}?)\s*[)）]").unwrap());
static NOT_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$[^$]*\$|\[[^\]]*\]|\([^)]*\)").unwrap());
static RUN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}]{2,}").unwrap());
static DE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"的$|^的").unwrap());
static FUNCTION_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(我们|这个|一个|因为|所以|如果|但是|可以|没有|就是|之间|以及|这种|它们|他们|其中|通过|对于|由于|这些|那么|因此|而且|并且|不是|还是|已经|只有)").unwrap()
});
static BAD_TERM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[|`\n]").unwrap());
static BAD_AVOID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[|`、\n]").unwrap());
static TERM_TABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());

const PROMPT: &str = "按以上说明选出本书的术语，写出 JSON 文件。最后只回复一行 JSON。";

struct Params {
    step: String,
    staging: String,
    max_terms: i64,
    max_candidates: i64,
    apply: bool,
    operation_prefix: String,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Step", "Staging", "MaxTerms", "MaxCandidates", "OperationPrefix"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["Apply"])?;
    let step = a.choice("Step", &["plan", "apply"])?;
    let p = Params {
        step,
        staging: common::mandatory(&a, "Staging")?,
        max_terms: a.int_or("MaxTerms", 60)?,
        max_candidates: a.int_or("MaxCandidates", 160)?,
        apply: a.flag("Apply"),
        operation_prefix: a.str("OperationPrefix"),
    };
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    let work = ps::join(&p.staging, "terms");
    let out = ps::join(&work, "out");
    std::fs::create_dir_all(&work)?;
    std::fs::create_dir_all(&out)?;
    if p.step == "plan" { plan(&p, &manifest, &key, &work, &out) } else { apply(&p, &manifest, &key, &out) }
}

/// `Get-Content -Raw -Encoding utf8`: the text without a BOM, or None for an empty file ($null).
fn raw(path: &str) -> Result<Option<String>> {
    let bytes = std::fs::read(path).map_err(|e| anyhow::anyhow!("failed to read {path}: {e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text).to_string();
    Ok(if bytes.is_empty() { None } else { Some(text) })
}

/// `Select-Object -First $n`: a negative count fails the parameter's validation.
fn first(n: i64) -> Result<usize> {
    if n < 0 {
        bail!(
            "Cannot validate argument on parameter 'First'. The {n} argument is less than the minimum allowed range of 0. Supply an argument that is greater than or equal to 0 and then try the command again."
        );
    }
    Ok(n as usize)
}

/// `Get-ChildItem -LiteralPath <dir> -Filter 's*.md' -File | Sort-Object Name`: the file names.
fn section_files(dir: &str) -> Result<Vec<String>> {
    if !std::path::Path::new(dir).is_dir() {
        bail!("Cannot find path '{dir}' because it does not exist.");
    }
    let mut names = ps::child_files(dir, "s", ".md");
    ps::ps_sort(&mut names, |a, b| ps::culture_cmp(a, b));
    Ok(names)
}

/// The frequent word groups: runs of 2 to 7 Chinese characters counted over `texts`, those seen
/// at least 6 times by count (most first), without a group that a longer group covers (a longer
/// group with 80% of its count), one that starts or ends with 的, or one that starts with a
/// function word; at most 60.
///
/// The script counted in a hashtable, whose order is random, before sorting by count; the tool
/// counts in order of first appearance, so groups with the same count keep one fixed order.
fn frequent_groups(texts: &[String]) -> Vec<(String, i64)> {
    let mut grams: Vec<(String, i64)> = Vec::new();
    let mut at: HashMap<String, usize> = HashMap::new();
    for text in texts {
        let text = NOT_TEXT.replace_all(text, " ");
        for run in RUN.find_iter(&text) {
            let chars: Vec<char> = run.as_str().chars().collect();
            for n in 2..=7usize {
                let mut i = 0;
                while i + n <= chars.len() {
                    let g: String = chars[i..i + n].iter().collect();
                    match at.get(&g) {
                        Some(&k) => grams[k].1 += 1,
                        None => {
                            at.insert(g.clone(), grams.len());
                            grams.push((g, 1));
                        }
                    }
                    i += 1;
                }
            }
        }
    }
    let mut frequent: Vec<(String, i64)> = grams.into_iter().filter(|g| g.1 >= 6).collect();
    ps::ps_sort(&mut frequent, |a, b| (-a.1).cmp(&-b.1));
    // `$covered`: some longer frequent group holds this one and has at least 80% of its count. The
    // largest count among the longer groups that hold a group decides it.
    let mut cover: HashMap<String, i64> = HashMap::new();
    for (k, v) in &frequent {
        let chars: Vec<char> = k.chars().collect();
        for n in 2..chars.len() {
            for i in 0..=chars.len() - n {
                let s: String = chars[i..i + n].iter().collect();
                let best = cover.entry(s).or_insert(*v);
                *best = (*best).max(*v);
            }
        }
    }
    let mut kept = Vec::new();
    for (k, v) in &frequent {
        let covered = cover.get(k).is_some_and(|&c| c as f64 >= 0.8 * *v as f64);
        if !covered && !DE.is_match(k) && !FUNCTION_WORDS.is_match(k) {
            kept.push((k.clone(), *v));
        }
        if kept.len() >= 60 {
            break;
        }
    }
    kept
}

fn plan(p: &Params, manifest: &Value, key: &str, work: &str, out: &str) -> Result<i32> {
    let mut sb = Sb::new();
    sb.line(format!("# 术语任务：《{}》", pstr(manifest, "book_title"))).blank();
    sb.line(format!("把结果写入 `{}`。", ps::join(out, "terms.json"))).blank();
    sb.line("## 小节标题").blank();
    for g in ps::pipe(prop(manifest, "groups")) {
        let headings: Vec<String> = ps::pipe(prop(g, "headings")).into_iter().map(ps::s).collect();
        sb.line(format!("- 第 {} 组：{}：{}", pstr(g, "n"), pstr(g, "chapter"), headings.join("；")));
    }
    sb.blank();
    let sections = ps::join(&p.staging, "sections");
    let files = section_files(&sections)?;
    let mut bold: CiMap<String> = CiMap::new();
    let mut defs: Vec<String> = Vec::new();
    for name in &files {
        let mut n = 0;
        for line in ps::read_lines(&ps::join(&sections, name))? {
            n += 1;
            for m in BOLD.captures_iter(&line) {
                let t = m[1].trim().to_string();
                if HAN.is_match(&t) && !bold.contains(&t) {
                    bold.insert(&t, format!("sections/{name}:{n}"));
                }
            }
            if (defs.len() as i64) < p.max_candidates && HAN.is_match(&line) && DEFINES.is_match(&line) {
                let mut clip = SPACES.replace_all(&line, " ").trim().to_string();
                if ps::len16(&clip) > 140 {
                    clip = ps::cut16(&clip, 140) + "…";
                }
                defs.push(format!("- `sections/{name}:{n}` {clip}"));
            }
        }
    }
    // A translation often keeps the original term in brackets: 剩余控制权（residual control rights）.
    let mut glossed: CiMap<String> = CiMap::new();
    let mut texts: Vec<String> = Vec::new();
    for name in &files {
        let Some(text) = raw(&ps::join(&sections, name))? else {
            bail!("Exception calling \"Matches\" with \"2\" argument(s): \"Value cannot be null. (Parameter 'input')\"");
        };
        for m in GLOSSED.captures_iter(&text) {
            let t = m[1].to_string();
            if !glossed.contains(&t) {
                glossed.insert(&t, m[2].trim().to_string());
            }
        }
        texts.push(text);
    }
    let kept = frequent_groups(&texts);
    sb.line("## 带原文的词（中文术语与括号中的英文）").blank();
    if !glossed.is_empty() {
        for (t, g) in glossed.iter().take(first(p.max_candidates)?) {
            sb.line(format!("- {t}（{g}）"));
        }
    } else {
        sb.line("（没有）");
    }
    sb.blank().line("## 高频词组（出现次数）").blank();
    if !kept.is_empty() {
        sb.line(kept.iter().map(|(k, v)| format!("{k}（{v}）")).collect::<Vec<_>>().join("、"));
    } else {
        sb.line("（没有）");
    }
    sb.blank();
    sb.line("## 书中加粗的词").blank();
    for (t, at) in bold.iter().take(first(p.max_candidates)?) {
        sb.line(format!("- {t}（`{at}`）"));
    }
    if bold.is_empty() {
        sb.line("（没有）");
    }
    sb.blank().line("## 定义术语的句子").blank();
    if !defs.is_empty() {
        for d in &defs {
            sb.line(d);
        }
    } else {
        sb.line("（没有）");
    }
    let job = ps::join(work, "job.md");
    util::write_text(&job, &sb.0)?;
    let doc_copy = ps::join(work, "dsh-terms-instructions.zh.md");
    let doc = ps::join(&common::scripts_dir(), "dsh-terms-instructions.zh.md");
    let text = raw(&doc)?.unwrap_or_default();
    util::write_text(&doc_copy, &text.replace("{{KEY}}", key).replace("{{STAGING}}", &p.staging))?;
    let last = json!({
        "bold": bold.len(), "definitions": defs.len(),
        "jobs": [{ "id": "terms", "files": [doc_copy, job], "prompt": PROMPT }]
    });
    util::print_line(&ps::json_compact(&last))?;
    Ok(0)
}

/// One entry of the table.
struct Term {
    term: String,
    avoid: Vec<String>,
    section: String,
    meaning: String,
}

/// The valid entries of terms.json: a term once (case ignored), without `|`, a backtick, or a
/// line break; its words not to use trimmed, without the term itself, a word inside the term, or
/// `|`, a backtick, `、`, or a line break, each once. At most `max` entries (at least one).
fn valid_terms(raw: &Value, max: i64) -> Vec<Term> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut terms = Vec::new();
    for t in ps::pipe(&raw["terms"]) {
        let term = ps::field(t, "term").trim().to_string();
        if term.is_empty() || seen.contains(&ps::key(&term)) || BAD_TERM.is_match(&term) {
            continue;
        }
        let avoid = ps::select_unique(
            ps::pipe(t.get("avoid").unwrap_or(&Value::Null))
                .into_iter()
                .map(|x| ps::s(x).trim().to_string())
                .filter(|x| !x.is_empty() && !ps::eq(x, &term) && !BAD_AVOID.is_match(x) && !term.contains(x.as_str())),
        );
        seen.insert(ps::key(&term));
        terms.push(Term { term, avoid, section: ps::field(t, "section").trim().to_string(), meaning: ps::field(t, "meaning").trim().to_string() });
        if terms.len() as i64 >= max {
            break;
        }
    }
    terms
}

/// The body of the Card 术语表 (and terms.md).
fn table(book_title: &str, key: &str, terms: &[Term]) -> String {
    let mut sb = Sb::new();
    sb.line(format!("本卡是《{book_title}》的术语表。本书的卡片用左列的术语，不用右列的词。检查脚本按本表检查规则 R6（一个术语一个意思）。")).blank();
    sb.line("术语表").blank().line("| 术语 | 不用 | 意思 | 首次定义 |").line("| --- | --- | --- | --- |");
    for t in terms {
        let avoid = if t.avoid.is_empty() { "—".to_string() } else { t.avoid.join("、") };
        sb.line(format!("| {} | {} | {} | {} |", t.term, avoid, t.meaning.replace('|', "/"), t.section));
    }
    sb.blank().line(format!("来源：[[{key}]]"));
    sb.0.trim_end().to_string()
}

fn apply(p: &Params, manifest: &Value, key: &str, out: &str) -> Result<i32> {
    let file = ps::join(out, "terms.json");
    if !ps::exists(&file) {
        util::print_line(&ps::json_compact(
            &json!({ "ok": false, "error": "no terms.json" }),
        ))?;
        return Ok(0);
    }
    let raw = common::read_json(&file)?;
    let terms = valid_terms(&raw, p.max_terms);
    let book_title = pstr(manifest, "book_title");
    let body = table(&book_title, key, &terms);
    let terms_md = ps::join(&p.staging, "terms.md");
    util::write_text(&terms_md, &body)?;
    let title = format!("术语表：《{book_title}》的 {} 个术语", terms.len());
    let mut address = Value::Null;
    let mut receipt_keys = Vec::new();
    if p.apply {
        let mut zt = Zt::start()?;
        if p.operation_prefix.is_empty() {
            let call = term_call(&mut zt, key, &title, &body, false)?;
            let lines = common::write_all(&mut zt, &[call])?;
            address = Value::String(common::first_ok(&lines));
        } else {
            let input = json!({"book":key,"title":title,"body":body});
            let work = ps::join(&p.staging, "terms");
            let plan_file = ps::join(
                &work,
                &format!(
                    "accepted-{}.json",
                    crate::canon::hash_value(&json!(p.operation_prefix))
                ),
            );
            let accepted = if ps::exists(&plan_file) {
                util::read_json(&plan_file)?
            } else {
                let call = term_call(&mut zt, key, &title, &body, true)?;
                let plan =
                    json!({"operation_prefix":p.operation_prefix,"input":input,"calls":[call]});
                let mut pending = tempfile::NamedTempFile::new_in(&work)?;
                pending.write_all(serde_json::to_string_pretty(&plan)?.as_bytes())?;
                pending.as_file().sync_all()?;
                match pending.persist_noclobber(&plan_file) {
                    Ok(_) => plan,
                    Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                        util::read_json(&plan_file)?
                    }
                    Err(error) => {
                        return Err(error.error).context("retain accepted terms write plan");
                    }
                }
            };
            if accepted["operation_prefix"] != p.operation_prefix || accepted["input"] != input {
                bail!(
                    "OperationPrefix already binds another accepted terms plan; use a newly accepted plan revision"
                );
            }
            let calls = accepted["calls"]
                .as_array()
                .filter(|calls| calls.len() == 1)
                .context("incomplete accepted terms write plan")?;
            let receipts = crate::calls::invoke_identified(&mut zt, calls, &p.operation_prefix, None)?;
            address = receipts[0]["result"]["address"].clone();
            receipt_keys = receipts.iter().map(|r| r["op_key"].clone()).collect();
        }
    }
    let mut last = json!({
        "ok": true, "terms": terms.len(), "with_avoid": terms.iter().filter(|t| !t.avoid.is_empty()).count(),
        "card": address, "file": terms_md
    });
    if !p.operation_prefix.is_empty() && p.apply {
        last["operation_prefix"] = json!(p.operation_prefix);
        last["receipt_keys"] = json!(receipt_keys);
    }
    util::print_line(&ps::json_compact(&last))?;
    Ok(0)
}

fn term_call(zt: &mut Zt, key: &str, title: &str, body: &str, guarded: bool) -> Result<Value> {
    let tree = common::read_first(zt, "get_tree", json!({"tree":key}))?;
    let existing = ps::each(prop(&tree, "cards"))
        .into_iter()
        .find(|c| TERM_TABLE.is_match(&pstr(c, "title")));
    Ok(match existing {
        Some(card) => {
            let address = pstr(card, "address");
            let mut args = json!({"at":address,"title":title,"body":body});
            if guarded {
                let read = zt.cards(&[address], "text")?;
                args["base_fingerprint"] = json!(crate::calls::authored_fingerprint(&read.cards[0]));
            }
            json!({"tool":"edit_card","arguments":args})
        }
        None => {
            json!({"tool":"create_successor","arguments":{"at":format!("{key}/1"),"kind":"side","title":title,"body":body}})
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terms_drop_duplicates_and_bad_words() {
        let raw = json!({ "terms": [
            { "term": " 剩余控制权 ", "avoid": ["残余控制权", "剩余控制权", "控制", "残余控制权", "a|b", ""], "meaning": "权利|定义", "section": "§1" },
            { "term": "敲竹杠", "avoid": "要挟", "meaning": "机会主义", "section": "§1" },
            { "term": "剩余控制权", "avoid": ["别的"] },
            { "term": "", "avoid": ["空"] },
            "plain text",
            { "term": "GDP" }, { "term": "gdp" },
            { "term": "a`b" }
        ] });
        let terms = valid_terms(&raw, 60);
        assert_eq!(terms.iter().map(|t| t.term.as_str()).collect::<Vec<_>>(), ["剩余控制权", "敲竹杠", "GDP"]);
        assert_eq!(terms[0].avoid, ["残余控制权"]);
        assert_eq!(terms[1].avoid, ["要挟"]);
        assert!(terms[2].avoid.is_empty());
        assert_eq!(valid_terms(&raw, 0).len(), 1);
        let body = table("书", "K", &terms);
        let nl = ps::NL;
        assert!(body.starts_with(&format!("本卡是《书》的术语表。")));
        assert!(body.contains(&format!("| 剩余控制权 | 残余控制权 | 权利/定义 | §1 |{nl}")));
        assert!(body.contains(&format!("| GDP | — |  |  |{nl}")));
        assert!(body.ends_with(&format!("{nl}{nl}来源：[[K]]")));
    }

    #[test]
    fn frequent_groups_keep_the_uncovered_ones() {
        // 控制权 appears 8 times, always inside 剩余控制权 (8): the longer group covers it and its
        // parts. 我们的 starts with a function word; 的 groups are dropped.
        let text = "剩余控制权，".repeat(8) + &"敲竹杠和机会".repeat(6) + "（不算的词语不算的词语）$公式公式$";
        let kept = frequent_groups(&[text]);
        let names: Vec<&str> = kept.iter().map(|k| k.0.as_str()).collect();
        assert!(names.contains(&"剩余控制权"));
        assert!(!names.contains(&"控制权") && !names.contains(&"剩余"));
        assert!(names.iter().all(|n| !n.contains("公式") && !n.contains("不算")));
        assert_eq!(kept[0].1, 8);
    }
}
