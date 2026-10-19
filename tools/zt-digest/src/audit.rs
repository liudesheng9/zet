//! `zt-digest audit`: the port of `scripts/book-digest/digest-audit.ps1`.
//!
//! Audit the Cards of a digested book (-CitationKey), or of every book in the agent Workspace.
//! Read-only: only zt read calls. It writes audit.json, audit.md, audit-faults.txt,
//! audit-addresses.txt, and audit-cards.json in -Out, prints three progress lines, and prints
//! one compact JSON summary line last. `card` runs it in-process (`run`).

pub mod book_of;
mod markdown;


use crate::args::Args;
use crate::check::Check;
use crate::util;
use crate::zt::Zt;
use anyhow::{Context, Result, bail};
use crate::ps::{self, key as ci, culture_cmp, group_by, ps_array, ps_count, eq, is_in, ps_int, ps_items, ps_sort, ps_sort_unique, field, s, truthy};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::LazyLock;
use std::time::Instant;

pub struct Params {
    pub citation_key: String,
    pub orphan_targets: String,
    pub out: String,
    pub max_sentences: i64,
    pub max_chars: i64,
    pub max_steps: i64,
    pub sample: i64,
    pub skip_models: bool,
}

/// One fault line of zt-check: `<address> <rule> <text>`.
#[derive(Clone)]
pub struct Fault {
    pub address: String,
    pub rule: String,
    pub line: String,
}

static FAULT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\S+ [RIEL]\d+ ").unwrap());
static SHOWN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[图\s?([^\]]+)\]\(artifact:").unwrap());
static IMAGE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[[^\]]*\]\([^)]*\)").unwrap());
static TERMS_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^术语表").unwrap());
static NOTATION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(符号表|notation)").unwrap());
static THINKING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?mi)^\s*\*\*思路\*\*\s*$").unwrap());
static READING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?mi)^\s*\*\*解读\*\*\s*$").unwrap());
static ALT1: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+(?:\.[0-9A-Za-z]+)*").unwrap());
static ALT2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[IVXL]+").unwrap());
static ALT3: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[一二三四五六七八九十]+").unwrap());

/// The figure labels a text names: the .NET pattern
/// `图\s?((?:\d+(?:\.[0-9A-Za-z]+)*)|[IVXL]+|[一二三四五六七八九十]+(?![㐀-鿿]))(?![0-9A-Za-z.])`.
/// Each alternative can only succeed at its longest run (a shorter one leaves a character the
/// look-ahead refuses), so the look-aheads are tested on the next character.
pub(crate) fn figure_labels(text: &str) -> Vec<String> {
    let after_ok = |rest: &str| !rest.chars().next().is_some_and(|c| c.is_ascii_alphanumeric() || c == '.');
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = text[at..].find('图') {
        let start = at + found + '图'.len_utf8();
        let mut body = start;
        if let Some(c) = text[start..].chars().next().filter(|c| c.is_whitespace()) {
            body += c.len_utf8();
        }
        let rest = &text[body..];
        let mut label = None;
        if let Some(m) = ALT1.find(rest).filter(|m| after_ok(&rest[m.end()..])) {
            label = Some(m.as_str());
        } else if let Some(m) = ALT2.find(rest).filter(|m| after_ok(&rest[m.end()..])) {
            label = Some(m.as_str());
        } else if let Some(m) = ALT3.find(rest).filter(|m| {
            let next = rest[m.end()..].chars().next();
            after_ok(&rest[m.end()..]) && !next.is_some_and(|c| ('\u{3400}'..='\u{9FFF}').contains(&c))
        }) {
            label = Some(m.as_str());
        }
        match label {
            Some(l) => {
                out.push(l.to_string());
                at = body + l.len();
            }
            None => at = start,
        }
    }
    out
}

/// `Get-Quantiles`.
fn quantiles(values: &[i64]) -> Value {
    let mut v = values.to_vec();
    v.sort();
    if v.is_empty() {
        return json!({ "n": 0 });
    }
    let at = |q: f64| v[((q * (v.len() - 1) as f64).floor() as usize).min(v.len() - 1)];
    json!({ "n": v.len(), "min": v[0], "p25": at(0.25), "median": at(0.5), "p75": at(0.75), "max": v[v.len() - 1] })
}

/// `Get-Tally`: the count per value, most first (Group-Object, then Sort-Object Count -Descending).
fn tally(items: &[String]) -> Value {
    let mut groups: Vec<(String, usize)> = group_by(items, |x| x.clone()).into_iter().map(|(k, g)| (k, g.len())).collect();
    ps_sort(&mut groups, |a, b| b.1.cmp(&a.1));
    let mut map = Map::new();
    for (k, n) in groups {
        map.insert(k, json!(n));
    }
    Value::Object(map)
}

/// `Bridge-Counts`: the semantic Bridges of book `key`, an actual distinct directed pair of
/// substantive Cards (idea, finding, event, model, part; not a notation table) in different
/// buckets (a book, `human`, `unattributed`), joined by a kinded Link that is not "refers".
/// `island` is true when there are none, null when a tree could not be read (`complete` false)
/// or no substantive Card outside the book exists (`peers` 0).
fn bridge_counts(graph_cards: &[Value], index: &book_of::BookIndex, graph_complete: bool, key: &str) -> Value {
    let category = |c: &Value| index.category.get(&field(c, "address")).cloned().unwrap_or_default();
    let graph_by_address: HashMap<String, &Value> = graph_cards.iter().map(|c| (field(c, "address"), c)).collect();
    let endpoint = |c: &Value| {
        field(c, "kind") == "regular" && ["idea", "finding", "event", "model", "part"].contains(&category(c).as_str()) && !NOTATION.is_match(&field(c, "title"))
    };
    let bucket = |c: &Value| -> String {
        if field(c, "workspace") == "human" {
            return "human".into();
        }
        index.book_of.get(&field(c, "address")).cloned().unwrap_or_else(|| "unattributed".into())
    };
    let mut books_n: [BTreeMap<String, i64>; 2] = [BTreeMap::new(), BTreeMap::new()];
    let mut human = [0i64; 2];
    let mut unattributed = [0i64; 2];
    let mut seen = HashSet::new();
    for carrier in graph_cards {
        if !endpoint(carrier) {
            continue;
        }
        let links = str_list(&carrier["links"]);
        let Some(kinds) = carrier["kinds"].as_object() else { continue };
        for (at, kind) in kinds {
            if s(kind) == "refers" || !links.contains(at) {
                continue;
            }
            let Some(target) = graph_by_address.get(at) else { continue };
            if !endpoint(target) || !seen.insert(format!("{}\0{at}", field(carrier, "address"))) {
                continue;
            }
            let (from, to) = (bucket(carrier), bucket(target));
            if from == to {
                continue;
            }
            for (d, (ours, other)) in [(&from, &to), (&to, &from)].into_iter().enumerate() {
                if ours != key {
                    continue;
                }
                match other.as_str() {
                    "human" => human[d] += 1,
                    "unattributed" => unattributed[d] += 1,
                    _ => *books_n[d].entry(other.clone()).or_default() += 1,
                }
            }
        }
    }
    let peers = graph_cards.iter().filter(|c| endpoint(c) && bucket(c) != key).count();
    // (PowerShell wrote `out` and `in` from an unordered @{}: their keys came in a random order.)
    let side = |d: usize| json!({ "books": books_n[d], "human": human[d], "unattributed": unattributed[d] });
    let mut island = Value::Null;
    if graph_complete && peers > 0 {
        let total: i64 = (0..2).map(|d| human[d] + unattributed[d] + books_n[d].values().sum::<i64>()).sum();
        island = json!(total == 0);
    }
    json!({ "out": side(0), "in": side(1), "island": island, "complete": graph_complete, "peers": peers })
}

fn str_list(value: &Value) -> Vec<String> {
    ps_items(value).iter().map(s).collect()
}

/// What `run` returns: the progress lines and the summary.
pub struct Outcome {
    pub summary: Value,
}

/// The audit; `host` prints a progress line (Write-Host).
pub fn run(zt: &mut Zt, p: &Params, host: &mut dyn FnMut(&str)) -> Result<Outcome> {
    let out_dir = p.out.clone();
    std::fs::create_dir_all(&out_dir)?;
    let sw = Instant::now();
    let secs = |sw: &Instant| ps_int(sw.elapsed().as_secs_f64());

    // The public global report needs every category, book attribution and
    // Bridge endpoint. Load that graph once, without model projections per Card.
    let revision = zt.cards(&[], "text")?.archive_revision;
    let mut all_trees = crate::snapshot::read_title_trees(zt)?;
    if all_trees.iter().any(|tree| tree["archive_revision"].as_u64() != Some(revision)) {
        bail!("incomplete audit inputs: changing title view");
    }
    for tree in &mut all_trees {
        let rows = ps_items(&tree["cards"]);
        tree["count"] = json!(rows.len());
        tree["title"] = rows.iter().find(|card| field(card,"edge")=="root")
            .context("incomplete audit catalogue: missing tree root")?["title"].clone();
    }
    let trees: Vec<Value> = all_trees.iter().filter(|t| field(t, "workspace") == "agent").cloned().collect();
    let all_books: Vec<String> = trees.iter().filter(|t| eq(&field(t, "kind"), "literature")).map(|t| field(t, "tree")).collect();
    let mut books = all_books.clone();
    if !p.citation_key.is_empty() {
        if !books.contains(&p.citation_key) {
            bail!("no agent Literature tree `{}`", p.citation_key);
        }
        books = vec![p.citation_key.clone()];
    }
    let mut tree_info: HashMap<String, Value> = HashMap::new();
    for t in &trees {
        tree_info.insert(ci(&field(t, "tree")), t.clone());
    }
    let topic_trees: Vec<String> = trees.iter().filter(|t| eq(&field(t, "kind"), "topic")).map(|t| field(t, "tree")).collect();
    let addresses: Vec<String> = all_trees.iter().flat_map(|tree|ps_items(&tree["cards"]))
        .map(|card|field(&card,"address")).collect();
    let read = zt.cards(&addresses, "text")?;
    if read.archive_revision != revision {bail!("incomplete audit inputs: changing graph view");}
    let graph_complete = true;
    let mut graph_cards = read.cards;
    let mut incoming: HashMap<String,usize> = HashMap::new();
    for card in &graph_cards {
        for target in ps::select_unique(str_list(&card["links"])) {
            if target != field(card, "address") {
                *incoming.entry(target).or_default() += 1;
            }
        }
    }
    for card in &mut graph_cards {
        card["parent"] = card["parent_address"].clone();
        card["inbound"] = json!(incoming.get(&field(card,"address")).copied().unwrap_or(0));
    }
    let cards: Vec<Value> = graph_cards.iter().filter(|c| field(c, "workspace") == "agent").cloned().collect();
    host(&format!("read {} Cards in {} trees ({} s)", cards.len(), trees.len(), secs(&sw)));

    let index = book_of::book_index(&all_trees, &graph_cards);
    let category = |c: &Value| index.category.get(&field(c, "address")).cloned().unwrap_or_default();
    let book_of = |c: &Value| index.book_of.get(&field(c, "address")).cloned();
    let bridge_counts = |key: &str| bridge_counts(&graph_cards, &index, graph_complete, key);

    let mut scope: Vec<&Value> = cards.iter().filter(|c| book_of(c).is_some_and(|b| is_in(&b, &books))).collect();
    let unattributed: Vec<&Value> = cards.iter().filter(|c| book_of(c).is_none()).collect();
    if p.citation_key.is_empty() {
        scope.extend(unattributed.iter().copied());
    }

    // Complete model contexts are required only for the requested report scope.
    let heads: Vec<&Value> = if p.skip_models { Vec::new() } else { scope.iter().copied().filter(|c| eq(&category(c), "model")).collect() };
    let structures: Vec<Value> = heads.iter().map(|head| {
        let at = field(head,"address");
        let model = zt.call("get_model",json!({"at":at}))
            .with_context(||format!("incomplete audit model `{at}`"))?;
        if model["head"]["address"].as_str()!=Some(at.as_str()) || !model["parts"].is_array() || !model["build"].is_object() {
            bail!("incomplete audit model `{at}`: missing head, parts or build");
        }
        Ok(model)
    }).collect::<Result<_>>()?;
    // address (case ignored) -> (title, body)
    let mut bodies: HashMap<String, (String, String)> = HashMap::new();
    let mut facets: HashMap<String, String> = HashMap::new();
    for m in &structures {
        if truthy(&m["head"]) {
            bodies.insert(ci(&field(&m["head"], "address")), (field(&m["head"], "title"), field(&m["head"], "body")));
        }
        for part in ps_items(&m["parts"]) {
            bodies.insert(ci(&field(&part, "address")), (field(&part, "title"), field(&part, "body")));
            facets.insert(ci(&field(&part, "address")), field(&part, "facet"));
        }
    }
    let missing: Vec<&Value> = scope
        .iter()
        .copied()
        .filter(|c| !bodies.contains_key(&ci(&field(c, "address"))) && !(p.skip_models && is_in(&category(c), &["model", "part"])))
        .collect();
    let unread: Vec<String> = Vec::new();
    for card in missing {
        bodies.insert(ci(&field(card,"address")),(field(card,"title"),field(card,"body")));
    }
    host(&format!("read {} bodies, {} models ({} s)", bodies.len(), structures.len(), secs(&sw)));

    // --- Card language: zt-check --json ---
    let tmp = ps::join_path(&out_dir, "audit-tmp");
    std::fs::create_dir_all(&tmp)?;
    // I6's Workspace title context: the same complete title projection the audit already
    // read, so a comparison never becomes an edit target and no second read is needed.
    let title_context = json!({
        "complete": true,
        "titles": all_trees.iter().flat_map(|tree| ps_items(&tree["cards"]))
            .map(|card| json!({"address": field(&card,"address"), "title": field(&card,"title"), "workspace": field(&card,"workspace")}))
            .collect::<Vec<_>>()
    });
    let mut fault_lines: Vec<String> = Vec::new();
    let mut fault_seen: HashSet<String> = HashSet::new();
    let mut invoke_check = |zt: &mut Zt, entries: &[Value], setting: &str, level: &str| -> Result<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let file = ps::join_path(&tmp, "cards.json");
        let bundle = json!({ "cards": entries, "context": title_context });
        util::write_text(&file, &ps::json_pretty(&bundle))?;
        let check = Check {
            json: file,
            setting: setting.to_string(),
            level: level.to_string(),
            max_sentences: p.max_sentences,
            max_chars: p.max_chars,
            max_steps: p.max_steps,
            ..Check::default()
        };
        for line in check.run(zt)? {
            if FAULT_LINE.is_match(&line) && fault_seen.insert(line.clone()) {
                fault_lines.push(line);
            }
        }
        Ok(())
    };
    let mut in_model: HashSet<String> = HashSet::new();
    for m in &structures {
        if !truthy(&m["head"]) {
            continue;
        }
        let mut line: HashMap<String, Value> = HashMap::new();
        for o in ps_items(&m["build"]["order"]) {
            line.insert(ci(&field(&o, "address")), json!(field(&o, "line")));
        }
        let head = &m["head"];
        let mut entries = vec![json!({ "address": field(head, "address"), "title": field(head, "title"), "body": field(head, "body"), "facet": null, "line": "head" })];
        for part in ps_items(&m["parts"]) {
            let a = field(&part, "address");
            entries.push(json!({
                "address": a, "title": field(&part, "title"), "body": field(&part, "body"), "facet": field(&part, "facet"),
                "line": line.get(&ci(&a)).cloned().unwrap_or(Value::Null)
            }));
            in_model.insert(ci(&a));
        }
        in_model.insert(ci(&field(head, "address")));
        invoke_check(zt, &entries, &field(&m["build"], "setting"), &field(m, "level"))?;
    }
    // The link graph of the Workspace: who links each Card, and how many Idea Cards link it with a kind word.
    let mut inbound_from: HashMap<String, Vec<String>> = HashMap::new();
    let mut kinded_from: HashMap<String, i64> = HashMap::new();
    for c in &graph_cards {
        let kinded = ps::keys(&c["kinds"]);
        let cat = category(c);
        for t in ps::select_unique(str_list(&c["links"])) {
            inbound_from
                .entry(ci(&t))
                .or_default()
                .push(field(c, "address"));
            if t != field(c, "address") && is_in(&t, &kinded) && eq(&cat, "idea") {
                *kinded_from.entry(ci(&t)).or_default() += 1;
            }
        }
    }
    let orphaned = |c: &Value| {
        zt_check::graph::is_orphan(
            &field(c, "address"),
            &category(c),
            inbound_from
                .get(&ci(&field(c, "address")))
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        )
    };
    let orphan_targets = if p.orphan_targets.is_empty() {
        scope.clone()
    } else {
        if p.citation_key.is_empty() {
            bail!("OrphanTargets requires one CitationKey");
        }
        let text = util::read_text(&p.orphan_targets)
            .context("incomplete declared orphan target scope")?;
        let mut targets = std::collections::BTreeSet::new();
        for at in text.lines().map(str::trim).filter(|at| !at.is_empty()) {
            targets.insert(at.to_string());
        }
        targets
            .into_iter()
            .map(|at| {
                let card = graph_cards
                    .iter()
                    .find(|c| field(c, "address") == at)
                    .with_context(|| format!("incomplete orphan target `{at}`"))?;
                if field(card, "workspace") != "agent" {
                    bail!("orphan target `{at}` must be an agent Card");
                }
                let tree = field(card, "tree");
                if index.book_set.contains(&tree) && tree != p.citation_key {
                    bail!("orphan target `{at}` belongs to another Literature tree");
                }
                Ok(card)
            })
            .collect::<Result<Vec<_>>>()?
    };
    let orphan_entries: Vec<_> = orphan_targets
        .iter()
        .map(|c| {
            let mut card = (*c).clone();
            card["category"] = json!(category(c));
            card["inbound"] = json!(
                inbound_from
                    .get(&ci(&field(c, "address")))
                    .cloned()
                    .unwrap_or_default()
            );
            card["inbound_kinded"] = json!(
                kinded_from
                    .get(&ci(&field(c, "address")))
                    .copied()
                    .unwrap_or(0)
            );
            card
        })
        .collect();
    let mut orphan_check = zt_check::graph::orphan_report(&orphan_entries, revision, &[])?;
    orphan_check["changes_since"] = json!("command_start");
    // A book's term table: a Card of its Literature tree whose title starts with 术语表.
    let mut term_card: HashMap<String, String> = HashMap::new();
    for c in &cards {
        let tree = field(c, "tree");
        if is_in(&tree, &all_books) && TERMS_TITLE.is_match(&field(c, "title")) && !term_card.contains_key(&ci(&tree)) {
            term_card.insert(ci(&tree), field(c, "address"));
        }
    }
    let rest: Vec<Value> = scope
        .iter()
        .filter(|c| !in_model.contains(&ci(&field(c, "address"))) && bodies.contains_key(&ci(&field(c, "address"))))
        .map(|c| {
            let a = field(c, "address");
            let (title, body) = bodies[&ci(&a)].clone();
            json!({
                "address": a, "title": title, "body": body,
                "facet": facets.get(&ci(&a)).cloned().map(Value::String).unwrap_or(Value::Null), "line": null,
                "category": category(c), "tree": field(c, "tree"), "workspace": field(c, "workspace"), "book": book_of(c).map(Value::String).unwrap_or(Value::Null),
                "parent": field(c, "parent"), "links": str_list(&c["links"]), "kinds": ps::keys(&c["kinds"]),
                // The kinded relation of each Link, which I7 reads; the array above keeps its order.
                "relations": c["kinds"].clone(),
                "inbound": inbound_from.get(&ci(&a)).cloned().unwrap_or_default(),
                "inbound_kinded": kinded_from.get(&ci(&a)).copied().unwrap_or(0)
            })
        })
        .collect();
    // The Cards the layer rules read, with their bodies: card builds its jobs from them.
    let addresses: String = cards.iter().map(|c| format!("{}{}", field(c, "address"), ps::NL)).collect();
    util::write_text(ps::join_path(&out_dir, "audit-addresses.txt"), &addresses)?;
    util::write_text(ps::join_path(&out_dir, "audit-cards.json"), &ps::json_pretty(&Value::Array(rest.clone())))?;
    // One call per book, with the book's term table (R6) when it has one.
    for (name, group) in group_by(&rest, |e| field(e, "book")) {
        let setting = if !name.is_empty() { term_card.get(&ci(&name)).cloned().unwrap_or_default() } else { String::new() };
        invoke_check(zt, &group, &setting, "")?;
    }
    let _ = std::fs::remove_dir_all(&tmp);
    let faults: Vec<Fault> = fault_lines
        .iter()
        .map(|line| {
            let mut parts = line.splitn(3, ' ');
            Fault {
                address: parts.next().unwrap_or_default().to_string(),
                rule: parts.next().unwrap_or_default().to_string(),
                line: line.clone(),
            }
        })
        .collect();
    host(&format!("Card language: {} faults ({} s)", faults.len(), secs(&sw)));

    // --- the Workspace ---
    let hubs = zt.call("list_hubs",json!({"workspace":"agent"}))
        .context("incomplete audit hub read")?;
    let broken_read = zt.call("list_broken_links",json!({"workspace":"agent"}))
        .context("incomplete audit Broken link read")?;
    let broken = broken_read["broken"].as_array().context("incomplete audit Broken link list")?;
    if !hubs["hubs"].is_array() || !hubs["missing"].is_array() || !hubs["counts"].is_object() {
        bail!("incomplete audit hub data");
    }
    let broken_from: Vec<String> = broken
        .iter()
        .map(|b| {
            for k in ["from", "source", "address"] {
                if truthy(&b[k]) {
                    return field(b, k);
                }
            }
            String::new()
        })
        .collect();

    // --- per book ---
    let limits = json!({ "MaxSentences": p.max_sentences, "MaxChars": p.max_chars, "MaxSteps": p.max_steps });
    let unattributed_style = if !p.citation_key.is_empty() {
        Value::Null
    } else {
        let set: HashMap<String, String> = unattributed.iter().map(|c| (ci(&field(c, "address")), category(c))).collect();
        let uf: Vec<&Fault> = faults.iter().filter(|f| set.contains_key(&ci(&f.address))).collect();
        let mut by_cat = Map::new();
        for (name, group) in group_by(&uf, |f| set[&ci(&f.address)].clone()) {
            by_cat.insert(name, tally(&group.iter().map(|f| f.rule.clone()).collect::<Vec<_>>()));
        }
        json!({
            "faults": uf.len(),
            "cards_with_faults": ps_sort_unique(&uf.iter().map(|f| f.address.clone()).collect::<Vec<_>>()).len(),
            "by_rule": tally(&uf.iter().map(|f| f.rule.clone()).collect::<Vec<_>>()),
            "by_category": by_cat
        })
    };
    let hub_entries = ps_items(&hubs["hubs"]);
    let workspace = json!({
        "trees": trees.len(),
        "topic_trees": topic_trees.len(),
        "books": all_books.len(),
        "cards": cards.len(),
        "cards_by_category": tally(&cards.iter().map(|c| category(c)).collect::<Vec<_>>()),
        "topic_tree_sizes": quantiles(&trees.iter().filter(|t| eq(&field(t, "kind"), "topic")).map(|t| t["count"].as_i64().unwrap_or(0)).collect::<Vec<_>>()),
        "hubs": {
            "counts": hubs["counts"].clone(),
            "missing": ps_count(&hubs["missing"]),
            "with_faults": hub_entries.iter().filter(|h| ps_count(&h["faults"]) > 0).count(),
            "missing_list": ps_array(&hubs["missing"])
        },
        "broken_links": broken.len(),
        "unattributed": {
            "count": unattributed.len(),
            "by_category": tally(&unattributed.iter().map(|c| category(c)).collect::<Vec<_>>()),
            "orphans": unattributed.iter().filter(|c| orphaned(c)).map(|c| field(c, "address")).collect::<Vec<_>>(),
            "style": unattributed_style,
            "list": unattributed.iter().map(|c| field(c, "address")).collect::<Vec<_>>()
        }
    });
    let advisory = ["I1", "I6", "E5", "L3"];
    let mut book_reports: Vec<Value> = Vec::new();
    for key in &books {
        let mine: Vec<&Value> = scope.iter().copied().filter(|c| book_of(c).is_some_and(|b| eq(&b, key))).collect();
        let mine_set: HashMap<String, &Value> = mine.iter().map(|c| (ci(&field(c, "address")), *c)).collect();
        let regular: Vec<&Value> = mine.iter().copied().filter(|c| !eq(&category(c), "root")).collect();
        let writing: Vec<&Value> = mine.iter().copied().filter(|c| is_in(&category(c), &["idea", "event", "finding"])).collect();

        // Topic trees with Cards of the book.
        let in_topics: Vec<&Value> = mine.iter().copied().filter(|c| is_in(&field(c, "tree"), &topic_trees)).collect();
        let tree_rows: Vec<Value> = group_by(&in_topics, |c| field(c, "tree"))
            .into_iter()
            .map(|(name, group)| {
                let t = tree_info.get(&ci(&name)).cloned().unwrap_or(Value::Null);
                json!({ "tree": name, "title": field(&t, "title"), "cards": group.len(), "size": t["count"].as_i64().unwrap_or(0) })
            })
            .collect();
        let majority = tree_rows.iter().filter(|r| r["cards"].as_i64().unwrap_or(0) * 2 > r["size"].as_i64().unwrap_or(0)).count();

        // Card language and Card layer. The standalone heuristics (R20, R21) are advice in
        // the pilot, so they are reported apart from the blocking style faults (D138).
        let my_style: Vec<&Fault> = faults.iter().filter(|f| mine_set.contains_key(&ci(&f.address)) && ps::starts_ci(&f.rule, "R")).collect();
        let my_faults: Vec<&Fault> = my_style.iter().copied().filter(|f| !f.line.contains("(advisory)")).collect();
        let my_advice: Vec<&Fault> = my_style.iter().copied().filter(|f| f.line.contains("(advisory)")).collect();
        let my_layer: Vec<&Fault> = faults.iter().filter(|f| mine_set.contains_key(&ci(&f.address)) && !ps::starts_ci(&f.rule, "R")).collect();
        let mut rule_by_category = Map::new();
        for (name, group) in group_by(&my_faults, |f| category(mine_set[&ci(&f.address)])) {
            rule_by_category.insert(name, tally(&group.iter().map(|f| f.rule.clone()).collect::<Vec<_>>()));
        }

        // Orphans, source lines, kinded Links.
        let orphan_scope=if p.orphan_targets.is_empty() {regular.clone()} else {orphan_targets.clone()};
        let orphans: Vec<&Value> = orphan_scope.iter().copied().filter(|c| orphaned(c)).collect();
        let no_source: Vec<&Value> = writing
            .iter()
            .copied()
            .filter(|c| {
                !str_list(&c["links"]).iter().any(|t| {
                    book_of::source_target(t, &index.book_set) && book_of::link_book(t, &index.book_set).is_some_and(|b| eq(b, key))
                })
            })
            .collect();
        let mut refers = 0usize;
        let mut unkinded: Vec<&Value> = Vec::new();
        for c in &writing {
            let kinded = ps::keys(&c["kinds"]);
            let others: Vec<String> = str_list(&c["links"]).into_iter().filter(|t| !book_of::source_target(t, &index.book_set)).collect();
            refers += others.iter().filter(|t| !is_in(t, &kinded)).count();
            let parent = field(c, "parent");
            if !kinded.iter().any(|k| !eq(k, &parent)) {
                unkinded.push(c);
            }
        }

        // Models.
        let mut model_rows: Vec<Value> = Vec::new();
        for (head, m) in heads.iter().zip(&structures) {
            if !book_of(head).is_some_and(|b| eq(&b, key)) {
                continue;
            }
            let b = &m["build"];
            let n = json!({
                "closure": ps_count(&b["closure"]), "forward": ps_count(&b["forward"]),
                "unanchored": ps_count(&b["unanchored"]), "beyond_level": ps_count(&b["beyond_level"])
            });
            let num = |k: &str| n[k].as_i64().unwrap_or(0);
            let open = num("closure") + num("forward") + num("unanchored") + if eq(&field(m, "level"), "concept") { num("beyond_level") } else { 0 };
            let derived: Vec<Value> = ps_items(&m["parts"]).into_iter().filter(|part| ps::starts_with(&field(part, "facet"), "推导")).collect();
            let explained = derived.iter().filter(|part| THINKING.is_match(&field(part, "body")) && READING.is_match(&field(part, "body"))).count();
            model_rows.push(json!({
                "address": field(head, "address"), "title": field(head, "title"), "level": m["level"].clone(),
                "parts": ps_count(&m["parts"]), "issues": ps_count(&m["issues"]), "faults": n,
                "self_sufficient": open == 0, "derivations": derived.len(), "explained": explained
            }));
        }

        // Figures: named but not shown.
        let mut shown_labels: Vec<String> = Vec::new();
        let mut shown_set: HashSet<String> = HashSet::new();
        for c in &regular {
            if is_in(&category(c), &["hub", "anchor"]) {
                continue;
            }
            let Some((_, body)) = bodies.get(&ci(&field(c, "address"))) else { continue };
            for m in SHOWN.captures_iter(body) {
                let label = m[1].trim().to_string();
                if shown_set.insert(ci(&label)) {
                    shown_labels.push(label);
                }
            }
        }
        let mut named: Vec<Value> = Vec::new();
        for c in &regular {
            if is_in(&category(c), &["hub", "anchor"]) {
                continue;
            }
            let Some((title, body)) = bodies.get(&ci(&field(c, "address"))) else { continue };
            let plain = IMAGE.replace_all(&format!("{title}\n{body}"), "").into_owned();
            for label in ps_sort_unique(&figure_labels(&plain)) {
                let shown_here = Regex::new(&format!(r"(?i)!\[图\s?{}\]\(artifact:", regex::escape(&label))).unwrap();
                if shown_here.is_match(body) {
                    continue;
                }
                named.push(json!({ "address": field(c, "address"), "figure": label, "known": shown_set.contains(&ci(&label)) }));
            }
        }

        let mut topics_list = tree_rows.clone();
        ps_sort(&mut topics_list, |a, b| (-a["cards"].as_i64().unwrap_or(0)).cmp(&-b["cards"].as_i64().unwrap_or(0)));
        let level_names: Vec<String> = model_rows.iter().map(|r| if truthy(&r["level"]) { s(&r["level"]) } else { "none".into() }).collect();
        let sum = |k: &str| model_rows.iter().map(|r| r[k].as_i64().unwrap_or(0)).sum::<i64>();
        let known = named.iter().filter(|n| n["known"] == true).count();
        let layer_hard: Vec<&&Fault> = my_layer.iter().filter(|f| !is_in(&f.rule, &advisory)).collect();
        book_reports.push(json!({
            "key": key,
            "title": field(tree_info.get(&ci(key)).unwrap_or(&Value::Null), "title"),
            "bridges": bridge_counts(key),
            "cards": mine.len(),
            "cards_by_category": tally(&mine.iter().map(|c| category(c)).collect::<Vec<_>>()),
            "cards_in_topic_trees": in_topics.len(),
            "unread": unread.iter().filter(|a| mine_set.contains_key(&ci(a))).cloned().collect::<Vec<_>>(),
            "topics": {
                "trees": tree_rows.len(), "majority": majority,
                "sizes": quantiles(&tree_rows.iter().map(|r| r["size"].as_i64().unwrap_or(0)).collect::<Vec<_>>()),
                "list": topics_list
            },
            "style": {
                "cards_checked": mine.iter().filter(|c| bodies.contains_key(&ci(&field(c, "address")))).count(),
                "faults": my_faults.len(),
                "advisory": my_advice.len(),
                "cards_with_faults": ps_sort_unique(&my_faults.iter().map(|f| f.address.clone()).collect::<Vec<_>>()).len(),
                "by_rule": tally(&my_faults.iter().map(|f| f.rule.clone()).collect::<Vec<_>>()),
                "advice_by_rule": tally(&my_advice.iter().map(|f| f.rule.clone()).collect::<Vec<_>>()),
                "by_category": rule_by_category
            },
            "layer": {
                "faults": layer_hard.len(),
                "advisory": my_layer.len() - layer_hard.len(),
                "cards_with_faults": ps_sort_unique(&layer_hard.iter().map(|f| f.address.clone()).collect::<Vec<_>>()).len(),
                "by_rule": tally(&my_layer.iter().map(|f| f.rule.clone()).collect::<Vec<_>>()),
                "term_table": term_card.get(&ci(key)).cloned().map(Value::String).unwrap_or(Value::Null)
            },
            "orphans": {
                "count": orphans.len(),
                "by_category": tally(&orphans.iter().map(|c| category(c)).collect::<Vec<_>>()),
                "list": orphans.iter().map(|c| field(c, "address")).collect::<Vec<_>>()
            },
            "no_source": {
                "count": no_source.len(), "of": writing.len(),
                "by_category": tally(&no_source.iter().map(|c| category(c)).collect::<Vec<_>>()),
                "list": no_source.iter().map(|c| field(c, "address")).collect::<Vec<_>>()
            },
            "unkinded": {
                "count": unkinded.len(), "of": writing.len(), "refers_links": refers,
                "list": unkinded.iter().map(|c| field(c, "address")).collect::<Vec<_>>()
            },
            "models": {
                "count": model_rows.len(),
                "levels": tally(&level_names),
                "not_self_sufficient": model_rows.iter().filter(|r| !truthy(&r["self_sufficient"])).count(),
                "with_issues": model_rows.iter().filter(|r| truthy(&r["issues"])).count(),
                "derivations": sum("derivations"),
                "explained": sum("explained"),
                "list": model_rows
            },
            "figures": {
                "shown_labels": shown_labels.len(),
                "missing_known": known, "missing_unknown": named.len() - known,
                "list": named
            },
            "broken_links": broken_from.iter().filter(|a| mine_set.contains_key(&ci(a))).count()
        }));
    }
    let scope_name = if p.citation_key.is_empty() { "agent".to_string() } else { p.citation_key.clone() };
    if zt.cards(&[], "text")?.archive_revision != revision {
        bail!("incomplete audit inputs: changing read view");
    }
    let seconds = secs(&sw);
    let report = json!({
        "generated": ps::local_minute(),
        "complete": true,
        "archive_revision": revision,
        "scope": scope_name,
        "orphan_check": orphan_check,
        "limits": limits,
        "seconds": seconds,
        "workspace": workspace,
        "books": book_reports
    });

    // --- files ---
    util::write_text(ps::join_path(&out_dir, "audit.json"), &ps::json_pretty(&report))?;
    let mut sorted = faults.clone();
    ps_sort(&mut sorted, |a, b| culture_cmp(&a.address, &b.address));
    let fault_text: String = sorted.iter().map(|f| format!("{}{}", f.line, ps::NL)).collect();
    util::write_text(ps::join_path(&out_dir, "audit-faults.txt"), &fault_text)?;
    util::write_text(ps::join_path(&out_dir, "audit.md"), &markdown::render(&report, p))?;

    // --- summary line ---
    let w = &report["workspace"];
    let summary = json!({
        "scope": report["scope"], "out": p.out, "seconds": seconds, "cards": cards.len(),
        "hubs_missing": w["hubs"]["missing"], "broken_links": w["broken_links"], "unattributed": w["unattributed"]["count"],
        "books": ps_items(&report["books"]).iter().map(|b| json!({
            "key": b["key"], "cards": b["cards"], "topic_trees": b["topics"]["trees"], "style_faults": b["style"]["faults"],
            "style_advisory": b["style"]["advisory"],
            "bridges": b["bridges"],
            "layer_faults": b["layer"]["faults"], "layer_advisory": b["layer"]["advisory"],
            "orphans": b["orphans"]["count"], "no_source": b["no_source"]["count"], "unkinded": b["unkinded"]["count"],
            "models": b["models"]["count"], "not_self_sufficient": b["models"]["not_self_sufficient"],
            "derivations": b["models"]["derivations"], "explained": b["models"]["explained"],
            "figures_missing": b["figures"]["missing_known"].as_i64().unwrap_or(0) + b["figures"]["missing_unknown"].as_i64().unwrap_or(0)
        })).collect::<Vec<_>>()
    });
    Ok(Outcome { summary })
}

/// `zt-digest audit`.
pub fn cli(argv: &[String]) -> Result<i32> {
    let params = ["CitationKey", "Out", "MaxSentences", "MaxChars", "MaxSteps", "Throttle", "Chunk", "Sample", "OrphanTargets"];
    let a = Args::parse(&util::bind_positional(argv, &params)?, &params, &["SkipModels"])?;
    // Accept existing wrappers' integer options. The bulk path owns its
    // server-limited chunk size and always uses this command's one client.
    let _legacy_read_options = (a.int_or("Throttle",1)?,a.int_or("Chunk",200)?);
    let p = Params {
        citation_key: a.str("CitationKey"),
        orphan_targets: a.str("OrphanTargets"),
        out: a.required("Out")?,
        max_sentences: a.int_or("MaxSentences", 12)?,
        max_chars: a.int_or("MaxChars", 400)?,
        max_steps: a.int_or("MaxSteps", 6)?,
        sample: a.int_or("Sample", 15)?,
        skip_models: a.flag("SkipModels"),
    };
    let mut zt = crate::zt::start()?;
    let outcome = run(&mut zt, &p, &mut |line| {
        let _ = util::print_line(line);
    })?;
    util::print_line(&ps::json_compact(&outcome.summary))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figure_names_follow_the_dotnet_pattern() {
        assert_eq!(figure_labels("见图 2.C.1 与图3，图 IV 和图十二。"), vec!["2.C.1", "3", "IV", "十二"]);
        // A label followed by a dot, a Latin letter, or a Han character after a Chinese numeral is no name.
        // (Checked against [regex]::Matches in pwsh: only `X`, a Roman numeral, is a name here.)
        assert_eq!(figure_labels("图 2.1. 图 3a 图三角 图X"), vec!["X"]);
        assert_eq!(figure_labels("图2.1：说明"), vec!["2.1"]);
    }

    fn bridge_fixture() -> (Vec<Value>, Vec<Value>) {
        let trees = vec![
            json!({ "tree": "New2026", "kind": "literature", "workspace": "agent" }),
            json!({ "tree": "Old2020", "kind": "literature", "workspace": "agent" }),
            json!({ "tree": "Lone2030", "kind": "literature", "workspace": "agent" }),
        ];
        let card = |address: &str, tree: &str, links: Value, kinds: Value| {
            json!({ "address": address, "tree": tree, "kind": "regular", "type": "idea", "workspace": "agent", "title": "观点", "links": links, "kinds": kinds })
        };
        let cards = vec![
            json!({ "address": "New2026", "tree": "New2026", "kind": "literature", "type": "idea", "workspace": "agent", "links": [], "kinds": {} }),
            card("New2026/1", "New2026", json!([]), json!({})),
            card("New2026/1|a", "New2026", json!(["Old2020/1|a", "New2026"]), json!({ "Old2020/1|a": "corresponds" })),
            card("Old2020/1", "Old2020", json!([]), json!({})),
            card("Old2020/1|a", "Old2020", json!(["Old2020"]), json!({})),
            card("Lone2030/1", "Lone2030", json!([]), json!({})),
            card("Lone2030/1|a", "Lone2030", json!(["Lone2030"]), json!({})),
        ];
        (trees, cards)
    }

    // The assertion of bridge-planning.tests.ps1 that made a get_tree read fail through a fake
    // helper ("incomplete external reads leave semantic isolation unknown rather than true"):
    // the Rust audit reads through its own `zt mcp`, so the seam is gone and the rule is tested here.
    #[test]
    fn an_incomplete_read_leaves_the_island_unknown() {
        let (trees, cards) = bridge_fixture();
        let index = book_of::book_index(&trees, &cards);
        let read = [Some(json!({ "cards": [] })), None, Some(json!({ "cards": [] }))];
        let complete = read.iter().all(Option::is_some) && read.len() == trees.len();
        assert!(!complete);
        let b = bridge_counts(&cards, &index, complete, "New2026");
        assert_eq!(b["complete"], json!(false));
        assert!(b["island"].is_null());
        // The same graph read in full: one Bridge out of New2026, one into Old2020, and Lone2030 is an island.
        let b = bridge_counts(&cards, &index, true, "New2026");
        assert_eq!(b["out"]["books"]["Old2020"], json!(1));
        assert_eq!(b["island"], json!(false));
        assert_eq!(bridge_counts(&cards, &index, true, "Old2020")["in"]["books"]["New2026"], json!(1));
        let lone = bridge_counts(&cards, &index, true, "Lone2030");
        assert_eq!(lone["island"], json!(true));
        assert!(lone["peers"].as_i64().unwrap() > 0);
        // A book alone: no peers, so not an island.
        let alone: Vec<Value> = cards.iter().filter(|c| field(c, "tree") == "Lone2030").cloned().collect();
        let index = book_of::book_index(&trees, &alone);
        let b = bridge_counts(&alone, &index, true, "Lone2030");
        assert!(b["island"].is_null());
        assert_eq!(b["peers"], json!(0));
    }

    #[test]
    fn quantiles_and_tallies() {
        assert_eq!(quantiles(&[5, 1, 3, 2]), json!({ "n": 4, "min": 1, "p25": 1, "median": 2, "p75": 3, "max": 5 }));
        assert_eq!(quantiles(&[]), json!({ "n": 0 }));
        let t = tally(&["idea", "anchor", "idea", "event", "anchor", "idea"].map(String::from));
        assert_eq!(ps::json_compact(&t), r#"{"idea":3,"anchor":2,"event":1}"#);
    }
}
