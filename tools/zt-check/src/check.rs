//! `zt-check`: the command-line input adapter of the Card quality check. It reads
//! the drafts (`--json`), or the live Cards through one `zt mcp` (`--model`,
//! `--addresses`, `--cards`/`--log`), materializes the term table, level, and
//! Workspace title context, hands them to the process-free core, and prints the
//! familiar fault lines.
//!
//! The rules themselves live in `core`, the text semantics in `text`, and the
//! report policy in `policy`; this module owns only the inputs, the output, and
//! the archive client. It is compiled only with the `cli` feature.

use crate::core::{Check, Diagnostic, Limits};
use crate::mcp::Zt;
use crate::policy::{Applicability, CheckReport, DEFAULT_RULES, LIST_SPLIT, ratchet, select_rules};
use crate::text::{
    TitleContext, BOOK_ANCHOR, BOOK_CARD, culture, key, prop, ps_eq, ps_in, ps_string, text,
    utf16_form,
};
use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::Path;

// The public library surface the digest and reports already call. The items now
// live in `core` and `text`; these re-exports keep every existing caller working.
pub use crate::core::source_evidence;
pub use crate::text::{
    TITLE_SIMILARITY, TermRow, sentence_fits, term_table, title_bigrams, title_fits, title_key,
    title_similarity,
};

const USAGE: &str = "usage:
  zt-check --version
  zt-check --json <file> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --draft <file> [--report <file>] [--workspace human|agent] [--setting <address>] [limits]
  zt-check --model <address> [--skip-shared] [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --addresses <a,b,...> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,R3,...] [limits]
every mode: [--workspace human|agent] [--report <file>]
limits: --max-sentences 12 --max-chars 400 --max-steps 6 --max-explain-sentences 16
        --max-explain-chars 600 --min-idea-chars 40 --max-idea-chars 300 --min-reason 8
exit: 0 complete and no blocking fault, 1 complete with a blocking fault,
      2 invalid, incomplete, stale contract, or infrastructure failure";

// --- inputs -----------------------------------------------------------------------------------

/// `--json`: the established entry format (a JSON array of entries, one entry, or a
/// bundle whose `context` gives I6 its read-only Workspace titles).
///
/// The returned count is the number of blocking diagnostics, the count every mode
/// publishes as `faults: N` (design 8.1, R0187).
#[cfg(test)]
fn check_json(
    raw: &str,
    setting: &str,
    limits: Limits,
    require_explain: bool,
    level: &str,
) -> Result<(Vec<String>, usize)> {
    let options = Options {
        setting: (!setting.is_empty()).then(|| setting.to_string()),
        level: level.to_string(),
        require_explain,
        limits,
        ..Options::default()
    };
    let outcome = check_draft(raw, InputKind::Json, &options)?;
    let lines = outcome.report.lines();
    Ok((lines, outcome.report.blockers))
}

/// Which draft format a file holds: the established entry array (`--json`) or the
/// exact proposed-write bundle whose Cards carry their own full text (`--draft`,
/// design 8.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InputKind {
    Json,
    Draft,
}

/// One draft as the bundle declares it: the candidate Cards, the context they were
/// checked against, the Workspace they belong to, the check families the bundle
/// declares complete, and the rule/policy binding it pins (design 8.2).
pub(crate) struct Bundle {
    pub(crate) cards: Vec<Value>,
    pub(crate) context: TitleContext,
    pub(crate) supplied_context: bool,
    pub(crate) context_complete: bool,
    pub(crate) workspace: Option<String>,
    pub(crate) declared: Option<Vec<String>>,
    pub(crate) rule_version: Option<String>,
    pub(crate) policy_hash: Option<String>,
}

/// Read a draft file. `--json` keeps its established entry format; `--draft` reads
/// the exact proposed-write bundle of design 8.2.
pub(crate) fn read_bundle(raw: &str, kind: InputKind) -> Result<Bundle> {
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let value: Value = serde_json::from_str(raw).context("the file is not JSON")?;
    let (cards, bundle) = match value {
        Value::Object(map) if map.contains_key("cards") => {
            let cards = match map.get("cards") {
                Some(Value::Array(items)) => items.clone(),
                Some(other) => vec![other.clone()],
                None => Vec::new(),
            };
            (cards, map)
        }
        Value::Array(items) => (items, Map::new()),
        other => (vec![other], Map::new()),
    };
    let cards = match kind {
        InputKind::Json => cards,
        InputKind::Draft => cards
            .iter()
            .enumerate()
            .map(|(index, card)| draft_entry(card, index))
            .collect::<Result<Vec<_>>>()?,
    };
    let (context, supplied_context, context_complete) = draft_context(&bundle, kind)?;
    let declared = bundle
        .get("context")
        .and_then(|context| context.get("complete_for"))
        .map(|names| {
            names
                .as_array()
                .map(|names| {
                    names
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        });
    let text = |name: &str| {
        bundle
            .get(name)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    };
    Ok(Bundle {
        cards,
        context,
        supplied_context,
        context_complete,
        workspace: text("workspace").map(|workspace| workspace.to_lowercase()),
        declared,
        rule_version: text("rule_version"),
        policy_hash: text("policy_hash"),
    })
}

/// The Workspace title context a bundle supplies, and what it declares about it.
fn draft_context(bundle: &Map<String, Value>, kind: InputKind) -> Result<(TitleContext, bool, bool)> {
    let Some(context) = bundle.get("context").filter(|context| context.is_object()) else {
        return Ok((TitleContext::default(), false, false));
    };
    if kind == InputKind::Json && context.get("cards").is_none() {
        // The established `--json` envelope: `{complete, titles}`.
        return Ok((TitleContext::from_json(context), true, context["complete"] == true));
    }
    let cards = context
        .get("cards")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if cards.is_empty() && context.get("titles").is_none() {
        return Ok((TitleContext::default(), false, false));
    }
    let titles = cards
        .iter()
        .map(|card| {
            json!({
                "address": card.get("address").cloned().unwrap_or(Value::Null),
                "title": card.get("title").cloned().unwrap_or(Value::Null),
                "workspace": card.get("workspace").cloned().unwrap_or(Value::Null),
                "type": card.get("type").cloned().unwrap_or(Value::Null),
                "facet": card.get("facet").cloned().unwrap_or(Value::Null),
                "restatement": card.get("restatement").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    let complete = declared(context, "graph");
    Ok((
        TitleContext::from_json(&json!({ "complete": complete, "titles": titles })),
        true,
        complete,
    ))
}

/// Whether a bundle declares one check family complete.
fn declared(context: &Value, family: &str) -> bool {
    context
        .get("complete_for")
        .and_then(Value::as_array)
        .is_some_and(|names| names.iter().any(|name| name.as_str() == Some(family)))
}

/// One `--draft` Card: the exact Card text with its type fields (design 8.2). An
/// address the caller cannot know yet stays provisional, so a fault line can name
/// the draft without inventing an allocation (R0195).
fn draft_entry(card: &Value, index: usize) -> Result<Value> {
    let address = ps_string(prop(card, "address"));
    let address = if address.trim().is_empty() {
        format!("draft:{}", index + 1)
    } else {
        address
    };
    let text_value = text_of(card, "text");
    if text_value.trim().is_empty() {
        bail!("draft Card `{address}` has no `text`: a draft carries the exact Card text");
    }
    let (title, body) =
        split_card_text(&text_value).with_context(|| format!("draft Card `{address}`"))?;
    let mut entry = Map::new();
    entry.insert("address".into(), json!(address));
    entry.insert("title".into(), json!(title));
    entry.insert("body".into(), json!(body));
    let declared_type = text_of(card, "type");
    let declared_category = text_of(card, "category");
    let (kind, other) = if declared_type.is_empty() {
        (declared_category.clone(), String::new())
    } else {
        (declared_type.clone(), declared_category.clone())
    };
    if !other.is_empty() && !ps_eq(&other, &kind) {
        bail!(
            "draft Card `{address}` names both a type `{declared_type}` and a category `{declared_category}`; name its type once"
        );
    }
    if !kind.trim().is_empty() {
        entry.insert(
            "category".into(),
            json!(draft_category(&kind).with_context(|| format!("draft Card `{address}`"))?),
        );
    }
    // Every other entry fact the rules read travels unchanged; the draft names the
    // exact proposed write, not a second input language.
    for name in [
        "facet",
        "level",
        "line",
        "book",
        "parent",
        "links",
        "kinds",
        "relations",
        "sources",
        "workspace",
        "inbound",
        "inbound_kinded",
        "term",
        "before",
    ] {
        if let Some(value) = prop(card, name) {
            entry.insert(name.to_string(), value.clone());
        }
    }
    Ok(Value::Object(entry))
}

/// The title and body of one exact Card text: the first line is `# <title>`, the rest
/// is the authored body. This is the envelope fact a draft check can verify offline.
fn split_card_text(text: &str) -> Result<(String, String)> {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines = text.split('\n');
    let heading = lines.next().unwrap_or_default();
    let title = heading
        .strip_prefix("# ")
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    if title.is_empty() {
        bail!("the text must open with `# <title>`");
    }
    let rest = lines.collect::<Vec<_>>().join("\n");
    let body = rest.strip_prefix('\n').unwrap_or(&rest).to_string();
    Ok((title, body))
}

/// The layer role a draft's type names. The Term type is the design's own example;
/// its T rules are pending, so it takes the R rules like a Hub Card does.
fn draft_category(kind: &str) -> Result<&'static str> {
    match kind.trim().to_ascii_lowercase().as_str() {
        "idea" => Ok("idea"),
        "event" => Ok("event"),
        "finding" => Ok("finding"),
        "model" => Ok("model"),
        "hub" => Ok("hub"),
        "term" => Ok("term"),
        other => bail!("unknown draft type `{other}`; expected idea, event, finding, model, hub, or term"),
    }
}

/// What one draft run decided, and the facts its status document publishes.
#[derive(Debug)]
pub(crate) struct DraftOutcome {
    pub(crate) report: CheckReport,
    pub(crate) missing: Vec<String>,
    pub(crate) reasons: Vec<String>,
    pub(crate) workspace: String,
    pub(crate) contract: crate::contract::Contract,
}

/// Check one draft under the checker's built-in policy and report exactly which
/// families the answer covers (design 8.2, US021). An unknown family, a stale rule
/// binding, or two different Workspaces is an invalid invocation, never a pass.
fn check_draft(raw: &str, kind: InputKind, options: &Options) -> Result<DraftOutcome> {
    let mut bundle = read_bundle(raw, kind)?;
    let workspace = match (bundle.workspace.clone(), options.workspace.clone()) {
        (Some(bundle), Some(flag)) if !ps_eq(&bundle, &flag) => bail!(
            "the draft names workspace {bundle} and --workspace names {flag}; name one Workspace"
        ),
        (Some(bundle), _) => bundle,
        (None, flag) => flag.unwrap_or_else(|| "agent".to_string()),
    };
    if let Some(rule_version) = &bundle.rule_version
        && !ps_eq(rule_version, crate::contract::RULE_VERSION)
    {
        bail!(
            "stale check contract: the draft binds rule version {rule_version}, this zt-check uses {}",
            crate::contract::RULE_VERSION
        );
    }
    let contract = crate::contract::Contract::built_in(&options.limits);
    let policy_bound = bundle
        .policy_hash
        .as_deref()
        .map(|hash| ps_eq(hash, &contract.policy_hash));
    let declared = bundle.declared.clone().unwrap_or_else(|| match kind {
        InputKind::Json => vec!["local".to_string()],
        InputKind::Draft => vec!["envelope".to_string(), "local".to_string()],
    });
    for family in &declared {
        if !crate::contract::FAMILIES.contains(&family.as_str()) {
            bail!(
                "unknown check family `{family}`; a draft declares {}",
                crate::contract::FAMILIES.join(", ")
            );
        }
    }
    let mut missing = Vec::new();
    let mut reasons = Vec::new();
    for family in crate::contract::FAMILIES {
        let is_declared = declared.iter().any(|name| name == family);
        let backed = match family {
            "envelope" => kind == InputKind::Draft,
            "graph" => bundle.supplied_context && bundle.context_complete,
            "policy" => policy_bound == Some(true),
            _ => true,
        };
        if is_declared && backed {
            continue;
        }
        missing.push(family.to_string());
        reasons.push(if is_declared {
            match family {
                "envelope" => "the entry format carries no exact Card text, so its envelope is unchecked".to_string(),
                "graph" => "the graph checks need a complete Workspace context (`context.cards` with `complete_for` graph)".to_string(),
                "policy" => "the draft binds no policy hash, or one other than this checker's built-in policy; a live check or a bound archive policy hash decides it".to_string(),
                _ => String::new(),
            }
        } else {
            match family {
                "placement" => "placement (the allocated address, Link targets, Access and Locks) needs a service preview; a self-check reserves no future write".to_string(),
                "policy" => "the draft carries no archive policy; the answer uses the checker's built-in defaults".to_string(),
                "graph" => "the bundle declares no complete Workspace context".to_string(),
                "envelope" => "the bundle does not carry the exact Card text".to_string(),
                _ => format!("the bundle does not declare `{family}` complete"),
            }
        });
    }
    for card in &mut bundle.cards {
        if ps_string(prop(card, "workspace")).is_empty() {
            card["workspace"] = json!(workspace);
        }
    }
    let setting = utf16_form(options.setting.as_deref().unwrap_or_default());
    let setting_body = (!setting.is_empty())
        .then(|| {
            bundle
                .cards
                .iter()
                .find(|card| ps_eq(&text(prop(card, "address")), &setting))
                .map(|card| ps_string(prop(card, "body")))
        })
        .flatten();
    let shared = bundle
        .cards
        .iter()
        .filter(|card| ps_eq(&text_of(card, "line"), "shared"))
        .map(|card| ps_string(prop(card, "address")))
        .collect();
    let check = Check::new(
        options.limits.clone(),
        options.require_explain,
        &options.level,
    )
    .with_terms(setting_body.as_deref())
    .with_shared(shared)
    .with_context(bundle.context)
    .with_policies(contract.policy.clone());
    let report = check_report(&check, &bundle.cards)?;
    Ok(DraftOutcome {
        report,
        missing,
        reasons,
        workspace,
        contract,
    })
}

/// Check the proposed Cards, applying the legacy ratchet when the bundle pairs any of
/// them with its old authored version (design 6.3).
fn check_report(check: &Check, cards: &[Value]) -> Result<CheckReport> {
    let before = before_entries(cards)?;
    if before.is_empty() {
        return Ok(CheckReport::of(check.report(cards)));
    }
    let old_cards = before
        .iter()
        .map(|(_, entry)| entry.clone())
        .collect::<Vec<_>>();
    let paired = before
        .iter()
        .map(|(proposed, entry)| (ps_string(prop(entry, "address")), proposed.clone()))
        .collect::<Vec<_>>();
    let mut old_report = check.report(&old_cards);
    // A `before` entry is this Card's own old version even when the proposed Card
    // moves to another address: pair the old diagnostics by the bundle's pairing,
    // so a pure move does not make unchanged faults new (design 6.3, R0149).
    for diagnostic in &mut old_report {
        if let Some((_, proposed)) = paired
            .iter()
            .find(|(old, _)| ps_eq(old, &diagnostic.address))
            && !ps_eq(&diagnostic.address, proposed)
        {
            diagnostic.address = proposed.clone();
        }
    }
    let proposed_report = check.report(cards);
    let applicability = before
        .iter()
        .filter_map(|(address, entry)| {
            let proposed = cards
                .iter()
                .find(|card| ps_eq(&ps_string(prop(card, "address")), address))?;
            Some((
                address.clone(),
                (Applicability::of(entry), Applicability::of(proposed)),
            ))
        })
        .collect::<HashMap<_, _>>();
    Ok(ratchet(&old_report, proposed_report, |diagnostic| {
        applicability
            .get(&diagnostic.address)
            .is_some_and(|(before, proposed)| before.comparable(proposed, &diagnostic.rule))
    }))
}

/// The old authored version each entry declares under `before`, paired with the
/// address of the Card it edits. A `before` that is absent or null means a new
/// Card, which has no legacy allowance.
fn before_entries(cards: &[Value]) -> Result<Vec<(String, Value)>> {
    let mut entries = Vec::new();
    for card in cards {
        let Some(before) = card.get("before") else {
            continue;
        };
        if before.is_null() {
            continue;
        }
        let mut entry = before
            .as_object()
            .cloned()
            .with_context(|| "the bundle's `before` must be an entry object")?;
        let address = ps_string(prop(card, "address"));
        entry
            .entry("address".to_string())
            .or_insert_with(|| json!(address.clone()));
        entries.push((address, Value::Object(entry)));
    }
    Ok(entries)
}

fn text_of(card: &Value, name: &str) -> String {
    text(prop(card, name))
}

/// A Card as card-style-check.ps1 reads it from get_card: address, title, body, facet.
/// The stored Workspace travels with it: a live check decides each Card under its own
/// Workspace's policy, and no flag may reclassify it (design 8.1, R0186).
fn card_entry(card: &Value) -> Value {
    let mut entry = json!({
        "address": card.get("address").cloned().unwrap_or(Value::Null),
        "title": card.get("title").cloned().unwrap_or(Value::Null),
        "body": card.get("body").cloned().unwrap_or(Value::Null),
        "facet": card.get("facet").cloned().unwrap_or(Value::Null),
    });
    if let Some(workspace) = card.get("workspace").and_then(Value::as_str)
        && !workspace.is_empty()
    {
        entry["workspace"] = json!(workspace);
    }
    entry
}

/// The body of the setting Card: from the Cards checked, else from zt.
fn setting_body(zt: &mut Zt, cards: &[Value], setting: &str) -> Result<Option<String>> {
    if setting.is_empty() {
        return Ok(None);
    }
    let wanted = utf16_form(setting);
    if let Some(card) = cards
        .iter()
        .find(|card| ps_eq(&text(prop(card, "address")), &wanted))
    {
        return Ok(Some(ps_string(prop(card, "body"))));
    }
    let read = zt.cards(&[setting.to_string()], "text")?;
    Ok(Some(ps_string(read.cards[0].get("body"))))
}

/// What one live run checked under the archive contract, and what it knows it did
/// not cover. `missing` is empty only when the answer is the gate's own.
#[derive(Default)]
pub(crate) struct Checked {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) missing: Vec<String>,
    pub(crate) reasons: Vec<String>,
}

/// The served check contract: the running service's published facts and the archive
/// revision they were read at (design 8.3). A client that cannot read them does not
/// guess.
pub(crate) struct Served {
    pub(crate) contract: crate::contract::Contract,
    pub(crate) revision: u64,
}

/// Read the running service's contract from the one verified empty `get_cards` view,
/// and refuse an answer whose rules or limits this binary does not share (R0206).
pub(crate) fn served(zt: &mut Zt) -> Result<Served> {
    let view = zt.call("get_cards", json!({ "addresses": [], "view": "text" }))?;
    if view["complete"] != true {
        bail!("incomplete check contract: the empty archive view was not complete");
    }
    if view["cards"].as_array().is_none_or(|cards| !cards.is_empty()) {
        bail!("incomplete check contract: the empty archive view returned Cards");
    }
    let Some(published) = view.get("check_contract").filter(|value| value.is_object()) else {
        bail!(
            "incomplete check contract: the running service publishes none; deploy the zt build that matches zt-check"
        );
    };
    let contract = crate::contract::Contract::read(published)
        .map_err(|error| anyhow::anyhow!("incomplete check contract: {error}"))?;
    let gaps = contract.version_gaps();
    if !gaps.is_empty() {
        bail!("stale check contract: {}", gaps.join("; "));
    }
    // A stdio proxy and the daemon it serves can be different builds. The proxy's
    // own declaration is compared with the served contract, so neither stands in
    // for the other (R0198).
    let initialized = zt.initialization();
    for (name, served) in [
        ("rule_version", &contract.rule_version),
        ("diagnostic_schema", &contract.diagnostic_schema),
        ("applicability_version", &contract.applicability_version),
    ] {
        match initialized["check_contract"].get(name).and_then(Value::as_str) {
            Some(declared) if ps_eq(declared, served) => {}
            Some(declared) => bail!(
                "stale check contract: the zt mcp proxy declares {name} {declared}, the running service's read reply says {served}"
            ),
            None => bail!(
                "incomplete check contract: the zt mcp proxy declares no {name}; update the zt on PATH"
            ),
        }
    }
    let revision = view["archive_revision"]
        .as_u64()
        .context("incomplete check contract: the archive view names no revision")?;
    Ok(Served { contract, revision })
}

/// A live check may not reclassify a stored Card: the flag must agree with every
/// Card's own Workspace, or the answer would claim a permission the owner never gave
/// (design 8.1, R0186).
fn workspace_gaps(cards: &[Value], flag: &str) -> Vec<String> {
    if flag.is_empty() {
        return Vec::new();
    }
    cards
        .iter()
        .filter_map(|card| {
            let address = ps_string(prop(card, "address"));
            let workspace = ps_string(prop(card, "workspace"));
            (!workspace.is_empty() && !ps_eq(&workspace, flag)).then(|| {
                format!("`{address}` is a {workspace} Card; --workspace {flag} cannot reclassify a stored Card")
            })
        })
        .collect()
}

/// The limits a run uses must be the limits the contract publishes, or its answer is
/// not the gate's (design 8.1 and 8.3).
fn limit_gaps(contract: &crate::contract::Contract, options: &Options) -> Vec<String> {
    contract.limit_gaps(&options.limits)
}

/// `--model`: every Card of the complete model context, with its setting's
/// term table and level. An external setting uses a required lean read.
fn check_model(
    zt: &mut Zt,
    at: &str,
    options: &Options,
    served: &Served,
) -> Result<Checked> {
    let revision = served.revision;
    let structure = zt.call("get_model", json!({ "at": at }))?;
    structure["head"]["address"]
        .as_str()
        .with_context(|| format!("`{at}` belongs to no model"))?;
    let mut cards = vec![card_entry(&structure["head"])];
    for part in structure["parts"].as_array().into_iter().flatten() {
        cards.push(card_entry(part));
    }
    let setting = if options.setting.as_deref().unwrap_or_default().is_empty() {
        ps_string(structure["build"].get("setting"))
    } else {
        options.setting.clone().unwrap_or_default()
    };
    let level = if options.level.is_empty() {
        ps_string(structure.get("level"))
    } else {
        options.level.clone()
    };
    // A shared part follows the term table of its home model, not this one.
    let shared = structure["build"]["order"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| ps_eq(&ps_string(item.get("line")), "shared"))
        .map(|item| ps_string(item.get("address")))
        .collect::<Vec<_>>();
    if options.skip_shared {
        cards.retain(|card| !ps_in(&ps_string(prop(card, "address")), &shared));
    }
    let body = setting_body(zt, &cards, &setting)?;
    let check = Check::new(options.limits.clone(), options.require_explain, &level)
        .with_terms(body.as_deref())
        .with_shared(shared)
        .with_policies(served.contract.policy.clone());
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    let mut checked = Checked {
        diagnostics: check.report(&cards),
        ..Checked::default()
    };
    checked.reasons = workspace_gaps(&cards, options.workspace.as_deref().unwrap_or_default());
    if !checked.reasons.is_empty() {
        checked.missing.push("workspace".to_string());
    }
    Ok(checked)
}

/// The Cards a Workspace's register declares under `词条分册`, in both Workspaces (§8.5, D188). A
/// Card whose address one root declares is a register part: a navigation Card that carries term
/// rows. A scope that reads no register (`get_register` refuses an unknown Workspace) reports none.
pub(crate) fn declared_register_parts(zt: &mut Zt) -> Result<HashSet<String>> {
    let mut parts = HashSet::new();
    for workspace in ["agent", "human"] {
        let view = zt.call(
            "get_register",
            json!({ "workspace": workspace, "entry_offset": 0, "entry_limit": 1 }),
        )?;
        if view["register_missing"] == true {
            continue;
        }
        for part in view["parts"].as_array().into_iter().flatten() {
            if part["place"] != "ok" {
                continue;
            }
            if let Some(address) = part["address"].as_str()
                && !address.is_empty()
            {
                parts.insert(address.to_string());
            }
        }
    }
    Ok(parts)
}

/// `--cards`: self-check.ps1. Each Card as digest-audit.ps1 sees it: a Model
/// part or Model Card with its model (setting, level, and every Derivation
/// part with an explanation); an Idea Card, an anchor, an Event Card, or a
/// Finding Card with
/// its layer rules; any other Card with the R rules only. Returns every
/// diagnostic, before the rule filter, under the archive policy.
fn check_cards(
    zt: &mut Zt,
    addresses: &[String],
    options: &Options,
    served: &Served,
) -> Result<Checked> {
    let revision = served.revision;
    let book = options.book.as_deref().unwrap_or_default();
    let limits = &options.limits;
    let batch = zt.cards(addresses, "text")?;
    if batch.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    let catalogue = zt.call("export_titles", json!({}))?;
    let titles = catalogue["titles"]
        .as_array()
        .context("incomplete checker classification catalogue")?;
    if catalogue["complete"] != true
        || catalogue["count"].as_u64() != Some(titles.len() as u64)
        || catalogue["archive_revision"].as_u64() != Some(batch.archive_revision)
    {
        bail!("incomplete checker classification: changing or partial title view");
    }
    // The same complete title projection is I6's Workspace context: one bulk read.
    let context = TitleContext::from_titles(titles);
    let literature: HashSet<String> = titles
        .iter()
        .filter_map(|card| card["tree"].as_str())
        .filter(|tree| BOOK_CARD.is_match(tree))
        .map(str::to_string)
        .collect();
    let mut anchors: HashMap<String, usize> = HashMap::new();
    for title in titles {
        let address = title["address"]
            .as_str()
            .context("incomplete checker classification address")?;
        if BOOK_ANCHOR.is_match(address) {
            *anchors
                .entry(
                    title["tree"]
                        .as_str()
                        .context("incomplete checker classification tree")?
                        .to_string(),
                )
                .or_default() += 1;
        }
    }
    let mut cards = batch.cards;
    // The parts a register declares under 词条分册 are navigation Cards, not substantive Ideas
    // (§8.5, D188): the root declaration is the one source of that category.
    let register_parts = declared_register_parts(zt)?;
    let mut owners: HashMap<String, Option<String>> = HashMap::new();
    let mut models: HashMap<String, Value> = HashMap::new();
    for card in &mut cards {
        if card["type"] == "model" {
            let at = ps_string(card.get("address"));
            if !owners.contains_key(&at) {
                let model = zt.call("get_model", json!({"at":at}))?;
                let head = model["head"]["address"].as_str().map(str::to_string);
                owners.insert(at.clone(), head.clone());
                if let Some(head) = head {
                    owners.insert(head.clone(), Some(head.clone()));
                    // A shared part may have a different primary owner. Only
                    // filed parts are known to have this model as their home.
                    for part in model["parts"].as_array().into_iter().flatten() {
                        if part["filed"] == true {
                            owners.insert(ps_string(part.get("address")), Some(head.clone()));
                        }
                    }
                    models.insert(head, model);
                }
            }
            if let Some(Some(head)) = owners.get(&at) {
                let model = &models[head];
                card["model"] = json!({"head":{"address":head},"level":model["level"],"build":{"setting":model["build"]["setting"]}});
            }
        }
    }
    let mut by_model: Vec<(String, Vec<Value>)> = Vec::new();
    let mut plain = Vec::new();
    for card in &cards {
        let address = ps_string(card.get("address"));
        let tree = ps_string(card.get("tree"));
        let title = ps_string(card.get("title"));
        let body = ps_string(card.get("body"));
        // Valid Topic tree ids are numeric; Literature Citation keys start with a letter.
        let in_book = card["workspace"] == "agent" && BOOK_CARD.is_match(&tree);
        let category = crate::graph::category_with_parts(card, &literature, &anchors, &register_parts);
        let category = category.as_str();
        let head = card["model"]["head"]["address"]
            .as_str()
            .map(str::to_string);
        if matches!(category, "part" | "model")
            && let Some(head) = head
        {
            match by_model
                .iter_mut()
                .find(|(known, _)| key(known) == key(&head))
            {
                Some((_, members)) => members.push(card.clone()),
                None => by_model.push((head, vec![card.clone()])),
            }
            continue;
        }
        let mut item = Map::new();
        item.insert("address".into(), json!(address));
        item.insert("title".into(), json!(title));
        item.insert("body".into(), json!(body));
        item.insert("facet".into(), Value::Null);
        if matches!(category, "idea" | "anchor" | "event" | "finding" | "restatement-entry") {
            let parent = ps_string(card.get("parent_address"));
            let links = card["links"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .filter(|link| !link.is_empty())
                .collect::<Vec<_>>();
            let kinds = card["kinds"]
                .as_object()
                .map(|kinds| kinds.keys().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            item.insert("category".into(), json!(category));
            item.insert("tree".into(), json!(tree));
            // I6 compares only inside one Workspace, so the entry says which one it is.
            item.insert("workspace".into(), card["workspace"].clone());
            item.insert("parent".into(), json!(parent));
            item.insert("links".into(), json!(links));
            item.insert("kinds".into(), json!(kinds));
            // The relationship evidence of I7: the kind word of every kinded Link.
            item.insert("relations".into(), card["kinds"].clone());
            let key = if !book.is_empty() {
                book.to_string()
            } else if in_book {
                tree.clone()
            } else {
                String::new()
            };
            if !key.is_empty() {
                item.insert("book".into(), json!(key));
            }
        }
        plain.push(Value::Object(item));
    }
    let mut diagnostics = Vec::new();
    // Model parts and Model Cards: per model, with its setting (term table) and
    // level, as card-style-check.ps1 -Addresses reads them from the first Card.
    for (_, members) in &by_model {
        let model = &members[0]["model"];
        let setting = ps_string(model["build"].get("setting"));
        let level = ps_string(model.get("level"));
        let checked = members.iter().map(card_entry).collect::<Vec<_>>();
        let body = setting_body(zt, &checked, &setting)?;
        let check = Check::new(limits.clone(), true, &level)
            .with_terms(body.as_deref())
            .with_policies(served.contract.policy.clone());
        diagnostics.extend(check.report(&checked));
    }
    if !plain.is_empty() {
        diagnostics.extend(
            Check::new(limits.clone(), false, "")
                .with_context(context)
                .with_policies(served.contract.policy.clone())
                .report(&plain),
        );
    }
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    let mut checked = Checked {
        diagnostics,
        ..Checked::default()
    };
    checked.reasons = workspace_gaps(&cards, options.workspace.as_deref().unwrap_or_default());
    if !checked.reasons.is_empty() {
        checked.missing.push("workspace".to_string());
    }
    Ok(checked)
}

// --- the command ------------------------------------------------------------------------------

#[derive(Default)]
struct Options {
    json: Option<String>,
    draft: Option<String>,
    model: Option<String>,
    addresses: Option<String>,
    cards: Option<String>,
    log: Option<String>,
    book: Option<String>,
    rules: Option<String>,
    setting: Option<String>,
    level: String,
    /// The Workspace a draft belongs to, or the live override a stored Card must
    /// already agree with (design 8.1, R0183 and R0186).
    workspace: Option<String>,
    /// Where to write the structured check status (design 8.1, R0185).
    report: Option<String>,
    require_explain: bool,
    skip_shared: bool,
    limits: Limits,
}

impl Options {
    fn parse(args: &[String]) -> Result<Self> {
        let mut options = Options::default();
        let mut index = 0;
        while index < args.len() {
            let flag = args[index].as_str();
            let mut value = || -> Result<String> {
                index += 1;
                args.get(index)
                    .cloned()
                    .with_context(|| format!("{flag} needs a value\n{USAGE}"))
            };
            let number = |text: String| -> Result<i64> {
                text.trim()
                    .parse::<i64>()
                    .with_context(|| format!("{flag} needs a whole number, not `{text}`"))
            };
            match flag {
                "--json" => options.json = Some(value()?),
                "--draft" => options.draft = Some(value()?),
                "--model" => options.model = Some(value()?),
                "--addresses" => options.addresses = Some(value()?),
                "--cards" => options.cards = Some(value()?),
                "--log" => options.log = Some(value()?),
                "--book" => options.book = Some(value()?),
                "--rules" => options.rules = Some(value()?),
                "--setting" => options.setting = Some(value()?),
                "--report" => options.report = Some(value()?),
                "--workspace" => {
                    let workspace = value()?.to_lowercase();
                    if !ps_in(&workspace, &["human", "agent"]) {
                        bail!("--workspace is human or agent, not `{workspace}`");
                    }
                    options.workspace = Some(workspace);
                }
                "--level" => {
                    let level = value()?;
                    if !ps_in(&level, &["", "concept", "formal"]) {
                        bail!("--level is concept or formal, not `{level}`");
                    }
                    options.level = level.to_lowercase();
                }
                "--require-explain" => options.require_explain = true,
                "--skip-shared" => options.skip_shared = true,
                "--max-sentences" => options.limits.max_sentences = number(value()?)?,
                "--max-chars" => options.limits.max_chars = number(value()?)?,
                "--max-steps" => options.limits.max_steps = number(value()?)?,
                "--max-explain-sentences" => {
                    options.limits.max_explain_sentences = number(value()?)?
                }
                "--max-explain-chars" => options.limits.max_explain_chars = number(value()?)?,
                "--min-idea-chars" => options.limits.min_idea_chars = number(value()?)?,
                "--max-idea-chars" => options.limits.max_idea_chars = number(value()?)?,
                "--min-reason" => options.limits.min_reason = number(value()?)?,
                "-h" | "--help" => bail!("{USAGE}"),
                other => bail!("unknown option `{other}`\n{USAGE}"),
            }
            index += 1;
        }
        let modes = [
            options.json.is_some(),
            options.draft.is_some(),
            options.model.is_some(),
            options.addresses.is_some(),
            options.cards.is_some() || options.log.is_some(),
        ]
        .into_iter()
        .filter(|on| *on)
        .count();
        if modes != 1 {
            bail!("name one input: --json, --draft, --model, --addresses, or --cards/--log\n{USAGE}");
        }
        let cards_mode = options.cards.is_some() || options.log.is_some();
        if cards_mode
            && (options.setting.is_some()
                || !options.level.is_empty()
                || options.require_explain
                || options.skip_shared)
        {
            bail!(
                "--cards takes each model's own setting and level; --setting, --level, --require-explain, and --skip-shared do not apply"
            );
        }
        if !cards_mode && (options.book.is_some() || options.rules.is_some()) {
            bail!("--book and --rules apply to --cards only");
        }
        if options.skip_shared && options.model.is_none() {
            bail!("--skip-shared applies to --model only");
        }
        Ok(options)
    }
}

/// `--addresses`: card-style-check.ps1 -Addresses. The Cards from lean batches;
/// the term table from --setting or the first Card's model setting; the level
/// from --level or the first Card's model.
fn check_addresses(
    zt: &mut Zt,
    addresses: &[String],
    options: &Options,
    served: &Served,
) -> Result<Checked> {
    let revision = served.revision;
    let batch = zt.cards(addresses, "text")?;
    if batch.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    let read = batch.cards;
    let first = match read.first().filter(|card| card["type"] == "model") {
        Some(card) => zt.call("get_model", json!({"at":card["address"]}))?,
        None => Value::Null,
    };
    let setting = if options.setting.as_deref().unwrap_or_default().is_empty() {
        ps_string(first["build"].get("setting"))
    } else {
        options.setting.clone().unwrap_or_default()
    };
    let level = if options.level.is_empty() {
        ps_string(first.get("level"))
    } else {
        options.level.clone()
    };
    let cards = read.iter().map(card_entry).collect::<Vec<_>>();
    let body = setting_body(zt, &cards, &setting)?;
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    let mut checked = Checked {
        diagnostics: Check::new(options.limits.clone(), options.require_explain, &level)
            .with_terms(body.as_deref())
            .with_policies(served.contract.policy.clone())
            .report(&cards),
        ..Checked::default()
    };
    checked.reasons = workspace_gaps(&cards, options.workspace.as_deref().unwrap_or_default());
    if !checked.reasons.is_empty() {
        checked.missing.push("workspace".to_string());
    }
    Ok(checked)
}

/// The checker's own identity: a report that records the software version of a run
/// reads it here instead of guessing it from a path or a file date.
fn version() -> String {
    format!("zt-check {}", crate::VERSION)
}

/// The structured check status of one run (design 8.1, R0185): the verdict, the
/// check families the answer covers, the counts, and the contract it was computed
/// under. `--report <file>` writes it beside the ordinary fault lines, so a caller
/// reads what the lines cannot say.
#[derive(Clone, Debug)]
pub struct Status {
    /// `live`, `draft` (the entry format), or `draft_bundle` (the exact-write bundle).
    pub mode: &'static str,
    /// `pass`, `block`, or `incomplete`.
    pub verdict: &'static str,
    /// The check families or facts this answer did not cover.
    pub missing: Vec<String>,
    /// Why each missing family is missing.
    pub reasons: Vec<String>,
    /// The blocking diagnostics after the policy, which `faults: N` publishes.
    pub blocking: usize,
    /// The advisory diagnostics, printed with `(advisory)` and not counted.
    pub advisory: usize,
    /// The Workspace a draft belongs to, or the flag a live check had to agree with.
    pub workspace: String,
    /// `archive` (the running service's policy), `built_in` (the checker's defaults),
    /// or `unverified` when no contract could be read.
    pub policy_source: &'static str,
    /// The published check contract (design 8.3).
    pub contract: Value,
    /// The gate stage's own decision, when an archive answered.
    pub gate: Value,
    /// Every diagnostic under the structured contract.
    pub diagnostics: Vec<Value>,
}

impl Status {
    /// Whether the evaluation covered everything its mode promises.
    pub fn complete(&self) -> bool {
        self.missing.is_empty()
    }

    /// The process exit contract (design 8.1, R0188–R0191): 0 for a complete
    /// evaluation with no blocking fault, 1 for a complete evaluation with one, and
    /// 2 for an invalid, incomplete, stale, or infrastructure outcome.
    pub fn exit_code(&self) -> u8 {
        match self.verdict {
            "pass" => 0,
            "block" => 1,
            _ => 2,
        }
    }

    /// The `--report` document.
    pub fn to_json(&self) -> Value {
        json!({
            "mode": self.mode,
            "verdict": self.verdict,
            "complete": self.complete(),
            "missing": self.missing,
            "reasons": self.reasons,
            "blocking": self.blocking,
            "advisory": self.advisory,
            "workspace": self.workspace,
            "policy_source": self.policy_source,
            "contract": self.contract,
            "gate": self.gate,
            "diagnostics": self.diagnostics,
        })
    }
}

/// One run's lines and status. The lines are the ordinary fault lines and
/// `faults: N`; the status carries what they cannot.
pub struct Run {
    pub lines: Vec<String>,
    pub status: Status,
}

impl Run {
    /// The process exit code this run answers (design 8.1, R0188–R0191).
    pub fn exit_code(&self) -> u8 {
        self.status.exit_code()
    }
}

fn verdict(missing: &[String], blocking: usize) -> &'static str {
    if !missing.is_empty() {
        "incomplete"
    } else if blocking > 0 {
        "block"
    } else {
        "pass"
    }
}

/// The fault lines, then the one count every mode publishes: the blocking
/// diagnostics after policy and ratchet (design 8.1, R0187).
fn report_lines(report: &CheckReport) -> Vec<String> {
    let mut lines = report.lines();
    lines.push(format!("faults: {}", report.blockers));
    lines
}

/// The gate's own decision for the archive's stage: under `advisory` a blocking
/// policy setting reports the line and refuses nothing.
fn gate_json(contract: &crate::contract::Contract) -> Value {
    let enforcing = contract.stage == "enforce";
    json!({
        "stage": contract.stage,
        "would_refuse": enforcing,
        "note": if enforcing {
            "the archive's gate stage is enforce: a blocking diagnostic refuses the write"
        } else {
            "the archive's gate stage is advisory: it reports these lines and refuses no write"
        },
    })
}

fn diagnostics_json(report: &CheckReport) -> Vec<Value> {
    report
        .diagnostics
        .iter()
        .map(Diagnostic::to_json)
        .collect()
}

/// A live run's answer under the running service's contract.
fn live_run(served: &Served, checked: Checked, options: &Options) -> Run {
    let report = CheckReport::of(checked.diagnostics);
    let blocking = report.blockers;
    let status = Status {
        mode: "live",
        verdict: verdict(&checked.missing, blocking),
        missing: checked.missing,
        reasons: checked.reasons,
        blocking,
        advisory: report.diagnostics.len() - blocking,
        workspace: options
            .workspace
            .clone()
            .unwrap_or_else(|| "each stored Card's own".to_string()),
        policy_source: crate::contract::POLICY_SOURCE_ARCHIVE,
        contract: served.contract.publish(),
        gate: gate_json(&served.contract),
        diagnostics: diagnostics_json(&report),
    };
    Run {
        lines: report_lines(&report),
        status,
    }
}

/// A draft's answer: the checker's built-in policy, and exactly the families the
/// bundle declared complete.
fn draft_run(mode: &'static str, outcome: DraftOutcome) -> Run {
    let report = outcome.report;
    let blocking = report.blockers;
    let status = Status {
        mode,
        verdict: verdict(&outcome.missing, blocking),
        missing: outcome.missing,
        reasons: outcome.reasons,
        blocking,
        advisory: report.diagnostics.len() - blocking,
        workspace: outcome.workspace,
        policy_source: crate::contract::POLICY_SOURCE_BUILT_IN,
        contract: outcome.contract.publish(),
        gate: Value::Null,
        diagnostics: diagnostics_json(&report),
    };
    Run {
        lines: report_lines(&report),
        status,
    }
}

/// A live run whose contract could not be read or does not match this binary: it
/// prints the mismatch and refuses to predict acceptance (R0206). No rule is
/// reported, because the rules that would decide are not known to be the same.
fn unverified(options: &Options, reason: &str) -> Run {
    let contract = crate::contract::Contract::built_in(&options.limits).publish();
    let status = Status {
        mode: "live",
        verdict: "incomplete",
        missing: vec!["contract".to_string()],
        reasons: vec![reason.to_string()],
        blocking: 0,
        advisory: 0,
        workspace: options
            .workspace
            .clone()
            .unwrap_or_else(|| "each stored Card's own".to_string()),
        policy_source: "unverified",
        contract,
        gate: Value::Null,
        diagnostics: Vec::new(),
    };
    Run {
        lines: vec![
            format!("check: incomplete: {reason}"),
            "faults: 0".to_string(),
        ],
        status,
    }
}

/// `zt-check …`: the lines, the optional `--report` document, and the exit code.
pub fn run_cli(args: &[String]) -> Result<u8> {
    // `--version` is answered before any archive is read: a review records the checker
    // that produced its lines, and a copied binary must be able to state which one it is.
    if args.len() == 1 && matches!(args[0].to_ascii_lowercase().as_str(), "--version" | "-v") {
        println!("{}", version());
        return Ok(0);
    }
    let options = Options::parse(args)?;
    let run = run(args, None)?;
    write_lines(&run.lines)?;
    if let Some(path) = &options.report {
        write_status(path, &run.status)?;
    }
    Ok(run.exit_code())
}

/// The structured status document, written whole so a reader never sees half of one.
fn write_status(path: &str, status: &Status) -> Result<()> {
    let mut document =
        serde_json::to_string_pretty(&status.to_json()).context("the check status is not JSON")?;
    document.push('\n');
    std::fs::write(path, document).with_context(|| format!("failed to write {path}"))?;
    Ok(())
}

/// One run of `zt-check …`: its lines (the ordinary fault lines and `faults: N`)
/// and the status that explains them. With `zt`, the reads go to that `zt mcp`;
/// without it, a new one starts when the mode reads the archive.
pub fn run(args: &[String], zt: Option<&mut Zt>) -> Result<Run> {
    fn server<'a>(shared: Option<&'a mut Zt>, own: &'a mut Option<Zt>) -> Result<&'a mut Zt> {
        match shared {
            Some(zt) => Ok(zt),
            None => Ok(own.insert(Zt::start()?)),
        }
    }
    let mut own: Option<Zt> = None;
    let options = Options::parse(args)?;
    if let Some(path) = &options.json {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
        let outcome = check_draft(&raw, InputKind::Json, &options)?;
        return Ok(draft_run("draft", outcome));
    }
    if let Some(path) = &options.draft {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
        let outcome = check_draft(&raw, InputKind::Draft, &options)?;
        return Ok(draft_run("draft_bundle", outcome));
    }
    // The `--cards` list is read before any archive call, so an empty list still
    // answers without a service, as it always has.
    let cards_mode = options.addresses.is_none() && options.model.is_none();
    let cards_list = if cards_mode {
        let mut list = LIST_SPLIT
            .split(options.cards.as_deref().unwrap_or_default())
            .filter(|at| !at.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        if let Some(log) = options.log.as_deref().filter(|log| Path::new(log).exists()) {
            let raw =
                std::fs::read_to_string(log).with_context(|| format!("failed to read {log}"))?;
            let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
            list.extend(
                raw.split(['\r', '\n'])
                    .map(str::trim)
                    .filter(|at| !at.is_empty())
                    .map(str::to_string),
            );
        }
        // Select-Object -Unique: case counts, ignorable characters do not.
        let mut unique: Vec<String> = Vec::new();
        for at in list {
            if !unique.iter().any(|seen| culture(seen) == culture(&at)) {
                unique.push(at);
            }
        }
        unique
    } else {
        Vec::new()
    };
    if cards_mode && cards_list.is_empty() {
        return Ok(Run {
            lines: vec!["no Cards to check".to_string(), "faults: 0".to_string()],
            status: Status {
                mode: "live",
                verdict: "pass",
                missing: Vec::new(),
                reasons: vec!["no Cards to check".to_string()],
                blocking: 0,
                advisory: 0,
                workspace: options
                    .workspace
                    .clone()
                    .unwrap_or_else(|| "each stored Card's own".to_string()),
                policy_source: "unverified",
                contract: crate::contract::Contract::built_in(&options.limits).publish(),
                gate: Value::Null,
                diagnostics: Vec::new(),
            },
        });
    }
    let zt = server(zt, &mut own)?;
    let served = match served(zt) {
        Ok(served) => served,
        Err(error) => return Ok(unverified(&options, &format!("{error:#}"))),
    };
    let mut missing = Vec::new();
    let mut reasons = Vec::new();
    let gaps = limit_gaps(&served.contract, &options);
    if !gaps.is_empty() {
        missing.push("limits".to_string());
        reasons.extend(gaps);
    }
    let mut checked = if let Some(list) = &options.addresses {
        let list = LIST_SPLIT
            .split(list)
            .filter(|at| !at.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        check_addresses(zt, &list, &options, &served)?
    } else if let Some(at) = &options.model {
        check_model(zt, at, &options, &served)?
    } else {
        let checked = check_cards(zt, &cards_list, &options, &served)?;
        let rules = options.rules.as_deref().unwrap_or(DEFAULT_RULES);
        Checked {
            diagnostics: select_rules(checked.diagnostics, rules),
            ..checked
        }
    };
    checked.missing.splice(0..0, missing);
    checked.reasons.splice(0..0, reasons);
    Ok(live_run(&served, checked, &options))
}

/// The lines `zt-check …` prints: one per fault, then `faults: N`. With `zt`,
/// the reads go to that `zt mcp`; without it, a new one starts when needed.
pub fn run_lines(args: &[String], zt: Option<&mut Zt>) -> Result<Vec<String>> {
    Ok(run(args, zt)?.lines)
}

/// One line each, UTF-8 whatever the console code page, with the line ends
/// PowerShell writes (CRLF on Windows).
fn write_lines(lines: &[String]) -> Result<()> {
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push_str(newline);
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()?;
    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const CARD_STYLE_EXPECTED: &str = include_str!("../tests/fixtures/card-style-expected.txt");
    const CARD_LAYER_FIXTURE: &str =
        include_str!("../../../scripts/book-digest/tests/card-layer-fixture.json");
    const CARD_LAYER_EXPECTED: &str = include_str!("../tests/fixtures/card-layer-expected.txt");
    const ENVELOPE_SINGLE: &str = include_str!("../tests/fixtures/envelope/single-object.json");
    const ENVELOPE_BOM: &str = include_str!("../tests/fixtures/envelope/bom-array.json");
    const ENVELOPE_BUNDLE: &str = include_str!("../tests/fixtures/envelope/bundle-context.json");
    const ENVELOPE_CASE: &str = include_str!("../tests/fixtures/envelope/case-folded.json");
    const ENVELOPE_SHARED: &str = include_str!("../tests/fixtures/envelope/shared-part.json");
    const ENVELOPE_NOT_JSON: &str = include_str!("../tests/fixtures/envelope/not-json.json");
    const CARD_STYLE_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../scripts/book-digest/tests/card-style-fixture.json"
    );

    /// The CLI adapter adds the `faults:` count to the core's lines; the helper
    /// does the same so one recorded list compares against the adapter directly.
    fn printed(mut lines: Vec<String>, faults: usize) -> Vec<String> {
        lines.push(format!("faults: {faults}"));
        lines
    }

    fn json_lines(raw: &str, setting: &str, limits: Limits, require_explain: bool, level: &str) -> Vec<String> {
        let (lines, faults) = check_json(raw, setting, limits, require_explain, level).unwrap();
        printed(lines, faults)
    }

    fn run_json(cards: &Value, setting: &str, limits: Limits, require_explain: bool, level: &str) -> Vec<String> {
        json_lines(&cards.to_string(), setting, limits, require_explain, level)
    }

    fn of_rule<'a>(lines: &'a [String], rule: &str) -> Vec<&'a str> {
        lines
            .iter()
            .map(String::as_str)
            .filter(|line| line.split(' ').nth(1) == Some(rule))
            .collect()
    }

    #[test]
    fn the_cli_reads_the_fixture_into_the_recorded_ordered_lines() {
        let args = [
            "--json".to_string(),
            CARD_STYLE_PATH.to_string(),
            "--setting".to_string(),
            "K/1|a|a".to_string(),
        ];
        let lines = run_lines(&args, None).unwrap();
        assert_eq!(lines, CARD_STYLE_EXPECTED.lines().collect::<Vec<_>>());
    }

    /// The Card layer fixture run through the CLI adapter: the same 16 ordered
    /// lines the standalone tool printed before the extraction.
    #[test]
    fn the_layer_fixture_lines_are_the_recorded_golden_lines() {
        let limits = Limits { max_idea_chars: 250, ..Limits::default() };
        let lines = json_lines(CARD_LAYER_FIXTURE, "", limits, false, "");
        assert_eq!(lines, CARD_LAYER_EXPECTED.lines().collect::<Vec<_>>());
    }

    /// The `--json` envelope keeps its old parsing: one object or a BOM'd array is
    /// the same entries, and property names ignore case.
    #[test]
    fn the_json_envelope_keeps_its_parsing() {
        let single = json_lines(ENVELOPE_SINGLE, "", Limits::default(), false, "");
        assert_eq!(single, ["E/1 R3 甲；乙。", "faults: 1"]);
        assert_eq!(json_lines(&format!("[{ENVELOPE_SINGLE}]"), "", Limits::default(), false, ""), single);
        assert_eq!(json_lines(ENVELOPE_BOM, "", Limits::default(), false, ""), ["E/2 R3 甲；乙。", "faults: 1"]);
        assert_eq!(json_lines(ENVELOPE_CASE, "", Limits::default(), false, ""), ["E/5 R3 甲；乙。", "faults: 1"]);
    }

    /// The bundle envelope still supplies the read-only Workspace title context.
    #[test]
    fn the_bundle_envelope_supplies_the_title_context() {
        assert_eq!(
            json_lines(ENVELOPE_BUNDLE, "", Limits::default(), false, ""),
            [
                "E/3 I2 (2 characters < 40) merge candidate",
                "E/3 I6 (advisory) a title very similar to [[E/4]]",
                "faults: 1",
            ]
        );
    }

    /// A setting names the term table, a shared part follows its home model's table,
    /// and a setting that is not in the file leaves no table.
    #[test]
    fn the_setting_and_the_shared_line_keep_their_meaning() {
        let limits = Limits::default();
        assert_eq!(
            json_lines(ENVELOPE_SHARED, "S/1", limits.clone(), false, ""),
            ["S/3 R6 (advisory) uses `卖资产` for `清算`: 双方卖资产。", "faults: 0"]
        );
        assert_eq!(json_lines(ENVELOPE_SHARED, "S/9", limits, false, ""), ["faults: 0"]);
    }

    /// A file that is not JSON and a file that cannot be read keep their messages.
    #[test]
    fn the_json_errors_keep_their_messages() {
        let error = check_json(ENVELOPE_NOT_JSON, "", Limits::default(), false, "").unwrap_err();
        assert!(format!("{error:#}").contains("the file is not JSON"), "{error:#}");
        let error = run_lines(&["--json".to_string(), "Z:/no/such/draft.json".to_string()], None).unwrap_err();
        assert!(format!("{error:#}").contains("failed to read"), "{error:#}");
    }

    #[test]
    fn two_inputs_are_refused_before_any_read() {
        let error = run_lines(
            &["--json".to_string(), "x".to_string(), "--cards".to_string(), "y".to_string()],
            None,
        )
        .unwrap_err();
        assert!(format!("{error:#}").contains("name one input"), "{error:#}");
    }

    #[test]
    fn an_empty_card_list_prints_no_faults_line() {
        assert_eq!(
            run_lines(&["--cards".to_string(), String::new()], None).unwrap(),
            ["no Cards to check", "faults: 0"]
        );
    }


    /// A restatement entry is navigation with its own checks (§4.5, D086): it takes no claim-title
    /// (I1) or Idea-size (I2) rule, and its title is no duplicate-title candidate (I6), while the
    /// source line (I3) and the Link reason (L2) are checked as for any other Card.
    #[test]
    fn a_restatement_entry_is_navigation_with_its_own_source_and_link_checks() {
        let entry = json!({
            "address": "12/1|a", "category": "restatement-entry", "workspace": "agent", "tree": "12",
            "parent": "12/0", "book": "book",
            "title": "斯卢茨基分解的入口",
            "body": "## 同述入口\n\n- [[12/2]] 定义。\n\n来源：[[book]] §2.1",
            "links": ["12/2", "book"],
        });
        let idea = json!({
            "address": "12/3", "category": "idea", "workspace": "agent", "tree": "12",
            "parent": "12/0", "book": "book",
            "title": "斯卢茨基分解把两种效应分开", "body": "斯卢茨基分解把替代效应与收入效应分开，二者相加等于总效应。\n\n来源：[[book]] §2.1",
            "links": ["book"],
        });
        let bundle = json!({
            "cards": [entry, idea],
            "context": { "complete": true, "titles": [
                // An entry title is no duplicate-title candidate for a substantive Idea.
                { "address": "13/1|a", "title": "斯卢茨基分解把两种效应分开", "workspace": "agent", "restatement": true },
                { "address": "13/1|b", "title": "斯卢茨基分解把两种效应分开", "workspace": "agent" }
            ] }
        });
        let lines = run_json(&bundle, "", Limits::default(), false, "");
        for rule in ["I1", "I2", "I4"] {
            assert!(
                !of_rule(&lines, rule)
                    .iter()
                    .any(|line| line.starts_with("12/1|a ")),
                "an entry takes no {rule}: {lines:#?}"
            );
        }
        // The navigation Link of the entry section is no semantic Link: no L1 and no L2 for it,
        // while I3 and L3 still apply to the entry.
        assert!(
            !of_rule(&lines, "L1")
                .iter()
                .any(|line| line.starts_with("12/1|a "))
                && !of_rule(&lines, "L2")
                    .iter()
                    .any(|line| line.starts_with("12/1|a ")),
            "the entry section's Link takes no L1/L2: {lines:#?}"
        );
        assert!(
            !of_rule(&lines, "I3")
                .iter()
                .any(|line| line.starts_with("12/1|a ")),
            "the entry's source line names its book: {lines:#?}"
        );
        // I6 names the substantive title row, never the entry's title row.
        let i6 = of_rule(&lines, "I6");
        assert!(
            i6.iter().any(|line| line.contains("[[13/1|b]]")),
            "the substantive title is still a candidate: {lines:#?}"
        );
        assert!(
            !i6.iter().any(|line| line.contains("[[13/1|a]]")),
            "an entry title is no duplicate-title candidate: {lines:#?}"
        );
        // A source line that names no Link of the book is still I3, and a Link with no reason is L2.
        let missing = json!({
            "address": "12/1|b", "category": "restatement-entry", "workspace": "agent", "tree": "12",
            "parent": "12/0", "book": "book", "title": "另一个入口",
            "body": "## 同述入口\n\n- [[12/2]] 见上。\n", "links": ["12/2"],
        });
        let lines = run_json(&json!([missing]), "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "I3"),
            ["12/1|b I3 no source line: no Link to [[book]] or one of its anchors"],
            "{lines:#?}"
        );
        assert!(
            of_rule(&lines, "L2").is_empty(),
            "the section's own Link takes no L2: {lines:#?}"
        );
        // The entry section's own Link is navigation: it needs no Relation kind, while a Link
        // outside the section is still checked (L1 and L2).
        let navigation = json!({
            "address": "12/1|d", "category": "restatement-entry", "workspace": "agent", "tree": "12",
            "parent": "12/0", "book": "book", "title": "入口卡",
            "body": "本卡给出 [[12/2]] 的一个推论：价格不变时两项效应都为零。\n\n## 同述入口\n\n- [[12/2]] 见上。\n\n来源：[[book]] §2.1",
            "links": ["12/2", "book"], "kinds": {},
        });
        let lines = run_json(&json!([navigation]), "", Limits::default(), false, "");
        assert!(
            of_rule(&lines, "L1").iter().any(|line| line.starts_with("12/1|d L1 ")),
            "a Link outside the entry section keeps L1: {lines:#?}"
        );
        assert!(
            !of_rule(&lines, "L2")
                .iter()
                .any(|line| line.contains("见上")),
            "the entry section's own Link takes no L2: {lines:#?}"
        );        // The role is derived, never stored: the same body under an `idea` category is a claim.
        let claim = json!({
            "address": "12/1|c", "category": "idea", "workspace": "agent", "tree": "12",
            "parent": "12/0", "book": "book", "title": "入口",
            "body": "## 同述入口\n\n- [[12/2]] 定义。\n", "links": ["12/2", "book"],
        });
        let lines = run_json(&json!([claim]), "", Limits::default(), false, "");
        assert!(
            of_rule(&lines, "I1").iter().any(|line| line.starts_with("12/1|c I1 ")),
            "a claim with a short title keeps I1: {lines:#?}"
        );
    }

    /// The calibration review records the checker that produced its lines, so the
    /// binary states its own version rather than being identified by a path.
    #[test]
    fn the_checker_states_its_version() {
        assert_eq!(version(), format!("zt-check {}", env!("CARGO_PKG_VERSION")));
        // Any other argument set is still a check, never a version answer.
        assert!(run_lines(&["--version".to_string(), "--json".to_string()], None).is_err());
    }

    /// S02-AC02/US005: the bundle carries the old authored version, so a fault
    /// both versions carry is reported as legacy advice and does not count.
    #[test]
    fn the_before_bundle_keeps_a_retained_fault_as_advisory() {
        let bundle = json!({
            "cards": [{
                "address": "K/1", "title": "均衡稳定", "body": "甲；乙。",
                "before": { "title": "均衡稳定", "body": "甲；乙。" }
            }]
        });
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            ["K/1 R3 (advisory) 甲；乙。", "faults: 0"]
        );
    }

    /// S02-AC03: one retained occurrence does not excuse a second copy of the
    /// same faulty sentence; the duplicate keeps its blocking severity.
    #[test]
    fn a_second_copy_of_a_faulty_sentence_blocks_in_the_before_bundle() {
        let bundle = json!({
            "cards": [{
                "address": "K/1", "title": "均衡稳定", "body": "甲；乙。\n\n丙；丁。",
                "before": { "title": "均衡稳定", "body": "甲；乙。" }
            }]
        });
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            [
                "K/1 R3 (advisory) 甲；乙。",
                "K/1 R3 丙；丁。",
                "faults: 1",
            ]
        );
    }

    /// S02-AC05/US007: a Card retyped from Idea to Event must satisfy the Event
    /// format, even though the authored body did not change.
    #[test]
    fn a_retyped_card_must_satisfy_the_newly_applicable_event_rules() {
        let bundle = json!({
            "cards": [{
                "address": "K/3|a", "category": "event",
                "title": "通用汽车收购费雪车身公司", "body": "通用汽车收购了费雪车身公司。",
                "before": {
                    "category": "idea",
                    "title": "通用汽车收购费雪车身公司", "body": "通用汽车收购了费雪车身公司。"
                }
            }]
        });
        let lines = run_json(&bundle, "", Limits::default(), false, "");
        assert_eq!(
            of_rule(&lines, "E1"),
            ["K/3|a E1 no date in the title: 通用汽车收购费雪车身公司"]
        );
        assert_eq!(of_rule(&lines, "E2").len(), 1, "{lines:#?}");
        assert_eq!(lines.last().map(String::as_str), Some("faults: 2"));
    }

    /// S02-AC05: promoting a concept model to a formal one makes the formal
    /// Derivation rules newly applicable; old membership in a concept model does
    /// not grandfather the sections the formal part must now carry.
    #[test]
    fn a_concept_to_formal_change_blocks_the_newly_relevant_model_rules() {
        let bundle = json!({
            "cards": [{
                "address": "K/9/1|a", "category": "part", "facet": "推导", "level": "formal",
                "title": "推导", "body": "**思路**\n\n一个想法。",
                "before": {
                    "category": "part", "facet": "推导", "level": "concept",
                    "title": "推导", "body": "**思路**\n\n一个想法。"
                }
            }]
        });
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            [
                "K/9/1|a R15 sections 思路; want 思路, 起点, 步骤, 结果, 解读",
                "faults: 1",
            ]
        );
        // The same Card and body already formal on both sides keeps the fault as advice.
        let retained = json!({
            "cards": [{
                "address": "K/9/1|a", "category": "part", "facet": "推导", "level": "formal",
                "title": "推导", "body": "**思路**\n\n一个想法。",
                "before": {
                    "category": "part", "facet": "推导", "level": "formal",
                    "title": "推导", "body": "**思路**\n\n一个想法。"
                }
            }]
        });
        assert_eq!(
            run_json(&retained, "", Limits::default(), false, ""),
            [
                "K/9/1|a R15 (advisory) sections 思路; want 思路, 起点, 步骤, 结果, 解读",
                "faults: 0",
            ]
        );
    }

    /// S02-AC05/R0146: a role change compares only rules that still apply. A
    /// regular Card promoted to a Model part must meet the part size rule even
    /// though its body did not change; the reverse change drops the rule instead
    /// of inheriting it, and a facet change between two part roles keeps the size
    /// fault that still means the same thing as advice.
    #[test]
    fn a_role_change_grandfathers_only_the_rules_that_still_apply() {
        let long = |sentences: usize| {
            (1..=sentences)
                .map(|n| format!("第 {n} 句说明这一步的推理。"))
                .collect::<String>()
        };
        let promoted = json!({ "cards": [{
            "address": "K/17|a", "category": "part", "facet": "设定", "title": "设定", "body": long(13),
            "before": { "address": "K/17|a", "category": "idea", "title": "设定", "body": long(13) }
        }]});
        assert_eq!(
            run_json(&promoted, "", Limits::default(), false, ""),
            [
                "K/17|a R13 (13 sentences > 12) split the part into two or more parts",
                "faults: 1",
            ]
        );
        let demoted = json!({ "cards": [{
            "address": "K/17|b", "category": "idea", "title": "均衡稳定提高社会福利", "body": long(13),
            "before": { "address": "K/17|b", "category": "part", "facet": "设定", "title": "设定", "body": long(13) }
        }]});
        assert_eq!(
            run_json(&demoted, "", Limits::default(), false, ""),
            ["faults: 0"]
        );
        let refaceted = json!({ "cards": [{
            "address": "K/17|c", "category": "part", "facet": "设定", "title": "设定", "body": long(13),
            "before": {
                "address": "K/17|c", "category": "part", "facet": "推导：参与约束",
                "title": "推导：参与约束", "body": long(13)
            }
        }]});
        assert_eq!(
            run_json(&refaceted, "", Limits::default(), false, "formal"),
            [
                "K/17|c R13 (advisory) (13 sentences > 12) split the part into two or more parts",
                "faults: 0",
            ]
        );
    }

    /// S02-AC03: moving an unchanged faulty sentence or paragraph within the body
    /// keeps its semantic identity, so the retained fault stays advice.
    #[test]
    fn moving_a_faulty_paragraph_keeps_its_legacy_advice() {
        let bundle = json!({ "cards": [{
            "address": "K/18|a", "title": "均衡稳定提高社会福利",
            "body": "新的一段说明背景。\n\n甲；乙。",
            "before": {
                "address": "K/18|a", "title": "均衡稳定提高社会福利",
                "body": "甲；乙。\n\n新的一段说明背景。"
            }
        }]});
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            ["K/18|a R3 (advisory) 甲；乙。", "faults: 0"]
        );
    }

    /// S02-AC03/AC04: two long sentences that clip to the same prefix stay
    /// distinct occurrences, so a changed fragment that still faults blocks and
    /// never hides behind the old clipped line.
    #[test]
    fn a_changed_long_fragment_sharing_the_clipped_prefix_is_a_new_fault() {
        let head = "甲".repeat(80);
        let bundle = json!({ "cards": [{
            "address": "K/18|b", "title": "均衡稳定提高社会福利",
            "body": format!("{head}丙。"),
            "before": {
                "address": "K/18|b", "title": "均衡稳定提高社会福利",
                "body": format!("{head}乙。")
            }
        }]});
        let lines = run_json(&bundle, "", Limits::default(), false, "");
        assert_eq!(lines.len(), 2, "{lines:#?}");
        assert!(lines[0].starts_with("K/18|b R2 ("), "{}", lines[0]);
        assert!(lines[0].ends_with('…'), "the familiar line is clipped: {}", lines[0]);
        assert_eq!(lines[1], "faults: 1");
    }

    /// S02-AC02: the multiset is per Card. One Card's old occurrence never
    /// excuses a duplicate on another Card that carries the same sentence.
    #[test]
    fn the_ratchet_never_consumes_another_cards_old_occurrence() {
        let bundle = json!({ "cards": [
            { "address": "K/19|a", "title": "均衡稳定提高社会福利", "body": "甲；乙。",
              "before": { "address": "K/19|a", "title": "均衡稳定提高社会福利", "body": "甲；乙。" } },
            { "address": "K/19|b", "title": "均衡稳定提高收入", "body": "甲；乙。\n\n起头说明。甲；乙。",
              "before": { "address": "K/19|b", "title": "均衡稳定提高收入", "body": "甲；乙。" } }
        ]});
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            [
                "K/19|a R3 (advisory) 甲；乙。",
                "K/19|b R3 (advisory) 甲；乙。",
                "K/19|b R3 甲；乙。",
                "faults: 1",
            ]
        );
    }

    /// S02-AC06: a new Card in a bundle has no legacy allowance; it blocks beside
    /// a retained fault from an edited Card. An explicit `before: null` is a new
    /// Card too.
    #[test]
    fn a_new_card_in_the_bundle_has_no_legacy_allowance() {
        let bundle = json!({
            "cards": [
                { "address": "K/1", "title": "均衡稳定", "body": "甲；乙。",
                  "before": { "title": "均衡稳定", "body": "甲；乙。" } },
                { "address": "K/2", "title": "均衡唯一", "body": "丙；丁。" },
                { "address": "K/3", "title": "均衡稳定提高收入", "body": "戊；己。", "before": null }
            ]
        });
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            [
                "K/1 R3 (advisory) 甲；乙。",
                "K/2 R3 丙；丁。",
                "K/3 R3 戊；己。",
                "faults: 2",
            ]
        );
    }

    /// S02-AC03: an unchanged fault survives the Card's own address change, so a
    /// move does not make prose rules newly blocking (design 6.3, R0149).
    #[test]
    fn a_moved_card_keeps_its_unchanged_fault_as_advice() {
        let bundle = json!({
            "cards": [{
                "address": "K/8|b", "title": "均衡稳定", "body": "甲；乙。",
                "before": { "address": "K/8|a", "title": "均衡稳定", "body": "甲；乙。" }
            }]
        });
        assert_eq!(
            run_json(&bundle, "", Limits::default(), false, ""),
            ["K/8|b R3 (advisory) 甲；乙。", "faults: 0"]
        );
    }

    /// S02-AC07: the ratchet saves no baseline. Both versions are checked by the
    /// current core on every run, so repeated runs agree and a rule-version
    /// change needs no migration of stored fault records.
    #[test]
    fn the_ratchet_keeps_no_saved_baseline() {
        let bundle = json!({
            "cards": [{
                "address": "K/1", "title": "均衡稳定", "body": "甲；乙。",
                "before": { "title": "均衡稳定", "body": "甲；乙。" }
            }]
        });
        let first = check_json(&bundle.to_string(), "", Limits::default(), false, "").unwrap();
        let again = check_json(&bundle.to_string(), "", Limits::default(), false, "").unwrap();
        assert_eq!(first, again);
        // A bundle with no `before` keeps the existing count of every printed line.
        let plain = json!({ "cards": [{ "address": "K/1", "title": "均衡稳定", "body": "甲；乙。" }] });
        assert_eq!(
            check_json(&plain.to_string(), "", Limits::default(), false, "").unwrap(),
            (vec!["K/1 R3 甲；乙。".to_string()], 1)
        );
        let advisory = json!({ "cards": [{ "address": "K/1", "title": "税", "category": "anchor" }] });
        assert_eq!(
            check_json(&advisory.to_string(), "", Limits::default(), false, "")
                .unwrap()
                .1,
            0,
            "the coordinated switch counts blocking diagnostics only (S06, R0187)"
        );
    }

    /// A `before` that is not an entry cannot stand for an authored version.
    #[test]
    fn a_before_that_is_not_an_entry_is_refused() {
        let bundle = json!({ "cards": [{ "address": "K/1", "title": "均衡稳定", "body": "甲；乙。", "before": "old" }] });
        let error = check_json(&bundle.to_string(), "", Limits::default(), false, "").unwrap_err();
        assert!(format!("{error:#}").contains("before"), "{error:#}");
    }

    /// S02-AC04: a size fault's identity is the measured value and the limit. An
    /// unchanged measurement is inherited; making an oversized part larger is a
    /// new fault and blocks.
    #[test]
    fn a_larger_size_fault_is_new_while_the_same_measurement_is_inherited() {
        let part = |sentences: usize| {
            (1..=sentences)
                .map(|n| format!("第 {n} 句说明这一步。"))
                .collect::<String>()
        };
        let bundle = |before: usize, proposed: usize| {
            json!({ "cards": [{
                "address": "K/6|a", "facet": "设定", "title": "设定", "body": part(proposed),
                "before": { "address": "K/6|a", "facet": "设定", "title": "设定", "body": part(before) }
            }]})
        };
        assert_eq!(
            run_json(&bundle(13, 14), "", Limits::default(), false, ""),
            [
                "K/6|a R13 (14 sentences > 12) split the part into two or more parts",
                "faults: 1",
            ]
        );
        assert_eq!(
            run_json(&bundle(13, 13), "", Limits::default(), false, ""),
            [
                "K/6|a R13 (advisory) (13 sentences > 12) split the part into two or more parts",
                "faults: 0",
            ]
        );
    }

    /// S02-AC04: a missing-label fault's identity is the missing-label set, so a
    /// wider set is another requirement even when the old fault is still present.
    #[test]
    fn a_wider_missing_label_set_is_a_new_event_fault() {
        let body = |labels: &str| format!("时间：1999 年。\n地点：上海。\n{labels}");
        let bundle = |before: &str, proposed: &str| {
            json!({ "cards": [{
                "address": "K/7|a", "category": "event", "title": "1999 年的一笔交易",
                "body": body(proposed),
                "before": {
                    "category": "event", "title": "1999 年的一笔交易", "body": body(before)
                }
            }]})
        };
        assert_eq!(
            run_json(
                &bundle("参与者：甲。\n经过：双方签约。", "经过：双方签约。"),
                "",
                Limits::default(),
                false,
                ""
            ),
            ["K/7|a E2 missing labels: 参与者、来源", "faults: 1"]
        );
        // The same missing set, after an edit elsewhere in the body, stays advice.
        assert_eq!(
            run_json(
                &bundle("参与者：甲。\n经过：双方签约。", "参与者：甲。\n经过：双方签约并交割。"),
                "",
                Limits::default(),
                false,
                ""
            ),
            ["K/7|a E2 (advisory) missing labels: 来源", "faults: 0"]
        );
    }

    // --- S06 (#148): the exact draft, the Workspace, and the published status -------------------

    fn draft(raw: &Value, options: &Options) -> DraftOutcome {
        check_draft(&raw.to_string(), InputKind::Draft, options).expect("a valid draft")
    }

    fn all_families() -> Vec<String> {
        crate::contract::FAMILIES
            .iter()
            .map(|family| family.to_string())
            .collect()
    }

    /// S06-AC01/AC02: a draft names the exact Card text and its type, and an
    /// address the caller cannot know yet stays provisional (R0195).
    #[test]
    fn a_draft_checks_the_exact_text_and_type_of_each_candidate() {
        let raw = json!({
            "workspace": "agent",
            "cards": [
                { "address": "K/14", "type": "idea", "text": "# 税负落在买方\n\n甲；乙。" },
                { "type": "event", "text": "# 通用汽车收购费雪车身公司\n\n时间：1999 年。\n地点：上海。\n参与者：甲。\n经过：双方签约。\n来源：[[K]]" }
            ]
        });
        let outcome = draft(&raw, &Options::default());
        assert_eq!(
            outcome.report.lines(),
            [
                "K/14 R3 甲；乙。",
                "K/14 I1 (advisory) the title states no claim: 税负落在买方",
                "K/14 I2 (4 characters < 40) merge candidate",
                "draft:2 E1 no date in the title: 通用汽车收购费雪车身公司",
            ]
        );
        // The default bundle declares the envelope and the local checks; the rest
        // stay explicitly missing rather than passing by omission (US021).
        assert_eq!(outcome.missing, ["graph", "placement", "policy"]);
        assert!(
            outcome
                .reasons
                .iter()
                .any(|reason| reason.contains("service preview")),
            "the missing placement names the one way to resolve it: {:?}",
            outcome.reasons
        );
        assert!(outcome.contract.archive.is_none(), "a draft has no archive");
        assert_eq!(outcome.workspace, "agent", "a draft defaults to the agent Workspace");
        assert_eq!(outcome.report.blockers, 3, "R3, I2 and E1 block; I1 is advice");
    }

    /// S06-AC02/AC07: a bundle that declares every family and binds the built-in
    /// policy is a complete evaluation; its verdict follows its blockers.
    #[test]
    fn a_draft_that_declares_every_family_and_binds_the_policy_is_complete() {
        let policy_hash = crate::contract::Contract::built_in(&Limits::default()).policy_hash;
        let raw = json!({
            "rule_version": crate::contract::RULE_VERSION,
            "policy_hash": policy_hash,
            "cards": [{ "address": "K/1", "text": "# 税负落在买方\n\n甲；乙。" }],
            "context": {
                "cards": [{ "address": "K/2", "title": "另一张卡", "workspace": "agent" }],
                "complete_for": all_families(),
            }
        });
        let outcome = draft(&raw, &Options::default());
        assert!(outcome.missing.is_empty(), "{:?}", outcome.missing);
        assert!(outcome.reasons.is_empty(), "{:?}", outcome.reasons);
        assert_eq!(outcome.report.blockers, 1);
    }

    /// S06-AC05: a declared family the bundle's own facts do not back is not
    /// complete: a graph declaration without a context, and a policy declaration
    /// without the matching policy hash, stay missing.
    #[test]
    fn a_declared_family_without_its_facts_stays_incomplete() {
        let raw = json!({
            "cards": [{ "address": "K/1", "text": "# 税负落在买方\n\n只此一句。" }],
            "context": { "complete_for": all_families() }
        });
        let outcome = draft(&raw, &Options::default());
        assert_eq!(outcome.missing, ["graph", "policy"]);
        assert!(
            outcome.reasons.iter().any(|reason| reason.contains("complete Workspace context")),
            "{:?}",
            outcome.reasons
        );
        assert!(
            outcome.reasons.iter().any(|reason| reason.contains("policy hash")),
            "{:?}",
            outcome.reasons
        );

        // A policy hash other than the built-in one is a different policy, not a pass.
        let raw = json!({
            "policy_hash": "0".repeat(64),
            "cards": [{ "address": "K/1", "text": "# 税负落在买方\n\n只此一句。" }],
            "context": { "complete_for": ["envelope", "local", "policy"] }
        });
        let outcome = draft(&raw, &Options::default());
        assert_eq!(outcome.missing, ["graph", "placement", "policy"]);
    }

    /// S06-AC05: a stale rule binding is refused, never reported as a pass (R0206).
    #[test]
    fn a_stale_rule_version_in_the_bundle_is_refused() {
        let raw = json!({
            "rule_version": "0",
            "cards": [{ "address": "K/1", "text": "# 税负落在买方\n\n只此一句。" }]
        });
        let error = check_draft(&raw.to_string(), InputKind::Draft, &Options::default())
            .expect_err("a stale rule version is refused");
        assert!(
            format!("{error:#}").contains("stale check contract"),
            "{error:#}"
        );
    }

    /// S06-AC01: an unknown check family and an unknown draft type are invalid
    /// invocations, never a silently narrower or broader check.
    #[test]
    fn an_unknown_family_or_type_is_refused() {
        let raw = json!({
            "cards": [{ "address": "K/1", "text": "# 税负落在买方\n\n只此一句。" }],
            "context": { "complete_for": ["everything"] }
        });
        let error = check_draft(&raw.to_string(), InputKind::Draft, &Options::default())
            .expect_err("an unknown family is refused");
        assert!(format!("{error:#}").contains("unknown check family"), "{error:#}");

        let raw = json!({ "cards": [{ "address": "K/1", "type": "chapter", "text": "# 标题\n\n正文。" }] });
        let error = check_draft(&raw.to_string(), InputKind::Draft, &Options::default())
            .expect_err("an unknown type is refused");
        assert!(format!("{error:#}").contains("unknown draft type"), "{error:#}");
    }

    /// S06-AC01/AC02: a draft's text must carry the envelope title, and its type
    /// is named once.
    #[test]
    fn a_draft_without_a_title_or_with_two_types_is_refused() {
        let raw = json!({ "cards": [{ "address": "K/1", "text": "没有标题的正文。" }] });
        let error = check_draft(&raw.to_string(), InputKind::Draft, &Options::default())
            .expect_err("a text without a title is refused");
        assert!(format!("{error:#}").contains("open with `# <title>`"), "{error:#}");

        let raw = json!({
            "cards": [{ "address": "K/1", "type": "event", "category": "idea", "text": "# 标题\n\n正文。" }]
        });
        let error = check_draft(&raw.to_string(), InputKind::Draft, &Options::default())
            .expect_err("two types are refused");
        assert!(format!("{error:#}").contains("name its type once"), "{error:#}");
    }

    /// S06-AC03: a draft belongs to the agent Workspace by default, and the
    /// Workspace policy decides its severities. `--workspace human` cannot
    /// reclassify a draft that names its own Workspace.
    #[test]
    fn the_workspace_flag_decides_a_draft_under_that_workspaces_policy() {
        let long = (1..=13)
            .map(|n| format!("第 {n} 句说明这一步的推理。"))
            .collect::<String>();
        let raw = json!({ "cards": [{ "address": "K/1", "facet": "设定", "text": format!("# 设定\n\n{long}") }] });
        let agent = draft(&raw, &Options::default());
        assert_eq!(agent.workspace, "agent");
        assert_eq!(agent.report.blockers, 1, "R13 blocks in the agent Workspace");
        let human = draft(
            &raw,
            &Options {
                workspace: Some("human".to_string()),
                ..Options::default()
            },
        );
        assert_eq!(human.workspace, "human");
        assert_eq!(human.report.blockers, 0, "R13 advises in the human Workspace");
        assert!(
            human.report.lines()[0].contains("R13 (advisory)"),
            "{:?}",
            human.report.lines()
        );

        // The bundle's own Workspace wins, and a different flag is a refusal.
        let named = json!({ "workspace": "human", "cards": [] });
        let error = check_draft(
            &named.to_string(),
            InputKind::Draft,
            &Options {
                workspace: Some("agent".to_string()),
                ..Options::default()
            },
        )
        .expect_err("two Workspaces are refused");
        assert!(format!("{error:#}").contains("name one Workspace"), "{error:#}");
    }

    /// S06-AC01/AC07: the status document carries the verdict, the missing
    /// families, the counts, the contract, and the gate's own stage decision.
    #[test]
    fn the_status_document_names_the_verdict_and_the_contract() {
        let raw = json!({
            "cards": [{ "address": "K/1", "text": "# 税负落在买方\n\n甲；乙。" }]
        });
        let run = draft(&raw, &Options::default());
        let status = Run {
            lines: printed(run.report.lines(), run.report.blockers),
            status: Status {
                mode: "draft_bundle",
                verdict: verdict(&run.missing, run.report.blockers),
                missing: run.missing,
                reasons: run.reasons,
                blocking: run.report.blockers,
                advisory: run.report.diagnostics.len() - run.report.blockers,
                workspace: run.workspace,
                policy_source: crate::contract::POLICY_SOURCE_BUILT_IN,
                contract: run.contract.publish(),
                gate: Value::Null,
                diagnostics: run
                    .report
                    .diagnostics
                    .iter()
                    .map(Diagnostic::to_json)
                    .collect(),
            },
        };
        let document = status.status.to_json();
        assert_eq!(document["mode"], json!("draft_bundle"));
        assert_eq!(document["verdict"], json!("incomplete"));
        assert_eq!(document["complete"], json!(false));
        assert_eq!(document["missing"], json!(["graph", "placement", "policy"]));
        assert_eq!(document["blocking"], json!(1));
        assert_eq!(document["advisory"], json!(0));
        assert_eq!(document["policy_source"], json!("built_in"));
        assert_eq!(
            document["contract"]["rule_version"],
            json!(crate::contract::RULE_VERSION)
        );
        assert_eq!(document["contract"]["archive"], Value::Null);
        assert_eq!(document["gate"], Value::Null);
        assert_eq!(document["diagnostics"][0]["rule"], json!("R3"));
        assert_eq!(status.status.exit_code(), 2);
        assert_eq!(status.lines.last().map(String::as_str), Some("faults: 1"));
    }

    /// S06-AC07: the exit contract is the verdict's: 0 pass, 1 block, 2 incomplete.
    #[test]
    fn the_exit_contract_follows_the_verdict() {
        let status = |verdict, missing: Vec<String>| Status {
            mode: "live",
            verdict,
            missing,
            reasons: Vec::new(),
            blocking: 0,
            advisory: 0,
            workspace: String::new(),
            policy_source: "archive",
            contract: Value::Null,
            gate: Value::Null,
            diagnostics: Vec::new(),
        };
        assert_eq!(status("pass", Vec::new()).exit_code(), 0);
        assert_eq!(status("block", Vec::new()).exit_code(), 1);
        assert_eq!(status("incomplete", vec!["placement".to_string()]).exit_code(), 2);
    }

    /// S06-AC03: a live check refuses a Workspace flag that would reclassify a
    /// stored Card (R0186).
    #[test]
    fn a_live_workspace_flag_must_agree_with_every_stored_card() {
        let cards = vec![
            json!({ "address": "1/1", "workspace": "agent" }),
            json!({ "address": "0/1", "workspace": "human" }),
        ];
        assert!(workspace_gaps(&cards, "").is_empty());
        assert!(workspace_gaps(&cards, "agent").len() == 1);
        assert_eq!(
            workspace_gaps(&cards, "agent")[0],
            "`0/1` is a human Card; --workspace agent cannot reclassify a stored Card"
        );
        let agent_only = vec![json!({ "address": "1/1", "workspace": "agent" })];
        assert!(workspace_gaps(&agent_only, "agent").is_empty());
    }
}
