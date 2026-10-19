//! `zt-digest finalize`: the declared finite finalization branch (R299, R300; #86).
//!
//! The run already published one read-only audit revision. Finalization is the finite
//! sequence `audit-plan → at most one compiler correction cycle → refresh affected
//! downstream publications → final read-only audit`:
//!
//! - `plan` publishes the scheduling audit revision when it is still open, then derives one
//!   immutable plan from it: every fault with its disposition (`fix`, `conflict`, `blocked`,
//!   `stale`, `advisory`), the single permitted correction cycle with its exact Card list,
//!   the refresh layers the changed Cards may affect, and the branch (`no_fix`, `fix`,
//!   `conflict`, `unresolved`). A recorded cycle makes the plan immutable, so the branch can
//!   never be re-entered.
//! - `scope` records the cycle's exact changed-Card/touched-tree manifest once, by comparing
//!   the audited Card fingerprints with the current archive. One cycle per plan: a second
//!   scope is refused instead of looping.
//! - `refresh` publishes and completes one affected aggregate publication at a new revision
//!   whose declared scope is that manifest. The same manifest reuses its revision; a changed
//!   one derives the next.
//! - `final` runs the read-only audit again (or verifies a given fresh report), publishes and
//!   completes the final audit revision, and writes the final report: the new graph revision,
//!   every residual hard/source/infrastructure fault, and `partial` instead of the scheduling
//!   audit whenever a hard fault remains.
//!
//! Nothing here writes Card prose; every write is a ledger revision completed through the
//! same verified publication boundary the run already uses.

use crate::args::Args;
use crate::audit;
use crate::canon;
use crate::ledger::{self, Prepared};
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub struct Params {
    pub step: String,
    pub out: String,
    pub report: String,
    pub fix_work: String,
    pub layer: String,
    pub changes: String,
    pub coverage: String,
    pub scope: String,
    pub decisions: String,
    pub applied: String,
    pub skeleton: String,
}

/// The advisory rules zt-check reports apart from the hard ones: they never block a run.
const ADVISORY: &[&str] = &["I1", "I6", "E5", "L3"];
/// The Categories the one bounded correction cycle may rewrite, as the Card compiler
/// declares them: everything else belongs to a done Model build, a Hub, a structure Card or
/// the term table, and correcting it would reopen work this branch must not reopen.
const WRITABLE: &[&str] = &["idea", "event", "finding", "anchor"];
/// The Card layer rules whose fault is a missing or wrong source line.
const SOURCE_RULES: &[&str] = &["I3", "E1", "E2"];
/// The rule zt-check uses for a note the Organize stage left unapplied.
const NOTE_RULE: &str = "N1";

/// The refresh layers in their declared dependency order (R299). A changed Card affects a
/// layer only through the change kinds it consumes: structure for Organize, the skeleton and
/// the hubs, links for the Link check, Bridge and the mathematics theory, figure references
/// for Figures, and every change for the final audit.
/// `(layer, change kinds it consumes, artifacts, coverage, variant, derived by this branch)`.
/// The last flag is the honest scope of the finite branch: the Link check re-derives from the
/// one correction cycle, and the final audit re-reads the archive. Every other affected layer
/// generates its own content (Organize's Opus decisions, the skeleton's job, the hub plan, the
/// Bridge judges, the theory summary), so its own stage owns that refresh; the changed-Card
/// manifest stays named as that stage's input and the plan says so.
const REFRESH: &[(&str, &[&str], &[&str], &str, &str, bool)] = &[
    ("link-check", &["any"], &["report"], "none", "", true),
    ("theory", &["links"], &["applied"], "derived", "math", false),
    ("organize", &["structure"], &["decisions", "applied"], "derived", "", false),
    ("skeleton", &["structure"], &["skeleton"], "derived", "", false),
    ("figures", &["figures"], &["report"], "required", "", false),
    ("hubs", &["structure"], &["report"], "none", "", false),
    ("bridge", &["links"], &["report"], "required", "", false),
    ("audit", &["any"], &["report"], "none", "", true),
];

/// The authored projection a finalization plan compares: the title, body and facet the
/// read-only audit itself retained. Both sides of every comparison use it, so a plan can
/// never mistake a projection difference for a Card that really changed.
fn card_fingerprint(title: &str, body: &str, facet: &str) -> String {
    let authored = json!({"title": title, "body": body, "facet": facet});
    canon::sha256_hex(canon::canonical(&authored).to_string().as_bytes())
}

fn must(value: &Value, name: &str) -> Result<String> {
    value[name]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .with_context(|| format!("incomplete value: {name}"))
}

/// A rule in its natural check order (R, I, E, F, L, N), then by number.
fn rule_key(rule: &str) -> (u8, i64, String) {
    let letter = rule.chars().next().unwrap_or_default().to_ascii_uppercase();
    let order = match letter {
        'R' => 0,
        'I' => 1,
        'E' => 2,
        'F' => 3,
        'L' => 4,
        'N' => 5,
        _ => 6,
    };
    let number = rule
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse::<i64>()
        .unwrap_or(0);
    (order, number, rule.to_string())
}

fn tally(list: &[String]) -> Value {
    let mut counts: BTreeMap<String, i64> = BTreeMap::new();
    for item in list {
        *counts.entry(item.clone()).or_default() += 1;
    }
    Value::Object(counts.into_iter().map(|(key, value)| (key, json!(value))).collect())
}

fn unit_state(conn: &Connection, unit: &str) -> Result<String> {
    Ok(conn
        .query_row("SELECT state FROM units WHERE id=?1", [unit], |row| {
            row.get::<_, String>(0)
        })
        .optional()?
        .unwrap_or_default())
}

/// The report artifact a publication unit retained, when it completed.
fn unit_report(conn: &Connection, unit: &str) -> Result<Option<PathBuf>> {
    let row: Option<String> = conn
        .query_row(
            "SELECT locator FROM outputs WHERE unit_id=?1 AND output_key='report'",
            [unit],
            |row| row.get(0),
        )
        .optional()?;
    Ok(row.map(PathBuf::from))
}

/// The layers the prepared run's recorded target enables.
fn prepared_layers(p: &Prepared) -> Result<BTreeSet<String>> {
    let target = ledger::target_of(&p.contract)?;
    Ok(ps::ps_items(&target["enabled"])
        .into_iter()
        .map(|layer| ps::s(&layer))
        .collect())
}

/// The audited view of the book: its faults and the Cards they describe.
struct AuditView {
    dir: PathBuf,
    report: Value,
}

impl AuditView {
    fn open(report: &Path) -> Result<Self> {
        Ok(Self {
            dir: report.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from(".")),
            report: util::read_json(report)?,
        })
    }

    fn graph_revision(&self) -> u64 {
        self.report["archive_revision"].as_u64().unwrap_or(0)
    }

    fn book(&self, key: &str) -> Value {
        ps::ps_items(&self.report["books"])
            .into_iter()
            .find(|book| ps::eq(&ps::field(&book, "key"), key))
            .unwrap_or(Value::Null)
    }

    /// The exact fault lines: `<address> <rule> <text>`.
    fn faults(&self) -> Result<Vec<Value>> {
        let path = self.dir.join("audit-faults.txt");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let mut faults = Vec::new();
        for line in util::read_text(&path)?.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, char::is_whitespace);
            let address = parts.next().unwrap_or_default();
            let rule = parts.next().unwrap_or_default();
            let text = parts.next().unwrap_or_default().trim();
            if address.is_empty() || rule.is_empty() {
                continue;
            }
            faults.push(json!({"address":address,"rule":rule,"text":text,"line":line}));
        }
        Ok(faults)
    }

    /// The audited Card state by folded address: authored fingerprint, Category and tree.
    fn cards(&self) -> Result<BTreeMap<String, Value>> {
        let path = self.dir.join("audit-cards.json");
        let mut cards = BTreeMap::new();
        if !path.exists() {
            return Ok(cards);
        }
        for card in ps::ps_items(&util::read_json(&path)?) {
            let address = ps::field(&card, "address");
            if address.is_empty() {
                continue;
            }
            let title = ps::field(&card, "title");
            let body = ps::field(&card, "body");
            let facet = ps::field(&card, "facet");
            let raw: Vec<String> = card["links"]
                .as_array()
                .map(|links| links.iter().filter_map(Value::as_str).map(str::to_string).collect())
                .unwrap_or_default();
            cards.insert(
                ps::key(&address),
                json!({
                    "address": address,
                    "category": ps::field(&card, "category"),
                    "tree": ps::field(&card, "tree"),
                    "fingerprint": card_fingerprint(&title, &body, &facet),
                    "title": title,
                    "links": ps::select_unique(raw),
                    "figures": audit::figure_labels(&body),
                }),
            );
        }
        Ok(cards)
    }
}

fn plan_path(out: &Path) -> PathBuf {
    out.join("finalize").join("audit-plan.json")
}

fn changes_path(out: &Path) -> PathBuf {
    out.join("finalize").join("changes.json")
}

fn cycle_path(out: &Path) -> PathBuf {
    out.join("finalize").join("fix-cycle.json")
}

fn final_report_path(out: &Path) -> PathBuf {
    out.join("finalize").join("final-report.json")
}

fn card_list_path(out: &Path) -> PathBuf {
    out.join("finalize").join("fix-cards.txt")
}

fn declared_artifacts(layer: &str) -> Result<&'static [&'static str]> {
    ledger::publication_artifacts(layer)
        .with_context(|| format!("`{layer}` is not an aggregate publication layer"))
}

/// One fault's disposition. `fix` is the one cycle's work, `conflict` a Card whose audited
/// base is stale, `blocked` a fault the finite branch may not rewrite, `stale` a Card that no
/// longer exists, and `advisory` a rule that never blocks the run.
fn disposition(
    fault: &Value,
    audited: &BTreeMap<String, Value>,
    current: &BTreeMap<String, Value>,
    faults_by_address: &BTreeMap<String, Vec<String>>,
) -> Value {
    let address = ps::field(&fault, "address");
    let rule = ps::field(&fault, "rule");
    let key = ps::key(&address);
    let mut out = json!({
        "address": address,
        "rule": rule,
        "text": ps::field(&fault, "text"),
        "line": fault["line"],
    });
    if ADVISORY.contains(&rule.as_str()) || rule.eq_ignore_ascii_case(NOTE_RULE) {
        out["outcome"] = json!("advisory");
        out["reason"] = json!("an advisory rule: it never blocks the run, and the bounded correction does not rewrite for it");
        return out;
    }
    let Some(before) = audited.get(&key) else {
        out["outcome"] = json!("blocked");
        out["reason"] = json!("the audit recorded no Card body for this fault, so no bounded rewrite can be scoped from it");
        return out;
    };
    let Some(now) = current.get(&key) else {
        out["outcome"] = json!("stale");
        out["reason"] = json!("the audited Card no longer exists at the current revision; the final audit decides the residue");
        return out;
    };
    if !WRITABLE.contains(&ps::field(before, "category").as_str()) {
        out["outcome"] = json!("blocked");
        out["reason"] = json!(format!(
            "the Card is a {} Card: the bounded correction may not reopen a done build, a hub, a structure Card or the term table",
            ps::field(before, "category")
        ));
        return out;
    }
    if before["fingerprint"] != now["fingerprint"] {
        out["outcome"] = json!("conflict");
        out["reason"] = json!("the Card changed after the audit it scheduled, so a rewrite from that stale base is refused");
        return out;
    }
    out["outcome"] = json!("fix");
    out["reason"] = json!("");
    out["card"] = now["address"].clone();
    out["tree"] = now["tree"].clone();
    let same: Vec<String> = faults_by_address
        .get(&key)
        .map(|rules| {
            rules
                .iter()
                .filter(|rule| !ADVISORY.contains(&rule.as_str()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    out["rules"] = json!(same);
    out
}

/// The plan's branch: nothing to correct, one correction cycle, a stale base, or only faults
/// the finite branch may not correct.
fn branch_of(dispositions: &[Value]) -> &'static str {
    let count = |outcome: &str| {
        dispositions
            .iter()
            .filter(|entry| entry["outcome"] == json!(outcome))
            .count()
    };
    if count("fix") > 0 {
        "fix"
    } else if count("conflict") > 0 {
        "conflict"
    } else if count("blocked") > 0 || count("stale") > 0 {
        "unresolved"
    } else {
        "no_fix"
    }
}

/// The layer a refresh entry applies to, or the explicit reason it does not.
fn refresh_plan(changes: &Value, enabled: &BTreeSet<String>, variant: &str) -> Vec<Value> {
    let kinds: BTreeSet<String> = ps::ps_items(&changes["kinds"])
        .into_iter()
        .map(|kind| ps::s(&kind))
        .collect();
    let changed = ps::ps_items(&changes["changed"]).len();
    let mut plan = Vec::new();
    for (layer, when, artifacts, coverage, only, by_branch) in REFRESH {
        let mut entry = json!({
            "layer": layer,
            "artifacts": artifacts,
            "coverage": coverage,
            "when": when,
            "branch_refreshable": by_branch,
        });
        if !only.is_empty() && *only != variant {
            entry["applicable"] = json!(false);
            entry["reason"] = json!(format!("the {only} variant owns this layer"));
        } else if layer == &"audit" {
            // The final audit always runs: it is the branch's last step, never a refresh of
            // the report that scheduled it.
            entry["applicable"] = json!(true);
            entry["reason"] = json!("");
        } else if !enabled.contains(*layer) {
            entry["applicable"] = json!(false);
            entry["reason"] = json!("the prepared run excluded this layer");
        } else if changed == 0 {
            entry["applicable"] = json!(false);
            entry["reason"] = json!("no changed Card affects it: there is nothing to refresh");
        } else {
            let affected = when.contains(&"any") || when.iter().any(|kind| kinds.contains(*kind));
            if affected && !by_branch {
                entry["applicable"] = json!(false);
                entry["stage_owned"] = json!(true);
                entry["reason"] = json!(format!(
                    "its own stage generates this content ({}), so that stage owns the refresh; the changed-Card manifest is its input and is recorded here",
                    when.join(", ")
                ));
            } else {
                entry["applicable"] = json!(affected);
                entry["reason"] = json!(if affected {
                    String::new()
                } else {
                    format!("the changed Cards affect none of its inputs ({})", when.join(", "))
                });
            }
        }
        plan.push(entry);
    }
    plan
}

// --- plan -------------------------------------------------------------------------------------

fn run_plan(p: &Prepared, zt: &mut Zt, a: &Params) -> Result<Value> {
    if cycle_path(&p.out).exists() {
        bail!(
            "the audit plan's one permitted compiler correction cycle is already recorded ({}); the plan is immutable (R299)",
            cycle_path(&p.out).display()
        );
    }
    let key = must(&p.contract, "citation_key")?;
    let variant = must(&p.contract, "variant")?;
    let given = if a.report.trim().is_empty() {
        p.out.join("audit.json")
    } else {
        PathBuf::from(&a.report)
    };
    let report_path = std::fs::canonicalize(&given)
        .with_context(|| format!("the scheduling audit report {} does not exist", given.display()))?;
    let given_report = util::read_json(&report_path)?;
    if given_report["complete"] != true {
        bail!("the scheduling audit report is incomplete; finalization cannot plan from it");
    }
    // The scheduling audit becomes one checked revision, so the plan reads a published
    // revision instead of a loose file.
    let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
    let Some((unit, _)) = ledger::required_publication(&conn, "audit")? else {
        bail!("this prepared run registered no audit publication");
    };
    let state = unit_state(&conn, &unit)?;
    if state != "done" {
        let mut artifacts = BTreeMap::new();
        artifacts.insert("report".to_string(), report_path.clone());
        complete_publication(p, zt, "audit", Value::Null, &artifacts, &[])?;
    }
    let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
    let (unit, _) = ledger::required_publication(&conn, "audit")?
        .context("the audit publication disappeared")?;
    let retained = unit_report(&conn, &unit)?
        .context("the scheduling audit revision retained no report")?;
    let audit = AuditView::open(&retained)?;
    if audit.graph_revision() == 0 {
        bail!("the scheduling audit revision retained no graph revision");
    }
    let faults = audit.faults()?;
    let audited = audit.cards()?;
    // The current archive state of exactly the faulting Cards, at one revision.
    let addresses: Vec<String> = faults
        .iter()
        .map(|fault| ps::field(&fault, "address"))
        .filter(|address| !address.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut current = BTreeMap::new();
    if !addresses.is_empty() {
        let read = zt.cards(&addresses, "text")?;
        for card in &read.cards {
            let address = ps::field(&card, "address");
            current.insert(
                ps::key(&address),
                json!({
                    "address": address,
                    "fingerprint": card_fingerprint(&ps::field(&card, "title"), &ps::field(&card, "body"), &ps::field(&card, "facet")),
                    "tree": ps::field(&card, "tree"),
                }),
            );
        }
    }
    let mut faults_by_address: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for fault in &faults {
        faults_by_address
            .entry(ps::key(&ps::field(&fault, "address")))
            .or_default()
            .push(ps::field(&fault, "rule"));
    }
    let dispositions: Vec<Value> = faults
        .iter()
        .map(|fault| disposition(fault, &audited, &current, &faults_by_address))
        .collect();
    let branch = branch_of(&dispositions);
    let mut cards: Vec<String> = dispositions
        .iter()
        .filter(|entry| entry["outcome"] == json!("fix"))
        .map(|entry| ps::field(&entry, "address"))
        .collect();
    cards.sort();
    cards.dedup();
    let mut rules: Vec<String> = dispositions
        .iter()
        .filter(|entry| entry["outcome"] == json!("fix"))
        .map(|entry| ps::field(&entry, "rule"))
        .collect();
    rules.sort_by_key(|rule| rule_key(rule));
    rules.dedup();
    if cards.is_empty() {
        if card_list_path(&p.out).exists() {
            std::fs::remove_file(card_list_path(&p.out))?;
        }
    } else {
        util::write_text(card_list_path(&p.out), &format!("{}\n", cards.join(&ps::NL)))?;
    }
    let enabled = prepared_layers(p)?;
    let empty = json!({"changed":[],"touched_trees":[],"kinds":[]});
    let refresh = refresh_plan(&empty, &enabled, &variant);
    let book = audit.book(&key);
    let plan = json!({
        "schema_version": 1,
        "kind": "audit_plan",
        "step": "plan",
        "run_id": ledger::meta(&conn, "run_id")?,
        "citation_key": key,
        "variant": variant,
        "graph_revision": audit.graph_revision(),
        "audit": {
            "unit": unit,
            "revision": ledger::publication_unit(&unit)?.2,
            "graph_revision": audit.graph_revision(),
            "report": {"path": retained.to_string_lossy(), "sha256": canon::sha256_hex(&std::fs::read(&retained)?)},
            "book": {
                "cards": book["cards"],
                "style_faults": book["style"]["faults"],
                "layer_faults": book["layer"]["faults"],
                "layer_advisory": book["layer"]["advisory"],
                "orphans": book["orphans"]["count"],
            },
            "hubs_missing": audit.report["workspace"]["hubs"]["missing"],
            "broken_links": audit.report["workspace"]["broken_links"],
        },
        "faults": {
            "total": faults.len(),
            "hard": dispositions.iter().filter(|entry| entry["outcome"] != json!("advisory")).count(),
            "advisory": dispositions.iter().filter(|entry| entry["outcome"] == json!("advisory")).count(),
            "by_rule": tally(&faults.iter().map(|fault| ps::field(&fault, "rule")).collect::<Vec<_>>()),
            "list": dispositions,
        },
        "counts": {
            "fix": dispositions.iter().filter(|entry| entry["outcome"] == json!("fix")).count(),
            "conflict": dispositions.iter().filter(|entry| entry["outcome"] == json!("conflict")).count(),
            "blocked": dispositions.iter().filter(|entry| entry["outcome"] == json!("blocked")).count(),
            "stale": dispositions.iter().filter(|entry| entry["outcome"] == json!("stale")).count(),
            "advisory": dispositions.iter().filter(|entry| entry["outcome"] == json!("advisory")).count(),
        },
        "branch": branch,
        "fix_cycle": {
            "permitted": 1,
            "used": 0,
            "rules": rules,
            "cards": cards,
            "card_list": if cards.is_empty() { Value::Null } else { json!(card_list_path(&p.out).to_string_lossy()) },
        },
        "refresh": refresh,
    });
    util::write_text(plan_path(&p.out), &ps::json_pretty(&plan))?;
    util::write_text(p.out.join("finalize").join("audit-plan.md"), &plan_markdown(&plan))?;
    Ok(plan)
}

fn plan_markdown(plan: &Value) -> String {
    let mut text = format!(
        "# 收尾审计计划 / Finalization audit plan\n\nRead-only audit revision `{}` at graph revision **{}**.\n\nBranch: **{}**. Faults: {} ({} hard, {} advisory).\n\n",
        ps::field(&plan["audit"], "unit"),
        plan["graph_revision"],
        ps::field(plan, "branch"),
        plan["faults"]["total"],
        plan["faults"]["hard"],
        plan["faults"]["advisory"],
    );
    text.push_str("## One permitted correction cycle\n\n");
    let cards = ps::ps_items(&plan["fix_cycle"]["cards"]);
    if cards.is_empty() {
        text.push_str("No correctable Card: the one cycle does not run.\n\n");
    } else {
        let rules: Vec<String> = ps::ps_items(&plan["fix_cycle"]["rules"]).iter().map(ps::s).collect();
        text.push_str(&format!("{} Cards with rules `{}`.\n\n", cards.len(), rules.join(",")));
        for card in &cards {
            text.push_str(&format!("- `{}`\n", ps::s(card)));
        }
        text.push('\n');
    }
    text.push_str("## Fault dispositions\n\n| address | rule | outcome | reason |\n|---|---|---|---|\n");
    for entry in ps::ps_items(&plan["faults"]["list"]) {
        text.push_str(&format!(
            "| `{}` | {} | {} | {} |\n",
            ps::field(&entry, "address"),
            ps::field(&entry, "rule"),
            ps::field(&entry, "outcome"),
            ps::field(&entry, "reason").replace('|', "/"),
        ));
    }
    text.push_str("\n## Refresh\n\n| layer | applicable | reason |\n|---|---|---|\n");
    for entry in ps::ps_items(&plan["refresh"]) {
        text.push_str(&format!(
            "| {} | {} | {} |\n",
            ps::field(&entry, "layer"),
            entry["applicable"],
            ps::field(&entry, "reason").replace('|', "/"),
        ));
    }
    text
}

// --- scope -------------------------------------------------------------------------------------

/// The Card kinds a change rewrites, from the audited and current authored state.
fn change_kinds(before: Option<&Value>, after: &Value) -> Vec<String> {
    let Some(before) = before else {
        // A split Card did not exist at the audit: every consumer of its text, Links and
        // figure references must refresh.
        return vec![
            "figures".to_string(),
            "links".to_string(),
            "new".to_string(),
            "structure".to_string(),
        ];
    };
    let mut kinds = BTreeSet::new();
    if before["title"] != after["title"] || before["body"] != after["body"] {
        kinds.insert("structure".to_string());
    }
    if before["links"] != after["links"] {
        kinds.insert("links".to_string());
    }
    if before["figures"] != after["figures"] {
        kinds.insert("figures".to_string());
    }
    kinds.into_iter().collect()
}

fn run_scope(p: &Prepared, zt: &mut Zt, _a: &Params) -> Result<Value> {
    let plan_file = plan_path(&p.out);
    if !plan_file.exists() {
        bail!("no audit plan: run `zt-digest finalize -Step plan` first");
    }
    let plan = util::read_json(&plan_file)?;
    if cycle_path(&p.out).exists() {
        bail!(
            "the audit plan's one permitted compiler correction cycle is already recorded ({}); a second scope is refused (R299)",
            cycle_path(&p.out).display()
        );
    }
    let branch = must(&plan, "branch")?;
    if branch != "fix" {
        bail!(
            "the audit plan's branch is `{branch}`: no correction cycle runs, so there is no changed-Card manifest to record"
        );
    }
    let work = if _a.fix_work.trim().is_empty() {
        p.out.join("finalize").join("fix")
    } else {
        PathBuf::from(&_a.fix_work)
    };
    let state_file = work.join("state.json");
    let state = util::read_json(&state_file)
        .with_context(|| format!("the correction cycle's state {} does not exist", state_file.display()))?;
    let report_file = work.join("report.json");
    let report = util::read_json(&report_file).ok();
    let mut addresses: BTreeSet<String> = BTreeSet::new();
    if let Some(applied) = state["applied"].as_object() {
        addresses.extend(applied.keys().cloned());
    }
    for split in ps::ps_items(&state["splits"]) {
        let address = ps::field(&split, "address");
        if !address.is_empty() {
            addresses.insert(address);
        }
    }
    for link in ps::ps_items(&state["orphan_links"]) {
        let address = ps::field(&link, "from");
        if !address.is_empty() {
            addresses.insert(address);
        }
    }
    let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
    let (unit, _) = ledger::required_publication(&conn, "audit")?
        .context("the scheduling audit revision is missing")?;
    let retained = unit_report(&conn, &unit)?
        .context("the scheduling audit revision retained no report")?;
    let audit = AuditView::open(&retained)?;
    let audited = audit.cards()?;
    let list: Vec<String> = addresses.into_iter().collect();
    let read = zt.cards(&list, "text")?;
    let mut changed = Vec::new();
    let mut trees = BTreeSet::new();
    let mut kinds = BTreeSet::new();
    for card in &read.cards {
        let address = ps::field(&card, "address");
        let body = ps::field(&card, "body");
        let raw_links: Vec<String> = card["links"]
            .as_array()
            .map(|links| links.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default();
        let links: Vec<String> = ps::select_unique(raw_links);
        let now = json!({
            "address": address,
            "title": ps::field(&card, "title"),
            "body": body,
            "links": links,
            "figures": audit::figure_labels(&body),
            "fingerprint": card_fingerprint(&ps::field(&card, "title"), &body, &ps::field(&card, "facet")),
            "tree": ps::field(&card, "tree"),
        });
        let before = audited.get(&ps::key(&address));
        let kinds_here = change_kinds(before, &now);
        if kinds_here.is_empty() {
            continue;
        }
        for kind in &kinds_here {
            kinds.insert(kind.clone());
        }
        if let Some(before) = before {
            trees.insert(ps::field(before, "tree"));
        }
        trees.insert(ps::field(&card, "tree"));
        changed.push(json!({
            "address": address,
            "kinds": kinds_here,
            "category": before.map(|before| ps::field(before, "category")).unwrap_or_else(|| "new".into()),
            "tree": ps::field(&card, "tree"),
            "fingerprint_before": before.map(|before| before["fingerprint"].clone()).unwrap_or(Value::Null),
            "fingerprint_after": now["fingerprint"],
        }));
    }
    let trees: Vec<String> = trees.into_iter().filter(|tree| !tree.is_empty()).collect();
    let kinds: Vec<String> = kinds.into_iter().collect();
    let change_hash = canon::hash_value(&json!({
        "graph_revision": read.archive_revision,
        "changed": changed,
        "trees": trees,
    }));
    let changes = json!({
        "schema_version": 1,
        "kind": "finalize_changes",
        "graph_revision": read.archive_revision,
        "change_hash": change_hash,
        "changed": changed,
        "touched_trees": trees,
        "kinds": kinds,
        "counts": {"cards": changed.len(), "trees": trees.len()},
        "cycle": {
            "state": state_file.to_string_lossy(),
            "report": report_file.to_string_lossy(),
            "verdicts": report.as_ref().map(|report| report["verdicts"].clone()).unwrap_or(Value::Null),
            "faults_before": report.as_ref().map(|report| report["faults_before"].clone()).unwrap_or(Value::Null),
            "faults_after": report.as_ref().map(|report| report["faults_after"].clone()).unwrap_or(Value::Null),
            "refused": report.as_ref().map(|report| report["refused"].clone()).unwrap_or(Value::Null),
        },
    });
    util::write_text(changes_path(&p.out), &ps::json_pretty(&changes))?;
    // The one cycle is spent: this marker is what refuses a second one.
    util::write_text(
        cycle_path(&p.out),
        &ps::json_pretty(&json!({"schema_version":1,"kind":"fix_cycle","used":1,"permitted":1,"work":work.to_string_lossy(),"changes":changes["change_hash"],"graph_revision":read.archive_revision})),
    )?;
    let enabled = prepared_layers(p)?;
    let variant = must(&p.contract, "variant")?;
    let refresh = refresh_plan(&changes, &enabled, &variant);
    Ok(json!({
        "schema_version": 1,
        "kind": "finalize_changes",
        "step": "scope",
        "graph_revision": read.archive_revision,
        "change_hash": changes["change_hash"],
        "changed": changes["changed"],
        "touched_trees": changes["touched_trees"],
        "kinds": changes["kinds"],
        "counts": changes["counts"],
        "refresh": refresh,
        "changes": changes_path(&p.out).to_string_lossy(),
    }))
}

// --- refresh -----------------------------------------------------------------------------------

/// The checked coverage a refresh must supply. Organize disposes every registered suggestion,
/// the skeleton classifies every prepared group, and the mathematics theory describes every
/// Theory Card: all three are read from what the run already recorded, never from a caller's
/// assertion. Figures and Bridge take -Coverage, because their report's own entries are the
/// only honest source of their classification.
fn derived_coverage(p: &Prepared, zt: &mut Zt, layer: &str, applied: &str) -> Result<Vec<Value>> {
    match layer {
        "organize" => {
            let notes = ledger::registered_notes(p)?;
            let mut out = Vec::new();
            for (id, suggestion) in notes {
                let outcome = suggestion["outcome"]
                    .as_str()
                    .with_context(|| format!("note suggestion `{id}` retained no outcome"))?;
                out.push(json!({"ref": id, "outcome": outcome, "reason": suggestion["reason"]}));
            }
            Ok(out)
        }
        "skeleton" => {
            let groups = p.contract["groups"]
                .as_array()
                .context("the prepared run has no group manifest")?
                .clone();
            let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
            let mut out = Vec::new();
            for group in &groups {
                let n = group["n"].as_u64().context("invalid prepared group number")?;
                let anchor = must(group, "anchor")?;
                let producer = format!("digest/g{n:03}@1");
                let row: Option<(String, Option<String>)> = conn
                    .query_row("SELECT state,result_json FROM units WHERE id=?1", [&producer], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })
                    .optional()?;
                let (state, result) = row.with_context(|| {
                    format!("the skeleton's group producer `{producer}` is not registered")
                })?;
                let disposition = result
                    .map(|raw| serde_json::from_str::<Value>(&raw))
                    .transpose()?
                    .map(|result| ps::s(&result["disposition"]))
                    .unwrap_or_default();
                if state == "done" && disposition != "no_cards" {
                    out.push(json!({"ref": anchor, "outcome": "indexed", "reason": ""}));
                } else {
                    out.push(json!({
                        "ref": anchor,
                        "outcome": "gap",
                        "reason": if state == "done" { "the group produced no substantive Cards" } else { "the group's producer is unresolved" },
                    }));
                }
            }
            Ok(out)
        }
        "theory" => {
            if applied.trim().is_empty() {
                bail!("a `theory` refresh requires -Applied naming the theory apply record");
            }
            let record = util::read_json(applied)?;
            let key = must(&p.contract, "citation_key")?;
            let tree = zt.call("get_tree", json!({"tree":key}))?;
            let theories: Vec<String> = ps::ps_items(&tree["cards"])
                .into_iter()
                .filter(|card| card["type"] == json!("model") && card["form"] == json!("theory") && card["facet"].is_null())
                .map(|card| ps::field(&card, "address"))
                .collect();
            let mut out = Vec::new();
            for theory in &theories {
                let recorded = ps::ps_items(&record["theories"])
                    .into_iter()
                    .find(|entry| entry["address"] == json!(theory))
                    .with_context(|| format!("the applied theory record has no entry for `{theory}`"))?;
                out.push(json!({"ref": theory, "outcome": "rebuilt", "reason": ps::field(&recorded, "reason")}));
            }
            Ok(out)
        }
        _ => bail!("`{layer}` coverage is not derived by this command"),
    }
}

fn artifact_paths(layer: &str, a: &Params) -> Result<BTreeMap<String, PathBuf>> {
    let mut paths = BTreeMap::new();
    for name in declared_artifacts(layer)? {
        let value = match *name {
            "report" => &a.report,
            "applied" => &a.applied,
            "decisions" => &a.decisions,
            "skeleton" => &a.skeleton,
            _ => bail!("`{layer}` declares an artifact this command cannot bind: {name}"),
        };
        if value.trim().is_empty() {
            bail!("`{layer}` requires -{} naming its {name} artifact", name);
        }
        paths.insert((*name).to_string(), PathBuf::from(value));
    }
    Ok(paths)
}

/// The declared scope of one refresh, merged into the hub scope for a `hubs` revision.
fn refresh_scope(changes: &Value, layer: &str, hub: Option<Value>) -> Value {
    let mut changed = Vec::new();
    for change in ps::ps_items(&changes["changed"]) {
        changed.push(json!({
            "address": ps::field(&change, "address"),
            "kinds": change["kinds"],
            "before": change["fingerprint_before"],
            "after": change["fingerprint_after"],
        }));
    }
    let trees: Vec<String> = ps::ps_items(&changes["touched_trees"]).into_iter().map(|tree| ps::s(&tree)).collect();
    let mut scope = json!({
        "kind": "finalize_refresh",
        "schema_version": 1,
        "layer": layer,
        "graph_revision": changes["graph_revision"],
        "change_hash": changes["change_hash"],
        "changed_cards": changed,
        "touched_trees": trees,
    });
    if let Some(hub) = hub {
        scope["tree"] = hub["tree"].clone();
        scope["upper"] = hub["upper"].clone();
        scope["register"] = hub["register"].clone();
    }
    scope
}

/// Publish one aggregate revision, claim it, complete it through the verified publication
/// boundary, and return the completed facts.
fn complete_publication(
    p: &Prepared,
    zt: &mut Zt,
    layer: &str,
    scope: Value,
    artifacts: &BTreeMap<String, PathBuf>,
    coverage: &[Value],
) -> Result<Value> {
    let published = ledger::publication_refresh(p, layer, &scope)?;
    let unit = must(&published, "unit")?;
    let revision = published["revision"].as_i64().unwrap_or(0);
    let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
    if unit_state(&conn, &unit)? == "done" {
        // The same manifest already has a checked revision: the refresh is idempotent, never
        // a second pass over the same changes.
        let raw: String = conn.query_row("SELECT result_json FROM units WHERE id=?1", [&unit], |row| row.get(0))?;
        let mut stored: Value = serde_json::from_str(&raw).context("invalid retained publication result")?;
        stored["reused"] = json!(true);
        return Ok(json!({
            "layer": layer,
            "unit": unit,
            "revision": revision,
            "state": stored["state"],
            "complete": stored["complete"],
            "disposition": stored["disposition"],
            "reused": true,
            "superseded": published["superseded"],
            "coverage": stored["coverage"],
            "changed_cards": ps::ps_items(&scope["changed_cards"]).len(),
            "touched_trees": scope["touched_trees"],
        }));
    }
    let claimed = ledger::next(p, layer, 1, "finalize", 30, 0)?;
    let Some(entry) = ps::ps_items(&claimed["units"])
        .into_iter()
        .find(|entry| entry["id"] == json!(unit))
    else {
        bail!(
            "the {layer} revision `{unit}` is not ready to be claimed: {} open units remain in its layer",
            claimed["counts"]["open"]
        );
    };
    let token = entry["token"].as_i64().context("the claimed publication has no token")?;
    let inputs = ledger::unit_inputs(p, &unit)?;
    let contributions = ps::ps_items(&inputs["contributions"]);
    let unresolved: Vec<Value> = ps::ps_items(&inputs["dispositions"])
        .into_iter()
        .map(|entry| json!({"unit": entry["unit"], "reason": entry["fault"]["code"]}))
        .collect();
    let consumed: Vec<Value> = contributions
        .iter()
        .map(|entry| json!({"unit": entry["unit"], "fingerprint": entry["fingerprint"]}))
        .collect();
    let disposition = if contributions.is_empty() && unresolved.is_empty() {
        "no_work"
    } else if unresolved.is_empty() {
        "published"
    } else {
        "partial"
    };
    // The evidence names each declared artifact by its canonical path; the ledger binds it to
    // its own content hash, so a swapped or edited artifact cannot complete this revision.
    let mut bound = serde_json::Map::new();
    for (name, path) in artifacts {
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("the {layer} {name} artifact {} does not exist", path.display()))?;
        bound.insert(name.clone(), json!(canonical.to_string_lossy()));
    }
    let evidence = p.out.join(format!("publication-{layer}-book-{revision}.json"));
    let plan = json!({
        "schema_version": 1,
        "kind": "publication_completion",
        "unit": unit,
        "token": token,
        "layer": layer,
        "key": "book",
        "input_hash": entry["input_hash"],
        "disposition": disposition,
        "consumed": consumed,
        "unresolved": unresolved,
        "artifacts": bound,
        "coverage": coverage,
        "refresh": scope,
    });
    util::write_text(&evidence, &ps::json_pretty(&plan))?;
    let done = ledger::publication_finish(p, zt, &unit, token, &evidence)?;
    Ok(json!({
        "layer": layer,
        "unit": done["unit"],
        "revision": done["revision"],
        "state": done["state"],
        "complete": done["complete"],
        "disposition": done["disposition"],
        "reused": published["reused"],
        "superseded": published["superseded"],
        "coverage": done["coverage"],
        "changed_cards": ps::ps_items(&scope["changed_cards"]).len(),
        "touched_trees": scope["touched_trees"],
    }))
}

fn run_refresh(p: &Prepared, zt: &mut Zt, a: &Params) -> Result<Value> {
    let layer = a.layer.trim().to_string();
    if layer.is_empty() {
        bail!("-Layer is required");
    }
    if layer == "audit" {
        bail!("the audit revision is published by `-Step final`, after every refresh (R300)");
    }
    let changes_file = if a.changes.trim().is_empty() {
        changes_path(&p.out)
    } else {
        PathBuf::from(&a.changes)
    };
    let changes = util::read_json(&changes_file)
        .with_context(|| format!("no changed-Card manifest at {}", changes_file.display()))?;
    let enabled = prepared_layers(p)?;
    let variant = must(&p.contract, "variant")?;
    let refresh = refresh_plan(&changes, &enabled, &variant);
    let entry = refresh
        .iter()
        .find(|entry| entry["layer"] == json!(layer))
        .with_context(|| format!("`{layer}` is not one of the declared refresh layers"))?
        .clone();
    if entry["applicable"] != true {
        bail!("`{layer}` needs no refresh for these changes: {}", ps::field(&entry, "reason"));
    }
    let hub = if layer == "hubs" {
        if a.scope.trim().is_empty() {
            bail!("a `hubs` refresh requires -Scope naming the touched tree, its upper hubs and the Register");
        }
        Some(util::read_json(&a.scope)?)
    } else {
        None
    };
    let scope = refresh_scope(&changes, &layer, hub);
    let artifacts = artifact_paths(&layer, a)?;
    let coverage = if !a.coverage.trim().is_empty() {
        util::read_json(&a.coverage)?
            .as_array()
            .context("coverage must be a JSON array")?
            .clone()
    } else {
        match ps::field(&entry, "coverage").as_str() {
            "none" => Vec::new(),
            "derived" => derived_coverage(p, zt, &layer, &a.applied)?,
            _ => bail!("`{layer}` requires -Coverage naming its checked coverage entries"),
        }
    };
    complete_publication(p, zt, &layer, scope, &artifacts, &coverage)
}

// --- final -------------------------------------------------------------------------------------

/// Whether the layer's current revision is a finalization refresh revision.
fn is_refresh_revision(conn: &Connection, unit: &str) -> Result<bool> {
    let inputs: Option<String> = conn
        .query_row("SELECT inputs_json FROM units WHERE id=?1", [unit], |row| row.get(0))
        .optional()?;
    Ok(inputs.is_some_and(|inputs| inputs.contains("\"finalize_refresh\"")))
}

fn refresh_facts(p: &Prepared, refresh: &[Value]) -> Result<Vec<Value>> {
    let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
    let mut facts = Vec::new();
    for entry in refresh {
        let layer = ps::field(&entry, "layer");
        let Some((unit, _)) = ledger::required_publication(&conn, &layer)? else {
            facts.push(json!({"layer": layer, "state": "unregistered", "applicable": entry["applicable"]}));
            continue;
        };
        let state = unit_state(&conn, &unit)?;
        facts.push(json!({
            "layer": layer,
            "unit": unit,
            "revision": ledger::publication_unit(&unit).map(|(_, _, revision)| revision).unwrap_or(0),
            "state": state,
            "refreshed": is_refresh_revision(&conn, &unit)?,
            "applicable": entry["applicable"],
            "reason": entry["reason"],
            "stage_owned": entry["stage_owned"],
        }));
    }
    Ok(facts)
}

fn run_final(p: &Prepared, zt: &mut Zt, a: &Params) -> Result<Value> {
    let plan_file = plan_path(&p.out);
    if !plan_file.exists() {
        bail!("no audit plan: run `zt-digest finalize -Step plan` first");
    }
    let plan = util::read_json(&plan_file)?;
    let branch = must(&plan, "branch")?;
    let changes_file = if a.changes.trim().is_empty() {
        changes_path(&p.out)
    } else {
        PathBuf::from(&a.changes)
    };
    let changes = if changes_file.exists() {
        util::read_json(&changes_file)?
    } else if branch == "fix" {
        bail!("the plan's one correction cycle ran but no changed-Card manifest was recorded: run `-Step scope` first");
    } else {
        json!({
            "graph_revision": plan["graph_revision"],
            "change_hash": canon::hash_value(&json!({"changed":[], "plan": plan["audit"]["unit"]})),
            "changed": [], "touched_trees": [], "kinds": [], "counts": {"cards":0,"trees":0},
        })
    };
    let scheduled_revision = plan["audit"]["revision"].as_i64().unwrap_or(0);
    let plan_revision = plan["graph_revision"].as_u64().unwrap_or(0);
    // The final audit is always a fresh read after the corrections and refreshes, never the
    // report that scheduled them (R300).
    let report_path = if a.report.trim().is_empty() {
        let dir = p.out.join("finalize").join("final-audit");
        let params = audit::Params {
            citation_key: must(&p.contract, "citation_key")?,
            orphan_targets: String::new(),
            out: dir.to_string_lossy().into_owned(),
            max_sentences: 12,
            max_chars: 400,
            max_steps: 6,
            sample: 15,
            skip_models: false,
        };
        audit::run(zt, &params, &mut |line| {
            let _ = util::print_line(line);
        })?;
        std::fs::canonicalize(dir.join("audit.json"))?
    } else {
        std::fs::canonicalize(&a.report)
            .with_context(|| format!("the final audit report {} does not exist", a.report))?
    };
    let final_view = AuditView::open(&report_path)?;
    let final_revision = final_view.graph_revision();
    let changed = ps::ps_items(&changes["changed"]).len();
    if changed > 0 && final_revision <= plan_revision {
        bail!(
            "the final audit describes graph revision {final_revision}, which is not after the corrections and refreshes of revision {plan_revision}: a stale report can never be the final audit (R300)"
        );
    }
    if changed == 0 && final_revision < plan_revision {
        bail!("the final audit describes graph revision {final_revision}, older than the scheduling audit's {plan_revision}");
    }
    let scope = json!({
        "kind": "finalize_refresh",
        "schema_version": 1,
        "layer": "audit",
        "graph_revision": final_revision,
        "scheduled_revision": scheduled_revision,
        "change_hash": changes["change_hash"],
        "changed_cards": refresh_scope(&changes, "audit", None)["changed_cards"],
        "touched_trees": changes["touched_trees"],
    });
    let mut artifacts = BTreeMap::new();
    artifacts.insert("report".to_string(), report_path.clone());
    let published = complete_publication(p, zt, "audit", scope, &artifacts, &[])?;
    let faults = final_view.faults()?;
    let mut residual = Vec::new();
    let mut by_class: BTreeMap<String, i64> = BTreeMap::new();
    for fault in &faults {
        let rule = ps::field(&fault, "rule");
        if ADVISORY.contains(&rule.as_str()) || rule.eq_ignore_ascii_case(NOTE_RULE) {
            continue;
        }
        let class = if SOURCE_RULES.contains(&rule.as_str()) { "source" } else { "hard" };
        *by_class.entry(class.to_string()).or_default() += 1;
        residual.push(json!({
            "address": ps::field(&fault, "address"),
            "rule": rule,
            "class": class,
            "text": ps::field(&fault, "text"),
        }));
    }
    // A declared applicable refresh that did not complete is this branch's infrastructure
    // fault: the final report names it instead of pretending the publication is current.
    let enabled = prepared_layers(p)?;
    let refresh_now = refresh_plan(&changes, &enabled, &must(&p.contract, "variant")?);
    let mut infrastructure = Vec::new();
    for entry in &refresh_now {
        let layer = ps::field(entry, "layer");
        if entry["applicable"] != true || layer == "audit" {
            continue;
        }
        let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
        match ledger::required_publication(&conn, &layer)? {
            Some((unit, _)) => {
                let state = unit_state(&conn, &unit)?;
                if state != "done" || !is_refresh_revision(&conn, &unit)? {
                    infrastructure.push(json!({
                        "layer": layer,
                        "unit": unit,
                        "state": state,
                        "reason": "the affected publication has no completed refresh revision",
                    }));
                }
            }
            None => infrastructure.push(json!({"layer": layer, "reason": "the affected publication is not registered"})),
        }
    }
    *by_class.entry("infrastructure".to_string()).or_default() += infrastructure.len() as i64;
    let status = if residual.is_empty() && infrastructure.is_empty() {
        "done"
    } else {
        "partial"
    };
    let key = must(&p.contract, "citation_key")?;
    let book = final_view.book(&key);
    let report = json!({
        "schema_version": 1,
        "kind": "finalize_report",
        "step": "final",
        "citation_key": key,
        "variant": must(&p.contract, "variant")?,
        "status": status,
        "complete": status == "done",
        "branch": plan["branch"],
        "graph_revision": final_revision,
        "audit": {
            "scheduled": {"unit": plan["audit"]["unit"], "revision": scheduled_revision, "graph_revision": plan_revision},
            "final": {"unit": published["unit"], "revision": published["revision"], "graph_revision": final_revision},
        },
        "changes": {
            "cards": changed,
            "trees": ps::ps_items(&changes["touched_trees"]).len(),
            "kinds": changes["kinds"],
            "graph_revision": changes["graph_revision"],
            "change_hash": changes["change_hash"],
        },
        "refresh": refresh_facts(p, &refresh_now)?,
        "faults": {
            "total": faults.len(),
            "residual": residual.len(),
            "advisory": faults.len() - residual.len(),
            "by_class": by_class,
            "list": residual,
        },
        "infrastructure": infrastructure,
        "audit_book": {
            "cards": book["cards"],
            "orphans": book["orphans"]["count"],
            "figures_missing": book["figures"]["missing_known"].as_i64().unwrap_or(0) + book["figures"]["missing_unknown"].as_i64().unwrap_or(0),
            "models_not_self_sufficient": book["models"]["not_self_sufficient"],
        },
        "report": {"path": final_report_path(&p.out).to_string_lossy()},
    });
    util::write_text(final_report_path(&p.out), &ps::json_pretty(&report))?;
    util::write_text(p.out.join("finalize").join("final-report.md"), &final_markdown(&report))?;
    Ok(report)
}

fn final_markdown(report: &Value) -> String {
    let mut text = format!(
        "# 收尾报告 / Finalization report\n\n**{}** at graph revision **{}**.\n\nThe scheduling audit was `{}` (revision {}); the final read-only audit is `{}` (revision {}).\n\n",
        ps::field(report, "status"),
        report["graph_revision"],
        ps::field(&report["audit"]["scheduled"], "unit"),
        report["audit"]["scheduled"]["revision"],
        ps::field(&report["audit"]["final"], "unit"),
        report["audit"]["final"]["revision"],
    );
    text.push_str(&format!(
        "Correction branch: **{}**. Changed Cards: {}. Touched trees: {}.\n\n",
        ps::field(report, "branch"),
        report["changes"]["cards"],
        report["changes"]["trees"],
    ));
    text.push_str("## Refreshed publications\n\n| layer | unit | state | refreshed |\n|---|---|---|---|\n");
    for entry in ps::ps_items(&report["refresh"]) {
        text.push_str(&format!(
            "| {} | `{}` | {} | {} |\n",
            ps::field(&entry, "layer"),
            ps::field(&entry, "unit"),
            ps::field(&entry, "state"),
            entry["refreshed"],
        ));
    }
    text.push_str("\n## Residual faults\n\n");
    if ps::ps_items(&report["faults"]["list"]).is_empty() {
        text.push_str("None: the final audit found no hard fault.\n\n");
    } else {
        for fault in ps::ps_items(&report["faults"]["list"]) {
            text.push_str(&format!(
                "- `{}` {} ({}): {}\n",
                ps::field(&fault, "address"),
                ps::field(&fault, "rule"),
                ps::field(&fault, "class"),
                ps::field(&fault, "text"),
            ));
        }
        text.push('\n');
    }
    if !ps::ps_items(&report["infrastructure"]).is_empty() {
        text.push_str("## Infrastructure faults\n\n");
        for fault in ps::ps_items(&report["infrastructure"]) {
            text.push_str(&format!(
                "- {} `{}`: {}\n",
                ps::field(&fault, "layer"),
                ps::field(&fault, "unit"),
                ps::field(&fault, "reason"),
            ));
        }
    }
    text
}

/// `zt-digest finalize`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = [
        "Step", "Out", "Report", "FixWork", "Layer", "Changes", "Coverage", "Scope", "Decisions", "Applied", "Skeleton",
    ];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &[])?;
    let step = a.choice("Step", &["plan", "scope", "refresh", "final"])?;
    let p = Params {
        step: step.to_ascii_lowercase(),
        out: a.required("Out")?,
        report: a.str("Report"),
        fix_work: a.str("FixWork"),
        layer: a.str("Layer"),
        changes: a.str("Changes"),
        coverage: a.str("Coverage"),
        scope: a.str("Scope"),
        decisions: a.str("Decisions"),
        applied: a.str("Applied"),
        skeleton: a.str("Skeleton"),
    };
    if p.out.trim().is_empty() {
        bail!("-Out is required");
    }
    let mut zt = crate::zt::start()?;
    // `plan` publishes the scheduling audit revision, so it reads the prepared Cards too.
    let require_cards = p.step == "plan";
    let prepared = Prepared::read_mode(&p.out, &mut zt, require_cards)?;
    let result = match p.step.as_str() {
        "plan" => run_plan(&prepared, &mut zt, &p)?,
        "scope" => run_scope(&prepared, &mut zt, &p)?,
        "refresh" => run_refresh(&prepared, &mut zt, &p)?,
        _ => run_final(&prepared, &mut zt, &p)?,
    };
    util::print_line(&ps::json_compact(&result))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(address: &str, rule: &str, text: &str) -> Value {
        json!({"address": address, "rule": rule, "text": text, "line": format!("{address} {rule} {text}")})
    }

    fn card(address: &str, category: &str, fingerprint: &str) -> Value {
        json!({"address": address, "category": category, "tree": "Key", "fingerprint": fingerprint,
               "title": "t", "links": [], "figures": []})
    }

    /// The audited view is keyed by the folded address, exactly as a real audit read keys it.
    fn view(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
        pairs
            .iter()
            .map(|(key, value)| (ps::key(key), value.clone()))
            .collect()
    }

    fn by_address(faults: &[Value]) -> BTreeMap<String, Vec<String>> {
        let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for fault in faults {
            map.entry(ps::key(&ps::field(&fault, "address")))
                .or_default()
                .push(ps::field(&fault, "rule"));
        }
        map
    }

    #[test]
    fn the_four_branches_of_the_finite_finalization() {
        // No fault at all: nothing to correct, the final audit is the only step.
        assert_eq!(branch_of(&[]), "no_fix");
        // Only advisory rules: still no correction cycle.
        let advisories = vec![json!({"outcome":"advisory"}), json!({"outcome":"advisory"})];
        assert_eq!(branch_of(&advisories), "no_fix");
        // One correctable Card: the one permitted cycle runs.
        assert_eq!(branch_of(&[json!({"outcome":"fix"}), json!({"outcome":"blocked"})]), "fix");
        // No correctable Card but a stale base: the plan refuses a rewrite from it.
        assert_eq!(branch_of(&[json!({"outcome":"conflict"}), json!({"outcome":"blocked"})]), "conflict");
        // Nothing correctable and nothing stale: the finite branch cannot correct the residue.
        assert_eq!(branch_of(&[json!({"outcome":"blocked"})]), "unresolved");
        assert_eq!(branch_of(&[json!({"outcome":"stale"})]), "unresolved");
    }

    #[test]
    fn a_writable_unchanged_card_is_the_one_cycle_work() {
        let faults = vec![fault("Key/1|a", "R2", "一句话太长"), fault("Key/1|a", "I1", "没有来源")];
        let audited = view(&[("Key/1|a", card("Key/1|a", "idea", "fp-1"))]);
        let current = view(&[("Key/1|a", card("Key/1|a", "idea", "fp-1"))]);
        let work = disposition(&faults[0], &audited, &current, &by_address(&faults));
        assert_eq!(work["outcome"], json!("fix"));
        assert_eq!(work["card"], json!("Key/1|a"));
        assert_eq!(work["rules"], json!(["R2"]));
        // The advisory rule on the same Card is never scheduled as work.
        let advisory = disposition(&faults[1], &audited, &current, &by_address(&faults));
        assert_eq!(advisory["outcome"], json!("advisory"));
    }

    #[test]
    fn a_done_build_or_a_stale_base_is_never_rewritten() {
        let audited = view(&[
            ("Key/2|c", card("Key/2|c", "model", "fp-m")),
            ("Key/1|b", card("Key/1|b", "idea", "fp-old")),
        ]);
        let current = view(&[
            ("Key/2|c", card("Key/2|c", "model", "fp-m")),
            ("Key/1|b", card("Key/1|b", "idea", "fp-new")),
        ]);
        let faulting = vec![fault("Key/2|c", "R3", "x"), fault("Key/1|b", "R4", "y")];
        let model = disposition(&faulting[0], &audited, &current, &by_address(&faulting));
        assert_eq!(model["outcome"], json!("blocked"));
        assert!(ps::field(&model, "reason").contains("may not reopen a done build"));
        let stale = disposition(&faulting[1], &audited, &current, &by_address(&faulting));
        assert_eq!(stale["outcome"], json!("conflict"));
        assert!(ps::field(&stale, "reason").contains("stale base"));
        // A Card that no longer exists is not a correction target at all.
        let gone = disposition(&fault("Key/1|z", "R2", "z"), &audited, &current, &by_address(&faulting));
        assert_eq!(gone["outcome"], json!("blocked"));
    }

    #[test]
    fn the_refresh_set_follows_the_changed_card_kinds() {
        let enabled: BTreeSet<String> = ["link-check", "organize", "skeleton", "figures", "hubs", "bridge", "audit"]
            .iter()
            .map(|name| name.to_string())
            .collect();
        let layers = |changes: &Value, enabled: &BTreeSet<String>, variant: &str| -> Vec<String> {
            refresh_plan(changes, enabled, variant)
                .iter()
                .filter(|entry| entry["applicable"] == true && entry["layer"] != json!("audit"))
                .map(|entry| ps::field(&entry, "layer"))
                .collect()
        };
        let structure = json!({"kinds": ["structure"], "changed": [{"address": "Key/1|a"}]});
        // Only the publications this finite branch re-derives are refreshed; the rest name their
        // own stage as the owner of that content.
        assert_eq!(layers(&structure, &enabled, "econ"), vec!["link-check"]);
        let links = json!({"kinds": ["links"], "changed": [{"address": "Key/1|a"}]});
        assert_eq!(layers(&links, &enabled, "econ"), vec!["link-check"]);
        let owned = refresh_plan(&structure, &enabled, "econ")
            .into_iter()
            .find(|entry| entry["layer"] == json!("hubs"))
            .unwrap();
        assert_eq!(owned["applicable"], json!(false));
        assert_eq!(owned["stage_owned"], json!(true));
        assert!(ps::field(&owned, "reason").contains("its own stage generates this content"));
        // The final audit is always applicable: it is the branch's last step.
        let plan = refresh_plan(&links, &enabled, "econ");
        assert_eq!(plan.iter().find(|entry| entry["layer"] == json!("audit")).unwrap()["applicable"], json!(true));
        // The mathematics theory layer belongs to the mathematics variant only.
        let theory = refresh_plan(&links, &enabled, "econ")
            .into_iter()
            .find(|entry| entry["layer"] == json!("theory"))
            .unwrap();
        assert_eq!(theory["applicable"], json!(false));
        assert!(ps::field(&theory, "reason").contains("the math variant owns this layer"));
        // An excluded layer names its own reason.
        let mut without_figures = enabled.clone();
        without_figures.remove("figures");
        let figures = refresh_plan(&json!({"kinds":["figures"],"changed":[{}]}), &without_figures, "econ")
            .into_iter()
            .find(|entry| entry["layer"] == json!("figures"))
            .unwrap();
        assert_eq!(figures["applicable"], json!(false));
        assert!(ps::field(&figures, "reason").contains("excluded"));
    }

    #[test]
    fn no_change_refreshes_nothing_but_the_final_audit() {
        let enabled: BTreeSet<String> = ["link-check", "audit"].iter().map(|name| name.to_string()).collect();
        let plan = refresh_plan(&json!({"kinds": [], "changed": []}), &enabled, "econ");
        let applicable: Vec<String> = plan
            .iter()
            .filter(|entry| entry["applicable"] == true)
            .map(|entry| ps::field(&entry, "layer"))
            .collect();
        assert_eq!(applicable, vec!["audit"]);
        for entry in plan.iter().filter(|entry| entry["layer"] != json!("audit")) {
            assert_eq!(entry["applicable"], json!(false), "{}", ps::field(&entry, "layer"));
            let reason = ps::field(&entry, "reason");
            assert!(
                reason.contains("nothing to refresh")
                    || reason.contains("owns this layer")
                    || reason.contains("excluded"),
                "{}: {reason}",
                ps::field(&entry, "layer")
            );
        }
    }

    #[test]
    fn change_kinds_separate_structure_links_and_figures() {
        let before = json!({"title":"旧标题","body":"见图 2.1。","links":["K/1"],"figures":["2.1"]});
        let after = json!({"title":"新标题","body":"见图 2.1。","links":["K/1"],"figures":["2.1"]});
        assert_eq!(change_kinds(Some(&before), &after), vec!["structure"]);
        let linked = json!({"title":"旧标题","body":"见图 2.1。","links":["K/1","K/2"],"figures":["2.1"]});
        assert_eq!(change_kinds(Some(&before), &linked), vec!["links"]);
        // A body edit that adds a figure reference is both a structure edit and a figure edit.
        let shown = json!({"title":"旧标题","body":"见图 2.1 与图 2.2。","links":["K/1"],"figures":["2.1","2.2"]});
        assert_eq!(change_kinds(Some(&before), &shown), vec!["figures", "structure"]);
        let figures_only = json!({"title":"旧标题","body":"见图 2.1。","links":["K/1"],"figures":["2.1"]});
        // A figure reference the Card no longer names is a figure edit only.
        let mut previous = before.clone();
        previous["figures"] = json!(["2.1", "2.2"]);
        assert_eq!(change_kinds(Some(&previous), &figures_only), vec!["figures"]);
        // A split Card is new: every consumer of its text, Links and figures must refresh.
        assert_eq!(change_kinds(None, &before), vec!["figures", "links", "new", "structure"]);
    }

    #[test]
    fn rules_are_ordered_for_a_readable_cycle() {
        assert_eq!(rule_key("R12"), (0, 12, "R12".into()));
        assert!(rule_key("R2") < rule_key("R12"));
        assert!(rule_key("R1") < rule_key("I3"));
        assert!(rule_key("I3") < rule_key("E1"));
        assert!(rule_key("E1") < rule_key("L1"));
    }
}
