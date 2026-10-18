//! `zt-digest card`: the port of `scripts/book-digest/card-compiler.ps1`, the deterministic half
//! of the card layer cycle. It checks the Idea Cards, Event Cards, and anchors of one book,
//! hands their faults to DeepSeek as jobs, checks what DeepSeek writes, applies it, checks again,
//! and reports. DeepSeek writes only JSON files; this tool makes every zt write. It never writes
//! a Model Card, a Model part, a Hub Card, or a Topic root.
//!
//!   plan      runs the audit on -Book (in-process, into <Work>\before) and writes the fix and
//!             orphan jobs
//!   apply     writes the rewrites, the splits, and the orphan Links of the jobs not applied yet
//!   check     runs the audit again (<Work>\after); one follow-up job per fix job with faults left
//!   report    compares the faults before and after; writes report.json and decisions.json
//!   precheck  checks one job's JSON file the way apply and the audit will, without a write
//!
//! The job files name their precheck command as `zt-digest card -Step precheck …` (this tool, no
//! pwsh). <scripts> is ZT_DIGEST_SCRIPTS (the forwarder sets it) or the scripts\book-digest folder
//! of the repository this binary was built from. The instruction
//! files dsh-card-instructions.zh.md and dsh-orphan-instructions.zh.md are read from there too.

pub(crate) mod relatedness;
mod text;

use crate::args::Args;
use crate::audit;
use crate::ps;
use crate::calls;
use crate::check::Check;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use crate::ps::{NL, key as ci, culture_cmp, join_path, leaf, ps_array, eq, is_in, ps_items, ps_sort, field, s, truthy};
use regex::Regex;
use relatedness::Relatedness;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

/// A Finding Card is writable as before the audit gave it its own category.
const WRITABLE: &[&str] = &["idea", "event", "finding", "anchor"];
/// Rules that need the whole Workspace: only the audit checks them.
const WHOLE_WORKSPACE: &[&str] = &["I5", "E4", "I6", "E5", "L3"];
const FIX_PROMPT: &str = "按以上说明处理任务文件中的每张卡，写出 JSON 文件。最后只回复一行 JSON。";
const ORPHAN_PROMPT: &str = "按以上说明为每张孤立卡片选出链接，写出 JSON 文件。最后只回复一行 JSON。";
const FOLLOW_PROMPT: &str = "按以上问题修正这些卡片，写出 JSON 文件。最后只回复一行 JSON。";

static RULE_SPLIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[,\s]+").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^#\s").unwrap());
static SECTION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)§\s?([0-9A-Za-z.]+)").unwrap());
static SEMICOLON: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[；;]").unwrap());
static RULE_NAME: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[RIEL]\d+$").unwrap());
static R_RULE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^R\d+$").unwrap());
static TERMS_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
static OK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+) ok (.+)$").unwrap());
static ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+) error (.*)$").unwrap());

struct Params {
    step: String,
    work: String,
    book: String,
    staging: String,
    groups: String,
    per_job: i64,
    sample: f64,
    seed: i32,
    rules: String,
    max_cards: i64,
    no_orphans: bool,
    notes: String,
    compact: bool,
    max_jobs: i64,
    job: String,
    file: String,
}

/// The audit's view: faults per Card and the Cards the layer rules read (audit-cards.json).
struct AuditView {
    faults: HashMap<String, Vec<(String, String)>>,
    cards: Vec<Value>,
    by: HashMap<String, usize>,
    orphan_check: Value,
}

impl AuditView {
    fn card(&self, address: &str) -> Option<&Value> {
        self.by.get(&ci(address)).map(|&i| &self.cards[i])
    }
    fn all_faults(&self, address: &str) -> &[(String, String)] {
        self.faults.get(&ci(address)).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// The result of one write: the address, or the error.
type Written = std::result::Result<String, String>;

struct Compiler {
    p: Params,
    zt: Option<Zt>,
    state: Value,
    state_file: String,
    out_dir: String,
    scripts: String,
    known: Option<HashSet<String>>,
}

fn read_json_file(path: &str) -> Result<Value> {
    let raw = util::read_text(path)?;
    ps::parse_json(&raw).with_context(|| format!("{path} is not JSON"))
}

fn trim_end(text: &str) -> String {
    text.trim_end().to_string()
}

/// `Format-Card`: a Card of a fix job, with its faults (or the review instruction).
fn format_card(card: &Value, faults: &[(String, String)], parent_title: &str, source: &str) -> String {
    let mut sb = String::new();
    let mut line = |t: &str| {
        sb.push_str(t);
        sb.push_str(NL);
    };
    line(&format!("## 卡片 `{}`（{}）", field(card, "address"), field(card, "category")));
    line("");
    if truthy(&card["parent"]) {
        line(&format!("- 父卡：`{}` {parent_title}", field(card, "parent")));
    }
    if !source.is_empty() {
        line(&format!("- 原文：{source}"));
    }
    if !faults.is_empty() {
        line("- 问题：");
        for (rule, text) in faults {
            line(&format!("  - {rule} {text}"));
        }
    } else {
        line("- 问题：检查脚本没有找到问题。这是抽查：对照原文核实这张卡（ok、fix、split、merge 或 unfaithful）。");
    }
    line("");
    line("### 标题");
    line("");
    line(&field(card, "title"));
    line("");
    line("### 正文");
    line("");
    line("````markdown");
    line(&trim_end(&field(card, "body")));
    line("````");
    line("");
    sb
}

/// Lines of a text file as Get-Content gives them.
fn file_lines(path: &str) -> Result<Vec<String>> {
    Ok(util::read_text(path)?.lines().map(str::to_string).collect())
}

/// The files of a folder matching `prefix*.txt`, in name order (Get-ChildItem -Filter).
fn matching_files(dir: &str, prefix: &str) -> Vec<String> {
    let mut found: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_lowercase().starts_with(prefix) && n.to_lowercase().ends_with(".txt"))
        .collect();
    found.sort_by_key(|n| n.to_uppercase());
    found.into_iter().map(|n| join_path(dir, &n)).collect()
}

/// The folder of the PowerShell scripts: ZT_DIGEST_SCRIPTS, else this repository's scripts\book-digest.
fn scripts_dir() -> String {
    if let Ok(dir) = std::env::var("ZT_DIGEST_SCRIPTS")
        && !dir.is_empty()
    {
        return dir.trim_end_matches(['\\', '/']).to_string();
    }
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = crate_dir.parent().and_then(Path::parent).unwrap_or(crate_dir);
    repo.join("scripts").join("book-digest").to_string_lossy().into_owned()
}

/// zt-check on a --json file, without a `zt mcp` (a JSON check reads no Card).
fn check_offline(check: &Check) -> Result<Vec<String>> {
    let lines = match zt_check::check::run_lines(&check.args(), None) {
        Ok(lines) => lines,
        Err(error) => bail!("zt-check did not run: {error:#}"),
    };
    if !lines.iter().any(|l| l.starts_with("faults: ") && l[8..].chars().all(|c| c.is_ascii_digit())) {
        bail!("zt-check did not run: {}", lines.join(" | "));
    }
    Ok(lines)
}

fn unique_temp(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let name = format!("{prefix}-{}-{:032x}.json", std::process::id(), nanos ^ ((N.fetch_add(1, Ordering::Relaxed) as u128) << 100));
    std::env::temp_dir().join(name).to_string_lossy().into_owned()
}

impl Compiler {
    fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(Zt::start()?);
        }
        Ok(self.zt.as_mut().unwrap())
    }

    fn fix_doc(&self) -> String {
        join_path(&self.scripts, "dsh-card-instructions.zh.md")
    }

    fn orphan_doc(&self) -> String {
        join_path(&self.scripts, "dsh-orphan-instructions.zh.md")
    }

    fn save_state(&self) -> Result<()> {
        util::write_text(&self.state_file, &ps::json_pretty(&self.state))
    }

    fn print_last(value: &Value) -> Result<()> {
        util::print_line(&ps::json_compact(value))
    }

    fn rule_list(&self) -> Vec<String> {
        RULE_SPLIT.split(&field(&self.state, "rules")).filter(|r| !r.is_empty()).map(str::to_string).collect()
    }

    fn hard_faults(&self, audit: &AuditView, address: &str) -> Vec<(String, String)> {
        let rules = self.rule_list();
        audit.all_faults(address).iter().filter(|(rule, _)| is_in(rule, &rules)).cloned().collect()
    }

    fn next_seq(&mut self) -> i64 {
        let seq = self.state["seq"].as_i64().unwrap_or(0) + 1;
        self.state["seq"] = json!(seq);
        seq
    }

    /// `Get-Known`: the addresses of before\audit-addresses.txt.
    fn known(&mut self) -> Result<&HashSet<String>> {
        if self.known.is_none() {
            let path = join_path(&self.p.work, "before\\audit-addresses.txt");
            let set = file_lines(&path)?.into_iter().filter(|a| !a.is_empty()).map(|a| a.trim().to_string()).collect();
            self.known = Some(set);
        }
        Ok(self.known.as_ref().unwrap())
    }

    /// The precheck line of a job file, with absolute paths.
    fn self_check(&self, id: &str) -> Result<String> {
        let dir = std::path::absolute(&self.p.work)?.to_string_lossy().into_owned();
        Ok(format!("SELF-CHECK: zt-digest card -Step precheck -Work \"{dir}\" -Job {id}"))
    }

    fn add_self_check(&self, sb: &mut String, id: &str) -> Result<()> {
        sb.push_str(&format!("写完 JSON 文件后，按说明的“自查”一节运行这条自查命令：{NL}{NL}{}{NL}{NL}", self.self_check(id)?));
        Ok(())
    }

    // --- the audit -------------------------------------------------------------------------------

    fn group_addresses(&self) -> Result<Vec<String>> {
        let staging = field(&self.state, "staging");
        if staging.is_empty() {
            bail!("-Groups needs -Staging (owned-gNN.txt)");
        }
        let book = field(&self.state, "book");
        let groups = field(&self.state, "groups");
        let mut addresses = BTreeSet::new();
        for group in RULE_SPLIT.split(&groups).filter(|g| !g.is_empty()) {
            let n = match group.parse::<i64>() {
                Ok(n) => n,
                Err(_) => group.parse::<f64>().map(ps::ps_int)?,
            };
            addresses.insert(format!("{book}/{n}"));
            let path = join_path(&staging, &format!("owned-g{n:02}.txt"));
            if !Path::new(&path).is_file() {
                bail!("incomplete group input: missing owned manifest `{path}`");
            }
            addresses.extend(
                file_lines(&path)?
                    .into_iter()
                    .map(|line| line.trim().to_string())
                    .filter(|line| !line.is_empty()),
            );
        }
        Ok(addresses.into_iter().collect())
    }

    /// Local groups need owned inputs; book cycles also need inferred book
    /// members and incoming agent carriers. Neither needs unrelated bodies.
    fn invoke_scoped_audit(&mut self, dir: &str, group_only: bool) -> Result<AuditView> {
        let book = field(&self.state, "book");
        let revision = self.zt()?.cards(&[], "text")?.archive_revision;
        let details = crate::snapshot::read_title_trees(self.zt()?)?;
        if details
            .iter()
            .any(|tree| tree["archive_revision"].as_u64() != Some(revision))
        {
            bail!("incomplete Card catalogue: changing read view");
        }
        if !details.iter().any(|tree| {
            field(tree, "tree") == book
                && field(tree, "workspace") == "agent"
                && field(tree, "zone") == "literature"
        }) {
            bail!("no agent Literature tree `{book}`");
        }
        let mut metadata: Vec<Value> = details
            .iter()
            .flat_map(|tree| ps_items(&tree["cards"]))
            .collect();
        let positions: BTreeMap<String, usize> = metadata
            .iter()
            .enumerate()
            .map(|(i, card)| (field(card, "address"), i))
            .collect();
        if !group_only {
            let graph = self
                .zt()?
                .call("list_links", json!({}))
                .context("incomplete book relationship read")?;
            for card in &mut metadata {
                card["links"] = json!([]);
                card["kinds"] = json!({});
            }
            for link in graph["links"]
                .as_array()
                .context("incomplete book relationship list")?
            {
                let source = link["source"]
                    .as_str()
                    .context("incomplete book Link source")?;
                let target = link["target"]
                    .as_str()
                    .context("incomplete book Link target")?;
                let source_index = *positions
                    .get(source)
                    .context("incomplete book relationship source catalogue")?;
                let broken = link["broken"]
                    .as_bool()
                    .context("incomplete book Link existence")?;
                if !broken {
                    if !positions.contains_key(target) {
                        bail!("incomplete book relationship target catalogue `{target}`");
                    }
                    metadata[source_index]["links"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!(target));
                    if let Some(kind) = link["kind"].as_str() {
                        metadata[source_index]["kinds"][target] = json!(kind);
                    }
                }
            }
        }
        let catalogue_index = audit::book_of::book_index(&details, &metadata);
        let mut declared = BTreeSet::new();
        let addresses: Vec<String> = if group_only {
            self.group_addresses()?
        } else {
            let staging = field(&self.state, "staging");
            if !staging.is_empty() {
                let files = std::fs::read_dir(&staging)
                    .with_context(|| format!("incomplete ownership staging `{staging}`"))?;
                for entry in files {
                    let entry = entry
                        .with_context(|| format!("incomplete ownership staging `{staging}`"))?;
                    let name = entry.file_name().to_string_lossy().to_lowercase();
                    if !name.starts_with("owned-g") || !name.ends_with(".txt") {
                        continue;
                    }
                    if !entry.file_type()?.is_file() {
                        bail!("owned manifest is not a file: {}", entry.path().display());
                    }
                    let path = entry.path().to_string_lossy().into_owned();
                    declared.extend(
                        file_lines(&path)?
                            .into_iter()
                            .map(|line| line.trim().to_string())
                            .filter(|line| !line.is_empty()),
                    );
                }
            }
            let mut selected = declared.clone();
            selected.extend(
                metadata
                    .iter()
                    .filter(|card| {
                        field(card, "workspace") == "agent"
                            && catalogue_index.book_of.get(&field(card, "address")) == Some(&book)
                    })
                    .map(|card| field(card, "address")),
            );
            selected.into_iter().collect()
        };
        let selected: BTreeSet<String> = addresses.iter().cloned().collect();
        let owned = self.zt()?.cards(&addresses, "text")?;
        if owned.archive_revision != revision {
            bail!("incomplete Card inputs: changing read view");
        }
        if owned
            .cards
            .iter()
            .any(|card| field(card, "workspace") != "agent")
        {
            bail!("Card ownership includes a read-only human Card");
        }
        // Topic ids are numeric; a letter-led tree is a Literature key.
        if let Some(card) = owned.cards.iter().find(|card| {
            let tree = card["tree"].as_str().unwrap_or_default();
            tree != book && tree.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        }) {
            bail!(
                "owned Card `{}` belongs to another Literature tree `{}`",
                field(card, "address"),
                field(card, "tree")
            );
        }
        let mut rows: BTreeMap<String, Value> = owned
            .cards
            .into_iter()
            .map(|card| (field(&card, "address"), card))
            .collect();
        let term = metadata
            .iter()
            .find(|card| field(card, "tree") == book && TERMS_TITLE.is_match(&field(card, "title")))
            .map(|card| field(card, "address"));
        let parents: BTreeSet<String> = rows
            .values()
            .map(|card| field(card, "parent_address"))
            .filter(|at| !at.is_empty() && !rows.contains_key(at))
            .collect();
        let mut required = BTreeSet::new();
        if let Some(at) = &term {
            if !rows.contains_key(at) {
                required.insert(at.clone());
            }
        }
        if !group_only {
            for card in &metadata {
                if catalogue_index
                    .category
                    .get(&field(card, "address"))
                    .is_some_and(|category| category == "idea")
                    && ps_items(&card["links"])
                        .iter()
                        .any(|target| selected.contains(&s(target)))
                    && !rows.contains_key(&field(card, "address"))
                {
                    required.insert(field(card, "address"));
                }
            }
        }
        if !required.is_empty() {
            let extra = self
                .zt()?
                .cards(&required.into_iter().collect::<Vec<_>>(), "text")?;
            if extra.archive_revision != revision {
                bail!("incomplete Card inputs: changing read view");
            }
            for card in extra.cards {
                rows.insert(field(&card, "address"), card);
            }
        }
        // Parent context needs its fresh title, not its authored body.
        for at in parents {
            if !rows.contains_key(&at) {
                let position = *positions
                    .get(&at)
                    .context("incomplete parent title catalogue")?;
                rows.insert(at, metadata[position].clone());
            }
        }
        let graph: Vec<Value> = metadata
            .iter()
            .map(|metadata| {
                let mut card = rows
                    .get(&field(metadata, "address"))
                    .unwrap_or(metadata)
                    .clone();
                card["parent"] = card["parent_address"].clone();
                card
            })
            .collect();
        let index = audit::book_of::book_index(&details, &graph);
        let mut inbound: HashMap<String, Vec<String>> = HashMap::new();
        let mut kinded: HashMap<String, i64> = HashMap::new();
        if !group_only {
            for card in &graph {
                let source = field(card, "address");
                let category = index
                    .category
                    .get(&source)
                    .map(String::as_str)
                    .unwrap_or_default();
                let kinds = ps::keys(&card["kinds"]);
                for target in ps::select_unique(ps_items(&card["links"]).iter().map(s)) {
                    if !selected.contains(&target) {
                        continue;
                    }
                    inbound
                        .entry(target.clone())
                        .or_default()
                        .push(source.clone());
                    if target != source && is_in(&target, &kinds) && category == "idea" {
                        *kinded.entry(target).or_default() += 1;
                    }
                }
            }
        }
        let mut cards = Vec::new();
        let mut entries = Vec::new();
        let mut by = HashMap::new();
        for (at, mut card) in rows {
            let category = index
                .category
                .get(&at)
                .cloned()
                .context("incomplete group category metadata")?;
            let primary = selected.contains(&at);
            card["category"] = json!(if primary {
                category.as_str()
            } else {
                "context"
            });
            card["book"] = if primary {
                json!(book)
            } else {
                json!(index.book_of.get(&at))
            };
            card["parent"] = card["parent_address"].clone();
            card["kinds"] = json!(ps::keys(&card["kinds"]));
            if !group_only {
                card["inbound"] = json!(inbound.get(&at).cloned().unwrap_or_default());
                card["inbound_kinded"] = json!(kinded.get(&at).copied().unwrap_or(0));
            }
            if primary && is_in(&category, WRITABLE) {
                entries.push(card.clone());
            }
            by.insert(ci(&at), cards.len());
            cards.push(card);
        }
        let orphan_check = if group_only {
            json!({"complete":false,"status":"not_run","archive_revision":revision})
        } else {
            let primary: Vec<_> = cards
                .iter()
                .filter(|card| selected.contains(&field(card, "address")))
                .cloned()
                .collect();
            let baseline = join_path(&self.p.work, "before/audit-cards.json");
            let mut changed = BTreeSet::new();
            let mut since = "command_start";
            if dir != join_path(&self.p.work, "before") && Path::new(&baseline).is_file() {
                let old = util::read_json(&baseline)?;
                let old: BTreeMap<_, _> = ps_items(&old)
                    .into_iter()
                    .filter(|card| field(card, "category") != "context")
                    .map(|card| {
                        (
                            field(&card, "address"),
                            crate::calls::authored_fingerprint(&card),
                        )
                    })
                    .collect();
                let current: BTreeMap<_, _> = primary
                    .iter()
                    .map(|card| {
                        (
                            field(card, "address"),
                            crate::calls::authored_fingerprint(card),
                        )
                    })
                    .collect();
                for at in old.keys().chain(current.keys()) {
                    if old.get(at) != current.get(at) {
                        changed.insert(at.clone());
                    }
                }
                since = "before";
            }
            let mut report = zt_check::graph::orphan_report(
                &primary,
                revision,
                &changed.into_iter().collect::<Vec<_>>(),
            )?;
            report["changes_since"] = json!(since);
            report
        };
        let term_card = term
            .as_ref()
            .and_then(|at| by.get(&ci(at)).map(|&i| &cards[i]));
        let checked = Self::style_check(&entries, term_card)?;
        let mut faults: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut lines = Vec::new();
        for (at, rule, text) in checked {
            lines.push(format!("{at} {rule} {text}"));
            faults.entry(ci(&at)).or_default().push((rule, text));
        }
        if self.zt()?.cards(&[], "text")?.archive_revision != revision {
            bail!("incomplete Card inputs: changing read view");
        }
        std::fs::create_dir_all(dir)?;
        util::write_text(
            join_path(dir, "audit-cards.json"),
            &ps::json_pretty(&json!(cards)),
        )?;
        util::write_text(join_path(dir, "audit-faults.txt"), &lines.join(NL))?;
        let known = metadata
            .iter()
            .filter(|card| field(card, "workspace") == "agent")
            .map(|card| field(card, "address"))
            .collect::<Vec<_>>();
        util::write_text(join_path(dir, "audit-addresses.txt"), &known.join(NL))?;
        let scope = if group_only { "groups" } else { "book" };
        let coverage = json!({"complete":true,"scope":{"kind":scope,"groups":self.state["groups"],"addresses":addresses,"checked_addresses":entries.iter().map(|card|field(card,"address")).collect::<Vec<_>>()},"archive_revision":revision,"global_checks":"not_run","relationship_checks":if group_only{"not_run"}else{"book"},"faults":lines.len()});
        let mut coverage = coverage;
        coverage["orphan_check"] = orphan_check.clone();
        util::write_text(join_path(dir, "audit.json"), &ps::json_pretty(&coverage))?;
        util::write_text(
            join_path(dir, "audit.md"),
            &format!(
                "# {scope} Card quality{NL}{NL}Required reads complete at archive revision {revision}. Global archive checks were not run.{NL}"
            ),
        )?;
        Ok(AuditView {
            cards,
            by,
            faults,
            orphan_check,
        })
    }

    fn invoke_audit(&mut self, dir: &str) -> Result<AuditView> {
        let needs_global = self
            .rule_list()
            .iter()
            .any(|rule| is_in(rule, WHOLE_WORKSPACE) || rule == "F4");
        let group_only = !field(&self.state, "groups").is_empty()
            && truthy(&self.state["no_orphans"])
            && !needs_global;
        self.invoke_scoped_audit(dir, group_only)
    }

    // --- writes ----------------------------------------------------------------------------------

    /// `Invoke-Writes`: the calls through the helper; after a failed call, the rest again from
    /// the call after it. One entry per call: the address, or the error.
    fn invoke_writes(&mut self, calls: &[Value]) -> Result<Vec<Written>> {
        let mut results: Vec<Option<Written>> = vec![None; calls.len()];
        let mut start = 0;
        while start < calls.len() {
            let batch = &calls[start..];
            let run = calls::run(self.zt()?, batch, &mut calls::Options::new());
            let lines: Vec<String> = match run {
                Ok((done, failure)) => {
                    let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
                    if let Some(f) = failure {
                        lines.extend(f.lines());
                    }
                    lines
                }
                Err(error) => {
                    // The helper died: a new zt mcp for what comes next.
                    self.zt = None;
                    vec![format!("{error:#}")]
                }
            };
            let mut failed_at: Option<usize> = None;
            for line in &lines {
                if let Some(m) = OK_LINE.captures(line) {
                    let k: usize = m[1].parse().unwrap_or(usize::MAX);
                    if let Some(slot) = results.get_mut(start + k) {
                        *slot = Some(Ok(m[2].trim().to_string()));
                    }
                } else if let Some(m) = ERROR_LINE.captures(line) {
                    let k: usize = m[1].parse().unwrap_or(usize::MAX);
                    failed_at = Some(k);
                    if let Some(slot) = results.get_mut(start + k) {
                        *slot = Some(Err(m[2].to_string()));
                    }
                }
            }
            match failed_at {
                Some(k) => start += k + 1,
                None => {
                    for slot in results.iter_mut().skip(start) {
                        if slot.is_none() {
                            *slot = Some(Err(format!("no result: {}", lines.join(" | "))));
                        }
                    }
                    break;
                }
            }
        }
        Ok(results.into_iter().map(|r| r.unwrap_or_else(|| Err("no result".into()))).collect())
    }

    /// `Add-Owned`: a Card a split made joins the group that owns the Card it came from.
    fn add_owned(&self, from: &str, new: &str) -> Result<()> {
        let staging = field(&self.state, "staging");
        if staging.is_empty() {
            return Ok(());
        }
        for file in matching_files(&staging, "owned-g") {
            let lines: Vec<String> = file_lines(&file)?.iter().map(|l| l.trim().to_string()).collect();
            if is_in(from, &lines) {
                util::append_text(&file, &format!("{new}\n"))?;
                return Ok(());
            }
        }
        Ok(())
    }

    // --- what a rewrite must keep ----------------------------------------------------------------

    /// `Test-Rewrite`: None, or why apply refuses the rewrite.
    fn test_rewrite(&mut self, old: &Value, entry: &Value) -> Result<Option<String>> {
        let title = field(entry, "title");
        let body = field(entry, "body");
        if title.trim().is_empty() || body.trim().is_empty() {
            return Ok(Some("the title or the body is empty".into()));
        }
        if HEADING.is_match(body.trim_start()) {
            return Ok(Some("the body starts with a # heading".into()));
        }
        let mut new = format!("{title}\n{body}");
        if eq(&field(entry, "verdict"), "split") {
            let parts = ps_array(&entry["split"]);
            if parts.is_empty() {
                return Ok(Some("split names no new Card".into()));
            }
            for p in &parts {
                if field(p, "title").trim().is_empty() || field(p, "body").trim().is_empty() {
                    return Ok(Some("a split Card has no title or body".into()));
                }
                new.push_str(&format!("\n{}\n{}", field(p, "title"), field(p, "body")));
            }
        }
        let keep = text::keep(&format!("{}\n{}", field(old, "title"), field(old, "body")));
        let now = text::keep(&new);
        let mut lost: Vec<String> = keep.links.iter().filter(|l| !is_in(l, &now.links)).cloned().collect();
        // An Organize note that names other Links may replace a wrongly cited Card: the rewrite may
        // drop as many Links as it adds of those the note names, and no more.
        let note_text = if truthy(&self.state["notes"]) { field(&self.state["notes"], &field(entry, "address")) } else { String::new() };
        let named = text::link_targets(&note_text);
        let added = ps::select_unique(named.into_iter().filter(|n| is_in(n, &now.links) && !is_in(n, &keep.links)));
        if !lost.is_empty() && lost.len() <= added.len() {
            lost.clear();
        }
        if !lost.is_empty() {
            return Ok(Some(format!("lost Links: {}", lost.iter().map(|l| format!("[[{l}]]")).collect::<Vec<_>>().join(" "))));
        }
        let new_links: Vec<String> = now.links.iter().filter(|l| !is_in(l, &keep.links)).cloned().collect();
        let mut made = Vec::new();
        if !new_links.is_empty() {
            let known = self.known()?;
            made = new_links.into_iter().filter(|l| !known.contains(l)).collect();
        }
        if !made.is_empty() {
            return Ok(Some(format!(
                "Links to Cards that do not exist: {}",
                made.iter().map(|l| format!("[[{l}]]")).collect::<Vec<_>>().join(" ")
            )));
        }
        if keep.figures.iter().any(|f| !is_in(f, &now.figures)) {
            return Ok(Some("lost a figure line".into()));
        }
        if let Some(first) = keep.formulas.iter().find(|f| !is_in(f, &now.formulas)) {
            return Ok(Some(format!("changed formulas: {first}")));
        }
        Ok(None)
    }

    // --- plan ------------------------------------------------------------------------------------

    fn source_hint(&self, card: &Value) -> String {
        let staging = field(&self.state, "staging");
        if staging.is_empty() {
            return String::new();
        }
        let src = field(&self.state["src"], &field(card, "address"));
        if !src.is_empty() {
            return format!("`{}`（只读这几行）", join_path(&staging, &src));
        }
        if let Some(m) = SECTION.captures(&field(card, "body")) {
            return format!("小节 §{}：在 `{}` 中用 Select-String 找到它，只读相关段落", &m[1], join_path(&staging, "sections"));
        }
        format!("在 `{}` 中用 Select-String 找到这张卡的出处，只读相关段落", join_path(&staging, "sections"))
    }

    /// `New-OrphanJobs`: one orphan job per 2 x -PerJob orphans, each orphan with its best
    /// candidates among the Idea Cards, Event Cards, and anchors.
    fn orphan_jobs(&mut self, audit: &AuditView, orphans: &[Value]) -> Result<Vec<Value>> {
        let mut made = Vec::new();
        if orphans.is_empty() {
            return Ok(made);
        }
        let docs: Vec<(String, String)> = audit
            .cards
            .iter()
            .filter(|c| is_in(&field(c, "category"), &["idea", "event", "finding", "anchor"]))
            .map(|c| (field(c, "address"), format!("{} {} {}", field(c, "title"), field(c, "title"), field(c, "body"))))
            .collect();
        let model = Relatedness::new(&docs, 80);
        let size = (2 * self.p.per_job).max(1) as usize;
        for chunk in orphans.chunks(size) {
            let seq = self.next_seq();
            let id = format!("orphans-{seq:02}");
            let out = join_path(&self.out_dir, &format!("{id}.json"));
            let mut sb = format!("# 任务 {id}{NL}{NL}把结果写入 `{out}`。本任务共 {} 张孤立卡片。{NL}{NL}", chunk.len());
            self.add_self_check(&mut sb, &id)?;
            let mut candidates = Map::new();
            for o in chunk {
                let a = field(o, "address");
                // An Event or Finding Card is linked from Idea Cards only (E4, F4).
                let is_event = is_in(&field(o, "category"), &["event", "finding"]);
                let linking: Vec<String> = audit
                    .cards
                    .iter()
                    .filter(|c| ps_items(&c["links"]).iter().any(|l| eq(&s(l), &a)))
                    .map(|c| field(c, "address"))
                    .collect();
                let parent = field(o, "parent");
                let cands = model.find(&a, 5, |id| {
                    let Some(card) = audit.card(id) else { return true };
                    let category = field(card, "category");
                    eq(id, &parent) || is_in(id, &linking) || !is_in(&category, WRITABLE) || (is_event && !eq(&category, "idea"))
                });
                candidates.insert(a.clone(), json!(cands));
                sb.push_str(&format!("## 孤立卡片 `{a}`（{}）{NL}{NL}", field(o, "category")));
                sb.push_str(&format!("### {}{NL}{NL}````markdown{NL}{}{NL}````{NL}{NL}", field(o, "title"), trim_end(&field(o, "body"))));
                sb.push_str(&format!("候选卡片：{NL}{NL}"));
                for cand in &cands {
                    let card = audit.card(cand).cloned().unwrap_or(Value::Null);
                    let mut cb = SPACES.replace_all(&field(&card, "body"), " ").trim().to_string();
                    if cb.encode_utf16().count() > 160 {
                        let mut cut = String::new();
                        let mut units = 0;
                        for ch in cb.chars() {
                            units += ch.len_utf16();
                            if units > 160 {
                                break;
                            }
                            cut.push(ch);
                        }
                        cb = format!("{cut}…");
                    }
                    sb.push_str(&format!("- `{cand}` {}：{cb}{NL}", field(&card, "title")));
                }
                sb.push_str(NL);
            }
            let file = join_path(&self.p.work, &format!("{id}.md"));
            util::write_text(&file, &sb)?;
            let cards: Vec<String> = chunk.iter().map(|c| field(c, "address")).collect();
            self.state["jobs"][&id] =
                json!({ "kind": "orphans", "file": file, "out": out, "cards": cards, "candidates": candidates, "applied": false });
            made.push(json!({ "id": id, "files": [join_path(&self.p.work, &leaf(&self.orphan_doc())), file], "prompt": ORPHAN_PROMPT }));
        }
        Ok(made)
    }

    /// `Get-CompactJobs`: each job as {id, kind, file}; the folder, instruction files, and prompts once.
    fn compact_jobs(&self, jobs: &[Value]) -> Value {
        let mut prompts = Map::new();
        let mut list = Vec::new();
        for j in jobs {
            let kind = if truthy(&j["continue"]) {
                "follow"
            } else if ps::starts_ci(&field(j, "id"), "orphans-") {
                "orphans"
            } else {
                "fix"
            };
            if !prompts.contains_key(kind) {
                prompts.insert(kind.to_string(), j["prompt"].clone());
            }
            let files = ps_array(&j["files"]);
            let mut c = json!({ "id": j["id"], "kind": kind, "file": leaf(&s(files.last().unwrap_or(&Value::Null))) });
            if truthy(&j["continue"]) {
                c["continue"] = j["continue"].clone();
            }
            list.push(c);
        }
        json!({
            "dir": self.p.work,
            "lead": { "fix": join_path(&self.p.work, &leaf(&self.fix_doc())), "orphans": join_path(&self.p.work, &leaf(&self.orphan_doc())) },
            "prompts": prompts,
            "jobs": list
        })
    }

    fn add_compact(&self, out: &mut Value, jobs: &[Value]) {
        if self.p.compact {
            let c = self.compact_jobs(jobs);
            for k in ["dir", "lead", "prompts", "jobs"] {
                out[k] = c[k].clone();
            }
        }
    }

    fn has_orphan_fault(audit: &AuditView, address: &str, rules: &[&str]) -> bool {
        audit.all_faults(address).iter().any(|(rule, _)| is_in(rule, rules))
    }

    fn step_plan(&mut self) -> Result<()> {
        let p = &self.p;
        if p.book.is_empty() {
            bail!("plan needs -Book");
        }
        self.state = json!({
            "book": p.book, "staging": p.staging, "groups": p.groups, "rules": p.rules, "sample": p.sample,
            "jobs": {}, "seq": 0, "picked": [], "before": {}, "plan": {}, "no_orphans": p.no_orphans,
            "applied": {}, "refused": {}, "merges": [], "splits": [], "orphan_links": [], "orphan_none": [],
            "src": {}, "notes": {}
        });
        let mut audit = self.invoke_audit(&join_path(&self.p.work, "before"))?;
        let p = &self.p;
        if !p.notes.is_empty() && Path::new(&p.notes).exists() {
            // A list of notes, or Organize's decisions.json with its "notes" field.
            let raw = read_json_file(&p.notes)?;
            let list = match &raw {
                Value::Object(m) => match m.get("notes") {
                    Some(notes) => ps_array(notes),
                    None => vec![raw.clone()],
                },
                other => ps_array(other),
            };
            for n in list {
                let a = field(&n, "address");
                let note = field(&n, "note").trim().to_string();
                if a.is_empty() || note.is_empty() {
                    continue;
                }
                audit.faults.entry(ci(&a)).or_default().push(("N1".into(), note.clone()));
                let joined = format!("{} {note}", field(&self.state["notes"], &a)).trim().to_string();
                self.state["notes"][&a] = json!(joined);
            }
            let rules = field(&self.state, "rules");
            if !is_in("N1", &RULE_SPLIT.split(&rules).collect::<Vec<_>>()) {
                self.state["rules"] = json!(format!("{rules},N1"));
            }
        }
        let p = &self.p;
        let mut owned: Option<HashSet<String>> = None;
        if !p.groups.is_empty() {
            if p.staging.is_empty() {
                bail!("-Groups needs -Staging (owned-gNN.txt)");
            }
            let mut set = HashSet::new();
            for g in RULE_SPLIT.split(&p.groups).filter(|g| !g.is_empty()) {
                let n: i64 = match g.trim().parse::<i64>() {
                    Ok(n) => n,
                    Err(_) => g.trim().parse::<f64>().map(ps::ps_int).with_context(|| format!("Cannot convert value \"{g}\" to type \"System.Int32\""))?,
                };
                set.insert(format!("{}/{n}", p.book));
                let file = join_path(&p.staging, &format!("owned-g{n:02}.txt"));
                if Path::new(&file).exists() {
                    for a in file_lines(&file)? {
                        if !a.trim().is_empty() {
                            set.insert(a.trim().to_string());
                        }
                    }
                }
            }
            owned = Some(set);
        }
        if !p.staging.is_empty() {
            for file in matching_files(&p.staging, "src-g") {
                for line in file_lines(&file)? {
                    if let Some((at, src)) = line.split_once('\t')
                        && !src.trim().is_empty()
                    {
                        self.state["src"][at.trim()] = json!(src.trim());
                    }
                }
            }
        }
        let mut picked: Vec<Value> = audit
            .cards
            .iter()
            .filter(|c| {
                eq(&field(c, "book"), &self.p.book)
                    && !c["book"].is_null()
                    && is_in(&field(c, "category"), WRITABLE)
                    && owned.as_ref().is_none_or(|o| o.contains(&field(c, "address")))
            })
            .cloned()
            .collect();
        ps_sort(&mut picked, |a, b| culture_cmp(&field(a, "address"), &field(b, "address")));
        let mut with_faults: Vec<Value> = picked.iter().filter(|c| !self.hard_faults(&audit, &field(c, "address")).is_empty()).cloned().collect();
        if self.p.max_cards > 0 {
            with_faults.truncate(self.p.max_cards as usize);
        }
        let mut review: Vec<Value> = Vec::new();
        if self.p.sample > 0.0 && !self.p.staging.is_empty() {
            let clean: Vec<Value> = picked
                .iter()
                .filter(|c| eq(&field(c, "category"), "idea") && self.hard_faults(&audit, &field(c, "address")).is_empty())
                .cloned()
                .collect();
            let count = clean.len().min((self.p.sample * clean.len() as f64).ceil() as usize);
            let mut rng = ps::DotNetRandom::new(self.p.seed);
            let mut keyed: Vec<(i32, Value)> = clean.into_iter().map(|c| (rng.next(), c)).collect();
            ps_sort(&mut keyed, |a, b| a.0.cmp(&b.0));
            review = keyed.into_iter().take(count).map(|(_, c)| c).collect();
            ps_sort(&mut review, |a, b| culture_cmp(&field(a, "address"), &field(b, "address")));
        }
        let mut orphans: Vec<Value> = Vec::new();
        if !self.p.no_orphans {
            orphans = picked
                .iter()
                .filter(|c| {
                    is_in(&field(c, "category"), &["idea", "event", "finding"]) && Self::has_orphan_fault(&audit, &field(c, "address"), &["I5", "E4", "F4"])
                })
                .cloned()
                .collect();
            if self.p.max_cards > 0 {
                orphans.truncate(self.p.max_cards as usize);
            }
        }
        for c in with_faults.iter().chain(&review).chain(&orphans) {
            let a = field(c, "address");
            self.state["plan"][&a] = json!({ "title": field(c, "title"), "body": field(c, "body"), "category": field(c, "category") });
            let rules: Vec<String> = self.hard_faults(&audit, &a).into_iter().map(|(r, _)| r).collect();
            self.state["before"][&a] = json!(rules);
        }
        self.state["picked"] = json!(picked.iter().map(|c| field(c, "address")).collect::<Vec<_>>());
        let parent_title = |a: &str| -> String {
            if a.is_empty() {
                return String::new();
            }
            audit.card(a).map(|c| field(c, "title")).unwrap_or_default()
        };
        let mut jobs: Vec<Value> = Vec::new();
        let fix_cards: Vec<(Value, bool)> =
            with_faults.iter().map(|c| (c.clone(), false)).chain(review.iter().map(|c| (c.clone(), true))).collect();
        let size = self.p.per_job.max(1) as usize;
        for chunk in fix_cards.chunks(size) {
            let seq = self.next_seq();
            let id = format!("fix-{seq:02}");
            let out = join_path(&self.out_dir, &format!("{id}.json"));
            let mut sb = format!("# 任务 {id}{NL}{NL}把结果写入 `{out}`。本任务共 {} 张卡。{NL}{NL}", chunk.len());
            self.add_self_check(&mut sb, &id)?;
            for (card, is_review) in chunk {
                let a = field(card, "address");
                let faults = self.hard_faults(&audit, &a);
                let source = if *is_review || faults.iter().any(|(r, _)| is_in(r, &["I3", "E1", "E2"])) { self.source_hint(card) } else { String::new() };
                sb.push_str(&format_card(card, &faults, &parent_title(&field(card, "parent")), &source));
            }
            let file = join_path(&self.p.work, &format!("{id}.md"));
            util::write_text(&file, &sb)?;
            let cards: Vec<String> = chunk.iter().map(|(c, _)| field(c, "address")).collect();
            self.state["jobs"][&id] = json!({ "kind": "fix", "file": file, "out": out, "cards": cards, "applied": false });
            jobs.push(json!({ "id": id, "files": [self.fix_doc(), file], "prompt": FIX_PROMPT }));
        }
        jobs.extend(self.orphan_jobs(&audit, &orphans)?);
        // The instruction files name the book.
        for doc in [self.fix_doc(), self.orphan_doc()] {
            let text = util::read_text(&doc)?.replace("{{KEY}}", &self.p.book);
            util::write_text(join_path(&self.p.work, &leaf(&doc)), &text)?;
        }
        for j in jobs.iter_mut() {
            let first = s(&j["files"][0]);
            j["files"][0] = json!(join_path(&self.p.work, &leaf(&first)));
        }
        self.save_state()?;
        let faults: usize = with_faults.iter().map(|c| self.hard_faults(&audit, &field(c, "address")).len()).sum();
        let mut out = json!({
            "book": self.p.book, "picked": picked.len(), "with_faults": with_faults.len(), "review": review.len(),
            "orphans": audit.orphan_check["count"], "inbound_repair_cards": orphans.len(), "faults": faults, "jobs": jobs
        });
        self.add_compact(&mut out, &jobs);
        Self::print_last(&out)
    }

    // --- apply -----------------------------------------------------------------------------------

    fn step_apply(&mut self) -> Result<()> {
        if self.state.is_null() {
            bail!("run plan first");
        }
        let job_ids: Vec<String> = ps::keys(&self.state["jobs"]);
        let exists = |job: &Value| {
            let out = field(job, "out");
            !out.is_empty() && Path::new(&out).exists()
        };
        let pending: Vec<String> =
            job_ids.iter().filter(|id| !truthy(&self.state["jobs"][id.as_str()]["applied"]) && exists(&self.state["jobs"][id.as_str()])).cloned().collect();
        let missing: Vec<String> =
            job_ids.iter().filter(|id| !truthy(&self.state["jobs"][id.as_str()]["applied"]) && !exists(&self.state["jobs"][id.as_str()])).cloned().collect();
        // A write zt refused (a conflict, a lost connection) is tried again: the job's JSON still holds it.
        let mut retry: Vec<(String, HashSet<String>)> = Vec::new();
        for a in ps::keys(&self.state["refused"]) {
            let r = &self.state["refused"][&a];
            let job = field(r, "job");
            if ps::starts_ci(&field(r, "reason"), "zt:")
                && !a.contains("<-")
                && self.state["jobs"].get(&job).is_some()
                && eq(&field(&self.state["jobs"][&job], "kind"), "fix")
            {
                match retry.iter_mut().find(|(k, _)| ci(k) == ci(&job)) {
                    Some((_, set)) => {
                        set.insert(a);
                    }
                    None => retry.push((job, HashSet::from([a]))),
                }
            }
        }
        let mut to_apply: Vec<String> = pending.clone();
        to_apply.extend(retry.iter().map(|(k, _)| k.clone()).filter(|k| !is_in(k, &pending)));
        // -MaxJobs: a share of the jobs per call; the caller repeats apply until pending_jobs is 0.
        let mut later: Vec<String> = Vec::new();
        if self.p.max_jobs > 0 && to_apply.len() > self.p.max_jobs as usize {
            later = to_apply.split_off(self.p.max_jobs as usize);
        }
        let mut addresses: Vec<String> = Vec::new();
        for id in &to_apply {
            let job = &self.state["jobs"][id.as_str()];
            addresses.extend(ps_items(&job["cards"]).iter().map(s));
            if truthy(&job["candidates"])
                && let Some(m) = job["candidates"].as_object()
            {
                for v in m.values() {
                    addresses.extend(ps_items(v).iter().map(s));
                }
            }
        }
        let addresses = ps::select_unique(addresses);
        let mut current: HashMap<String, Value> = HashMap::new();
        if !addresses.is_empty() {
            let calls: Vec<Value> = addresses.iter().map(|a| json!({ "tool": "get_card", "arguments": { "at": a } })).collect();
            let read = calls::invoke(self.zt()?, &calls)?;
            for (a, v) in addresses.iter().zip(read) {
                current.insert(ci(a), v);
            }
        }
        let mut counts: Vec<(&str, i64)> = ["fix", "split", "unfaithful", "ok", "merge", "refused", "retried", "orphan_links", "orphan_none", "unread"]
            .iter()
            .map(|k| (*k, 0))
            .collect();
        let bump = |counts: &mut Vec<(&str, i64)>, k: &str| {
            if let Some(c) = counts.iter_mut().find(|(n, _)| *n == k) {
                c.1 += 1;
            }
        };
        for id in &to_apply {
            let job = self.state["jobs"][id.as_str()].clone();
            let only: Option<HashSet<String>> =
                if is_in(id, &pending) { None } else { retry.iter().find(|(k, _)| k == id).map(|(_, set)| set.clone()) };
            let result = match read_json_file(&field(&job, "out")) {
                Ok(r) => r,
                Err(error) => {
                    self.state["refused"][&format!("job:{id}")] = json!({ "job": id, "reason": format!("unreadable JSON: {error:#}") });
                    bump(&mut counts, "unread");
                    continue;
                }
            };
            let job_cards: Vec<String> = ps_items(&job["cards"]).iter().map(s).collect();
            let answer = |name: &str| if result.is_object() { result[name].clone() } else { Value::Null };
            if eq(&field(&job, "kind"), "fix") {
                let mut edits: Vec<Value> = Vec::new();
                let mut entries: HashMap<String, Value> = HashMap::new();
                let mut late_splits: Vec<(String, Value)> = Vec::new();
                for entry in ps_array(&answer("cards")) {
                    let a = field(&entry, "address");
                    if !is_in(&a, &job_cards) {
                        continue;
                    }
                    if let Some(only) = &only {
                        if !only.contains(&a) {
                            continue;
                        }
                        if let Some(m) = self.state["refused"].as_object_mut() {
                            m.shift_remove(&a);
                        }
                        bump(&mut counts, "retried");
                    }
                    let verdict = field(&entry, "verdict");
                    let old = self.state["plan"][&a].clone();
                    let refuse = |state: &mut Value, counts: &mut Vec<(&str, i64)>, reason: String| {
                        state["refused"][&a] = json!({ "job": id, "reason": reason });
                        bump(counts, "refused");
                    };
                    if eq(&verdict, "ok") {
                        bump(&mut counts, "ok");
                        self.state["applied"][&a] = json!({ "job": id, "verdict": "ok", "note": field(&entry, "note") });
                        continue;
                    }
                    if eq(&verdict, "merge") {
                        bump(&mut counts, "merge");
                        push(&mut self.state["merges"], json!({ "address": a, "merge_with": field(&entry, "merge_with"), "note": field(&entry, "note") }));
                        self.state["applied"][&a] = json!({ "job": id, "verdict": "merge" });
                        continue;
                    }
                    if !is_in(&verdict, &["fix", "split", "unfaithful"]) {
                        refuse(&mut self.state, &mut counts, format!("unknown verdict `{verdict}`"));
                        continue;
                    }
                    if !is_in(&field(&old, "category"), WRITABLE) {
                        refuse(&mut self.state, &mut counts, format!("category {} is not writable", field(&old, "category")));
                        continue;
                    }
                    // The title and the body decide a change; the stored text also holds the Reverse links.
                    let now = current.get(&ci(&a)).cloned();
                    // Already written (a run stopped part way): record it, and make a split Card that run did not make.
                    if let Some(now) = &now
                        && eq(&field(now, "title"), field(&entry, "title").trim())
                        && eq(&trim_end(&field(now, "body")), &trim_end(&field(&entry, "body")))
                    {
                        self.state["applied"][&a] = json!({ "job": id, "verdict": verdict, "note": field(&entry, "note") });
                        if eq(&verdict, "split") && !self.split_recorded(&a, id) {
                            let side_titles: Vec<String> = ps_items(&now["sides"]).iter().map(|x| field(x, "title").trim().to_string()).collect();
                            for part in ps_array(&entry["split"]) {
                                if !is_in(field(&part, "title").trim(), &side_titles) {
                                    late_splits.push((a.clone(), part));
                                }
                            }
                        }
                        continue;
                    }
                    let changed = match &now {
                        None => true,
                        Some(now) => {
                            !eq(&field(now, "title"), &field(&old, "title")) || !eq(&trim_end(&field(now, "body")), &trim_end(&field(&old, "body")))
                        }
                    };
                    if changed {
                        refuse(&mut self.state, &mut counts, "the Card changed since the plan".into());
                        continue;
                    }
                    if let Some(why) = self.test_rewrite(&old, &entry)? {
                        refuse(&mut self.state, &mut counts, why);
                        continue;
                    }
                    edits.push(json!({ "tool": "edit_card", "arguments": { "at": a, "title": field(&entry, "title").trim(), "body": trim_end(&field(&entry, "body")) } }));
                    entries.insert(ci(&a), entry);
                }
                // The edits first; a split Card only for an edit that is written, and only once.
                let done = self.invoke_writes(&edits)?;
                let mut creates: Vec<Value> = Vec::new();
                let mut from: Vec<String> = Vec::new();
                for (edit, r) in edits.iter().zip(&done) {
                    let a = field(&edit["arguments"], "at");
                    let entry = entries.get(&ci(&a)).cloned().unwrap_or(Value::Null);
                    if let Err(error) = r {
                        self.state["refused"][&a] = json!({ "job": id, "reason": format!("zt: {error}") });
                        bump(&mut counts, "refused");
                        continue;
                    }
                    self.state["applied"][&a] = json!({ "job": id, "verdict": field(&entry, "verdict"), "note": field(&entry, "note") });
                    if eq(&field(&entry, "verdict"), "unfaithful") {
                        bump(&mut counts, "unfaithful");
                    } else {
                        bump(&mut counts, "fix");
                    }
                    if eq(&field(&entry, "verdict"), "split") && !self.split_recorded(&a, id) {
                        for part in ps_array(&entry["split"]) {
                            creates.push(json!({ "tool": "create_successor", "arguments": { "at": a, "kind": "side", "title": field(&part, "title").trim(), "body": trim_end(&field(&part, "body")) } }));
                            from.push(a.clone());
                        }
                    }
                }
                for (at, part) in late_splits {
                    creates.push(json!({ "tool": "create_successor", "arguments": { "at": at, "kind": "side", "title": field(&part, "title").trim(), "body": trim_end(&field(&part, "body")) } }));
                    from.push(at);
                }
                let made = self.invoke_writes(&creates)?;
                for (k, r) in made.iter().enumerate() {
                    match r {
                        Ok(address) => {
                            push(&mut self.state["splits"], json!({ "from": from[k], "address": address, "job": id }));
                            bump(&mut counts, "split");
                            self.add_owned(&from[k], address)?;
                        }
                        Err(error) => {
                            self.state["refused"][&format!("split:{}:{k}", from[k])] = json!({ "job": id, "reason": format!("zt: {error}") });
                            bump(&mut counts, "refused");
                        }
                    }
                }
            } else {
                let mut calls: Vec<Value> = Vec::new();
                let mut what: Vec<(String, String)> = Vec::new();
                for l in ps_array(&answer("links")) {
                    let to = field(&l, "to");
                    let from = field(&l, "from");
                    let text = field(&l, "text").trim().to_string();
                    if !is_in(&to, &job_cards) {
                        continue;
                    }
                    let cands: Vec<String> = ps_array(&job["candidates"][&to]).iter().map(s).collect();
                    let mut reason = None;
                    if !is_in(&from, &cands) {
                        reason = Some(format!("`{from}` is not a candidate"));
                    } else if !text.contains(&format!("[[{to}]]")) {
                        reason = Some("the text does not link the orphan".into());
                    } else if !text::kinded(&text, &to) {
                        reason = Some("no kind word before the Link".into());
                    } else if SEMICOLON.is_match(&text) {
                        reason = Some("a semicolon".into());
                    } else {
                        let raw: Vec<String> = text::LINK.captures_iter(&text).map(|c| c[1].to_string()).collect();
                        let known = self.known()?;
                        if raw.iter().any(|t| !known.contains(t) && !eq(t, &to)) {
                            reason = Some("a Link to a Card that does not exist".into());
                        }
                    }
                    if let Some(reason) = reason {
                        self.state["refused"][&format!("{to}<-{from}")] = json!({ "job": id, "reason": reason });
                        bump(&mut counts, "refused");
                        continue;
                    }
                    // Already appended by a run that stopped part way: record it, do not append it twice.
                    if let Some(card) = current.get(&ci(&from))
                        && field(card, "body").contains(&text)
                    {
                        push(&mut self.state["orphan_links"], json!({ "to": to, "from": from }));
                        bump(&mut counts, "orphan_links");
                        continue;
                    }
                    calls.push(json!({ "tool": "append_card", "arguments": { "at": from, "text": text } }));
                    what.push((to, from));
                }
                for n in ps_array(&answer("none")) {
                    push(&mut self.state["orphan_none"], json!({ "orphan": field(&n, "orphan"), "note": field(&n, "note") }));
                    bump(&mut counts, "orphan_none");
                }
                let done = self.invoke_writes(&calls)?;
                for (k, (to, from)) in what.iter().enumerate() {
                    match done.get(k).cloned().unwrap_or_else(|| Err("no result".into())) {
                        Ok(_) => {
                            push(&mut self.state["orphan_links"], json!({ "to": to, "from": from }));
                            bump(&mut counts, "orphan_links");
                        }
                        Err(error) => {
                            self.state["refused"][&format!("{to}<-{from}")] = json!({ "job": id, "reason": format!("zt: {error}") });
                            bump(&mut counts, "refused");
                        }
                    }
                }
            }
            self.state["jobs"][id.as_str()]["applied"] = json!(true);
            // After every job: a run stopped part way loses at most the job it was writing.
            self.save_state()?;
        }
        self.save_state()?;
        let counts: Map<String, Value> = counts.into_iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        Self::print_last(&json!({
            "applied_jobs": to_apply.len(), "pending_jobs": later.len(), "retried_jobs": retry.len(),
            "waiting_jobs": missing, "counts": counts
        }))
    }

    fn split_recorded(&self, from: &str, job: &str) -> bool {
        ps_items(&self.state["splits"]).iter().any(|x| eq(&field(x, "from"), from) && eq(&field(x, "job"), job))
    }

    // --- check -----------------------------------------------------------------------------------

    fn step_check(&mut self) -> Result<()> {
        if self.state.is_null() {
            bail!("run plan first");
        }
        let audit = self.invoke_audit(&join_path(&self.p.work, "after"))?;
        let mut jobs: Vec<Value> = Vec::new();
        let mut total: usize = 0;
        for id in ps::keys(&self.state["jobs"]) {
            let job = self.state["jobs"][&id].clone();
            if !eq(&field(&job, "kind"), "fix") || !truthy(&job["applied"]) || truthy(&job["followed"]) {
                continue;
            }
            // The job's Cards and the Cards its splits made: a new Card is checked like the others.
            let mut addresses: Vec<String> = ps_items(&job["cards"]).iter().map(s).collect();
            addresses.extend(ps_items(&self.state["splits"]).iter().filter(|x| eq(&field(x, "job"), &id)).map(|x| field(x, "address")));
            let mut open: Vec<(String, Vec<(String, String)>, Value)> = Vec::new();
            for a in addresses {
                let left = self.hard_faults(&audit, &a);
                let refused = self.state["refused"].get(&a).cloned().unwrap_or(Value::Null);
                if left.is_empty() && !truthy(&refused) {
                    continue;
                }
                let applied = &self.state["applied"][&a];
                if truthy(applied) && is_in(&field(applied, "verdict"), &["ok", "merge"]) && !truthy(&refused) {
                    continue;
                }
                open.push((a, left, refused));
            }
            if open.is_empty() {
                continue;
            }
            total += open.iter().map(|o| o.1.len()).sum::<usize>();
            let fid = format!("{id}-r2");
            let out = join_path(&self.out_dir, &format!("{fid}.json"));
            let mut sb = format!(
                "# 任务 {fid}（第二轮）{NL}{NL}检查脚本核对了你写入的卡片。下面的卡片仍有问题，或你的改写被拒绝。只修正这些卡片，把结果写入 `{out}`（格式同前）。{NL}{NL}"
            );
            // The follow-up has its own id and file: its own self-check line, not the first job's.
            self.add_self_check(&mut sb, &fid)?;
            for (a, left, refused) in &open {
                let Some(card) = audit.card(a) else { continue };
                self.state["plan"][a.as_str()] = json!({ "title": field(card, "title"), "body": field(card, "body"), "category": field(card, "category") });
                let mut faults = left.clone();
                if truthy(refused) {
                    faults.push(("拒绝".into(), format!("脚本拒绝了上一轮的改写：{}", field(refused, "reason"))));
                }
                sb.push_str(&format_card(card, &faults, "", ""));
            }
            let file = join_path(&self.p.work, &format!("{fid}.md"));
            util::write_text(&file, &sb)?;
            self.state["jobs"][&id]["followed"] = json!(true);
            let cards: Vec<String> = open.iter().map(|o| o.0.clone()).collect();
            self.state["jobs"][&fid] = json!({ "kind": "fix", "file": file, "out": out, "cards": cards, "applied": false, "followed": true });
            for (a, _, _) in &open {
                if let Some(m) = self.state["refused"].as_object_mut() {
                    m.shift_remove(a);
                }
            }
            jobs.push(json!({
                "id": fid, "continue": id, "files": [file],
                "fallback": [join_path(&self.p.work, &leaf(&self.fix_doc())), file], "prompt": FOLLOW_PROMPT
            }));
        }
        self.save_state()?;
        // A Card that took an orphan Link got new text: check it too, in a fresh fix job.
        let linked = ps::select_unique(ps_items(&self.state["orphan_links"]).iter().filter(|l| !truthy(&l["checked"])).map(|l| field(l, "from")));
        if let Some(list) = self.state["orphan_links"].as_array_mut() {
            for l in list.iter_mut() {
                l["checked"] = json!(true);
            }
        }
        let open: Vec<String> = linked.into_iter().filter(|a| audit.card(a).is_some() && !self.hard_faults(&audit, a).is_empty()).collect();
        if !open.is_empty() {
            let seq = self.next_seq();
            let fid = format!("fix-{seq:02}");
            let out = join_path(&self.out_dir, &format!("{fid}.json"));
            let mut sb = format!(
                "# 任务 {fid}{NL}{NL}把结果写入 `{out}`。本任务共 {} 张卡：它们刚追加了一句链接，检查脚本在其中找到问题。{NL}{NL}",
                open.len()
            );
            self.add_self_check(&mut sb, &fid)?;
            for a in &open {
                let card = audit.card(a).cloned().unwrap_or(Value::Null);
                self.state["plan"][a.as_str()] = json!({ "title": field(&card, "title"), "body": field(&card, "body"), "category": field(&card, "category") });
                let faults = self.hard_faults(&audit, a);
                if self.state["before"].get(a).is_none() {
                    self.state["before"][a.as_str()] = json!(faults.iter().map(|f| f.0.clone()).collect::<Vec<_>>());
                }
                sb.push_str(&format_card(&card, &faults, "", ""));
                total += faults.len();
            }
            let file = join_path(&self.p.work, &format!("{fid}.md"));
            util::write_text(&file, &sb)?;
            self.state["jobs"][&fid] = json!({ "kind": "fix", "file": file, "out": out, "cards": open, "applied": false, "followed": true });
            jobs.push(json!({ "id": fid, "files": [join_path(&self.p.work, &leaf(&self.fix_doc())), file], "prompt": FIX_PROMPT }));
        }
        // Orphans the plan did not see: mostly Cards a split made. They get an orphan job of their own.
        let mut orphans: Vec<Value> = Vec::new();
        if !truthy(&self.state["no_orphans"]) {
            let mut handled: HashSet<String> = HashSet::new();
            if let Some(m) = self.state["jobs"].as_object() {
                for j in m.values().filter(|j| eq(&field(j, "kind"), "orphans")) {
                    handled.extend(ps_items(&j["cards"]).iter().map(s));
                }
            }
            let mut scope: HashSet<String> = ps_items(&self.state["picked"]).iter().map(s).collect();
            scope.extend(ps_items(&self.state["splits"]).iter().map(|x| field(x, "address")));
            orphans = audit
                .cards
                .iter()
                .filter(|c| {
                    let a = field(c, "address");
                    scope.contains(&a)
                        && !handled.contains(&a)
                        && is_in(&field(c, "category"), &["idea", "event", "finding"])
                        && Self::has_orphan_fault(&audit, &a, &["I5", "E4", "F4"])
                })
                .cloned()
                .collect();
            ps_sort(&mut orphans, |a, b| culture_cmp(&field(a, "address"), &field(b, "address")));
            for o in &orphans {
                let a = field(o, "address");
                if self.state["before"].get(&a).is_none() {
                    self.state["before"][a.as_str()] = json!([]);
                }
            }
            jobs.extend(self.orphan_jobs(&audit, &orphans)?);
        }
        self.save_state()?;
        let mut out = json!({ "jobs": jobs, "faults": total, "orphans": audit.orphan_check["count"], "inbound_repair_cards": orphans.len() });
        self.add_compact(&mut out, &jobs);
        Self::print_last(&out)
    }

    // --- report ----------------------------------------------------------------------------------

    fn step_report(&mut self) -> Result<()> {
        if self.state.is_null() {
            bail!("run plan first");
        }
        let audit = self.invoke_audit(&join_path(&self.p.work, "after"))?;
        let rules = self.rule_list();
        let mut before: Vec<(String, i64)> = Vec::new();
        let mut after: Vec<(String, i64)> = Vec::new();
        let add = |list: &mut Vec<(String, i64)>, k: &str| match list.iter_mut().find(|(n, _)| eq(n, k)) {
            Some(e) => e.1 += 1,
            None => list.push((k.to_string(), 1)),
        };
        let before_keys = ps::keys(&self.state["before"]);
        for a in &before_keys {
            for r in ps_items(&self.state["before"][a.as_str()]) {
                add(&mut before, &s(&r));
            }
            for (rule, _) in self.hard_faults(&audit, a) {
                add(&mut after, &rule);
            }
        }
        let orphans_after = audit.orphan_check["count"].clone();
        let mut verdicts: Vec<(String, i64)> = Vec::new();
        if let Some(m) = self.state["applied"].as_object() {
            for v in m.values() {
                add(&mut verdicts, &field(v, "verdict"));
            }
        }
        let as_map = |list: &[(String, i64)]| -> Value { Value::Object(list.iter().map(|(k, v)| (k.clone(), json!(v))).collect()) };
        let refused: Vec<Value> = ps::keys(&self.state["refused"])
            .into_iter()
            .map(|a| {
                let r = &self.state["refused"][&a];
                json!({ "address": a, "job": r["job"], "reason": r["reason"] })
            })
            .collect();
        let report = json!({
            "book": self.state["book"], "groups": self.state["groups"], "rules": rules,
            "cards": before_keys.len(), "picked": ps_items(&self.state["picked"]).len(),
            "faults_before": before.iter().map(|x| x.1).sum::<i64>(), "faults_after": after.iter().map(|x| x.1).sum::<i64>(),
            "by_rule_before": as_map(&before), "by_rule_after": as_map(&after),
            "verdicts": as_map(&verdicts), "splits": ps_items(&self.state["splits"]).len(), "merges": ps_items(&self.state["merges"]).len(),
            "refused": refused,
            "orphan_links": ps_items(&self.state["orphan_links"]).len(), "orphan_none": ps_items(&self.state["orphan_none"]).len(),
            "orphans_after": orphans_after,
            "orphan_check": audit.orphan_check,
            "job_count": ps::keys(&self.state["jobs"]).len()
        });
        util::write_text(join_path(&self.p.work, "report.json"), &ps::json_pretty(&report))?;
        let decisions = json!({
            "merges": ps_array(&self.state["merges"]), "orphan_none": ps_array(&self.state["orphan_none"]), "splits": ps_array(&self.state["splits"])
        });
        util::write_text(join_path(&self.p.work, "decisions.json"), &ps::json_pretty(&decisions))?;
        let mut short = Map::new();
        for (k, v) in report.as_object().unwrap() {
            if k != "refused" {
                short.insert(k.clone(), v.clone());
            }
        }
        short.insert("refused".into(), json!(ps_items(&report["refused"]).len()));
        Self::print_last(&Value::Object(short))
    }

    // --- precheck --------------------------------------------------------------------------------

    /// `Get-AuditCards`: the audit's view of each Card; the later run wins.
    fn audit_cards(&self) -> Result<Vec<Value>> {
        let mut cards: Vec<Value> = Vec::new();
        let mut at: HashMap<String, usize> = HashMap::new();
        for dir in ["before", "after"] {
            let f = join_path(&self.p.work, &format!("{dir}\\audit-cards.json"));
            if Path::new(&f).exists() {
                for c in ps_array(&read_json_file(&f)?) {
                    let key = ci(&field(&c, "address"));
                    match at.get(&key) {
                        Some(&i) => cards[i] = c,
                        None => {
                            at.insert(key, cards.len());
                            cards.push(c);
                        }
                    }
                }
            }
        }
        Ok(cards)
    }

    /// `New-CheckEntry`: a --json entry of zt-check, with the Links and kinded Links as zt reads them.
    fn check_entry(address: &str, title: &str, body: &str, category: &str, book: &str, parent: &str) -> Value {
        json!({
            "address": address, "title": title, "body": body, "category": category, "book": book, "parent": parent,
            "links": ps::select_unique(text::link_targets(body)), "kinds": text::link_kinds(body)
        })
    }

    /// `Invoke-StyleCheck`: zt-check on the entries (and the term table entry for R6).
    fn style_check(entries: &[Value], term: Option<&Value>) -> Result<Vec<(String, String, String)>> {
        let want: Vec<String> = entries.iter().map(|e| field(e, "address")).collect();
        let mut all = entries.to_vec();
        if let Some(t) = term {
            all.push(t.clone());
        }
        let tmp = unique_temp("card-precheck");
        let result = (|| {
            util::write_text(&tmp, &ps::json_pretty(&Value::Array(all)))?;
            let check = Check { json: tmp.clone(), setting: term.map(|t| field(t, "address")).unwrap_or_default(), ..Check::default() };
            check_offline(&check)
        })();
        let _ = std::fs::remove_file(&tmp);
        let mut out = Vec::new();
        for line in result? {
            let p: Vec<&str> = line.splitn(3, ' ').collect();
            if p.len() == 3 && is_in(p[0], &want) && RULE_NAME.is_match(p[1]) {
                out.push((p[0].to_string(), p[1].to_string(), p[2].to_string()));
            }
        }
        Ok(out)
    }

    fn fix_draft(&mut self, the_job: &Value, result: &Value, add: &mut dyn FnMut(&str, &str, &str)) -> Result<()> {
        let job = self.p.job.clone();
        if result.get("cards").is_none() {
            add(&job, "json", "the file holds no \"cards\" list");
            return Ok(());
        }
        let rules: Vec<String> = self.rule_list().into_iter().filter(|r| !is_in(r, WHOLE_WORKSPACE)).collect();
        let info = self.audit_cards()?;
        let info_of = |a: &str| info.iter().rev().find(|c| eq(&field(c, "address"), a)).cloned();
        let job_cards: Vec<String> = ps_items(&the_job["cards"]).iter().map(s).collect();
        let mut entries: Vec<Value> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for entry in ps_items(&result["cards"]).into_iter().filter(truthy) {
            let a = field(&entry, "address");
            if !is_in(&a, &job_cards) {
                add(if a.is_empty() { "-" } else { &a }, "refuse", &format!("`{a}` is not a Card of job {job}"));
                continue;
            }
            if !seen.insert(ci(&a)) {
                add(&a, "entry", "a second entry for this Card: write one entry per Card");
                continue;
            }
            let verdict = field(&entry, "verdict");
            if eq(&verdict, "ok") {
                continue;
            }
            if eq(&verdict, "merge") {
                if field(&entry, "merge_with").trim().is_empty() {
                    add(&a, "entry", "merge names no merge_with");
                }
                continue;
            }
            if !is_in(&verdict, &["fix", "split", "unfaithful"]) {
                add(&a, "refuse", &format!("unknown verdict `{verdict}`"));
                continue;
            }
            let old = self.state["plan"][&a].clone();
            if !is_in(&field(&old, "category"), WRITABLE) {
                add(&a, "refuse", &format!("category {} is not writable", field(&old, "category")));
                continue;
            }
            if let Some(why) = self.test_rewrite(&old, &entry)? {
                add(&a, "refuse", &why);
            }
            let card = info_of(&a);
            let book = match &card {
                Some(c) if truthy(&c["book"]) => field(c, "book"),
                _ => field(&self.state, "book"),
            };
            let parent = card.as_ref().map(|c| field(c, "parent")).unwrap_or_default();
            let title = field(&entry, "title").trim().to_string();
            let body = trim_end(&field(&entry, "body"));
            if !title.is_empty() && !body.trim().is_empty() {
                entries.push(Self::check_entry(&a, &title, &body, &field(&old, "category"), &book, &parent));
            }
            if eq(&verdict, "split") {
                // A split Card is an Idea Card filed as a Side successor of this Card: its parent.
                for (n, part) in ps_items(&entry["split"]).into_iter().filter(truthy).enumerate() {
                    let pt = field(&part, "title").trim().to_string();
                    let pb = trim_end(&field(&part, "body"));
                    if !pt.is_empty() && !pb.trim().is_empty() {
                        entries.push(Self::check_entry(&format!("{a}+split{}", n + 1), &pt, &pb, "idea", &book, &a));
                    }
                }
            }
        }
        for a in &job_cards {
            if !seen.contains(&ci(a)) {
                add(a, "entry", "no entry for this Card: write one entry per Card");
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        let mut term = None;
        if is_in("R6", &rules) {
            let book = field(&self.state, "book");
            if let Some(t) = info.iter().find(|c| eq(&field(c, "tree"), &book) && TERMS_TITLE.is_match(&field(c, "title"))) {
                term = Some(json!({ "address": field(t, "address"), "title": field(t, "title"), "body": field(t, "body") }));
            }
        }
        for (address, rule, text) in Self::style_check(&entries, term.as_ref())? {
            if is_in(&rule, &rules) {
                add(&address, &rule, &text);
            }
        }
        Ok(())
    }

    fn orphan_draft(&mut self, the_job: &Value, result: &Value, add: &mut dyn FnMut(&str, &str, &str)) -> Result<()> {
        let job = self.p.job.clone();
        let mut rules: Vec<String> = self.rule_list().into_iter().filter(|r| (R_RULE.is_match(r) && !eq(r, "R3")) || eq(r, "L1")).collect();
        rules.push("L2".into());
        let job_cards: Vec<String> = ps_items(&the_job["cards"]).iter().map(s).collect();
        let book = field(&self.state, "book");
        let mut entries: Vec<Value> = Vec::new();
        let mut done: HashSet<String> = HashSet::new();
        for l in ps_items(&result["links"]).into_iter().filter(truthy) {
            let to = field(&l, "to");
            let from = field(&l, "from");
            let text = field(&l, "text").trim().to_string();
            let at = format!("{to}<-{from}");
            if !is_in(&to, &job_cards) {
                add(&at, "refuse", &format!("`{to}` is not an orphan of job {job}"));
                continue;
            }
            done.insert(ci(&to));
            let cands: Vec<String> = ps_array(&the_job["candidates"][&to]).iter().map(s).collect();
            if !is_in(&from, &cands) {
                add(&at, "refuse", &format!("`{from}` is not a candidate: choose one of {}", cands.join(", ")));
            }
            if !text.contains(&format!("[[{to}]]")) {
                add(&at, "refuse", "the text does not link the orphan");
            } else if !text::kinded(&text, &to) {
                add(&at, "refuse", "no kind word before the Link");
            }
            if SEMICOLON.is_match(&text) {
                add(&at, "refuse", "a semicolon");
            }
            let raw: Vec<String> = text::LINK.captures_iter(&text).map(|c| c[1].to_string()).collect();
            let known = self.known()?;
            let unknown: Vec<String> = raw.into_iter().filter(|t| !known.contains(t) && !eq(t, &to)).collect();
            if !unknown.is_empty() {
                add(
                    &at,
                    "refuse",
                    &format!("a Link to a Card that does not exist: {}", unknown.iter().map(|u| format!("[[{u}]]")).collect::<Vec<_>>().join(" ")),
                );
            }
            // The sentence alone, as an anchor: no I2 on one sentence; I1 on the empty title is dropped.
            if !text.is_empty() {
                entries.push(Self::check_entry(&at, "", &text, "anchor", &book, ""));
            }
        }
        for n in ps_items(&result["none"]).into_iter().filter(truthy) {
            let o = field(&n, "orphan");
            if !is_in(&o, &job_cards) {
                add(if o.is_empty() { "-" } else { &o }, "entry", &format!("`{o}` is not an orphan of job {job}"));
            } else {
                done.insert(ci(&o));
            }
        }
        for a in &job_cards {
            if !done.contains(&ci(a)) {
                add(a, "entry", "no entry for this orphan: put it in \"links\" or in \"none\"");
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        for (address, rule, text) in Self::style_check(&entries, None)? {
            if is_in(&rule, &rules) {
                add(&address, &rule, &text);
            }
        }
        Ok(())
    }

    fn step_precheck(&mut self) -> Result<()> {
        let mut faults: Vec<String> = Vec::new();
        let mut add = |address: &str, rule: &str, text: &str| {
            faults.push(format!("{address} {rule} {}", SPACES.replace_all(text, " ").trim()));
        };
        let job = self.p.job.clone();
        let the_job = if !self.state.is_null() && !job.is_empty() { self.state["jobs"].get(&job).cloned() } else { None };
        match the_job {
            None => add(if job.is_empty() { "-" } else { &job }, "job", &format!("no job `{job}` in {}", self.state_file)),
            Some(the_job) => {
                let path = if !self.p.file.is_empty() { self.p.file.clone() } else { field(&the_job, "out") };
                let mut result = None;
                if !Path::new(&path).is_file() {
                    add(&job, "json", &format!("no JSON file at {path}"));
                } else {
                    match read_json_file(&path) {
                        Ok(Value::Null) => add(&job, "json", &format!("the file {path} is empty")),
                        Ok(Value::Object(m)) => result = Some(Value::Object(m)),
                        Ok(_) => add(&job, "json", &format!("the file {path} holds no JSON object")),
                        Err(error) => add(&job, "json", &format!("unreadable JSON in {path}: {error:#}")),
                    }
                }
                if let Some(result) = result {
                    if eq(&field(&the_job, "kind"), "fix") {
                        self.fix_draft(&the_job, &result, &mut add)?;
                    } else {
                        self.orphan_draft(&the_job, &result, &mut add)?;
                    }
                }
            }
        }
        let count = faults.len();
        faults.push(format!("faults: {count}"));
        util::print_lines(&faults)
    }
}

fn push(list: &mut Value, item: Value) {
    if !list.is_array() {
        *list = Value::Array(ps_items(list));
    }
    if let Some(items) = list.as_array_mut() {
        items.push(item);
    }
}

/// The rules the whole-book Card cycle checks an owned scope with. The Cards/group
/// completion runs the same set, so the work it hands to the book cycle is exactly
/// the work that cycle will see (R216).
pub const DEFAULT_CARD_RULES: &str = "R2,R3,R4,R5,R7,R9,R12,R18,R19,I1,I2,I3,I4,L1,L2,E1,E2";

/// `zt-digest card`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Work", "Book", "Staging", "Groups", "PerJob", "Sample", "Seed", "Rules", "MaxCards", "Notes", "MaxJobs", "Job", "File"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["NoOrphans", "Compact"])?;
    let seed = a.int_or("Seed", 1)?;
    let p = Params {
        step: a.choice("Step", &["plan", "apply", "check", "report", "precheck"])?,
        work: a.required("Work")?,
        book: a.str("Book"),
        staging: a.str("Staging"),
        groups: a.str("Groups"),
        per_job: a.int_or("PerJob", 8)?,
        sample: a.float_or("Sample", 0.0)?,
        seed: i32::try_from(seed).with_context(|| format!("-Seed: `{seed}` is not an Int32"))?,
        rules: a.str_or("Rules", DEFAULT_CARD_RULES),
        max_cards: a.int_or("MaxCards", 0)?,
        no_orphans: a.flag("NoOrphans"),
        notes: a.str("Notes"),
        compact: a.flag("Compact"),
        max_jobs: a.int_or("MaxJobs", 0)?,
        job: a.str("Job"),
        file: a.str("File"),
    };
    let out_dir = join_path(&p.work, "out");
    // precheck writes nothing, not even the folders.
    if p.step != "precheck" {
        std::fs::create_dir_all(&p.work)?;
        std::fs::create_dir_all(&out_dir)?;
    }
    let state_file = join_path(&p.work, "state.json");
    let state = if Path::new(&state_file).exists() { read_json_file(&state_file)? } else { Value::Null };
    let mut compiler = Compiler { p, zt: None, state, state_file, out_dir, scripts: scripts_dir(), known: None };
    match compiler.p.step.as_str() {
        "plan" => compiler.step_plan()?,
        "apply" => compiler.step_apply()?,
        "check" => compiler.step_check()?,
        "report" => compiler.step_report()?,
        _ => compiler.step_precheck()?,
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_card_in_a_job_file() {
        let card = json!({ "address": "0/2", "category": "idea", "parent": "0/1", "title": "标题", "body": "正文。\n\n来源 " });
        let text = format_card(&card, &[("R4".into(), "passive".into())], "父卡标题", "");
        let expected = [
            "## 卡片 `0/2`（idea）", "", "- 父卡：`0/1` 父卡标题", "- 问题：", "  - R4 passive", "", "### 标题", "", "标题", "", "### 正文", "",
            "````markdown", "正文。\n\n来源", "````", "", "",
        ]
        .join(NL);
        assert_eq!(text, expected);
        assert!(format_card(&card, &[], "", "").contains("这是抽查"));
    }
}
