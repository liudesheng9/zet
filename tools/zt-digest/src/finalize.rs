//! `zt-digest finalize`: the declared finite finalization branch (R299, R300; #86).
//!
//! The run already published one read-only audit revision. Finalization is the finite
//! sequence `audit-plan → at most one compiler correction cycle → changed-Card manifest →
//! the round's Term-link pass → completion of the affected publications → final read-only
//! audit`:
//!
//! - `plan` publishes the scheduling audit revision when it is still open, then derives one
//!   immutable plan from it: every fault with its disposition (`fix`, `conflict`, `blocked`,
//!   `stale`, `advisory`), the single permitted correction cycle with its exact Card list,
//!   the refresh layers the changed Cards may affect, and the branch (`no_fix`, `fix`,
//!   `conflict`, `unresolved`). A recorded cycle makes the plan immutable, so the branch can
//!   never be re-entered.
//! - `scope` records the cycle's exact changed-Card/touched-tree manifest once, by comparing
//!   the audited Card fingerprints with the current archive, and classifies the round's
//!   Term-link scope: the changed eligible Cards, or the whole book when the correction
//!   changed the Term mapping (S26, #169; R0438/R0439). One cycle per plan: a second scope
//!   is refused instead of looping.
//! - `changes` returns that retained manifest read-only, with whether the branch already
//!   published its final audit and whether its round completed, so a resume continues a
//!   recorded branch without re-entering the cycle (S26, #169).
//! - `refresh` publishes and completes one affected aggregate publication at a new revision
//!   whose declared scope is that manifest. The same manifest reuses its revision; a changed
//!   one derives the next.
//! - `final` runs the read-only audit again (or verifies a given fresh report), publishes and
//!   completes the final audit revision, and writes the final report: the new graph revision,
//!   the round's Term-link facts, the final audit's Term quality, every residual
//!   hard/source/infrastructure fault, and `partial` instead of the scheduling audit whenever
//!   a hard fault or an incomplete round remains.
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
use serde_json::{Map, Value, json};
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
    if ps::field(&fault, "line").contains("(advisory)") || rule.eq_ignore_ascii_case(NOTE_RULE) {
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
        .cloned()
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
        // The published `(advisory)` tag is the one advisory decision every caller
        // reads (S06, R0187): no caller keeps a private advisory list.
        if ps::field(fault, "line").contains("(advisory)") {
            continue;
        }
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

/// The finalization round's Term-link scope (S26, #169; design 14.4, R0438/R0439): the scheduling
/// audit revision is the round's generation identity; a changed Term list, head name, auto-link
/// policy or whole vocabulary invalidates the book mapping and replans the whole book, while a
/// body-only correction relinks exactly the changed eligible Cards. A round never owns no unit by
/// accident: a disabled layer, a book without one Term list, or a mapping that cannot be derived
/// states that reason instead.
fn term_link_round(
    enabled: bool,
    round: i64,
    list: Option<&str>,
    unavailable: Option<&str>,
    current: Option<&Value>,
    before: &[(&'static str, &Value)],
    eligible: &[String],
    excluded: &[Value],
    term_list_changed: bool,
) -> Value {
    let mut block = json!({
        "schema_version": 1,
        "kind": "term_link_round",
        "applicable": true,
        "round": round,
        "scope": "none",
        "cards": [],
        "excluded": excluded,
        "mapping_changed": false,
        "reasons": [],
        "before": Value::Null,
        "current": current.cloned().unwrap_or(Value::Null),
        "reason": "",
    });
    let mut reasons: Vec<String> = Vec::new();
    if !enabled {
        block["applicable"] = json!(false);
        block["reason"] = json!("the prepared run excluded the Term-link layer");
        return block;
    }
    let Some(list) = list else {
        block["applicable"] = json!(false);
        block["reason"] = json!(format!(
            "the book has no single Term list: {}",
            unavailable.unwrap_or("the Term structure could not be read")
        ));
        return block;
    };
    block["list"] = json!(list);
    let Some(current) = current else {
        block["applicable"] = json!(false);
        block["reason"] = json!(format!(
            "the book's Term mapping could not be derived: {}",
            unavailable.unwrap_or("no Term structure")
        ));
        return block;
    };
    let mut mapping_changed = term_list_changed;
    if term_list_changed {
        reasons.push("the corrected Card is the Term list".to_string());
    }
    let keys = ["list", "mapping", "autolink", "vocabulary"];
    let mut compared = false;
    for (source, fingerprints) in before {
        if !fingerprints.is_object() {
            continue;
        }
        compared = true;
        for key in keys {
            let was = fingerprints[key].as_str().unwrap_or_default();
            let now = current[key].as_str().unwrap_or_default();
            if !was.is_empty() && was != now {
                mapping_changed = true;
                reasons.push(format!(
                    "the {source} Term mapping changed its `{key}` fingerprint"
                ));
            }
        }
    }
    if !compared {
        // No applied mapping is recorded (an older audit, or a whole-book pass that never planned):
        // the safe scope is the whole book, never a subset that could miss a changed vocabulary.
        mapping_changed = true;
        reasons.push(
            "no applied Term mapping fingerprint is available; the whole book is replanned"
                .to_string(),
        );
    }
    let mut stored_before = Map::new();
    for (source, fingerprints) in before {
        stored_before.insert((*source).to_string(), Value::clone(fingerprints));
    }
    block["before"] = Value::Object(stored_before);
    block["mapping_changed"] = json!(mapping_changed);
    if mapping_changed {
        block["scope"] = json!("book");
        block["reasons"] = json!(reasons);
        return block;
    }
    block["reasons"] = json!(reasons);
    if eligible.is_empty() {
        block["scope"] = json!("none");
        block["reason"] = json!("no eligible Card changed: there is nothing to relink");
        return block;
    }
    block["scope"] = json!("cards");
    block["cards"] = json!(eligible);
    block
}

/// Read the book's current Term mapping and classify this round's scope. One Term-structure read
/// and one bounded read of the heads' bodies; every fact comes from the same archive revision the
/// scope manifest was recorded at.
fn run_term_scope(
    p: &Prepared,
    zt: &mut Zt,
    key: &str,
    plan: &Value,
    audit: &AuditView,
    changed: &[Value],
    current_cards: &BTreeMap<String, Value>,
) -> Result<Value> {
    // The prepared target names the layers it excluded; the Term layers are switches whose layer
    // names (`termlink`) are not in COMMON's enabled list, so the excluded list is the one fact.
    let target = ledger::target_of(&p.contract)?;
    let enabled = !ps::ps_items(&target["excluded"])
        .iter()
        .any(|layer| ps::eq(&ps::s(layer), "termlink"));
    let round = plan["audit"]["revision"].as_i64().unwrap_or(0);
    if !enabled {
        return Ok(term_link_round(false, round, None, None, None, &[], &[], &[], false));
    }
    let terms = match zt.call("get_terms", json!({"at": key})) {
        Ok(terms) => terms,
        Err(error) => {
            return Ok(term_link_round(
                true,
                round,
                None,
                Some(&error.to_string()),
                None,
                &[],
                &[],
                &[],
                false,
            ));
        }
    };
    let list = terms["list"].as_str().filter(|list| !list.is_empty()).map(str::to_string);
    let Some(list) = list else {
        return Ok(term_link_round(
            true,
            round,
            None,
            Some("the Term structure names no list"),
            None,
            &[],
            &[],
            &[],
            false,
        ));
    };
    // The shared derivation needs the heads' authored bodies (their auto-link lines), never the
    // whole tree: the Term structure already carries every other mapping fact.
    let mut head_addresses: BTreeSet<String> = BTreeSet::new();
    // The list's own body is the `list` fingerprint's input.
    head_addresses.insert(list.clone());
    for row in ps::ps_items(&terms["rows"]) {
        for head in ps::ps_items(&row["heads"]) {
            if let Some(head) = head.as_str() {
                head_addresses.insert(head.to_string());
            }
        }
    }
    for head in ps::ps_items(&terms["heads"]) {
        let address = ps::field(&head, "address");
        if !address.is_empty() {
            head_addresses.insert(address);
        }
    }
    let addresses: Vec<String> = head_addresses.into_iter().collect();
    let mut by_address: BTreeMap<String, Value> = BTreeMap::new();
    if !addresses.is_empty() {
        let read = zt.cards(&addresses, "text")?;
        for card in &read.cards {
            by_address.insert(ps::field(card, "address"), card.clone());
        }
    }
    let refs: BTreeMap<String, &Value> = by_address
        .iter()
        .map(|(address, card)| (address.clone(), card))
        .collect();
    let (current, unavailable) = match crate::term_link::vocabulary(&terms, &refs) {
        Ok(facts) => (
            Some(json!({
                "list": by_address
                    .get(&list)
                    .map(|card| canon::sha256_hex(ps::field(card, "body").as_bytes())),
                "mapping": facts.mapping_hash,
                "autolink": facts.autolink_hash,
                "vocabulary": facts.vocabulary_hash,
            })),
            None,
        ),
        Err(error) => (None, Some(error.to_string())),
    };
    // The recorded mappings this round may have moved away from: the scheduling audit's own
    // terms section, and the whole-book plan the run applied.
    let mut before: Vec<(&'static str, Value)> = Vec::new();
    let audit_fingerprints = audit.book(key)["terms"]["fingerprints"].clone();
    if audit_fingerprints.is_object() {
        before.push(("audit", audit_fingerprints));
    }
    let applied = p.out.join("term-link").join(crate::term_link::PLAN);
    if applied.is_file()
        && let Ok(plan) = util::read_json(&applied)
        && let Some(fingerprints) = plan.get("fingerprints").filter(|value| value.is_object())
    {
        before.push(("whole-book plan", fingerprints.clone()));
    }
    let before_refs: Vec<(&'static str, &Value)> =
        before.iter().map(|(source, fingerprints)| (*source, fingerprints)).collect();
    let mut eligible: Vec<String> = Vec::new();
    let mut excluded: Vec<Value> = Vec::new();
    let mut term_list_changed = false;
    for entry in changed {
        let address = ps::field(entry, "address");
        let Some(card) = current_cards.get(&address) else { continue };
        let reason = crate::term_link::exclusion_reason(
            &address,
            &ps::field(card, "title"),
            &ps::field(card, "body"),
            &ps::field(card, "type"),
            card.get("term").unwrap_or(&Value::Null),
            key,
            &list,
        );
        if ps::field(card, "term_role") == "list" {
            term_list_changed = true;
        }
        match reason {
            Some(reason) => excluded.push(json!({"address": address, "reason": reason})),
            None => eligible.push(address),
        }
    }
    let current_ref = current.as_ref();
    Ok(term_link_round(
        true,
        round,
        Some(&list),
        unavailable.as_deref(),
        current_ref,
        &before_refs,
        &eligible,
        &excluded,
        term_list_changed,
    ))
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
    let mut current_cards: BTreeMap<String, Value> = BTreeMap::new();
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
            // S26 (#169): the Term facts the round's scope is classified from.
            "type": ps::field(&card, "type"),
            "term": card["term"].clone(),
            "term_role": card["term_role"].clone(),
        });
        current_cards.insert(address.clone(), now.clone());
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
            "type": ps::field(&card, "type"),
            "term": card["term"].clone(),
            "term_role": card["term_role"].clone(),
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
    // S26 (#169): the round's Term-link scope, decided from the actual changed Cards and the
    // applied mapping, is part of the immutable manifest the finalization's relink runs.
    let key = must(&plan, "citation_key")?;
    let term_link = run_term_scope(p, zt, &key, &plan, &audit, &changed, &current_cards)?;
    let changes = json!({
        "schema_version": 1,
        "kind": "finalize_changes",
        "graph_revision": read.archive_revision,
        "change_hash": change_hash,
        "changed": changed,
        "touched_trees": trees,
        "kinds": kinds,
        "counts": {"cards": changed.len(), "trees": trees.len()},
        "term_link": term_link,
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
        "term_link": changes["term_link"],
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

/// The Figures coverage a stage publication supplies: every figure the prepared run recognized,
/// `shown` when a Card of the figures report names it (by its alt text `图 III` or `图 ` + a label),
/// otherwise `unmatched` with its reason. The prepared figures file and the stage's own report are the
/// only sources; nothing is asserted by a caller.
fn figures_coverage(recognized: &[Value], report: &Value) -> Vec<Value> {
    // A figure some Card of the book already shows, and one outside the digested range, as the stage's
    // report lists them by sha256 (ebrill2001modern: four figures reported unmatched, three of them shown).
    let already: BTreeSet<String> = ps::ps_items(&report["shown"]).iter().map(ps::s).collect();
    let outside: BTreeSet<String> = ps::ps_items(&report["outside"]).iter().map(ps::s).collect();
    let shown: BTreeSet<String> = ps::ps_items(&report["cards"])
        .into_iter()
        .flat_map(|card| ps::ps_items(&card["figures"]).into_iter().map(|label| ps::s(&label).trim().to_string()))
        .collect();
    recognized
        .iter()
        .map(|figure| {
            let mut names: Vec<String> = Vec::new();
            let alt = ps::s(&figure["alt"]).trim().to_string();
            if !alt.is_empty() {
                names.push(alt);
            }
            for label in ps::ps_items(&figure["labels"]) {
                names.push(format!("图 {}", ps::s(&label).trim()));
            }
            let sha = ps::s(&figure["sha256"]);
            let is_shown = names.iter().any(|name| shown.contains(name)) || already.contains(&sha);
            if outside.contains(&sha) && !is_shown {
                return json!({"ref": figure["name"], "outcome": "outside", "reason": "the image lies outside the digested range of the source"});
            }
            if is_shown {
                json!({"ref": figure["name"], "outcome": "shown", "path": figure["path"], "sha256": figure["sha256"]})
            } else {
                json!({
                    "ref": figure["name"],
                    "outcome": "unmatched",
                    "reason": if names.is_empty() { "the source gives this figure no label, so no Card names it".to_string() } else { format!("no Card of the digest names {}", names[0]) },
                })
            }
        })
        .collect()
}

/// The Bridge coverage: every proposal of the report once, `accepted` when the apply state of this report
/// accepted it (new or earlier carrier), else `open`. An apply state of another report accepts nothing.
fn bridge_coverage(report: &Value, state: &Value) -> Vec<Value> {
    let same = !state.is_null() && ps::s(&state["report_id"]) == ps::s(&report["report_id"]);
    let accepted: BTreeSet<String> = if same {
        ["new", "earlier"].iter().flat_map(|section| ps::ps_items(&state["accepted"][*section]).into_iter().map(|id| ps::s(&id))).collect()
    } else {
        BTreeSet::new()
    };
    ps::ps_items(&report["proposals"])
        .into_iter()
        .map(|proposal| {
            let outcome = if accepted.contains(&ps::s(&proposal["id"])) { "accepted" } else { "open" };
            json!({"ref": proposal["id"], "outcome": outcome})
        })
        .collect()
}

/// Publish the Bridge stage revision of a prepared run after an apply (`zt-digest bridge -Step apply -Apply`),
/// so the ledger records the proposals now accepted. Before this an apply wrote its Links and the run kept
/// reporting every proposal open.
pub(crate) fn publish_bridge_stage(out: &str, report: &str) -> Result<Value> {
    let mut zt = crate::zt::start()?;
    let prepared = Prepared::read(out, &mut zt)?;
    let a = Params {
        step: "stage".into(), out: out.into(), report: report.into(), fix_work: String::new(), layer: "bridge".into(),
        changes: String::new(), coverage: String::new(), scope: String::new(), decisions: String::new(), applied: String::new(), skeleton: String::new(),
    };
    run_stage(&prepared, &mut zt, &a)
}

/// The scope of a stage publication: its layer and the hash of its checked coverage. A stage run again on
/// unchanged inputs with the same coverage reuses its revision; one whose coverage changed publishes a new
/// revision that supersedes the old. With no scope the old revision was always reused, so a Figures stage run
/// again after its gaps were fixed kept reporting them (ebrill2001modern: four figures unmatched, three shown).
fn stage_scope(layer: &str, coverage: &[Value]) -> Value {
    json!({"stage": layer, "coverage_sha256": crate::canon::sha256_hex(crate::canon::canonical_json(&json!(coverage)).as_bytes())})
}

/// `-Step stage`: a stage-owned publication (Organize, the skeleton, Figures, Bridge) completed by its own
/// stage, through the same verified boundary as a refresh: its declared artifacts are bound by hash and
/// its coverage is derived from what the run recorded. Before this step nothing completed these
/// revisions: finalize leaves them to their stages, and no stage published, so every run kept
/// `organize/book@1`, `skeleton/book@1`, `figures/book@1` and `bridge/book@1` ready and could never be
/// content-complete (grossman2002integration trial 8). The Link check publishes its first revision
/// here too: the finalization refreshes it only when its one correction cycle changed a Card, so a run
/// whose final audit found nothing to correct kept `link-check/book@1` ready (acemoglu2025nobel trial 1).
fn run_stage(p: &Prepared, zt: &mut Zt, a: &Params) -> Result<Value> {
    let layer = a.layer.trim().to_string();
    if !["link-check", "organize", "skeleton", "figures", "bridge"].contains(&layer.as_str()) {
        bail!("`{layer}` is not a stage-owned publication (link-check, organize, skeleton, figures, bridge)");
    }
    if !prepared_layers(p)?.contains(&layer) {
        return Ok(json!({"layer": layer, "state": "excluded", "reason": "the prepared run excluded this layer"}));
    }
    let artifacts = artifact_paths(&layer, a)?;
    let coverage = if !a.coverage.trim().is_empty() {
        util::read_json(&a.coverage)?.as_array().context("coverage must be a JSON array")?.clone()
    } else {
        match layer.as_str() {
            "organize" | "skeleton" => derived_coverage(p, zt, &layer, &a.applied)?,
            "figures" => {
                let prepared = util::read_json(must(&p.contract["artifacts"]["figures_file"], "path")?)?;
                let recognized = ps::ps_items(&prepared["figures"]);
                let report = util::read_json(&a.report).context("the figures report is not readable")?;
                figures_coverage(&recognized, &report)
            }
            // A proposal of the immutable Bridge report is open until it is applied: accepting one is the
            // review decision (mode bridge with -bridgeApply), never digest work left undone. The apply
            // state beside the report names the accepted ones.
            "bridge" => {
                let report = util::read_json(&a.report).context("the Bridge report is not readable")?;
                let state = Path::new(&a.report).parent().map(|work| work.join("state.json")).filter(|path| path.is_file());
                let state = match state {
                    Some(path) => util::read_json(&path).context("the Bridge apply state is not readable")?,
                    None => Value::Null,
                };
                bridge_coverage(&report, &state)
            }
            _ => Vec::new(),
        }
    };
    // The stage reads this line through a relay, so the checked coverage comes back as counts per
    // outcome; the evidence file and the ledger keep every entry.
    let mut done = complete_publication(p, zt, &layer, stage_scope(&layer, &coverage), &artifacts, &coverage)?;
    let mut counts = BTreeMap::<String, u64>::new();
    for entry in ps::ps_items(&done["coverage"]) {
        *counts.entry(ps::s(&entry["outcome"])).or_default() += 1;
    }
    done["coverage"] = json!(counts);
    Ok(done)
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

/// `-Step changes`: the retained changed-Card manifest, read-only (S26, #169). A resume uses it to
/// continue a finalization whose correction cycle and scope are already recorded, without
/// re-entering the one permitted cycle. It also reports whether the branch already published its
/// final audit (the scheduling audit is published with no refresh scope; the final one carries the
/// `finalize_refresh` scope) and whether every applicable Term-link round completed.
fn run_changes(p: &Prepared, a: &Params) -> Result<Value> {
    let file = if a.changes.trim().is_empty() {
        changes_path(&p.out)
    } else {
        PathBuf::from(&a.changes)
    };
    let mut changes = util::read_json(&file).with_context(|| {
        format!(
            "no changed-Card manifest at {}: the correction cycle has not been recorded",
            file.display()
        )
    })?;
    let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
    let mut finalized = false;
    if let Some((unit, _)) = ledger::required_publication(&conn, "audit")? {
        let inputs: Option<String> = conn
            .query_row("SELECT inputs_json FROM units WHERE id=?1", [&unit], |row| {
                row.get(0)
            })
            .optional()?;
        let inputs: Value = inputs
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or(Value::Null);
        finalized = inputs["scope"]["kind"] == json!("finalize_refresh");
    }
    let round = &changes["term_link"];
    if finalized && round["applicable"] == json!(true) && round["scope"] != json!("none") {
        let number = round["round"].as_i64().unwrap_or(0);
        finalized = unit_state(&conn, &format!("termlink/final@{number}"))? == "done";
    }
    changes["step"] = json!("changes");
    changes["finalized"] = json!(finalized);
    Ok(changes)
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
            sample_seed: 1,
            baseline: String::new(),
            concept_report: String::new(),
            decisions: String::new(),
            run_report: String::new(),
            task_sheet: String::new(),
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
        // The checker's own `(advisory)` tag decides advice (S06, R0187).
        if ps::field(fault, "line").contains("(advisory)") || rule.eq_ignore_ascii_case(NOTE_RULE) {
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
    // S26 (#169, R0438/R0442): the finalization round's own Term-link pass, read from the ledger
    // the workflow claimed it in. A round the workflow never ran, or one whose apply was refused,
    // is infrastructure debt: the final audit ran, but the book's Term links are not verified.
    let round_scope = changes["term_link"].clone();
    let term_round = round_scope["round"].as_i64().unwrap_or(scheduled_revision);
    let mut term_link = json!({
        "applicable": round_scope["applicable"],
        "round": term_round,
        "scope": round_scope["scope"],
        "cards": round_scope["cards"],
        "mapping_changed": round_scope["mapping_changed"],
        "reasons": round_scope["reasons"],
        "unit": Value::Null,
        "state": Value::Null,
        "status": "skipped",
        "counts": Value::Null,
        "no_op": false,
        "reason": round_scope["reason"],
    });
    if round_scope["applicable"] == json!(true) && round_scope["scope"] != json!("none") {
        let unit = format!("termlink/final@{term_round}");
        let conn = ledger::connect(&p.out.join("ledger.sqlite3"), false)?;
        let row: Option<(String, Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT state,fault_json,result_json FROM units WHERE id=?1",
                [&unit],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        term_link["unit"] = json!(unit);
        match row {
            None => {
                term_link["status"] = json!("failed");
                term_link["reason"] = json!("the round's Term-link unit was never registered: the final audit ran before any relink");
                infrastructure.push(json!({
                    "layer": "term-link",
                    "unit": unit,
                    "state": "missing",
                    "reason": "the finalization round's Term-link pass did not run",
                }));
            }
            Some((state, fault, result)) => {
                let fault: Value = fault
                    .as_deref()
                    .and_then(|text| serde_json::from_str(text).ok())
                    .unwrap_or(Value::Null);
                term_link["state"] = json!(state);
                if !fault.is_null() {
                    term_link["fault"] = fault.clone();
                }
                if let Some(result) = result
                    .as_deref()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                {
                    term_link["counts"] = result["counts"].clone();
                    term_link["no_op"] = result["no_op"].clone();
                }
                if state == "done" {
                    term_link["status"] = json!("complete");
                    term_link["reason"] = json!("");
                } else {
                    let code = fault["code"].as_str().unwrap_or_default();
                    let refused = code == "card_check" || code == "card_locked";
                    term_link["status"] = json!(if refused { "refused" } else { "degraded" });
                    term_link["reason"] = json!(format!(
                        "the round's Term-link unit is {state}{}",
                        if code.is_empty() { String::new() } else { format!(" ({code})") }
                    ));
                    infrastructure.push(json!({
                        "layer": "term-link",
                        "unit": unit,
                        "state": state,
                        "code": if code.is_empty() { Value::Null } else { json!(code) },
                        "reason": term_link["reason"],
                    }));
                }
            }
        }
    }
    by_class.insert("infrastructure".to_string(), infrastructure.len() as i64);
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
        // S26 (#169, R0438/R0442): the round that relinked the corrected Cards before this final
        // audit, and the audit's own Term quality. `audit ran` never means `all terms passed`.
        "term_link": term_link,
        "terms": {
            "status": book["terms"]["status"],
            "rows": book["terms"]["rows"], "published": book["terms"]["published"],
            "missing": book["terms"]["missing"], "heads": book["terms"]["heads"],
            "parts": book["terms"]["parts"], "unfiled": book["terms"]["unfiled"],
            "uses": book["terms"]["uses"]["total"], "faults": book["terms"]["faults"]["total"],
            "reason": book["terms"]["reason"],
        },
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
    let term = &report["term_link"];
    if term["applicable"] == json!(true) {
        let scope = if term["scope"] == json!("book") {
            "the whole book (the Term mapping changed)".to_string()
        } else if term["scope"] == json!("cards") {
            format!("{} changed Card(s)", ps::ps_items(&term["cards"]).len())
        } else {
            "no eligible Card".to_string()
        };
        let counts = &term["counts"];
        text.push_str(&format!(
            "Term-link round {}: **{}**, scope {}; unit `{}` is {}{}{}.\n\n",
            term["round"],
            ps::field(term, "status"),
            scope,
            ps::field(term, "unit"),
            ps::field(term, "state"),
            if term["no_op"] == json!(true) { " (proven no-op)" } else { "" },
            if counts.is_object() {
                format!(
                    "; {} Card(s) changed, {} link(s) added",
                    counts["cards_changed"], counts["links_added"]
                )
            } else {
                String::new()
            }
        ));
    } else {
        text.push_str(&format!(
            "Term-link round {}: skipped ({}).\n\n",
            term["round"],
            ps::field(term, "reason")
        ));
    }
    let terms = &report["terms"];
    text.push_str(&format!(
        "Term quality of the final audit: **{}** ({} row(s), {} published, {} head(s), {} part(s), {} use(s), {} fault(s)).\n\n",
        ps::field(terms, "status"),
        terms["rows"],
        terms["published"],
        terms["heads"],
        terms["parts"],
        terms["uses"],
        terms["faults"],
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
    let step = a.choice("Step", &["plan", "scope", "changes", "refresh", "stage", "final"])?;
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
        "changes" => run_changes(&prepared, &p)?,
        "refresh" => run_refresh(&prepared, &mut zt, &p)?,
        "stage" => run_stage(&prepared, &mut zt, &p)?,
        _ => run_final(&prepared, &mut zt, &p)?,
    };
    util::print_line(&ps::json_compact(&result))?;
    Ok(0)
}

#[cfg(test)]
mod stage_tests {
    use super::{bridge_coverage, figures_coverage, stage_scope};

    #[test]
    fn bridge_coverage_names_the_accepted_proposals_of_this_report() {
        let report = json!({"report_id":"r-1","proposals":[{"id":"b001"},{"id":"b002"},{"id":"b003"}]});
        let state = json!({"report_id":"r-1","accepted":{"new":["b002"],"earlier":["b003"]}});
        let coverage = bridge_coverage(&report, &state);
        assert_eq!(coverage[0], json!({"ref":"b001","outcome":"open"}));
        assert_eq!(coverage[1], json!({"ref":"b002","outcome":"accepted"}));
        assert_eq!(coverage[2], json!({"ref":"b003","outcome":"accepted"}));
        let other = json!({"report_id":"r-2","accepted":{"new":["b001"]}});
        assert!(bridge_coverage(&report, &other).iter().all(|entry| entry["outcome"] == json!("open")));
        assert!(bridge_coverage(&report, &serde_json::Value::Null).iter().all(|entry| entry["outcome"] == json!("open")));
    }

    #[test]
    fn a_stage_scope_changes_with_its_coverage_only() {
        let unmatched = vec![json!({"ref":"a.jpg","outcome":"unmatched","reason":"no label"})];
        let shown = vec![json!({"ref":"a.jpg","outcome":"shown","path":"p","sha256":"aa"})];
        assert_eq!(stage_scope("figures", &unmatched), stage_scope("figures", &unmatched.clone()));
        assert_ne!(stage_scope("figures", &unmatched), stage_scope("figures", &shown));
        assert_ne!(stage_scope("figures", &shown), stage_scope("bridge", &shown));
    }
    use serde_json::json;

    // A stage publication's Figures coverage: a figure a Card names is shown (by its alt text or a label),
    // one no Card names, or with no label, is unmatched with its reason.
    #[test]
    fn figures_coverage_classifies_every_recognized_figure() {
        let recognized = vec![
            json!({"name":"a.jpg","labels":["III","三"],"alt":"图 III","path":"p/a.jpg","sha256":"aa"}),
            json!({"name":"b.jpg","labels":["VI"],"alt":"","path":"p/b.jpg","sha256":"bb"}),
            json!({"name":"c.jpg","labels":[],"alt":"","path":"p/c.jpg","sha256":"cc"}),
            json!({"name":"d.jpg","labels":["IV"],"alt":"图 IV","path":"p/d.jpg","sha256":"dd"}),
        ];
        let report = json!({"cards":[{"figures":["图 III"]},{"figures":["图 VI"]}]});
        let coverage = figures_coverage(&recognized, &report);
        assert_eq!(coverage[0], json!({"ref":"a.jpg","outcome":"shown","path":"p/a.jpg","sha256":"aa"}));
        assert_eq!(coverage[1]["outcome"], json!("shown"));
        assert_eq!(coverage[2]["outcome"], json!("unmatched"));
        assert!(coverage[2]["reason"].as_str().unwrap().contains("no label"));
        assert_eq!(coverage[3]["outcome"], json!("unmatched"));
        assert!(coverage[3]["reason"].as_str().unwrap().contains("图 IV"));
        // ebrill2001modern: a figure a Card already shows is shown, and one outside the digested range is
        // not a gap, whatever its labels.
        let report = json!({"cards":[], "shown":["cc","dd"], "outside":["bb"]});
        let coverage = figures_coverage(&recognized, &report);
        assert_eq!(coverage[0]["outcome"], json!("unmatched"));
        assert_eq!(coverage[1], json!({"ref":"b.jpg","outcome":"outside","reason":"the image lies outside the digested range of the source"}));
        assert_eq!(coverage[2]["outcome"], json!("shown"));
        assert_eq!(coverage[3]["outcome"], json!("shown"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(address: &str, rule: &str, text: &str) -> Value {
        // The line carries the checker's own advisory tag, as an audit line does.
        let tag = if zt_check::policy::severity(rule) == zt_check::core::Severity::Advise {
            "(advisory) "
        } else {
            ""
        };
        json!({"address": address, "rule": rule, "text": text, "line": format!("{address} {rule} {tag}{text}")})
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
            // The plan's per-Card rules are the blocking ones, as the plan builds them.
            if ps::field(fault, "line").contains("(advisory)") {
                continue;
            }
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

    fn fingerprints(list: &str, mapping: &str, autolink: &str, vocabulary: &str) -> Value {
        json!({"list": list, "mapping": mapping, "autolink": autolink, "vocabulary": vocabulary})
    }

    #[test]
    fn a_body_only_correction_relinks_its_exact_changed_cards() {
        let current = fingerprints("l1", "m1", "a1", "v1");
        let audit = fingerprints("l1", "m1", "a1", "v1");
        let eligible = vec!["Key/1|a".to_string(), "Key/1|b".to_string()];
        let block = term_link_round(
            true,
            3,
            Some("Key/1|t"),
            None,
            Some(&current),
            &[("audit", &audit)],
            &eligible,
            &[],
            false,
        );
        assert_eq!(block["applicable"], json!(true));
        assert_eq!(block["round"], json!(3));
        assert_eq!(block["scope"], json!("cards"));
        assert_eq!(block["cards"], json!(["Key/1|a", "Key/1|b"]));
        assert_eq!(block["mapping_changed"], json!(false));
    }

    #[test]
    fn a_changed_term_mapping_replans_the_whole_book_with_the_round() {
        // A changed Term list, head name or auto-link policy moves one of the four fingerprints;
        // the round must not limit itself to the correction cycle's body edits (R0439).
        let current = fingerprints("l2", "m1", "a1", "v1");
        let audit = fingerprints("l1", "m1", "a1", "v1");
        let block = term_link_round(
            true,
            2,
            Some("Key/1|t"),
            None,
            Some(&current),
            &[("audit", &audit)],
            &["Key/1|a".to_string()],
            &[],
            false,
        );
        assert_eq!(block["scope"], json!("book"));
        assert_eq!(block["cards"], json!([]));
        assert_eq!(block["mapping_changed"], json!(true));
        assert!(
            ps::field(&block, "reasons").contains("\"list\"") || format!("{}", block["reasons"]).contains("list"),
            "{}",
            block["reasons"]
        );
        // A changed auto-link choice is the same book-wide effect.
        let autolink = fingerprints("l1", "m1", "a2", "v1");
        let block = term_link_round(
            true,
            2,
            Some("Key/1|t"),
            None,
            Some(&current),
            &[("whole-book plan", &autolink)],
            &["Key/1|a".to_string()],
            &[],
            false,
        );
        assert_eq!(block["scope"], json!("book"));
        assert!(format!("{}", block["reasons"]).contains("autolink"), "{}", block["reasons"]);
        // The corrected Term list itself is a mapping change whatever the fingerprints say.
        let block = term_link_round(
            true,
            2,
            Some("Key/1|t"),
            None,
            Some(&current),
            &[("audit", &current)],
            &[],
            &[],
            true,
        );
        assert_eq!(block["scope"], json!("book"));
        assert!(format!("{}", block["reasons"]).contains("Term list"), "{}", block["reasons"]);
    }

    #[test]
    fn a_round_with_no_applied_mapping_replans_the_whole_book_or_states_why_it_cannot() {
        // No audit fingerprint and no whole-book plan: the safe scope is the whole book.
        let current = fingerprints("l1", "m1", "a1", "v1");
        let block = term_link_round(
            true,
            1,
            Some("Key/1|t"),
            None,
            Some(&current),
            &[],
            &["Key/1|a".to_string()],
            &[],
            false,
        );
        assert_eq!(block["scope"], json!("book"));
        assert!(format!("{}", block["reasons"]).contains("no applied Term mapping"));
        // A disabled layer, a book without a Term list, and a mapping that cannot be derived each
        // state their own reason instead of pretending there is nothing to relink.
        let off = term_link_round(false, 1, None, None, None, &[], &[], &[], false);
        assert_eq!(off["applicable"], json!(false));
        assert!(ps::field(&off, "reason").contains("excluded the Term-link layer"));
        let no_list = term_link_round(true, 1, None, Some("tree `Key` has no Term list"), None, &[], &[], &[], false);
        assert_eq!(no_list["applicable"], json!(false));
        assert!(ps::field(&no_list, "reason").contains("no Term list"));
        let unavailable = term_link_round(true, 1, Some("Key/1|t"), Some("duplicate rows"), None, &[], &[], &[], false);
        assert_eq!(unavailable["applicable"], json!(false));
        assert!(ps::field(&unavailable, "reason").contains("duplicate rows"));
    }

    #[test]
    fn an_unchanged_mapping_with_no_eligible_change_is_a_stated_no_op() {
        let current = fingerprints("l1", "m1", "a1", "v1");
        let block = term_link_round(
            true,
            4,
            Some("Key/1|t"),
            None,
            Some(&current),
            &[("audit", &current)],
            &[],
            &[json!({"address": "Key/1|s", "reason": "structure"})],
            false,
        );
        assert_eq!(block["applicable"], json!(true));
        assert_eq!(block["scope"], json!("none"));
        assert!(ps::field(&block, "reason").contains("no eligible Card changed"));
        assert_eq!(block["excluded"], json!([{"address": "Key/1|s", "reason": "structure"}]));
    }
}
