//! `zt-digest figures -Step explain-start|explain-check|explain-finish`: the explanation pass
//! (docs/figure-explanation-design.md). DeepSeek looks at each figure and says what it shows; the
//! explanation is saved beside the image in the Scholar Archive, so it belongs to the image and to no
//! digest. The pass needs no staging, no ledger and no Card.
//!
//!   explain-start   settles the answers a stopped pass left, then keeps each document's figures, takes
//!                   those without an explanation for their current bytes, and writes one job per four images
//!                   (one job list per manager). The agent gets the images, their captions and the book text
//!                   around them, never the converter's OCR block.
//!   explain-check   the job's SELF-CHECK, and the finish's gate: names, kinds, length, Chinese, no OCR copy.
//!   explain-finish  checks every answer, saves the good ones through `zt scholar explain-save`, and answers how
//!                   many images stay open. The module runs start, queue and finish once more for those.

use crate::args::Args;
use crate::figures::{self, Figure};
use crate::ps::{self, s};
use crate::util;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
use std::sync::LazyLock;

/// Images of one job, the most one `deepseek_run` takes: 317 figures need about 40 jobs and 5 Haiku managers.
const IMAGES_PER_JOB: usize = 8;
const JOBS_PER_MANAGER: usize = 8;
const KINDS: [&str; 8] = ["chart", "map", "photo", "diagram", "table", "text", "logo", "other"];
const MIN_CHINESE: usize = 20;
const MIN_CHINESE_LOGO: usize = 6;
const MAX_CHARS: usize = 1500;
const MAX_TITLE: usize = 40;
/// A run of this many shared characters counts as copied OCR.
const OCR_RUN: usize = 8;
const CONTEXT_CHARS: usize = 500;

static CJK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\x{3400}-\x{9FFF}\x{F900}-\x{FAFF}]").unwrap());

pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Work", "FileIds", "FileId", "Book", "Source", "Job", "Model", "MinFigureBytes", "CitationKey"];
    let a = Args::parse(argv, &params, &[])?;
    let work = a.required("Work")?;
    match a.str("Step").as_str() {
        "explain-start" => start(&a, &work),
        "explain-check" => check_job(&work, &a.required("Job")?),
        "explain-finish" => finish(&work, &a.required("Model")?),
        other => bail!("unknown figures step `{other}`"),
    }
}

/// The documents of this pass: `-FileIds`, or the one book a digest names.
fn file_ids(a: &Args, work: &str) -> Result<Vec<i64>> {
    let mut ids: Vec<i64> = Vec::new();
    for id in a.str("FileIds").split([',', ';']).map(str::trim).filter(|id| !id.is_empty()) {
        ids.push(id.parse().with_context(|| format!("`{id}` is not a file id"))?);
    }
    let one = a.int_or("FileId", 0)?;
    if one != 0 {
        ids.push(one);
    }
    let book = a.str("Book");
    if ids.is_empty() && !book.trim().is_empty() {
        let mut zt = crate::zt::start()?;
        let explicit = crate::prepare::resolve::Explicit {
            source: a.str("Source"),
            source_given: a.has("Source"),
            file_id: 0,
            citation_key: a.str("CitationKey"),
            citation_key_given: a.has("CitationKey"),
            bibtex: String::new(),
            bibtex_given: false,
            book_title: String::new(),
            book_title_given: false,
            start: None,
            end: None,
            skip: None,
            continues: false,
        };
        // An unresolved choice is the preparation's to report: this pass then has nothing to read.
        if let crate::prepare::resolve::Outcome::Ready(resolved) = crate::prepare::resolve::run(&mut zt, work, book.trim(), &explicit)? {
            ids.push(resolved.file_id);
        }
    } else if ids.is_empty()
        && let Some(id) = figures::file_id_in_path(&a.str("Source"))
    {
        ids.push(id);
    }
    Ok(ids)
}

/// One image of a job, as the job's scope file keeps it. `ocr` stays here: the agent never reads this file.
fn scope_image(file_id: i64, figure: &Figure, ocr: &str) -> Value {
    json!({ "file_id": file_id, "path": figure.path, "sha256": figure.sha256, "ocr": ocr })
}

fn job_text(work: &str, id: &str, title: &str, images: &[(Figure, String, String)]) -> String {
    let mut body = format!(
        "# 图像说明任务 {id}\n\n书：{title}\n\n本任务附带 {} 张图像，顺序与下面的编号相同。\n\n答案路径：`{}`\nSELF-CHECK: zt-digest figures -Step explain-check -Work \"{work}\" -Job {id}\n",
        images.len(),
        ps::join_path(work, &format!("out/{id}.json")),
    );
    for (k, (figure, caption, around)) in images.iter().enumerate() {
        let label = if figure.labels.is_empty() { "（书中没有编号）".to_string() } else { format!("图 {}", figure.labels.join("、")) };
        body.push_str(&format!("\n## 第 {} 张：{label}\n\n图注：{}\n\n上下文：{}\n", k + 1, if caption.is_empty() { "（无）" } else { caption }, if around.is_empty() { "（无）" } else { around }));
    }
    body
}

/// The book text around an image, with the converter's blocks gone: what the reader sees beside the figure.
fn context_of(text: &str, name: &str) -> String {
    let blocks: Vec<&str> = text.split("\n\n").map(str::trim).filter(|b| !b.is_empty()).collect();
    let Some(at) = blocks.iter().position(|block| block.lines().any(|line| figures::image_name(line).as_deref() == Some(name))) else {
        return String::new();
    };
    let plain = |block: &&&str| figures::image_name(block.lines().next().unwrap_or_default()).is_none();
    let heading = blocks[..at].iter().rev().find(|block| block.starts_with('#')).map(|block| block.trim_start_matches('#').trim().to_string());
    let before: Vec<&str> = blocks[at.saturating_sub(2)..at].iter().filter(plain).copied().collect();
    let after: Vec<&str> = blocks[at + 1..(at + 3).min(blocks.len())].iter().filter(plain).copied().collect();
    let mut parts: Vec<String> = heading.into_iter().map(|h| format!("（{h}）")).collect();
    parts.push(cut_chars(&before.join(" "), CONTEXT_CHARS));
    parts.push(cut_chars(&after.join(" "), CONTEXT_CHARS));
    parts.retain(|part| !part.is_empty());
    parts.join(" … ")
}

fn cut_chars(text: &str, most: usize) -> String {
    text.chars().take(most).collect()
}

/// The saved explanations are the pass's record, so each job is written once; a new start clears the job folders.
fn start(a: &Args, work: &str) -> Result<i32> {
    for folder in ["jobs", "scope", "out"] {
        std::fs::create_dir_all(ps::join_path(work, folder))?;
    }
    // A pass that stopped after its readers: their answers are saved, not written again.
    let settled = settle(work, a.str("Model").as_str())?;
    for name in ["jobs", "scope", "out"] {
        std::fs::remove_dir_all(ps::join_path(work, name))?;
        std::fs::create_dir_all(ps::join_path(work, name))?;
    }
    let min_bytes = a.int_or("MinFigureBytes", 3000)?;
    let mut jobs: Vec<Value> = Vec::new();
    let mut total = 0usize;
    let instructions = ps::join_path(work, "instructions.md");
    util::write_atomic(&instructions, include_str!("../../../scripts/book-digest/dsh-figure-explain-instructions.zh.md"))?;
    for file_id in file_ids(a, work)? {
        let source = figures::document_path(file_id)?;
        let raw = util::read_text(&source)?.replace("\r\n", "\n");
        let kept = figures::book_figures(&raw, file_id, min_bytes)?;
        total += kept.iter().count();
        let mut ordered: Vec<(String, Figure)> = kept.iter().map(|(name, figure)| (name.clone(), figure.clone())).collect();
        ps::ps_sort(&mut ordered, |x, y| x.1.position.cmp(&y.1.position));
        let ocr = figures::converter_blocks(&raw);
        let text = figures::strip_converter_blocks(&raw);
        let title = source.rsplit(['/', '\\']).next().unwrap_or_default().to_string();
        let open: Vec<(Figure, String, String)> = ordered
            .iter()
            .filter(|(_, figure)| figures::current_explanation(figure).is_none())
            .map(|(name, figure)| (figure.clone(), figures::caption_of(&text, name), context_of(&text, name)))
            .collect();
        for (k, chunk) in open.chunks(IMAGES_PER_JOB).enumerate() {
            let id = format!("f{file_id}-{:03}", k + 1);
            let job_file = ps::join_path(work, &format!("jobs/{id}.md"));
            util::write_atomic(&job_file, &job_text(work, &id, &title, chunk))?;
            let scope: Vec<Value> = chunk
                .iter()
                .map(|(figure, _, _)| {
                    let name = s(&figure.path).rsplit(['/', '\\']).next().unwrap_or_default().to_string();
                    scope_image(file_id, figure, ocr.iter().find(|(image, _)| ps::eq(image, &name)).map_or("", |(_, text)| text.as_str()))
                })
                .collect();
            util::write_atomic(&ps::join_path(work, &format!("scope/{id}.json")), &serde_json::to_string(&json!({ "images": scope }))?)?;
            jobs.push(json!({
                "id": id,
                "files": [instructions, job_file],
                "images": chunk.iter().map(|(figure, _, _)| figure.stored.clone()).collect::<Vec<_>>(),
                "prompt": "看附带的每一张图像，按任务说明写 JSON 答案，并运行 SELF-CHECK。",
                "output": ps::join_path(work, &format!("out/{id}.json")),
            }));
        }
    }
    let chunks = jobs.len().div_ceil(JOBS_PER_MANAGER);
    for (k, chunk) in jobs.chunks(JOBS_PER_MANAGER).enumerate() {
        util::write_atomic(&ps::join_path(work, &format!("jobs-{:03}.json", k + 1)), &serde_json::to_string_pretty(chunk)?)?;
    }
    let file = ps::join_path(work, "jobs.json");
    util::write_atomic(&file, &serde_json::to_string_pretty(&jobs)?)?;
    util::print_line(&util::compact(&json!({ "ok": true, "figures": total, "settled": settled, "jobs": jobs.len(), "chunks": chunks, "file": file })))?;
    Ok(0)
}

fn normal(text: &str) -> Vec<char> {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// The share of the explanation's characters that lie in runs of `OCR_RUN` shared with the OCR text.
fn ocr_share(explanation: &str, ocr: &str) -> f64 {
    let (text, ocr) = (normal(explanation), normal(ocr));
    if text.len() < OCR_RUN || ocr.len() < OCR_RUN {
        return 0.0;
    }
    let runs: HashSet<&[char]> = ocr.windows(OCR_RUN).collect();
    let mut covered = vec![false; text.len()];
    for (at, window) in text.windows(OCR_RUN).enumerate() {
        if runs.contains(window) {
            covered[at..at + OCR_RUN].iter_mut().for_each(|c| *c = true);
        }
    }
    covered.iter().filter(|c| **c).count() as f64 / text.len() as f64
}

/// The faults of one image's answer.
fn image_faults(answer: &Value, ocr: &str) -> Vec<String> {
    let (kind, title, text) = (s(&answer["kind"]), s(&answer["title"]), s(&answer["explanation"]));
    let mut faults = Vec::new();
    if !KINDS.contains(&kind.as_str()) {
        faults.push(format!("kind `{kind}` is not one of {}", KINDS.join(", ")));
    }
    if title.trim().is_empty() || title.chars().count() > MAX_TITLE {
        faults.push(format!("title must have 1 to {MAX_TITLE} characters"));
    }
    let chinese = text.chars().filter(|c| CJK.is_match(c.encode_utf8(&mut [0; 4]))).count();
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    let least = if kind == "logo" { MIN_CHINESE_LOGO } else { MIN_CHINESE };
    if chinese < least {
        faults.push(format!("the explanation has {chinese} Chinese characters; write at least {least}"));
    } else if (chinese as f64) < 0.4 * letters as f64 {
        faults.push("the explanation must be mainly Chinese: say what the image shows, do not copy its text".to_string());
    }
    if text.chars().count() > MAX_CHARS {
        faults.push(format!("the explanation is over {MAX_CHARS} characters"));
    }
    if ocr_share(&text, ocr) >= 0.5 {
        faults.push("ocr_copy: the explanation repeats the text inside the image; say what the image shows".to_string());
    }
    faults
}

/// One job's answers against its scope: (image number, the answer's faults, the answer) per image, with the job's faults.
fn job_answers(work: &str, id: &str) -> Result<(Vec<(Value, Value, Vec<String>)>, Vec<String>)> {
    let scope = util::read_json(ps::join_path(work, &format!("scope/{id}.json")))?;
    let images = ps::ps_items(&scope["images"]);
    let answer = match util::read_json(ps::join_path(work, &format!("out/{id}.json"))) {
        Ok(answer) => answer,
        Err(error) => return Ok((Vec::new(), vec![format!("the answer cannot be read: {error}")])),
    };
    let Some(given) = answer["images"].as_array() else {
        return Ok((Vec::new(), vec!["the answer has no `images` list".to_string()]));
    };
    let mut rows = Vec::new();
    let mut faults = Vec::new();
    for (k, image) in images.iter().enumerate() {
        let number = (k + 1) as u64;
        let matching: Vec<&Value> = given.iter().filter(|row| row["image"].as_u64() == Some(number)).collect();
        match matching.as_slice() {
            [row] => {
                let own = image_faults(row, &s(&image["ocr"]));
                faults.extend(own.iter().map(|fault| format!("image {number}: {fault}")));
                rows.push((image.clone(), (*row).clone(), own));
            }
            [] => faults.push(format!("image {number} has no answer")),
            _ => faults.push(format!("image {number} has more than one answer")),
        }
    }
    for row in given {
        if !row["image"].as_u64().is_some_and(|number| (1..=images.len() as u64).contains(&number)) {
            faults.push(format!("`{}` is not an image of this job", s(&row["image"])));
        }
    }
    Ok((rows, faults))
}

fn check_job(work: &str, id: &str) -> Result<i32> {
    let (_, faults) = job_answers(work, id)?;
    let mut lines = faults.clone();
    lines.push(format!("faults: {}", faults.len()));
    util::print_lines(&lines)?;
    Ok(if faults.is_empty() { 0 } else { 1 })
}

/// `zt scholar explain-save`: the archive's answer to one document's explanations.
fn save(file_id: i64, items: &[Value]) -> Result<Value> {
    let path = std::env::temp_dir().join(format!("zt-digest-explanations-{}-{file_id}.json", std::process::id()));
    std::fs::write(&path, serde_json::to_string(&json!({ "items": items }))?)?;
    let said = std::process::Command::new("zt").args(["scholar", "explain-save", &file_id.to_string(), "--in"]).arg(&path).output().context("zt did not start")?;
    let _ = std::fs::remove_file(&path);
    if !said.status.success() {
        bail!("zt scholar explain-save {file_id} failed: {}", String::from_utf8_lossy(&said.stderr).trim());
    }
    ps::parse_json(&String::from_utf8_lossy(&said.stdout)).context("zt scholar explain-save printed no JSON")
}

/// Saves every good answer on disk. Answers: how many were saved, and how many images stay open.
fn settle_counts(work: &str, model: &str) -> Result<(usize, usize, Vec<String>)> {
    let mut by_file: BTreeMap<i64, Vec<Value>> = BTreeMap::new();
    let (mut asked, mut refused) = (0usize, Vec::new());
    for name in ps::child_files(&ps::join_path(work, "scope"), "", ".json") {
        let id = name.rsplit(['/', '\\']).next().unwrap_or_default().trim_end_matches(".json").to_string();
        asked += ps::ps_count(&util::read_json(ps::join_path(work, &format!("scope/{id}.json")))?["images"]);
        let (rows, _) = job_answers(work, &id)?;
        for (image, answer, faults) in rows {
            if faults.is_empty() {
                by_file.entry(ps::int(&image["file_id"])?).or_default().push(json!({
                    "path": image["path"], "image_sha256": image["sha256"], "kind": answer["kind"],
                    "title": s(&answer["title"]).trim(), "explanation": s(&answer["explanation"]).trim(), "made_by": model,
                }));
            }
        }
    }
    let mut saved = 0usize;
    for (file_id, items) in by_file {
        for chunk in items.chunks(32) {
            let answer = save(file_id, chunk)?;
            saved += ps::ps_count(&answer["saved"]);
            for row in ps::ps_items(&answer["refused"]) {
                // An image another pass explained is done.
                if s(&row["error"]) == "explanation_exists" {
                    saved += 1;
                } else {
                    refused.push(format!("{}: {}", s(&row["path"]), s(&row["error"])));
                }
            }
        }
    }
    Ok((saved, asked.saturating_sub(saved), refused))
}

fn settle(work: &str, model: &str) -> Result<usize> {
    if ps::child_files(&ps::join_path(work, "out"), "", ".json").is_empty() {
        return Ok(0);
    }
    if model.is_empty() {
        bail!("explain-start found answers to save: pass -Model");
    }
    Ok(settle_counts(work, model)?.0)
}

fn finish(work: &str, model: &str) -> Result<i32> {
    let (saved, open, refused) = settle_counts(work, model)?;
    // The saved answers are done: a new start must not save them again.
    let out = ps::join_path(work, "out");
    if ps::exists(&out) {
        std::fs::remove_dir_all(&out)?;
    }
    std::fs::create_dir_all(&out)?;
    let first = refused.first().map(|line| ps::cut16(line, 120)).unwrap_or_default();
    util::print_line(&util::compact(&json!({ "ok": true, "saved": saved, "open": open, "refused": refused.len(), "first_refused": first })))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_carries_as_many_images_as_one_deepseek_run_takes() {
        assert_eq!(IMAGES_PER_JOB, 8, "the owner's range is 6 to 15; the images input of deepseek_run takes at most 8");
    }

    const GOOD: &str = "这是一幅九世纪巴格达的城区平面图，标出底格里斯河两岸的主要城区和运河走向，圆城位于西岸。";

    #[test]
    fn a_good_explanation_passes() {
        assert!(image_faults(&json!({"kind":"map","title":"巴格达平面图","explanation":GOOD}), "AL-RUSAFA Tsuch of Taha").is_empty());
    }

    #[test]
    fn a_copy_of_the_images_ocr_is_refused() {
        // The live case: a map's OCR list handed back as the explanation.
        let ocr = "AL-SHAMMASIYYA\nAL-RUSĀFA\nTsuch of Tāha\nAL-ḤARBIYYA\nGreater Sarāt Canal\nBĀB AL-MUHAWWAL\n这是城内的主要街区名称和运河名称的列表文字说明内容";
        let copied = json!({"kind":"map","title":"地图","explanation":"AL-SHAMMASIYYA AL-RUSĀFA Tsuch of Tāha AL-ḤARBIYYA 这是城内的主要街区名称和运河名称的列表文字说明内容"});
        let faults = image_faults(&copied, ocr);
        assert!(faults.iter().any(|fault| fault.starts_with("ocr_copy")), "{faults:?}");
        // Place names used in a sentence are no copy.
        let used = json!({"kind":"map","title":"地图","explanation":format!("{GOOD}图中可见 AL-RUSĀFA 与 AL-ḤARBIYYA 两区分居河的两岸。")});
        assert!(image_faults(&used, ocr).is_empty(), "{:?}", image_faults(&used, ocr));
    }

    #[test]
    fn an_english_or_short_or_unknown_answer_is_refused() {
        let english = json!({"kind":"photo","title":"Photo","explanation":"Historic black-and-white photo of a harbor scene with people handling large boats and baskets 没有文字"});
        assert!(image_faults(&english, "").iter().any(|fault| fault.contains("Chinese")));
        assert!(!image_faults(&json!({"kind":"chart","title":"图","explanation":"一张折线图。"}), "").is_empty());
        assert!(image_faults(&json!({"kind":"logo","title":"印章","explanation":"出版社的圆形印章。"}), "").is_empty(), "a mark may be short");
        assert!(image_faults(&json!({"kind":"poster","title":"图","explanation":GOOD}), "").iter().any(|fault| fault.contains("kind")));
        assert!(image_faults(&json!({"kind":"map","title":"","explanation":GOOD}), "").iter().any(|fault| fault.contains("title")));
    }

    #[test]
    fn a_job_answer_covers_each_of_its_images_once() {
        let work = std::env::temp_dir().join(format!("zt-explain-job-{}", std::process::id()));
        let work_text = work.display().to_string();
        std::fs::create_dir_all(work.join("scope")).unwrap();
        std::fs::create_dir_all(work.join("out")).unwrap();
        let scope = json!({"images":[{"file_id":7,"path":"a/1.jpg","sha256":"aa","ocr":""},{"file_id":7,"path":"a/2.jpg","sha256":"bb","ocr":""}]});
        std::fs::write(work.join("scope/j.json"), scope.to_string()).unwrap();
        let row = |n: u64| json!({"image":n,"kind":"map","title":"平面图","explanation":GOOD});
        std::fs::write(work.join("out/j.json"), json!({"images":[row(1),row(2)]}).to_string()).unwrap();
        assert!(job_answers(&work_text, "j").unwrap().1.is_empty());
        std::fs::write(work.join("out/j.json"), json!({"images":[row(1),row(1),row(3)]}).to_string()).unwrap();
        let faults = job_answers(&work_text, "j").unwrap().1;
        assert_eq!(faults.len(), 3, "{faults:?}");
        std::fs::remove_dir_all(&work).unwrap();
    }

    #[test]
    fn a_jobs_text_gives_caption_and_context_and_never_the_ocr_block() {
        let raw = "# 第一章\n\n前文一段。\n\n![](images/a.jpg)\n\n<details>\n<summary>text_image</summary>\n\nGARBLED OCR\n\n</details>\n\nFigure 1 The city\n\n图 1 城市\n\n后文。\n";
        let text = figures::strip_converter_blocks(raw);
        let figure = Figure { labels: vec!["1".into()], ..Figure::default() };
        let around = context_of(&text, "a.jpg");
        let body = job_text("W", "j1", "书名", &[(figure, figures::caption_of(&text, "a.jpg"), around)]);
        assert!(!body.contains("GARBLED"), "{body}");
        assert!(body.contains("图 1") && body.contains("Figure 1 The city") && body.contains("前文一段") && body.contains("第一章"), "{body}");
        assert_eq!(figures::converter_blocks(raw), vec![("a.jpg".to_string(), "GARBLED OCR".to_string())]);
    }
}
