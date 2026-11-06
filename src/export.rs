//! `zt export`: one tree as a folder that reads by itself (#257).
//!
//! A portable export holds exactly two files: `cards.json`, the tree's Card data with each Card's
//! rendered HTML, its models, figures and chart pages, and `index.html`, the GUI itself with no Card
//! data. It needs no `zt`, service, Scholar Archive or network: `src/gui/portable-rules.js` answers
//! the page's `/api/*` reads from `cards.json`. It is for reading: unlike a dump (`src/dump.rs`) it
//! records one tree, restores nothing, and nothing loads it back.
//!
//! Everything is read from one [`Snapshot`], so all Cards belong to one archive revision. Like a
//! dump it takes no edit lock (#253).
//!
//! The export names no machine: no archive root, no row id, no Lock fact, no artifact source. The
//! generated Reverse and Term-use sections stay behind too, because they name other trees' Cards;
//! the page derives both lists from `links` and `term_use_targets` inside the tree. For the same
//! reason a model keeps only what points out of the tree, never what points in from other trees.

use anyhow::{Context, Result, bail};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::gui::{Portable, base64, portable_viewer};
use crate::Card;
use crate::ops::{self, Snapshot};

const CARDS_FILE: &str = "cards.json";
const VIEWER_FILE: &str = "index.html";
const FORMAT: &str = "zt-portable-export";
const VERSION: u64 = 2;
const USAGE: &str = "usage: zt export <tree> [--out <folder>]";

pub(crate) fn run(args: &[String]) -> Result<()> {
    let (tree, out) = parse_args(args)?;
    let root = crate::require_service_up()?;
    let snapshot = Snapshot::read(&root)?;
    let written = write(&root, &snapshot, &tree, out)?;
    println!(
        "exported {} cards of {} to {} ({:.1} MB)",
        written.cards,
        written.tree,
        written.folder.display(),
        written.bytes as f64 / 1_048_576.0
    );
    if written.outside > 0 {
        println!("{} Link targets lie outside this tree and stay inert", written.outside);
    }
    if written.missing > 0 {
        println!("{} figures are not in this archive", written.missing);
    }
    Ok(())
}

/// What one export wrote; the command line and the GUI each report it in their own words.
pub(crate) struct Written {
    pub(crate) tree: String,
    pub(crate) folder: PathBuf,
    pub(crate) cards: usize,
    pub(crate) bytes: usize,
    pub(crate) outside: usize,
    pub(crate) missing: usize,
}

/// Write one tree, named by its id or any address in it, into `folder` or the default folder.
pub(crate) fn write(
    root: &Path,
    snapshot: &Snapshot,
    tree: &str,
    out: Option<PathBuf>,
) -> Result<Written> {
    let tree = crate::tree_id(tree).to_string();
    let root_address = crate::tree_root_address(&tree);
    if !snapshot.by_address.contains_key(&root_address) {
        bail!("tree `{tree}` does not exist");
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    let folder = out.unwrap_or_else(|| {
        root.join("export")
            .join(format!("{}-{seconds}", folder_name(&tree)))
    });
    if folder.exists() && fs::read_dir(&folder)?.next().is_some() {
        bail!("{} already exists and is not empty", folder.display());
    }

    let export = collect(root, snapshot, &tree, &root_address, seconds)?;
    let viewer = portable_viewer();
    let cards = serde_json::to_vec(&export.document)?;
    publish(&folder, &cards, viewer.as_bytes())?;
    Ok(Written {
        tree,
        folder,
        cards: export.cards,
        bytes: cards.len() + viewer.len(),
        outside: export.outside,
        missing: export.missing,
    })
}

fn parse_args(args: &[String]) -> Result<(String, Option<PathBuf>)> {
    match args {
        [tree] if !tree.starts_with("--") => Ok((tree.clone(), None)),
        [tree, flag, out] | [flag, out, tree] if flag == "--out" && !tree.starts_with("--") => {
            Ok((tree.clone(), Some(PathBuf::from(out))))
        }
        _ => bail!("{USAGE}"),
    }
}

/// A Citation key may hold characters a folder name may not.
fn folder_name(tree: &str) -> String {
    tree.chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '-' })
        .collect()
}

struct Export {
    document: Value,
    cards: usize,
    outside: usize,
    missing: usize,
}

fn collect(
    root: &Path,
    snapshot: &Snapshot,
    tree: &str,
    root_address: &str,
    seconds: u64,
) -> Result<Export> {
    let members: Vec<&Card> = snapshot
        .cards
        .iter()
        .filter(|card| crate::tree_id(&card.address) == tree)
        .collect();
    let ordered = reading_order(&members, root_address);
    let portable = Portable::new(tree);
    let mut cards = Vec::with_capacity(ordered.len());
    let mut outside: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut heads = BTreeSet::new();
    for card in &ordered {
        let (value, targets) = card_value(snapshot, card, &portable, tree, &mut heads);
        for target in targets {
            outside.entry(target).or_default().push(card.address.clone());
        }
        cards.push(value);
    }
    let models: BTreeMap<String, Value> = heads
        .iter()
        .filter_map(|head| {
            let model = snapshot.model_in(head, Some(head))?;
            Some((head.clone(), inside_model(model, tree)))
        })
        .collect();
    let broken: Vec<Value> = snapshot
        .broken(None)?
        .into_iter()
        .filter(|row| row["source"].as_str().is_some_and(|source| crate::tree_id(source) == tree))
        .collect();
    let (charts, figures) = portable.gathered();
    let (mut artifacts, missing) = artifacts(root, &figures);
    add_explanations(root, &mut artifacts);
    let root_card = &snapshot.by_address[root_address];
    let outside: Vec<Value> = outside
        .into_iter()
        .map(|(address, from)| {
            let title = snapshot.title(&address);
            json!({ "address": address, "title": title, "from": from })
        })
        .collect();
    let outside_count = outside.len();
    let document = json!({
        "format": FORMAT,
        "version": VERSION,
        "tree": {
            "id": tree,
            "root": root_address,
            "kind": ops::kind(root_card),
            "workspace": root_card.workspace.id(),
            "title": snapshot.title(root_address),
        },
        "exported": {
            "at": timestamp(root, seconds)?,
            "zt": env!("CARGO_PKG_VERSION"),
            "archive_revision": snapshot.revision(),
        },
        "cards": cards,
        "models": models,
        "broken": broken,
        "artifacts": artifacts,
        "charts": charts,
        "outside": outside,
    });
    Ok(Export { document, cards: ordered.len(), outside: outside_count, missing })
}

/// One Card of the export, and the Link targets it names outside the tree.
fn card_value(
    snapshot: &Snapshot,
    card: &Card,
    portable: &Portable,
    tree: &str,
    heads: &mut BTreeSet<String>,
) -> (Value, Vec<String>) {
    let summary = snapshot.summary(card);
    let targets = summary["links"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .filter(|target| crate::tree_id(target) != tree)
        .map(str::to_string)
        .collect();
    let mut value = Map::new();
    for field in [
        "address", "parent", "edge", "kind", "title", "type", "facet", "emphasis", "level", "form",
        "head", "excerpt", "restatement", "links", "kinds", "term_use_targets", "term", "term_role",
        "term_list", "literature_type",
    ] {
        if !summary[field].is_null() {
            value.insert(field.to_string(), summary[field].clone());
        }
    }
    if card.card_type == crate::CardType::Model {
        let own: Vec<String> = snapshot
            .model_heads(&card.address)
            .into_iter()
            .filter(|head| crate::tree_id(head) == tree)
            .collect();
        if own.is_empty() {
            // A part of no model of this tree answers for itself, as the reader's fault line does.
            let alone = snapshot.model_in(&card.address, None).map(|model| inside_model(model, tree));
            value.insert("model".to_string(), json!(alone));
        } else {
            heads.extend(own.iter().cloned());
            value.insert("models".to_string(), json!(own));
        }
    }
    let hubs: Vec<Value> = crate::hub::hubs_of(snapshot, &card.address)
        .into_iter()
        .filter(|hub| hub["address"].as_str().is_some_and(|at| crate::tree_id(at) == tree))
        .collect();
    if !hubs.is_empty() {
        value.insert("hubs".to_string(), json!(hubs));
    }
    let broken = broken_targets(snapshot, &card.address);
    if !broken.is_empty() {
        value.insert("broken".to_string(), json!(broken));
    }
    value.insert("bibtex".to_string(), json!(card.bibtex));
    value.insert("text".to_string(), json!(authored_text(&card.text)));
    value.insert(
        "html".to_string(),
        json!(portable.render(&snapshot.parsed[&card.address].body, snapshot)),
    );
    (Value::Object(value), targets)
}

/// A model as an export shows it: what it points at stays, and what other trees write about it
/// does not travel, for the reason the generated sections do not.
fn inside_model(mut model: Value, tree: &str) -> Value {
    let inside = |at: &Value| at.as_str().is_some_and(|at| crate::tree_id(at) == tree);
    if let Some(rows) = model["neighbours"].as_array_mut() {
        rows.retain(|row| row["way"] == "out" || inside(&row["card"]["address"]));
    }
    if let Some(parts) = model["parts"].as_array_mut() {
        for part in parts {
            if let Some(also) = part["also"].as_array_mut() {
                also.retain(|other| inside(&other["address"]));
            }
        }
    }
    if let Some(lent) = model["lent"].as_array_mut() {
        lent.retain(|row| inside(&row["model"]));
    }
    model
}

/// The Link targets of one Card that name no Card at all.
fn broken_targets(snapshot: &Snapshot, address: &str) -> Vec<String> {
    snapshot
        .outbound(address)
        .into_iter()
        .filter(|link| link["broken"] == true)
        .filter_map(|link| link["address"].as_str().map(str::to_string))
        .collect()
}

/// The Card text up to the generated sections: the Reverse links and the Term uses behind the
/// marker name Cards of other trees.
fn authored_text(text: &str) -> &str {
    let mut at = 0;
    for line in text.split_inclusive('\n') {
        if line.trim() == crate::card::REVERSE_MARKER {
            return text[..at].trim_end();
        }
        at += line.len();
    }
    text.trim_end()
}

/// The Cards in reading order, the order of the GUI's outline (`outline` in `layout.js`): a tree
/// root's Direct run, and under each Card the runs of its Side successors. A Card the address
/// rules cannot reach comes last rather than being lost.
fn reading_order<'a>(members: &[&'a Card], root: &str) -> Vec<&'a Card> {
    let by_address: BTreeMap<&str, &Card> =
        members.iter().map(|card| (card.address.as_str(), *card)).collect();
    let mut direct: BTreeMap<String, &str> = BTreeMap::new();
    let mut sides: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for card in members {
        let Some(parent) = crate::parent_location(&card.address) else {
            continue;
        };
        if ops::edge(card) == "side" {
            sides.entry(parent).or_default().push(&card.address);
        } else {
            direct.insert(parent, &card.address);
        }
    }
    for list in sides.values_mut() {
        list.sort_by_key(|address| crate::side_label_to_number(crate::last_segment(address)));
    }
    let mut seen = BTreeSet::new();
    let mut order = Vec::with_capacity(members.len());
    visit(root, root, &direct, &sides, &mut seen, &mut order);
    for card in members {
        if !seen.contains(card.address.as_str()) {
            visit(&card.address, root, &direct, &sides, &mut seen, &mut order);
        }
    }
    order.into_iter().filter_map(|address| by_address.get(address).copied()).collect()
}

fn visit<'a>(
    at: &'a str,
    root: &str,
    direct: &BTreeMap<String, &'a str>,
    sides: &BTreeMap<String, Vec<&'a str>>,
    seen: &mut BTreeSet<&'a str>,
    order: &mut Vec<&'a str>,
) {
    if !seen.insert(at) {
        return;
    }
    order.push(at);
    let heads: Vec<&str> = if at == root {
        direct.get(at).copied().into_iter().collect()
    } else {
        sides.get(at).cloned().unwrap_or_default()
    };
    for head in heads {
        let mut step = Some(head);
        while let Some(card) = step {
            visit(card, root, direct, sides, seen, order);
            step = direct.get(card).copied();
        }
    }
}

/// Each figure once, as base64, with how many the archive does not hold.
fn artifacts(root: &Path, figures: &BTreeSet<String>) -> (BTreeMap<String, Value>, usize) {
    let mut held = BTreeMap::new();
    let mut missing = 0;
    for digest in figures {
        match crate::artifact::read(root, digest) {
            Some((bytes, media_type)) => {
                held.insert(
                    digest.clone(),
                    json!({ "type": media_type, "base64": base64(&bytes) }),
                );
            }
            None => missing += 1,
        }
    }
    (held, missing)
}

/// A portable export has no Scholar Archive, so each figure's explanation goes with it. Figures
/// with no recorded source ask nothing; the export stands without the texts the archive cannot give.
fn add_explanations(root: &Path, artifacts: &mut BTreeMap<String, Value>) {
    let sources: Vec<(String, u64, String)> = artifacts
        .keys()
        .filter_map(|digest| {
            let source = crate::artifact::source(root, digest)?;
            Some((digest.clone(), source["file_id"].as_u64()?, source["path"].as_str()?.to_string()))
        })
        .collect();
    if sources.is_empty() {
        return;
    }
    let Ok(mut bridge) = crate::scholar::Bridge::connect() else { return };
    for (digest, file_id, path) in sources {
        if let Ok(Some(held)) = bridge.explanation(file_id, &path, &digest) {
            artifacts.insert(
                digest.clone(),
                json!({
                    "type": artifacts[&digest]["type"],
                    "base64": artifacts[&digest]["base64"],
                    "explanation": { "kind": held["kind"], "title": held["title"], "explanation": held["explanation"] },
                }),
            );
        }
    }
}

fn timestamp(root: &Path, seconds: u64) -> Result<String> {
    let at = crate::open_db(root)?.query_row(
        "SELECT strftime('%Y-%m-%dT%H:%M:%SZ', ?1, 'unixepoch')",
        [seconds],
        |row| row.get(0),
    )?;
    Ok(at)
}

/// Write both files into a staging folder beside the destination and rename it into place, so a
/// failure leaves no half export. An existing destination is known to be empty.
fn publish(folder: &Path, cards: &[u8], viewer: &[u8]) -> Result<()> {
    let name = folder
        .file_name()
        .with_context(|| format!("{} names no folder", folder.display()))?
        .to_string_lossy()
        .to_string();
    let parent = match folder.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::create_dir_all(&parent)
        .with_context(|| format!("cannot create {}", parent.display()))?;
    let staging = Staging(parent.join(format!(".{name}.partial")));
    fs::create_dir(&staging.0)
        .with_context(|| format!("cannot create {}", staging.0.display()))?;
    fs::write(staging.0.join(CARDS_FILE), cards)?;
    fs::write(staging.0.join(VIEWER_FILE), viewer)?;
    if folder.exists() {
        fs::remove_dir(folder)?;
    }
    fs::rename(&staging.0, folder)
        .with_context(|| format!("cannot move the export into {}", folder.display()))?;
    Ok(())
}

/// Removes the staging folder on every path; after the rename it is already gone.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_marker_line_ends_the_authored_text_and_a_mention_does_not() {
        assert_eq!(
            authored_text("# T\n\nbody\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[x]]\n"),
            "# T\n\nbody"
        );
        assert_eq!(
            authored_text("# T\n\nthe `<!-- zt:reverse-links -->` line\n"),
            "# T\n\nthe `<!-- zt:reverse-links -->` line"
        );
    }

    #[test]
    fn a_folder_name_keeps_only_what_a_file_system_takes() {
        assert_eq!(folder_name("Smith2024"), "Smith2024");
        assert_eq!(folder_name("a:b/c"), "a-b-c");
    }
}
