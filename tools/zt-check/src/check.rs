//! `zt-check`: the command-line input adapter of the Card quality check. It reads
//! the drafts (`--json`), or the live Cards through one `zt mcp` (`--model`,
//! `--addresses`, `--cards`/`--log`), materializes the term table, level, and
//! Workspace title context, hands them to the process-free core, and prints the
//! familiar fault lines.
//!
//! The rules themselves live in `core`, the text semantics in `text`, and the
//! report policy in `policy`; this module owns only the inputs, the output, and
//! the archive client. It is compiled only with the `cli` feature.

use crate::core::{Check, Limits};
use crate::mcp::Zt;
use crate::policy::{DEFAULT_RULES, LIST_SPLIT, filter_rules};
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
  zt-check --model <address> [--skip-shared] [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --addresses <a,b,...> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
  zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,R3,...] [limits]
limits: --max-sentences 12 --max-chars 400 --max-steps 6 --max-explain-sentences 16
        --max-explain-chars 600 --min-idea-chars 40 --max-idea-chars 300 --min-reason 8";

// --- inputs -----------------------------------------------------------------------------------

/// `--json`: a JSON array of entries (or one entry), as card-style-check.ps1 -Json reads
/// it, or a bundle `{"cards": [...], "context": {"titles": [...], "complete": true}}`
/// whose context is the read-only Workspace title comparison I6 needs (D135).
pub(crate) fn check_json(
    raw: &str,
    setting: &str,
    limits: Limits,
    require_explain: bool,
    level: &str,
) -> Result<Vec<String>> {
    let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(raw);
    let (cards, context) = match serde_json::from_str::<Value>(raw).context("the file is not JSON")? {
        Value::Object(map) if map.contains_key("cards") => {
            let context = map.get("context").map(TitleContext::from_json).unwrap_or_default();
            let cards = match map.get("cards") {
                Some(Value::Array(items)) => items.clone(),
                Some(other) => vec![other.clone()],
                None => Vec::new(),
            };
            (cards, context)
        }
        Value::Array(items) => (items, TitleContext::default()),
        other => (vec![other], TitleContext::default()),
    };
    let shared = cards
        .iter()
        .filter(|card| ps_eq(&text_of(card, "line"), "shared"))
        .map(|card| ps_string(prop(card, "address")))
        .collect();
    let setting = utf16_form(setting);
    let setting_body = (!setting.is_empty())
        .then(|| {
            cards
                .iter()
                .find(|card| ps_eq(&text(prop(card, "address")), &setting))
                .map(|card| ps_string(prop(card, "body")))
        })
        .flatten();
    let check = Check::new(limits, require_explain, level)
        .with_terms(setting_body.as_deref())
        .with_shared(shared)
        .with_context(context);
    Ok(check.run(&cards))
}

fn text_of(card: &Value, name: &str) -> String {
    text(prop(card, name))
}

/// A Card as card-style-check.ps1 reads it from get_card: address, title, body, facet.
fn card_entry(card: &Value) -> Value {
    json!({
        "address": card.get("address").cloned().unwrap_or(Value::Null),
        "title": card.get("title").cloned().unwrap_or(Value::Null),
        "body": card.get("body").cloned().unwrap_or(Value::Null),
        "facet": card.get("facet").cloned().unwrap_or(Value::Null),
    })
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

/// `--model`: every Card of the complete model context, with its setting's
/// term table and level. An external setting uses a required lean read.
pub(crate) fn check_model(
    zt: &mut Zt,
    at: &str,
    setting: &str,
    level: &str,
    limits: Limits,
    require_explain: bool,
    skip_shared: bool,
) -> Result<Vec<String>> {
    let revision = zt.cards(&[], "text")?.archive_revision;
    let structure = zt.call("get_model", json!({ "at": at }))?;
    structure["head"]["address"]
        .as_str()
        .with_context(|| format!("`{at}` belongs to no model"))?;
    let mut cards = vec![card_entry(&structure["head"])];
    for part in structure["parts"].as_array().into_iter().flatten() {
        cards.push(card_entry(part));
    }
    let setting = if setting.is_empty() {
        ps_string(structure["build"].get("setting"))
    } else {
        setting.to_string()
    };
    let level = if level.is_empty() {
        ps_string(structure.get("level"))
    } else {
        level.to_string()
    };
    // A shared part follows the term table of its home model, not this one.
    let shared = structure["build"]["order"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|item| ps_eq(&ps_string(item.get("line")), "shared"))
        .map(|item| ps_string(item.get("address")))
        .collect::<Vec<_>>();
    if skip_shared {
        cards.retain(|card| !ps_in(&ps_string(prop(card, "address")), &shared));
    }
    let body = setting_body(zt, &cards, &setting)?;
    let check = Check::new(limits, require_explain, &level)
        .with_terms(body.as_deref())
        .with_shared(shared);
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete checker inputs: changing read view");
    }
    Ok(check.run(&cards))
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
/// its layer rules; any other Card with the R rules only. Returns every line,
/// before the rule filter.
pub(crate) fn check_cards(
    zt: &mut Zt,
    addresses: &[String],
    book: &str,
    limits: Limits,
) -> Result<Vec<String>> {
    let batch = zt.cards(addresses, "text")?;
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
    let mut lines = Vec::new();
    // Model parts and Model Cards: per model, with its setting (term table) and
    // level, as card-style-check.ps1 -Addresses reads them from the first Card.
    for (_, members) in &by_model {
        let model = &members[0]["model"];
        let setting = ps_string(model["build"].get("setting"));
        let level = ps_string(model.get("level"));
        let checked = members.iter().map(card_entry).collect::<Vec<_>>();
        let body = setting_body(zt, &checked, &setting)?;
        let check = Check::new(limits.clone(), true, &level).with_terms(body.as_deref());
        lines.extend(check.run(&checked));
    }
    if !plain.is_empty() {
        lines.extend(Check::new(limits, false, "").with_context(context).run(&plain));
    }
    if zt.cards(&[], "text")?.archive_revision != batch.archive_revision {
        bail!("incomplete checker inputs: changing read view");
    }
    Ok(lines)
}

// --- the command ------------------------------------------------------------------------------

#[derive(Default)]
struct Options {
    json: Option<String>,
    model: Option<String>,
    addresses: Option<String>,
    cards: Option<String>,
    log: Option<String>,
    book: Option<String>,
    rules: Option<String>,
    setting: Option<String>,
    level: Option<String>,
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
                "--model" => options.model = Some(value()?),
                "--addresses" => options.addresses = Some(value()?),
                "--cards" => options.cards = Some(value()?),
                "--log" => options.log = Some(value()?),
                "--book" => options.book = Some(value()?),
                "--rules" => options.rules = Some(value()?),
                "--setting" => options.setting = Some(value()?),
                "--level" => {
                    let level = value()?;
                    if !ps_in(&level, &["", "concept", "formal"]) {
                        bail!("--level is concept or formal, not `{level}`");
                    }
                    options.level = Some(level.to_lowercase());
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
            options.model.is_some(),
            options.addresses.is_some(),
            options.cards.is_some() || options.log.is_some(),
        ]
        .into_iter()
        .filter(|on| *on)
        .count();
        if modes != 1 {
            bail!("name one input: --json, --model, --addresses, or --cards/--log\n{USAGE}");
        }
        let cards_mode = options.cards.is_some() || options.log.is_some();
        if cards_mode
            && (options.setting.is_some()
                || options.level.is_some()
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
pub(crate) fn check_addresses(
    zt: &mut Zt,
    addresses: &[String],
    setting: &str,
    level: &str,
    limits: Limits,
    require_explain: bool,
) -> Result<Vec<String>> {
    let batch = zt.cards(addresses, "text")?;
    let read = batch.cards;
    let first = match read.first().filter(|card| card["type"] == "model") {
        Some(card) => zt.call("get_model", json!({"at":card["address"]}))?,
        None => Value::Null,
    };
    let setting = if setting.is_empty() {
        ps_string(first["build"].get("setting"))
    } else {
        setting.to_string()
    };
    let level = if level.is_empty() {
        ps_string(first.get("level"))
    } else {
        level.to_string()
    };
    let cards = read.iter().map(card_entry).collect::<Vec<_>>();
    let body = setting_body(zt, &cards, &setting)?;
    if zt.cards(&[], "text")?.archive_revision != batch.archive_revision {
        bail!("incomplete checker inputs: changing read view");
    }
    Ok(Check::new(limits, require_explain, &level)
        .with_terms(body.as_deref())
        .run(&cards))
}

/// The checker's own identity: a report that records the software version of a run
/// reads it here instead of guessing it from a path or a file date.
fn version() -> String {
    format!("zt-check {}", crate::VERSION)
}

/// `zt-check …`.
pub fn run_cli(args: &[String]) -> Result<()> {
    // `--version` is answered before any archive is read: a review records the checker
    // that produced its lines, and a copied binary must be able to state which one it is.
    if args.len() == 1 && matches!(args[0].to_ascii_lowercase().as_str(), "--version" | "-v") {
        println!("{}", version());
        return Ok(());
    }
    write_lines(&run_lines(args, None)?)
}

/// The lines `zt-check …` prints: one per fault, then `faults: N`. With `zt`,
/// the reads go to that `zt mcp`; without it, a new one starts when needed.
pub fn run_lines(args: &[String], zt: Option<&mut Zt>) -> Result<Vec<String>> {
    fn server<'a>(shared: Option<&'a mut Zt>, own: &'a mut Option<Zt>) -> Result<&'a mut Zt> {
        match shared {
            Some(zt) => Ok(zt),
            None => Ok(own.insert(Zt::start()?)),
        }
    }
    let mut own: Option<Zt> = None;
    let options = Options::parse(args)?;
    let setting = options.setting.clone().unwrap_or_default();
    let level = options.level.clone().unwrap_or_default();
    let (mut out, count) = if let Some(path) = &options.json {
        let raw =
            std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
        let lines = check_json(
            &raw,
            &setting,
            options.limits.clone(),
            options.require_explain,
            &level,
        )?;
        let count = lines.len();
        (lines, count)
    } else if let Some(list) = &options.addresses {
        let list = LIST_SPLIT
            .split(list)
            .filter(|at| !at.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        let zt = server(zt, &mut own)?;
        let lines = check_addresses(
            zt,
            &list,
            &setting,
            &level,
            options.limits.clone(),
            options.require_explain,
        )?;
        let count = lines.len();
        (lines, count)
    } else if let Some(at) = &options.model {
        let zt = server(zt, &mut own)?;
        let lines = check_model(
            zt,
            at,
            &setting,
            &level,
            options.limits.clone(),
            options.require_explain,
            options.skip_shared,
        )?;
        let count = lines.len();
        (lines, count)
    } else {
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
        if unique.is_empty() {
            (vec!["no Cards to check".to_string()], 0)
        } else {
            let zt = server(zt, &mut own)?;
            let lines = check_cards(
                zt,
                &unique,
                options.book.as_deref().unwrap_or_default(),
                options.limits.clone(),
            )?;
            filter_rules(&lines, options.rules.as_deref().unwrap_or(DEFAULT_RULES))
        }
    };
    out.push(format!("faults: {count}"));
    Ok(out)
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
    fn printed(mut lines: Vec<String>) -> Vec<String> {
        let count = lines.len();
        lines.push(format!("faults: {count}"));
        lines
    }

    fn json_lines(raw: &str, setting: &str, limits: Limits, require_explain: bool, level: &str) -> Vec<String> {
        printed(check_json(raw, setting, limits, require_explain, level).unwrap())
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
                "faults: 2",
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
            ["S/3 R6 uses `卖资产` for `清算`: 双方卖资产。", "faults: 1"]
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
}
