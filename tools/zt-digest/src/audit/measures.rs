//! The reproducible Luhmann baseline of `audit` (#94 S01, design §4.3 and §14.3).
//!
//! Three things live here, all derived from one read, and none of them writes a Card:
//!
//! 1. **Evidence** (D353, S01-AC01): archive identity and revision, software and rule versions,
//!    thresholds, the exact scope, the sample seed with the addresses it drew, the inputs this
//!    run read, and every piece of evidence that is missing. A measure whose evidence is
//!    missing is `known: false` with its reason; it is never a zero.
//! 2. **Source evidence** (D068–D074, S01-AC02): the explicit source set of every Card — the
//!    Citation keys its `来源` lines name, else its own Literature tree, plus a model head's
//!    evidence for a part that has none (`derived_from`) — kept apart from the existing
//!    inferred attribution so the report can show where the two differ. Unknown and multiple
//!    sources stay unknown and multiple. A semantic Link alone is no source evidence.
//! 3. **The measures** P1–P15 (D354–D366, S01-AC03–AC06) with their denominators, the fixed
//!    manual task sheet, and the comparison with an earlier baseline that refuses to compare
//!    one scope's denominator with another's as though they matched.
//!
//! The existing inferred attribution (`book_of`) is untouched: it is reported beside this
//! evidence set, never replaced by it (D067).

use super::book_of::{self, BookIndex};
use super::Params;
use crate::report::number_json;
use crate::ps::{self, field, is_in, ps_items, s};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The report's own schema, so a reader can tell one generation of the bytes from another.
pub const SCHEMA_VERSION: i64 = 1;

/// P8's reach: at most two authored semantic Links from a register entry target, outbound
/// (D365). Navigation through a register part is not one of them.
pub const REGISTER_REACH_LINKS: usize = 2;

/// The near-title threshold, taken from the checker so a report never introduces a second
/// title rule (I6, D361).
pub const NEAR_TITLE: f64 = zt_check::check::TITLE_SIMILARITY;

/// The fixed manual task sheet's schema and task kinds (S01-AC05). A sheet is one hand-written
/// JSON file with these fields and nothing else: no telemetry store and no production quota.
pub const SHEET_SCHEMA: i64 = 1;
pub const TASK_KINDS: [&str; 3] = ["first-useful-card", "kept-spread", "distant-card"];

/// The accepted proposal's measured baseline, with the scope it was measured on (D366,
/// S01-AC06). These numbers are never compared with this report's as though the scopes
/// matched: the proposal read a hand sample of the whole live agent Workspace, and a report
/// counts the Cards of its own scope under its own thresholds.
pub const PROPOSAL_BASELINE: &str = "docs/luhmann-comparison-proposals.md";

/// The live agent Workspace the proposal measured, and what it counted.
fn proposal_baseline() -> Value {
    json!({
        "source": PROPOSAL_BASELINE,
        "measured": "2026-10-18",
        "method": "a hand-read sample of 200 Topic Cards, plus list_trees/get_tree/export_titles counts",
        "scope": {
            "kind": "workspace",
            "workspace": "agent",
            "cards": 10887,
            "topic_cards": 3546,
            "topics": 208,
            "books": 6,
            "literature_cards": 7341,
            "sample": 200
        },
        "values": {
            "P1": { "measure": "sampled Topic Cards that rest on a Card of their own book's Literature tree", "value": "150 of 200 (75%)" },
            "P2": { "measure": "one statement written again in several Cards", "value": "the Slutsky equation in 7 Cards" },
            "P4": { "measure": "sampled Cards ending with an appended 关联： paragraph", "value": "122 of 200 (61%)" },
            "P7": { "measure": "sampled Cards citing the Literature trees of more than one book", "value": "6 of 200 (3%)" },
            "P8": { "measure": "keywords in register 196/1, and hub unreached Cards", "value": "78 keywords; 7,488 unreached" },
            "P11": { "measure": "hubs whose title copies the Card they are filed under", "value": "33" },
            "P14": { "measure": "human Cards, and human Cards that link an agent Card", "value": "8 human Cards; 0 human-to-agent Links" }
        }
    })
}

// --- source evidence (D068–D074) ------------------------------------------------------------------

/// The explicit source evidence of one Card (D068–D072). `direct` is what the Card itself
/// carries; `derived` is a model head's evidence a Model part borrows because it has none of
/// its own, and `derived_from` names the head so the derivation stays visible.
#[derive(Clone, Default)]
pub struct SourceSet {
    pub direct: Vec<String>,
    pub derived: Vec<String>,
    pub derived_from: Vec<String>,
}

impl SourceSet {
    /// Every book this Card is evidence for: its own first, then the model head's.
    pub fn all(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for book in self.direct.iter().chain(self.derived.iter()) {
            if !out.iter().any(|seen| ps::eq(seen, book)) {
                out.push(book.clone());
            }
        }
        out
    }

    /// No explicit evidence: `unknown`, which is not a pass (D073).
    pub fn unknown(&self) -> bool {
        self.all().is_empty()
    }

    /// More than one book: the Card is kept as multiple, never forced to one owner (D073).
    pub fn multiple(&self) -> bool {
        self.all().len() > 1
    }

    fn names(&self, book: &str) -> bool {
        self.all().iter().any(|source| ps::eq(source, book))
    }

    fn json(&self) -> Value {
        json!({
            "direct": self.direct,
            "derived": self.derived,
            "derived_from": self.derived_from,
            "unknown": self.unknown(),
            "multiple": self.multiple()
        })
    }
}

/// The source set of every Card: the checker's one direct derivation, plus D071's inheritance
/// for a Model part that has no direct evidence of its own.
fn source_sets(graph: &[Value], models: &[Value]) -> HashMap<String, SourceSet> {
    let mut sets: HashMap<String, SourceSet> = HashMap::new();
    for card in graph {
        sets.insert(
            ps::key(&field(card, "address")),
            SourceSet { direct: zt_check::check::source_evidence(card), derived: Vec::new(), derived_from: Vec::new() },
        );
    }
    for model in models {
        let head = &model["head"];
        let head_at = field(head, "address");
        if head_at.is_empty() {
            continue;
        }
        let head_sources = sets.get(&ps::key(&head_at)).map(|set| set.direct.clone()).unwrap_or_default();
        if head_sources.is_empty() {
            continue;
        }
        for part in ps_items(&model["parts"]) {
            let Some(set) = sets.get_mut(&ps::key(&field(&part, "address"))) else { continue };
            if !set.direct.is_empty() {
                continue;
            }
            set.derived = head_sources.clone();
            if !set.derived_from.contains(&head_at) {
                set.derived_from.push(head_at.clone());
            }
        }
    }
    sets
}

/// Whether the Card's own source evidence names the book a Link target lives in: the one
/// predicate P1 and I7 share (D074).
fn own_source_link(target: &str, set: Option<&SourceSet>, books: &HashSet<String>) -> bool {
    let Some(book) = book_of::link_book(target, books) else { return false };
    set.is_some_and(|set| set.names(book))
}

// --- inputs ---------------------------------------------------------------------------------------

/// One named input file the audit read.
pub struct Input {
    pub path: String,
    pub json: Value,
}

/// A reviewed concept report (#97) and the decision file that resolved some of its findings.
pub struct Concept {
    pub review: Input,
    pub decisions: Option<Input>,
}

/// An earlier `audit.json`, with the address list beside it (the new-Card interval) and the
/// source counts it published when it was produced by this version.
pub struct Baseline {
    pub path: String,
    pub report: Value,
    pub addresses: Option<HashSet<String>>,
    pub sources: Option<Value>,
}

/// Everything the baseline section reads. The audit gathers it; nothing here writes.
pub struct Facts<'a> {
    pub p: &'a Params,
    /// The report's own scope name: `agent`, or the Citation key of one book.
    pub scope: &'a str,
    pub books: &'a [String],
    pub cards: &'a [&'a Value],
    pub graph: &'a [Value],
    pub trees: &'a [Value],
    pub index: &'a BookIndex,
    pub models: &'a [Value],
    pub hubs: &'a [Value],
    pub register: Option<&'a Value>,
    /// The report's own `books` rows and its `workspace` block: the current side of a
    /// comparison with an earlier report.
    pub book_rows: &'a [Value],
    pub workspace_row: &'a Value,
    pub revision: u64,
    pub identity: &'a Value,
    pub faults: &'a [super::Fault],
    pub baseline: Option<&'a Baseline>,
    pub concept: Option<&'a Concept>,
    pub run_report: Option<&'a Input>,
    pub sheet: &'a Value,
    pub zt_version: &'a str,
}

fn n(v: &Value) -> String {
    s(v)
}

/// A count per 100 of its denominator, or `null` when the denominator is unknown or zero.
fn per_100(value: i64, of: i64) -> Value {
    if of <= 0 {
        return Value::Null;
    }
    number_json((value as f64) * 100.0 / (of as f64))
}

/// One measure: whether its evidence is complete, its numbers, and its rate when both the
/// numerator and the denominator are known.
fn measure(name: &str, known: bool, reason: &str, values: Value, rate: Value, note: &str) -> Value {
    let mut map = Map::new();
    map.insert("known".into(), json!(known));
    map.insert("measure".into(), json!(name));
    if !reason.is_empty() {
        map.insert("reason".into(), json!(reason));
    }
    map.insert("values".into(), values);
    if !rate.is_null() {
        map.insert("rate".into(), rate);
    }
    if !note.is_empty() {
        map.insert("note".into(), json!(note));
    }
    Value::Object(map)
}

fn unknown(name: &str, reason: &str) -> Value {
    measure(name, false, reason, json!({}), Value::Null, "")
}

// --- the whole section ----------------------------------------------------------------------------

/// The advisory facts the report publishes (S27, #166, AC06): the checker's own default advisory
/// catalog, plus the advisory subcases whose rule blocks in general. One blocker/advisory decision
/// serves the audit and every gate, so no caller keeps a private rule list.
fn advisory_facts() -> Value {
    json!({
        "rules": zt_check::policy::ADVISORY_RULES,
        "subcases": ["I7"],
    })
}

/// The `baseline` section of `audit.json`: evidence, source sets, measures, the task sheet and
/// the comparison. Read-only and derived: the same read gives the same bytes apart from the
/// report's own timestamp and seconds.
pub fn build(f: &Facts) -> Result<Value> {
    let sets = source_sets(f.graph, f.models);
    // The complete evidence set beside the report: which book each Card is evidence for, and
    // where the existing inferred attribution differs. A reader can check every Card, not only
    // the sampled differences the report quotes.
    let evidence_rows: Vec<Value> = f
        .graph
        .iter()
        .map(|card| {
            let at = field(card, "address");
            json!({
                "address": at,
                "inferred": f.index.book_of.get(&at).cloned().map(Value::String).unwrap_or(Value::Null),
                "sources": sets.get(&ps::key(&at)).cloned().unwrap_or_default().json()
            })
        })
        .collect();
    crate::util::write_text(
        ps::join_path(&f.p.out, "audit-sources.json"),
        &ps::json_pretty(&json!({
            "schema_version": SCHEMA_VERSION,
            "archive_revision": f.revision,
            "rule": "direct: the Citation keys the Card's 来源 lines name, else its own Literature tree (D069, D070); derived: a model head's evidence for a part with none (D071)",
            "cards": evidence_rows
        })),
    )?;
    let new = new_cards(f);
    let sample = sample_block(f);
    let sources = source_block(f, &sets);
    let mut missing: Vec<Value> = Vec::new();
    if f.baseline.is_none() {
        missing.push(json!({
            "kind": "baseline",
            "reason": "no -Baseline report: the new Cards of a comparison interval are unknown",
            "measures": ["P1", "P2", "P3_P5", "P12_P13", "P14"]
        }));
    } else if new.is_none() {
        missing.push(json!({
            "kind": "baseline_addresses",
            "reason": "the baseline folder holds no audit-addresses.txt: the new Cards are unknown",
            "measures": ["P1", "P2", "P3_P5"]
        }));
    }
    if f.concept.is_none() {
        missing.push(json!({
            "kind": "concept_report",
            "reason": "no -ConceptReport: the restatement pairs of a reviewed concept report are unknown",
            "measures": ["P2"]
        }));
    }
    if f.run_report.is_none() {
        missing.push(json!({
            "kind": "run_report",
            "reason": "no -RunReport: the Link jobs and the exact per-run provider usage are unknown",
            "measures": ["P4"]
        }));
    }
    if f.sheet["present"] != json!(true) {
        missing.push(json!({
            "kind": "task_sheet",
            "reason": n(&f.sheet["reason"]),
            "measures": ["P9", "P12_P13", "P15"]
        }));
    } else if f.sheet["valid"] != json!(true) {
        missing.push(json!({
            "kind": "task_sheet",
            "reason": "the task sheet is present but its observations do not match the archive",
            "measures": ["P12_P13", "P15"]
        }));
    }
    let measures = json!({
        "P1": p1(f, &sets, new.as_ref()),
        "P2": p2(f, new.as_ref()),
        "P3_P5": p3_p5(f, &sets, new.as_ref()),
        "P4": p4(f),
        "P7": p7(f, &sets),
        "P8": p8(f),
        "P9": p9(f, &sample),
        "P11": p11(f),
        "P12_P13": p12_p13(f),
        "P14": p14(f),
        "P15": p15(f),
    });
    let comparison = comparison(f, &measures, new.as_ref());
    let topic_trees = f.trees.iter().filter(|tree| ps::eq(&field(tree, "kind"), "topic")).count();
    let evidence = json!({
        "archive": {
            "id": n(&f.identity["id"]),
            "path": n(&f.identity["path"]),
            "revision": f.revision
        },
        "software": {
            "zt_digest": env!("CARGO_PKG_VERSION"),
            "zt_check": zt_check::VERSION,
            "zt": f.zt_version,
            "rules": "R2-R21, I1-I7, E1-E5, F1-F4, L1-L3",
            "advisory_rules": advisory_facts()["rules"].clone(),
            "advisory_subcases": advisory_facts()["subcases"].clone()
        },
        "thresholds": {
            "max_sentences": f.p.max_sentences,
            "max_chars": f.p.max_chars,
            "max_steps": f.p.max_steps,
            "sample": f.p.sample,
            "sample_seed": f.p.sample_seed,
            "near_title": NEAR_TITLE,
            "register_reach_links": REGISTER_REACH_LINKS,
            "task_sheet_schema": SHEET_SCHEMA
        },
        "scope": {
            "kind": scope_kind(f),
            "name": f.scope,
            "workspace": "agent",
            "books": f.books,
            "cards": f.cards.len(),
            "trees": topic_trees
        },
        "sample": sample,
        "inputs": {
            "baseline": f.baseline.map(|b| json!({
                "path": b.path, "read": true,
                "generated": b.report["generated"], "scope": b.report["scope"],
                "archive_revision": b.report["archive_revision"],
                "addresses": b.addresses.as_ref().map(HashSet::len)
            })).unwrap_or_else(|| json!({"path":"","read":false})),
            "concept_report": f.concept.map(|c| json!({
                "path": c.review.path, "read": true,
                "report_id": c.review.json["report_id"], "status": c.review.json["status"],
                "book": c.review.json["book"], "archive": c.review.json["archive"]
            })).unwrap_or_else(|| json!({"path":"","read":false})),
            "decisions": f.concept.as_ref().and_then(|c| c.decisions.as_ref()).map(|d| json!({
                "path": d.path, "read": true, "decision_id": d.json["decision_id"], "report_id": d.json["report_id"]
            })).unwrap_or_else(|| json!({"path":"","read":false})),
            "run_report": f.run_report.map(|r| json!({
                "path": r.path, "read": true, "identity": r.json["identity"], "counts": r.json["counts"]
            })).unwrap_or_else(|| json!({"path":"","read":false})),
            "task_sheet": {"path": f.sheet["path"].clone(), "read": f.sheet["present"] == json!(true)}
        },
        "missing": missing.clone(),
        "complete": missing.is_empty()
    });
    Ok(json!({
        "schema_version": SCHEMA_VERSION,
        "complete": missing.is_empty(),
        "evidence": evidence,
        "sources": sources,
        "measures": measures,
        "comparison": comparison,
        "task_sheet": f.sheet.clone(),
        "proposal": proposal_baseline()
    }))
}

fn scope_kind(f: &Facts) -> &'static str {
    if ps::eq(f.scope, "agent") { "workspace" } else { "book" }
}

/// The Cards this report can call new: inside its scope and absent from the baseline's address
/// list. `None` when no baseline address list exists.
fn new_cards(f: &Facts) -> Option<HashSet<String>> {
    let known = f.baseline?.addresses.as_ref()?;
    if known.is_empty() {
        return None;
    }
    Some(
        f.cards
            .iter()
            .map(|card| field(card, "address"))
            .filter(|at| !known.contains(at))
            .collect(),
    )
}

/// The annotation sample: the Idea Cards of the scope, in ordinal order, drawn by the seed.
fn sample_block(f: &Facts) -> Value {
    let population: Vec<String> = f.cards.iter().filter(|card| ps::eq(&category(f, card), "idea")).map(|card| field(card, "address")).collect();
    let drawn = seeded_sample(population.clone(), f.p.sample_seed as i32, f.p.sample.max(0) as usize);
    json!({
        "kind": "annotation",
        "rule": "the scope's Idea Cards in ordinal order, shuffled by .NET's seeded System.Random and cut to -Sample",
        "seed": f.p.sample_seed,
        "population": population.len(),
        "drawn": drawn.len(),
        "addresses": drawn
    })
}

fn category(f: &Facts, card: &Value) -> String {
    f.index.category.get(&field(card, "address")).cloned().unwrap_or_default()
}

fn in_topic_tree(f: &Facts, card: &Value) -> bool {
    f.trees.iter().any(|tree| ps::eq(&field(tree, "kind"), "topic") && field(tree, "tree") == field(card, "tree"))
}

fn navigation(f: &Facts, card: &Value) -> bool {
    matches!(
        category(f, card).as_str(),
        "hub" | "terms" | "structure" | "root" | "anchor" | "restatement-entry"
    )
}

/// The source sets of every Card in scope, with the places where they differ from the existing
/// inferred attribution (D074): the report shows both, and neither replaces the other.
fn source_block(f: &Facts, sets: &HashMap<String, SourceSet>) -> Value {
    let mut known = 0usize;
    let mut unknown = 0usize;
    let mut multiple = 0usize;
    let mut derived = 0usize;
    let mut from_tree = 0usize;
    let mut from_source_line = 0usize;
    let mut differences: Vec<Value> = Vec::new();
    let mut by_book: BTreeMap<String, [i64; 4]> = BTreeMap::new();
    for card in f.cards {
        let at = field(card, "address");
        let set = sets.get(&ps::key(&at)).cloned().unwrap_or_default();
        let inferred = f.index.book_of.get(&at).cloned();
        if set.unknown() {
            unknown += 1;
        } else {
            known += 1;
        }
        if set.multiple() {
            multiple += 1;
        }
        if !set.derived.is_empty() {
            derived += 1;
        }
        let own = book_of::link_book(&at, &f.index.book_set);
        if !set.direct.is_empty() {
            if own.is_some_and(|tree| set.names(tree)) {
                from_tree += 1;
            }
            if set.direct.iter().any(|book| own.is_none_or(|tree| !ps::eq(tree, book))) {
                from_source_line += 1;
            }
        }
        let bucket = set.all().first().cloned().unwrap_or_else(|| "unknown".into());
        let row = by_book.entry(bucket).or_default();
        row[0] += 1;
        if !set.unknown() {
            row[1] += 1;
        }
        if !set.derived.is_empty() {
            row[2] += 1;
        }
        if set.unknown() {
            row[3] += 1;
        }
        // The difference a reader needs: inferred attribution and the evidence disagree.
        let differs = match &inferred {
            Some(book) => !set.names(book),
            None => !set.unknown(),
        };
        if differs {
            differences.push(json!({
                "address": at,
                "title": field(card, "title"),
                "inferred": inferred.clone().map(Value::String).unwrap_or(Value::Null),
                "sources": set.all(),
                "derived_from": set.derived_from
            }));
        }
    }
    let mut human_cards = 0usize;
    let mut human_links = 0usize;
    let graph_by: HashMap<String, &Value> = f.graph.iter().map(|card| (ps::key(&field(card, "address")), card)).collect();
    for card in f.graph.iter().filter(|card| field(card, "workspace") == "human") {
        human_cards += 1;
        for target in ps_items(&card["links"]).iter().map(s) {
            if graph_by.get(&ps::key(&target)).is_some_and(|found| field(found, "workspace") == "agent") {
                human_links += 1;
            }
        }
    }
    let difference_total = differences.len();
    let list: Vec<Value> = differences.into_iter().take(f.p.sample.max(0) as usize).collect();
    let books: Vec<Value> = by_book
        .into_iter()
        .map(|(book, row)| json!({ "book": book, "cards": row[0], "known": row[1], "derived": row[2], "unknown": row[3] }))
        .collect();
    json!({
        "rule": "direct: the Citation keys the Card's 来源 lines name, else its own Literature tree (D069, D070); derived: a model head's evidence for a part with none (D071); a kinded Link to another book is no source evidence (D072)",
        "cards": { "total": f.cards.len(), "known": known, "unknown": unknown, "multiple": multiple, "derived": derived },
        "direct": { "literature_tree": from_tree, "source_line": from_source_line },
        "differs": difference_total,
        "differences": list,
        "by_book": books,
        "human": { "cards": human_cards, "to_agent_links": human_links }
    })
}

// --- P1: the Topic test (D354) --------------------------------------------------------------------

fn p1(f: &Facts, sets: &HashMap<String, SourceSet>, new: Option<&HashSet<String>>) -> Value {
    let population: Vec<&&Value> =
        f.cards.iter().filter(|card| ps::eq(&category(f, card), "idea") && in_topic_tree(f, card)).collect();
    let count = |cards: &[&&Value]| -> Value {
        let mut own_source = 0usize;
        let mut with_rests_on = 0usize;
        let mut unknown = 0usize;
        let mut addresses: Vec<String> = Vec::new();
        for card in cards {
            let at = field(card, "address");
            let set = sets.get(&ps::key(&at));
            if set.is_none_or(SourceSet::unknown) {
                unknown += 1;
            }
            let Some(kinds) = card["kinds"].as_object() else { continue };
            let rests: Vec<String> = kinds.iter().filter(|(_, kind)| ps::eq(&s(kind), "rests-on")).map(|(target, _)| target.clone()).collect();
            if rests.is_empty() {
                continue;
            }
            with_rests_on += 1;
            if rests.iter().any(|target| own_source_link(target, set, &f.index.book_set)) {
                own_source += 1;
                addresses.push(at);
            }
        }
        json!({
            "value": own_source,
            "with_rests_on": with_rests_on,
            "unknown_source": unknown,
            "denominator": cards.len(),
            "addresses": addresses
        })
    };
    let current = count(&population);
    let Some(new) = new else {
        return measure(
            "New Topic Idea Cards with an own-source rests-on Link, of all checked new Topic Idea Cards",
            false,
            "no baseline report: the new Cards of a comparison interval are unknown",
            json!({ "current": current }),
            Value::Null,
            "the current-scope figures count every Topic Idea Card, not the new ones, so they are no pilot measure",
        );
    };
    let fresh: Vec<&&Value> = population.iter().copied().filter(|card| new.contains(&field(card, "address"))).collect();
    let values = count(&fresh);
    let rate = json!({
        "per_100": per_100(values["value"].as_i64().unwrap_or(0), values["denominator"].as_i64().unwrap_or(0)),
        "of": values["denominator"],
        "unit": "new Topic Idea Cards"
    });
    let mut out = measure(
        "New Topic Idea Cards with an own-source rests-on Link, of all checked new Topic Idea Cards",
        true,
        "",
        json!({
            "value": values["value"], "denominator": values["denominator"],
            "with_rests_on": values["with_rests_on"], "unknown_source": values["unknown_source"],
            "addresses": values["addresses"], "current": current
        }),
        rate,
        "a rests-on Link into the Card's own source book is a book note filed as a Topic claim; unknown-source Cards are counted apart",
    );
    out["target"] = json!(30.0);
    out
}

// --- P2: restatements (D355) ----------------------------------------------------------------------

fn p2(f: &Facts, new: Option<&HashSet<String>>) -> Value {
    let name = "candidate, judged-same, resolved and pending restatement pairs per 100 new Cards";
    let Some(concept) = f.concept else {
        return unknown(name, "no -ConceptReport was supplied");
    };
    let report = &concept.review.json;
    let book = n(&report["book"]["citation_key"]);
    if scope_kind(f) == "book" && !ps::eq(f.scope, &book) {
        return unknown(name, &format!("the concept report reviews `{book}`, which is outside this report's scope `{}`", f.scope));
    }
    if report["archive"]["id"].as_str() != f.identity["id"].as_str() {
        return unknown(name, "the concept report reviews another archive");
    }
    let findings: Vec<Value> = ps_items(&report["findings"]).into_iter().filter(|row| n(&row["kind"]) == "restatement").collect();
    let candidates = findings.len();
    let judged = findings.iter().filter(|row| row["unjudged"] != json!(true)).count();
    let judged_same = findings.iter().filter(|row| n(&row["verdict"]) == "same").count();
    let resolved = concept
        .decisions
        .as_ref()
        .map(|decisions| {
            ps_items(&decisions.json["decisions"]).into_iter().filter(|row| n(&row["kind"]) == "restatement").count()
        })
        .unwrap_or(0);
    let pending = candidates.saturating_sub(resolved);
    let denominator = new.map(HashSet::len).unwrap_or(0) as i64;
    let values = json!({
        "value": resolved,
        "candidates": candidates,
        "judged": judged,
        "judged_same": judged_same,
        "resolved": resolved,
        "pending": pending,
        "new_cards": if new.is_some() { json!(denominator) } else { Value::Null },
        "report_id": report["report_id"],
        "status": report["status"],
        "decisions": concept.decisions.as_ref().map(|d| d.json["decision_id"].clone()).unwrap_or(Value::Null)
    });
    let rate = json!({
        "per_100": if new.is_some() { per_100(resolved as i64, denominator) } else { Value::Null },
        "candidates": if new.is_some() { per_100(candidates as i64, denominator) } else { Value::Null },
        "judged_same": if new.is_some() { per_100(judged_same as i64, denominator) } else { Value::Null },
        "pending": if new.is_some() { per_100(pending as i64, denominator) } else { Value::Null },
        "of": if new.is_some() { json!(denominator) } else { Value::Null },
        "unit": "new Cards"
    });
    let known = new.is_some();
    measure(
        name,
        known,
        if known { "" } else { "no baseline report: the new-Card denominator is unknown" },
        values,
        rate,
        "a detected pair is not a verified duplicate and not a completed merge",
    )
}

// --- P3/P5: mixed-source filing and Topics (D356) -------------------------------------------------

fn p3_p5(f: &Facts, sets: &HashMap<String, SourceSet>, new: Option<&HashSet<String>>) -> Value {
    // The books each Topic tree's substantive Cards are evidence for, kept apart from the
    // inference. A Topic's hub, root and anchors are navigation, not the Topic's claims.
    let mut tree_books: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut tree_cards: HashSet<String> = HashSet::new();
    for card in f.cards {
        if !in_topic_tree(f, card) || navigation(f, card) {
            continue;
        }
        let tree = field(card, "tree");
        tree_cards.insert(tree.clone());
        if let Some(set) = sets.get(&ps::key(&field(card, "address"))) {
            for book in set.all() {
                tree_books.entry(tree.clone()).or_default().insert(book);
            }
        }
    }
    let mixed: BTreeSet<String> = tree_books.iter().filter(|(_, books)| books.len() > 1).map(|(tree, _)| tree.clone()).collect();
    let topics = tree_cards.len();
    let filing = match new {
        None => json!({ "known": false, "reason": "no baseline report: the new Cards of a comparison interval are unknown" }),
        Some(new) => {
            let fresh: Vec<&&Value> = f.cards.iter().filter(|card| new.contains(&field(card, "address"))).collect();
            let filed = fresh.iter().filter(|card| mixed.contains(&field(card, "tree"))).count();
            json!({
                "known": true,
                "value": filed,
                "denominator": fresh.len(),
                "per_100": per_100(filed as i64, fresh.len() as i64),
                "trees": mixed.iter().cloned().collect::<Vec<String>>(),
                "note": "an earlier mixed-source Topic tree is one whose Cards, read now, are evidence for two or more books; a 来源 line added after the baseline is counted"
            })
        }
    };
    json!({
        "known": new.is_some(),
        "reason": if new.is_some() { "" } else { "the new-Card filing measure needs a baseline report" },
        "measure": "new Cards filed in earlier mixed-source Topic trees, and Topics whose supported sources name several books",
        "values": { "value": filing["value"].clone(), "denominator": filing["denominator"].clone() },
        "filing": filing,
        "topics": {
            "known": true,
            "value": mixed.len(),
            "denominator": topics,
            "per_100": per_100(mixed.len() as i64, topics as i64),
            "list": mixed.iter().cloned().collect::<Vec<String>>()
        },
        "note": "a Topic's sources come from its Cards' explicit evidence, never from a tree's numeric age or its newest contributor"
    })
}

// --- P4: appended tails, duplicate owned paragraphs, Link jobs and usage (D357) -------------------

fn p4(f: &Facts) -> Value {
    let mut tails = 0usize;
    let mut cards_with_tails: Vec<String> = Vec::new();
    let mut owners: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for card in f.cards {
        let at = field(card, "address");
        for paragraph in field(card, "body").split("\n\n") {
            let text = paragraph.trim();
            if !ps::starts_with(text, "关联：") {
                continue;
            }
            tails += 1;
            if !cards_with_tails.contains(&at) {
                cards_with_tails.push(at.clone());
            }
            let key = text.split_whitespace().collect::<Vec<_>>().join(" ");
            owners.entry(key).or_default().push(at.clone());
        }
    }
    let duplicates: Vec<Value> =
        owners.iter().filter(|(_, rows)| rows.len() > 1).map(|(text, rows)| json!({ "text": text, "addresses": rows })).collect();
    let run = f.run_report.map(|report| {
        let link = &report.json["counts"]["per_layer"]["link"];
        json!({
            "known": link.is_object(),
            "link": link,
            "usage": report.json["usage"],
            "path": report.path,
            "reason": if link.is_object() { "" } else { "the run report records no Link layer" }
        })
    });
    json!({
        "known": run.as_ref().is_some_and(|run| run["known"] == json!(true)),
        "reason": if f.run_report.is_some() { "" } else { "no -RunReport was supplied: the Link jobs and the exact per-run provider usage are unknown" },
        "measure": "appended 关联： tails, duplicate owned paragraphs, Link jobs and exact per-run provider usage",
        "values": { "value": tails, "denominator": f.cards.len() },
        "tails": {
            "known": true,
            "value": tails,
            "cards": cards_with_tails.len(),
            "denominator": f.cards.len(),
            "per_100": per_100(tails as i64, f.cards.len() as i64),
            "list": cards_with_tails.iter().take(f.p.sample.max(0) as usize).cloned().collect::<Vec<String>>()
        },
        "duplicate_owned_paragraphs": {
            "known": true,
            "value": duplicates.len(),
            "list": duplicates.iter().take(f.p.sample.max(0) as usize).cloned().collect::<Vec<Value>>()
        },
        "run": run.unwrap_or_else(|| json!({"known": false, "reason": "no -RunReport was supplied"})),
        "note": "quality and cost are read in the same scope: a fault rate beside the exact provider usage"
    })
}

// --- P7: hub crossings (D358) ---------------------------------------------------------------------

fn p7(f: &Facts, sets: &HashMap<String, SourceSet>) -> Value {
    let mut cross_lines = 0usize;
    let mut lines = 0usize;
    let mut one_book = 0usize;
    let mut mirror = 0usize;
    let mut unsupported = 0usize;
    let mut unsupported_lines: Vec<Value> = Vec::new();
    let mut crossing: Vec<Value> = Vec::new();
    for hub in f.hubs {
        let at = field(hub, "address");
        let anchor = field(hub, "anchor");
        let anchor_sources = sets.get(&ps::key(&anchor));
        let anchor_known = anchor_sources.is_some_and(|set| !set.unknown());
        let mut hub_lines = 0usize;
        let mut hub_cross = 0usize;
        for line in ps_items(&hub["lines"]) {
            let target = field(&line, "address");
            if target.is_empty() {
                continue;
            }
            hub_lines += 1;
            lines += 1;
            let target_sources = sets.get(&ps::key(&target));
            let target_known = target_sources.is_some_and(|set| !set.unknown());
            if !anchor_known || !target_known {
                unsupported += 1;
                if unsupported_lines.len() < f.p.sample.max(0) as usize {
                    let reason = if !anchor_known { "the hub's anchor has no explicit source evidence" } else { "the target has no explicit source evidence" };
                    unsupported_lines.push(json!({ "hub": at, "target": target, "reason": reason }));
                }
                continue;
            }
            let anchor_books = anchor_sources.map(SourceSet::all).unwrap_or_default();
            let target_books = target_sources.map(SourceSet::all).unwrap_or_default();
            if target_books.iter().any(|book| anchor_books.iter().any(|own| ps::eq(own, book))) {
                continue;
            }
            hub_cross += 1;
            cross_lines += 1;
            if crossing.len() < f.p.sample.max(0) as usize {
                crossing.push(json!({ "hub": at, "target": target, "target_sources": target_books, "anchor_sources": anchor_books }));
            }
        }
        if hub_lines > 0 && hub_cross == 0 {
            one_book += 1;
        }
        if hub["mirror"] == json!(true) {
            mirror += 1;
        }
    }
    json!({
        "known": true,
        "measure": "cross-book hub lines, and the justified one-book and mirror advice",
        "values": { "value": cross_lines, "denominator": lines },
        "cross_book_lines": {
            "value": cross_lines,
            "denominator": lines,
            "per_100": per_100(cross_lines as i64, lines as i64),
            "list": crossing
        },
        "hubs": { "total": f.hubs.len(), "one_book": one_book, "mirror": mirror },
        "unsupported": { "value": unsupported, "sample": unsupported_lines },
        "note": "a line crosses books only on explicit source evidence; an unsupported Link is counted apart, so no ratio rewards one"
    })
}

// --- P8: the register and reach (D359, D365) ------------------------------------------------------

fn p8(f: &Facts) -> Value {
    let unreached: i64 = f.hubs.iter().map(|hub| hub["unreached_count"].as_i64().unwrap_or(0)).sum();
    let hubs_with_unreached = f.hubs.iter().filter(|hub| hub["unreached_count"].as_i64().unwrap_or(0) > 0).count();
    let hub_part = json!({
        "known": true,
        "unreached": unreached,
        "hubs_with_unreached": hubs_with_unreached,
        "note": "the existing hub measure: a Card of a hub's range that no hub reaches"
    });
    let name = "accepted register terms, unresolved rows, and the Cards reachable within two semantic Links of an entry target";
    let Some(register) = f.register else {
        return json!({ "known": false, "reason": "the agent Workspace has no readable register", "measure": name, "hub_unreached": hub_part });
    };
    if register["register"].is_null() {
        return json!({
            "known": false,
            "reason": "the agent Workspace has no register, or more than one",
            "measure": name,
            "hub_unreached": hub_part
        });
    }
    let entries = ps_items(&register["entries"]);
    let mut targets: Vec<String> = Vec::new();
    let mut all_targets: Vec<String> = Vec::new();
    let mut navigation_targets = 0usize;
    let mut incomplete = 0usize;
    let mut navigable: HashSet<String> = HashSet::new();
    navigable.insert(ps::key(&n(&register["register"])));
    for part in ps_items(&register["parts"]) {
        navigable.insert(ps::key(&field(&part, "address")));
    }
    for card in f.graph.iter().filter(|card| navigation(f, card)) {
        navigable.insert(ps::key(&field(card, "address")));
    }
    for entry in &entries {
        if entry["complete"] != json!(true) {
            incomplete += 1;
        }
        for target in ps_items(&entry["targets"]) {
            if target["exists"] != json!(true) {
                continue;
            }
            let at = field(&target, "address");
            if !all_targets.iter().any(|seen| ps::eq(seen, &at)) {
                all_targets.push(at.clone());
            }
            if navigable.contains(&ps::key(&at)) {
                navigation_targets += 1;
                continue;
            }
            if !targets.iter().any(|seen| ps::eq(seen, &at)) {
                targets.push(at);
            }
        }
    }
    let graph_by: HashMap<String, &Value> = f.graph.iter().map(|card| (ps::key(&field(card, "address")), card)).collect();
    let (at_1, at_2) = register_reach(&targets, &graph_by, &navigable);
    json!({
        "known": true,
        "measure": name,
        "values": {
            "value": targets.len(),
            "denominator": entries.len(),
            "targets": all_targets.len(),
            "navigation_targets": navigation_targets
        },
        "terms": {
            "accepted": entries.len(),
            "rows": register["entry_total"].as_i64().unwrap_or(0),
            "unresolved_rows": ps::ps_count(&register["faults"]),
            "incomplete_entries": incomplete
        },
        "direct": {
            "value": targets.len(),
            "navigation": navigation_targets,
            "addresses": targets.iter().take(f.p.sample.max(0) as usize).cloned().collect::<Vec<String>>()
        },
        "reach": {
            "direction": "outbound",
            "hops": REGISTER_REACH_LINKS,
            "at_1": at_1.len(),
            "at_2": at_2.len(),
            "cards": at_1.len() + at_2.len(),
            "sample": at_2.iter().take(f.p.sample.max(0) as usize).cloned().collect::<Vec<String>>(),
            "rule": "authored semantic Links (kinded, not refers) from the substantive entry targets; the register, its parts, and navigation Cards are neither a hop nor a destination"
        },
        "hub_unreached": hub_part,
        "complete": register["complete"].clone(),
        "note": "direct register destinations, semantic reach, and the hub unreached measure are three different definitions and are reported apart"
    })
}

// --- P9: local references and unnamed referents (D360) --------------------------------------------

fn p9(f: &Facts, sample: &Value) -> Value {
    let in_scope: HashSet<String> = f.cards.iter().map(|card| field(card, "address")).collect();
    let count = |rule: &str| -> (usize, usize) {
        let rows: Vec<&super::Fault> = f.faults.iter().filter(|fault| ps::eq(&fault.rule, rule) && in_scope.contains(&fault.address)).collect();
        let cards: HashSet<&String> = rows.iter().map(|fault| &fault.address).collect();
        (rows.len(), cards.len())
    };
    let (local_faults, local_cards) = count("R20");
    let (unnamed_faults, unnamed_cards) = count("R21");
    let annotated: Vec<Value> = ps_items(&f.sheet["annotation"]);
    let sampled: HashSet<String> = ps_items(&sample["addresses"]).iter().map(s).collect();
    let outside_sample = annotated.iter().filter(|row| !sampled.contains(&field(row, "address"))).count();
    let local_fp = annotated.iter().filter(|row| row["local_reference_false_positive"] == json!(true)).count();
    let unnamed_fp = annotated.iter().filter(|row| row["unnamed_referent_false_positive"] == json!(true)).count();
    let manual_known = f.sheet["present"] == json!(true) && !annotated.is_empty();
    json!({
        "known": true,
        "measure": "local-reference and unnamed-referent rates, and the manually annotated false-positive rate",
        "values": { "value": local_cards, "denominator": f.cards.len() },
        "local_reference": { "faults": local_faults, "cards": local_cards, "denominator": f.cards.len(), "per_100": per_100(local_cards as i64, f.cards.len() as i64) },
        "unnamed_referent": { "faults": unnamed_faults, "cards": unnamed_cards, "denominator": f.cards.len(), "per_100": per_100(unnamed_cards as i64, f.cards.len() as i64) },
        "manual": if manual_known {
            json!({
                "known": true,
                "annotated": annotated.len(),
                "outside_sample": outside_sample,
                "local_reference_false_positive": local_fp,
                "unnamed_referent_false_positive": unnamed_fp,
                "local_reference_rate": per_100(local_fp as i64, annotated.len() as i64),
                "unnamed_referent_rate": per_100(unnamed_fp as i64, annotated.len() as i64)
            })
        } else {
            json!({ "known": false, "reason": "the task sheet holds no annotation of the sample" })
        },
        "sample": sample,
        "note": "a count from a heuristic is not proof that a Card is incomprehensible; the annotation is what judges it"
    })
}

// --- P11: titles (D361) ---------------------------------------------------------------------------

fn p11(f: &Facts) -> Value {
    let ideas: Vec<(String, String)> =
        f.cards.iter().filter(|card| !navigation(f, card)).map(|card| (field(card, "address"), field(card, "title"))).collect();
    let nav: Vec<(String, String)> =
        f.cards.iter().filter(|card| navigation(f, card)).map(|card| (field(card, "address"), field(card, "title"))).collect();
    let (idea_exact, idea_near, idea_truncated) = title_pairs(&ideas);
    let (nav_exact, nav_near, nav_truncated) = title_pairs(&nav);
    let anchor_title = f
        .hubs
        .iter()
        .filter(|hub| ps_items(&hub["faults"]).iter().any(|fault| n(&fault["fault"]) == "anchor-title"))
        .count();
    json!({
        "known": true,
        "measure": "exact and near title pairs and the anchor-title faults, with the idea and navigation populations apart",
        "values": { "value": idea_exact + idea_near, "denominator": ideas.len() },
        "idea": {
            "cards": ideas.len(), "exact_pairs": idea_exact, "near_pairs": idea_near,
            "truncated": idea_truncated, "per_100": per_100((idea_exact + idea_near) as i64, ideas.len() as i64)
        },
        "navigation": {
            "cards": nav.len(), "exact_pairs": nav_exact, "near_pairs": nav_near,
            "truncated": nav_truncated, "per_100": per_100((nav_exact + nav_near) as i64, nav.len() as i64)
        },
        "anchor_title_faults": anchor_title,
        "threshold": NEAR_TITLE,
        "note": "exact is the checker's title identity (letters and digits, case ignored); near is I6's bigram Dice at the same threshold"
    })
}

/// The exact and near title pairs of one population, deduplicated as unordered pairs. The
/// bigram index bounds the candidates, so a whole-Workspace report does not compare every Card
/// with every other; a truncated scan says so.
fn title_pairs(rows: &[(String, String)]) -> (usize, usize, bool) {
    const CANDIDATE_MAX: usize = 400;
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut exact = 0usize;
    for (_, title) in rows {
        let seen = by_key.entry(zt_check::check::title_key(title)).or_default();
        exact += *seen;
        *seen += 1;
    }
    let mut index: HashMap<String, Vec<usize>> = HashMap::new();
    let mut grams: Vec<HashSet<String>> = Vec::with_capacity(rows.len());
    for (at, (_, title)) in rows.iter().enumerate() {
        let set = zt_check::check::title_bigrams(title);
        for gram in &set {
            index.entry(gram.clone()).or_default().push(at);
        }
        grams.push(set);
    }
    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    let mut near = 0usize;
    let mut truncated = false;
    for (at, set) in grams.iter().enumerate() {
        let mut candidates: BTreeSet<usize> = BTreeSet::new();
        for gram in set {
            for other in index.get(gram).into_iter().flatten() {
                if *other != at {
                    candidates.insert(*other);
                }
            }
        }
        if candidates.len() > CANDIDATE_MAX {
            truncated = true;
        }
        for other in candidates.into_iter().take(CANDIDATE_MAX) {
            let pair = if at < other { (at, other) } else { (other, at) };
            if !pairs.insert(pair) {
                continue;
            }
            if zt_check::check::title_key(&rows[pair.0].1) == zt_check::check::title_key(&rows[pair.1].1) {
                continue;
            }
            if zt_check::check::title_similarity(&rows[pair.0].1, &rows[pair.1].1) >= NEAR_TITLE {
                near += 1;
            }
        }
    }
    (exact, near, truncated)
}

// --- P12/P13 and P15: the fixed manual task sheet (D362, D364) ------------------------------------

fn p12_p13(f: &Facts) -> Value {
    let name = "time to the first useful Card and to a kept Spread, removed Cards, and the entry-target rank";
    if f.sheet["present"] != json!(true) {
        return unknown(name, "no -TaskSheet was supplied");
    }
    if f.sheet["valid"] != json!(true) {
        return measure(
            name,
            false,
            "the task sheet is present but its observations do not match the archive",
            json!({ "faults": f.sheet["faults"] }),
            Value::Null,
            "",
        );
    }
    let tasks = ps_items(&f.sheet["tasks"]);
    let numbers = |kind: &str, name: &str| -> Vec<i64> {
        tasks.iter().filter(|task| n(&task["kind"]) == kind).filter_map(|task| task[name].as_i64()).collect()
    };
    let first = numbers("first-useful-card", "seconds");
    let kept = numbers("kept-spread", "seconds");
    let ranks = numbers("first-useful-card", "entry_rank");
    json!({
        "known": true,
        "measure": name,
        "values": { "value": first.len(), "denominator": ps::ps_count(&f.sheet["tasks"]) },
        "first_useful_card": { "tasks": first.len(), "median_seconds": median(&first), "seconds": first },
        "kept_spread": { "tasks": kept.len(), "median_seconds": median(&kept), "seconds": kept },
        "entry_rank": { "tasks": ranks.len(), "median": median(&ranks) },
        "removed_cards": { "recorded": ps::ps_count(&f.sheet["removed_cards"]), "verified_absent": f.sheet["removed_verified"] },
        "human_replies": f.sheet["human_replies"],
        "interval": f.sheet["interval"],
        "note": "a small repeatable task sheet, not an automatic claim about usefulness; the archive itself records no reading time"
    })
}

fn p15(f: &Facts) -> Value {
    let name = "distant Cards offered, opened and laid";
    if f.sheet["present"] != json!(true) {
        return unknown(name, "no -TaskSheet was supplied");
    }
    let distant: Vec<Value> = ps_items(&f.sheet["tasks"]).into_iter().filter(|task| n(&task["kind"]) == "distant-card").collect();
    let sum = |name: &str| -> i64 { distant.iter().map(|task| task[name].as_i64().unwrap_or(0)).sum() };
    json!({
        "known": true,
        "measure": name,
        "values": { "value": sum("laid"), "denominator": sum("offered") },
        "tasks": distant.len(),
        "offered": sum("offered"),
        "opened": sum("opened"),
        "laid": sum("laid"),
        "list": distant,
        "note": "manual observation in the task sheet; there is no telemetry store"
    })
}

fn median(values: &[i64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort();
    let Some(last) = sorted.len().checked_sub(1) else { return Value::Null };
    if sorted.len() % 2 == 1 {
        json!(sorted[last / 2])
    } else {
        number_json((sorted[last / 2] as f64 + sorted[last / 2 + 1] as f64) / 2.0)
    }
}

// --- P14: the human half (D363) -------------------------------------------------------------------

fn p14(f: &Facts) -> Value {
    let human_cards = f.graph.iter().filter(|card| field(card, "workspace") == "human").count();
    let graph_by: HashMap<String, &Value> = f.graph.iter().map(|card| (ps::key(&field(card, "address")), card)).collect();
    let mut authored = 0usize;
    let mut semantic = 0usize;
    for card in f.graph.iter().filter(|card| field(card, "workspace") == "human") {
        let Some(kinds) = card["kinds"].as_object() else { continue };
        for target in ps_items(&card["links"]).iter().map(s) {
            let Some(found) = graph_by.get(&ps::key(&target)) else { continue };
            if field(found, "workspace") != "agent" {
                continue;
            }
            authored += 1;
            if kinds.get(&target).is_some_and(|kind| !ps::eq(&s(kind), "refers")) {
                semantic += 1;
            }
        }
    }
    let interval = f.baseline.and_then(|baseline| {
        let before = &baseline.sources.as_ref()?["human"];
        if !before.is_object() {
            return None;
        }
        Some(json!({
            "baseline_revision": baseline.report["archive_revision"],
            "human_cards_before": before["cards"],
            "to_agent_links_before": before["to_agent_links"],
            "human_cards_net_change": human_cards as i64 - before["cards"].as_i64().unwrap_or(0),
            "to_agent_links_net_change": authored as i64 - before["to_agent_links"].as_i64().unwrap_or(0),
            "note": "a net change between two reads, not a count of authored events: a removed Card or Link offsets an added one"
        }))
    });
    json!({
        "known": true,
        "measure": "human Cards and human-to-agent authored Links, with the comparison interval when a baseline reports them",
        "values": { "value": human_cards, "denominator": f.graph.len() },
        "human_cards": human_cards,
        "to_agent_links": { "authored": authored, "semantic": semantic },
        "interval": interval.unwrap_or(Value::Null),
        "note": "an observation, not a production quota"
    })
}

// --- the comparison (S01-AC06, D366) --------------------------------------------------------------

/// The comparison with an earlier report: a baseline value is carried with the scope it was
/// measured on, and a delta exists only when both reports describe the same scope. A
/// full-Workspace baseline is never compared with a selected-book report as though matched.
fn comparison(f: &Facts, measures: &Value, new: Option<&HashSet<String>>) -> Value {
    let Some(baseline) = f.baseline else {
        return json!({
            "known": false,
            "reason": "no -Baseline report was supplied",
            "proposal": proposal_scope_note()
        });
    };
    let before = &baseline.report;
    let matched = n(&before["scope"]) == f.scope;
    let scope_change = if matched {
        Value::Null
    } else {
        json!(format!(
            "the baseline audits `{}` and this report audits `{}`; their denominators differ, so no value is compared as matched",
            n(&before["scope"]),
            f.scope
        ))
    };
    let current_row = if matched && scope_kind(f) == "book" { f.book_rows.first().cloned().unwrap_or(Value::Null) } else { f.workspace_row.clone() };
    let before_row = if matched && scope_kind(f) == "book" { before["books"][0].clone() } else { before["workspace"].clone() };
    let mut rows = Map::new();
    for (name, path) in [
        ("cards", "cards"),
        ("orphans", "orphans.count"),
        ("no_source", "no_source.count"),
        ("unkinded", "unkinded.count"),
        ("style_faults", "style.faults"),
        ("layer_faults", "layer.faults"),
    ] {
        rows.insert(name.into(), compare_row(lookup(&before_row, path), lookup(&current_row, path), matched));
    }
    // The S01 measures an earlier report of this version published.
    if let Some(published) = before["baseline"]["measures"].as_object() {
        for (name, measure) in published {
            let at = measure["values"]["value"].clone();
            let now = measures[name]["values"]["value"].clone();
            if at.is_null() && now.is_null() {
                continue;
            }
            rows.insert(name.clone(), compare_row(Some(at), Some(now), matched));
        }
    }
    json!({
        "known": true,
        "baseline": {
            "path": baseline.path,
            "generated": before["generated"],
            "scope": before["scope"],
            "archive_revision": before["archive_revision"],
            "addresses": baseline.addresses.as_ref().map(HashSet::len),
            "own_measures": before["baseline"]["measures"].is_object()
        },
        "matched": matched,
        "scope_change": scope_change,
        "new_cards": new.map(|set| {
            let mut list: Vec<String> = set.iter().cloned().collect();
            list.sort();
            json!({
                "count": list.len(),
                "rule": "inside this report's scope and absent from the baseline's audit-addresses.txt",
                "addresses": list.iter().take(f.p.sample.max(0) as usize).cloned().collect::<Vec<String>>()
            })
        }).unwrap_or(Value::Null),
        "measures": rows,
        "proposal": proposal_scope_note()
    })
}

fn proposal_scope_note() -> Value {
    json!({
        "scope_change": "the accepted proposal's counts keep their original scope (the whole live agent Workspace, 10,887 Cards); this report's counts are its own scope, so the two are shown side by side and never as a delta"
    })
}

/// One compared value: the baseline, the current value, and a delta only when the scopes match.
fn compare_row(before: Option<Value>, now: Option<Value>, matched: bool) -> Value {
    let delta = match (&before, &now) {
        (Some(a), Some(b)) if matched => match (a.as_i64(), b.as_i64()) {
            (Some(a), Some(b)) => json!(b - a),
            _ => Value::Null,
        },
        _ => Value::Null,
    };
    let status = if !matched {
        "scope_mismatch"
    } else if delta.is_null() {
        "not_comparable"
    } else {
        "matched"
    };
    json!({
        "baseline": before.unwrap_or(Value::Null),
        "current": now.unwrap_or(Value::Null),
        "delta": delta,
        "status": status
    })
}

/// The value at a dotted path of a report row, or `None` when the path is absent.
fn lookup(row: &Value, path: &str) -> Option<Value> {
    let mut value = row;
    for part in path.split('.') {
        value = value.get(part)?;
    }
    Some(value.clone())
}

// --- the seeded sample (D353) ---------------------------------------------------------------------

/// A reproducible sample: the population in ordinal order, shuffled by .NET's seeded
/// `System.Random` (the one the digest already uses), and the first `take` addresses kept,
/// sorted. The same seed and population draw the same addresses, so a hand annotation can name
/// exactly the Cards it read.
pub fn seeded_sample(mut population: Vec<String>, seed: i32, take: usize) -> Vec<String> {
    population.sort();
    if take == 0 || population.is_empty() {
        return Vec::new();
    }
    let mut random = ps::DotNetRandom::new(seed);
    for at in (1..population.len()).rev() {
        let pick = (random.next() as usize) % (at + 1);
        population.swap(at, pick);
    }
    let mut drawn: Vec<String> = population.into_iter().take(take).collect();
    drawn.sort();
    drawn
}

// --- P8's register reach (D365) -------------------------------------------------------------------

/// The Cards reachable from the register's substantive entry targets along authored semantic
/// Links, outbound, at most `REGISTER_REACH_LINKS` hops, with navigation Cards neither a hop
/// nor a destination: a register part is not a semantic step.
fn register_reach(targets: &[String], graph: &HashMap<String, &Value>, nav: &HashSet<String>) -> (Vec<String>, Vec<String>) {
    let mut at_1: Vec<String> = Vec::new();
    let mut at_2: Vec<String> = Vec::new();
    let start: HashSet<String> = targets.iter().map(|at| ps::key(at)).collect();
    for target in targets {
        let Some(card) = graph.get(&ps::key(target)) else { continue };
        for next in semantic_neighbours(card, graph, nav) {
            let key = ps::key(&next);
            if start.contains(&key) || at_1.iter().any(|seen| ps::key(seen) == key) {
                continue;
            }
            at_1.push(next);
        }
    }
    let first: HashSet<String> = at_1.iter().map(|at| ps::key(at)).collect();
    for from in &at_1 {
        let Some(card) = graph.get(&ps::key(from)) else { continue };
        for next in semantic_neighbours(card, graph, nav) {
            let key = ps::key(&next);
            if start.contains(&key) || first.contains(&key) || at_2.iter().any(|seen| ps::key(seen) == key) {
                continue;
            }
            at_2.push(next);
        }
    }
    (at_1, at_2)
}

/// The semantic neighbours of one Card: a kinded Link that is not `refers`, to a Card that
/// exists, is no navigation Card, and is not the Card itself.
fn semantic_neighbours(card: &Value, graph: &HashMap<String, &Value>, nav: &HashSet<String>) -> Vec<String> {
    let Some(kinds) = card["kinds"].as_object() else { return Vec::new() };
    let mut out: Vec<String> = Vec::new();
    for (target, kind) in kinds {
        if ps::eq(&s(kind), "refers") || nav.contains(&ps::key(target)) || !graph.contains_key(&ps::key(target)) {
            continue;
        }
        if ps::key(target) != ps::key(&field(card, "address")) {
            out.push(target.clone());
        }
    }
    out
}

// --- the fixed manual task sheet (S01-AC05) -------------------------------------------------------

/// Read and check the fixed manual task sheet. A sheet that does not answer the fixed schema is
/// refused before the report is published; a sheet whose observations do not match the archive
/// is reported as an invalid observation, never as a measured zero.
pub fn task_sheet(path: &str, cards: &HashSet<String>) -> Result<Value> {
    if path.trim().is_empty() {
        return Ok(json!({ "path": "", "present": false, "valid": false, "reason": "no -TaskSheet was supplied" }));
    }
    let raw = crate::util::read_json(path).with_context(|| format!("the task sheet `{path}` is not readable JSON"))?;
    let sheet = validate_sheet(&raw).with_context(|| format!("the task sheet `{path}` is not a valid fixed task sheet"))?;
    let mut faults: Vec<Value> = Vec::new();
    for task in ps_items(&sheet["tasks"]) {
        if n(&task["kind"]) != "first-useful-card" {
            continue;
        }
        let at = field(&task, "useful_address");
        if !cards.contains(&at) {
            faults.push(json!({ "task": field(&task, "id"), "fault": "useful_address", "detail": format!("`{at}` is not a Card of this archive") }));
        }
    }
    let removed: Vec<String> = ps_items(&sheet["removed_cards"]).iter().map(s).collect();
    let still_present: Vec<String> = removed.iter().filter(|at| cards.contains(*at)).cloned().collect();
    if !still_present.is_empty() {
        faults.push(json!({
            "task": "removed_cards",
            "fault": "still_present",
            "detail": format!("{} recorded as removed still exist: {}", still_present.len(), still_present.join(", "))
        }));
    }
    let mut out = sheet.clone();
    out["path"] = json!(path);
    out["present"] = json!(true);
    out["valid"] = json!(faults.is_empty());
    out["faults"] = json!(faults);
    out["removed_verified"] = json!(removed.len() - still_present.len());
    Ok(out)
}

/// The sheet's fixed schema: every field is named, every kind is one of the three, and every
/// number is a real number. An unknown field is refused, so a later version's column cannot be
/// read as this one's.
fn validate_sheet(raw: &Value) -> Result<Value> {
    let map = raw.as_object().context("the sheet is not an object")?;
    for name in map.keys() {
        if !["schema_version", "kind", "interval", "tasks", "removed_cards", "human_replies", "annotation"].contains(&name.as_str()) {
            bail!("unknown field `{name}`");
        }
    }
    if raw["schema_version"].as_i64() != Some(SHEET_SCHEMA) {
        bail!("schema_version must be {SHEET_SCHEMA}");
    }
    if field(raw, "kind") != "zt-task-sheet" {
        bail!("kind must be `zt-task-sheet`");
    }
    let interval = raw["interval"].as_object().context("interval is required")?;
    for name in interval.keys() {
        if !["from", "to"].contains(&name.as_str()) {
            bail!("interval has an unknown field `{name}`");
        }
    }
    for name in ["from", "to"] {
        if field(&raw["interval"], name).is_empty() {
            bail!("interval.{name} is required");
        }
    }
    let tasks = raw["tasks"].as_array().context("tasks must be an array")?;
    let mut seen: Vec<String> = Vec::new();
    for (index, task) in tasks.iter().enumerate() {
        let number = index + 1;
        let row = task.as_object().with_context(|| format!("task {number} is not an object"))?;
        let id = field(task, "id");
        if id.is_empty() {
            bail!("task {number} has no id");
        }
        if seen.contains(&id) {
            bail!("task `{id}` appears twice");
        }
        seen.push(id.clone());
        let kind = field(task, "kind");
        if !is_in(&kind, &TASK_KINDS) {
            bail!("task `{id}` names the kind `{kind}`; it must be one of {}", TASK_KINDS.join(", "));
        }
        let allowed: &[&str] = match kind.as_str() {
            "first-useful-card" => &["id", "kind", "question", "seconds", "useful_address", "entry_rank"],
            "kept-spread" => &["id", "kind", "question", "seconds", "kept_spread"],
            _ => &["id", "kind", "question", "offered", "opened", "laid"],
        };
        for name in row.keys() {
            if !allowed.contains(&name.as_str()) {
                bail!("task `{id}` of kind `{kind}` has an unknown field `{name}`");
            }
        }
        if field(task, "question").trim().is_empty() {
            bail!("task `{id}` has no question");
        }
        match kind.as_str() {
            "first-useful-card" => {
                number_field(task, &id, "seconds")?;
                if number_field(task, &id, "entry_rank")? < 1 {
                    bail!("task `{id}` must have an entry_rank of 1 or more");
                }
                if field(task, "useful_address").is_empty() {
                    bail!("task `{id}` names no useful_address");
                }
            }
            "kept-spread" => {
                number_field(task, &id, "seconds")?;
                if field(task, "kept_spread").is_empty() {
                    bail!("task `{id}` names no kept_spread");
                }
            }
            _ => {
                let offered = number_field(task, &id, "offered")?;
                let opened = number_field(task, &id, "opened")?;
                let laid = number_field(task, &id, "laid")?;
                if opened > offered || laid > opened {
                    bail!("task `{id}` has laid {laid}, opened {opened}, offered {offered}: each must not exceed the one before");
                }
            }
        }
    }
    for name in ["removed_cards"] {
        let rows = raw[name].as_array().with_context(|| format!("{name} must be an array"))?;
        if rows.iter().any(|row| row.as_str().is_none_or(str::is_empty)) {
            bail!("{name} holds an entry that is not an address");
        }
    }
    if raw["human_replies"].as_i64().is_none_or(|replies| replies < 0) {
        bail!("human_replies must be a whole number of 0 or more");
    }
    let annotations = raw["annotation"].as_array().context("annotation must be an array")?;
    for (index, annotation) in annotations.iter().enumerate() {
        let number = index + 1;
        let object = annotation.as_object().with_context(|| format!("annotation {number} is not an object"))?;
        for name in object.keys() {
            if !["address", "local_reference_false_positive", "unnamed_referent_false_positive"].contains(&name.as_str()) {
                bail!("annotation {number} has an unknown field `{name}`");
            }
        }
        if field(annotation, "address").is_empty() {
            bail!("annotation {number} names no address");
        }
        for name in ["local_reference_false_positive", "unnamed_referent_false_positive"] {
            if annotation[name].as_bool().is_none() {
                bail!("annotation {number} needs a true or false `{name}`");
            }
        }
    }
    Ok(raw.clone())
}

/// One whole number of the sheet, kept non-negative.
fn number_field(task: &Value, id: &str, name: &str) -> Result<i64> {
    let value = task[name].as_i64().with_context(|| format!("task `{id}` needs a whole number `{name}`"))?;
    if value < 0 {
        bail!("task `{id}` has a negative `{name}`");
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_audit_publishes_the_shared_advisory_rules() {
        // S27 (#166, AC06): the audit is a check-result consumer like every compiler. Its published
        // advice reads the checker's own default advisory catalog instead of a private list, so a
        // rule that changes state changes the audit's metadata with it. I7's unknown-source line
        // stays a subcase of a blocking rule, never an advisory rule of its own.
        let facts = advisory_facts();
        assert_eq!(facts["rules"], json!(zt_check::policy::ADVISORY_RULES));
        let rules = zt_check::policy::ADVISORY_RULES;
        assert!(rules.contains(&"T4"), "T4 is an advisory rule");
        assert!(!rules.contains(&"I7"), "I7 is a blocking rule with an advisory subcase");
        assert_eq!(facts["subcases"], json!(["I7"]));
        for rule in &["R20", "R21", "I1", "I6", "L3", "E5"] {
            assert!(rules.contains(rule), "{rule} is advisory");
        }
    }

    fn lib_index(books: &[&str]) -> BookIndex {
        let trees: Vec<Value> = books
            .iter()
            .map(|book| json!({ "tree": book, "kind": "literature", "workspace": "agent" }))
            .collect();
        book_of::book_index(&trees, &[])
    }

    fn set(direct: &[&str]) -> SourceSet {
        SourceSet { direct: direct.iter().map(|s| s.to_string()).collect(), derived: Vec::new(), derived_from: Vec::new() }
    }

    #[test]
    fn a_source_set_stays_unknown_or_multiple() {
        let none = SourceSet::default();
        assert!(none.unknown() && !none.multiple());
        let one = set(&["A"]);
        assert!(!one.unknown() && !one.multiple() && one.names("a"));
        let two = set(&["A", "B"]);
        assert!(two.multiple() && two.all() == ["A", "B"]);
        // A model head's evidence fills a part that has none of its own (D071).
        let inherited = SourceSet { direct: Vec::new(), derived: vec!["A".into()], derived_from: vec!["A/1|m".into()] };
        assert!(!inherited.unknown() && inherited.derived_from == ["A/1|m"]);
        assert_eq!(inherited.json()["derived_from"], json!(["A/1|m"]));
    }

    #[test]
    fn own_source_links_read_the_explicit_set_not_the_tree() {
        let index = lib_index(&["Alpha", "Beta"]);
        // A Card whose evidence is Alpha rests on an Alpha anchor by its own source.
        assert!(own_source_link("Alpha/1|a", Some(&set(&["Alpha"])), &index.book_set));
        // The same Link is no own-source premise for a Card whose evidence is Beta (D072).
        assert!(!own_source_link("Alpha/1|a", Some(&set(&["Beta"])), &index.book_set));
        // Unknown evidence is never an own source, and never a pass (D073).
        assert!(!own_source_link("Alpha/1|a", Some(&SourceSet::default()), &index.book_set));
        assert!(!own_source_link("Alpha/1|a", None, &index.book_set));
        // A Link that names no book has no own source.
        assert!(!own_source_link("12/1|a", Some(&set(&["Alpha"])), &index.book_set));
    }

    #[test]
    fn a_part_inherits_its_model_head_evidence_and_keeps_its_own() {
        let trees = vec![
            json!({ "tree": "Alpha", "kind": "literature", "workspace": "agent" }),
            json!({ "tree": "12", "kind": "topic", "workspace": "agent" }),
        ];
        let graph = vec![
            json!({ "address": "Alpha/1|m", "tree": "Alpha", "workspace": "agent", "body": "", "links": [] }),
            json!({ "address": "12/1|a", "tree": "12", "workspace": "agent", "body": "依赖 [[Alpha/1]]。", "links": ["Alpha/1"] }),
            json!({ "address": "12/1|b", "tree": "12", "workspace": "agent", "body": "来源：[[Alpha/1]]", "links": ["Alpha/1"] }),
        ];
        let models = vec![json!({
            "head": { "address": "Alpha/1|m" },
            "parts": [{ "address": "12/1|a" }, { "address": "12/1|b" }]
        })];
        let sets = source_sets(&graph, &models);
        // The head is in the Alpha Literature tree: its tree is its evidence.
        assert_eq!(sets["alpha/1|m"].direct, ["Alpha"]);
        // A part with no direct evidence takes the head's and says where from (D071).
        assert!(sets["12/1|a"].direct.is_empty() && sets["12/1|a"].derived == ["Alpha"]);
        assert_eq!(sets["12/1|a"].derived_from, ["Alpha/1|m"]);
        // A part whose own 来源 line names the book keeps its own evidence.
        assert_eq!(sets["12/1|b"].direct, ["Alpha"]);
        assert!(sets["12/1|b"].derived.is_empty());
        let _ = trees;
    }

    #[test]
    fn the_seeded_sample_is_reproducible_and_bounded() {
        let population: Vec<String> = (1..=20).map(|n| format!("12/{n}")).collect();
        let first = seeded_sample(population.clone(), 7, 5);
        assert_eq!(first.len(), 5);
        assert_eq!(first, seeded_sample(population.clone(), 7, 5));
        assert_ne!(first, seeded_sample(population.clone(), 8, 5));
        assert_eq!(seeded_sample(population.clone(), 7, 0), Vec::<String>::new());
        assert_eq!(seeded_sample(population.clone(), 7, 99).len(), 20);
        // Sorted, and drawn from the population only.
        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(first, sorted);
        assert!(first.iter().all(|at| population.contains(at)));
    }

    #[test]
    fn reach_traverses_semantic_links_only_and_never_navigation() {
        let graph_rows = vec![
            json!({ "address": "12/1", "workspace": "agent", "kinds": { "12/2": "rests-on" } }),
            json!({ "address": "12/2", "workspace": "agent", "kinds": { "12/3": "corresponds", "13/1": "refers", "12/4": "illustrates" } }),
            json!({ "address": "12/3", "workspace": "agent", "kinds": {} }),
            json!({ "address": "12/4", "workspace": "agent", "kinds": {} }),
            json!({ "address": "13/1", "workspace": "agent", "kinds": {} }),
            json!({ "address": "12/9|h", "workspace": "agent", "kinds": {} }),
        ];
        let graph: HashMap<String, &Value> = graph_rows.iter().map(|row| (ps::key(&field(row, "address")), row)).collect();
        let navigation: HashSet<String> = ["12/9|h".to_string()].into_iter().map(|at| ps::key(&at)).collect();
        let (at_1, at_2) = register_reach(&["12/1".to_string()], &graph, &navigation);
        // One hop: the semantic Link. A `refers` Link is no hop, and neither is an unknown target.
        assert_eq!(at_1, ["12/2"]);
        // Two hops: both semantic neighbours of the first hop, deduplicated.
        assert_eq!(at_2, ["12/3", "12/4"]);
        // A navigation Card is neither a destination nor a hop.
        let with_nav = vec![
            json!({ "address": "12/1", "workspace": "agent", "kinds": { "12/9|h": "rests-on" } }),
            json!({ "address": "12/9|h", "workspace": "agent", "kinds": { "12/3": "rests-on" } }),
            json!({ "address": "12/3", "workspace": "agent", "kinds": {} }),
        ];
        let graph: HashMap<String, &Value> = with_nav.iter().map(|row| (ps::key(&field(row, "address")), row)).collect();
        let navigation: HashSet<String> = ["12/9|h".to_string()].into_iter().map(|at| ps::key(&at)).collect();
        let (at_1, at_2) = register_reach(&["12/1".to_string()], &graph, &navigation);
        assert!(at_1.is_empty() && at_2.is_empty());
    }

    #[test]
    fn exact_and_near_title_pairs_are_counted_once_each() {
        let rows = vec![
            ("1/1".to_string(), "剩余控制权决定投资".to_string()),
            ("1/2".to_string(), "剩余控制权决定投资".to_string()),
            ("1/3".to_string(), "剩余控制权决定投资水平".to_string()),
            ("1/4".to_string(), "企业因为融资约束减少投资".to_string()),
            ("1/5".to_string(), "企业因为融资约束减少投资额".to_string()),
        ];
        let (exact, near, truncated) = title_pairs(&rows);
        assert_eq!(exact, 1, "one exact pair");
        // 1/3 extends 1/1 and 1/2, and 1/5 extends 1/4: three unordered near pairs, and the
        // exact pair is never counted again as a near one.
        assert_eq!(near, 3, "three near pairs, never the exact pair again");
        assert!(!truncated);
        // Case and spacing never split two spellings of one title.
        let rows = vec![("1/1".to_string(), "A  Model".to_string()), ("1/2".to_string(), "a model".to_string())];
        assert_eq!(title_pairs(&rows).0, 1);
    }

    fn sheet(raw: Value) -> Value {
        validate_sheet(&raw).expect("a valid sheet")
    }

    fn base_sheet() -> Value {
        json!({
            "schema_version": 1,
            "kind": "zt-task-sheet",
            "interval": { "from": "2026-10-01", "to": "2026-10-18" },
            "tasks": [
                { "id": "T1", "kind": "first-useful-card", "question": "融资约束如何影响投资？", "seconds": 40, "useful_address": "12/1|a", "entry_rank": 2 },
                { "id": "T2", "kind": "first-useful-card", "question": "剩余控制权是什么？", "seconds": 60, "useful_address": "13/2", "entry_rank": 4 },
                { "id": "T3", "kind": "kept-spread", "question": "多重均衡", "seconds": 100, "kept_spread": "均衡" },
                { "id": "T4", "kind": "distant-card", "question": "跨书对照", "offered": 3, "opened": 2, "laid": 1 }
            ],
            "removed_cards": ["12/9"],
            "human_replies": 2,
            "annotation": [
                { "address": "12/1|a", "local_reference_false_positive": true, "unnamed_referent_false_positive": false }
            ]
        })
    }

    #[test]
    fn the_fixed_sheet_is_validated_field_by_field() {
        assert!(sheet(base_sheet()).is_object());
        let mut wrong_kind = base_sheet();
        wrong_kind["tasks"][0]["kind"] = json!("first-useful-card-plus");
        assert!(validate_sheet(&wrong_kind).is_err());
        let mut unknown_field = base_sheet();
        unknown_field["tasks"][0]["started_at"] = json!("10:00");
        assert!(validate_sheet(&unknown_field).is_err());
        let mut unknown_top = base_sheet();
        unknown_top["telemetry"] = json!(true);
        assert!(validate_sheet(&unknown_top).is_err());
        let mut backwards = base_sheet();
        backwards["tasks"][3]["laid"] = json!(9);
        assert!(validate_sheet(&backwards).is_err());
        let mut twice = base_sheet();
        twice["tasks"][1]["id"] = json!("T1");
        assert!(validate_sheet(&twice).is_err());
        let mut negative = base_sheet();
        negative["tasks"][0]["seconds"] = json!(-1);
        assert!(validate_sheet(&negative).is_err());
        let mut rank0 = base_sheet();
        rank0["tasks"][0]["entry_rank"] = json!(0);
        assert!(validate_sheet(&rank0).is_err());
        let mut bad_annotation = base_sheet();
        bad_annotation["annotation"][0]["unnamed_referent_false_positive"] = json!("no");
        assert!(validate_sheet(&bad_annotation).is_err());
        let mut wrong_schema = base_sheet();
        wrong_schema["schema_version"] = json!(2);
        assert!(validate_sheet(&wrong_schema).is_err());
    }

    #[test]
    fn a_sheet_observation_is_checked_against_the_archive() {
        let dir = std::env::temp_dir().join(format!("zt-audit-measures-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sheet.json");
        crate::util::write_text(&path, &crate::ps::json_pretty(&base_sheet())).unwrap();
        let cards: HashSet<String> = ["12/1|a".to_string(), "13/2".to_string()].into_iter().collect();
        let out = task_sheet(path.to_str().unwrap(), &cards).unwrap();
        assert_eq!(out["present"], json!(true));
        assert_eq!(out["valid"], json!(true), "{}", out["faults"]);
        assert_eq!(out["removed_verified"], json!(1));
        // A useful address that no Card has, and a removed Card that still exists.
        let mut wrong = base_sheet();
        wrong["tasks"][0]["useful_address"] = json!("12/1|zz");
        wrong["removed_cards"] = json!(["12/1|a"]);
        crate::util::write_text(&path, &crate::ps::json_pretty(&wrong)).unwrap();
        let out = task_sheet(path.to_str().unwrap(), &cards).unwrap();
        assert_eq!(out["valid"], json!(false));
        let faults = ps_items(&out["faults"]);
        assert!(faults.iter().any(|fault| n(&fault["fault"]) == "useful_address"));
        assert!(faults.iter().any(|fault| n(&fault["fault"]) == "still_present"));
        assert_eq!(out["removed_verified"], json!(0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn medians_and_rates_never_fabricate_a_zero() {
        assert_eq!(median(&[]), Value::Null);
        assert_eq!(median(&[5]), json!(5));
        assert_eq!(median(&[10, 20, 30]), json!(20));
        assert_eq!(median(&[10, 20]), json!(15));
        assert_eq!(per_100(1, 0), Value::Null);
        assert_eq!(per_100(1, 4), json!(25));
    }

    #[test]
    fn a_comparison_delta_exists_only_when_the_scopes_match() {
        let matched = compare_row(Some(json!(10)), Some(json!(14)), true);
        assert_eq!(matched["delta"], json!(4));
        assert_eq!(matched["status"], json!("matched"));
        let mismatched = compare_row(Some(json!(900)), Some(json!(30)), false);
        assert_eq!(mismatched["delta"], Value::Null);
        assert_eq!(mismatched["status"], json!("scope_mismatch"));
        // A non-numeric or absent value is not comparable, never a zero delta.
        assert_eq!(compare_row(Some(json!("150 of 200")), Some(json!(3)), true)["status"], json!("not_comparable"));
        assert_eq!(compare_row(None, Some(json!(3)), true)["status"], json!("not_comparable"));
    }
}
