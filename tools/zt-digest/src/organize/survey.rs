//! `zt-digest organize-survey` (organize-survey.ps1): everything Opus needs to decide in the
//! Organize stage, and nothing it must find by reading the trees. Read-only.
//!
//! It runs the audit on the book in-process (into <Work>\audit) and writes survey.md (for Opus)
//! and survey.json: the book's Topics; duplicate candidates (pairs of agent Topics, one of them the
//! book's, whose titles are similar (character bigrams, -Similar) or one inside the other, or that
//! link each other -CrossLinks times); small Topics (-Small Cards or fewer); merge proposals of the
//! card cycles and the audit (I6, I2); models with faults and the architect's notes (-Notes);
//! broken Links and missing hubs. Prints one compact JSON summary line last.

use super::common::{self, Sb, prop, pstr};
use crate::args::Args;
use crate::ps::{self, CiMap, eq, field};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+$").unwrap());
static NOT_LETTERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[\s\p{P}\p{S}]").unwrap());
static I6: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^(\S+) I6 \(advisory\) a title very similar to \[\[([^\]]+)\]\]").unwrap());
static I2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(\S+) I2 \((\d+) characters < \d+\) merge candidate").unwrap());

struct Params {
    staging: String,
    work: String,
    notes: String,
    similar: f64,
    cross_links: i64,
    small: i64,
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Staging", "Work", "Notes", "Similar", "CrossLinks", "Small"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &[])?;
    let p = Params {
        staging: common::mandatory(&a, "Staging")?,
        work: common::mandatory(&a, "Work")?,
        notes: a.str("Notes"),
        similar: a.float_or("Similar", 0.5)?,
        cross_links: a.int_or("CrossLinks", 4)?,
        small: a.int_or("Small", 3)?,
    };
    std::fs::create_dir_all(&p.work)?;
    let manifest = common::manifest(&p.staging)?;
    let key = pstr(&manifest, "citation_key");
    let audit_dir = ps::join(&p.work, "audit");
    let mut zt = crate::zt::start()?;
    common::run_audit(&mut zt, &key, &audit_dir)?;
    let audit = common::read_json(&ps::join(&audit_dir, "audit.json"))?;
    let book = ps::at(&ps::pipe(&audit["books"]), 0);
    let cards = ps::ps_items(&common::read_json(&ps::join(&audit_dir, "audit-cards.json"))?);
    let faults = ps::read_lines(&ps::join(&audit_dir, "audit-faults.txt"))?;
    let listed = common::read_first(&mut zt, "list_trees", json!({ "workspace": "agent" }))?;
    let trees: Vec<Value> = ps::each(prop(&listed, "trees")).into_iter().filter(|t| eq(&pstr(t, "kind"), "topic")).cloned().collect();
    // Tree Lock S16 (#139): `list_trees` answers whether an active Lock governs each Topic; the
    // root's own read gives its Lock id and start, so a planner sees the tree it may not organize
    // without this step becoming a Lock authority.
    let locked: Vec<Value> = trees.iter().filter(|t| ps::truthy(&t["locked"])).map(|t| json!(pstr(t, "root"))).collect();
    let mut locks = serde_json::Map::new();
    if !locked.is_empty() {
        let read = common::read_first(&mut zt, "get_cards", json!({ "addresses": locked, "view": "text" }))?;
        for card in ps::each(&read["cards"]) {
            let lock = &card["lock"];
            if lock.is_object() {
                locks.insert(
                    pstr(card, "tree"),
                    json!({"lock_id": lock["lock_id"], "tree": lock["tree"], "since": lock["since"]}),
                );
            }
        }
    }
    survey(&p, &manifest, &key, &audit, &book, &cards, &faults, &trees, Value::Object(locks))
}

/// `Get-Bigrams`: the set of character bigrams (UTF-16 units, as .NET's Substring) of a title
/// without white space, punctuation, and symbols.
fn bigrams(title: &str) -> HashSet<[u16; 2]> {
    let units: Vec<u16> = NOT_LETTERS.replace_all(title, "").encode_utf16().collect();
    units.windows(2).map(|w| [w[0], w[1]]).collect()
}

/// `Get-Similarity`: the Dice coefficient of two bigram sets (0 when one is empty).
fn similarity(a: Option<&HashSet<[u16; 2]>>, b: Option<&HashSet<[u16; 2]>>) -> f64 {
    let (Some(a), Some(b)) = (a, b) else { return 0.0 };
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let common = a.iter().filter(|x| b.contains(*x)).count();
    2.0 * common as f64 / (a.len() + b.len()) as f64
}

/// `[Math]::Round($x, 2)`: scaled, rounded half to even, scaled back.
fn round2(x: f64) -> f64 {
    if x.abs() < 1e16 { (x * 100.0).round_ties_even() / 100.0 } else { x }
}

/// The key of two trees in the cross-Link count: `@($a, $b) | Sort-Object` joined by `~`.
fn pair_key(a: &str, b: &str) -> String {
    let mut pair = [a.to_string(), b.to_string()];
    ps::ps_sort(&mut pair, |x, y| ps::culture_cmp(x, y));
    format!("{}~{}", pair[0], pair[1])
}

/// `[int]$_.count` of a tree that list_trees returned: its `count`, or 1 (PowerShell's own
/// Count of one object) when it has none.
fn count(tree: &Value) -> Result<i64> {
    match tree.as_object().and_then(|m| m.iter().find(|(k, _)| k.eq_ignore_ascii_case("count"))) {
        Some((_, v)) => ps::int(v),
        None => Ok(1),
    }
}

/// `$value | ConvertTo-Json -Compress`: an array goes down the pipeline item by item, so an
/// empty one writes nothing, one item writes the item, and more write an array.
fn piped_json(value: &Value) -> String {
    match value {
        Value::Array(items) if items.is_empty() => String::new(),
        Value::Array(items) if items.len() == 1 => ps::json_compact(&items[0]),
        other => ps::json_compact(other),
    }
}

/// The `cards*` folders of the staging folder, in the order Get-ChildItem lists them.
fn card_folders(staging: &str) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(staging) else { return Vec::new() };
    let mut names: Vec<String> = read
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.to_lowercase().starts_with("cards"))
        .collect();
    names.sort_by_key(|n| n.to_uppercase());
    names.into_iter().map(|n| ps::join(staging, &n)).collect()
}

/// The Topic pairs that may be duplicates, sorted by similarity, then Links (most first).
fn duplicate_pairs(p: &Params, topics: &[Value], trees: &[Value], mine: &CiMap<Value>, cross: &HashMap<String, i64>) -> Result<Vec<Value>> {
    let mut grams: HashMap<String, HashSet<[u16; 2]>> = HashMap::new();
    for t in trees {
        grams.insert(ps::key(&pstr(t, "tree")), bigrams(&pstr(t, "title")));
    }
    let mut pairs: Vec<Value> = Vec::new();
    for a in topics {
        let a_tree = field(a, "tree");
        let a_title = field(a, "title");
        for b in trees {
            let bt = pstr(b, "tree");
            if eq(&bt, &a_tree) || (mine.contains(&bt) && ps::int(&json!(bt))? < ps::int(&json!(a_tree))?) {
                continue;
            }
            let sim = similarity(grams.get(&ps::key(&a_tree)), grams.get(&ps::key(&bt)));
            let b_title = pstr(b, "title");
            let inside = a_title.contains(b_title.as_str()) || b_title.contains(a_title.as_str());
            let links = cross.get(&pair_key(&a_tree, &bt)).copied().unwrap_or(0);
            if sim >= p.similar || inside || links >= p.cross_links {
                pairs.push(json!({
                    "a": a["root"], "a_title": a_title, "a_size": a["size"],
                    "b": pstr(b, "root"), "b_title": b_title, "b_size": count(b)?,
                    "similarity": round2(sim), "inside": inside, "links": links
                }));
            }
        }
    }
    let num = |v: &Value, k: &str| v[k].as_f64().unwrap_or(0.0);
    ps::ps_sort(&mut pairs, |x, y| {
        (-num(x, "similarity"))
            .partial_cmp(&-num(y, "similarity"))
            .unwrap_or(Ordering::Equal)
            .then_with(|| (-num(x, "links")).partial_cmp(&-num(y, "links")).unwrap_or(Ordering::Equal))
    });
    Ok(pairs)
}

#[allow(clippy::too_many_arguments)]
fn survey(
    p: &Params,
    manifest: &Value,
    key: &str,
    audit: &Value,
    book: &Value,
    cards: &[Value],
    faults: &[String],
    trees: &[Value],
    locks: Value,
) -> Result<i32> {
    // --- Topics ---
    let mut mine: CiMap<Value> = CiMap::new();
    for t in ps::pipe(&book["topics"]["list"]) {
        mine.insert(&field(t, "tree"), t.clone());
    }
    let mut topics: Vec<Value> = Vec::new();
    for t in trees {
        let tree = pstr(t, "tree");
        if let Some(m) = mine.get(&tree) {
            topics.push(json!({
                "root": pstr(t, "root"), "tree": tree, "title": pstr(t, "title"), "size": count(t)?,
                "book_cards": ps::int(&m["cards"])?, "excerpt": pstr(t, "excerpt")
            }));
        }
    }
    // Links between trees, from the book's Cards.
    let mut cross: HashMap<String, i64> = HashMap::new();
    for c in cards {
        let from = field(c, "tree");
        for l in ps::pipe(&c["links"]) {
            let to = ps::s(l).split('/').next().unwrap_or_default().to_string();
            if eq(&to, &from) || !DIGITS.is_match(&to) || !DIGITS.is_match(&from) {
                continue;
            }
            *cross.entry(pair_key(&from, &to)).or_insert(0) += 1;
        }
    }
    let pairs = duplicate_pairs(p, &topics, trees, &mine, &cross)?;
    let small_topics: Vec<Value> = topics
        .iter()
        .filter(|t| t["size"].as_i64().unwrap_or(0) <= p.small && t["book_cards"].as_i64().unwrap_or(0) * 2 > t["size"].as_i64().unwrap_or(0))
        .cloned()
        .collect();

    // --- merge proposals ---
    let mut proposals: Vec<Value> = Vec::new();
    for folder in card_folders(&p.staging) {
        let file = ps::join(&folder, "decisions.json");
        if !ps::exists(&file) {
            continue;
        }
        let d = common::read_json(&file)?;
        for m in ps::pipe(&d["merges"]) {
            proposals.push(json!({ "card": field(m, "address"), "with": field(m, "merge_with"), "why": field(m, "note"), "from": "card cycle" }));
        }
        for n in ps::pipe(&d["orphan_none"]) {
            proposals.push(json!({ "card": field(n, "orphan"), "with": "", "why": format!("no related Card: {}", field(n, "note")), "from": "orphan pass" }));
        }
    }
    for f in faults {
        if let Some(m) = I6.captures(f) {
            proposals.push(json!({ "card": &m[1], "with": &m[2], "why": "similar titles (I6)", "from": "audit" }));
        } else if let Some(m) = I2.captures(f) {
            proposals.push(json!({ "card": &m[1], "with": "", "why": format!("short Card ({} characters, I2)", &m[2]), "from": "audit" }));
        }
    }

    // --- models and notes ---
    let models: Vec<Value> =
        ps::ps_items(&book["models"]["list"]).into_iter().filter(|m| !ps::truthy(&m["self_sufficient"]) || ps::truthy(&m["issues"])).collect();
    let mut note_list: Vec<Value> = Vec::new();
    if !p.notes.is_empty() && ps::exists(&p.notes) {
        note_list = ps::ps_items(&common::read_json(&p.notes)?)
            .into_iter()
            .filter(|n| ps::ps_count(n.get("notes").unwrap_or(&Value::Null)) > 0)
            .collect();
    }

    // --- survey.md ---
    let mut sb = Sb::new();
    let pairs_of = |v: &Value| -> String { v.as_object().map(|m| m.iter().map(|(k, x)| format!("{k} {}", ps::s(x))).collect::<Vec<_>>().join(", ")).unwrap_or_default() };
    let group_count = ps::ps_count(prop(manifest, "groups"));
    sb.line(format!("# Organize survey: {key}")).blank();
    sb.line(format!(
        "《{}》: {} Cards ({}). Anchors {key}/1 … {key}/{group_count}.",
        pstr(manifest, "book_title"),
        ps::s(&book["cards"]),
        pairs_of(&book["cards_by_category"])
    ))
    .blank();
    sb.line("## 1. The book's Topics").blank();
    sb.line("| Topic | Title | Cards | of the book |").line("| --- | --- | --- | --- |");
    let mut by_cards = topics.clone();
    ps::ps_sort(&mut by_cards, |x, y| (-x["book_cards"].as_i64().unwrap_or(0)).cmp(&-y["book_cards"].as_i64().unwrap_or(0)));
    for t in &by_cards {
        sb.line(format!("| `{}` | {} | {} | {} |", field(t, "root"), field(t, "title"), field(t, "size"), field(t, "book_cards")));
    }
    // A Locked tree stays readable and its Cards may be linked, but no organize plan changes it
    // (R332): name it before the decisions are written.
    if let Some(locks) = locks.as_object().filter(|locks| !locks.is_empty()) {
        sb.blank();
        sb.line("## 1b. Locked Topics (read-only: no plan may move or delete their Cards)").blank();
        for t in trees.iter().filter(|t| ps::truthy(&t["locked"])) {
            let entry = locks.get(&ps::key(&pstr(t, "tree"))).cloned().unwrap_or(Value::Null);
            sb.line(format!(
                "- `{}` {}（已锁定，Lock {}）：可读、可链接，不能写入或组织。",
                pstr(t, "root"),
                pstr(t, "title"),
                ps::s(&entry["lock_id"])
            ));
        }
    }
    sb.blank().line("## 2. Duplicate candidates").blank();
    if !pairs.is_empty() {
        sb.line("| Topic A | Topic B | similarity | one inside the other | Links between |").line("| --- | --- | --- | --- | --- |");
        for x in &pairs {
            sb.line(format!(
                "| `{}` {} ({}) | `{}` {} ({}) | {} | {} | {} |",
                field(x, "a"),
                field(x, "a_title"),
                field(x, "a_size"),
                field(x, "b"),
                field(x, "b_title"),
                field(x, "b_size"),
                field(x, "similarity"),
                field(x, "inside"),
                field(x, "links")
            ));
        }
    } else {
        sb.line("None.");
    }
    sb.blank().line(format!("## 3. Small Topics (≤ {} Cards)", p.small)).blank();
    if !small_topics.is_empty() {
        for t in &small_topics {
            sb.line(format!("- `{}` {} ({} Cards)", field(t, "root"), field(t, "title"), field(t, "size")));
        }
    } else {
        sb.line("None.");
    }
    sb.blank().line("## 4. Card merge proposals").blank();
    if !proposals.is_empty() {
        for x in &proposals {
            let with = field(x, "with");
            let with = if with.is_empty() { String::new() } else { format!(" with `{with}`") };
            sb.line(format!("- `{}`{with}: {} ({})", field(x, "card"), field(x, "why"), field(x, "from")));
        }
    } else {
        sb.line("None.");
    }
    sb.blank().line("## 5. Models").blank();
    let m = &book["models"];
    sb.line(format!(
        "{} Model Cards ({}); {} not self-sufficient.",
        ps::s(&m["count"]),
        pairs_of(&m["levels"]),
        ps::s(&m["not_self_sufficient"])
    ))
    .blank();
    for x in &models {
        sb.line(format!(
            "- `{}` {}: level {}, faults {}, issues {}",
            field(x, "address"),
            field(x, "title"),
            field(x, "level"),
            piped_json(&x["faults"]),
            field(x, "issues")
        ));
    }
    if !note_list.is_empty() {
        sb.blank().line("Architect notes (Cards outside a model that should link a part of it):").blank();
        for n in &note_list {
            for t in ps::pipe(n.get("notes").unwrap_or(&Value::Null)) {
                sb.line(format!("- `{}`: {}", field(n, "model"), ps::s(t)));
            }
        }
    }
    let w = &audit["workspace"];
    sb.blank().line("## 6. Links and hubs").blank();
    sb.line(format!("- Broken Links from the book's Cards: {}. In the Workspace: {}.", ps::s(&book["broken_links"]), ps::s(&w["broken_links"])));
    sb.line(format!("- Required hubs missing in the Workspace: {} (the hub stage builds them after Organize).", ps::s(&w["hubs"]["missing"])));
    sb.line(format!(
        "- Card language faults left: {}; Card layer faults left: {}; orphans: {}.",
        ps::s(&book["style"]["faults"]),
        ps::s(&book["layer"]["faults"]),
        ps::s(&book["orphans"]["count"])
    ));
    let survey_md = ps::join(&p.work, "survey.md");
    util::write_text(&survey_md, &sb.0)?;
    let survey = json!({
        "key": key, "title": pstr(manifest, "book_title"), "topics": topics, "pairs": pairs, "small": small_topics,
        "proposals": proposals, "models": models, "notes": note_list, "broken_links": book["broken_links"],
        "hubs_missing": w["hubs"]["missing"], "locks": locks
    });
    util::write_text(ps::join(&p.work, "survey.json"), &ps::json_pretty(&survey))?;
    let last = json!({
        "survey": survey_md, "topics": topics.len(), "pairs": pairs.len(), "small": small_topics.len(), "proposals": proposals.len(),
        "models": models.len(), "notes": note_list.len(), "broken_links": book["broken_links"],
        "locked": survey["locks"].as_object().map_or(0, serde_json::Map::len)
    });
    util::print_line(&ps::json_compact(&last))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_compare_by_bigrams_and_round_as_dotnet() {
        let a = bigrams("剩余控制权");
        let b = bigrams("剩余控制权的配置");
        assert_eq!(a.len(), 4);
        assert_eq!(similarity(Some(&a), Some(&b)), 2.0 * 4.0 / 11.0);
        assert_eq!(round2(2.0 * 4.0 / 11.0), 0.73);
        assert_eq!(round2(0.125), 0.12);
        assert_eq!(similarity(Some(&bigrams("、")), Some(&a)), 0.0);
        assert_eq!(bigrams("A, b!").len(), 1);
        assert_eq!(pair_key("9", "12"), "12~9");
        assert_eq!(ps::json_compact(&json!(round2(1.0))), "1.0");
    }

    #[test]
    fn piped_values_and_counts_as_powershell() {
        assert_eq!(piped_json(&json!({ "closure": 1, "forward": 0 })), r#"{"closure":1,"forward":0}"#);
        assert_eq!(piped_json(&json!(null)), "null");
        assert_eq!(piped_json(&json!([])), "");
        assert_eq!(piped_json(&json!([3])), "3");
        assert_eq!(count(&json!({ "Count": 7 })).unwrap(), 7);
        assert_eq!(count(&json!({ "tree": "1" })).unwrap(), 1);
    }
}
