//! `zt-digest register`: draft and publish a Workspace's concept register (#101, design §4.1,
//! §8.3-8.5, D058-D062, D170-D175, D276, D289, D296, D297, D299).
//!
//! The register is one ordinary Hub Card of level `register` (the first Card of the Topic
//! `索引`). Its `## 词条` section holds one term before a colon and one to three Links to the Cards
//! that carry the term's substance. This step is the only publication path for those rows: the hub
//! compiler's `fields.json` register still lists field and topic hubs, and this step keeps every
//! group it does not own (the `## 领域` field-hub navigation among them).
//!
//!     plan   reads the agent book term tables (through zt-check's term-table parser) and the
//!            model-head names, reads an optional reviewed concept report as read-only evidence,
//!            and stages `<Work>\out\register-plan.json`: one entry per accepted spelling with up
//!            to three substantive targets, the accepted spellings of one term, the unresolved
//!            source rows, the exact register body, and the id of the report it was read from.
//!     apply  publishes the staged body (creating the Topic 索引 and the register Hub Card when
//!            the Workspace has none, otherwise editing the one register), then reads the register
//!            back through `get_register` and verifies every term and target. A second run over
//!            the same plan writes nothing: no new Card, no changed body, the same plan id.
//!     check  compares the staged plan with the published entries and prints the differences.
//!
//! What the step does not do: it does not judge meanings (the concept review does, S04), it does
//! not publish register parts or pinyin grouping (S11), and it does not correct an existing Card
//! other than the one register body it publishes. Discovery writes only the staging file, and a
//! plan staged for another archive or another revision is refused rather than applied.

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
/// The title every book term table carries (R6's own marker).
const TERM_TABLE_PREFIX: &str = "术语表";
/// The default first line of a register this step creates.
const DEFAULT_SCOPE: &str = "范围：代理工作区的入口。每个词条指向一到三个实质卡。";
/// The register term group this step writes (§8.4); `关键词` is the older compiler's heading.
const TERM_GROUPS: [&str; 2] = ["词条", "关键词"];
const DEFAULT_TITLE: &str = "索引：代理工作区的入口";
const INDEX_TITLE: &str = "索引";
const INDEX_BODY: &str = "本主题收放代理工作区的入口：索引卡与领域枢纽。";

struct Params {
    step: String,
    work: String,
    review: String,
}

/// `zt-digest register`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["Step", "Work", "Review"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let p = Params {
        step: a.choice("Step", &["plan", "apply", "check"])?,
        work: a.required("Work")?,
        review: a.str("Review"),
    };
    if p.work.is_empty() {
        bail!("Cannot bind argument to parameter 'Work' because it is an empty string.");
    }
    let work = full_path(&p.work)?;
    let out = crate::ps::join(&work, "out");
    std::fs::create_dir_all(&out).with_context(|| format!("failed to make {out}"))?;
    let mut register = Register {
        out,
        review: p.review,
        zt: None,
    };
    match p.step.as_str() {
        "plan" => register.plan()?,
        "apply" => register.apply()?,
        _ => register.check()?,
    }
    Ok(0)
}

struct Register {
    out: String,
    review: String,
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
}

impl Review {
    fn summary(&self) -> Value {
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
            let tables = self.term_tables(&titles)?;
            let review = self.read_review(&identity)?;
            let titles = parse_titles(&titles);
            let draft_here = draft_terms(&titles, &tables, review);
            if checked == revision {
                draft = Some((identity, revision, draft_here));
                break;
            }
        }
        let Some((identity, revision, draft)) = draft else {
            bail!("the archive changed during every register read; run -Step plan again");
        };
        let current = self.current_register()?;
        let current_body = current["body"].as_str().unwrap_or_default().to_string();
        let body = register_body(&current_body, &draft.terms);
        let plan = plan_value(&identity, revision, &current, &draft, &body);
        canon::save_json(&self.plan_file(), &plan)?;
        util::print_last(&json!({
            "ok": true,
            "step": "plan",
            "plan_id": plan["plan_id"],
            "archive": plan["archive"],
            "register": plan["register"],
            "terms": draft.terms.len(),
            "entries": draft.terms.iter().filter(|term| !term.targets.is_empty()).count(),
            "unresolved": draft.unresolved.len(),
            "faults": draft.faults.len(),
            "excluded": draft.excluded.len(),
            "review": plan["review"],
            "body_lines": body.lines().count(),
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
    /// report id is recorded so the plan says which review it was read from.
    fn read_review(&mut self, identity: &Value) -> Result<Option<Review>> {
        if self.review.is_empty() {
            return Ok(None);
        }
        let path = full_path(&self.review)?;
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

    /// The one register of the agent Workspace, as `get_register` reads it. Two registers are
    /// reported, never chosen between; no register is an empty answer.
    fn current_register(&mut self) -> Result<Value> {
        let view = self.call("get_register", json!({"workspace": "agent"}))?;
        if view["register_missing"] == json!(true) {
            return Ok(json!({"address": Value::Null, "body": "", "missing": true}));
        }
        if view["register"].is_null() {
            bail!(
                "the agent Workspace has more than one register: {}",
                crate::ps::json_compact(&view["duplicate_registers"])
            );
        }
        let address = crate::ps::s(&view["register"]);
        let card = self.call("get_card", json!({"at": address}))?;
        Ok(json!({
            "address": view["register"],
            "body": card["body"],
            "missing": false,
            "entries": view["entry_total"],
            "complete": view["complete"],
        }))
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

/// The plan's own content id: the accepted body, terms, unresolved rows, and review, bound to the
/// archive identity but not to its revision. Re-planning the same acceptance gives the same id,
/// and a publication in between does not change it.
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
    });
    format!("rp-{}", canon::content_hash(&content, &[]))
}

fn plan_value(
    identity: &Value,
    revision: u64,
    current: &Value,
    draft: &Draft,
    body: &str,
) -> Value {
    let mut payload = json!({
        "schema_version": SCHEMA_VERSION,
        "archive": {"identity": identity, "revision": revision},
        "register": {
            "address": current["address"],
            "missing": current["missing"],
            "entries": current["entries"],
            "complete": current["complete"],
        },
        "terms": draft.terms.iter().map(TermDraft::json).collect::<Vec<_>>(),
        "unresolved": draft.unresolved,
        "faults": draft.faults,
        "excluded": draft.excluded,
        "review": draft.review.as_ref().map(Review::summary),
        "body": body,
    });
    let id = plan_id(&payload);
    payload["plan_id"] = json!(id);
    payload["created_at"] = json!(canon::utc_now_o());
    payload
}

/// The accepted spellings of each term and their targets, as the plan stages them.
fn draft_terms(
    titles: &[Title],
    tables: &[(String, String, Vec<zt_check::check::TermRow>)],
    review: Option<Review>,
) -> Draft {
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
    // Model-head names are terms too (D170-D175, §8.3).
    for title in titles.iter().filter(|title| title.is_model_head()) {
        let spelling = title.title.trim().to_string();
        if spelling.is_empty() {
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
                    if titles.iter().any(|title| &title.address == entry) {
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
    // A reviewed entry the agent Workspace does not hold cannot be a target: a human Card needs
    // an explicit selection, and a missing Card is a stale review.
    let mut excluded = Vec::new();
    if let Some(review) = &review {
        for (spelling, entries) in &review.accepted {
            for entry in entries {
                if !titles.iter().any(|title| &title.address == entry) {
                    excluded.push(json!({
                        "term": spelling,
                        "address": entry,
                        "reason": "reviewed_entry_not_in_agent_workspace",
                        "detail": "a reviewed entry outside the agent Workspace is a human or missing Card; it needs an explicit selection",
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
/// `## 词条` row per accepted term. Text after a term's Links never enters a row.
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
        if TERM_GROUPS.contains(&heading.as_str()) {
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
        let planned = planned_terms(&plan)?;
        // Every planned target must still exist: a moved or deleted Card makes the staged
        // addresses stale, and the write path would otherwise refuse a new broken Link.
        let titles = self.call("export_titles", json!({"workspace": "agent"}))?;
        let present = crate::ps::pipe(&titles["titles"])
            .into_iter()
            .map(|row| crate::ps::s(&row["address"]))
            .collect::<BTreeSet<_>>();
        let gone = planned
            .iter()
            .flat_map(|(_, targets)| targets.iter())
            .filter(|address| !present.contains(*address))
            .cloned()
            .collect::<Vec<_>>();
        if !gone.is_empty() {
            bail!(
                "the staged plan names Cards that are no longer in the agent Workspace: {}; run -Step plan again",
                gone.join(", ")
            );
        }
        let current = self.current_register()?;
        let mut created = 0;
        let mut changed = false;
        let address = if current["missing"] == json!(true) {
            let topic = self.call(
                "create_topic",
                json!({"title": INDEX_TITLE, "body": INDEX_BODY}),
            )?;
            let root = crate::ps::s(&topic["address"]);
            let made = self.call(
                "create_successor",
                json!({"at": root, "type": "hub", "level": "register", "title": DEFAULT_TITLE, "body": body}),
            )?;
            created = 2;
            crate::ps::s(&made["address"])
        } else {
            let address = crate::ps::s(&current["address"]);
            if current["body"].as_str().unwrap_or_default() == body.as_str() {
                address
            } else {
                self.call("edit_card", json!({"at": address, "body": body}))?;
                changed = true;
                address
            }
        };
        // A fresh read must confirm the exact register, terms, and targets (§8.5).
        let view = self.call("get_register", json!({"workspace": "agent"}))?;
        let published = published_terms(&view)?;
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
                "the register was published at `{address}` but does not verify: diverged={} extra={} complete={} faults={}",
                crate::ps::json_compact(&Value::Array(divergence)),
                crate::ps::json_compact(&Value::Array(extra)),
                view["complete"],
                crate::ps::json_compact(&view["faults"])
            );
        }
        util::print_last(&json!({
            "ok": true,
            "step": "apply",
            "plan_id": plan["plan_id"],
            "register": address,
            "created": created,
            "changed": changed,
            "entries": published.len(),
            "unresolved": plan["unresolved"].as_array().map_or(0, Vec::len),
            "review": plan["review"],
            "complete": view["complete"],
            "archive_revision": view["archive_revision"],
            "verified": true,
        }))
    }

    fn check(&mut self) -> Result<()> {
        let view = self.call("get_register", json!({"workspace": "agent"}))?;
        let published = published_terms(&view)?;
        let mut missing = Vec::new();
        let mut diverged = Vec::new();
        let mut extra = Vec::new();
        let mut plan_id = Value::Null;
        let mut unresolved = 0;
        if crate::ps::exists(&self.plan_file()) {
            let plan = canon::read_json(&self.plan_file())?;
            plan_id = plan["plan_id"].clone();
            unresolved = plan["unresolved"].as_array().map_or(0, Vec::len);
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
        }
        util::print_last(&json!({
            "ok": true,
            "step": "check",
            "plan_id": plan_id,
            "register": view["register"],
            "register_missing": view["register_missing"],
            "entries": published.len(),
            "unresolved": unresolved,
            "complete": view["complete"],
            "faults": view["faults"],
            "missing": missing,
            "diverged": diverged,
            "extra": extra,
            "archive_revision": view["archive_revision"],
        }))
    }
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

/// The published entries as term → target addresses, in register order.
fn published_terms(view: &Value) -> Result<Vec<(String, Vec<String>)>> {
    let mut terms = Vec::new();
    for entry in crate::ps::pipe(&view["entries"]) {
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
    Ok(terms)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(address: &str, tree: &str, kind: &str, facet: Option<&str>, title: &str) -> Title {
        Title {
            address: address.to_string(),
            tree: tree.to_string(),
            kind: kind.to_string(),
            facet: facet.map(str::to_string),
            title: title.to_string(),
        }
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
}
