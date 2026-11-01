//! `zt-digest terms`: the port of `scripts/book-digest/book-terms.ps1`, the book term table of a
//! digest (docs/digest-card-layers-proposal.md, proposal 2). The tool finds the candidate terms;
//! DeepSeek chooses the terms and the words not to use as a JSON file; the tool writes the table.
//! Rule R6 (one term, one meaning) then checks every Card of the book against it (the audit reads
//! a Card of the Literature tree whose title starts with 术语表).
//!
//!   zt-digest terms -Step plan|apply -Staging <folder> [-MaxTerms 0] [-MaxCandidates 160] [-Apply]
//!   zt-digest terms -Step plan|merge|apply -Staging <folder> -Token <n> [-MaxTerms 0] [-ExploreChars 30000] [-Apply]
//!
//! -MaxTerms 0 (the default) sets no upper bound: a book has as many terms as its length and content
//! need. A positive value caps the table.
//!
//! A claimed run (-Token, the prepared ledger) splits the book: `plan` cuts the sections into explorer
//! parts of about -ExploreChars characters and lists one DeepSeek explorer job per part; `merge` checks
//! every explorer answer and lists the one merge job that writes terms\out\terms.json; `apply` writes
//! the table Card from that answer (ledger.rs: terms_plan, terms_merge, terms_apply).
//!
//! -Staging is the digest's staging folder (manifest.json, sections\). The job and its output live
//! in <Staging>\terms\. Every step prints compact JSON as its last line.
//!
//!   plan   writes terms\job.md (the section headings, the words with their original in brackets,
//!          the frequent word groups, the bold words, and the sentences that define a term, each
//!          with its section file and line) and terms\dsh-terms-instructions.zh.md. Prints
//!          {"bold","definitions","jobs":[{"id","files","prompt"}]}.
//!   apply  reads terms\out\terms.json, keeps the valid entries (at most a positive -MaxTerms), writes
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
    let names = [
        "Step",
        "Staging",
        "MaxTerms",
        "MaxCandidates",
        "ExploreChars",
        "OperationPrefix",
        "Token",
    ];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["Apply"])?;
    if a.has("Token") {
        return crate::ledger::terms_cli(&a);
    }
    let step = a.choice("Step", &["plan", "apply"])?;
    let p = Params {
        step,
        staging: common::mandatory(&a, "Staging")?,
        max_terms: a.int_or("MaxTerms", 0)?,
        max_candidates: a.int_or("MaxCandidates", 160)?,
        apply: a.flag("Apply"),
        operation_prefix: a.str("OperationPrefix"),
    };
    if std::path::Path::new(&p.staging)
        .join("ledger.sqlite3")
        .exists()
    {
        bail!("prepared ledger Terms steps require -Token");
    }
    let manifest = common::manifest(&p.staging)?;
    if manifest.get("run_contract").is_some_and(|v| !v.is_null()) {
        bail!("prepared ledger Terms steps require -Token");
    }
    let key = pstr(&manifest, "citation_key");
    let work = ps::join(&p.staging, "terms");
    let out = ps::join(&work, "out");
    // R328 (§15.2): the plan allocates the one Terms job and the apply writes the table Card into
    // the book's Literature tree. Both stop before their first file or Card when that tree is
    // locked by the owner; `precheck`, `report` and a plan without `-Apply` never ask.
    if p.step == "plan" || p.apply {
        let mut zt = crate::zt::start()?;
        if let Some(refusal) = crate::lockcheck::preflight(&mut zt, "terms", &p.step, &key)? {
            return crate::lockcheck::stop("terms", &refusal);
        }
    }
    std::fs::create_dir_all(&work)?;
    std::fs::create_dir_all(&out)?;
    if p.step == "plan" {
        plan(&p, &manifest, &key, &work, &out)
    } else {
        apply(&p, &manifest, &key, &out)
    }
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
/// at least `min_count` times by count (most first), without a group that a longer group covers (a
/// longer group with 80% of its count), one that starts or ends with 的, or one that starts with a
/// function word; at most `max` (the whole book: 6 and 60; an explorer's part: 4 and 80).
///
/// The script counted in a hashtable, whose order is random, before sorting by count; the tool
/// counts in order of first appearance, so groups with the same count keep one fixed order.
fn frequent_groups(texts: &[String], min_count: i64, max: usize) -> Vec<(String, i64)> {
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
    let mut frequent: Vec<(String, i64)> = grams.into_iter().filter(|g| g.1 >= min_count).collect();
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
        if kept.len() >= max {
            break;
        }
    }
    kept
}

fn plan_data(p: &Params, manifest: &Value, key: &str, work: &str, out: &str) -> Result<Value> {
    let sections = ps::join(&p.staging, "sections");
    let files = section_files(&sections)?;
    let head = format!("# 术语任务：《{}》", pstr(manifest, "book_title"));
    let scan = scan_candidates(&p.staging, manifest, &files, p.max_candidates, &head, &ps::join(out, "terms.json"), None, 6, 60)?;
    let job = ps::join(work, "job.md");
    let doc_copy = ps::join(work, "dsh-terms-instructions.zh.md");
    let doc = ps::join(&common::scripts_dir(), "dsh-terms-instructions.zh.md");
    let text = raw(&doc)?.unwrap_or_default();
    let last = json!({
        "bold": scan.counts["bold"], "definitions": scan.counts["definitions"],
        "jobs": [{ "id": "terms", "files": [doc_copy, job], "prompt": PROMPT }]
    });
    Ok(
        json!({"summary":last,"job_text":scan.text,"instructions_text":text.replace("{{KEY}}", key).replace("{{STAGING}}", &p.staging),"candidate_count":scan.candidate_count,"counts":scan.counts}),
    )
}

/// The candidate list of one explorer's sections (or of the whole book): the job text and its counts.
struct Scan {
    text: String,
    counts: Value,
    candidate_count: usize,
}

/// The candidates of `files` (section file names under `<staging>\sections`): the section headings of
/// the whole book (context), the bracketed originals, the frequent word groups (seen at least
/// `min_count` times in these files, at most `max_groups`), the bold words and the defining
/// sentences. `scope` names the explorer's own range when the book is split.
#[allow(clippy::too_many_arguments)]
fn scan_candidates(
    staging: &str,
    manifest: &Value,
    files: &[String],
    max_candidates: i64,
    head: &str,
    answer: &str,
    scope: Option<&str>,
    min_count: i64,
    max_groups: usize,
) -> Result<Scan> {
    let mut sb = Sb::new();
    sb.line(head).blank();
    sb.line(format!("把结果写入 `{answer}`。")).blank();
    if let Some(scope) = scope {
        sb.line(scope).blank();
    }
    sb.line("## 小节标题").blank();
    for g in ps::pipe(prop(manifest, "groups")) {
        let headings: Vec<String> = ps::pipe(prop(g, "headings"))
            .into_iter()
            .map(ps::s)
            .collect();
        sb.line(format!(
            "- 第 {} 组：{}：{}",
            pstr(g, "n"),
            pstr(g, "chapter"),
            headings.join("；")
        ));
    }
    sb.blank();
    let sections = ps::join(staging, "sections");
    let mut bold: CiMap<String> = CiMap::new();
    let mut defs: Vec<String> = Vec::new();
    for name in files {
        let mut n = 0;
        for line in ps::read_lines(&ps::join(&sections, name))? {
            n += 1;
            for m in BOLD.captures_iter(&line) {
                let t = m[1].trim().to_string();
                if HAN.is_match(&t) && !bold.contains(&t) {
                    bold.insert(&t, format!("sections/{name}:{n}"));
                }
            }
            if (defs.len() as i64) < max_candidates
                && HAN.is_match(&line)
                && DEFINES.is_match(&line)
            {
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
    for name in files {
        let Some(text) = raw(&ps::join(&sections, name))? else {
            bail!(
                "Exception calling \"Matches\" with \"2\" argument(s): \"Value cannot be null. (Parameter 'input')\""
            );
        };
        for m in GLOSSED.captures_iter(&text) {
            let t = m[1].to_string();
            if !glossed.contains(&t) {
                glossed.insert(&t, m[2].trim().to_string());
            }
        }
        texts.push(text);
    }
    let kept = frequent_groups(&texts, min_count, max_groups);
    sb.line("## 带原文的词（中文术语与括号中的英文）").blank();
    if !glossed.is_empty() {
        for (t, g) in glossed.iter().take(first(max_candidates)?) {
            sb.line(format!("- {t}（{g}）"));
        }
    } else {
        sb.line("（没有）");
    }
    sb.blank().line("## 高频词组（出现次数）").blank();
    if !kept.is_empty() {
        sb.line(
            kept.iter()
                .map(|(k, v)| format!("{k}（{v}）"))
                .collect::<Vec<_>>()
                .join("、"),
        );
    } else {
        sb.line("（没有）");
    }
    sb.blank();
    sb.line("## 书中加粗的词").blank();
    for (t, at) in bold.iter().take(first(max_candidates)?) {
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
    let candidate_count = bold.len() + defs.len() + glossed.len() + kept.len();
    Ok(Scan {
        text: sb.0,
        counts: json!({"bold":bold.len(),"definitions":defs.len(),"glossed":glossed.len(),"frequent":kept.len()}),
        candidate_count,
    })
}

/// The section files cut into explorer parts in reading order: a part takes files until it holds about
/// `explore_chars` UTF-16 characters, so a long book gets more explorers and a short paper one or two.
fn split_parts(sections: &str, files: &[String], explore_chars: i64) -> Result<Vec<Vec<String>>> {
    let mut parts: Vec<Vec<String>> = Vec::new();
    let mut size = 0i64;
    for name in files {
        let len = raw(&ps::join(sections, name))?.map(|t| ps::len16(&t) as i64).unwrap_or(0);
        if parts.is_empty() || (size > 0 && size + len > explore_chars) {
            parts.push(Vec::new());
            size = 0;
        }
        parts.last_mut().unwrap().push(name.clone());
        size += len;
    }
    Ok(parts)
}

/// The claimed Terms plan of a split book: one explorer part per about `explore_chars` characters of
/// sections, each with its own candidates, and the explorer and merge instructions. A part keeps the
/// frequent word groups seen at least 4 times in its own sections (at most 80).
pub(crate) fn claimed_parts(staging: &str, max_candidates: i64, explore_chars: i64) -> Result<Value> {
    let manifest = common::manifest(staging)?;
    let key = pstr(&manifest, "citation_key");
    let title = pstr(&manifest, "book_title");
    let out = ps::join(&ps::join(staging, "terms"), "out");
    let sections = ps::join(staging, "sections");
    let files = section_files(&sections)?;
    let split = split_parts(&sections, &files, explore_chars.max(1))?;
    let total = split.len();
    let mut parts = Vec::new();
    let mut candidates = 0usize;
    for (i, names) in split.iter().enumerate() {
        let part = i + 1;
        let head = format!("# 术语探索任务 {part}/{total}：《{title}》");
        let scope = format!(
            "你的范围：`sections/{}` 到 `sections/{}`（{} 个小节）。只从这些小节选词；其他探索代理读其余小节，合并代理最后汇总。",
            names.first().unwrap(),
            names.last().unwrap(),
            names.len()
        );
        let answer = ps::join(&out, &explore_name(part));
        let scan = scan_candidates(staging, &manifest, names, max_candidates, &head, &answer, Some(&scope), 4, 80)?;
        candidates += scan.candidate_count;
        parts.push(json!({"part":part,"files":names,"job_text":scan.text,"candidate_count":scan.candidate_count,"counts":scan.counts}));
    }
    let doc = |name: &str| -> Result<String> {
        let text = raw(&ps::join(&common::scripts_dir(), name))?.unwrap_or_default();
        Ok(text.replace("{{KEY}}", &key).replace("{{STAGING}}", staging))
    };
    Ok(json!({
        "parts": parts, "candidate_count": candidates,
        "instructions_text": doc("dsh-terms-instructions.zh.md")?,
        "merge_instructions_text": doc("dsh-terms-merge-instructions.zh.md")?,
    }))
}

/// The answer file of explorer `part`: `explore-01.json`, `explore-02.json`, …
pub(crate) fn explore_name(part: usize) -> String {
    format!("explore-{part:02}.json")
}

/// The merge job's candidate table: every explorer row, grouped by term (case ignored), with the parts
/// that proposed it. `rows` holds (part, row) pairs in part order.
pub(crate) fn merge_text(book_title: &str, rows: &[(usize, Value)], parts: usize) -> String {
    let mut order: Vec<String> = Vec::new();
    let mut by: HashMap<String, Vec<(usize, &Value)>> = HashMap::new();
    for (part, row) in rows {
        let term = ps::field(row, "term").trim().to_string();
        if term.is_empty() {
            continue;
        }
        let k = ps::key(&term);
        if !by.contains_key(&k) {
            order.push(k.clone());
        }
        by.entry(k).or_default().push((*part, row));
    }
    let mut sb = Sb::new();
    sb.line(format!("# 术语合并任务：《{book_title}》")).blank();
    sb.line(format!("{parts} 个探索代理各读了本书的一段，共提出 {} 行候选，合为 {} 个不同的词。每行写明是哪个探索代理提出的。", rows.len(), order.len())).blank();
    sb.line("## 候选术语").blank();
    sb.line("| 术语 | 不用 | 意思 | 首次定义 | 提出者 |").line("| --- | --- | --- | --- | --- |");
    for k in &order {
        for (part, row) in &by[k] {
            let avoid = ps::pipe(row.get("avoid").unwrap_or(&Value::Null)).into_iter().map(ps::s).collect::<Vec<_>>().join("、");
            let cell = |s: String| s.replace(['|', '\n', '\r'], " ");
            sb.line(format!(
                "| {} | {} | {} | {} | 探索 {part} |",
                cell(ps::field(row, "term").trim().to_string()),
                cell(if avoid.is_empty() { "—".into() } else { avoid }),
                cell(ps::field(row, "meaning")),
                cell(ps::field(row, "section"))
            ));
        }
    }
    sb.0
}

fn plan(p: &Params, manifest: &Value, key: &str, work: &str, out: &str) -> Result<i32> {
    let data = plan_data(p, manifest, key, work, out)?;
    util::write_atomic(
        ps::join(work, "job.md"),
        data["job_text"]
            .as_str()
            .context("missing Terms job text")?,
    )?;
    util::write_atomic(
        ps::join(work, "dsh-terms-instructions.zh.md"),
        data["instructions_text"]
            .as_str()
            .context("missing Terms instructions")?,
    )?;
    util::print_line(&ps::json_compact(&data["summary"]))?;
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
/// `|`, a backtick, `、`, or a line break, each once. At most `max` entries when `max` is positive
/// (at least one); `max` 0 or less keeps every valid entry.
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
        if max > 0 && terms.len() as i64 >= max {
            break;
        }
    }
    terms
}

/// The ways the term table can name its book, longest first: the full title, the main title
/// before a colon, and none (the Card then says 本书).
fn book_names(book_title: &str) -> Vec<String> {
    let full = book_title.trim().to_string();
    let main = full.split([':', '：']).next().unwrap_or("").trim().to_string();
    let mut names = vec![full];
    if !main.is_empty() && main != names[0] {
        names.push(main);
    }
    names
}

/// The Card's title: 术语表：《…》的 N 个术语, with the longest book name R12 allows.
fn term_title(book_title: &str, count: usize) -> String {
    book_names(book_title)
        .into_iter()
        .map(|name| format!("术语表：《{name}》的 {count} 个术语"))
        .find(|title| zt_check::check::title_fits(title))
        .unwrap_or_else(|| format!("术语表：本书的 {count} 个术语"))
}

/// The body of the Card 术语表 (and terms.md).
fn table(book_title: &str, key: &str, terms: &[Term]) -> String {
    let mut sb = Sb::new();
    // The first sentence names the book by the longest name R2 allows.
    let opening = book_names(book_title)
        .into_iter()
        .map(|name| format!("本卡是《{name}》的术语表。"))
        .find(|sentence| zt_check::check::sentence_fits(sentence))
        .unwrap_or_else(|| "本卡是本书的术语表。".to_string());
    sb.line(format!("{opening}本书的卡片用左列的术语，不用右列的词。检查脚本按本表检查规则 R6（一个术语一个意思）。")).blank();
    sb.line("术语表").blank().line("| 术语 | 不用 | 意思 | 首次定义 |").line("| --- | --- | --- | --- |");
    for t in terms {
        let avoid = if t.avoid.is_empty() { "—".to_string() } else { t.avoid.join("、") };
        sb.line(format!("| {} | {} | {} | {} |", t.term, avoid, t.meaning.replace('|', "/"), t.section));
    }
    sb.blank().line(format!("来源：[[{key}]]"));
    sb.0.trim_end().to_string()
}

/// Claimed runs reject incomplete/ambiguous rows instead of silently dropping them.
pub(crate) fn claimed_table(raw: &Value, max: i64, book_title: &str, key: &str) -> Result<Value> {
    let rows = raw["terms"]
        .as_array()
        .context("Terms answer requires an array of terms")?;
    if max > 0 && rows.len() > max as usize {
        bail!("Terms answer exceeds the frozen MaxTerms");
    }
    let mut seen = HashSet::new();
    for row in rows {
        let fields = row.as_object().context("Terms row must be an object")?;
        if fields
            .keys()
            .any(|name| !["term", "avoid", "meaning", "section"].contains(&name.as_str()))
        {
            bail!("unknown Terms row field");
        }
        for name in ["term", "meaning", "section"] {
            let value = row[name]
                .as_str()
                .filter(|s| !s.trim().is_empty())
                .with_context(|| format!("Terms row requires {name}"))?;
            if value.contains(['|', '`', '\n', '\r', '\t']) {
                bail!("Terms row {name} must be safe one-line table text");
            }
        }
        let term = row["term"].as_str().unwrap().trim();
        if !seen.insert(ps::key(term)) {
            bail!("duplicate Terms definition: {term}");
        }
        let avoid = row["avoid"]
            .as_array()
            .context("Terms avoid must be an array")?;
        for value in avoid {
            let word = value.as_str().context("Terms avoid entries must be text")?;
            if word.contains(['|', '`', '、', '\n', '\r', '\t']) {
                bail!("Terms avoid entries must be safe one-line words");
            }
        }
    }
    for row in rows {
        let term = row["term"].as_str().unwrap().trim();
        for word in row["avoid"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
        {
            if !ps::eq(word.trim(), term) && seen.contains(&ps::key(word.trim())) {
                bail!("Terms avoid entry names another selected term: {word}");
            }
        }
    }
    let terms = valid_terms(raw, max);
    if terms.len() != rows.len() {
        bail!("Terms answer contains an invalid or skipped row");
    }
    Ok(
        json!({"terms":terms.len(),"with_avoid":terms.iter().filter(|t|!t.avoid.is_empty()).count(),"title":term_title(book_title,terms.len()),"body":table(book_title,key,&terms)}),
    )
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
    let title = term_title(&book_title, terms.len());
    let mut address = Value::Null;
    let mut receipt_keys = Vec::new();
    if p.apply {
        let mut zt = crate::zt::start()?;
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
            let receipts = crate::calls::invoke_identified(&mut zt, calls, &p.operation_prefix)?;
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

pub(crate) fn term_call(zt: &mut Zt, key: &str, title: &str, body: &str, guarded: bool) -> Result<Value> {
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
        None => new_table_call(key, title, body),
    })
}

/// The call that files a new book's list. A new book's list is a Term Card with no name from the
/// start (S21, #165, design 12.5, R0336): the row parser and the Term-card compiler both read the
/// list by that role, so a book never has to be retyped after its first publication. A book whose
/// table predates the Term type keeps its Idea Card, kept byte for byte, and the Term-card
/// compiler's first publication retypes it (metadata only). The claimed apply checks its accepted
/// call against this same constructor.
pub(crate) fn new_table_call(key: &str, title: &str, body: &str) -> Value {
    json!({"tool":"create_successor","arguments":{"at":format!("{key}/1"),"kind":"side","type":"term","title":title,"body":body}})
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
        assert_eq!(valid_terms(&raw, 1).len(), 1);
        // No cap: 0 keeps every valid entry, however long the book's table is.
        assert_eq!(valid_terms(&raw, 0).len(), 3);
        let body = table("书", "K", &terms);
        let nl = ps::NL;
        assert!(body.starts_with(&format!("本卡是《书》的术语表。")));
        assert!(body.contains(&format!("| 剩余控制权 | 残余控制权 | 权利/定义 | §1 |{nl}")));
        assert!(body.contains(&format!("| GDP | — |  |  |{nl}")));
        assert!(body.ends_with(&format!("{nl}{nl}来源：[[K]]")));
    }

    // The term table names its book in its title and first sentence. A long English title
    // (stiglitz2014learning: 86 characters with its subtitle) broke R12 and R2, so the Card
    // names the book by the longest form that fits: the full title, the main title, or 本书.
    #[test]
    fn the_term_table_names_its_book_within_the_card_limits() {
        let long = "Creating a Learning Society: A New Approach to Growth, Development, and Social Progress";
        let (title, body) = (term_title(long, 60), table(long, "K", &[]));
        let first = body.split('。').next().unwrap().to_string() + "。";
        assert!(zt_check::check::title_fits(&title), "{title}");
        assert!(zt_check::check::sentence_fits(&first), "{first}");
        assert!(title.contains("Creating a Learning Society"), "{title}");
        assert_eq!(term_title("企业、合同与财务结构", 12), "术语表：《企业、合同与财务结构》的 12 个术语");
        assert!(table("书", "K", &[]).starts_with("本卡是《书》的术语表。"));
        let huge = "An Extraordinarily Long Main Title Without Any Subtitle That Never Ends Here";
        assert!(zt_check::check::title_fits(&term_title(huge, 60)));
        // zt-check counts a book title in 《…》 as one name, so even a very long title fits the first sentence.
        assert!(table(huge, "K", &[]).starts_with(&format!("本卡是《{huge}》的术语表。")));
    }

    #[test]
    fn frequent_groups_keep_the_uncovered_ones() {
        // 控制权 appears 8 times, always inside 剩余控制权 (8): the longer group covers it and its
        // parts. 我们的 starts with a function word; 的 groups are dropped.
        let text = "剩余控制权，".repeat(8) + &"敲竹杠和机会".repeat(6) + "（不算的词语不算的词语）$公式公式$";
        let kept = frequent_groups(&[text], 6, 60);
        let names: Vec<&str> = kept.iter().map(|k| k.0.as_str()).collect();
        assert!(names.contains(&"剩余控制权"));
        assert!(!names.contains(&"控制权") && !names.contains(&"剩余"));
        assert!(names.iter().all(|n| !n.contains("公式") && !n.contains("不算")));
        assert_eq!(kept[0].1, 8);
    }

    /// A staging folder with `sizes.len()` sections of the given lengths (in Chinese characters).
    fn staging(sizes: &[usize]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let sections = dir.path().join("sections");
        std::fs::create_dir_all(&sections).unwrap();
        for (i, n) in sizes.iter().enumerate() {
            let body = format!("**剩余控制权**是指契约没有写明的权利。{}", "企".repeat(*n));
            std::fs::write(sections.join(format!("s{:03}.md", i + 1)), body).unwrap();
        }
        std::fs::write(
            dir.path().join("manifest.json"),
            r#"{"citation_key":"K","book_title":"书","groups":[{"n":1,"chapter":"第一章","headings":["一"]}]}"#,
        )
        .unwrap();
        dir
    }

    // The owner, 2026-11-01: a long book is split among several explorers by its length, in reading
    // order; a short paper gets one. Every part lists only its own sections' candidates.
    #[test]
    fn a_book_is_split_among_explorers_by_its_length() {
        let long = staging(&[20000, 20000, 20000, 5000]);
        let s = long.path().to_string_lossy().to_string();
        let data = claimed_parts(&s, 160, 30000).unwrap();
        let parts = data["parts"].as_array().unwrap();
        let files: Vec<Vec<String>> = parts
            .iter()
            .map(|p| p["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect())
            .collect();
        assert_eq!(files, [vec!["s001.md"], vec!["s002.md"], vec!["s003.md", "s004.md"]]);
        let third = parts[2]["job_text"].as_str().unwrap();
        assert!(third.starts_with("# 术语探索任务 3/3：《书》"), "{third}");
        assert!(third.contains("`sections/s003.md` 到 `sections/s004.md`（2 个小节）"), "{third}");
        assert!(third.contains("sections/s003.md:1") && !third.contains("sections/s001.md:1"), "a part lists only its own candidates: {third}");
        assert!(third.contains(&explore_name(3)), "{third}");
        assert!(data["merge_instructions_text"].as_str().unwrap().contains("术语合并任务说明"));
        assert!(!data["instructions_text"].as_str().unwrap().contains("20–60"), "the explorer has no term range");
        let short = staging(&[3000, 4000]);
        let one = claimed_parts(&short.path().to_string_lossy(), 160, 30000).unwrap();
        assert_eq!(one["parts"].as_array().unwrap().len(), 1);
    }

    // The merge job sees every explorer row, grouped by term (case ignored), with the part that
    // proposed it, so it can keep one row per concept.
    #[test]
    fn the_merge_job_lists_every_explorer_row_grouped_by_term() {
        let rows = vec![
            (1, json!({"term":"GDP","avoid":["国内生产总值"],"meaning":"总产出","section":"§1"})),
            (2, json!({"term":"敲竹杠","avoid":[],"meaning":"机会主义","section":"§3"})),
            (3, json!({"term":"gdp","avoid":[],"meaning":"产出|总量","section":"§2"})),
            (3, json!({"term":"  ","avoid":[]})),
        ];
        let text = merge_text("书", &rows, 3);
        assert!(text.contains("3 个探索代理各读了本书的一段，共提出 4 行候选，合为 2 个不同的词。"), "{text}");
        let gdp = text.find("| GDP | 国内生产总值 | 总产出 | §1 | 探索 1 |").unwrap();
        let low = text.find("| gdp | — | 产出 总量 | §2 | 探索 3 |").unwrap();
        let other = text.find("| 敲竹杠 |").unwrap();
        assert!(gdp < low && low < other, "rows of one term stay together: {text}");
    }
}
