//! The compiler cycle that `event` (event-compiler.ps1) and `finding` (finding-compiler.ps1) share:
//! the staging folder, the owned Cards of a group, state.json, the answer files, zt-check on the
//! drafts, and the four steps plan, precheck, apply, and report. What differs between the two
//! layers (the candidates, the checks of an answer, the body of a Card, the first write run) is
//! the `Layer` trait; the texts that differ are in `Spec`.

use crate::calls;
use crate::canon;
use crate::check::Check;
use crate::model;
use crate::ps::{self, CiMap, NL, join_path};
use crate::util;
use crate::zt::Zt;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

static NULL: Value = Value::Null;
static FAULTS_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^faults: \d+$").unwrap());
static PLACE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:event|finding):(\d+) ").unwrap());

/// The texts and names of one layer.
pub(crate) struct Spec {
    /// The tool name: `zt-digest <tool>`.
    pub tool: &'static str,
    /// The list in an answer file: `{"<list>": [...]}`.
    pub list: &'static str,
    /// The job ids: `<prefix>-NN`.
    pub prefix: &'static str,
    /// The fault code of an answer file that is missing or not JSON.
    pub code0: &'static str,
    /// The fault code of an entry that names no declared candidate.
    pub candidate_code: &'static str,
    /// The default -Work folder under -Staging.
    pub work_dir: &'static str,
    /// The instruction file in the scripts folder.
    pub doc: &'static str,
    /// The heading of a job file: `# <job_title> <id>：《<book>》`.
    pub job_title: &'static str,
    /// What follows the anchor line of a group.
    pub anchor_note: &'static str,
    /// Cards of these types are not listed as a group's Idea Cards.
    pub skip_types: &'static [&'static str],
    pub candidates_heading: &'static str,
    pub prompt: &'static str,
    /// The kind word of the Link an Idea Card gets: `<kind> [[card]]：title。`.
    pub kind: &'static str,
    /// The layer merges into existing Cards (`merged` in the entry, the summary, and the report).
    pub merges: bool,
}

/// What the first write run of one job does.
#[derive(Default)]
pub(crate) struct First {
    pub calls: Vec<Value>,
    /// call index -> the place of the entry it creates
    pub created_by: Vec<(usize, usize)>,
    /// place -> the existing Card the entry is merged into
    pub refs: CiMap<String>,
    pub merged: Vec<String>,
}

pub(crate) trait Layer {
    fn spec(&self) -> &'static Spec;
    /// plan: the number of candidates and the candidates by group (`src`, `text`), in order.
    fn candidates(&self, ctx: &Ctx) -> Result<(usize, Vec<(i64, Vec<(String, String)>)>)>;
    /// plan: the frozen source the candidate enumeration reads, recorded in the accepted plan.
    fn candidate_source(&self, ctx: &Ctx) -> Result<Value>;
    /// The faults of an answer that apply refuses: `<item>:<place> <code> <text>`.
    fn test(&self, ctx: &mut Ctx, items: &[Value]) -> Result<Vec<String>>;
    /// The new Cards as zt-check `--json` entries.
    fn drafts(&self, ctx: &Ctx, items: &[Value]) -> Result<Vec<Value>>;
    /// The first write run: the merges and the creates.
    fn first(&self, ctx: &mut Ctx, items: &[Value], bad: &[usize], made: &CiMap<String>) -> Result<First>;
}

/// A property as PowerShell reads it from a `ConvertFrom-Json` object: the name ignores case.
pub(crate) fn prop<'a>(v: &'a Value, name: &str) -> &'a Value {
    match v.as_object() {
        Some(map) => map.get(name).or_else(|| map.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v)).unwrap_or(&NULL),
        None => &NULL,
    }
}

/// `Get-Field`: `[string]$e[$name]` of an answer entry (a hashtable: the name counts case), or ''.
pub(crate) fn get_field(e: &Value, name: &str) -> String {
    match e.as_object().and_then(|m| m.get(name)) {
        Some(v) => ps::s(v),
        None => String::new(),
    }
}

/// `@($e['supports'] | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ })`.
pub(crate) fn supports(e: &Value) -> Vec<String> {
    let v = e.as_object().and_then(|m| m.get("supports")).unwrap_or(&NULL);
    ps::pipe(v).into_iter().map(|x| ps::s(x).trim().to_string()).filter(|x| !x.is_empty()).collect()
}

/// `'{0:d2}' -f $n`.
/// A job's name: its layer prefix and the groups it covers (`fd-g001`, `fd-g002-g005`). The step's
/// state.json lives across claims, so a name must never be taken again by another set of groups.
pub(crate) fn job_id(prefix: &str, groups: &[i64]) -> String {
    match (groups.first(), groups.last()) {
        (Some(first), Some(last)) if first != last => format!("{prefix}-g{first:03}-g{last:03}"),
        (Some(first), _) => format!("{prefix}-g{first:03}"),
        _ => format!("{prefix}-g000"),
    }
}

pub(crate) fn d2(n: i64) -> String {
    if n < 0 { format!("-{:02}", n.unsigned_abs()) } else { format!("{n:02}") }
}

/// `[int]` of a string (Get-Field's result).
pub(crate) fn int_of(text: &str) -> Result<i64> {
    ps::int(&Value::String(text.to_string()))
}

/// `g001`: the group key a unit, a candidate id and the plan manifest share.
pub(crate) fn group_key(n: i64) -> String {
    format!("g{n:03}")
}

/// The stable id of one planned candidate: its group and its place in that group's
/// enumeration (`g001/c001`). A later plan pass recomputes the same enumeration from
/// the frozen source, so the id names the same sentence.
pub(crate) fn candidate_id(n: i64, index: usize) -> String {
    format!("{}/c{:03}", group_key(n), index + 1)
}

/// The accepted candidate manifest of every planned group, with stable ids and source
/// spans: what the layer's completion verifies against the frozen source.
pub(crate) fn candidate_manifest(by_group: &[(i64, Vec<(String, String)>)]) -> Value {
    let mut map = Map::new();
    for (n, list) in by_group {
        let items = list
            .iter()
            .enumerate()
            .map(|(index, (src, text))| json!({"id": candidate_id(*n, index), "src": src, "text": text}))
            .collect::<Vec<_>>();
        map.insert(group_key(*n), Value::Array(items));
    }
    Value::Object(map)
}

/// The declared candidate span of every group, in order. An answer entry names the
/// candidate it writes, so no text is written for a sentence the plan never listed.
pub(crate) fn declared_spans(layer: &dyn Layer, ctx: &Ctx) -> Result<Vec<(i64, Vec<String>)>> {
    let (_, by_group) = layer.candidates(ctx)?;
    Ok(by_group
        .into_iter()
        .map(|(n, list)| (n, list.into_iter().map(|(src, _)| src).collect()))
        .collect())
}

/// The place of `src` among a group's declared candidates, if any.
pub(crate) fn candidate_at(spans: &[(i64, Vec<String>)], n: i64, src: &str) -> Option<usize> {
    let (_, list) = spans.iter().find(|(group, _)| *group == n)?;
    list.iter().position(|span| span == src)
}

/// The plans folder's accepted manifest: `<work>/plan.json`.
fn plan_path(ctx: &Ctx) -> String {
    join_path(&ctx.work, "plan.json")
}

/// Writes the accepted plan manifest: the candidates of every planned group with stable
/// ids and source spans, and the jobs that carry them. It is keep-by-id and refuses a
/// changed candidate enumeration after its first jobs were planned, so an accepted
/// enumeration cannot be silently shortened by a later plan pass.
fn write_plan(ctx: &Ctx, spec: &Spec, source: &Value, candidates: &Value, jobs: &[Value]) -> Result<Value> {
    let path = plan_path(ctx);
    let staging = std::fs::canonicalize(&ctx.staging)?.to_string_lossy().into_owned();
    let mut plan = if ps::exists(&path) {
        ps::parse_json(&util::read_text(&path)?)?
    } else {
        json!({
            "schema_version": 1,
            "layer": spec.work_dir,
            "tool": spec.tool,
            "staging": staging,
            "candidate_source": source,
            "candidates": {},
            "jobs": []
        })
    };
    if plan["schema_version"] != 1
        || plan["layer"] != spec.work_dir
        || plan["tool"] != spec.tool
        || plan["staging"] != json!(staging)
        || !plan["candidates"].is_object()
        || !plan["jobs"].is_array()
    {
        bail!("the accepted plan manifest {path} belongs to another layer or staging folder");
    }
    if plan["candidate_source"]["kind"] != source["kind"] {
        bail!("the accepted plan candidate source changed; use a new output directory");
    }
    let planned = plan["jobs"].as_array().map(|jobs| !jobs.is_empty()).unwrap_or(false);
    if plan["candidates"] != *candidates {
        if planned {
            bail!("the candidate manifest changed after jobs were planned; use a new output directory");
        }
        plan["candidates"] = candidates.clone();
        plan["candidate_source"] = source.clone();
    }
    let known = plan["jobs"].as_array_mut().expect("jobs is an array");
    for job in jobs {
        let id = ps::s(prop(job, "id"));
        match known.iter().find(|old| ps::s(prop(old, "id")) == id) {
            Some(old) => {
                if old["groups"] != job["groups"] || old["answer"] != job["answer"] {
                    bail!("planned job {id} changed since it was accepted; use a new output directory");
                }
            }
            None => known.push(job.clone()),
        }
    }
    let text = ps::json_pretty(&plan);
    util::write_atomic(&path, &text)?;
    Ok(json!({"path": std::fs::canonicalize(&path)?.to_string_lossy(), "sha256": canon::sha256_hex(text.as_bytes())}))
}

/// The folder of the scripts: ZT_DIGEST_SCRIPTS (the forwarder sets it), else the
/// scripts\book-digest folder of the repository (ZT_DIGEST_REPO, or the one this binary was built from).
fn scripts_dir() -> String {
    match std::env::var("ZT_DIGEST_SCRIPTS") {
        Ok(dir) if !dir.is_empty() => dir.trim_end_matches(['\\', '/']).to_string(),
        _ => model::Scripts::find().dir,
    }
}

/// zt-check `--json` in-process. A JSON check reads no Card, so it needs no `zt mcp`.
fn check_json(path: &str) -> Result<Vec<String>> {
    let check = Check { json: path.to_string(), ..Check::default() };
    let lines = match zt_check::check::run_lines(&check.args(), None) {
        Ok(lines) => lines,
        Err(error) => bail!("zt-check did not run: {error:#}"),
    };
    if !lines.iter().any(|l| FAULTS_LINE.is_match(l)) {
        bail!("zt-check did not run: {}", lines.join(" | "));
    }
    Ok(lines)
}

/// The order in which a PowerShell `@{}` keyed by Int32 values (a .NET Hashtable) enumerates its
/// entries, given the keys in insertion order (no duplicates, nothing removed): indices into `keys`.
/// The Hashtable starts with 3 buckets, grows to the next prime above twice its size when it holds
/// 72 % of it, places a key by double hashing (Int32's hash is the value), and enumerates its
/// buckets from the last to the first.
pub(crate) fn int_hashtable_order(keys: &[i64]) -> Vec<usize> {
    const PRIMES: &[usize] = &[
        3, 7, 11, 17, 23, 29, 37, 47, 59, 71, 89, 107, 131, 163, 197, 239, 293, 353, 431, 521, 631, 761, 919, 1103, 1327, 1597, 1931, 2333, 2801,
        3371, 4049, 4861, 5839, 7013, 8419, 10103, 12143, 14591, 17519, 21023, 25229, 30293, 36353, 43627, 52361, 62851, 75431, 90523,
    ];
    fn is_prime(n: usize) -> bool {
        if n % 2 == 0 {
            return n == 2;
        }
        let mut d = 3;
        while d * d <= n {
            if n % d == 0 {
                return false;
            }
            d += 2;
        }
        true
    }
    fn get_prime(min: usize) -> usize {
        if let Some(&p) = PRIMES.iter().find(|&&p| p >= min) {
            return p;
        }
        let mut i = min | 1;
        while !(is_prime(i) && (i - 1) % 101 != 0) {
            i += 2;
        }
        i
    }
    const COLL: u32 = 0x8000_0000;
    // (key index, hash_coll)
    fn put(buckets: &mut [(Option<usize>, u32)], index: usize, hash: u32) {
        let len = buckets.len() as u64;
        let incr = 1 + (hash.wrapping_mul(101) as u64 % (len - 1));
        let mut bn = hash as u64 % len;
        loop {
            let slot = &mut buckets[bn as usize];
            if slot.0.is_none() {
                slot.0 = Some(index);
                slot.1 |= hash;
                return;
            }
            slot.1 |= COLL;
            bn = (bn + incr) % len;
        }
    }
    let hash_of = |k: i64| (k as i32 as u32) & 0x7FFF_FFFF;
    let mut buckets: Vec<(Option<usize>, u32)> = vec![(None, 0); 3];
    let mut loadsize = (0.72f32 * 3.0) as usize;
    for (index, &k) in keys.iter().enumerate() {
        if index >= loadsize {
            let size = get_prime(2 * buckets.len());
            let mut grown: Vec<(Option<usize>, u32)> = vec![(None, 0); size];
            for &(key, hash_coll) in &buckets {
                if let Some(key) = key {
                    put(&mut grown, key, hash_coll & 0x7FFF_FFFF);
                }
            }
            buckets = grown;
            loadsize = (0.72f32 * size as f32) as usize;
        }
        put(&mut buckets, index, hash_of(k));
    }
    buckets.iter().rev().filter_map(|(key, _)| *key).collect()
}

/// The text of one job file.
pub(crate) struct GroupBlock {
    pub n: i64,
    pub chapter: String,
    pub headings: String,
    /// (address, title) of the group's own Cards that are listed
    pub cards: Vec<(String, String)>,
    pub candidates: Vec<(String, String)>,
}

pub(crate) fn job_text(spec: &Spec, id: &str, book_title: &str, answer: &str, self_check: &str, key: &str, groups: &[GroupBlock]) -> String {
    let mut sb = String::new();
    let mut line = |t: &str| {
        sb.push_str(t);
        sb.push_str(NL);
    };
    line(&format!("# {} {id}：《{book_title}》", spec.job_title));
    line("");
    line(&format!("把结果写入 `{answer}`。"));
    line("");
    line(self_check);
    line("");
    for g in groups {
        line(&format!("## 第 {} 组：{}：{}", g.n, g.chapter, g.headings));
        line("");
        line(&format!("锚点：`{key}/{}`{}", g.n, spec.anchor_note));
        line("");
        line("本组的观点卡（supports 只能取这些地址）：");
        line("");
        for (a, title) in &g.cards {
            line(&format!("- `{a}` {title}"));
        }
        line("");
        line(spec.candidates_heading);
        line("");
        for (src, text) in &g.candidates {
            line(&format!("- `{src}` {text}"));
        }
        line("");
    }
    sb
}

/// The precheck command a job file names.
pub(crate) fn self_check_line(spec: &Spec, staging: &str, work: &str, id: &str) -> String {
    format!("SELF-CHECK: zt-digest {} -Step precheck -Staging \"{staging}\" -Work \"{work}\" -Job {id}", spec.tool)
}

/// The Link line an Idea Card gets, and the part of it that marks it as written: `[[card]]`.
pub(crate) fn link_line(kind: &str, card: &str, title: &str) -> String {
    format!("{kind} [[{card}]]：{}。", title.trim().trim_end_matches('。'))
}

/// `$_ -replace '^<kind> (\[\[[^\]]+\]\]).*$', '$1'`.
pub(crate) fn link_mark(kind: &str, line: &str) -> String {
    let re = Regex::new(&format!(r"^{} (\[\[[^\]]+\]\]).*$", regex::escape(kind))).unwrap();
    match re.captures(line) {
        Some(c) => c[1].to_string(),
        None => line.to_string(),
    }
}

/// The run of one step: the staging folder, the work folder, state.json, and one `zt mcp`.
pub(crate) struct Ctx {
    pub staging: String,
    pub work: String,
    pub out: String,
    pub state_file: String,
    pub key: String,
    pub manifest: Value,
    zt: Option<Zt>,
}

impl Ctx {
    /// Reads manifest.json and makes the work folder and its out folder (every step does).
    pub fn open(spec: &Spec, staging: &str, work: &str) -> Result<Self> {
        let raw = util::read_text(join_path(staging, "manifest.json"))?;
        let manifest = ps::parse_json(&raw)?;
        let key = ps::s(prop(&manifest, "citation_key"));
        let work = if work.is_empty() { join_path(staging, spec.work_dir) } else { work.to_string() };
        let out = join_path(&work, "out");
        std::fs::create_dir_all(&work)?;
        std::fs::create_dir_all(&out)?;
        let state_file = join_path(&work, "state.json");
        Ok(Self { staging: staging.to_string(), work, out, state_file, key, manifest, zt: None })
    }

    pub fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().unwrap())
    }

    /// `Invoke-Zt` with get_card on each address: the answers that are objects, in order.
    pub fn get_cards(&mut self, addresses: &[String]) -> Result<Vec<Value>> {
        let calls: Vec<(&str, Value)> = addresses.iter().map(|a| ("get_card", json!({ "at": a }))).collect();
        model::read_calls(self.zt()?, &calls)
    }

    /// `@($manifest.groups)` as the pipeline sees it.
    pub fn groups(&self) -> Vec<&Value> {
        ps::pipe(prop(&self.manifest, "groups"))
    }

    /// `@($manifest.groups | Where-Object { [int]$_.n -eq $n })[0]`.
    pub fn group(&self, n: i64) -> Result<Option<&Value>> {
        for g in self.groups() {
            if ps::int(prop(g, "n"))? == n {
                return Ok(Some(g));
            }
        }
        Ok(None)
    }

    /// `Get-Owned`: the addresses the group's digest run logged, then its anchor.
    pub fn owned(&self, n: i64) -> Result<Vec<String>> {
        let file = join_path(&self.staging, &format!("owned-g{}.txt", d2(n)));
        let mut list: Vec<String> = Vec::new();
        if ps::exists(&file) {
            list = ps::read_lines(&file)?.into_iter().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
        }
        list.push(format!("{}/{n}", self.key));
        Ok(ps::select_unique(list))
    }

    /// `Get-State`.
    pub fn load_state(&self) -> Result<Value> {
        let mut s = if ps::exists(&self.state_file) {
            ps::parse_json(&util::read_text(&self.state_file)?)?
        } else {
            json!({ "applied": {}, "refused": [] })
        };
        let Some(map) = s.as_object_mut() else { bail!("{} holds no JSON object", self.state_file) };
        if !map.contains_key("partial") {
            map.insert("partial".into(), json!({}));
        }
        Ok(s)
    }

    pub fn save_state(&self, s: &Value) -> Result<()> {
        util::write_text(&self.state_file, &ps::json_pretty(s))
    }

    /// `Read-Answer`: `@($raw.<list>)`, or the reason the file is not JSON.
    pub fn read_answer(&self, spec: &Spec, path: &str) -> std::result::Result<Vec<Value>, String> {
        let raw = util::read_text(path).map_err(|e| format!("{e:#}"))?;
        if raw.trim().is_empty() {
            return Err("the file is empty".into());
        }
        let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let list = value.as_object().and_then(|m| m.get(spec.list)).unwrap_or(&NULL);
        Ok(ps::ps_array(list))
    }

    /// `Get-DraftFaults`: zt-check on the drafts (written to drafts-<tag>.json), without `faults: N`.
    pub fn draft_faults(&self, drafts: Vec<Value>, tag: &str) -> Result<Vec<String>> {
        if drafts.is_empty() {
            return Ok(Vec::new());
        }
        let file = join_path(&self.work, &format!("drafts-{tag}.json"));
        util::write_text(&file, &ps::json_pretty(&Value::Array(drafts)))?;
        Ok(check_json(&file)?.into_iter().filter(|l| !FAULTS_LINE.is_match(l)).collect())
    }

    /// `Invoke-Helper`: the calls through the helper with `-Log <Work>\created.txt`; the result
    /// lines. A failed call (or a helper that died) sets `error`.
    fn write(&mut self, calls: &[Value], error: &mut String) -> Result<Vec<String>> {
        self.race(calls)?;
        let mut options = calls::Options::new();
        options.log = join_path(&self.work, "created.txt");
        match calls::run(self.zt()?, calls, &mut options) {
            Ok((done, failure)) => {
                let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
                if let Some(failure) = failure {
                    let failed = failure.lines();
                    *error = failed[0].clone();
                    lines.extend(failed);
                }
                Ok(lines)
            }
            Err(_) => {
                // The helper died (zt mcp closed): no lines, exit 1. A new zt mcp for what comes next.
                self.zt = None;
                *error = "exit 1".into();
                Ok(Vec::new())
            }
        }
    }

    /// A test seam, as tests\racing-zt-mcp.ps1 was for the helper: when ZT_RACE_TARGET names a Card
    /// that this batch edits and the file ZT_RACE_FLAG does not exist yet, it makes that file and
    /// appends ZT_RACE_LINE to the Card first, as a parallel stage would between a read and a write.
    fn race(&mut self, calls: &[Value]) -> Result<()> {
        let var = |n: &str| std::env::var(n).unwrap_or_default();
        let (target, flag, line) = (var("ZT_RACE_TARGET"), var("ZT_RACE_FLAG"), var("ZT_RACE_LINE"));
        if target.is_empty() || flag.is_empty() || ps::exists(&flag) {
            return Ok(());
        }
        if !calls.iter().any(|c| c["tool"] == "edit_card" && ps::eq(&ps::s(&c["arguments"]["at"]), &target)) {
            return Ok(());
        }
        util::write_text(&flag, "")?;
        let card = calls::read(self.zt()?, "get_card", json!({ "at": target }))?;
        let body = format!("{}\n\n{line}", ps::s(&card["body"]).trim_end());
        calls::invoke(self.zt()?, &[json!({ "tool": "edit_card", "arguments": { "at": target, "body": body } })])?;
        Ok(())
    }
}

/// `<step> plan`.
pub(crate) fn plan(layer: &dyn Layer, ctx: &mut Ctx, per_job: i64) -> Result<()> {
    let spec = layer.spec();
    let (count, by_group) = layer.candidates(ctx)?;
    let state = ctx.load_state()?;
    let groups: Vec<i64> = by_group.iter().map(|(n, _)| *n).collect();
    // The titles of every group's own Cards, in one zt call.
    let mut owned: Vec<Vec<String>> = Vec::new();
    let mut all: Vec<String> = Vec::new();
    for &n in &groups {
        let list = ctx.owned(n)?;
        all.extend(list.iter().cloned());
        owned.push(list);
    }
    let mut cards: CiMap<Value> = CiMap::new();
    if !all.is_empty() {
        let got = ctx.get_cards(&all)?;
        for (a, c) in all.iter().zip(got) {
            cards.insert(a, c);
        }
    }
    let doc_copy = join_path(&ctx.work, spec.doc);
    let doc = util::read_text(join_path(&scripts_dir(), spec.doc))?;
    util::write_text(&doc_copy, &doc.replace("{{KEY}}", &ctx.key).replace("{{STAGING}}", &ctx.staging).replace("{{HELPER}}", "zt-digest call"))?;
    if per_job <= 0 && !groups.is_empty() {
        bail!("-GroupsPerJob must be at least 1, not {per_job}");
    }
    let applied = state.get("applied").cloned().unwrap_or(Value::Null);
    let book_title = ps::s(prop(&ctx.manifest, "book_title"));
    let mut jobs: Vec<Value> = Vec::new();
    let mut planned: Vec<Value> = Vec::new();
    let per = per_job.max(1) as usize;
    let mut j = 0usize;
    while j * per < groups.len() {
        let start = j * per;
        j += 1;
        let end = (start + per).min(groups.len());
        let job_groups: Vec<i64> = groups[start..end].to_vec();
        let id = job_id(spec.prefix, &job_groups);
        // The answer does not exist yet at the first plan; its folder does, so the accepted
        // plan records a stable path. A job is recorded even after it was applied, so a plan
        // pass that follows an apply still states which groups each answer covers.
        let answer = std::fs::canonicalize(&ctx.out)
            .map(|dir| join_path(&dir.to_string_lossy(), &format!("{id}.json")))
            .unwrap_or_else(|_| join_path(&ctx.out, &format!("{id}.json")));
        planned.push(json!({ "id": id, "groups": job_groups, "answer": answer }));
        if applied.get(&id).is_some() {
            continue;
        }
        let mut blocks: Vec<GroupBlock> = Vec::new();
        for k in start..end {
            let n = groups[k];
            let g = ctx.group(n)?.cloned().unwrap_or(Value::Null);
            let headings: Vec<String> = ps::pipe(prop(&g, "headings")).into_iter().map(ps::s).collect();
            let mut listed = Vec::new();
            for a in &owned[k] {
                let c = cards.get(a);
                if let Some(c) = c
                    && ps::truthy(c)
                    && ps::is_in(&ps::s(prop(c, "type")), spec.skip_types)
                {
                    continue;
                }
                let title = match c {
                    Some(c) if ps::truthy(c) => ps::s(prop(c, "title")),
                    _ => String::new(),
                };
                listed.push((a.clone(), title));
            }
            blocks.push(GroupBlock {
                n,
                chapter: ps::s(prop(&g, "chapter")),
                headings: headings.join("；"),
                cards: listed,
                candidates: by_group[k].1.clone(),
            });
        }
        let answer = join_path(&ctx.out, &format!("{id}.json"));
        let check = self_check_line(spec, &ctx.staging, &ctx.work, &id);
        let job_file = join_path(&ctx.work, &format!("{id}.md"));
        util::write_text(&job_file, &job_text(spec, &id, &book_title, &answer, &check, &ctx.key, &blocks))?;
        jobs.push(json!({ "id": id, "files": [doc_copy, job_file], "prompt": spec.prompt }));
    }
    let candidates = candidate_manifest(&by_group);
    let source = layer.candidate_source(ctx)?;
    let manifest = write_plan(ctx, spec, &source, &candidates, &planned)?;
    // `applied_jobs` says why `jobs` can be shorter than `expected_jobs`: a later claim's plan lists only the
    // jobs still to run (acemoglu2025nobel trial 1: a relay read `jobs: []` as an inconsistent plan).
    let applied_jobs = planned.len() - jobs.len();
    util::print_line(&ps::json_compact(&json!({ "candidates": count, "groups": groups.len(), "jobs": jobs, "expected_jobs": planned.len(), "applied_jobs": applied_jobs, "manifest": manifest })))
}

/// `<step> precheck`: the faults of out\<Job>.json, then `faults: N`.
pub(crate) fn precheck(layer: &dyn Layer, ctx: &mut Ctx, job: &str) -> Result<()> {
    let spec = layer.spec();
    if job.is_empty() {
        bail!("precheck needs -Job");
    }
    let file = join_path(&ctx.out, &format!("{job}.json"));
    if !ps::exists(&file) {
        return util::print_lines(&[format!("{job} {} no answer file {file}", spec.code0), "faults: 1".into()]);
    }
    let items = match ctx.read_answer(spec, &file) {
        Ok(items) => items,
        Err(message) => return util::print_lines(&[format!("{job} {} the answer is not JSON: {message}", spec.code0), "faults: 1".into()]),
    };
    let mut lines = layer.test(ctx, &items)?;
    let drafts = layer.drafts(ctx, &items)?;
    lines.extend(ctx.draft_faults(drafts, job)?);
    let count = lines.len();
    lines.push(format!("faults: {count}"));
    util::print_lines(&lines)
}

/// `<step> report`: the counts of state.json.
pub(crate) fn report(spec: &Spec, ctx: &Ctx) -> Result<()> {
    let s = ctx.load_state()?;
    let jobs: Vec<Value> = match s.get("applied") {
        Some(Value::Object(m)) => m.values().cloned().collect(),
        _ => Vec::new(),
    };
    let count_of = |name: &str| -> usize { jobs.iter().flat_map(|j| ps::pipe(prop(j, name)).into_iter().cloned().collect::<Vec<_>>()).filter(ps::truthy).count() };
    let mut linked = 0i64;
    for j in &jobs {
        linked += ps::int(prop(j, "linked"))?;
    }
    let partial = s.get("partial").and_then(Value::as_object).cloned().unwrap_or_default();
    let mut errors: Vec<Value> = jobs
        .iter()
        .filter(|j| ps::truthy(prop(j, "error")))
        .map(|j| Value::String(format!("{}: {}", ps::s(prop(j, "id")), ps::s(prop(j, "error")))))
        .collect();
    for (k, v) in &partial {
        errors.push(Value::String(format!("{k}: {}", ps::s(prop(v, "error")))));
    }
    let mut out = Map::new();
    out.insert("jobs".into(), json!(jobs.len()));
    out.insert("created".into(), json!(count_of("created")));
    if spec.merges {
        out.insert("merged".into(), json!(count_of("merged")));
    }
    out.insert("linked".into(), json!(linked));
    out.insert("refused".into(), json!(ps::ps_count(s.get("refused").unwrap_or(&NULL))));
    out.insert("pending".into(), json!(partial.len()));
    out.insert("errors".into(), Value::Array(errors));
    util::print_line(&ps::json_compact(&Value::Object(out)))
}

/// `<step> apply [-Apply]`.
pub(crate) fn apply(layer: &dyn Layer, ctx: &mut Ctx, write: bool) -> Result<()> {
    let spec = layer.spec();
    let mut state = ctx.load_state()?;
    if !state["partial"].is_object() {
        bail!("{}: partial is not an object", ctx.state_file);
    }
    let mut ok = true;
    let (mut applied_jobs, mut created, mut merged, mut linked, mut pending_jobs) = (0i64, 0i64, 0i64, 0i64, 0i64);
    let mut refused: Vec<Value> = Vec::new();
    let mut files = ps::child_files(&ctx.out, &format!("{}-", spec.prefix), ".json");
    ps::ps_sort(&mut files, |a, b| ps::culture_cmp(a, b));
    for name in files {
        let id = match name.rfind('.') {
            Some(dot) => name[..dot].to_string(),
            None => name.clone(),
        };
        let answer_file = join_path(&ctx.out, &name);
        let answer_hash = std::fs::read(&answer_file)
            .map(|bytes| canon::sha256_hex(&bytes))
            .unwrap_or_default();
        let previous = state["applied"].get(&id).cloned();
        if let Some(entry) = &previous {
            if entry["answer"]["sha256"].as_str() == Some(answer_hash.as_str()) && !refused_by_advice(entry) {
                continue;
            }
            // A revised answer is a new attempt at the same job: its Cards are reused by
            // place, so no Card is made twice and the corrected entries can be accepted.
            if let Some(applied) = state["applied"].as_object_mut() {
                applied.shift_remove(&id);
            }
        }
        let items = match ctx.read_answer(spec, &answer_file) {
            Ok(items) => items,
            Err(_) => {
                refused.push(Value::String(format!("{id}: the answer is not JSON")));
                continue;
            }
        };
        // Refuse what the self-check would report: no unchecked text is written.
        let mut faults = layer.test(ctx, &items)?;
        let drafts = layer.drafts(ctx, &items)?;
        faults.extend(ctx.draft_faults(drafts, &id)?);
        let bad = refusing_faults(&faults);
        let keys: Vec<i64> = bad.iter().map(|(p, _)| *p as i64).collect();
        let bad_values: Vec<String> = int_hashtable_order(&keys).into_iter().map(|i| bad[i].1.clone()).collect();
        for b in &bad_values {
            refused.push(Value::String(format!("{id} {b}")));
        }
        if !write {
            continue;
        }
        let answer_path = join_path(&ctx.out, &name);
        let answer_record = json!({
            "path": std::fs::canonicalize(&answer_path)?.to_string_lossy(),
            "sha256": canon::sha256_hex(&std::fs::read(&answer_path)?)
        });
        let spans = declared_spans(layer, ctx)?;
        let group_of = |e: &Value| int_of(&get_field(e, "group")).unwrap_or(0);
        let mut job_groups: Vec<i64> = Vec::new();
        let mut refused_facts: Vec<Value> = Vec::new();
        for (place, fault) in &bad {
            let entry = items.get(place.saturating_sub(1)).cloned().unwrap_or(Value::Null);
            let group = group_of(&entry);
            let src = get_field(&entry, "src").trim().to_string();
            if group > 0 && !job_groups.contains(&group) {
                job_groups.push(group);
            }
            refused_facts.push(json!({
                "place": place,
                "group": group,
                "src": src,
                "candidate": candidate_at(&spans, group, &src).map(|index| candidate_id(group, index)),
                "fault": fault
            }));
        }
        let bad_places: Vec<usize> = bad.iter().map(|(p, _)| *p).collect();
        // A run of this job that failed part way left the Cards it made (place -> address): reuse them,
        // so the rerun makes no Card twice and writes the Links that run missed.
        let partial = state["partial"].get(&id).cloned().unwrap_or_else(|| json!({ "made": {}, "linked": 0 }));
        let mut made: CiMap<String> = CiMap::new();
        if let Some(m) = prop(&partial, "made").as_object() {
            for (p, a) in m {
                made.insert(p, ps::s(a));
            }
        }
        if let Some(entries) = previous.as_ref().and_then(|entry| entry["entries"].as_array()) {
            for entry in entries {
                if let (Some(place), Some(card)) = (entry["place"].as_i64(), entry["card"].as_str()) {
                    made.insert(&place.to_string(), card.to_string());
                }
            }
        }
        let mut entry_linked = ps::int(prop(&partial, "linked"))?;
        let mut error = String::new();
        let mut new = 0i64;

        // First run: the merges (event) and the new Cards.
        let first = layer.first(ctx, &items, &bad_places, &made)?;
        if !first.calls.is_empty() {
            let result = ctx.write(&first.calls, &mut error)?;
            for &(c, place) in &first.created_by {
                let head = format!("{c} ok ");
                if let Some(line) = result.iter().find(|l| l.starts_with(&head)) {
                    made.insert(&place.to_string(), line[head.len()..].trim().to_string());
                    new += 1;
                }
            }
        }

        // Second run: each supported Idea Card links its new Cards once, in one edit. It reads the
        // Cards after the first run, and each edit carries the text it was built on (base), so a Card
        // another stage changed in between is refused, not overwritten; the rerun reads it again.
        if error.is_empty() {
            let mut links: CiMap<Vec<String>> = CiMap::new();
            for (i, e) in items.iter().enumerate() {
                let place = i + 1;
                if bad_places.contains(&place) {
                    continue;
                }
                let key = place.to_string();
                let card = first.refs.get(&key).or_else(|| made.get(&key)).cloned().unwrap_or_default();
                let line = link_line(spec.kind, &card, &get_field(e, "title"));
                for s in supports(e) {
                    let mut list = links.get(&s).cloned().unwrap_or_default();
                    list.push(line.clone());
                    links.insert(&s, list);
                }
            }
            let mut calls: Vec<Value> = Vec::new();
            let mut link_count: Vec<(usize, usize)> = Vec::new();
            if !links.is_empty() {
                let ideas = ctx.get_cards(&links.keys())?;
                for (k, (s, lines)) in links.iter().enumerate() {
                    let idea = ideas.get(k).unwrap_or(&NULL);
                    let body = ps::s(prop(idea, "body"));
                    let text = ps::s(prop(idea, "text"));
                    let add: Vec<&String> = lines.iter().filter(|l| !body.contains(&link_mark(spec.kind, l))).collect();
                    if add.is_empty() {
                        continue;
                    }
                    let joined = add.iter().map(|l| l.as_str()).collect::<Vec<_>>().join("\n");
                    calls.push(json!({ "tool": "edit_card", "arguments": { "at": s, "body": format!("{}\n\n{joined}", body.trim_end()), "base": text } }));
                    link_count.push((calls.len() - 1, add.len()));
                }
            }
            if !calls.is_empty() {
                let result = ctx.write(&calls, &mut error)?;
                for (c, n) in link_count {
                    let head = format!("{c} ok ");
                    if result.iter().any(|l| l.starts_with(&head)) {
                        entry_linked += n as i64;
                    }
                }
            }
        }

        let mut places: Vec<(i64, String)> = Vec::new();
        for (p, a) in made.iter() {
            places.push((int_of(p)?, a.clone()));
        }
        ps::ps_sort(&mut places, |a, b| a.0.cmp(&b.0));
        let created_list: Vec<Value> = places.into_iter().map(|(_, a)| Value::String(a)).collect();
        created += new;
        // Every written entry keeps the candidate it accepted and the Card it produced: the
        // layer's completion rereads these Cards instead of trusting a printed count.
        let mut entries: Vec<Value> = Vec::new();
        if error.is_empty() {
            for (index, e) in items.iter().enumerate() {
                let place = index + 1;
                if bad_places.contains(&place) {
                    continue;
                }
                let key = place.to_string();
                let card = first.refs.get(&key).or_else(|| made.get(&key)).cloned().unwrap_or_default();
                if card.is_empty() {
                    error = format!("{}{place} was written without a returned Card address", spec.prefix);
                    break;
                }
                let group = group_of(e);
                if group > 0 && !job_groups.contains(&group) {
                    job_groups.push(group);
                }
                let src = get_field(e, "src").trim().to_string();
                let candidate = candidate_at(&spans, group, &src).map(|index| candidate_id(group, index));
                let Some(candidate) = candidate else {
                    error = format!("{}{place} src `{src}` is not a declared candidate of group {group}", spec.prefix);
                    break;
                };
                entries.push(json!({
                    "place": place,
                    "group": group,
                    "src": src,
                    "candidate": candidate,
                    "title": get_field(e, "title").trim(),
                    "section": get_field(e, "section").trim(),
                    "supports": supports(e),
                    "merged": first.refs.contains(&key),
                    "card": card
                }));
            }
        }
        let partial_map = state["partial"].as_object_mut().expect("partial is an object");
        if !error.is_empty() {
            // Not applied: the rerun tries the job again with the Cards made so far.
            let mut made_map = Map::new();
            for (p, a) in made.iter() {
                made_map.insert(p.clone(), Value::String(a.clone()));
            }
            partial_map.insert(id.clone(), json!({ "made": made_map, "linked": entry_linked, "error": error }));
            ok = false;
            pending_jobs += 1;
        } else {
            partial_map.shift_remove(&id);
            let mut entry = Map::new();
            entry.insert("id".into(), json!(id));
            entry.insert("created".into(), Value::Array(created_list));
            if spec.merges {
                entry.insert("merged".into(), json!(first.merged));
            }
            entry.insert("linked".into(), json!(entry_linked));
            entry.insert("error".into(), json!(""));
            entry.insert("answer".into(), answer_record);
            entry.insert("groups".into(), json!(job_groups));
            entry.insert("entries".into(), Value::Array(entries));
            entry.insert("refused".into(), Value::Array(refused_facts));
            if !state["applied"].is_object() {
                state["applied"] = json!({});
            }
            state["applied"].as_object_mut().unwrap().insert(id.clone(), Value::Object(entry));
            let mut all = ps::ps_array(state.get("refused").unwrap_or(&NULL));
            all.extend(bad_values.iter().map(|b| Value::String(format!("{id} {b}"))));
            state["refused"] = Value::Array(all);
            applied_jobs += 1;
            merged += first.merged.len() as i64;
            linked += entry_linked;
        }
        ctx.save_state(&state)?;
    }
    let mut summary = Map::new();
    summary.insert("ok".into(), json!(ok));
    summary.insert("applied_jobs".into(), json!(applied_jobs));
    summary.insert("created".into(), json!(created));
    if spec.merges {
        summary.insert("merged".into(), json!(merged));
    }
    summary.insert("linked".into(), json!(linked));
    summary.insert("refused".into(), Value::Array(refused));
    summary.insert("pending_jobs".into(), json!(pending_jobs));
    util::print_line(&ps::json_compact(&Value::Object(summary)))
}

/// The steps of both tools, after the parameters.
pub(crate) fn run(layer: &dyn Layer, step: &str, staging: &str, work: &str, job: &str, per_job: i64, write: bool) -> Result<i32> {
    // R328 (§15.2): the plan allocates the layer's jobs and the apply writes its Cards into the
    // book's Literature tree. Both stop before the first job file or Card when that tree is locked
    // by the owner; `precheck` and `report` read the tree normally and never ask.
    if step == "plan" || (step == "apply" && write) {
        let spec = layer.spec();
        let key = crate::lockcheck::key_from_staging(staging);
        let mut zt = crate::zt::start()?;
        if let Some(refusal) = crate::lockcheck::preflight(&mut zt, spec.tool, step, &key)? {
            return crate::lockcheck::stop(spec.tool, &refusal);
        }
    }
    let mut ctx = Ctx::open(layer.spec(), staging, work)?;
    match step {
        "plan" => plan(layer, &mut ctx, per_job)?,
        "precheck" => precheck(layer, &mut ctx, job)?,
        "report" => report(layer.spec(), &ctx)?,
        _ => apply(layer, &mut ctx, write)?,
    }
    Ok(0)
}

/// The first rule fault of each entry place, in fault order. An "(advisory)" line is advice, never a
/// fault (as in the audit), so it refuses no entry.
fn refusing_faults(faults: &[String]) -> Vec<(usize, String)> {
    let mut bad: Vec<(usize, String)> = Vec::new();
    for f in faults.iter().filter(|f| !f.contains("(advisory)")) {
        if let Some(m) = PLACE.captures(f) {
            let place: usize = m[1].parse().unwrap_or(usize::MAX);
            if !bad.iter().any(|(p, _)| *p == place) {
                bad.push((place, f.clone()));
            }
        }
    }
    bad
}

/// An applied job whose recorded refusals include an advisory line was refused by an older rule
/// (`refusing_faults`): it is applied again like a revised answer, its made Cards reused by place.
fn refused_by_advice(entry: &Value) -> bool {
    entry["refused"]
        .as_array()
        .is_some_and(|refused| refused.iter().any(|r| ps::s(prop(r, "fault")).contains("(advisory)")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn int_keys_enumerate_as_a_dotnet_hashtable() {
        // The orders pwsh 7.6 printed for `$b=@{}; foreach ($k in ...) { $b[$k] = $k }; $b.Values`.
        let order = |keys: &[i64]| int_hashtable_order(keys).into_iter().map(|i| keys[i].to_string()).collect::<Vec<_>>().join(",");
        assert_eq!(order(&[3, 4]), "4,3");
        assert_eq!(order(&[1, 2, 3]), "3,2,1");
        assert_eq!(order(&[4, 3, 1]), "4,3,1");
        assert_eq!(order(&[7, 2, 9, 4, 1, 12, 30, 5]), "30,12,9,7,5,4,2,1");
        assert_eq!(order(&[2, 5, 1, 3]), "5,3,2,1");
        assert_eq!(order(&(1..=20).collect::<Vec<_>>()), "20,19,18,17,16,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1");
        assert_eq!(order(&[15, 3, 8, 22, 1, 4, 9, 10, 11, 40, 2]), "15,11,10,9,8,40,22,4,3,2,1");
        assert_eq!(order(&[]), "");
    }

    // ebrill2001modern trial 1: an advisory L3 line refused finding:1, so the group's unit failed and a
    // new DeepSeek attempt was spent on a draft that broke no rule.
    #[test]
    fn only_rule_faults_refuse_an_entry() {
        let faults = [
            "finding:1 L3 (advisory) [[K/57]] is linked in 2 paragraphs".to_string(),
            "finding:2 F1 the result names no size or direction".to_string(),
            "finding:2 F3 the source line is missing".to_string(),
            "faults: 2".to_string(),
        ];
        assert_eq!(
            refusing_faults(&faults),
            vec![(2, "finding:2 F1 the result names no size or direction".to_string())]
        );
    }

    #[test]
    fn a_job_refused_by_advice_is_applied_again() {
        let advised = json!({ "refused": [{ "place": 1, "fault": "finding:1 L3 (advisory) [[K/57]] is linked in 2 paragraphs" }] });
        let faulted = json!({ "refused": [{ "place": 1, "fault": "finding:1 F1 the result names no size" }] });
        assert!(refused_by_advice(&advised));
        assert!(!refused_by_advice(&faulted));
        assert!(!refused_by_advice(&json!({ "refused": [] })));
        assert!(!refused_by_advice(&json!({})));
    }

    #[test]
    fn link_lines_and_their_marks() {
        let line = link_line("案例", "K/1|a", " 1926 年收购。。 ");
        assert_eq!(line, "案例 [[K/1|a]]：1926 年收购。");
        assert_eq!(link_mark("案例", &line), "[[K/1|a]]");
        assert_eq!(link_mark("印证", "印证 [[]]：x。"), "印证 [[]]：x。");
        assert_eq!(d2(3), "03");
        assert_eq!(d2(12), "12");
        assert_eq!(d2(-1), "-01");
    }

    #[test]
    fn answer_fields_read_as_the_scripts_read_them() {
        let e = json!({ "title": "t", "group": 2, "supports": [" a ", "", null, "b"], "n": null });
        assert_eq!(get_field(&e, "group"), "2");
        assert_eq!(get_field(&e, "Title"), "");
        assert_eq!(get_field(&json!("x"), "title"), "");
        assert_eq!(supports(&e), ["a", "b"]);
        assert_eq!(supports(&json!({ "supports": "c" })), ["c"]);
        assert!(supports(&json!({})).is_empty());
        assert_eq!(ps::s(prop(&json!({ "Citation_Key": "K" }), "citation_key")), "K");
    }

    // A job is named by the groups it covers. The step's state.json lives across claims, so a positional
    // name (fd-01) taken again by a later claim's plan was read as already applied and its jobs were never
    // printed (grossman2002integration trial 1: 3 Finding jobs expected, none run, the unit recorded done).
    #[test]
    fn a_job_is_named_by_the_groups_it_covers() {
        assert_eq!(job_id("fd", &[1]), "fd-g001");
        assert_eq!(job_id("fd", &[2, 3, 4, 5]), "fd-g002-g005");
        assert_eq!(job_id("ev", &[12, 13]), "ev-g012-g013");
        assert_ne!(job_id("fd", &[1]), job_id("fd", &[2, 3]));
    }
}
