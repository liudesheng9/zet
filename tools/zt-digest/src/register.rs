//! `zt-digest register`: draft and publish a Workspace's concept register (#101, #104, design §4.1,
//! §8.3-8.5, D058-D062, D170-D179, D183-D191, D276, D289-D291, D296-D299).
//!
//! The register is one ordinary Hub Card of level `register` (the first Card of the Topic
//! `索引`). Its `## 词条` section holds one term before a colon and one to three Links to the Cards
//! that carry the term's substance. This step is the only publication path for those rows: the hub
//! compiler's `fields.json` register still lists field and topic hubs, and this step keeps every
//! group it does not own (the `## 领域` field-hub navigation among them).
//!
//!     plan   reads the agent book term tables (through zt-check's term-table parser), the
//!            model-head names, and both Workspaces' titles, reads an optional reviewed concept
//!            report as read-only evidence and the accepted pinyin readings, and stages
//!            `<Work>\out\register-plan.json`. A register over 120 term rows is planned as parts:
//!            one ordinary Idea Card per accepted pinyin-initial band, each holding at most 120
//!            rows, declared by the root under `词条分册`. The plan holds the exact part bodies,
//!            the root body with one `{{part:N}}` reference per new part, the identified calls of
//!            the parts batch, the root call, and the readings it accepted and could not classify.
//!            Part addresses are reused by label and by position, so a rebuild keeps them and a
//!            part that is no longer needed stays declared and is emptied, never renumbered.
//!     apply  publishes the staged parts first through their own identified batch and verifies
//!            their exact bodies, then publishes the root last with the addresses the receipts
//!            returned. Retrying the same acceptance reuses the receipts instead of creating a
//!            second part. It then reads every page of the register back through `get_register` at
//!            one revision and marks the publication complete only after the exact root, parts,
//!            targets, and revision agree with the plan.
//!     check  reads every page and compares the staged plan with the published entries, parts,
//!            labels, and readings.
//!
//! What the step does not do: it does not judge meanings (the concept review does, S04), it does
//! not transliterate (the accepted readings are an input; `ps::culture_cmp` is not pinyin, D184),
//! and it does not correct an existing Card other than the one register and the parts it declares.
//! A human Card is only ever a read-only target the review selects explicitly (D174); the step
//! never writes the human Workspace. Discovery writes only the staging file, and a plan staged for
//! another archive is refused rather than applied.

use crate::args::Args;
use crate::canon;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

const SCHEMA_VERSION: i64 = 1;
/// The most targets a term may name (D289).
const MAX_TARGETS: usize = 3;
/// The page size of one `get_register` read (D276, §8.5): the largest page it serves.
const ENTRY_PAGE: usize = 1000;
/// The most pages one register read follows, so a broken `entry_next_offset` cannot loop.
const ENTRY_PAGES_MAX: usize = 1000;
/// The title every book term table carries (R6's own marker).
const TERM_TABLE_PREFIX: &str = "术语表";
/// The default first line of a register this step creates.
const DEFAULT_SCOPE: &str = "范围：代理工作区的入口。每个词条指向一到三个实质卡。";
/// The register term group this step writes (§8.4); `关键词` is the older compiler's heading.
const TERM_GROUPS: [&str; 2] = ["词条", "关键词"];
const DEFAULT_TITLE: &str = "索引：代理工作区的入口";
const INDEX_TITLE: &str = "索引";
const INDEX_BODY: &str = "本主题收放代理工作区的入口：索引卡与领域枢纽。";
/// The most term rows one register Card carries before its rows move into parts (D179, D290).
const PART_SPLIT: usize = 120;
/// The group a large register declares its parts under (§8.4); the root body rewrites it whole.
const PART_GROUP: &str = "词条分册";
/// The bucket of a term whose reading is missing or uncertain (D183).
const OTHER_LABEL: &str = "其他";
/// The bucket of a term whose accepted initial is a digit (D183).
const DIGIT_LABEL: &str = "0–9";
/// The accepted pinyin-initial bands a register plans (D177): six-letter ranges, `S–Z` last.
const BANDS: [(char, char, &str); 4] = [
    ('a', 'f', "A–F"),
    ('g', 'l', "G–L"),
    ('m', 'r', "M–R"),
    ('s', 'z', "S–Z"),
];

struct Params {
    step: String,
    work: String,
    review: String,
    readings: String,
}

/// `zt-digest register`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Work", "Review", "Readings"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let p = Params {
        step: a.choice("Step", &["plan", "apply", "check"])?,
        work: a.required("Work")?,
        review: a.str("Review"),
        readings: a.str("Readings"),
    };
    if p.work.is_empty() {
        bail!("Cannot bind argument to parameter 'Work' because it is an empty string.");
    }
    let work = full_path(&p.work)?;
    let out = crate::ps::join(&work, "out");
    std::fs::create_dir_all(&out).with_context(|| format!("failed to make {out}"))?;
    // The accepted pinyin readings of the plan: `-Readings` names the file the drafting job wrote.
    let readings_file = if p.readings.trim().is_empty() {
        crate::ps::join(&out, "register-readings.json")
    } else {
        full_path(&p.readings)?
    };
    let mut register = Register {
        out,
        review: p.review,
        readings_file,
        zt: None,
    };
    match p.step.as_str() {
        "plan" => register.plan()?,
        "apply" => {
            // A Lock that keeps a staged publication out (S16-AC05): the archive's typed refusal is
            // the answer, the earlier parts publication keeps its receipts, and the stage exits with
            // its own code instead of a crash.
            if let Err(error) = register.apply() {
                return match crate::lockcheck::stopped_refusal(&error) {
                    Some(refusal) => crate::lockcheck::stop("register", &refusal),
                    None => Err(error),
                };
            }
        }
        _ => register.check()?,
    }
    Ok(0)
}

pub(crate) struct Register {
    out: String,
    review: String,
    /// The accepted pinyin readings of the plan; an absent file leaves every term unclassified.
    readings_file: String,
    zt: Option<Zt>,
}

impl Register {
    fn plan_file(&self) -> String {
        crate::ps::join(&self.out, "register-plan.json")
    }

    fn zt(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().unwrap())
    }

    fn call(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        self.zt()?.call(tool, arguments)
    }

    /// The register's current state from one coherent `get_register` read (S27, #119): the root,
    /// its body, every declared part's exact body and the entry count.
    pub(crate) fn read_current(&mut self, tolerate_unlisted: bool) -> Result<Value> {
        self.current_register(tolerate_unlisted)
    }

    /// Every published entry of the agent register at one revision (S27, #119).
    pub(crate) fn read_published(&mut self) -> Result<(Value, Vec<(String, Vec<String>)>)> {
        self.published()
    }

    /// This publisher's one `zt mcp` client, for a caller that reads through the same process.
    pub(crate) fn client(&mut self) -> Result<&mut Zt> {
        self.zt()
    }

    /// One tool call on that client.
    pub(crate) fn call_tool(&mut self, tool: &str, arguments: Value) -> Result<Value> {
        self.call(tool, arguments)
    }
}

/// A register publisher for the review gate (S27, #119). It publishes the plan it is given, not
/// the Work folder's plan file, so the gate's own re-staged subset keeps its own receipt base.
pub(crate) fn publisher() -> Register {
    Register {
        out: String::new(),
        review: String::new(),
        readings_file: String::new(),
        zt: None,
    }
}

fn full_path(path: &str) -> Result<String> {
    let path = std::path::Path::new(path);
    let full = std::path::absolute(path).with_context(|| format!("resolve {}", path.display()))?;
    Ok(full.to_string_lossy().to_string())
}

fn tree_id(address: &str) -> String {
    address.split('/').next().unwrap_or_default().to_string()
}

// --- the read set ---------------------------------------------------------------------------------

/// One title of the agent Workspace, classified for entry selection.
#[derive(Clone, Debug)]
struct Title {
    address: String,
    tree: String,
    kind: String,
    facet: Option<String>,
    title: String,
}

impl Title {
    fn is_topic_root(&self) -> bool {
        self.address == format!("{}/0", self.tree)
    }

    fn is_literature_root(&self) -> bool {
        self.address == self.tree
    }

    fn is_hub(&self) -> bool {
        self.kind == "hub"
    }

    fn is_model_head(&self) -> bool {
        self.kind == "model" && self.facet.is_none()
    }

    fn is_term_table(&self) -> bool {
        self.title.trim_start().starts_with(TERM_TABLE_PREFIX)
    }

    /// The preference tier of this Card as an entry for a term (D170-D173): 1 a clear definition
    /// or statement (a Model part that is the actual definition is one), 2 the Model Card that
    /// explains a whole model, 3 a standalone Topic Card. These are preferences, not slots: a
    /// Card is a candidate only when its title names the term.
    fn tier(&self) -> Option<u8> {
        if self.is_hub() || self.is_term_table() || self.is_literature_root() {
            return None;
        }
        if self.is_topic_root() {
            return Some(3);
        }
        if self.is_model_head() {
            return Some(2);
        }
        Some(1)
    }

    fn contains(&self, term: &str) -> bool {
        self.title.to_lowercase().contains(&term.to_lowercase())
    }

    /// Whether this Card names `term`. A Topic Card is the entry of a term only when its title is
    /// the term itself (`学习` is not named by a Topic titled `学习模型`), while a statement or a
    /// model that mentions the term in its title names it (D173-D175).
    fn names(&self, term: &str) -> bool {
        if self.is_topic_root() {
            self.title.trim().eq_ignore_ascii_case(term)
        } else {
            self.contains(term)
        }
    }
}

/// One term row of one book term table, for one accepted spelling.
#[derive(Clone, Debug)]
struct RowSource {
    book: String,
    table: String,
    row: usize,
    meaning: String,
    spelling: String,
    preferred: bool,
}

impl RowSource {
    fn json(&self) -> Value {
        json!({
            "book": self.book,
            "term_table": self.table,
            "row": self.row,
            "meaning": self.meaning,
            "spelling": self.spelling,
            "preferred": self.preferred,
        })
    }
}

/// The reviewed concept report, read-only (§4.2, §4.4).
#[derive(Clone, Debug, Default)]
struct Review {
    report_id: String,
    evidence_id: Option<String>,
    status: String,
    complete: bool,
    /// spelling → the accepted entry addresses of a `same` finding.
    accepted: BTreeMap<String, Vec<String>>,
    /// spelling → the addressed Card of a `different` finding, which is a distinct sense.
    distinct: BTreeMap<String, BTreeSet<String>>,
    /// spelling → an `uncertain` finding's reason.
    uncertain: BTreeMap<String, String>,
    findings: usize,
    /// The reviewed spellings that no term table row or model head names.
    unmatched: Vec<String>,
    /// The summary of each report when several were combined (one concept review per source book,
    /// zt-idea-grow 3.1.0); empty for one report.
    reports: Vec<Value>,
}

/// The report files of `-Review`: one path, or several separated by `;` (one concept review per
/// source book).
fn review_paths(given: &str) -> Vec<String> {
    given.split(';').map(str::trim).filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// Several reviewed reports as one read-only evidence set: the accepted entries and the distinct
/// senses of every report, the first uncertain reason of a spelling, and each report's own
/// summary. One report is returned exactly as it was read.
fn merge_reviews(list: Vec<Review>) -> Option<Review> {
    if list.len() <= 1 {
        return list.into_iter().next();
    }
    let mut merged = Review {
        report_id: list.iter().map(|r| r.report_id.clone()).collect::<Vec<_>>().join(";"),
        status: if list.iter().all(|r| r.status == list[0].status) { list[0].status.clone() } else { "partial".into() },
        complete: list.iter().all(|r| r.complete),
        ..Review::default()
    };
    for review in &list {
        merged.findings += review.findings;
        for (spelling, entries) in &review.accepted {
            let known = merged.accepted.entry(spelling.clone()).or_default();
            for entry in entries {
                if !known.contains(entry) {
                    known.push(entry.clone());
                }
            }
        }
        for (spelling, senses) in &review.distinct {
            merged.distinct.entry(spelling.clone()).or_default().extend(senses.iter().cloned());
        }
        for (spelling, reason) in &review.uncertain {
            merged.uncertain.entry(spelling.clone()).or_insert_with(|| reason.clone());
        }
        merged.reports.push(review.summary());
    }
    Some(merged)
}

impl Review {
    fn summary(&self) -> Value {
        if !self.reports.is_empty() {
            return json!({
                "report_id": self.report_id,
                "evidence_id": self.evidence_id,
                "status": self.status,
                "complete": self.complete,
                "findings": self.findings,
                "accepted_spellings": self.accepted.keys().cloned().collect::<Vec<_>>(),
                "distinct_senses": self.distinct.keys().cloned().collect::<Vec<_>>(),
                "uncertain_spellings": self.uncertain.keys().cloned().collect::<Vec<_>>(),
                "unmatched_spellings": self.unmatched.clone(),
                "reports": self.reports,
            });
        }
        json!({
            "report_id": self.report_id,
            "evidence_id": self.evidence_id,
            "status": self.status,
            "complete": self.complete,
            "findings": self.findings,
            "accepted_spellings": self.accepted.keys().cloned().collect::<Vec<_>>(),
            "distinct_senses": self.distinct.keys().cloned().collect::<Vec<_>>(),
            "uncertain_spellings": self.uncertain.keys().cloned().collect::<Vec<_>>(),
            "unmatched_spellings": self.unmatched.clone(),
        })
    }
}

/// The staged plan.
#[derive(Clone, Debug, Default)]
struct Draft {
    terms: Vec<TermDraft>,
    unresolved: Vec<Value>,
    faults: Vec<Value>,
    excluded: Vec<Value>,
    review: Option<Review>,
}

#[derive(Clone, Debug)]
struct TermDraft {
    term: String,
    targets: Vec<String>,
    sources: Vec<RowSource>,
    ambiguous: bool,
    overflow: usize,
    reviewed: Vec<String>,
    /// The targets the review selected in the human Workspace: read-only Cards (D174). The
    /// register names them; no agent write ever touches them.
    read_only: Vec<String>,
}

impl TermDraft {
    fn json(&self) -> Value {
        json!({
            "term": self.term,
            "targets": self.targets,
            "sources": self.sources.iter().map(RowSource::json).collect::<Vec<_>>(),
            "ambiguous": self.ambiguous,
            "overflow": self.overflow,
            "reviewed": self.reviewed,
            "read_only_targets": self.read_only,
        })
    }
}

// --- plan -----------------------------------------------------------------------------------------

impl Register {
    fn plan(&mut self) -> Result<()> {
        let mut draft = None;
        // One read set, retried once when the archive moves under it (design §9.1).
        for _ in 0..2 {
            let (identity, revision) = self.archive_identity()?;
            let titles = self.call("export_titles", json!({"workspace": "agent"}))?;
            let checked = titles["archive_revision"].as_u64().unwrap_or_default();
            // The human Workspace is read too: a review may select one of its Cards explicitly
            // as a read-only target (D174). A human Card is never a candidate by itself.
            let humans = self.call("export_titles", json!({"workspace": "human"}))?;
            let tables = self.term_tables(&titles)?;
            let review = self.read_review(&identity)?;
            if checked == revision {
                let titles = parse_titles(&titles);
                let human = parse_titles(&humans);
                let draft_here = draft_terms(&titles, &human, &tables, review);
                draft = Some((identity, revision, draft_here));
                break;
            }
        }
        let Some((identity, revision, draft)) = draft else {
            bail!("the archive changed during every register read; run -Step plan again");
        };
        let current = self.current_register(false)?;
        let (readings, unresolved_readings, reading_faults) =
            read_readings(&self.readings_file, &draft.terms)?;
        let mut faults = draft.faults.clone();
        faults.extend(reading_faults);
        // A register over 120 term rows, or one already split, keeps its rows in declared parts
        // (D179, D185): never renumber a split register back onto its root.
        let declared = crate::ps::pipe(&current["parts"]).len();
        let parts = if draft.terms.len() > PART_SPLIT || declared > 0 {
            bucket_parts(&draft.terms, &readings)
        } else {
            Vec::new()
        };
        let staged = stage_register(&current, &draft.terms, &parts, &readings)?;
        let mut payload_draft = draft;
        payload_draft.faults = faults;
        let readings_payload = json!({
            "accepted": staged.accepted_readings,
            "unresolved": unresolved_readings,
            "count": readings.len(),
        });
        let plan = plan_value(
            &identity,
            revision,
            &current,
            &payload_draft,
            &staged,
            &readings_payload,
        );
        canon::save_json(&self.plan_file(), &plan)?;
        util::print_last(&json!({
            "ok": true,
            "step": "plan",
            "plan_id": plan["plan_id"],
            "archive": plan["archive"],
            "register": plan["register"],
            "terms": payload_draft.terms.len(),
            "entries": payload_draft.terms.iter().filter(|term| !term.targets.is_empty()).count(),
            "unresolved": payload_draft.unresolved.len(),
            "faults": payload_draft.faults.len(),
            "excluded": payload_draft.excluded.len(),
            "read_only": payload_draft.terms.iter().map(|term| term.read_only.len()).sum::<usize>(),
            "review": plan["review"],
            "parts": staged.parts.len(),
            "obsolete_parts": staged.obsolete.len(),
            "terms_on_root": staged.terms_on_root,
            "readings": {
                "accepted": readings.len(),
                "unresolved": unresolved_readings.len(),
            },
            "calls": staged.calls.len(),
            "path": self.plan_file(),
        }))
    }

    /// The archive identity and revision from a body-free read.
    fn archive_identity(&mut self) -> Result<(Value, u64)> {
        let view = self.zt()?.archive_view()?;
        Ok((
            view["archive_identity"].clone(),
            view["archive_revision"].as_u64().unwrap_or_default(),
        ))
    }

    /// Every agent book term table, through zt-check's own row parser (§4.2, D063).
    fn term_tables(
        &mut self,
        titles: &Value,
    ) -> Result<Vec<(String, String, Vec<zt_check::check::TermRow>)>> {
        let tables = crate::ps::pipe(&titles["titles"])
            .into_iter()
            .filter(|row| {
                crate::ps::starts_with(&crate::ps::s(&row["title"]).trim_start(), TERM_TABLE_PREFIX)
            })
            .map(|row| crate::ps::s(&row["address"]))
            .collect::<Vec<_>>();
        let mut rows = Vec::new();
        for chunk in tables.chunks(200) {
            let addresses = chunk.iter().map(|at| json!(at)).collect::<Vec<_>>();
            let read = self.call("get_cards", json!({"addresses": addresses, "view": "text"}))?;
            if read["complete"] != json!(true) {
                bail!(
                    "a term table is missing from the read: {}",
                    crate::ps::json_compact(&read["missing"])
                );
            }
            for card in crate::ps::pipe(&read["cards"]) {
                let address = crate::ps::s(&card["address"]);
                let title = crate::ps::s(&card["title"]);
                rows.push((
                    address,
                    title,
                    zt_check::check::term_table(&crate::ps::s(&card["body"])),
                ));
            }
        }
        rows.sort_by(|a, b| canon::ordinal_cmp(&a.0, &b.0));
        Ok(rows)
    }

    /// The reviewed concept report, when the caller names one. It is read-only evidence: the
    /// accepted meanings combine spellings, a `different` verdict keeps a sense apart, and the
    /// report id is recorded so the plan says which review it was read from. Several reports (one
    /// per source book, `a.json;b.json`) are combined into one evidence set (`merge_reviews`).
    fn read_review(&mut self, identity: &Value) -> Result<Option<Review>> {
        let mut list = Vec::new();
        for path in review_paths(&self.review) {
            if let Some(review) = self.read_one_review(&path, identity)? {
                list.push(review);
            }
        }
        Ok(merge_reviews(list))
    }

    /// One reviewed concept report.
    fn read_one_review(&self, given: &str, identity: &Value) -> Result<Option<Review>> {
        let path = full_path(given)?;
        let report = canon::read_json(&path)
            .with_context(|| format!("the reviewed concept report `{path}` is not JSON"))?;
        let report_id = crate::ps::s(&report["report_id"]);
        if !is_report_id(&report_id) {
            bail!(
                "the reviewed concept report has no content id (`report_id`); it is not a reviewed report"
            );
        }
        let archive_id = crate::ps::s(&report["archive"]["id"]);
        if !archive_id.is_empty() && archive_id != crate::ps::s(&identity["id"]) {
            bail!(
                "the reviewed concept report belongs to archive `{archive_id}`, not `{}`; read the review of this archive",
                crate::ps::s(&identity["id"])
            );
        }
        let proposals: BTreeMap<String, String> = crate::ps::pipe(&report["proposals"])
            .into_iter()
            .map(|row| {
                (
                    crate::ps::s(&row["id"]),
                    crate::ps::s(&row["title"]),
                )
            })
            .collect();
        let mut review = Review {
            report_id,
            evidence_id: Some(crate::ps::s(&report["evidence_path"])).filter(|id| !id.is_empty()),
            status: crate::ps::s(&report["status"]),
            complete: report["completeness"]["complete"] == json!(true),
            findings: 0,
            ..Review::default()
        };
        for finding in crate::ps::pipe(&report["findings"]) {
            if crate::ps::s(&finding["kind"]) != "topic-concept" {
                continue;
            }
            let proposal = crate::ps::s(&finding["proposal"]);
            let Some(spelling) = proposals.get(&proposal).filter(|title| !title.is_empty()) else {
                bail!(
                    "the review's finding `{}` names proposal `{proposal}`, which the report does not describe",
                    crate::ps::s(&finding["id"])
                );
            };
            review.findings += 1;
            match crate::ps::s(&finding["verdict"]).as_str() {
                "same" => {
                    let entry = crate::ps::s(&finding["entry"]);
                    if !entry.is_empty() {
                        let entries = review.accepted.entry(spelling.clone()).or_default();
                        if !entries.contains(&entry) {
                            entries.push(entry);
                        }
                    }
                }
                "different" => {
                    let candidate = crate::ps::s(&finding["candidate"]);
                    if !candidate.is_empty() {
                        review
                            .distinct
                            .entry(spelling.clone())
                            .or_default()
                            .insert(candidate);
                    }
                }
                _ => {
                    review
                        .uncertain
                        .entry(spelling.clone())
                        .or_insert_with(|| crate::ps::s(&finding["reason"]));
                }
            }
        }
        Ok(Some(review))
    }

    /// The typed refusal of the first Locked publication destination, or None. A destination is
    /// `(address, creates_here)`: a create at an existing Card is reported as `create-inside`, an
    /// edit of one as `edit`. The rewrite of a Locked source is the archive's own check.
    fn locked_destination(&mut self, destinations: &[(String, bool)]) -> Result<Option<Value>> {
        let mut unique: Vec<String> = destinations.iter().map(|(address, _)| address.clone()).collect();
        unique.sort();
        unique.dedup();
        for chunk in unique.chunks(200) {
            let addresses: Vec<Value> = chunk.iter().map(|address| json!(address)).collect();
            let answer = self.call("get_cards", json!({ "addresses": addresses, "view": "text" }))?;
            for card in crate::ps::pipe(&answer["cards"]) {
                let Some(lock) = crate::lockcheck::lock_in(&card["lock"]) else { continue };
                let at = crate::ps::s(&card["address"]);
                let creates = destinations.iter().any(|(address, creates)| *creates && crate::ps::eq(address, &at));
                let reason = if creates { "create-inside" } else { "edit" };
                return Ok(Some(crate::lockcheck::blocked_destination(reason, &at, &lock)));
            }
        }
        Ok(None)
    }

    /// The one register of the agent Workspace, as `get_register` reads it, with the exact bodies
    /// of the parts it declares. Two registers are reported, never chosen between; no register is
    /// an empty answer. A part the register cannot own in its place (moved, missing, a Hub, listed
    /// twice, nested, or a Side successor it never declares) is refused before any write: the
    /// compiler rewrites the whole `词条分册` layout, so it must read a coherent one (D182, D185).
    fn current_register(&mut self, tolerate_unlisted: bool) -> Result<Value> {
        let view = self.call("get_register", json!({"workspace": "agent"}))?;
        if view["register_missing"] == json!(true) {
            return Ok(json!({
                "address": Value::Null,
                "body": "",
                "missing": true,
                "parts": [],
                "faults": [],
            }));
        }
        if view["register"].is_null() {
            bail!(
                "the agent Workspace has more than one register: {}",
                crate::ps::json_compact(&view["duplicate_registers"])
            );
        }
        let address = crate::ps::s(&view["register"]);
        obstructed_parts(&view, tolerate_unlisted)?;
        let card = self.call("get_card", json!({"at": address}))?;
        let declared = crate::ps::pipe(&view["parts"])
            .into_iter()
            .filter(|part| part["place"] == json!("ok"))
            .map(|part| (crate::ps::s(&part["label"]), crate::ps::s(&part["address"])))
            .collect::<Vec<_>>();
        let mut parts = Vec::new();
        if !declared.is_empty() {
            let addresses = declared.iter().map(|(_, at)| json!(at)).collect::<Vec<_>>();
            let read = self.call("get_cards", json!({"addresses": addresses, "view": "text"}))?;
            if read["complete"] != json!(true) {
                bail!(
                    "a declared register part is missing from the read: {}",
                    crate::ps::json_compact(&read["missing"])
                );
            }
            for (label, at) in declared {
                let card = crate::ps::pipe(&read["cards"])
                    .into_iter()
                    .find(|card| crate::ps::s(&card["address"]) == at)
                    .cloned()
                    .unwrap_or(Value::Null);
                parts.push(json!({
                    "label": label,
                    "address": at,
                    "body": crate::ps::s(&card["body"]),
                    "card": card,
                }));
            }
        }
        Ok(json!({
            "address": view["register"],
            "body": card["body"],
            "missing": false,
            "entries": view["entry_total"],
            "complete": view["complete"],
            "parts": parts,
            "faults": view["faults"],
        }))
    }

    /// The Card addresses of both Workspaces, and those of the human one: a planned target may
    /// be an agent Card, or a human Card the review selected explicitly (D174).
    fn workspace_addresses(&mut self) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
        let mut all = BTreeSet::new();
        let mut human = BTreeSet::new();
        for workspace in ["agent", "human"] {
            let titles = self.call("export_titles", json!({"workspace": workspace}))?;
            for row in crate::ps::pipe(&titles["titles"]) {
                let address = crate::ps::s(&row["address"]);
                if workspace == "human" {
                    human.insert(address.clone());
                }
                all.insert(address);
            }
        }
        Ok((all, human))
    }

    /// Every published entry of the agent register at one revision: the pages of `get_register`
    /// followed until `entry_next_offset` is null (D276, §8.5). A revision that moves between
    /// pages is retried once; a read set that keeps moving is one changing-view fault (§12.3).
    fn published(&mut self) -> Result<(Value, Vec<(String, Vec<String>)>)> {
        let mut changing = String::new();
        for _ in 0..2 {
            let pages = self.entry_pages()?;
            match merged_terms(&pages) {
                Ok(read) => return Ok(read),
                Err(Merge::Changing(detail)) => changing = detail,
                Err(Merge::Fault(detail)) => bail!("{detail}"),
            }
        }
        bail!("{changing}")
    }

    /// The register's entry pages, in order, from offset 0 to the page whose
    /// `entry_next_offset` is null.
    fn entry_pages(&mut self) -> Result<Vec<Value>> {
        let mut pages = Vec::new();
        let mut offset = 0u64;
        for _ in 0..ENTRY_PAGES_MAX {
            let page = self.call(
                "get_register",
                json!({
                    "workspace": "agent",
                    "entry_offset": offset,
                    "entry_limit": ENTRY_PAGE,
                }),
            )?;
            let next = page["entry_next_offset"].as_u64();
            pages.push(page);
            match next {
                Some(next) if next > offset => offset = next,
                Some(_) => bail!(
                    "the register's entry_next_offset did not advance past {offset}; the pages cannot be one catalog"
                ),
                None => return Ok(pages),
            }
        }
        bail!(
            "the register needs more than {ENTRY_PAGES_MAX} pages of {ENTRY_PAGE} entries; read a smaller register"
        )
    }
}

fn is_report_id(value: &str) -> bool {
    value.len() == 66
        && value.starts_with("r-")
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn parse_titles(titles: &Value) -> Vec<Title> {
    crate::ps::pipe(&titles["titles"])
        .into_iter()
        .map(|row| Title {
            address: crate::ps::s(&row["address"]),
            tree: crate::ps::s(&row["tree"]),
            kind: crate::ps::s(&row["type"]),
            facet: row["facet"].as_str().map(str::to_string),
            title: crate::ps::s(&row["title"]),
        })
        .collect()
}

/// The plan's own content id: the accepted body, terms, unresolved rows, parts, readings, and
/// review, bound to the archive identity but not to its revision. Re-planning the same acceptance
/// gives the same id, and a publication in between does not change it. A changed reading is an
/// explicit new plan input, so it changes the id (D184).
fn plan_id(payload: &Value) -> String {
    let content = json!({
        "schema_version": payload["schema_version"],
        "archive_id": payload["archive"]["identity"]["id"],
        "terms": payload["terms"],
        "unresolved": payload["unresolved"],
        "faults": payload["faults"],
        "excluded": payload["excluded"],
        "review": payload["review"],
        "body": payload["body"],
        "parts": payload["parts"],
        "obsolete_parts": payload["obsolete_parts"],
        "readings": payload["readings"],
    });
    format!("rp-{}", canon::content_hash(&content, &[]))
}

fn plan_value(
    identity: &Value,
    revision: u64,
    current: &Value,
    draft: &Draft,
    staged: &Stage,
    readings: &Value,
) -> Value {
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "archive": {"identity": identity, "revision": revision},
        "register": {
            "address": current["address"],
            "missing": current["missing"],
            "entries": current["entries"],
            "complete": current["complete"],
            "parts": current["parts"].as_array().map_or(0, Vec::len),
        },
        "terms": draft.terms.iter().map(TermDraft::json).collect::<Vec<_>>(),
        "unresolved": draft.unresolved,
        "faults": draft.faults,
        "excluded": draft.excluded,
        "review": draft.review.as_ref().map(Review::summary),
        "readings": readings,
        "parts": staged.parts.iter().map(StagedPart::json).collect::<Vec<_>>(),
        "obsolete_parts": staged.obsolete.iter().map(StagedObsolete::json).collect::<Vec<_>>(),
        "terms_on_root": staged.terms_on_root,
        "root_before": staged.root_before,
        "body": staged.root_body,
        "calls": staged.calls,
        "root_call": staged.root_call,
    });
    let id = plan_id(&payload);
    payload["plan_id"] = json!(id);
    payload["created_at"] = json!(canon::utc_now_o());
    payload
}

/// The accepted spellings of each term and their targets, as the plan stages them.
fn draft_terms(
    titles: &[Title],
    human: &[Title],
    tables: &[(String, String, Vec<zt_check::check::TermRow>)],
    review: Option<Review>,
) -> Draft {
    // The human Workspace is read-only: it is never a source of automatic candidates, and only a
    // review's explicit selection makes one of its Cards a target (D174).
    let human: BTreeSet<&str> = human.iter().map(|title| title.address.as_str()).collect();
    // One concept is one source row: its preferred spelling and the book's rejected words for it
    // name the same thing, so every spelling of the concept gets the same targets. Two rows that
    // share a spelling (two books, or a model head and a book term) are combined at the spelling.
    struct Concept {
        spellings: Vec<String>,
        sources: Vec<RowSource>,
    }
    let mut concepts = Vec::new();
    for (table, _title, term_rows) in tables {
        for (index, row) in term_rows.iter().enumerate() {
            let mut accepted = vec![row.term.trim().to_string()];
            accepted.extend(row.avoid.iter().map(|avoid| avoid.trim().to_string()));
            accepted.retain(|spelling| !spelling.is_empty());
            let mut seen = BTreeSet::new();
            accepted.retain(|spelling| seen.insert(spelling.clone()));
            let mut spellings = Vec::new();
            let mut sources = Vec::new();
            for spelling in accepted {
                let preferred = spelling == row.term.trim();
                sources.push(RowSource {
                    book: book_of(table),
                    table: table.clone(),
                    row: index + 1,
                    meaning: row.meaning.trim().to_string(),
                    spelling: spelling.clone(),
                    preferred,
                });
                spellings.push(spelling);
            }
            if !spellings.is_empty() {
                concepts.push(Concept { spellings, sources });
            }
        }
    }
    // Model-head names are terms too (D170-D175, §8.3). A term row is `term：links` (§8.4), so a
    // label that itself holds `：` cannot be a term: it would publish a row whose term is a prefix
    // and whose targets never read back. It is excluded with its reason, never rewritten.
    let mut excluded = Vec::new();
    for title in titles.iter().filter(|title| title.is_model_head()) {
        let spelling = title.title.trim().to_string();
        if spelling.is_empty() {
            continue;
        }
        if spelling.contains('：') || spelling.contains(':') {
            excluded.push(json!({
                "term": spelling,
                "address": title.address,
                "reason": "term_separator",
                "detail": "a term label cannot hold the row separator `：`; rename the Card or name the term in a book term table",
            }));
            continue;
        }
        concepts.push(Concept {
            spellings: vec![spelling.clone()],
            sources: vec![RowSource {
                book: String::new(),
                table: title.address.clone(),
                row: 0,
                meaning: String::new(),
                spelling,
                preferred: true,
            }],
        });
    }
    let mut candidates: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
    let mut vetoes: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for concept in &concepts {
        let mut own = Vec::new();
        for spelling in &concept.spellings {
            for title in titles {
                let Some(tier) = title.tier() else { continue };
                if title.names(spelling) {
                    own.push((tier, title.address.clone()));
                }
            }
            if let Some(review) = &review {
                for entry in review.accepted.get(spelling).into_iter().flatten() {
                    if titles.iter().any(|title| &title.address == entry)
                        || human.contains(entry.as_str())
                    {
                        // A reviewed entry outside the agent Workspace is an explicitly
                        // selected human Card: a read-only target, never an agent write.
                        own.push((1, entry.clone()));
                    }
                }
            }
        }
        let mut distinct = BTreeSet::new();
        if let Some(review) = &review {
            for spelling in &concept.spellings {
                distinct.extend(
                    review
                        .distinct
                        .get(spelling)
                        .into_iter()
                        .flatten()
                        .cloned(),
                );
            }
        }
        for spelling in &concept.spellings {
            candidates
                .entry(spelling.clone())
                .or_default()
                .extend(own.iter().cloned());
            vetoes
                .entry(spelling.clone())
                .or_default()
                .extend(distinct.iter().cloned());
        }
    }
    let rows = concepts
        .iter()
        .flat_map(|concept| concept.sources.iter().cloned())
        .collect::<Vec<_>>();
    let mut draft = build_terms(&rows, &candidates, &vetoes);
    // The register names a reviewed human target, but agent maintenance never writes that Card.
    for term in &mut draft.terms {
        term.read_only = term
            .targets
            .iter()
            .filter(|target| human.contains(target.as_str()))
            .cloned()
            .collect();
    }
    // A reviewed entry the archive does not hold cannot be a target: a missing Card is a stale
    // review and needs a fresh review, never an invented address.
    if let Some(review) = &review {
        for (spelling, entries) in &review.accepted {
            for entry in entries {
                if !titles.iter().any(|title| &title.address == entry)
                    && !human.contains(entry.as_str())
                {
                    excluded.push(json!({
                        "term": spelling,
                        "address": entry,
                        "reason": "reviewed_entry_missing",
                        "detail": "the reviewed entry Card is in neither Workspace; a stale review needs reviewing again",
                    }));
                }
            }
        }
    }
    draft.excluded.extend(excluded);
    draft.faults.sort_by(|a, b| {
        canon::ordinal_cmp(
            &format!("{}|{}", crate::ps::s(&a["fault"]), crate::ps::s(&a["term"])),
            &format!("{}|{}", crate::ps::s(&b["fault"]), crate::ps::s(&b["term"])),
        )
    });
    if let Some(review) = review {
        let known = draft
            .terms
            .iter()
            .map(|term| term.term.clone())
            .chain(
                draft
                    .unresolved
                    .iter()
                    .filter_map(|row| row["term"].as_str().map(str::to_string)),
            )
            .collect::<BTreeSet<_>>();
        let unmatched = review
            .accepted
            .keys()
            .chain(review.distinct.keys())
            .chain(review.uncertain.keys())
            .filter(|spelling| !spelling.is_empty() && !known.contains(*spelling))
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut review = review;
        review.unmatched = unmatched.into_iter().collect();
        draft.review = Some(review);
    }
    draft
}

/// The book a term table belongs to: the tree it is filed in (a Citation key in a Literature
/// tree, the Topic number elsewhere).
fn book_of(table: &str) -> String {
    tree_id(table)
}

/// Group the source rows by accepted spelling and choose up to three targets for each. A spelling
/// whose rows give different meanings is reported as ambiguous, and its destinations are all kept
/// (D299: never choose arbitrarily).
fn build_terms(
    rows: &[RowSource],
    candidates: &BTreeMap<String, Vec<(u8, String)>>,
    vetoes: &BTreeMap<String, BTreeSet<String>>,
) -> Draft {
    let mut grouped: BTreeMap<String, Vec<RowSource>> = BTreeMap::new();
    for row in rows {
        grouped.entry(row.spelling.clone()).or_default().push(row.clone());
    }
    let mut terms = Vec::new();
    let mut unresolved = Vec::new();
    let mut faults = Vec::new();
    for (spelling, sources) in grouped {
        let empty = Vec::new();
        let candidates = candidates.get(&spelling).unwrap_or(&empty);
        let empty_set = BTreeSet::new();
        let vetoed = vetoes.get(&spelling).unwrap_or(&empty_set);
        let (targets, overflow) = select_targets(candidates, vetoed);
        let meanings = sources
            .iter()
            .map(|source| source.meaning.trim().to_string())
            .filter(|meaning| !meaning.is_empty())
            .collect::<BTreeSet<_>>();
        let ambiguous = meanings.len() > 1;
        if ambiguous {
            faults.push(json!({                "fault": "ambiguous",
                "term": spelling,
                "detail": "the same accepted spelling is defined differently; keep its destinations and read the senses before merging them",
                "sources": sources.iter().map(RowSource::json).collect::<Vec<_>>(),
            }));
        }
        if targets.is_empty() {
            unresolved.push(json!({
                "term": spelling,
                "reason": if vetoed.is_empty() { "no_defensible_target" } else { "distinct_senses_only" },
                "detail": if vetoed.is_empty() {
                    "no clear definition, whole model, or standalone Topic claim names this term; it stays unresolved with its source row"
                } else {
                    "the reviewed meanings keep every naming Card in a distinct sense; the term needs an accepted label for each sense"
                },
                "sources": sources.iter().map(RowSource::json).collect::<Vec<_>>(),
            }));
            continue;
        }
        terms.push(TermDraft {
            term: spelling,
            targets,
            sources: sources.clone(),
            ambiguous,
            overflow,
            reviewed: Vec::new(),
            read_only: Vec::new(),
        });
    }
    terms.sort_by(|a, b| canon::ordinal_cmp(&a.term, &b.term));
    Draft {
        terms,
        unresolved: merge_unresolved(unresolved),
        faults,
        excluded: Vec::new(),
        review: None,
    }
}

/// One unresolved finding per source row: a book row whose preferred word and rejected words all
/// lack a target is one unresolved term, not one per spelling.
fn merge_unresolved(rows: Vec<Value>) -> Vec<Value> {
    let mut merged: BTreeMap<String, (Value, Vec<Value>)> = BTreeMap::new();
    for row in rows {
        let key = row["sources"]
            .as_array()
            .and_then(|sources| sources.first())
            .map(|source| {
                format!(
                    "{}|{}",
                    crate::ps::s(&source["term_table"]),
                    crate::ps::s(&source["row"])
                )
            })
            .unwrap_or_default();
        match merged.get_mut(&key) {
            Some((existing, spellings)) => {
                // The book's preferred word names the unresolved term; its rejected words are
                // spellings of it.
                if crate::ps::truthy(&row["sources"][0]["preferred"]) {
                    existing["term"] = row["term"].clone();
                }
                spellings.push(row["term"].clone());
            }
            None => {
                let spelling = row["term"].clone();
                merged.insert(key, (row, vec![spelling]));
            }
        }
    }
    merged
        .into_values()
        .map(|(mut row, mut spellings)| {
            let term = crate::ps::s(&row["term"]);
            if let Some(index) = spellings
                .iter()
                .position(|spelling| crate::ps::s(spelling) == term)
            {
                let first = spellings.remove(index);
                spellings.insert(0, first);
            }
            row["spellings"] = Value::Array(spellings);
            row
        })
        .collect()
}

/// Up to three targets, best preference first, one address each, excluding a reviewed distinct
/// sense's address. Returns the chosen addresses and how many acceptable ones were left out.
fn select_targets(candidates: &[(u8, String)], vetoes: &BTreeSet<String>) -> (Vec<String>, usize) {
    let mut best: BTreeMap<&str, u8> = BTreeMap::new();
    for (tier, address) in candidates {
        if vetoes.contains(address) {
            continue;
        }
        best.entry(address.as_str())
            .and_modify(|known| *known = (*known).min(*tier))
            .or_insert(*tier);
    }
    let mut ordered = best.into_iter().collect::<Vec<_>>();
    ordered.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| canon::ordinal_cmp(a.0, b.0)));
    let overflow = ordered.len().saturating_sub(MAX_TARGETS);
    (
        ordered
            .into_iter()
            .take(MAX_TARGETS)
            .map(|(address, _)| address.to_string())
            .collect(),
        overflow,
    )
}

/// The exact register body: the preamble and every group the register already holds, then one
/// `## 词条` row per accepted term. Text after a term's Links never enters a row. The `词条分册`
/// declaration is the compiler's own group and is rewritten by [`root_body_text`].
fn register_body(current: &str, terms: &[TermDraft]) -> String {
    let mut preamble = Vec::new();
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    let mut inside: Option<usize> = None;
    for line in current.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            sections.push((heading.trim().to_string(), Vec::new()));
            inside = Some(sections.len() - 1);
            continue;
        }
        match inside {
            None => preamble.push(trimmed.trim_end().to_string()),
            Some(index) => sections[index].1.push(trimmed.trim_end().to_string()),
        }
    }
    let mut out = String::new();
    let scope = preamble.join("\n").trim().to_string();
    out.push_str(if scope.is_empty() { DEFAULT_SCOPE } else { &scope });
    out.push_str("\n\n");
    for (heading, lines) in sections {
        if TERM_GROUPS.contains(&heading.as_str()) || heading == PART_GROUP {
            continue;
        }
        out.push_str(&format!("## {heading}\n"));
        let body = lines.join("\n").trim().to_string();
        if !body.is_empty() {
            out.push_str(&body);
            out.push('\n');
        }
        out.push('\n');
    }
    if !terms.is_empty() {
        out.push_str("## 词条\n");
        for term in terms {
            let links = term
                .targets
                .iter()
                .map(|address| format!("[[{address}]]"))
                .collect::<Vec<_>>()
                .join("、");
            out.push_str(&format!("- {}：{links}\n", term.term));
        }
    }
    out.trim_end().to_string()
}

// --- readings and parts (S11, #104, design §8.4) ---------------------------------------------------

/// One accepted pinyin reading of a term: the Latin reading the drafting job supplied and the
/// initial it groups under. This compiler validates their shape, sorts the accepted readings and
/// groups their initials; it never transliterates (D183, D184).
#[derive(Clone, Debug)]
struct Reading {
    reading: String,
    initial: Option<char>,
}

/// One planned register part: an ordinary Idea Card holding a contiguous bucket of term rows. Its
/// identity is the root's declaration plus its place as an immediate Side successor of the
/// register (D181, D182).
#[derive(Clone, Debug)]
struct Part {
    label: String,
    bucket: &'static str,
    /// Indices into the planned term list, in the part's own order.
    terms: Vec<usize>,
}

/// One part as the plan stages it: the body it publishes, the address it reuses or the call that
/// creates it, and the terms it is planned to hold.
#[derive(Clone, Debug)]
struct StagedPart {
    label: String,
    bucket: &'static str,
    title: String,
    address: Option<String>,
    body: String,
    terms: Vec<String>,
    call: Option<usize>,
    reused: bool,
}

impl StagedPart {
    fn json(&self) -> Value {
        json!({
            "label": self.label,
            "bucket": self.bucket,
            "title": self.title,
            "address": self.address,
            "body": self.body,
            "terms": self.terms,
            "call": self.call,
            "reused": self.reused,
        })
    }
}

/// A part the register declares that the plan no longer needs: it keeps its address and stays
/// declared as an empty navigation entry, pending a separate deletion plan (D185, AC04).
#[derive(Clone, Debug)]
struct StagedObsolete {
    label: String,
    address: String,
    body: String,
}

impl StagedObsolete {
    fn json(&self) -> Value {
        json!({"label": self.label, "address": self.address, "body": self.body})
    }
}

/// The staged publication: the parts, the root body with one `{{part:N}}` reference per part, and
/// the identified calls that publish it (§8.5, D189, D190).
#[derive(Clone, Debug, Default)]
struct Stage {
    parts: Vec<StagedPart>,
    obsolete: Vec<StagedObsolete>,
    calls: Vec<Value>,
    root_call: Value,
    root_body: String,
    root_before: String,
    terms_on_root: bool,
    /// The accepted readings of the staged plan, term by term, in term order.
    accepted_readings: Vec<Value>,
}

/// Whether a character may occur in an accepted Latin reading: the tone-marked vowels, digits (a
/// term such as `3G` reads `3g`), the apostrophe, and spaces. A reading is never a second
/// transliteration input (D183, D184).
fn is_reading_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
        || matches!(
            ch,
            'ā' | 'á'
                | 'ǎ'
                | 'à'
                | 'ē'
                | 'é'
                | 'ě'
                | 'è'
                | 'ī'
                | 'í'
                | 'ǐ'
                | 'ì'
                | 'ō'
                | 'ó'
                | 'ǒ'
                | 'ò'
                | 'ū'
                | 'ú'
                | 'ǔ'
                | 'ù'
                | 'ǖ'
                | 'ǘ'
                | 'ǚ'
                | 'ǜ'
                | 'ü'
                | 'Ü'
                | 'ń'
                | 'ň'
                | 'ǹ'
                | 'ḿ'
        )
        || ch == '\''
        || ch == ' '
}

/// The sort key of one accepted reading: its base letters, so a tone mark does not reorder a part
/// (a byte order would put `xué` before `xìn`). Deterministic and platform independent (D184).
fn reading_key(reading: &str) -> String {
    reading
        .chars()
        .map(|ch| match ch {
            'ā' | 'á' | 'ǎ' | 'à' => 'a',
            'ē' | 'é' | 'ě' | 'è' => 'e',
            'ī' | 'í' | 'ǐ' | 'ì' => 'i',
            'ō' | 'ó' | 'ǒ' | 'ò' => 'o',
            'ū' | 'ú' | 'ǔ' | 'ù' => 'u',
            'ǖ' | 'ǘ' | 'ǚ' | 'ǜ' => 'ü',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}

/// `-Readings <file>`: `{"schema_version":1,"readings":{"学习":{"reading":"xué xí","initial":"x"}}}`.
/// The drafting job supplies a reading and its initial; this step checks their shape only (D183).
/// A term with no reading, a `null` initial, or `"uncertain": true` is unclassified: it stays in
/// the `其他` bucket and in the unresolved-reading report until it is corrected (AC03). Returns
/// the accepted readings, the unresolved rows, and the shape faults.
fn read_readings(
    path: &str,
    terms: &[TermDraft],
) -> Result<(BTreeMap<String, Reading>, Vec<Value>, Vec<Value>)> {
    let mut accepted = BTreeMap::new();
    let mut unresolved = Vec::new();
    let mut faults = Vec::new();
    if path.trim().is_empty() || !crate::ps::exists(path) {
        for term in terms {
            unresolved.push(json!({"term": term.term, "reading": "", "reason": "missing"}));
        }
        return Ok((accepted, unresolved, faults));
    }
    let document = canon::read_json(path)?;
    if document["schema_version"] != json!(1) {
        bail!("the readings file is not schema version 1");
    }
    let readings = document["readings"]
        .as_object()
        .context("the readings file has no `readings` object")?;
    for (term, value) in readings {
        let reading = value["reading"].as_str().unwrap_or_default().trim().to_string();
        let uncertain = value["uncertain"] == json!(true);
        let initial = match value.get("initial").map(|initial| initial.as_str()) {
            Some(Some(text)) => {
                let mut chars = text.trim().chars();
                match (chars.next(), chars.next()) {
                    (Some(ch), None) if ch.is_ascii_alphanumeric() => Some(ch.to_ascii_lowercase()),
                    _ => {
                        faults.push(json!({
                            "fault": "reading_initial",
                            "term": term,
                            "detail": format!("the initial `{text}` is not one Latin letter or digit; the term stays in 其他"),
                        }));
                        None
                    }
                }
            }
            _ => None,
        };
        let shape_ok = !reading.is_empty() && reading.chars().all(is_reading_char);
        if !shape_ok && !uncertain {
            faults.push(json!({
                "fault": "reading_shape",
                "term": term,
                "detail": "a reading is Latin letters, tone marks, spaces and apostrophes; a missing or uncertain reading stays in 其他",
            }));
        }
        if uncertain || initial.is_none() || !shape_ok {
            unresolved.push(json!({
                "term": term,
                "reading": reading,
                "reason": if uncertain { "uncertain" } else { "missing" },
            }));
            continue;
        }
        accepted.insert(term.clone(), Reading { reading, initial });
    }
    for term in terms {
        if !accepted.contains_key(&term.term)
            && !unresolved
                .iter()
                .any(|row| crate::ps::s(&row["term"]) == term.term)
        {
            unresolved.push(json!({"term": term.term, "reading": "", "reason": "missing"}));
        }
    }
    unresolved.sort_by(|a, b| canon::ordinal_cmp(&crate::ps::s(&a["term"]), &crate::ps::s(&b["term"])));
    Ok((accepted, unresolved, faults))
}

/// Group accepted terms into the accepted pinyin-initial bands, at most [`PART_SPLIT`] rows each
/// (D177, D183, D290). The Latin bands come first, then the digit bucket, then `其他`; inside a band
/// the terms are ordered by their accepted reading, so a rebuild from the same plan is
/// deterministic on every platform, with no other dependency or service (D184).
fn bucket_parts(terms: &[TermDraft], readings: &BTreeMap<String, Reading>) -> Vec<Part> {
    let mut latin: Vec<(char, String, String, usize)> = Vec::new();
    let mut digit: Vec<(char, String, String, usize)> = Vec::new();
    let mut other: Vec<(String, usize)> = Vec::new();
    for (index, term) in terms.iter().enumerate() {
        match readings.get(&term.term) {
            Some(reading) => match reading.initial {
                Some(initial) if initial.is_ascii_lowercase() => latin.push((
                    initial,
                    reading_key(&reading.reading),
                    term.term.clone(),
                    index,
                )),
                Some(initial) if initial.is_ascii_digit() => digit.push((
                    initial,
                    reading_key(&reading.reading),
                    term.term.clone(),
                    index,
                )),
                _ => other.push((term.term.clone(), index)),
            },
            None => other.push((term.term.clone(), index)),
        }
    }
    latin.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)).then_with(|| canon::ordinal_cmp(&a.2, &b.2)));
    digit.sort_by(|a, b| (a.0, &a.1).cmp(&(b.0, &b.1)).then_with(|| canon::ordinal_cmp(&a.2, &b.2)));
    other.sort_by(|a, b| canon::ordinal_cmp(&a.0, &b.0));
    let mut parts = Vec::new();
    for (low, high, label) in BANDS {
        let rows = latin
            .iter()
            .filter(|(initial, ..)| *initial >= low && *initial <= high)
            .map(|(_, _, _, index)| *index)
            .collect::<Vec<_>>();
        for (label, terms) in contiguous_parts(&rows, label) {
            parts.push(Part {
                label,
                bucket: "latin",
                terms,
            });
        }
    }
    let digit_rows = digit
        .iter()
        .map(|(_, _, _, index)| *index)
        .collect::<Vec<_>>();
    for (label, terms) in contiguous_parts(&digit_rows, DIGIT_LABEL) {
        parts.push(Part {
            label,
            bucket: "digit",
            terms,
        });
    }
    let other_rows = other.iter().map(|(_, index)| *index).collect::<Vec<_>>();
    for (label, terms) in contiguous_parts(&other_rows, OTHER_LABEL) {
        parts.push(Part {
            label,
            bucket: "other",
            terms,
        });
    }
    parts
}

/// Cut one bucket's ordered rows into contiguous parts of at most [`PART_SPLIT`] rows (D290). A
/// split keeps the bucket's label and adds the part number, so every label stays an accepted
/// initial range and the declaration order stays deterministic.
fn contiguous_parts(rows: &[usize], band: &str) -> Vec<(String, Vec<usize>)> {
    let mut parts: Vec<(String, Vec<usize>)> = Vec::new();
    let mut index = 0;
    while index < rows.len() {
        let end = (index + PART_SPLIT).min(rows.len());
        let label = if parts.is_empty() {
            band.to_string()
        } else {
            format!("{band}({})", parts.len() + 1)
        };
        parts.push((label, rows[index..end].to_vec()));
        index = end;
    }
    parts
}

/// The title of one part Card: the accepted range it navigates (D181).
fn part_title(label: &str) -> String {
    format!("词条 {label}")
}

/// One part's body: the register's own term-row grammar, so a part is a navigation Card and not a
/// second register (D178, D181).
fn part_body(terms: &[TermDraft], part: &Part) -> String {
    let rows = part
        .terms
        .iter()
        .map(|index| {
            let term = &terms[*index];
            let links = term
                .targets
                .iter()
                .map(|address| format!("[[{address}]]"))
                .collect::<Vec<_>>()
                .join("、");
            format!("- {}：{links}", term.term)
        })
        .collect::<Vec<_>>()
        .join("\n");
    if rows.is_empty() {
        format!("## 词条")
    } else {
        format!("## 词条\n{rows}")
    }
}

/// The register root's final body: its own groups and, when the rows moved into parts, the
/// `词条分册` declaration that names them, each part by `{{part:N}}` until apply knows its address
/// (D176, D177, D179, D189). Without parts the rows stay on the root and there is no declaration.
fn root_body_text(
    current: &str,
    terms: &[TermDraft],
    declarations: &[(String, String)],
) -> String {
    if declarations.is_empty() {
        return register_body(current, terms);
    }
    let mut body = register_body(current, &[]);
    body.push_str("\n\n## 词条分册\n");
    for (label, link) in declarations {
        body.push_str(&format!("\n- [[{link}]] {label}"));
    }
    body.trim_end().to_string()
}

/// Stage the exact identified call list that publishes the register and its parts (§8.5, D189,
/// D190, D191): the root shell when the Workspace has no register, the new parts as Side successors
/// of the root, the changed existing parts in place, the emptied obsolete parts, and the root last.
/// A part address is reused by its accepted label, then by its position, so a rebuild keeps it and
/// allocates a new Side successor only for a genuinely new part (D185, AC04).
fn stage_register(
    current: &Value,
    terms: &[TermDraft],
    parts: &[Part],
    readings: &BTreeMap<String, Reading>,
) -> Result<Stage> {
    let missing = current["missing"] == json!(true);
    let current_body = crate::ps::s(&current["body"]);
    let declared = crate::ps::pipe(&current["parts"]);
    // apply compares the root it reads back with this body before it publishes the root: an existing register's
    // is its current body (left empty, every first plan with parts over an existing root stopped half-published).
    let mut stage = Stage {
        root_before: if missing { String::new() } else { current_body.clone() },
        ..Stage::default()
    };
    let mut calls: Vec<Value> = Vec::new();
    if missing {
        calls.push(json!({"tool": "create_topic", "arguments": {"title": INDEX_TITLE, "body": INDEX_BODY}}));
    }
    let root_ref = if missing { "@@0".to_string() } else { crate::ps::s(&current["address"]) };
    // A new register is created as a shell first so its parts have a parent to be filed under; the
    // root's own rows and declaration are published last, in their own identified call (D189).
    let parts_expected = !parts.is_empty();
    if missing && parts_expected {
        let shell = register_body(&current_body, &[]);
        stage.root_before = shell.clone();
        calls.push(json!({"tool": "create_successor", "arguments": {
            "at": root_ref, "type": "hub", "level": "register", "title": DEFAULT_TITLE, "body": shell,
        }}));
    }
    let root_ref = if missing && parts_expected {
        "@@1".to_string()
    } else if missing {
        "@@0".to_string()
    } else {
        root_ref
    };
    if missing && !parts_expected {
        // A small register is created complete in one Card, as before.
        let body = register_body(&current_body, terms);
        stage.root_before = String::new();
        calls.push(json!({"tool": "create_successor", "arguments": {
            "at": "@@0", "type": "hub", "level": "register", "title": DEFAULT_TITLE, "body": body,
        }}));
        stage.calls = calls;
        stage.root_body = body;
        stage.terms_on_root = true;
        stage.accepted_readings = accepted_reading_rows(terms, readings);
        return Ok(stage);
    }
    // Reuse a declared part by its accepted label, then by its position (D185).
    let mut reused: Vec<Option<usize>> = vec![None; parts.len()];
    let mut used: Vec<bool> = vec![false; declared.len()];
    for (index, part) in parts.iter().enumerate() {
        if let Some(found) = declared
            .iter()
            .enumerate()
            .position(|(at, row)| !used[at] && crate::ps::s(&row["label"]) == part.label)
        {
            reused[index] = Some(found);
            used[found] = true;
        }
    }
    for (index, part) in parts.iter().enumerate() {
        let _ = part;
        if reused[index].is_none() && index < declared.len() && !used[index] {
            reused[index] = Some(index);
            used[index] = true;
        }
    }
    let mut links: Vec<(String, String)> = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let body = part_body(terms, part);
        let title = part_title(&part.label);
        let names = part
            .terms
            .iter()
            .map(|term| terms[*term].term.clone())
            .collect::<Vec<_>>();
        match reused[index] {
            Some(existing) => {
                let row = &declared[existing];
                let address = crate::ps::s(&row["address"]);
                let before = crate::ps::s(&row["body"]);
                let card = row["card"].clone();
                let before_title = crate::ps::s(&card["title"]);
                if before != body || before_title != title {
                    let mut arguments = json!({
                        "at": address,
                        "body": body,
                        "base_fingerprint": crate::calls::authored_fingerprint(&card),
                    });
                    if before_title != title {
                        arguments["title"] = json!(title);
                    }
                    calls.push(json!({"tool": "edit_card", "arguments": arguments}));
                }
                links.push((part.label.clone(), address.clone()));
                stage.parts.push(StagedPart {
                    label: part.label.clone(),
                    bucket: part.bucket,
                    title,
                    address: Some(address),
                    body,
                    terms: names,
                    call: None,
                    reused: true,
                });
            }
            None => {
                calls.push(json!({"tool": "create_successor", "arguments": {
                    "at": root_ref, "kind": "side", "title": title, "body": body,
                }}));
                let call = calls.len() - 1;
                links.push((part.label.clone(), format!("{{{{part:{index}}}}}")));
                stage.parts.push(StagedPart {
                    label: part.label.clone(),
                    bucket: part.bucket,
                    title,
                    address: None,
                    body,
                    terms: names,
                    call: Some(call),
                    reused: false,
                });
            }
        }
    }
    // A declared part the plan no longer needs keeps its address and stays declared, emptied of
    // its rows, pending a separate deletion plan (D185, AC04).
    for (index, row) in declared.iter().enumerate() {
        if used[index] {
            continue;
        }
        let address = crate::ps::s(&row["address"]);
        let label = crate::ps::s(&row["label"]);
        let body = "## 词条".to_string();
        if crate::ps::s(&row["body"]) != body {
            let card = row["card"].clone();
            calls.push(json!({"tool": "edit_card", "arguments": {
                "at": address,
                "body": body,
                "base_fingerprint": crate::calls::authored_fingerprint(&card),
            }}));
        }
        links.push((label.clone(), address.clone()));
        stage.obsolete.push(StagedObsolete {
            label,
            address,
            body,
        });
    }
    let root_body = root_body_text(&current_body, terms, &links);
    let root_call = if missing || root_body != current_body {
        json!({"tool": "edit_card", "arguments": {"at": Value::Null, "body": root_body}})
    } else {
        Value::Null
    };
    stage.calls = calls;
    stage.root_call = root_call;
    stage.root_body = root_body;
    stage.terms_on_root = false;
    stage.accepted_readings = accepted_reading_rows(terms, readings);
    Ok(stage)
}

/// The accepted readings of the staged terms, in term order, for the plan's review evidence.
fn accepted_reading_rows(terms: &[TermDraft], readings: &BTreeMap<String, Reading>) -> Vec<Value> {
    let mut rows = terms
        .iter()
        .filter_map(|term| {
            readings.get(&term.term).map(|reading| {
                json!({
                    "term": term.term,
                    "reading": reading.reading,
                    "initial": reading.initial.map(|initial| initial.to_string()),
                })
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| canon::ordinal_cmp(&crate::ps::s(&a["term"]), &crate::ps::s(&b["term"])));
    rows
}

/// Publish a body that still holds `{{part:N}}` references by filling the addresses of the N-th
/// planned part. An unresolved reference is refused, never written (D189).
fn fill_parts(body: &str, addresses: &[String]) -> Result<String> {
    let mut text = body.to_string();
    for (index, address) in addresses.iter().enumerate() {
        text = text.replace(&format!("{{{{part:{index}}}}}"), address);
    }
    if text.contains("{{part:") {
        bail!("the staged register body holds a part reference with no address");
    }
    Ok(text)
}

// --- the review gate's re-staging (S27, #119) -----------------------------------------------------

/// The term rows of a plan, as the staging code sees them.
fn term_drafts(rows: &[Value]) -> Vec<TermDraft> {
    rows.iter()
        .map(|row| TermDraft {
            term: crate::ps::s(&row["term"]),
            targets: crate::ps::pipe(&row["targets"]).into_iter().map(crate::ps::s).collect(),
            sources: crate::ps::pipe(&row["sources"])
                .into_iter()
                .map(|source| RowSource {
                    book: crate::ps::s(&source["book"]),
                    table: crate::ps::s(&source["term_table"]),
                    row: source["row"].as_u64().unwrap_or_default() as usize,
                    meaning: crate::ps::s(&source["meaning"]),
                    spelling: crate::ps::s(&source["spelling"]),
                    preferred: source["preferred"] == json!(true),
                })
                .collect(),
            ambiguous: row["ambiguous"] == json!(true),
            overflow: row["overflow"].as_u64().unwrap_or_default() as usize,
            reviewed: crate::ps::pipe(&row["reviewed"]).into_iter().map(crate::ps::s).collect(),
            read_only: crate::ps::pipe(&row["read_only_targets"]).into_iter().map(crate::ps::s).collect(),
        })
        .collect()
}

/// The accepted readings of a plan, as the stager's `-Readings` input parses them.
fn accepted_readings(value: &Value) -> BTreeMap<String, Reading> {
    crate::ps::pipe(&value["accepted"])
        .into_iter()
        .filter_map(|row| {
            let initial = crate::ps::s(&row["initial"]).chars().next()?;
            Some((
                crate::ps::s(&row["term"]),
                Reading {
                    reading: crate::ps::s(&row["reading"]),
                    initial: Some(initial),
                },
            ))
        })
        .collect()
}

/// Re-stage one register publication from a reviewed plan's term rows, keeping only the accepted
/// ones (S27, #119). The gate presents those rows for a decision; the rows it keeps are published
/// through [`Register::publish_plan`] with the same part layout, band grouping and root body the
/// register compiler stages, so a declined term simply is not written and a part left without
/// rows keeps its address (D185). `current` is a fresh `get_register` view of the register.
pub(crate) fn restage(plan: &Value, accepted: &[Value], current: &Value) -> Result<Value> {
    let terms = term_drafts(accepted);
    let readings = accepted_readings(&plan["readings"]);
    // The same part decision the plan took: a register that is already split keeps its parts, and
    // an over-120-row register is staged as parts (D179, D185).
    let declared = crate::ps::pipe(&current["parts"]).len();
    let parts = if terms.len() > PART_SPLIT || declared > 0 {
        bucket_parts(&terms, &readings)
    } else {
        Vec::new()
    };
    let staged = stage_register(current, &terms, &parts, &readings)?;
    let accepted_rows = accepted.to_vec();
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "archive": plan["archive"].clone(),
        "register": {
            "address": current["address"],
            "missing": current["missing"],
            "entries": current["entries"],
            "complete": current["complete"],
            "parts": crate::ps::pipe(&current["parts"]).len(),
        },
        "terms": accepted_rows,
        "unresolved": plan["unresolved"],
        "faults": plan["faults"],
        "excluded": plan["excluded"],
        "review": plan["review"],
        "readings": plan["readings"],
        "parts": staged.parts.iter().map(StagedPart::json).collect::<Vec<_>>(),
        "obsolete_parts": staged.obsolete.iter().map(StagedObsolete::json).collect::<Vec<_>>(),
        "terms_on_root": staged.terms_on_root,
        "root_before": staged.root_before,
        "body": staged.root_body,
        "calls": staged.calls,
        "root_call": staged.root_call,
    });
    payload["plan_id"] = json!(plan_id(&payload));
    payload["restaged_from"] = plan["plan_id"].clone();
    payload["created_at"] = json!(canon::utc_now_o());
    Ok(payload)
}

// --- apply ----------------------------------------------------------------------------------------

impl Register {
    fn apply(&mut self) -> Result<()> {
        let path = self.plan_file();
        if !crate::ps::exists(&path) {
            bail!("no staged register plan at `{path}`; run -Step plan first");
        }
        let plan = canon::read_json(&path)?;
        if plan["schema_version"] != json!(SCHEMA_VERSION) {
            bail!("the staged register plan is not schema version {SCHEMA_VERSION}");
        }
        let (identity, _revision) = self.archive_identity()?;
        let planned_identity = crate::ps::s(&plan["archive"]["identity"]["id"]);
        if planned_identity != crate::ps::s(&identity["id"]) {
            bail!(
                "the staged plan belongs to archive `{planned_identity}`, not `{}`; stage it again",
                crate::ps::s(&identity["id"])
            );
        }
        let body = plan["body"].as_str().unwrap_or_default().to_string();
        if body.trim().is_empty() {
            bail!("the staged register plan has no body");
        }
        // A plan staged by an older build predates the staged publication calls: publishing half of
        // it is worse than refusing it.
        if plan.get("terms_on_root").is_none() {
            bail!("the staged register plan predates the identified parts publication; run -Step plan again");
        }
        // More than 120 term rows stay on the root only in a legacy register; a staged publication
        // puts them into parts, so a plan that keeps a large root is refused instead of published
        // (D179, AC01).
        if plan["terms_on_root"] == json!(true)
            && crate::ps::pipe(&plan["terms"]).len() > PART_SPLIT
        {
            bail!(
                "the staged plan keeps {} term rows on the register root; a register over {PART_SPLIT} rows is staged as 词条分册 parts",
                crate::ps::pipe(&plan["terms"]).len()
            );
        }
        // The plan's operation prefix: the same accepted plan reuses its receipts, so retrying an
        // interrupted publication replays each committed call instead of creating a second Card
        // (§8.5, D189). The plan id is the acceptance, not the attempt.
        let short = crate::ps::s(&plan["plan_id"])
            .trim_start_matches("rp-")
            .chars()
            .take(16)
            .collect::<String>();
        let base = if short.is_empty() {
            "register/plan".to_string()
        } else {
            format!("register/{short}")
        };
        let summary = self.publish_plan(&plan, &base)?;
        util::print_last(&summary)
    }

    /// Publish one staged register plan through its identified calls and verify the exact root,
    /// parts, entries and revision (§8.5, D189, D190). `-Step apply` publishes the plan in its
    /// Work folder; the review gate (S27, `publish`) re-stages the accepted term rows first and
    /// publishes that subset through this same path, with a receipt base of its own.
    pub(crate) fn publish_plan(&mut self, plan: &Value, base: &str) -> Result<Value> {
        let planned = planned_terms(plan)?;
        let body = crate::ps::s(&plan["body"]);
        // Every planned target must still exist: a moved or deleted Card makes the staged
        // addresses stale, and the write path would otherwise refuse a new broken Link. A
        // review-selected human Card is a target too, and stays read-only (D174).
        let (present, human) = self.workspace_addresses()?;
        let gone = planned
            .iter()
            .flat_map(|(_, targets)| targets.iter())
            .filter(|address| !present.contains(*address))
            .cloned()
            .collect::<Vec<_>>();
        if !gone.is_empty() {
            bail!(
                "the staged plan names Cards that are in neither Workspace: {}; run -Step plan again",
                gone.join(", ")
            );
        }
        let current = self.current_register(true)?;
        // Tree Lock S16 (#139): a publication writes only at unlocked destinations (R331). The
        // check runs before the first write and names the Locked register tree with the archive's
        // typed refusal; a register that becomes Locked after it is stopped by the writer itself.
        let calls = plan["calls"].as_array().cloned().unwrap_or_default();
        let mut destinations: Vec<(String, bool)> = Vec::new();
        let current_address = crate::ps::s(&current["address"]);
        if !current_address.is_empty() {
            destinations.push((current_address, false));
        }
        for call in &calls {
            let at = crate::ps::s(&call["arguments"]["at"]);
            if at.is_empty() || at.starts_with("@@") {
                continue;
            }
            destinations.push((at, crate::ps::s(&call["tool"]) != "edit_card"));
        }
        for part in crate::ps::pipe(&plan["parts"]) {
            let address = crate::ps::s(&part["address"]);
            if !address.is_empty() {
                destinations.push((address, false));
            }
        }
        if let Some(refusal) = self.locked_destination(&destinations)? {
            return Err(anyhow::Error::new(crate::lockcheck::Stopped(refusal)));
        }
        let read_only = planned
            .iter()
            .flat_map(|(_, targets)| targets.iter())
            .filter(|address| human.contains(*address))
            .count();
        let prefix = base.to_string();
        let mut wrote = 0;
        let mut replayed = 0;
        let mut receipts: Vec<String> = Vec::new();
        if !calls.is_empty() {
            for receipt in crate::calls::invoke_identified_staged(
                self.zt()?,
                &calls,
                &format!("{prefix}/1/parts"),
                None,
            )? {
                if receipt["replayed"] == json!(true) {
                    replayed += 1;
                } else {
                    wrote += 1;
                }
                receipts.push(crate::ps::s(&receipt["result"]["address"]));
            }
            if receipts.len() != calls.len() {
                bail!(
                    "the parts publication answered {} receipts for {} calls",
                    receipts.len(),
                    calls.len()
                );
            }
        }
        let missing = current["missing"] == json!(true);
        let root_address = if missing {
            // The register was created in the parts batch: the Topic, then the register shell.
            receipts.get(1).cloned().unwrap_or_default()
        } else {
            crate::ps::s(&current["address"])
        };
        if root_address.is_empty() {
            bail!("the staged plan published no register root");
        }
        // The root names each part: a reused part by its staged address, a created one by the
        // address its receipt returned (D189).
        let planned_parts = crate::ps::pipe(&plan["parts"]);
        let mut part_addresses: Vec<String> = Vec::new();
        for part in &planned_parts {
            let reused = crate::ps::s(&part["address"]);
            let address = if !reused.is_empty() {
                reused
            } else {
                let call = part["call"].as_u64().unwrap_or(u64::MAX) as usize;
                receipts.get(call).cloned().unwrap_or_default()
            };
            if address.is_empty() {
                bail!(
                    "the staged plan resolved no address for the part `{}`",
                    crate::ps::s(&part["label"])
                );
            }
            part_addresses.push(address);
        }
        let final_body = fill_parts(&body, &part_addresses)?;
        // The parts, and the root, are verified before the root is rewritten (D190): the root is
        // published only after its parts are exact. A retry over a publication that already
        // finished finds the root at its final body and has nothing left to publish.
        let mut root_final = false;
        if !planned_parts.is_empty() {
            let mut wanted: Vec<String> = part_addresses.clone();
            wanted.push(root_address.clone());
            let read = self.call(
                "get_cards",
                json!({"addresses": wanted.iter().map(|at| json!(at)).collect::<Vec<_>>(), "view": "text"}),
            )?;
            if read["complete"] != json!(true) {
                bail!(
                    "a published part could not be read back: missing={}",
                    crate::ps::json_compact(&read["missing"])
                );
            }
            let body_at = |at: &str| {
                crate::ps::pipe(&read["cards"])
                    .into_iter()
                    .find(|card| crate::ps::s(&card["address"]) == at)
                    .map(|card| crate::ps::s(&card["body"]))
                    .unwrap_or_default()
            };
            let root_before = crate::ps::s(&plan["root_before"]);
            let root_now = body_at(&root_address);
            root_final = root_now == final_body;
            if root_now != root_before && !root_final {
                bail!(
                    "the register changed while its parts were published: expected `{root_before}` or the staged body, read `{root_now}`"
                );
            }
            for (index, part) in planned_parts.iter().enumerate() {
                let staged = crate::ps::s(&part["body"]);
                if body_at(&part_addresses[index]) != staged {
                    bail!(
                        "the published part `{}` does not carry its staged body",
                        crate::ps::s(&part["label"])
                    );
                }
            }
        }
        // The root is published last, in its own identified call, with the addresses its parts
        // received.
        if !plan["root_call"].is_null() && !root_final {
            let mut call = plan["root_call"].clone();
            let card = self.call("get_card", json!({"at": root_address}))?;
            call["arguments"]["at"] = json!(root_address);
            call["arguments"]["body"] = json!(final_body);
            call["arguments"]["base_fingerprint"] = json!(crate::calls::authored_fingerprint(&card));
            for receipt in crate::calls::invoke_identified(
                self.zt()?,
                &[call],
                &format!("{prefix}/1/root"),
                None,
            )? {
                if receipt["replayed"] == json!(true) {
                    replayed += 1;
                } else {
                    wrote += 1;
                }
            }
        }
        // A fresh read must confirm the exact root, parts, targets, and revision (§8.5, D190): the
        // publication unit is complete only after every page, both bodies, and the layout agree.
        let (view, published) = self.published()?;
        let mut divergence = Vec::new();
        for (term, targets) in &planned {
            match published.iter().find(|(name, _)| name == term) {
                Some((_, found)) if found == targets => {}
                Some((_, found)) => divergence.push(json!({
                    "term": term, "planned": targets, "published": found,
                })),
                None => divergence.push(json!({"term": term, "planned": targets, "published": Value::Null})),
            }
        }
        let extra = published
            .iter()
            .filter(|(term, _)| !planned.iter().any(|(name, _)| name == term))
            .map(|(term, targets)| json!({"term": term, "targets": targets}))
            .collect::<Vec<_>>();
        if !divergence.is_empty() || !extra.is_empty() || view["complete"] != json!(true) {
            bail!(
                "the register was published at `{root_address}` but does not verify: diverged={} extra={} complete={} faults={}",
                crate::ps::json_compact(&Value::Array(divergence)),
                crate::ps::json_compact(&Value::Array(extra)),
                view["complete"],
                crate::ps::json_compact(&view["faults"])
            );
        }
        if !view["duplicates"].as_array().is_none_or(Vec::is_empty) {
            bail!(
                "the published register still serves a row twice: {}",
                crate::ps::json_compact(&view["duplicates"])
            );
        }
        // One more fresh read for the exact bodies: the root, every planned part, and every
        // obsolete part must carry exactly the staged text (D190, AC08).
        let mut wanted: Vec<String> = vec![root_address.clone()];
        wanted.extend(part_addresses.iter().cloned());
        for part in crate::ps::pipe(&plan["obsolete_parts"]) {
            wanted.push(crate::ps::s(&part["address"]));
        }
        let read = self.call(
            "get_cards",
            json!({"addresses": wanted.iter().map(|at| json!(at)).collect::<Vec<_>>(), "view": "text"}),
        )?;
        if read["complete"] != json!(true) {
            bail!(
                "the published register or a part could not be read back: missing={}",
                crate::ps::json_compact(&read["missing"])
            );
        }
        let mut bodies = BTreeMap::new();
        for card in crate::ps::pipe(&read["cards"]) {
            bodies.insert(crate::ps::s(&card["address"]), crate::ps::s(&card["body"]));
        }
        if bodies.get(&root_address).map(String::as_str) != Some(final_body.as_str()) {
            bail!(
                "the published register body differs from the staged body:\nstaged:\n{final_body}\nread:\n{}",
                bodies.get(&root_address).cloned().unwrap_or_default()
            );
        }
        for (index, part) in planned_parts.iter().enumerate() {
            let staged = crate::ps::s(&part["body"]);
            if bodies.get(&part_addresses[index]).map(String::as_str) != Some(staged.as_str()) {
                bail!(
                    "the published part `{}` does not carry its staged body",
                    crate::ps::s(&part["label"])
                );
            }
        }
        for part in crate::ps::pipe(&plan["obsolete_parts"]) {
            let address = crate::ps::s(&part["address"]);
            if bodies.get(&address).map(String::as_str) != Some(crate::ps::s(&part["body"]).as_str()) {
                bail!("the obsolete part `{address}` is not the empty navigation entry the plan staged");
            }
        }
        let verified_parts = verify_published_layout(plan, &view, &part_addresses)?;
        Ok(json!({
            "ok": true,
            "step": "apply",
            "plan_id": plan["plan_id"],
            "register": root_address,
            "created": if missing { 2 } else { 0 },
            "changed": wrote > 0,
            "wrote": wrote,
            "replayed": replayed,
            "parts": verified_parts,
            "obsolete_parts": plan["obsolete_parts"],
            "terms_on_root": plan["terms_on_root"],
            "entries": published.len(),
            "read_only": read_only,
            "unresolved": plan["unresolved"].as_array().map_or(0, Vec::len),
            "readings": {
                "accepted": plan["readings"]["accepted"].as_array().map_or(0, Vec::len),
                "unresolved": plan["readings"]["unresolved"].as_array().map_or(0, Vec::len),
            },
            "review": plan["review"],
            "complete": view["complete"],
            "publication": "complete",
            "archive_revision": view["archive_revision"],
            "verified": true,
        }))
    }

    fn check(&mut self) -> Result<()> {
        let (view, published) = self.published()?;
        let mut missing = Vec::new();
        let mut diverged = Vec::new();
        let mut extra = Vec::new();
        let mut plan_id = Value::Null;
        let mut unresolved = 0;
        let mut read_only = 0;
        let mut plan_parts = Vec::new();
        let mut obsolete_parts = Value::Null;
        let mut readings = Value::Null;
        if crate::ps::exists(&self.plan_file()) {
            let plan = canon::read_json(&self.plan_file())?;
            plan_id = plan["plan_id"].clone();
            unresolved = plan["unresolved"].as_array().map_or(0, Vec::len);
            readings = plan["readings"].clone();
            obsolete_parts = plan["obsolete_parts"].clone();
            read_only = crate::ps::pipe(&plan["terms"])
                .into_iter()
                .map(|row| crate::ps::pipe(&row["read_only_targets"]).len())
                .sum::<usize>();
            let planned = planned_terms(&plan)?;
            for (term, targets) in &planned {
                match published.iter().find(|(name, _)| name == term) {
                    None => missing.push(json!({"term": term, "planned": targets})),
                    Some((_, found)) if found != targets => {
                        diverged.push(json!({"term": term, "planned": targets, "published": found}))
                    }
                    Some(_) => {}
                }
            }
            extra = published
                .iter()
                .filter(|(term, _)| !planned.iter().any(|(name, _)| name == term))
                .map(|(term, targets)| json!({"term": term, "targets": targets}))
                .collect();
            // The layout is part of the acceptance: a published part whose label or address moved,
            // or an obsolete part that is declared empty, is reported here too (AC04, AC08).
            let published_parts = crate::ps::pipe(&view["parts"]);
            for part in crate::ps::pipe(&plan["parts"]) {
                let label = crate::ps::s(&part["label"]);
                let address = crate::ps::s(&part["address"]);
                match published_parts
                    .iter()
                    .find(|declared| declared["place"] == json!("ok") && crate::ps::s(&declared["label"]) == label)
                {
                    None => plan_parts.push(json!({"label": label, "address": address, "published": Value::Null})),
                    Some(declared) if !address.is_empty() && crate::ps::s(&declared["address"]) != address => {
                        plan_parts.push(json!({
                            "label": label,
                            "address": address,
                            "published": declared["address"],
                        }))
                    }
                    Some(_) => {}
                }
            }
        }
        util::print_last(&json!({
            "ok": true,
            "step": "check",
            "plan_id": plan_id,
            "register": view["register"],
            "register_missing": view["register_missing"],
            "entries": published.len(),
            "read_only": read_only,
            "unresolved": unresolved,
            "readings": readings,
            "parts": published_parts_json(&view),
            "plan_parts": plan_parts,
            "obsolete_parts": obsolete_parts,
            "duplicates": view["duplicates"],
            "complete": view["complete"],
            "faults": view["faults"],
            "missing": missing,
            "diverged": diverged,
            "extra": extra,
            "archive_revision": view["archive_revision"],
        }))
    }
}

/// The published parts of a register view: label, address, place, and row count, in declaration
/// order.
fn published_parts_json(view: &Value) -> Vec<Value> {
    crate::ps::pipe(&view["parts"])
        .into_iter()
        .map(|part| {
            json!({
                "label": part["label"],
                "address": part["address"],
                "place": part["place"],
                "entries": part["entries"],
            })
        })
        .collect()
}

/// Confirm the published layout is the accepted one: the declared parts are exactly the planned
/// parts in order, each at the address its receipt returned with as many rows as it was staged
/// with, and an obsolete part keeps its declaration and is empty (D185, D190, AC04, AC08).
fn verify_published_layout(
    plan: &Value,
    view: &Value,
    part_addresses: &[String],
) -> Result<Vec<Value>> {
    let declared = crate::ps::pipe(&view["parts"]);
    let planned = crate::ps::pipe(&plan["parts"]);
    let obsolete = crate::ps::pipe(&plan["obsolete_parts"]);
    if planned.is_empty() {
        if !declared.is_empty() {
            bail!(
                "the published register declares parts the plan did not stage: {}",
                crate::ps::json_compact(&view["parts"])
            );
        }
        return Ok(Vec::new());
    }
    if declared.len() != planned.len() + obsolete.len() {
        bail!(
            "the published register declares {} parts; the plan staged {} and kept {} obsolete",
            declared.len(),
            planned.len(),
            obsolete.len()
        );
    }
    let mut verified = Vec::new();
    for (index, part) in planned.iter().enumerate() {
        let Some(found) = declared.get(index) else {
            bail!(
                "the published register does not declare the part `{}`",
                crate::ps::s(&part["label"])
            );
        };
        let label = crate::ps::s(&part["label"]);
        if crate::ps::s(&found["label"]) != label {
            bail!(
                "the published part on line {} carries the label `{}`, not `{label}`",
                index + 1,
                crate::ps::s(&found["label"])
            );
        }
        if crate::ps::s(&found["address"]) != part_addresses[index] {
            bail!(
                "the published part `{label}` is at `{}`, not `{}`",
                crate::ps::s(&found["address"]),
                part_addresses[index]
            );
        }
        let rows = part["terms"].as_array().map_or(0, Vec::len);
        if found["place"] != json!("ok") || found["entries"].as_u64() != Some(rows as u64) {
            bail!(
                "the published part `{label}` is not a placed part with {rows} rows: {}",
                crate::ps::json_compact(found)
            );
        }
        verified.push(json!({
            "label": label,
            "address": part_addresses[index],
            "entries": found["entries"],
            "reused": part["reused"],
        }));
    }
    for (offset, part) in obsolete.iter().enumerate() {
        let Some(found) = declared.get(planned.len() + offset) else {
            bail!(
                "the obsolete part `{}` is no longer declared",
                crate::ps::s(&part["address"])
            );
        };
        if crate::ps::s(&found["address"]) != crate::ps::s(&part["address"])
            || found["entries"].as_u64() != Some(0)
        {
            bail!(
                "the obsolete part `{}` is not an empty navigation entry: {}",
                crate::ps::s(&part["address"]),
                crate::ps::json_compact(found)
            );
        }
    }
    Ok(verified)
}

fn planned_terms(plan: &Value) -> Result<Vec<(String, Vec<String>)>> {
    let mut terms = Vec::new();
    for row in crate::ps::pipe(&plan["terms"]) {
        let term = crate::ps::s(&row["term"]);
        let targets = crate::ps::pipe(&row["targets"])
            .into_iter()
            .map(crate::ps::s)
            .collect::<Vec<_>>();
        if term.is_empty() || targets.is_empty() || targets.len() > MAX_TARGETS {
            bail!("the staged plan holds a malformed term `{term}`");
        }
        terms.push((term, targets));
    }
    Ok(terms)
}

/// One register read that is not one catalog: a page sequence that crossed a revision (retried
/// once), or one that cannot be the whole catalog.
enum Merge {
    Changing(String),
    Fault(String),
}

/// The part faults that make the declared layout incoherent: a part the register names but cannot
/// own where it says (moved, missing, a Hub, listed twice, or nested). The compiler rewrites the
/// whole `词条分册` layout, so it refuses to plan over one of these and names the exact finding
/// instead of treating an arbitrary Card as a part (D182, AC05). An *unlisted* part (a Side
/// successor holding `词条` rows the root no longer declares) blocks a new plan too, because a fresh
/// acceptance would compound a half-published tree; `apply` alone tolerates it, since finishing an
/// interrupted publication is exactly what redeclares those Cards (D189, D306).
const PART_FAULTS: [&str; 5] = [
    "part-place",
    "part-missing",
    "part-hub",
    "part-duplicate",
    "part-nested",
];

fn obstructed_parts(view: &Value, tolerate_unlisted: bool) -> Result<()> {
    let obstructed = crate::ps::pipe(&view["faults"])
        .into_iter()
        .filter(|fault| {
            let id = crate::ps::s(&fault["fault"]);
            PART_FAULTS.contains(&id.as_str()) || (!tolerate_unlisted && id == "part-unlisted")
        })
        .map(|fault| format!("{}: {}", crate::ps::s(&fault["fault"]), crate::ps::s(&fault["detail"])))
        .collect::<Vec<_>>();
    if obstructed.is_empty() {
        return Ok(());
    }
    bail!(
        "the agent register's 词条分册 layout is obstructed and this step will not rewrite it: {}; repair the parts, then run -Step plan again",
        obstructed.join("; ")
    )
}

/// The pages as one catalog: every page at the same revision and of the same register, the pages
/// contiguous from offset 0, and their entries adding up to `entry_total` (§8.5, S10-AC06).
fn merged_terms(
    pages: &[Value],
) -> std::result::Result<(Value, Vec<(String, Vec<String>)>), Merge> {
    let Some(first) = pages.first() else {
        return Err(Merge::Fault(
            "the register read returned no page at all".into(),
        ));
    };
    let revision = first["archive_revision"].as_u64();
    let total = first["entry_total"].as_u64();
    let mut terms = Vec::new();
    let mut offset = 0u64;
    for page in pages {
        let page_revision = page["archive_revision"].as_u64();
        if page_revision != revision || page["register"] != first["register"] {
            return Err(Merge::Changing(format!(
                "the agent register changed while it was read (revision {} then {}, register {} then {}); run the step again",
                revision.map_or("none".to_string(), |value| value.to_string()),
                page_revision.map_or("none".to_string(), |value| value.to_string()),
                crate::ps::json_compact(&first["register"]),
                crate::ps::json_compact(&page["register"]),
            )));
        }
        if page["entry_total"].as_u64() != total {
            return Err(Merge::Changing(format!(
                "the agent register changed while it was read ({} entries then {}); run the step again",
                total.map_or("none".to_string(), |value| value.to_string()),
                page["entry_total"]
                    .as_u64()
                    .map_or("none".to_string(), |value| value.to_string()),
            )));
        }
        if page["entry_offset"].as_u64() != Some(offset) {
            return Err(Merge::Fault(format!(
                "the register's entry pages are not contiguous: offset {offset} came back as {}",
                crate::ps::json_compact(&page["entry_offset"])
            )));
        }
        let rows = crate::ps::pipe(&page["entries"]);
        terms.extend(published_terms(&rows));
        offset += rows.len() as u64;
    }
    if total != Some(offset) {
        return Err(Merge::Fault(format!(
            "the register names {} entries but its pages hold {offset}; the read is not the whole catalog",
            total.map_or("none".to_string(), |value| value.to_string())
        )));
    }
    Ok((first.clone(), terms))
}

/// The published entries as term → target addresses, in register order.
fn published_terms(entries: &[&Value]) -> Vec<(String, Vec<String>)> {
    let mut terms = Vec::new();
    for entry in entries {
        let term = crate::ps::s(&entry["term"]);
        let targets = crate::ps::pipe(&entry["targets"])
            .into_iter()
            .filter(|target| target["exists"] == json!(true))
            .map(|target| crate::ps::s(&target["address"]))
            .collect::<Vec<_>>();
        if !term.is_empty() && !targets.is_empty() {
            terms.push((term, targets));
        }
    }
    terms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ps;

    fn title(address: &str, tree: &str, kind: &str, facet: Option<&str>, title: &str) -> Title {
        Title {
            address: address.to_string(),
            tree: tree.to_string(),
            kind: kind.to_string(),
            facet: facet.map(str::to_string),
            title: title.to_string(),
        }
    }

    // zt-idea-grow 3.1.0: one concept review per source book; the register plan reads them all.
    #[test]
    fn several_reviews_combine_their_meanings_and_keep_each_report_id() {
        let mut a = Review { report_id: "r-a".into(), status: "complete".into(), complete: true, findings: 2, ..Review::default() };
        a.accepted.insert("剩余控制权".into(), vec!["5/1".into()]);
        a.uncertain.insert("学习".into(), "两个意思".into());
        let mut b = Review { report_id: "r-b".into(), status: "partial".into(), complete: false, findings: 3, ..Review::default() };
        b.accepted.insert("剩余控制权".into(), vec!["5/1".into(), "6/1".into()]);
        b.distinct.insert("学习".into(), BTreeSet::from(["7/0".to_string()]));
        // One review stays exactly as it was read, so a plan with one review keeps its id.
        let one = merge_reviews(vec![a.clone()]).unwrap();
        assert_eq!(one.summary(), a.summary());
        assert!(merge_reviews(Vec::new()).is_none());
        let both = merge_reviews(vec![a, b]).unwrap();
        assert_eq!(both.accepted["剩余控制权"], vec!["5/1", "6/1"]);
        assert!(both.distinct["学习"].contains("7/0") && both.uncertain.contains_key("学习"));
        assert_eq!(both.findings, 5);
        assert!(!both.complete && both.status == "partial");
        let summary = both.summary();
        assert_eq!(summary["report_id"], json!("r-a;r-b"));
        assert_eq!(summary["reports"].as_array().map(Vec::len), Some(2));
        assert_eq!(summary["reports"][1]["report_id"], json!("r-b"));
        assert_eq!(review_paths(" a.json ; ;b.json"), vec!["a.json", "b.json"]);
    }

    #[test]
    fn selection_prefers_a_statement_then_a_model_then_a_topic_and_keeps_three() {
        let candidates = vec![
            (3u8, "5/0".to_string()),
            (2u8, "6/1".to_string()),
            (1u8, "5/2".to_string()),
            (1u8, "5/1".to_string()),
            (2u8, "7/1".to_string()),
        ];
        let (targets, overflow) = select_targets(&candidates, &BTreeSet::new());
        assert_eq!(targets, vec!["5/1", "5/2", "6/1"]);
        assert_eq!(overflow, 2);
        // A reviewed distinct sense's Card is never a target.
        let veto = BTreeSet::from(["5/1".to_string()]);
        let (targets, _) = select_targets(&candidates, &veto);
        assert!(!targets.contains(&"5/1".to_string()));
    }

    #[test]
    fn a_statement_prefers_its_whole_model_and_a_topic_claim_in_that_order() {
        let titles = vec![
            title("5/1", "5", "idea", None, "学习提高产业生产率"),
            title("5/2", "5", "model", Some("设定"), "学习模型的设定"),
            title("6/1", "6", "model", None, "学习模型"),
            title("7/0", "7", "idea", None, "学习"),
            title("8/1", "8", "hub", Some("topic"), "学习的入口"),
            title("9/1", "9", "idea", None, "术语表：学习"),
        ];
        let rows = vec![RowSource {
            book: "K".into(),
            table: "K/1|a".into(),
            row: 1,
            meaning: "个体积累技能".into(),
            spelling: "学习".into(),
            preferred: true,
        }];
        let mut candidates: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
        for title in &titles {
            if let Some(tier) = title.tier()
                && title.contains("学习")
            {
                candidates
                    .entry("学习".to_string())
                    .or_default()
                    .push((tier, title.address.clone()));
            }
        }
        let draft = build_terms(&rows, &candidates, &BTreeMap::new());
        assert_eq!(draft.terms.len(), 1);
        assert_eq!(draft.terms[0].targets, vec!["5/1", "5/2", "6/1"]);
        assert_eq!(draft.terms[0].overflow, 1);
        assert!(draft.unresolved.is_empty() && draft.faults.is_empty());
    }

    #[test]
    fn a_term_with_no_target_is_unresolved_with_its_source_row() {
        let rows = vec![RowSource {
            book: "K".into(),
            table: "K/1|a".into(),
            row: 3,
            meaning: "没有着落".into(),
            spelling: "无着落".into(),
            preferred: true,
        }];
        let draft = build_terms(&rows, &BTreeMap::new(), &BTreeMap::new());
        assert!(draft.terms.is_empty());
        assert_eq!(draft.unresolved.len(), 1);
        assert_eq!(draft.unresolved[0]["term"], "无着落");
        assert_eq!(draft.unresolved[0]["sources"][0]["term_table"], "K/1|a");
        assert_eq!(draft.unresolved[0]["sources"][0]["row"], 3);
    }

    #[test]
    fn one_spelling_with_two_meanings_is_ambiguous_and_keeps_its_destinations() {
        let rows = vec![
            RowSource {
                book: "A".into(),
                table: "A/1|a".into(),
                row: 1,
                meaning: "个体行为".into(),
                spelling: "学习".into(),
                preferred: true,
            },
            RowSource {
                book: "B".into(),
                table: "B/1|a".into(),
                row: 2,
                meaning: "产业生产率".into(),
                spelling: "学习".into(),
                preferred: true,
            },
        ];
        let mut candidates: BTreeMap<String, Vec<(u8, String)>> = BTreeMap::new();
        candidates.insert("学习".into(), vec![(1, "5/1".into()), (1, "6/1".into())]);
        let draft = build_terms(&rows, &candidates, &BTreeMap::new());
        assert!(draft.terms[0].ambiguous);
        assert_eq!(draft.terms[0].targets.len(), 2);
        assert_eq!(draft.faults[0]["fault"], "ambiguous");
        assert_eq!(draft.faults[0]["sources"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn the_body_keeps_field_navigation_and_replaces_only_the_term_group() {
        let current = "范围：旧的范围。\n\n## 领域\n- 信息经济学：[[196/1|b]]\n\n## 关键词\n- 旧的词：[[196/1|b]]\n";
        let terms = vec![TermDraft {
            term: "多重均衡".into(),
            targets: vec!["161/1".into(), "stiglitz2014learning/109|a".into()],
            sources: Vec::new(),
            ambiguous: false,
            overflow: 0,
            reviewed: Vec::new(),
            read_only: Vec::new(),
        }];
        let body = register_body(current, &terms);
        assert_eq!(
            body,
            "范围：旧的范围。\n\n## 领域\n- 信息经济学：[[196/1|b]]\n\n## 词条\n- 多重均衡：[[161/1]]、[[stiglitz2014learning/109|a]]"
        );
        // The same body twice: the second publication writes nothing.
        assert_eq!(register_body(&body, &terms), body);
    }

    #[test]
    fn a_report_id_is_the_reviewed_shape() {
        assert!(is_report_id(&format!("r-{}", "a".repeat(64))));
        assert!(!is_report_id("r-1"));
        assert!(!is_report_id(&format!("e-{}", "a".repeat(64))));
        assert!(!is_report_id(&format!("r-{}", "A".repeat(64))));
    }

    // --- readings and parts (S11, #104) ---

    fn draft(term: &str) -> TermDraft {
        TermDraft {
            term: term.to_string(),
            targets: vec!["5/1".to_string()],
            sources: Vec::new(),
            ambiguous: false,
            overflow: 0,
            reviewed: Vec::new(),
            read_only: Vec::new(),
        }
    }

    fn reading(reading: &str, initial: Option<char>) -> Reading {
        Reading {
            reading: reading.to_string(),
            initial,
        }
    }

    fn readings_file(content: &str) -> String {
        let directory = std::env::temp_dir().join(format!(
            "zt-register-readings-{}-{}",
            std::process::id(),
            crate::canon::sha256_hex(content.as_bytes())
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let file = directory.join("register-readings.json");
        std::fs::write(&file, content).unwrap();
        file.to_string_lossy().to_string()
    }

    #[test]
    fn readings_are_validated_and_an_uncertain_or_malformed_one_stays_in_other() {
        let terms = vec![draft("信用"), draft("学习"), draft("投资"), draft("重"), draft("长")];
        // A polyphonic term is accepted with the reading the drafting job chose; a second one is
        // uncertain, a digit initial goes to the digit bucket, and a Chinese "reading" is a shape
        // fault: this step never transliterates (D183, D184, AC02-A03).
        let file = readings_file(
            "{\"schema_version\":1,\"readings\":{\
             \"信用\":{\"reading\":\"xìn yòng\",\"initial\":\"X\"},\
             \"学习\":{\"reading\":\"xué xí\",\"initial\":null,\"uncertain\":true},\
             \"投资\":{\"reading\":\"tóu zī\",\"initial\":\"tt\"},\
             \"重\":{\"reading\":\"教学\",\"initial\":\"z\"},\
             \"长\":{\"reading\":\"cháng\",\"initial\":\"7\"}}}",
        );
        let (accepted, unresolved, faults) = read_readings(&file, &terms).unwrap();
        assert_eq!(accepted.len(), 2, "{accepted:?} unresolved={unresolved:?} faults={faults:?}");
        assert_eq!(accepted["信用"].initial, Some('x'), "the initial is lower-cased");
        assert_eq!(accepted["长"].initial, Some('7'), "a digit initial is its own bucket");
        assert_eq!(accepted["信用"].reading, "xìn yòng", "the accepted reading is kept");
        let reasons = unresolved
            .iter()
            .map(|row| (crate::ps::s(&row["term"]), crate::ps::s(&row["reason"])))
            .collect::<Vec<_>>();
        assert!(
            reasons.contains(&("学习".to_string(), "uncertain".to_string())),
            "an uncertain reading stays unresolved: {reasons:?}"
        );
        assert!(
            reasons.contains(&("投资".to_string(), "missing".to_string())),
            "a malformed initial stays unresolved: {reasons:?}"
        );
        assert!(
            reasons.contains(&("重".to_string(), "missing".to_string())),
            "a reading that is not Latin is unresolved: {reasons:?}"
        );
        let kinds = faults
            .iter()
            .map(|fault| (crate::ps::s(&fault["fault"]), crate::ps::s(&fault["term"])))
            .collect::<Vec<_>>();
        assert!(
            kinds.contains(&("reading_initial".to_string(), "投资".to_string())),
            "{kinds:?}"
        );
        assert!(
            kinds.contains(&("reading_shape".to_string(), "重".to_string())),
            "{kinds:?}"
        );
        // A term the file never mentions is missing its reading, not silently classified.
        let (accepted, unresolved, faults) =
            read_readings(&crate::ps::join(&file, "absent.json"), &[draft("信用")]).unwrap();
        assert!(accepted.is_empty() && faults.is_empty());
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0]["reason"], json!("missing"));
    }

    #[test]
    fn buckets_group_the_accepted_initials_and_a_long_bucket_splits_in_order() {
        let terms = vec![
            draft("信用"),
            draft("学习"),
            draft("投资"),
            draft("储蓄"),
            draft("需求"),
            draft("数字"),
        ];
        let mut readings = BTreeMap::new();
        readings.insert("信用".to_string(), reading("xìn yòng", Some('x')));
        readings.insert("学习".to_string(), reading("xué xí", Some('x')));
        readings.insert("投资".to_string(), reading("tóu zī", Some('t')));
        readings.insert("储蓄".to_string(), reading("chǔ xù", Some('c')));
        readings.insert("需求".to_string(), reading("xū qiú", Some('x')));
        readings.insert("数字".to_string(), reading("9", Some('9')));
        let parts = bucket_parts(&terms, &readings);
        let labels = parts.iter().map(|part| part.label.as_str()).collect::<Vec<_>>();
        assert_eq!(
            labels,
            vec!["A–F", "S–Z", "0–9"],
            "the accepted bands come first, then the digit bucket"
        );
        // Inside a band the accepted reading orders the terms, so a tone mark never reorders them.
        let s = parts.iter().find(|part| part.label == "S–Z").unwrap();
        let names = s.terms.iter().map(|at| terms[*at].term.as_str()).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["投资", "信用", "需求", "学习"],
            "tou, xin, xu, xue: the base letters order the band"
        );
        assert_eq!(parts[1].bucket, "latin");
        assert_eq!(parts[2].bucket, "digit");
        // A bucket over the row limit splits into contiguous parts that keep the accepted range.
        let long: Vec<TermDraft> = (0..PART_SPLIT + 5)
            .map(|index| draft(&format!("词{index:03}")))
            .collect();
        let mut readings = BTreeMap::new();
        for (index, term) in long.iter().enumerate() {
            readings.insert(
                term.term.clone(),
                reading(&format!("sheng{index}"), Some(if index < PART_SPLIT { 's' } else { 'w' })),
            );
        }
        let parts = bucket_parts(&long, &readings);
        let s_parts = parts.iter().filter(|part| part.bucket == "latin").collect::<Vec<_>>();
        assert_eq!(s_parts.len(), 2, "S–Z and W–Z? no: the second band is S–Z(2)");
        assert_eq!(s_parts[0].label, "S–Z");
        assert_eq!(s_parts[0].terms.len(), PART_SPLIT);
        assert_eq!(s_parts[1].label, "S–Z(2)");
        assert_eq!(s_parts[1].terms.len(), 5);
        // A term with no accepted reading is in 其他, which comes last.
        let mut readings = BTreeMap::new();
        readings.insert("学习".to_string(), reading("xué xí", Some('x')));
        let parts = bucket_parts(&[draft("学习"), draft("无读")], &readings);
        assert_eq!(parts.last().unwrap().label, OTHER_LABEL);
        assert_eq!(parts.last().unwrap().bucket, "other");
    }

    #[test]
    fn a_part_body_uses_the_register_row_grammar_and_a_part_title_names_the_range() {
        let terms = vec![draft("信用"), draft("学习")];
        let part = Part {
            label: "X".to_string(),
            bucket: "latin",
            terms: vec![0, 1],
        };
        assert_eq!(
            part_body(&terms, &part),
            "## 词条\n- 信用：[[5/1]]\n- 学习：[[5/1]]"
        );
        assert_eq!(part_title("A–F"), "词条 A–F");
        let empty = Part {
            label: OTHER_LABEL.to_string(),
            bucket: "other",
            terms: Vec::new(),
        };
        assert_eq!(part_body(&terms, &empty), "## 词条", "an empty part is still a navigation entry");
        // The reading order is stable across platforms: base letters, then the term.
        assert_eq!(reading_key("Xué Xí"), reading_key("xue xi"));
        assert_eq!(reading_key("xìn"), "xin");
    }

    #[test]
    fn the_root_declares_its_parts_and_keeps_its_other_groups() {
        let terms = vec![draft("信用")];
        let root = root_body_text(
            "范围：代理工作区的入口。\n\n## 领域\n- 市场：[[9/0]]",
            &terms,
            &[("A–F".to_string(), "{{part:0}}".to_string())],
        );
        assert!(root.contains("## 领域"), "{root}");
        assert!(root.contains("## 词条分册"), "{root}");
        assert!(root.contains("- [[{{part:0}}]] A–F"), "{root}");
        assert!(!root.contains("## 词条\n"), "the rows moved into the parts: {root}");
        assert_eq!(
            fill_parts(&root, &["3/1".to_string()]).unwrap(),
            "范围：代理工作区的入口。\n\n## 领域\n- 市场：[[9/0]]\n\n## 词条分册\n\n- [[3/1]] A–F"
        );
        // Without parts the rows stay on the root and there is no declaration section.
        let small = root_body_text("范围：x", &terms, &[]);
        assert!(small.contains("## 词条"), "{small}");
        assert!(!small.contains("## 词条分册"), "{small}");
        // A reference with no address is refused, never written.
        assert!(fill_parts("- [[{{part:1}}]] B", &["3/1".to_string()]).is_err());
    }

    fn declared_part(label: &str, address: &str, body: &str) -> Value {
        json!({
            "label": label,
            "address": address,
            "body": body,
            "card": {
                "address": address,
                "title": part_title(label),
                "body": body,
                "type": "idea",
            },
        })
    }

    fn current_register(address: Value, body: &str, parts: Vec<Value>) -> Value {
        json!({
            "address": address,
            "body": body,
            "missing": false,
            "parts": parts,
        })
    }

    #[test]
    fn a_new_split_register_is_created_as_a_shell_and_its_parts_filed_under_it() {
        let terms = vec![draft("信用"), draft("学习")];
        let mut readings = BTreeMap::new();
        readings.insert("学习".to_string(), reading("xué xí", Some('x')));
        readings.insert("信用".to_string(), reading("xìn yòng", Some('x')));
        let parts = bucket_parts(&terms, &readings);
        let current = json!({"address": Value::Null, "body": "", "missing": true, "parts": []});
        let staged = stage_register(&current, &terms, &parts, &readings).unwrap();
        assert_eq!(staged.calls.len(), 3, "the Topic, the root shell, and the one part: {:?}", staged.calls);
        assert_eq!(staged.calls[0]["tool"], json!("create_topic"));
        assert_eq!(staged.calls[1]["tool"], json!("create_successor"));
        assert_eq!(staged.calls[1]["arguments"]["type"], json!("hub"));
        assert_eq!(staged.calls[1]["arguments"]["body"], json!(DEFAULT_SCOPE));
        assert_eq!(staged.calls[2]["tool"], json!("create_successor"));
        assert_eq!(staged.calls[2]["arguments"]["at"], json!("@@1"), "a part is filed under the new root");
        assert_eq!(staged.calls[2]["arguments"]["kind"], json!("side"));
        assert_eq!(staged.parts.len(), 1);
        assert_eq!(staged.parts[0].label, "S–Z");
        assert!(staged.parts[0].address.is_none() && !staged.parts[0].reused);
        assert!(staged.root_body.contains("{{part:0}}"), "{}", staged.root_body);
        assert!(staged.root_call.is_object(), "the root is published last");
        assert!(!staged.terms_on_root);
    }

    // The 2026-10-24 idea-grow pilot: the hub compiler had just written the register root (its 领域 and 关键词), and
    // the first register plan needed parts. Its root_before stayed empty, so apply wrote both parts, read the
    // root back, called it "changed while its parts were published" and stopped before the root edit, leaving
    // two undeclared parts that every later plan refused as an obstructed layout.
    #[test]
    fn an_existing_register_that_gains_parts_stages_its_current_root_as_root_before() {
        let terms = vec![draft("信用"), draft("学习")];
        let mut readings = BTreeMap::new();
        readings.insert("学习".to_string(), reading("xué xí", Some('x')));
        readings.insert("信用".to_string(), reading("xìn yòng", Some('x')));
        let parts = bucket_parts(&terms, &readings);
        let root = "范围：代理工作区的全部入口。

## 领域
- 市场：[[8/1|a]]

## 关键词
- 信用：[[8/1|a]]";
        let current = current_register(json!("8/1"), root, vec![]);
        let staged = stage_register(&current, &terms, &parts, &readings).unwrap();
        assert_eq!(staged.parts.len(), 1);
        assert!(staged.root_call.is_object(), "the root is published last");
        assert_eq!(staged.root_before, root, "apply compares the root it reads with the root the plan read");
    }

    #[test]
    fn a_rebuild_reuses_part_addresses_and_keeps_an_obsolete_part_declared_and_empty() {
        let terms = vec![draft("信用"), draft("学习")];
        let mut readings = BTreeMap::new();
        readings.insert("学习".to_string(), reading("xué xí", Some('x')));
        readings.insert("信用".to_string(), reading("xìn yòng", Some('x')));
        let parts = bucket_parts(&terms, &readings);
        let root = "范围：代理工作区的入口。\n\n## 词条分册\n\n- [[3/1]] S–Z\n- [[3/2]] 其他\n";
        let current = current_register(
            json!("2/1"),
            root,
            vec![
                declared_part("S–Z", "3/1", "## 词条\n- 学习：[[5/1]]"),
                declared_part("其他", "3/2", "## 词条\n- 旧词：[[5/1]]"),
            ],
        );
        let staged = stage_register(&current, &terms, &parts, &readings).unwrap();
        assert_eq!(staged.parts.len(), 1);
        assert_eq!(staged.parts[0].address.as_deref(), Some("3/1"), "the label keeps its address");
        assert!(staged.parts[0].reused);
        // The row moved into the part: the reused part's body is edited in place.
        assert_eq!(staged.calls.len(), 2, "{:?}", staged.calls);
        assert_eq!(staged.calls[0]["tool"], json!("edit_card"));
        assert_eq!(staged.calls[0]["arguments"]["at"], json!("3/1"));
        assert_eq!(
            staged.calls[0]["arguments"]["body"],
            json!("## 词条\n- 信用：[[5/1]]\n- 学习：[[5/1]]")
        );
        // The part no longer needed keeps its declaration and is emptied, never renumbered.
        assert_eq!(staged.obsolete.len(), 1);
        assert_eq!(staged.obsolete[0].address, "3/2");
        assert_eq!(staged.obsolete[0].body, "## 词条");
        assert_eq!(staged.calls[1]["arguments"]["at"], json!("3/2"));
        assert!(staged.root_body.contains("- [[3/1]] S–Z"), "{}", staged.root_body);
        assert!(staged.root_body.contains("- [[3/2]] 其他"), "{}", staged.root_body);
        // A part whose label changed keeps its address by position.
        let root = "范围：代理工作区的入口。\n\n## 词条分册\n\n- [[3/1]] G–L\n";
        let current = current_register(
            json!("2/1"),
            root,
            vec![declared_part("G–L", "3/1", "## 词条\n- 信用：[[5/1]]\n- 学习：[[5/1]]")],
        );
        let staged = stage_register(&current, &terms, &parts, &readings).unwrap();
        assert_eq!(staged.parts[0].address.as_deref(), Some("3/1"));
        let call = staged
            .calls
            .iter()
            .find(|call| call["tool"] == json!("edit_card"))
            .expect("the reused part is relabelled in place");
        assert_eq!(call["arguments"]["title"], json!("词条 S–Z"), "the title follows the label");
    }

    #[test]
    fn an_unchanged_split_register_stages_no_write_at_all() {
        let terms = vec![draft("信用")];
        let mut readings = BTreeMap::new();
        readings.insert("信用".to_string(), reading("xìn yòng", Some('x')));
        let parts = bucket_parts(&terms, &readings);
        let root = "范围：代理工作区的入口。\n\n## 词条分册\n\n- [[3/1]] S–Z";
        let body = "## 词条\n- 信用：[[5/1]]";
        let current = current_register(json!("2/1"), root, vec![declared_part("S–Z", "3/1", body)]);
        let staged = stage_register(&current, &terms, &parts, &readings).unwrap();
        assert!(staged.calls.is_empty(), "{:?}", staged.calls);
        assert!(staged.root_call.is_null(), "the root body is unchanged");
        assert_eq!(staged.root_body, root);
        // A new part beyond the declared ones is a genuinely new Side successor.
        let terms = vec![draft("信用"), draft("储蓄")];
        let mut readings = BTreeMap::new();
        readings.insert("信用".to_string(), reading("xìn yòng", Some('x')));
        readings.insert("储蓄".to_string(), reading("chǔ xù", Some('c')));
        let parts = bucket_parts(&terms, &readings);
        let current = current_register(json!("2/1"), root, vec![declared_part("S–Z", "3/1", body)]);
        let staged = stage_register(&current, &terms, &parts, &readings).unwrap();
        assert_eq!(staged.parts.len(), 2);
        assert_eq!(
            staged.parts[0].label, "A–F",
            "the band the register never declared is genuinely new"
        );
        assert!(staged.parts[0].address.is_none() && !staged.parts[0].reused);
        assert_eq!(
            staged.parts[1].address.as_deref(),
            Some("3/1"),
            "the declared S–Z part keeps its address by label"
        );
        assert!(staged.parts[1].reused);
        let created = staged
            .calls
            .iter()
            .find(|call| call["tool"] == json!("create_successor"))
            .expect("a genuinely new part is created");
        assert_eq!(created["arguments"]["at"], json!("2/1"));
        assert_eq!(staged.root_call["arguments"]["body"], json!(staged.root_body));
    }

    #[test]
    fn a_part_fault_obstructs_the_layout_the_step_would_rewrite() {
        let view = json!({"faults": [
            {"fault": "part-place", "detail": "[[3/1]] is declared under 词条分册 but is not an immediate Side successor"},
        ]});
        let refusal = obstructed_parts(&view, false).unwrap_err().to_string();
        assert!(refusal.contains("part-place") && refusal.contains("3/1"), "{refusal}");
        // A row fault that a rebuild repairs does not obstruct the layout.
        let view = json!({"faults": [{"fault": "broken", "detail": "[[9/9]] does not exist"}]});
        assert!(obstructed_parts(&view, false).is_ok());
    }

    #[test]
    fn the_published_layout_must_be_the_accepted_one() {
        let plan = json!({
            "parts": [
                {"label": "A–F", "address": null, "terms": ["甲", "乙"], "reused": false},
                {"label": "其他", "address": "3/2", "terms": ["丙"], "reused": true},
            ],
            "obsolete_parts": [{"label": "S–Z", "address": "3/3", "body": "## 词条"}],
        });
        let view = json!({"parts": [
            {"label": "A–F", "address": "3/1", "place": "ok", "entries": 2},
            {"label": "其他", "address": "3/2", "place": "ok", "entries": 1},
            {"label": "S–Z", "address": "3/3", "place": "ok", "entries": 0},
        ]});
        let verified =
            verify_published_layout(&plan, &view, &["3/1".to_string(), "3/2".to_string()]).unwrap();
        assert_eq!(verified.len(), 2);
        assert_eq!(verified[0]["reused"], json!(false));
        assert_eq!(verified[1]["reused"], json!(true));
        // A part whose address moved under the label is not the accepted layout.
        let moved = json!({"parts": [
            {"label": "A–F", "address": "3/9", "place": "ok", "entries": 2},
            {"label": "其他", "address": "3/2", "place": "ok", "entries": 1},
            {"label": "S–Z", "address": "3/3", "place": "ok", "entries": 0},
        ]});
        let refusal = verify_published_layout(&plan, &moved, &["3/1".to_string(), "3/2".to_string()])
            .unwrap_err()
            .to_string();
        assert!(refusal.contains("3/9"), "{refusal}");
        // An obsolete part that still holds rows is not the empty navigation entry the plan kept.
        let full = json!({"parts": [
            {"label": "A–F", "address": "3/1", "place": "ok", "entries": 2},
            {"label": "其他", "address": "3/2", "place": "ok", "entries": 1},
            {"label": "S–Z", "address": "3/3", "place": "ok", "entries": 4},
        ]});
        assert!(
            verify_published_layout(&plan, &full, &["3/1".to_string(), "3/2".to_string()]).is_err()
        );
        // A plan without parts accepts a register without a declaration and nothing else.
        let small = json!({"parts": [], "obsolete_parts": []});
        assert!(verify_published_layout(&small, &json!({"parts": []}), &[]).unwrap().is_empty());
        assert!(verify_published_layout(&small, &view, &[]).is_err());
    }

    #[test]
    fn a_changed_reading_is_a_new_plan_input() {
        let terms = vec![draft("信用")];
        let mut payload = json!({
            "schema_version": SCHEMA_VERSION,
            "archive": {"identity": {"id": "a"}},
            "terms": terms.iter().map(TermDraft::json).collect::<Vec<_>>(),
            "unresolved": [],
            "faults": [],
            "excluded": [],
            "review": Value::Null,
            "body": "范围：x",
            "parts": [],
            "obsolete_parts": [],
            "readings": {"accepted": [{"term": "信用", "reading": "xìn yòng", "initial": "x"}]},
        });
        let first = plan_id(&payload);
        assert!(first.starts_with("rp-"));
        assert_eq!(plan_id(&payload), first, "the same acceptance has the same id");
        payload["readings"]["accepted"][0]["reading"] = json!("xìn róng");
        assert_ne!(plan_id(&payload), first, "a changed reading is a new plan");
    }

    fn term_row(term: &str, avoid: &[&str], meaning: &str) -> zt_check::check::TermRow {
        zt_check::check::TermRow {
            term: term.to_string(),
            avoid: avoid.iter().map(|word| word.to_string()).collect(),
            meaning: meaning.to_string(),
            defined: "§1".to_string(),
        }
    }

    fn term_tables(
        rows: Vec<zt_check::check::TermRow>,
    ) -> Vec<(String, String, Vec<zt_check::check::TermRow>)> {
        vec![("K/1|a".to_string(), "术语表：产权".to_string(), rows)]
    }

    #[test]
    fn a_reviewed_human_entry_is_a_read_only_target_and_a_missing_one_is_excluded() {
        let titles = vec![title("5/1", "5", "idea", None, "别的卡")];
        let human = vec![title("0/1", "0", "idea", None, "所有权的归属")];
        let tables = term_tables(vec![term_row("产权", &[], "谁拥有企业")]);
        let mut review = Review::default();
        review
            .accepted
            .insert("产权".into(), vec!["0/1".into(), "9/9".into()]);
        let draft = draft_terms(&titles, &human, &tables, Some(review));
        assert_eq!(draft.terms.len(), 1);
        assert_eq!(draft.terms[0].targets, vec!["0/1"]);
        assert_eq!(
            draft.terms[0].read_only,
            vec!["0/1"],
            "the reviewed human Card is named as a read-only target"
        );
        assert_eq!(draft.excluded.len(), 1);
        assert_eq!(draft.excluded[0]["address"], "9/9");
        assert_eq!(draft.excluded[0]["reason"], "reviewed_entry_missing");
        assert!(draft.unresolved.is_empty());
    }

    #[test]
    fn a_human_card_is_never_a_target_without_the_review() {
        let titles = vec![title("5/1", "5", "idea", None, "别的卡")];
        let human = vec![title("0/1", "0", "idea", None, "产权")];
        let tables = term_tables(vec![term_row("产权", &[], "谁拥有企业")]);
        let draft = draft_terms(&titles, &human, &tables, None);
        assert!(draft.terms.is_empty(), "{:?}", draft.terms);
        assert_eq!(draft.unresolved.len(), 1);
        assert_eq!(draft.unresolved[0]["term"], "产权");
        assert_eq!(draft.unresolved[0]["reason"], "no_defensible_target");
    }

    fn page(
        offset: u64,
        total: u64,
        revision: u64,
        register: &str,
        entries: Vec<Value>,
    ) -> Value {
        json!({
            "register": register,
            "entries": entries,
            "entry_offset": offset,
            "entry_limit": 1000,
            "entry_total": total,
            "entry_next_offset": Value::Null,
            "archive_revision": revision,
        })
    }

    fn entry(term: &str, targets: &[&str]) -> Value {
        json!({
            "term": term,
            "targets": targets
                .iter()
                .map(|address| json!({ "address": address, "exists": true }))
                .collect::<Vec<_>>(),
        })
    }

    #[test]
    fn a_register_is_read_as_one_catalog_across_pages() {
        let pages = vec![
            page(
                0,
                3,
                7,
                "196/1",
                vec![entry("甲", &["5/1"]), entry("乙", &["5/2"])],
            ),
            page(2, 3, 7, "196/1", vec![entry("丙", &["5/3"])]),
        ];
        let merged = merged_terms(&pages).ok().expect("one catalog");
        assert_eq!(merged.0["entry_total"], 3);
        assert_eq!(
            merged.1,
            vec![
                ("甲".to_string(), vec!["5/1".to_string()]),
                ("乙".to_string(), vec!["5/2".to_string()]),
                ("丙".to_string(), vec!["5/3".to_string()]),
            ]
        );
    }

    #[test]
    fn a_read_that_crosses_a_revision_is_a_changing_view() {
        let pages = vec![
            page(
                0,
                3,
                7,
                "196/1",
                vec![entry("甲", &["5/1"]), entry("乙", &["5/2"])],
            ),
            page(2, 3, 8, "196/1", vec![entry("丙", &["5/3"])]),
        ];
        assert!(matches!(merged_terms(&pages), Err(Merge::Changing(_))));
    }

    #[test]
    fn pages_that_do_not_add_up_are_a_fault() {
        let short = vec![page(
            0,
            3,
            7,
            "196/1",
            vec![entry("甲", &["5/1"]), entry("乙", &["5/2"])],
        )];
        assert!(matches!(merged_terms(&short), Err(Merge::Fault(_))));
        let gap = vec![
            page(
                0,
                3,
                7,
                "196/1",
                vec![entry("甲", &["5/1"]), entry("乙", &["5/2"])],
            ),
            page(3, 3, 7, "196/1", vec![entry("丙", &["5/3"])]),
        ];
        assert!(matches!(merged_terms(&gap), Err(Merge::Fault(_))));
    }
    /// S27 (#119): the review gate re-stages a plan from the rows a reviewer accepted, so a
    /// declined new term is simply not written and a part it no longer fills keeps its address.
    #[test]
    fn a_restaged_publication_holds_only_the_accepted_rows() {
        let plan = json!({
            "schema_version": 1,
            "archive": {"identity": {"id": "a".repeat(32)}, "revision": 3},
            "unresolved": [],
            "faults": [],
            "excluded": [],
            "review": Value::Null,
            "readings": {"accepted": [{"term": "学习", "reading": "xué xí", "initial": "x"}], "unresolved": [], "count": 1},
            "terms": [
                {"term": "学习", "targets": ["3/1"], "sources": [], "ambiguous": false, "overflow": 0, "reviewed": [], "read_only_targets": []},
                {"term": "信用", "targets": ["3/2"], "sources": [], "ambiguous": false, "overflow": 0, "reviewed": [], "read_only_targets": []},
            ],
        });
        let accepted = vec![json!({"term": "学习", "targets": ["3/1"], "sources": [], "ambiguous": false, "overflow": 0, "reviewed": [], "read_only_targets": []})];
        let current = current_register(json!("2/1"), "范围：入口。\n\n## 词条\n- 旧词：[[3/9]]", vec![]);
        let restaged = restage(&plan, &accepted, &current).unwrap();
        assert!(ps::s(&restaged["body"]).contains("- 学习：[[3/1]]"));
        assert!(!ps::s(&restaged["body"]).contains("信用"), "a declined row is not written: {}", ps::s(&restaged["body"]));
        assert!(!ps::s(&restaged["body"]).contains("旧词"), "the current rows are rebuilt: {}", ps::s(&restaged["body"]));
        assert_eq!(restaged["terms"].as_array().map(Vec::len), Some(1));
        assert!(ps::s(&restaged["plan_id"]).starts_with("rp-"));
        assert_eq!(ps::s(&restaged["restaged_from"]), "");
        // The same acceptance re-stages the same publication: the id is the content, not the attempt.
        let again = restage(&plan, &accepted, &current).unwrap();
        assert_eq!(ps::s(&restaged["plan_id"]), ps::s(&again["plan_id"]));
        // A different accepted set is a different publication.
        let both = vec![
            json!({"term": "学习", "targets": ["3/1"], "sources": [], "ambiguous": false, "overflow": 0, "reviewed": [], "read_only_targets": []}),
            json!({"term": "信用", "targets": ["3/2"], "sources": [], "ambiguous": false, "overflow": 0, "reviewed": [], "read_only_targets": []}),
        ];
        let wider = restage(&plan, &both, &current).unwrap();
        assert_ne!(ps::s(&restaged["plan_id"]), ps::s(&wider["plan_id"]));
        assert!(ps::s(&wider["body"]).contains("信用"));
    }
}

