//! `zt-digest term-link`: the deterministic Term-link pass (design 13.1-13.7, S22 / #160 and
//! S23 / #163).
//!
//!   zt-digest term-link -Step plan|apply -Staging <folder>
//!                       [-CitationKey <key>] [-Occurrences first|all]
//!                       [-Cards <addresses>] [-Round <n>] [-Token <claim>] [-Apply]
//!
//! One agent Literature tree is read once, with its bodies: the book's Term list is read by role
//! through the archive's own computed Term structure, each table row is joined to its one valid
//! head and that head's auto-link choice, and every in-scope Card body is planned by byte offset.
//! A Card's same-book Term macros are normalized lexically (a stale, missing or retyped target
//! still reads as its label), a foreign book's macro is retained as authored, and malformed
//! Term-like markup refuses that Card rather than guessing. New links are matched by one
//! leftmost-longest literal scan over the link words and blockers in the book's table order, only
//! inside eligible prose runs.
//!
//! The plan is a pure function of its frozen inputs: `<Staging>/term-link/plan.json` and
//! `preview.md` carry no clock, so equal inputs produce equal bytes. Every Card freezes its
//! authored fingerprint (the archive's own receipt projection) and, when changed, its planned
//! output's fingerprint, so an apply never edits from a stale offset or base.
//!
//! `apply` revalidates the frozen plan against the archive (identity, book/list/head mapping,
//! policy contract, options, scope and each Card's authored guard), explicitly replans a Card whose
//! body changed into the next plan revision with its updated preview and diff, rechecks the
//! preservation equation, and writes at most one `edit_card` per changed Card through bounded
//! identified chunks with stable operation keys. It reports the committed, replayed, pending and
//! refused chunks, stops on the archive's typed `card_locked` (exit 2) or `card_check`/stale-base
//! refusal (exit 1) without rewriting prose, and writes no receipt and no revision when the plan
//! has no edit.

mod apply;
mod card;
mod matches;
mod spans;

use crate::args::Args;
use crate::calls;
use crate::canon;
use crate::organize::common::{self, pstr};
use crate::ps;
use crate::util;
use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use card::Counts;
use matches::Policy;

/// The declared steps, in command order (design 13.1).
pub const STEPS: [&str; 2] = ["plan", "apply"];

/// The work folder under `-Staging`.
const WORK: &str = "term-link";
pub(crate) const PLAN: &str = "plan.json";
pub(crate) const PREVIEW: &str = "preview.md";
/// The apply report's two artifacts.
pub(crate) const APPLY: &str = "apply.json";
pub(crate) const APPLY_MD: &str = "apply.md";

/// The syntax version the plan freezes (design 13.2; R0358). A changed matcher or protected-span
/// rule changes it, so an old plan is never replayed under a new parser.
pub const SYNTAX_VERSION: u8 = 1;

/// The plan artifact's own schema. An apply refuses a plan frozen by another schema instead of
/// reading fields it does not carry; the operator runs `-Step plan` again.
pub const PLAN_SCHEMA: u8 = 2;

/// The structure Card's authored role marker (the same first line the checker and the hub walk
/// read): a Side successor of the root that opens with it is navigation, not prose.
static STRUCTURE_MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^本卡是《.*》的结构卡").unwrap());

// --- the command surface -------------------------------------------------------------------------

/// One invocation's parsed parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Params {
    pub(crate) step: String,
    pub(crate) staging: String,
    pub(crate) citation_key: Option<String>,
    pub(crate) occurrences: Policy,
    /// The exact `-Cards` subset; empty means the whole eligible tree.
    pub(crate) cards: Vec<String>,
    /// The finalization correction round a `-Cards` pass serves (S24, #167; design 14.2); None means
    /// the run's one whole-book pass, and a subset without a round is a diagnostic pass that owns no
    /// ledger unit.
    pub(crate) round: Option<i64>,
    pub(crate) token: Option<i64>,
    pub(crate) apply: bool,
}

impl Params {
    pub(crate) fn work(&self) -> String {
        ps::join(&self.staging, WORK)
    }

    /// The ledger obligation this invocation serves: the whole book, one finalization round, or a
    /// diagnostic subset that completes nothing (R0429).
    pub(crate) fn pass(&self) -> Result<Option<crate::ledger::TermLinkPass>> {
        if let Some(round) = self.round {
            return crate::ledger::TermLinkPass::final_round(round, &self.cards).map(Some);
        }
        Ok((self.cards.is_empty()).then(crate::ledger::TermLinkPass::book))
    }
}

/// The declared parameters and switch of the command contract (R0352, S22-AC01).
const NAMES: [&str; 7] = ["Step", "Staging", "CitationKey", "Occurrences", "Cards", "Round", "Token"];

pub(crate) fn params(argv: &[String]) -> Result<Params> {
    let a = Args::parse(&util::bind_positional(argv, &NAMES)?, &NAMES, &["Apply"])?;
    let step = a.choice("Step", &STEPS)?;
    let staging = common::mandatory(&a, "Staging")?;
    if staging.trim().is_empty() {
        bail!("Cannot bind argument to parameter 'Staging' because it is an empty string.");
    }
    let citation_key = if a.has("CitationKey") {
        let key = a.required("CitationKey")?.trim().to_string();
        if key.is_empty() {
            bail!("-CitationKey names no book");
        }
        Some(key)
    } else {
        None
    };
    let occurrences = match a.str_or("Occurrences", "first").trim().to_ascii_lowercase().as_str() {
        "first" => Policy::First,
        "all" => Policy::All,
        other => bail!("-Occurrences must be first or all, not `{other}`"),
    };
    let cards = card_addresses(&a.str("Cards"))?;
    let token = if a.has("Token") {
        Some(
            a.required("Token")?
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .context("-Token must be a positive integer")?,
        )
    } else {
        None
    };
    let apply = a.flag("Apply");
    if (step == "apply") != apply {
        bail!("term-link apply requires -Apply; no other step accepts it");
    }
    if token.is_some() && step == "plan" {
        bail!("term-link plan is not a claimed step; remove -Token");
    }
    let round = if a.has("Round") {
        Some(
            a.required("Round")?
                .trim()
                .parse::<i64>()
                .ok()
                .filter(|value| *value > 0)
                .context("-Round must be a positive whole number")?,
        )
    } else {
        None
    };
    if round.is_some() && cards.is_empty() {
        bail!("-Round names a finalization correction round: it relinks the -Cards that round changed");
    }
    Ok(Params {
        step,
        staging,
        citation_key,
        occurrences,
        cards,
        round,
        token,
        apply,
    })
}

/// `-Cards a,b`: trimmed addresses, each once, in the order given.
fn card_addresses(text: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split(',') {
        let at = part.trim();
        if at.is_empty() {
            continue;
        }
        if out.iter().any(|seen| seen == at) {
            bail!("-Cards names `{at}` twice");
        }
        out.push(at.to_string());
    }
    Ok(out)
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let p = params(argv)?;
    match p.step.as_str() {
        "plan" => plan(&p),
        _ => apply::run(&p),
    }
}

// --- the archive reads ---------------------------------------------------------------------------

fn plan(p: &Params) -> Result<i32> {
    let staging = p.staging.as_str();
    let manifest = common::manifest(staging)?;
    let key = book_key(&manifest, p)?;
    let mut zt = crate::zt::start()?;
    // The tree is read once for its titles and once for the bodies of its members, and once more
    // for the computed Term structure. `get_tree` is deliberately not used: it parses every body
    // strictly and would refuse the whole tree on one malformed macro, while this pass must name
    // the Card that needs correction and plan the rest.
    let titles = zt.call("export_titles", json!({"zone": "literature"}))?;
    ensure_book_tree(&titles, &key)?;
    let addresses = tree_addresses(&titles, &key);
    let read = zt.cards(&addresses, "text")?;
    let terms = zt.call("get_terms", json!({"at": key}))?;
    let built = build_plan(&manifest, &titles, read.archive_revision, &read.cards, &terms, p)?;
    let mut built = built;
    let work = p.work();
    std::fs::create_dir_all(&work)
        .with_context(|| format!("cannot create the term-link work folder `{work}`"))?;
    util::write_atomic(&ps::join(&work, PLAN), &ps::json_pretty(&built.plan))?;
    util::write_atomic(&ps::join(&work, PREVIEW), &built.preview)?;
    // S24 (#167): in a prepared run the plan registers its pass's ledger unit. A whole-book pass is
    // `termlink/book@1`, a finalization round is `termlink/final@R`, and a diagnostic subset owns no
    // unit, so it can never complete the book's Term linking (R0429). The ledger stays the scheduler.
    let mut units = Value::Null;
    if std::path::Path::new(&ps::join(staging, "ledger.sqlite3")).is_file()
        && let Some(pass) = p.pass()?
    {
        let prepared = crate::ledger::Prepared::read(staging, &mut zt)?;
        units = crate::ledger::term_link_register(&prepared, &built.plan, &pass)?["units"].clone();
    }
    built.summary["unit"] = units["unit"].clone();
    built.summary["pass"] = units["pass"].clone();
    built.summary["round"] = units["round"].clone();
    for fault in &built.card_faults {
        eprintln!(
            "term-link: refused {} ({} at line {}): {}",
            fault["address"].as_str().unwrap_or_default(),
            fault["code"].as_str().unwrap_or_default(),
            fault["line"].as_u64().unwrap_or(0),
            fault["message"].as_str().unwrap_or_default()
        );
    }
    util::print_line(&canon::canonical_json(&built.summary))?;
    Ok(0)
}

/// One Literature tree in the agent Workspace, read as this book. The checks run before the Term
/// read so a Topic tree is refused with the scope's own sentence.
pub(crate) fn ensure_book_tree(titles: &Value, key: &str) -> Result<()> {
    if is_topic_tree(key) {
        bail!("`{key}` is a topic tree, not a Literature tree; the pass is Literature-scoped");
    }
    let rows = titles["titles"]
        .as_array()
        .context("the title read carries no titles")?;
    let Some(root) = rows.iter().find(|row| pstr(row, "address") == key) else {
        bail!("tree `{key}` does not exist");
    };
    if pstr(root, "tree") != key {
        bail!("the title read does not describe the book `{key}`");
    }
    if pstr(root, "workspace") != "agent" {
        bail!(
            "the book tree `{key}` is in the {} Workspace; the pass changes only the agent Workspace",
            pstr(root, "workspace")
        );
    }
    Ok(())
}

/// A Topic tree id is a Topic number; a Literature tree id is its Citation key.
fn is_topic_tree(key: &str) -> bool {
    key == "0"
        || (!key.is_empty()
            && key.bytes().all(|byte| byte.is_ascii_digit())
            && !key.starts_with('0'))
}

/// The addresses of one tree, in the natural address order the title read already uses.
pub(crate) fn tree_addresses(titles: &Value, key: &str) -> Vec<String> {
    titles["titles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| pstr(row, "tree") == key)
        .map(|row| pstr(row, "address"))
        .collect()
}

/// The Citation key the staging folder was prepared for; `-CitationKey` may only repeat it.
pub(crate) fn book_key(manifest: &Value, p: &Params) -> Result<String> {
    let key = pstr(manifest, "citation_key").trim().to_string();
    if key.is_empty() {
        bail!("manifest.json names no citation_key; term-link planning needs its book");
    }
    if let Some(named) = &p.citation_key
        && named != &key
    {
        bail!(
            "the staging folder is prepared for `{key}`; -CitationKey `{named}` is another book"
        );
    }
    Ok(key)
}

// --- the frozen plan -----------------------------------------------------------------------------

/// The artifacts one plan run produces.
#[derive(Debug)]
pub(crate) struct Built {
    pub(crate) plan: Value,
    pub(crate) preview: String,
    pub(crate) summary: Value,
    pub(crate) card_faults: Vec<Value>,
}

/// One Card's place in the pass: in scope, or excluded with the role that excludes it.
#[derive(Debug, Default)]
struct Scope {
    included: Vec<Value>,
    excluded: Vec<Value>,
    total: usize,
    unselected: usize,
}

/// The tree's cards minus the root, the Term list, the structure Card and the notation table
/// (R0354); with `-Cards`, exactly the named eligible subset. Every exclusion is by an existing
/// role marker, never by a title that happens to contain a common word (except the notation
/// table's established title, which is the archive's own role test for it).
fn scope_of(cards: &[Value], root: &str, list: &str, selected: &[String]) -> Result<Scope> {
    let mut scope = Scope {
        total: cards.len(),
        ..Scope::default()
    };
    let mut eligible: Vec<Value> = Vec::new();
    let mut excluded: BTreeMap<String, (&'static str, String)> = BTreeMap::new();
    for card in cards {
        let address = pstr(card, "address");
        let title = pstr(card, "title");
        let body = pstr(card, "body");
        let first_line = body.lines().next().unwrap_or_default().trim();
        // A named Term Card titled 符号表 defines that word; it is no notation table.
        let named_term = pstr(card, "type") == "term" && card["term"].is_string();
        let reason = if address == root {
            Some("root")
        } else if address == list {
            Some("term_list")
        } else if STRUCTURE_MARKER.is_match(first_line) {
            Some("structure")
        } else if is_notation_table(&title) && !named_term {
            Some("notation")
        } else {
            None
        };
        match reason {
            Some(reason) => {
                excluded.insert(address.clone(), (reason, title.clone()));
                scope.excluded.push(json!({
                    "address": address,
                    "title": title,
                    "reason": reason,
                }));
            }
            None => eligible.push(card.clone()),
        }
    }
    if selected.is_empty() {
        scope.included = eligible;
        return Ok(scope);
    }
    let tree_addresses: BTreeSet<String> =
        cards.iter().map(|card| pstr(card, "address")).collect();
    let wanted: BTreeSet<&String> = selected.iter().collect();
    for at in selected {
        let Some((reason, _)) = excluded.get(at) else {
            if !tree_addresses.contains(at) {
                bail!(
                    "-Cards names `{at}`, which is not a Card of this Literature tree; the subset must be exact"
                );
            }
            continue;
        };
        bail!("-Cards names `{at}`, which the pass excludes as `{reason}`; remove it");
    }
    scope.unselected = eligible.len();
    scope.included = eligible
        .into_iter()
        .filter(|card| wanted.contains(&pstr(card, "address")))
        .collect();
    scope.unselected -= scope.included.len();
    Ok(scope)
}

/// The Book notation table, by the archive's own title role (`符号表…` or `notation…`).
fn is_notation_table(title: &str) -> bool {
    let title = title.trim();
    title.starts_with("符号表") || title.to_lowercase().starts_with("notation")
}

/// Build the plan, its preview and its summary from the title, body and Term-structure reads. This function writes
/// nothing, so a unit test can freeze a synthetic tree.
pub(crate) fn build_plan(
    manifest: &Value,
    titles: &Value,
    body_revision: u64,
    cards: &[Value],
    terms: &Value,
    p: &Params,
) -> Result<Built> {
    let key = book_key(manifest, p)?;
    // --- the read's own consistency and scope ------------------------------------------------
    ensure_book_tree(titles, &key)?;
    let titles_revision = titles["archive_revision"]
        .as_u64()
        .context("the title read carries no archive revision")?;
    if body_revision != titles_revision {
        bail!(
            "the archive changed while reading the bodies (revision {titles_revision} then {body_revision}); run -Step plan again"
        );
    }
    let terms_revision = terms["archive_revision"]
        .as_u64()
        .context("the Term-structure read carries no archive revision")?;
    if titles_revision != terms_revision {
        bail!(
            "the archive changed while planning (revision {titles_revision} then {terms_revision}); run -Step plan again"
        );
    }
    if pstr(terms, "tree") != key {
        bail!("the Term-structure read is not about the book `{key}`");
    }
    let list_address = pstr(terms, "list");
    if list_address.is_empty() {
        bail!("the Term-structure read names no list");
    }
    let identity = &terms["archive_identity"];
    if identity["id"].as_str().is_none_or(str::is_empty)
        || identity["path"].as_str().is_none_or(str::is_empty)
    {
        bail!("the Term-structure read carries no archive identity; the plan cannot bind it");
    }
    if let Some(contract) = manifest.get("run_contract").filter(|value| value.is_object())
        && let Some(expected) = contract["archive_identity"]["id"].as_str()
        && !expected.is_empty()
        && Some(expected) != identity["id"].as_str()
    {
        bail!(
            "the staging run belongs to archive `{expected}` but the service reads `{}`; point the service at the run's archive",
            identity["id"].as_str().unwrap_or_default()
        );
    }
    let by_address: BTreeMap<String, &Value> = cards
        .iter()
        .map(|card| (pstr(card, "address"), card))
        .collect();
    let list_card = by_address.get(&list_address).with_context(|| {
        format!("the Term list `{list_address}` is not in the tree read; run -Step plan again")
    })?;
    let list_body = pstr(list_card, "body");
    let scope = scope_of(cards, &key, &list_address, &p.cards)?;

    // --- the vocabulary: rows, their one head, and the head's auto-link choice ----------------
    if let Some(duplicates) = terms["row_duplicates"]
        .as_array()
        .filter(|duplicates| !duplicates.is_empty())
    {
        let named: Vec<String> = duplicates
            .iter()
            .map(|group| {
                let spellings = group["spellings"]
                    .as_array()
                    .map(|spellings| {
                        spellings
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                format!(
                    "{spellings} (rows {})",
                    group["rows"]
                        .as_array()
                        .map(|rows| rows
                            .iter()
                            .filter_map(Value::as_u64)
                            .map(|row| row.to_string())
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_default()
                )
            })
            .collect();
        bail!(
            "the Term list repeats a term: {}; one row per term is required before planning",
            named.join("; ")
        );
    }
    let rows = terms["rows"].as_array().context("the Term structure carries no rows")?;
    let mut term_rows: Vec<matches::Term> = Vec::new();
    let mut heads_json: Vec<Value> = Vec::new();
    let mut own_terms: BTreeMap<String, String> = BTreeMap::new();
    for row in rows {
        let term = pstr(row, "term");
        let heads: Vec<String> = row["heads"]
            .as_array()
            .map(|heads| heads.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        let auto_link = heads.iter().all(|head| {
            by_address
                .get(head)
                .map(|card| !auto_link_off(&pstr(card, "body")))
                .unwrap_or(false)
        });
        heads_json.push(json!({
            "row": row["row"],
            "term": term,
            "head": (heads.len() == 1).then(|| heads[0].clone()),
            "heads": heads,
            "auto_link": auto_link,
            "published": row["published"],
        }));
        for head in &heads {
            own_terms.insert(head.clone(), term.clone());
        }
        term_rows.push(matches::Term {
            row: row["row"].as_u64().unwrap_or(0) as usize,
            term,
            avoid: row["avoid"]
                .as_array()
                .map(|avoid| avoid.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default(),
            heads,
            auto_link,
        });
    }
    for part in terms["parts"].as_array().into_iter().flatten() {
        own_terms.insert(pstr(part, "address"), pstr(part, "term"));
    }
    for unfiled in terms["unfiled"].as_array().into_iter().flatten() {
        own_terms.insert(pstr(unfiled, "address"), pstr(unfiled, "term"));
    }
    let row_names: BTreeSet<String> = term_rows.iter().map(|term| term.term.clone()).collect();
    let mut unrowed: Vec<String> = own_terms
        .values()
        .filter(|name| !row_names.contains(*name))
        .cloned()
        .collect();
    unrowed.sort();
    unrowed.dedup();
    let vocabulary = matches::Vocabulary::build(&term_rows, &unrowed);
    // The whole vocabulary derivation, not only the row→head and auto-link choices: a new named
    // Term Card that publishes no row, a retyped part, or a changed avoid word changes what the
    // pass may link, so an apply revalidates this hash beside the two mapping hashes.
    let vocabulary_hash = canon::hash_value(&json!({
        "terms": term_rows.iter().map(|term| json!({
            "row": term.row,
            "term": term.term,
            "avoid": term.avoid,
            "heads": term.heads,
            "auto_link": term.auto_link,
        })).collect::<Vec<_>>(),
        "own": own_terms,
        "unrowed": unrowed,
    }));

    // --- every in-scope Card, in natural address order ---------------------------------------
    let mut card_values: Vec<Value> = Vec::new();
    for card in &scope.included {
        let address = pstr(card, "address");
        let title = pstr(card, "title");
        let body = pstr(card, "body");
        let planned = {
            let own_term = own_terms.get(&address).map(String::as_str);
            let profile = spans::Profile {
                own_term,
                term_card: pstr(card, "type") == "term",
            };
            card::plan_card(&body, &profile, &vocabulary, p.occurrences, &key)
        };
        let fault = planned.fault.as_ref().map(|fault| {
            let (line, line_text) = match fault.at {
                Some(at) => card::line_of(&body, at),
                None => (0, String::new()),
            };
            json!({
                "address": address,
                "title": title,
                "code": fault.code,
                "at": fault.at,
                "line": line,
                "line_text": line_text,
                "message": fault.message,
            })
        });
        let mut value = json!({
            "address": address,
            "title": title,
            "type": pstr(card, "type"),
            "term_role": card.get("term_role").cloned().unwrap_or(Value::Null),
            "body_sha256": canon::sha256_hex(body.as_bytes()),
            // The archive's own authored fingerprint projection: the base guard every edit of this
            // plan carries (R0406), and, for a changed Card, the planned output the apply reads
            // back after the commit. It excludes the generated Reverse/Term-use text, which the
            // pass itself changes; hashing the whole stored text would make the pass its own
            // concurrent edit.
            "authored_fingerprint": calls::authored_fingerprint(card),
            "new_fingerprint": Value::Null,
            "changed": planned.changed(),
            "edits": planned.edits.iter().map(|edit| json!({
                "kind": edit.kind.id(),
                "start": edit.start,
                "end": edit.end,
                "term": edit.term,
                "target": edit.target,
                "label": edit.label,
                "before": edit.before,
                "after": edit.after,
            })).collect::<Vec<_>>(),
            "kept": planned.kept.iter().map(|kept| json!({
                "start": kept.start, "end": kept.end, "term": kept.term, "target": kept.target,
            })).collect::<Vec<_>>(),
            "retained": planned.retained.iter().map(|retained| json!({
                "start": retained.start, "end": retained.end,
                "label": retained.label, "target": retained.target, "reason": retained.reason,
            })).collect::<Vec<_>>(),
            "skips": planned.skips.iter().map(|skip| json!({
                "start": skip.start, "end": skip.end, "text": skip.text, "reason": skip.reason,
            })).collect::<Vec<_>>(),
            "fault": fault,
        });
        if planned.changed() {
            let mut output = card.clone();
            output["body"] = json!(planned.new_body);
            value["new_fingerprint"] = json!(calls::authored_fingerprint(&output));
            value["body"] = json!(body);
            value["new_body"] = json!(planned.new_body);
            value["new_body_sha256"] = json!(canon::sha256_hex(planned.new_body.as_bytes()));
        }
        card_values.push(value);
    }
    let counts = counts_of(&card_values);
    let cards_hash = canon::hash_value(&json!(card_values
        .iter()
        .map(|card| json!({
            "address": card["address"],
            "authored_fingerprint": card["authored_fingerprint"],
            "body_sha256": card["body_sha256"],
        }))
        .collect::<Vec<_>>()));
    let list_rows: Vec<Value> = rows
        .iter()
        .map(|row| {
            json!({
                "row": row["row"],
                "term": row["term"],
                "avoid": row["avoid"],
                "meaning": row["meaning"],
                "defined": row["defined"],
                "published": row["published"],
            })
        })
        .collect();
    let book_title = {
        let title = pstr(manifest, "book_title");
        if title.trim().is_empty() { key.clone() } else { title }
    };
    let frozen = json!({
        "schema_version": PLAN_SCHEMA,
        "kind": "term_link_plan",
        "syntax_version": SYNTAX_VERSION,
        // The plan's own revision: 1 when the plan step freezes it, one more each time an apply
        // explicitly replans a changed Card into the next revision (R0405).
        "revision": 1,
        "replaces": Value::Null,
        "replanned": [],
        "book": {
            "citation_key": key,
            "title": book_title,
            "variant": pstr(&manifest["run_contract"], "variant"),
        },
        "archive": {
            "id": identity["id"],
            "path": identity["path"],
            "revision": titles_revision,
        },
        "versions": {
            "compiler": env!("CARGO_PKG_VERSION"),
            "checker": zt_check::VERSION,
            "rule": zt_check::contract::RULE_VERSION,
        },
        "options": {
            "occurrences": p.occurrences.id(),
            "cards": (!p.cards.is_empty()).then(|| p.cards.clone()),
        },
        "scope": {
            "tree": key,
            "workspace": "agent",
            "zone": "literature",
            "cards": scope.total,
            "eligible": scope.included.len() + scope.unselected,
            "included": scope.included.len(),
            "unselected": scope.unselected,
            "changed": counts["cards_changed"],
            "refused": counts["cards_refused"],
            "excluded": scope.excluded,
        },
        "list": {
            "address": list_address,
            "title": pstr(list_card, "title"),
            "body_sha256": canon::sha256_hex(list_body.as_bytes()),
            "row_count": term_rows.len(),
            "rows": list_rows,
        },
        "heads": heads_json,
        "fingerprints": {
            "list": canon::sha256_hex(list_body.as_bytes()),
            "mapping": vocabulary.mapping_hash,
            "autolink": vocabulary.autolink_hash,
            "vocabulary": vocabulary_hash,
            "cards": cards_hash,
        },
        "term_structure": {
            "faults": terms["faults"],
            "row_duplicates": terms["row_duplicates"],
            "coverage": terms["coverage"],
        },
        "cards": card_values,
        "counts": counts,
    });
    let plan_hash = canon::hash_value(&frozen);
    let mut plan = frozen;
    plan["plan_hash"] = json!(plan_hash);
    let preview = preview_of(&plan);
    let cards = plan["cards"].as_array().cloned().unwrap_or_default();
    let counts = &plan["counts"];
    let summary = json!({
        "schema_version": 1,
        "ok": true,
        "step": "plan",
        "book": key,
        "archive_revision": titles_revision,
        "plan_hash": plan_hash,
        "plan_revision": 1,
        "cards_scanned": counts["cards_scanned"],
        "cards_changed": counts["cards_changed"],
        "cards_refused": counts["cards_refused"],
        "links_added": counts["links_added"],
        "links_repaired": counts["links_repaired"],
        "links_kept": counts["links_kept"],
        "links_removed": counts["links_removed"],
        "retained_foreign": counts["retained_foreign"],
        "retained_other": counts["retained_other"],
        "skipped": counts["skipped"],
        "complete": counts["cards_refused"] == 0,
        "plan": ps::join(&p.work(), PLAN),
        "preview": ps::join(&p.work(), PREVIEW),
        "work": p.work(),
    });
    let card_faults: Vec<Value> = cards
        .iter()
        .filter(|card| card["fault"].is_object())
        .map(|card| card["fault"].clone())
        .collect();
    Ok(Built {
        plan,
        preview,
        summary,
        card_faults,
    })
}

/// The plan's report counts: the Card scope facts plus the link, retained and skip counts, read
/// back from the frozen Card entries. A fresh plan and a merged apply revision therefore count
/// their Cards the same way.
pub(crate) fn counts_of(cards: &[Value]) -> Value {
    let mut counts = Counts::of_cards(cards).report();
    counts["cards_scanned"] = json!(cards.len());
    counts["cards_changed"] = json!(cards
        .iter()
        .filter(|card| card["changed"] == true)
        .count());
    counts["cards_refused"] = json!(cards
        .iter()
        .filter(|card| card["fault"].is_object())
        .count());
    // A revision can record Cards whose planned output an earlier run already committed; they are
    // no longer edits this plan owes, but they are still part of what the pass changed.
    counts["cards_applied"] = json!(cards
        .iter()
        .filter(|card| card["applied"] == true)
        .count());
    counts
}

/// Whether a Term head's authored metadata disables automatic linking.
fn auto_link_off(body: &str) -> bool {
    zt_check::term::parse_template(body)
        .line(zt_check::term::AUTO_LINK_LABEL)
        .is_some_and(|line| line.value.trim() == zt_check::term::AUTO_LINK_OFF)
}

/// The deterministic preview: totals, every changed line, every substitution, and the faults.
pub(crate) fn preview_of(plan: &Value) -> String {
    let mut out = String::new();
    out.push_str("# Term-link preview\n\n");
    out.push_str(&format!(
        "Book: {} — {}\n\n",
        plan["book"]["citation_key"].as_str().unwrap_or_default(),
        plan["book"]["title"].as_str().unwrap_or_default()
    ));
    out.push_str(&format!(
        "Plan revision {}{}\n\n",
        plan["revision"].as_u64().unwrap_or(1),
        match plan["replaces"].as_str() {
            Some(replaces) => format!(" (replaces {})", &replaces[..12.min(replaces.len())]),
            None => String::new(),
        }
    ));
    out.push_str(&format!(
        "Policy: {} occurrence · syntax {} · compiler {} · checker {} · rule {}\n\n",
        plan["options"]["occurrences"].as_str().unwrap_or_default(),
        plan["syntax_version"],
        plan["versions"]["compiler"].as_str().unwrap_or_default(),
        plan["versions"]["checker"].as_str().unwrap_or_default(),
        plan["versions"]["rule"].as_str().unwrap_or_default()
    ));
    out.push_str(&format!(
        "Archive revision {} · archive {}\n\n",
        plan["archive"]["revision"],
        plan["archive"]["id"].as_str().unwrap_or_default()
    ));
    let counts = &plan["counts"];
    out.push_str(&format!(
        "{} Cards scanned, {} changed, {} refused. {} eligible in scope ({} unselected, {} excluded by role).\n\n",
        counts["cards_scanned"],
        counts["cards_changed"],
        counts["cards_refused"],
        plan["scope"]["eligible"],
        plan["scope"]["unselected"],
        plan["scope"]["excluded"].as_array().map_or(0, Vec::len)
    ));
    out.push_str(&format!(
        "Links: {} added, {} repaired, {} kept, {} removed to their label; {} foreign, {} other target retained.\n\n",
        counts["links_added"],
        counts["links_repaired"],
        counts["links_kept"],
        counts["links_removed"],
        counts["retained_foreign"],
        counts["retained_other"]
    ));
    let skipped: Vec<String> = counts["skipped"]
        .as_object()
        .map(|map| {
            map.iter()
                .filter(|(_, count)| count.as_u64().unwrap_or(0) > 0)
                .map(|(reason, count)| format!("{reason} {count}"))
                .collect()
        })
        .unwrap_or_default();
    out.push_str(&format!(
        "Skipped: {}.\n\n",
        if skipped.is_empty() {
            "none".to_string()
        } else {
            skipped.join(" · ")
        }
    ));

    let excluded = plan["scope"]["excluded"].as_array().cloned().unwrap_or_default();
    if !excluded.is_empty() {
        out.push_str("## Excluded by role\n\n");
        for card in &excluded {
            out.push_str(&format!(
                "- `{}` {} ({})\n",
                card["address"].as_str().unwrap_or_default(),
                card["title"].as_str().unwrap_or_default(),
                card["reason"].as_str().unwrap_or_default()
            ));
        }
        out.push('\n');
    }

    // An apply that replanned a changed Card freezes the next revision and publishes this preview
    // again, so the reader sees which Cards moved out of the plan they reviewed (R0405).
    let replanned = plan["replanned"].as_array().cloned().unwrap_or_default();
    if !replanned.is_empty() {
        out.push_str("## Replanned Cards\n\n");
        for card in &replanned {
            out.push_str(&format!(
                "- `{}` {} ({})\n",
                card["address"].as_str().unwrap_or_default(),
                card["title"].as_str().unwrap_or_default(),
                card["reason"].as_str().unwrap_or_default()
            ));
        }
        out.push('\n');
    }

    let mut any_changed = false;
    for card in plan["cards"].as_array().into_iter().flatten() {
        let edits = card["edits"].as_array().cloned().unwrap_or_default();
        if edits.is_empty() {
            continue;
        }
        any_changed = true;
        out.push_str(&format!(
            "## {} {}\n\n",
            card["address"].as_str().unwrap_or_default(),
            card["title"].as_str().unwrap_or_default()
        ));
        for edit in &edits {
            out.push_str(&format!(
                "- {} `{}` → `{}` (bytes {}–{})\n",
                edit["kind"].as_str().unwrap_or_default(),
                edit["before"].as_str().unwrap_or_default(),
                edit["after"].as_str().unwrap_or_default(),
                edit["start"],
                edit["end"]
            ));
        }
        out.push('\n');
        // The changed prose lines: a substitution never adds or removes a newline, so the old and
        // new bodies line up one to one and every differing line is a real change to review.
        let old_body = card["body"].as_str().unwrap_or_default();
        let new_body = card["new_body"].as_str().unwrap_or_default();
        for (index, (old, new)) in old_body.lines().zip(new_body.lines()).enumerate() {
            if old != new {
                out.push_str(&format!("line {}:\n", index + 1));
                out.push_str(&format!("  - {old}\n"));
                out.push_str(&format!("  + {new}\n\n"));
            }
        }
        let skipped: Vec<String> = card["skips"]
            .as_array()
            .map(|skips| {
                let mut reasons: BTreeMap<&str, usize> = BTreeMap::new();
                for skip in skips {
                    *reasons.entry(skip["reason"].as_str().unwrap_or_default()).or_default() += 1;
                }
                reasons
                    .into_iter()
                    .map(|(reason, count)| format!("{reason} {count}"))
                    .collect()
            })
            .unwrap_or_default();
        if !skipped.is_empty() {
            out.push_str(&format!("Skips: {}.\n\n", skipped.join(" · ")));
        }
    }
    if !any_changed {
        out.push_str("No Card changes.\n\n");
    }
    let faults = card_faults_of(plan);
    if !faults.is_empty() {
        out.push_str("## Refused Cards\n\n");
        for fault in faults {
            out.push_str(&format!(
                "- `{}` {} line {}: {} ({})\n",
                fault["address"].as_str().unwrap_or_default(),
                fault["title"].as_str().unwrap_or_default(),
                fault["line"],
                fault["message"].as_str().unwrap_or_default(),
                fault["code"].as_str().unwrap_or_default()
            ));
        }
        out.push('\n');
    }
    out
}

/// The refused Cards recorded among the plan's cards.
fn card_faults_of(plan: &Value) -> Vec<Value> {
    plan["cards"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|card| card["fault"].is_object())
        .map(|card| card["fault"].clone())
        .collect()
}



#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|value| value.to_string()).collect()
    }

    fn parsed(list: &[&str]) -> Result<Params> {
        super::params(&args(list))
    }

    fn plan_params(step: &str, occurrences: Policy, cards: &[&str]) -> Params {
        Params {
            step: step.to_string(),
            staging: "D:\\staging".to_string(),
            citation_key: None,
            occurrences,
            cards: cards.iter().map(|at| at.to_string()).collect(),
            round: None,
            token: None,
            apply: step == "apply",
        }
    }

    #[test]
    fn the_command_contract_takes_every_declared_option_and_refuses_misuse() {
        let p = parsed(&["-Step", "plan", "-Staging", "D:\\s"]).unwrap();
        assert_eq!(p.occurrences, Policy::First);
        assert!(p.cards.is_empty());
        let p = parsed(&[
            "-Step", "plan", "-Staging", "D:\\s", "-Occurrences", "ALL", "-Cards", "K/1, K/2",
            "-CitationKey", "K",
        ])
        .unwrap();
        assert_eq!(p.occurrences, Policy::All);
        assert_eq!(p.cards, ["K/1", "K/2"]);
        assert_eq!(p.citation_key.as_deref(), Some("K"));
        let p = parsed(&["-Step", "apply", "-Staging", "D:\\s", "-Apply", "-Token", "7"]).unwrap();
        assert!(p.apply);
        assert_eq!(p.token, Some(7));
        for bad in [
            vec!["-Step", "apply", "-Staging", "D:\\s"],
            vec!["-Step", "plan", "-Staging", "D:\\s", "-Apply"],
            vec!["-Step", "plan", "-Staging", "D:\\s", "-Token", "1"],
            vec!["-Step", "apply", "-Staging", "D:\\s", "-Apply", "-Token", "0"],
            vec!["-Step", "plan", "-Staging", "D:\\s", "-Occurrences", "some"],
            vec!["-Step", "plan", "-Staging", "D:\\s", "-Cards", "K/1,K/1"],
            vec!["-Step", "plan", "-Staging", ""],
            vec!["-Step", "plan", "-Staging", "D:\\s", "-CitationKey", " "],
            vec!["-Step", "plan", "-Staging", "D:\\s", "-Nope", "1"],
        ] {
            assert!(parsed(&bad).is_err(), "{bad:?} was accepted");
        }
    }

    #[test]
    fn topic_ids_are_numbers_and_books_are_citation_keys() {
        for topic in ["0", "1", "42"] {
            assert!(is_topic_tree(topic), "{topic}");
        }
        for book in ["K", "testbook2026", "01", "1a", ""] {
            assert!(!is_topic_tree(book), "{book}");
        }
    }

    /// One synthetic read row: metadata plus the authored body and stored text `get_cards text`
    /// returns.
    fn card(address: &str, title: &str, body: &str) -> Value {
        json!({
            "address": address,
            "title": title,
            "type": "idea",
            "body": body,
            "text": format!("# {title}\n\n{body}\n\n<!-- zt:reverse-links -->\n"),
        })
    }

    fn term_card(address: &str, term: &str, role: &str, body: &str) -> Value {
        let mut value = card(address, if term.is_empty() { "术语表" } else { term }, body);
        value["type"] = json!("term");
        value["term"] = if term.is_empty() { Value::Null } else { json!(term) };
        value["term_role"] = json!(role);
        value
    }

    fn head_body(term: &str, section: &str, auto_off: bool) -> String {
        let mut body = format!("术语：{term}\n类别：概念\n首次定义：§1\n");
        if auto_off {
            body.push_str("自动链接：否\n自动链接理由：这是过于一般的词。\n");
        }
        body.push_str(&format!("\n## 释义\n\n{section}\n\n来源：[[K]] §1"));
        body
    }

    fn row(n: u64, term: &str, avoid: &[&str], heads: &[&str]) -> Value {
        json!({
            "row": n,
            "term": term,
            "avoid": avoid,
            "meaning": format!("{term} 的意思"),
            "defined": "§1",
            "heads": heads,
            "published": !heads.is_empty(),
        })
    }

    fn head(address: &str, term: &str) -> Value {
        json!({
            "address": address, "term": term, "title": term, "role": "card", "parts": [], "uses": 0,
        })
    }

    /// The title read's rows for one tree: the metadata a bulk title pass answers.
    fn titles_from(cards: &[Value], workspace: &str) -> Value {
        let rows: Vec<Value> = cards
            .iter()
            .map(|card| {
                let address = pstr(card, "address");
                let tree = address
                    .split_once('/')
                    .map_or(address.clone(), |(tree, _)| tree.to_string());
                json!({
                    "address": address,
                    "tree": tree,
                    "workspace": workspace,
                    "type": card["type"],
                    "facet": Value::Null,
                    "title": card["title"],
                    "restatement": false,
                })
            })
            .collect();
        json!({
            "count": rows.len(),
            "titles": rows,
            "complete": true,
            "archive_revision": 5,
        })
    }

    fn terms(rows: Vec<Value>, heads: Vec<Value>) -> Value {
        let n = rows.len();
        let heads_count = heads.len();
        json!({
            "list": "K/3",
            "title": "术语表",
            "workspace": "agent",
            "tree": "K",
            "rows": rows,
            "row_duplicates": [],
            "heads": heads,
            "parts": [],
            "unfiled": [],
            "uses": [],
            "faults": [],
            "coverage": {
                "rows": n, "raw_rows": n, "published": n,
                "missing": 0, "heads": heads_count, "parts": 0, "unfiled": 0, "uses": 0
            },
            "complete": true,
            "archive_revision": 5,
            "archive_identity": {"id": "a".repeat(32), "path": "\\\\?\\D:\\zt"},
        })
    }

    fn manifest() -> Value {
        json!({"citation_key": "K", "book_title": "书"})
    }

    /// The synthetic book: a root, one structure Card, a Term list, a notation table, two Idea
    /// Cards, three Term heads, and one Card with a foreign Term use.
    fn book() -> (Value, Vec<Value>, Value) {
        let cards = vec![
            card("K", "书", "《书》的索引：全书论证见 [[K/1|a]]。"),
            card("K/1", "净生产函数决定产出", "净生产函数决定产出。"),
            card(
                "K/1|a",
                "书的论证结构",
                "本卡是《书》的结构卡：按阅读顺序列出全书的论证。",
            ),
            card("K/2", "生产函数影响均衡", "生产函数影响均衡。均衡是稳定的。"),
            term_card(
                "K/3",
                "",
                "list",
                "术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | — | 稳定状态 | §1 |\n",
            ),
            term_card("K/4", "均衡", "card", &head_body("均衡", "均衡是稳定的状态。", false)),
            term_card(
                "K/5",
                "生产函数",
                "card",
                &head_body("生产函数", "生产函数是投入与产出的关系。", true),
            ),
            card("K/6", "符号表：《书》的 2 个符号", "| 符号 | 含义 | 定义于 |\n| --- | --- | --- |\n| $K$ | 资本 | §1 |\n"),
            card("K/7", "外来术语", "见 [[均衡@bookB/2]] 与 生产函数。"),
            term_card("K/8", "净生产函数", "card", &head_body("净生产函数", "净生产函数是扣除折旧后的产出。", false)),
        ];
        let rows = vec![
            row(1, "均衡", &[], &["K/4"]),
            row(2, "生产函数", &[], &["K/5"]),
            row(3, "净生产函数", &[], &["K/8"]),
        ];
        let heads = vec![
            head("K/4", "均衡"),
            head("K/5", "生产函数"),
            head("K/8", "净生产函数"),
        ];
        let titles = titles_from(&cards, "agent");
        (titles, cards, terms(rows, heads))
    }

    fn planned(titles: &Value, cards: &[Value], terms: &Value, p: &Params) -> Result<Built> {
        build_plan(&manifest(), titles, 5, cards, terms, p)
    }

    #[test]
    fn the_scope_excludes_only_identified_roles_and_an_exact_subset() {
        let (_, cards, terms) = book();
        let list = pstr(&terms, "list");
        let scope = scope_of(&cards, "K", &list, &[]).unwrap();
        let included: Vec<&str> = scope
            .included
            .iter()
            .map(|card| card["address"].as_str().unwrap())
            .collect();
        assert_eq!(included, ["K/1", "K/2", "K/4", "K/5", "K/7", "K/8"]);
        let excluded: Vec<(&str, &str)> = scope
            .excluded
            .iter()
            .map(|card| (card["address"].as_str().unwrap(), card["reason"].as_str().unwrap()))
            .collect();
        assert_eq!(
            excluded,
            [
                ("K", "root"),
                ("K/1|a", "structure"),
                ("K/3", "term_list"),
                ("K/6", "notation")
            ]
        );
        // A title that merely looks like navigation is not excluded: only the authored role is.
        let lookalike = card("K/9", "结构卡与索引", "均衡与生产函数。");
        let scope = scope_of(&[lookalike], "K", &list, &[]).unwrap();
        assert_eq!(scope.included.len(), 1);
        // The subset is exact, keeps tree order, and refuses a Card the pass excludes.
        let scope = scope_of(&cards, "K", &list, &["K/2".to_string(), "K/4".to_string()]).unwrap();
        let included: Vec<&str> = scope
            .included
            .iter()
            .map(|card| card["address"].as_str().unwrap())
            .collect();
        assert_eq!(included, ["K/2", "K/4"]);
        assert_eq!(scope.unselected, 4);
        for bad in ["K", "K/3", "K/1|a", "K/6", "NOPE"] {
            assert!(
                scope_of(&cards, "K", &list, &[bad.to_string()]).is_err(),
                "-Cards {bad} was accepted"
            );
        }
    }

    #[test]
    fn the_plan_freezes_the_scope_vocabulary_edits_and_counts() {
        let (titles, cards, terms) = book();
        let p = plan_params("plan", Policy::First, &[]);
        let built = planned(&titles, &cards, &terms, &p).unwrap();
        let plan = &built.plan;
        assert_eq!(plan["kind"], "term_link_plan");
        assert_eq!(plan["syntax_version"], SYNTAX_VERSION);
        assert_eq!(plan["book"]["citation_key"], "K");
        assert_eq!(plan["book"]["title"], "书");
        assert_eq!(plan["archive"]["revision"], 5);
        assert_eq!(plan["options"]["occurrences"], "first");
        assert_eq!(plan["scope"]["cards"], 10);
        assert_eq!(plan["scope"]["included"], 6);
        assert_eq!(plan["scope"]["changed"], 2);
        assert_eq!(plan["scope"]["refused"], 0);
        assert_eq!(plan["scope"]["excluded"].as_array().unwrap().len(), 4);
        assert_eq!(plan["list"]["address"], "K/3");
        assert_eq!(plan["list"]["row_count"], 3);
        assert_eq!(plan["heads"][1]["term"], "生产函数");
        assert_eq!(plan["heads"][1]["auto_link"], false);
        assert_eq!(plan["heads"][1]["head"], "K/5");
        for key in ["list", "mapping", "autolink", "vocabulary", "cards"] {
            let hash = plan["fingerprints"][key].as_str().unwrap();
            assert_eq!(hash.len(), 64, "{key}");
        }
        assert_eq!(plan["plan_hash"].as_str().unwrap().len(), 64);
        let counted = &plan["counts"];
        assert_eq!(counted["cards_scanned"], 6);
        assert_eq!(counted["cards_changed"], 2);
        assert_eq!(counted["links_added"], 2);
        assert_eq!(counted["links_removed"], 0);
        assert_eq!(counted["retained_foreign"], 1);
        assert_eq!(counted["skipped"]["auto_link_off"], 2);
        assert_eq!(counted["skipped"]["own_term"], 2);
        assert_eq!(counted["skipped"]["metadata"], 2);
        assert_eq!(counted["skipped"]["repeat"], 1);
        let planned_cards = plan["cards"].as_array().unwrap();
        let changed: Vec<&Value> = planned_cards
            .iter()
            .filter(|card| card["changed"] == true)
            .collect();
        assert_eq!(changed.len(), 2);
        for card in &changed {
            assert!(card["body"].is_string());
            assert!(card["new_body"].is_string());
            assert!(card["new_body_sha256"].is_string());
        }
        let first = planned_cards.iter().find(|card| card["address"] == "K/1").unwrap();
        assert_eq!(first["new_body"], "[[净生产函数@K/8]]决定产出。");
        let second = planned_cards.iter().find(|card| card["address"] == "K/2").unwrap();
        assert_eq!(second["new_body"], "生产函数影响[[均衡@K/4]]。均衡是稳定的。");
    }

    #[test]
    fn the_plan_freezes_each_cards_authored_guards_and_a_revision() {
        let (titles, cards, terms) = book();
        let p = plan_params("plan", Policy::First, &[]);
        let built = planned(&titles, &cards, &terms, &p).unwrap();
        let plan = &built.plan;
        assert_eq!(plan["schema_version"], PLAN_SCHEMA);
        assert_eq!(plan["revision"], 1);
        assert!(plan["replaces"].is_null());
        assert_eq!(plan["replanned"], json!([]));
        let read = |at: &str| cards.iter().find(|card| card["address"] == at).unwrap().clone();
        for at in ["K/1", "K/2", "K/7"] {
            let entry = plan["cards"]
                .as_array()
                .unwrap()
                .iter()
                .find(|card| card["address"] == at)
                .unwrap();
            assert_eq!(
                entry["authored_fingerprint"],
                crate::calls::authored_fingerprint(&read(at)),
                "{at}"
            );
        }
        // A changed Card freezes the projection of its planned output: the archived receipt's
        // after_fingerprint, which the apply reads back.
        let first = plan["cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|card| card["address"] == "K/1")
            .unwrap();
        let mut output = read("K/1");
        output["body"] = json!("[[净生产函数@K/8]]决定产出。");
        assert_eq!(first["new_fingerprint"], crate::calls::authored_fingerprint(&output));
        assert_eq!(first["new_fingerprint"].as_str().unwrap().len(), 64);
        // A Card the plan does not edit has no output fingerprint to claim.
        let unchanged = plan["cards"]
            .as_array()
            .unwrap()
            .iter()
            .find(|card| card["address"] == "K/7")
            .unwrap();
        assert!(unchanged["new_fingerprint"].is_null());
        assert_eq!(built.summary["plan_revision"], 1);
    }

    #[test]
    fn the_counts_come_from_the_frozen_cards_once() {
        let (titles, cards, terms) = book();
        let p = plan_params("plan", Policy::First, &[]);
        let built = planned(&titles, &cards, &terms, &p).unwrap();
        let counts = &built.plan["counts"];
        // A merged plan counts its Cards again, so this reconstruction is the one source.
        assert_eq!(
            counts,
            &super::counts_of(built.plan["cards"].as_array().unwrap())
        );
        assert_eq!(counts["cards_scanned"], 6);
        assert_eq!(counts["cards_changed"], 2);
        assert_eq!(counts["cards_refused"], 0);
        assert_eq!(counts["links_added"], 2);
        assert_eq!(counts["links_kept"], 0);
        assert_eq!(counts["retained_foreign"], 1);
        assert_eq!(counts["skipped"]["auto_link_off"], 2);
    }

    #[test]
    fn the_option_scope_and_occurrence_policy_change_the_frozen_edits() {
        let (titles, cards, terms) = book();
        let mut p = plan_params("plan", Policy::All, &["K/2"]);
        let built = planned(&titles, &cards, &terms, &p).unwrap();
        assert_eq!(built.plan["scope"]["included"], 1);
        assert_eq!(built.plan["scope"]["unselected"], 5);
        assert_eq!(built.plan["options"]["cards"], json!(["K/2"]));
        let card = &built.plan["cards"][0];
        assert_eq!(card["address"], "K/2");
        assert_eq!(card["edits"].as_array().unwrap().len(), 2);
        assert_eq!(built.plan["options"]["occurrences"], "all");
        let again = planned(&titles, &cards, &terms, &p).unwrap();
        assert_eq!(
            canon::canonical_json(&built.plan),
            canon::canonical_json(&again.plan)
        );
        assert_eq!(built.preview, again.preview);
        p.occurrences = Policy::First;
        let first = planned(&titles, &cards, &terms, &p).unwrap();
        assert_ne!(first.plan["plan_hash"], built.plan["plan_hash"]);
        assert_eq!(first.plan["cards"][0]["edits"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_malformed_card_refuses_that_card_and_leaves_the_plan_incomplete() {
        let (_, mut cards, terms) = book();
        cards.push(card("K/9", "格式错", "见 [[均衡@K/4@x]]。"));
        let titles = titles_from(&cards, "agent");
        let p = plan_params("plan", Policy::First, &[]);
        let built = planned(&titles, &cards, &terms, &p).unwrap();
        assert_eq!(built.plan["counts"]["cards_refused"], 1);
        assert_eq!(built.summary["complete"], false);
        assert_eq!(built.card_faults.len(), 1);
        assert_eq!(built.card_faults[0]["address"], "K/9");
        assert_eq!(built.card_faults[0]["code"], "malformed_term_markup");
        assert_eq!(built.card_faults[0]["line"], 1);
        assert!(built.preview.contains("Refused Cards"));
        assert!(built.preview.contains("malformed_term_markup"));
    }

    #[test]
    fn the_preview_shows_the_totals_every_changed_line_and_the_skips() {
        let (titles, cards, terms) = book();
        let p = plan_params("plan", Policy::First, &[]);
        let built = planned(&titles, &cards, &terms, &p).unwrap();
        let preview = &built.preview;
        assert!(preview.starts_with("# Term-link preview\n"));
        assert!(preview.contains("Book: K — 书"));
        assert!(preview.contains("Policy: first occurrence"));
        assert!(preview.contains("Archive revision 5"));
        assert!(preview.contains("6 Cards scanned, 2 changed, 0 refused"));
        assert!(preview.contains("Links: 2 added, 0 repaired, 0 kept, 0 removed"));
        assert!(preview.contains("## K/1 净生产函数决定产出"));
        assert!(preview.contains("line 1:\n  - 净生产函数决定产出。\n  + [[净生产函数@K/8]]决定产出。"));
        assert!(preview.contains("- link `净生产函数` → `[[净生产函数@K/8]]`"));
        assert!(preview.contains("Skips:"));
        assert!(preview.contains("Excluded by role"));
        assert!(preview.contains("`K/3` 术语表 (term_list)"));
        assert!(!preview.contains("No Card changes"));
    }

    #[test]
    fn the_plan_refuses_another_book_a_topic_tree_a_moving_archive_and_another_archive() {
        let (titles, cards, terms) = book();
        let p = plan_params("plan", Policy::First, &[]);
        assert!(planned(&titles, &cards, &terms, &p).is_ok());
        // -CitationKey, or a manifest key that is not the tree's book.
        let mut wrong = plan_params("plan", Policy::First, &[]);
        wrong.citation_key = Some("Other".to_string());
        assert!(planned(&titles, &cards, &terms, &wrong).is_err());
        // A human Workspace tree, a Topic tree, and a tree the title read does not describe.
        let human = titles_from(&cards, "human");
        assert!(planned(&human, &cards, &terms, &p).is_err());
        let topic_cards = vec![card("0/0", "主题一", "一个主题。")];
        let topic_titles = titles_from(&topic_cards, "agent");
        let mut topic_manifest = manifest();
        topic_manifest["citation_key"] = json!("0");
        assert!(
            build_plan(&topic_manifest, &topic_titles, 5, &topic_cards, &terms, &p)
                .is_err()
        );
        let mut missing_root = titles.clone();
        missing_root["titles"] = json!(titles["titles"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["address"] != "K")
            .cloned()
            .collect::<Vec<_>>());
        assert!(planned(&missing_root, &cards, &terms, &p).is_err());
        // A Term structure of another book, a moving archive, and a body read on another revision.
        let mut broken = terms.clone();
        broken["tree"] = json!("Other");
        assert!(planned(&titles, &cards, &broken, &p).is_err());
        let mut moving = titles.clone();
        moving["archive_revision"] = json!(6);
        assert!(planned(&moving, &cards, &terms, &p).is_err());
        assert!(
            build_plan(&manifest(), &titles, 6, &cards, &terms, &p).is_err(),
            "a body read from another revision is refused"
        );
        // A run prepared against another archive identity.
        let mut other = manifest();
        other["run_contract"] =
            json!({"archive_identity": {"id": "b".repeat(32), "path": "\\\\?\\D:\\other"}});
        assert!(build_plan(&other, &titles, 5, &cards, &terms, &p).is_err());
        let mut without = terms.clone();
        without["archive_identity"] = Value::Null;
        assert!(planned(&titles, &cards, &without, &p).is_err());
    }

    #[test]
    fn a_named_term_titled_like_the_notation_table_is_no_notation_table() {
        let (_, cards, terms) = book();
        let mut cards = cards;
        cards.push(term_card(
            "K/9",
            "符号表",
            "card",
            &head_body("符号表", "符号表是符号的清单。", false),
        ));
        let scope = scope_of(&cards, "K", &pstr(&terms, "list"), &[]).unwrap();
        assert!(
            scope.included.iter().any(|card| card["address"] == "K/9"),
            "a Term head that defines 符号表 is prose, not the notation table: {:?}",
            scope.excluded
        );
        // The established notation table is still excluded.
        assert!(scope.excluded.iter().any(|card| card["reason"] == "notation"));
    }

    #[test]
    fn duplicate_rows_stop_the_plan_before_anything_is_frozen() {
        let (titles, cards, mut terms) = book();
        terms["row_duplicates"] = json!([{
            "term": "均衡",
            "spellings": ["均衡", "均衡"],
            "rows": [1, 2],
        }]);
        let p = plan_params("plan", Policy::First, &[]);
        let error = build_plan(&manifest(), &titles, 5, &cards, &terms, &p).unwrap_err();
        assert!(
            error.to_string().contains("repeats a term"),
            "{error:#}"
        );
    }
}
