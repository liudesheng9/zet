//! Literature tree structural audit (`zt-check --tree <key> [--snapshot <file>] [--format json]`).
//!
//! Mirrors `skills/zt-literature-organize/scripts/check.mjs`, using the core Term list role
//! (`term_role == "list"`), strict `source` broken-link attribution, and cycle-safe snapshot checks.

use crate::mcp::Zt;
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, Default)]
pub struct TreeReport {
    pub top: Vec<Value>,
    pub faults: Vec<String>,
    pub ok: bool,
}

impl TreeReport {
    pub fn to_json(&self, key: &str) -> Value {
        json!({
            "key": key,
            "ok": self.ok,
            "top": self.top,
            "faults": self.faults,
        })
    }
}

fn is_whole(c: &Value) -> bool {
    let typ = c.get("type").and_then(Value::as_str).unwrap_or("");
    let edge = c.get("edge").and_then(Value::as_str).unwrap_or("");
    !matches!(typ, "idea" | "event" | "finding") && edge != "root"
}

fn is_top_address(addr: &str) -> bool {
    let Some((_, tail)) = addr.rsplit_once('/') else {
        return false;
    };
    !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit())
}

fn build_children<'a>(list: &'a [Value]) -> HashMap<&'a str, Vec<&'a Value>> {
    let mut kids: HashMap<&'a str, Vec<&'a Value>> = HashMap::new();
    for c in list {
        if let Some(parent) = c.get("parent").and_then(Value::as_str).filter(|s| !s.is_empty()) {
            kids.entry(parent).or_default().push(c);
        }
    }
    kids
}

/// Validate that a tree/snapshot array is non-empty, has unique non-empty addresses, exactly one
/// root (`edge == "root"` with no parent), connected parent pointers for every non-root node,
/// and no parent cycles (M8). Returns the validated single root address.
pub fn validate_tree_nodes(list: &[Value], label: &str) -> Result<String> {
    if list.is_empty() {
        bail!("{label} is empty (expected a tree with a root card)");
    }
    let mut parent_of: HashMap<&str, Option<&str>> = HashMap::new();
    let mut roots: Vec<&str> = Vec::new();
    for (idx, item) in list.iter().enumerate() {
        let Some(obj) = item.as_object() else {
            bail!("{label}[{idx}] must be a JSON object");
        };
        let addr = obj
            .get("address")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .with_context(|| format!("{label}[{idx}] lacks a non-empty string `address`"))?;
        let edge = obj.get("edge").and_then(Value::as_str).unwrap_or("");
        let parent_val = obj.get("parent");
        let parent = match parent_val {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) if s.is_empty() => None,
            Some(Value::String(s)) => Some(s.as_str()),
            Some(_) => bail!("{label}[{idx}] (`{addr}`) has non-string `parent`"),
        };
        if edge == "root" {
            if parent.is_some() {
                bail!("{label} root `{addr}` cannot have a parent");
            }
            roots.push(addr);
        } else if parent.is_none() {
            bail!("{label} non-root node `{addr}` lacks a `parent`");
        }
        if parent_of.insert(addr, parent).is_some() {
            bail!("{label} contains duplicate address `{addr}`");
        }
    }
    if roots.len() != 1 {
        bail!("{label} must have exactly one root card, found {}", roots.len());
    }
    let root_addr = roots[0];
    for (&addr, &parent) in &parent_of {
        if let Some(p) = parent
            && !parent_of.contains_key(p)
        {
            bail!("{label} node `{addr}` references missing parent `{p}`");
        }
    }
    for &start in parent_of.keys() {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut cur = Some(start);
        let mut reached_root = false;
        while let Some(node) = cur {
            if !seen.insert(node) {
                bail!("{label} contains a parent cycle involving `{node}`");
            }
            if node == root_addr {
                reached_root = true;
                break;
            }
            cur = parent_of.get(node).copied().flatten();
        }
        if !reached_root {
            bail!("{label} node `{start}` is not connected to root `{root_addr}`");
        }
    }
    Ok(root_addr.to_string())
}

fn subtree_size<'a>(
    kids: &HashMap<&'a str, Vec<&'a Value>>,
    address: &'a str,
    visited: &mut HashSet<&'a str>,
) -> usize {
    if !visited.insert(address) {
        return 0;
    }
    1 + kids
        .get(address)
        .map(|list| {
            list.iter()
                .map(|k| {
                    let a = k.get("address").and_then(Value::as_str).unwrap_or("");
                    subtree_size(kids, a, visited)
                })
                .sum::<usize>()
        })
        .unwrap_or(0)
}

fn unit_size<'a>(kids: &HashMap<&'a str, Vec<&'a Value>>, c: &'a Value) -> usize {
    let addr = c.get("address").and_then(Value::as_str).unwrap_or("");
    let on_top = is_top_address(addr);
    let mut visited = HashSet::from([addr]);
    1 + kids
        .get(addr)
        .map(|list| {
            list.iter()
                .filter(|k| !(on_top && k.get("edge").and_then(Value::as_str) == Some("direct")))
                .map(|k| {
                    let a = k.get("address").and_then(Value::as_str).unwrap_or("");
                    subtree_size(kids, a, &mut visited)
                })
                .sum::<usize>()
        })
        .unwrap_or(0)
}

/// Whole units whose size changed between two snapshots.
/// Units are matched by `(type, title)` as sorted size multisets.
pub fn unit_changes(old: &[Value], list: &[Value]) -> Vec<String> {
    fn sizes(cards: &[Value]) -> BTreeMap<(String, String), Vec<usize>> {
        let kids = build_children(cards);
        let mut out: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for c in cards.iter().filter(|c| is_whole(c)) {
            let typ = c.get("type").and_then(Value::as_str).unwrap_or("").to_string();
            let title = c.get("title").and_then(Value::as_str).unwrap_or("").to_string();
            out.entry((typ, title)).or_default().push(unit_size(&kids, c));
        }
        for v in out.values_mut() {
            v.sort_unstable();
        }
        out
    }

    let before = sizes(old);
    let now = sizes(list);
    let mut keys: std::collections::BTreeSet<&(String, String)> = before.keys().collect();
    keys.extend(now.keys());
    let mut faults = Vec::new();
    for key in keys {
        let (_, title) = key;
        let clip: String = title.chars().take(40).collect();
        match (before.get(key), now.get(key)) {
            (Some(want), Some(got)) if want == got => continue,
            (Some(want), Some(got)) => {
                let want_s = want.iter().map(usize::to_string).collect::<Vec<_>>().join("+");
                let got_s = got.iter().map(usize::to_string).collect::<Vec<_>>().join("+");
                faults.push(format!("{clip}: {want_s} Cards before, {got_s} now"));
            }
            (Some(want), None) => {
                let want_s = want.iter().map(usize::to_string).collect::<Vec<_>>().join("+");
                faults.push(format!("{clip}: {want_s} Cards before, 0 now (unit missing)"));
            }
            (None, Some(got)) => {
                let got_s = got.iter().map(usize::to_string).collect::<Vec<_>>().join("+");
                faults.push(format!("{clip}: 0 Cards before, {got_s} now (unit added)"));
            }
            (None, None) => unreachable!(),
        }
    }
    faults
}

/// Whether `c` is a Term list head (`term_role == "list"`, falling back to `type == "term"` with
/// absent `term_list` only when `term_role` is omitted in minimal fixtures) or a Cited list head.
fn is_list_head(c: &Value, by_addr: &HashMap<&str, &Value>) -> bool {
    let typ = c.get("type").and_then(Value::as_str).unwrap_or("");
    if typ == "term" {
        if let Some(role) = c.get("term_role").and_then(Value::as_str) {
            return role == "list";
        }
        let has_term_list = c
            .get("term_list")
            .is_some_and(|v| !v.is_null() && v.as_bool() != Some(false) && v.as_str() != Some(""));
        return !has_term_list;
    }
    if typ == "cited" {
        // A Cited list head is a `cited` card whose parent is NOT a `cited` card (Cited Cards are
        // Side children of the Cited list head; adjacent top-line Term + Cited lists stay distinct).
        let parent_is_cited = c
            .get("parent")
            .and_then(Value::as_str)
            .and_then(|p| by_addr.get(p))
            .is_some_and(|p| p.get("type").and_then(Value::as_str) == Some("cited"));
        return !parent_is_cited;
    }
    false
}

fn source_belongs_to_book(source: &str, key: &str) -> bool {
    source == key || source.starts_with(&format!("{key}/"))
}

pub fn check_tree_data(
    key: &str,
    list: &[Value],
    broken: &[Value],
    snapshot: Option<&[Value]>,
) -> TreeReport {
    let kids = build_children(list);
    let by_addr: HashMap<&str, &Value> = list
        .iter()
        .filter_map(|c| c.get("address").and_then(Value::as_str).map(|a| (a, c)))
        .collect();

    let mut top_cards: Vec<&Value> = Vec::new();
    let mut visited_top: HashSet<&str> = HashSet::new();
    let mut cur = list
        .iter()
        .find(|x| x.get("edge").and_then(Value::as_str) == Some("root"));
    while let Some(c) = cur {
        let addr = c.get("address").and_then(Value::as_str).unwrap_or("");
        if !visited_top.insert(addr) {
            break;
        }
        if c.get("edge").and_then(Value::as_str) == Some("direct") {
            top_cards.push(c);
        }
        cur = kids.get(addr).and_then(|ks| {
            ks.iter()
                .copied()
                .find(|k| k.get("edge").and_then(Value::as_str) == Some("direct"))
        });
    }

    let mut top = Vec::with_capacity(top_cards.len());
    for (i, c) in top_cards.iter().enumerate() {
        let addr = c.get("address").and_then(Value::as_str).unwrap_or("");
        let next_size = top_cards
            .get(i + 1)
            .and_then(|n| n.get("address").and_then(Value::as_str))
            .map(|a| subtree_size(&kids, a, &mut HashSet::new()))
            .unwrap_or(0);
        let own = subtree_size(&kids, addr, &mut HashSet::new()).saturating_sub(next_size);
        let loc = addr.strip_prefix(&format!("{key}/")).unwrap_or(addr);
        top.push(json!({
            "address": addr,
            "location": loc,
            "type": c.get("type").and_then(Value::as_str).unwrap_or(""),
            "own": own,
            "title": c.get("title").and_then(Value::as_str).unwrap_or(""),
        }));
    }

    let mut faults = Vec::new();
    let top_addrs: HashSet<&str> = top_cards
        .iter()
        .filter_map(|c| c.get("address").and_then(Value::as_str))
        .collect();
    let list_heads: Vec<&str> = list
        .iter()
        .filter(|c| is_list_head(c, &by_addr))
        .filter_map(|c| c.get("address").and_then(Value::as_str))
        .collect();
    for addr in &list_heads {
        if !top_addrs.contains(addr) {
            faults.push(format!("a list ({addr}) is not on the top line"));
        }
    }
    if let Some(first) = top_cards.iter().position(|c| is_list_head(c, &by_addr))
        && top_cards[first..].iter().any(|c| !is_list_head(c, &by_addr))
    {
        faults.push("a content Card stands after a list".to_string());
    }
    let first_cited = top_cards.iter().position(|c| {
        is_list_head(c, &by_addr) && c.get("type").and_then(Value::as_str) == Some("cited")
    });
    let last_term = top_cards.iter().rposition(|c| {
        is_list_head(c, &by_addr) && c.get("type").and_then(Value::as_str) == Some("term")
    });
    if let (Some(fc), Some(lt)) = (first_cited, last_term)
        && fc < lt
    {
        faults.push("Cited list stands before Term list on the top line (expected Term list, then Cited list)".to_string());
    }

    for b in broken {
        let source = b
            .get("source")
            .or_else(|| b.get("from"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if source_belongs_to_book(source, key) {
            let s = b.to_string();
            let clip: String = s.chars().take(160).collect();
            faults.push(format!("broken: {clip}"));
        }
    }

    if let Some(old) = snapshot {
        if old.len() != list.len() {
            faults.push(format!("{} Cards before, {} now", old.len(), list.len()));
        }
        faults.extend(unit_changes(old, list));
    }

    let ok = faults.is_empty();
    TreeReport { top, faults, ok }
}

pub fn format_text(r: &TreeReport) -> Vec<String> {
    let mut out = Vec::new();
    for t in &r.top {
        let loc = t["location"].as_str().unwrap_or("");
        let typ = t["type"].as_str().unwrap_or("");
        let own = t["own"].as_u64().unwrap_or(0);
        let title: String = t["title"].as_str().unwrap_or("").chars().take(50).collect();
        out.push(format!("{loc:>4} {typ:<6} {own:>5} {title}"));
    }
    if r.faults.is_empty() {
        out.push("no fault".to_string());
    } else {
        out.extend(r.faults.iter().cloned());
    }
    out
}

/// Normalizes a requested tree key or member address to its tree ID (`src/main.rs::tree_id`).
fn expected_tree_id(requested: &str) -> &str {
    let trimmed = requested.trim();
    if trimmed.starts_with('~')
        && let Some((tree, _)) = trimmed.split_once(['#', '/'])
    {
        return tree;
    }
    trimmed
        .split_once('/')
        .map(|(tree, _)| tree)
        .unwrap_or(trimmed)
}

/// Validates the live `get_tree` envelope (`archive_revision`, `tree` matching requested tree ID,
/// `root`, `zone == "literature"`, `complete == true`, and connected `cards` rooted at `tree`) (M8).
pub fn validate_tree_envelope(tree_reply: &Value, requested: &str) -> Result<(u64, String, Vec<Value>)> {
    let expected = expected_tree_id(requested);
    if expected.is_empty() {
        bail!("requested tree key cannot be empty");
    }
    let rev_before = tree_reply["archive_revision"]
        .as_u64()
        .context("get_tree answer lacks `archive_revision`")?;
    let canonical_key = tree_reply["tree"]
        .as_str()
        .filter(|s| !s.is_empty())
        .context("get_tree answer lacks non-empty `tree` key")?
        .to_string();
    if canonical_key != expected {
        bail!(
            "get_tree returned tree `{canonical_key}`, expected `{expected}` (from requested `{requested}`)"
        );
    }
    match tree_reply["zone"].as_str() {
        Some("literature") => {}
        Some(other) => bail!("`{canonical_key}` is in the `{other}` zone, not a Literature tree"),
        None => bail!("get_tree answer for `{canonical_key}` lacks `zone` (expected `literature`)"),
    }
    if tree_reply["complete"].as_bool() != Some(true) {
        bail!("get_tree answer for `{canonical_key}` is not `complete: true`");
    }
    let declared_root = tree_reply["root"]
        .as_str()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("get_tree answer for `{canonical_key}` lacks non-empty `root`"))?;
    if declared_root != canonical_key {
        bail!(
            "get_tree answer for `{canonical_key}` declares mismatched root `{declared_root}`"
        );
    }
    let list = tree_reply["cards"]
        .as_array()
        .cloned()
        .context("get_tree answer lacks `cards` array")?;
    let live_root = validate_tree_nodes(&list, "tree")?;
    if live_root != canonical_key {
        bail!("get_tree cards root `{live_root}` does not match requested tree `{canonical_key}`");
    }
    Ok((rev_before, canonical_key, list))
}

pub fn run_live(
    zt: &mut Zt,
    key: &str,
    snapshot: Option<&str>,
    json_out: bool,
) -> Result<(Vec<String>, u8, usize)> {
    let tree_reply = zt.call("get_tree", json!({ "tree": key }))?;
    let (rev_before, canonical_key, list) = validate_tree_envelope(&tree_reply, key)?;
    let broken_reply = zt.call("list_broken_links", json!({}))?;
    let broken = broken_reply["broken"]
        .as_array()
        .cloned()
        .context("list_broken_links answer lacks `broken` array")?;
    for (idx, b) in broken.iter().enumerate() {
        let Some(obj) = b.as_object() else {
            bail!("list_broken_links broken[{idx}] must be a JSON object");
        };
        let has_src = obj
            .get("source")
            .or_else(|| obj.get("from"))
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty());
        if !has_src {
            bail!("list_broken_links broken[{idx}] lacks a string `source`");
        }
    }
    let after = zt.cards(&[], "text")?.archive_revision;
    if after != rev_before {
        bail!("incomplete checker inputs: changing read view");
    }
    let snap_cards = match snapshot {
        Some(path) => {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read snapshot {path}"))?;
            let raw = raw.strip_prefix('\u{FEFF}').unwrap_or(&raw);
            let parsed: Value =
                serde_json::from_str(raw).with_context(|| format!("{path} is not valid JSON"))?;
            let arr = parsed
                .as_array()
                .cloned()
                .with_context(|| format!("snapshot {path} must be a JSON array of cards"))?;
            let snap_root = validate_tree_nodes(&arr, path)?;
            if snap_root != canonical_key {
                bail!(
                    "snapshot {path} is rooted at `{snap_root}`, not requested tree `{canonical_key}`"
                );
            }
            Some(arr)
        }
        None => None,
    };
    let report = check_tree_data(&canonical_key, &list, &broken, snap_cards.as_deref());
    let fault_count = report.faults.len();
    let exit = if report.ok { 0 } else { 1 };
    let lines = if json_out {
        vec![serde_json::to_string_pretty(&report.to_json(&canonical_key))?]
    } else {
        format_text(&report)
    };
    Ok((lines, exit, fault_count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn card(address: &str, parent: Option<&str>, edge: &str, typ: &str, title: &str) -> Value {
        json!({ "address": address, "parent": parent, "edge": edge, "type": typ, "title": title })
    }

    fn unit(address: &str, parent: &str, n: usize) -> Vec<Value> {
        let mut out = vec![json!({
            "address": address,
            "parent": parent,
            "edge": "side",
            "type": "model",
            "facet": "规格",
            "title": "规格：基准"
        })];
        for i in 0..n {
            let ch = (b'a' + i as u8) as char;
            let a = format!("{address}|{ch}");
            out.push(card(&a, Some(address), "side", "idea", &a));
        }
        out
    }

    #[test]
    fn unit_changes_compares_multiset_of_sizes_per_type_and_title() {
        let mut before = vec![
            card("K", None, "root", "idea", "K"),
            card("K/1", Some("K"), "direct", "idea", "K/1"),
            card("K/2", Some("K/1"), "direct", "idea", "K/2"),
        ];
        before.extend(unit("K/1|a", "K/1", 3));
        before.extend(unit("K/2|a", "K/2", 1));

        let mut reordered = vec![
            card("K", None, "root", "idea", "K"),
            card("K/1", Some("K"), "direct", "idea", "K/1"),
            card("K/2", Some("K/1"), "direct", "idea", "K/2"),
        ];
        reordered.extend(unit("K/1|a", "K/1", 1));
        reordered.extend(unit("K/2|a", "K/2", 3));
        assert!(unit_changes(&before, &reordered).is_empty());

        let mut shrunk = vec![
            card("K", None, "root", "idea", "K"),
            card("K/1", Some("K"), "direct", "idea", "K/1"),
            card("K/2", Some("K/1"), "direct", "idea", "K/2"),
        ];
        shrunk.extend(unit("K/1|a", "K/1", 2));
        shrunk.extend(unit("K/2|a", "K/2", 1));
        assert_eq!(unit_changes(&before, &shrunk).len(), 1);
    }

    #[test]
    fn live_term_role_adjacent_lists_strict_broken_source_and_cyclic_snapshot_guards() {
        // Real core shape: Term list has term_role:"list" + term_list:"K/2"; adjacent Cited list is K/3;
        // an unfiled Term card K/1|a has term_role:"card" and term_list:null -> must NOT be flagged as a list!
        let clean_list = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", Some("K"), "direct", "idea", "Chapter 1"),
            json!({ "address": "K/1|a", "parent": "K/1", "edge": "side", "type": "term", "term_role": "card", "term_list": null, "title": "术语" }),
            json!({ "address": "K/2", "parent": "K/1", "edge": "direct", "type": "term", "term_role": "list", "term_list": "K/2", "title": "术语表" }),
            json!({ "address": "K/3", "parent": "K/2", "edge": "direct", "type": "cited", "title": "参考文献" }),
            json!({ "address": "K/3|a", "parent": "K/3", "edge": "side", "type": "cited", "title": "引用卡" }),
        ];
        // Broken link from another book pointing to K/99 must NOT be attributed to K;
        // broken link from root K pointing to X/9 MUST be attributed to K!
        let broken = vec![
            json!({ "source": "Other/1", "target": "K/99", "reason": "missing" }),
            json!({ "source": "K", "target": "X/9", "reason": "missing" }),
        ];
        let r = check_tree_data("K", &clean_list, &broken, None);
        assert_eq!(r.faults.len(), 1, "{:?}", r.faults);
        assert!(r.faults[0].contains("X/9"), "{:?}", r.faults);

        // Misplaced Term list (with term_role:"list" and term_list:"K/1|b") MUST be caught!
        let misplaced = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", Some("K"), "direct", "idea", "Chapter 1"),
            json!({ "address": "K/1|b", "parent": "K/1", "edge": "side", "type": "term", "term_role": "list", "term_list": "K/1|b", "title": "术语表" }),
        ];
        let rm = check_tree_data("K", &misplaced, &[], None);
        assert!(!rm.ok);
        assert!(rm.faults.iter().any(|f| f.contains("K/1|b")), "{:?}", rm.faults);

        // Cyclic parent snapshot must fail validation cleanly instead of recursing forever.
        let cyclic = vec![
            card("K/1", Some("K/2"), "side", "model", "A"),
            card("K/2", Some("K/1"), "side", "model", "B"),
        ];
        assert!(validate_tree_nodes(&cyclic, "snap").is_err());

        // P1-5: dangling parent pointer must fail validation cleanly.
        let dangling = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", Some("K/missing"), "direct", "idea", "Orphan"),
        ];
        assert!(validate_tree_nodes(&dangling, "snap").is_err());

        // P1-5: unit_changes reports vanished and newly added whole-unit keys even when total card count matches.
        let snap_before = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", Some("K"), "direct", "model", "Model A"),
        ];
        let snap_after = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", Some("K"), "direct", "model", "Model B"),
        ];
        let diff = unit_changes(&snap_before, &snap_after);
        assert_eq!(diff.len(), 2, "must report both vanished Model A and added Model B: {diff:?}");

        // P1-5: Cited list before Term list on top line must be flagged.
        let wrong_order = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", Some("K"), "direct", "idea", "Chapter 1"),
            json!({ "address": "K/2", "parent": "K/1", "edge": "direct", "type": "cited", "title": "参考文献" }),
            json!({ "address": "K/3", "parent": "K/2", "edge": "direct", "type": "term", "term_role": "list", "term_list": "K/3", "title": "术语表" }),
        ];
        let r_order = check_tree_data("K", &wrong_order, &[], None);
        assert!(!r_order.ok);
        assert!(r_order.faults.iter().any(|f| f.contains("Cited") && f.contains("Term")), "{:?}", r_order.faults);

        // M8: empty list, multiple roots, and non-root missing parent must all fail validation.
        assert!(validate_tree_nodes(&[], "snap").is_err());
        let multi_root = vec![
            card("K", None, "root", "idea", "Book 1"),
            card("K2", None, "root", "idea", "Book 2"),
        ];
        assert!(validate_tree_nodes(&multi_root, "snap").is_err());
        let orphan_non_root = vec![
            card("K", None, "root", "idea", "Book"),
            card("K/1", None, "direct", "idea", "No parent"),
        ];
        assert!(validate_tree_nodes(&orphan_non_root, "snap").is_err());

        // M8: live get_tree envelope requires zone == "literature", complete == true, matching root,
        // and root agreement with cards; snapshot root is returned for book-identity verification.
        let valid_env = json!({
            "archive_revision": 1,
            "tree": "K",
            "root": "K",
            "zone": "literature",
            "complete": true,
            "cards": [card("K", None, "root", "idea", "Book"), card("K/1", Some("K"), "direct", "idea", "Ch1")]
        });
        assert!(validate_tree_envelope(&valid_env, "K").is_ok());
        assert!(validate_tree_envelope(&valid_env, "K/1").is_ok());
        assert!(validate_tree_envelope(&valid_env, "OtherBook").is_err());
        let mut no_zone = valid_env.clone();
        no_zone.as_object_mut().unwrap().remove("zone");
        assert!(validate_tree_envelope(&no_zone, "K").is_err());
        let mut incomplete = valid_env.clone();
        incomplete["complete"] = json!(false);
        assert!(validate_tree_envelope(&incomplete, "K").is_err());
        let mut wrong_root = valid_env.clone();
        wrong_root["cards"] = json!([card("WrongBook", None, "root", "idea", "Book")]);
        assert!(validate_tree_envelope(&wrong_root, "K").is_err());
        assert_eq!(
            validate_tree_nodes(&[card("WrongBook", None, "root", "idea", "Other")], "snap").unwrap(),
            "WrongBook"
        );
    }
}
