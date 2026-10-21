//! `zt-digest organize-apply` (organize-apply.ps1): carries out the Topic merges and Card moves
//! that Opus decided in the Organize stage (<Work>\decisions.json) and logs them in applied.json.
//!
//! A merge moves the first Card of the dropped Topic (`<n>/1`, with every Card after it) to the
//! next free Side successor slot of `under` (a Card of the kept Topic; default `<keep>/1`), then
//! deletes the emptied Topic root. A move puts one Card with its successors under `under` the
//! same way. Each write is one call through the helper with -Moves <Work>\moves.jsonl; every
//! later address is translated through the `moved` maps zt returns, so a decision written before
//! the first move still names the right Card. It refuses a decision that names a Literature tree
//! Card, a Model Card or part, a Hub Card, or a Topic that is not the agent's, and a drop root
//! that still holds Cards after its move. `notes` are not applied here (the workflow hands them
//! to a card cycle, `zt-digest card -Notes`); their addresses follow the moves, and decisions.json
//! is written again. Prints {"merges","moves","refused","renamed"}.

use super::common::{self, Lazy, prop, pstr};
use crate::args::Args;
use crate::calls;
use crate::ps::{self, CiMap, eq, field};
use crate::util;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::sync::LazyLock;

static ROOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d+/0$").unwrap());
static ROOT_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/0$").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d+$").unwrap());
static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z]+$").unwrap());
static ERROR_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+ error ").unwrap());

/// `$text.Substring($start)` in UTF-16 units.
fn substring16(text: &str, start: usize) -> String {
    let mut used = 0;
    let mut out = String::new();
    for ch in text.chars() {
        if used >= start {
            out.push(ch);
        }
        used += ch.len_utf16();
    }
    out
}

/// `Get-Now`: the address today, through every rename so far, longest old address first (a
/// moved subtree).
fn get_now(map: &CiMap<String>, address: &str) -> String {
    let mut olds = map.keys();
    ps::ps_sort(&mut olds, |a, b| (-(ps::len16(a) as i64)).cmp(&-(ps::len16(b) as i64)));
    for old in &olds {
        let now = map.get(old).cloned().unwrap_or_default();
        if eq(address, old) {
            return now;
        }
        if ps::starts_with(address, &format!("{old}|")) || ps::starts_with(address, &format!("{old}/")) {
            return now + &substring16(address, ps::len16(old));
        }
    }
    address.to_string()
}

/// The next free Side successor slot of `under`, from the addresses of its tree.
fn free_side(under: &str, addresses: &[String]) -> String {
    let mut max = 0i64;
    for c in addresses {
        if !ps::starts_with(c, &format!("{under}|")) {
            continue;
        }
        let rest = substring16(c, ps::len16(under) + 1);
        let label = rest.split('|').next().unwrap_or_default();
        if !LABEL.is_match(label) {
            continue;
        }
        let mut n = 0i64;
        for ch in label.encode_utf16() {
            n = n * 26 + (ch as i64 - 96);
        }
        max = max.max(n);
    }
    let mut n = max + 1;
    let mut label = String::new();
    while n > 0 {
        n -= 1;
        label.insert(0, char::from_u32(97 + (n % 26) as u32).unwrap_or('a'));
        n /= 26;
    }
    format!("{under}|{label}")
}

/// `Test-Movable`: why a Card may not move, or None.
fn test_movable(card: Option<&Value>, what: &str) -> Option<String> {
    let Some(card) = card else { return Some(format!("{what} does not exist")) };
    if !eq(&pstr(card, "workspace"), "agent") {
        return Some(format!("{what} is not in the agent Workspace"));
    }
    let kind = pstr(card, "type");
    if ps::is_in(&kind, &["model", "hub"]) {
        return Some(format!("{what} is a {kind} Card"));
    }
    if eq(&pstr(card, "kind"), "literature") || (ps::truthy(prop(card, "tree")) && !DIGITS.is_match(&pstr(card, "tree"))) {
        return Some(format!("{what} is in a Literature tree"));
    }
    None
}

/// What one decision came to: done (its log entry) or refused (the reason).
enum Outcome {
    Done(Value),
    Refused(String),
}

struct Organizer {
    work: String,
    dry_run: bool,
    /// Old address → address now ([ordered]@{}: keys ignore case).
    map: CiMap<String>,
    zt: Lazy,
}

impl Organizer {
    /// `Get-Card`: the Card, or None when it cannot be read.
    fn get_card(&mut self, at: &str) -> Option<Value> {
        let zt = self.zt.get().ok()?;
        match common::read_first(zt, "get_card", json!({ "at": at })) {
            Ok(Value::Null) | Err(_) => None,
            Ok(card) => Some(card),
        }
    }

    fn tree_addresses(&mut self, tree: &str) -> Result<Vec<Value>> {
        let t = common::read_first(self.zt.get()?, "get_tree", json!({ "tree": tree }))?;
        Ok(ps::each(prop(&t, "cards")).into_iter().cloned().collect())
    }

    /// `Invoke-Calls`: the calls through the helper with -Moves; every rename goes into the map.
    fn invoke_calls(&mut self, calls: &[Value]) -> Result<()> {
        let moves_file = ps::join(&self.work, "moves.jsonl");
        if ps::exists(&moves_file) {
            std::fs::remove_file(&moves_file)?;
        }
        let mut options = calls::Options::new();
        options.moves = moves_file.clone();
        let (lines, ok) = match common::helper(self.zt.get()?, calls, &mut options) {
            Ok(result) => result,
            Err(error) => bail!("zt: {error:#}"),
        };
        let first_error = lines.iter().find(|l| ERROR_LINE.is_match(l));
        if !ok || first_error.is_some() {
            bail!("zt: {}", first_error.or(lines.first()).cloned().unwrap_or_default());
        }
        if ps::exists(&moves_file) {
            for line in ps::read_lines(&moves_file)? {
                let entry = ps::parse_json(&line)?;
                let Some(moved) = entry["moved"].as_object() else { continue };
                // A later rename of an address the map already holds updates that entry too.
                for old in self.map.keys() {
                    let now = self.map.get(&old).cloned().unwrap_or_default();
                    if let Some(to) = moved.get(&now) {
                        self.map.insert(&old, ps::s(to));
                    }
                }
                for (k, v) in moved {
                    self.map.insert(k, ps::s(v));
                }
            }
        }
        Ok(())
    }

    /// `Move-Under`: `at` (with its successors) to the next free Side successor of `under`.
    fn move_under(&mut self, at: &str, under: &str) -> Result<String> {
        let tree = under.split('/').next().unwrap_or_default().to_string();
        let addresses: Vec<String> = self.tree_addresses(&tree)?.iter().map(|c| pstr(c, "address")).collect();
        let to = free_side(under, &addresses);
        if self.dry_run {
            return Ok(to);
        }
        self.invoke_calls(&[json!({ "tool": "move_card", "arguments": { "at": at, "to": to, "confirm": "move" } })])?;
        if !self.map.contains(at) {
            self.map.insert(at, to.clone());
        }
        Ok(to)
    }

    fn merge(&mut self, m: &Value) -> Outcome {
        let drop = get_now(&self.map, &field(m, "drop"));
        let keep = get_now(&self.map, &field(m, "keep"));
        let mut why: Option<String> = None;
        if !ROOT.is_match(&drop) || !ROOT.is_match(&keep) {
            why = Some("drop and keep must be Topic roots (<n>/0)".into());
        } else if eq(&drop, &keep) {
            why = Some("drop and keep are the same Topic".into());
        }
        if why.is_none() {
            let drop_root = self.get_card(&drop);
            let keep_root = self.get_card(&keep);
            let agent = |c: &Option<Value>| c.as_ref().is_some_and(|c| eq(&pstr(c, "workspace"), "agent"));
            if !agent(&drop_root) || !agent(&keep_root) {
                why = Some("both Topics must exist in the agent Workspace".into());
            }
        }
        let under = if ps::truthy(&m["under"]) { get_now(&self.map, &field(m, "under")) } else { ROOT_END.replace(&keep, "/1").into_owned() };
        if why.is_none() && !eq(under.split('/').next().unwrap_or_default(), keep.split('/').next().unwrap_or_default()) {
            why = Some(format!("under `{under}` is not in the kept Topic"));
        }
        if why.is_none() && self.get_card(&under).is_none() {
            why = Some(format!("under `{under}` does not exist"));
        }
        if let Some(why) = why {
            return Outcome::Refused(why);
        }
        let first = ROOT_END.replace(&drop, "/1").into_owned();
        let result = (|| -> Result<Outcome> {
            let to = if self.get_card(&first).is_some() { Value::String(self.move_under(&first, &under)?) } else { Value::Null };
            let tree = drop.split('/').next().unwrap_or_default().to_string();
            let left = self.tree_addresses(&tree)?.iter().filter(|c| !eq(&pstr(c, "address"), &drop)).count();
            if left > 0 && !self.dry_run {
                return Ok(Outcome::Refused(format!("the dropped Topic still holds {left} Cards; its root stays")));
            }
            if !self.dry_run {
                self.invoke_calls(&[json!({ "tool": "delete_card", "arguments": { "at": drop, "into": keep, "confirm": drop } })])?;
            }
            Ok(Outcome::Done(json!({ "decision": "merge", "drop": drop, "keep": keep, "moved_to": to, "reason": field(m, "reason") })))
        })();
        result.unwrap_or_else(|error| Outcome::Refused(format!("{error:#}")))
    }

    fn movement(&mut self, m: &Value) -> Outcome {
        let at = get_now(&self.map, &field(m, "at"));
        let under = get_now(&self.map, &field(m, "under"));
        let card = self.get_card(&at);
        let mut why = test_movable(card.as_ref(), &format!("`{at}`"));
        if why.is_none() && ROOT.is_match(&at) {
            why = Some("a Topic root moves only by a merge".into());
        }
        if why.is_none() && self.get_card(&under).is_none() {
            why = Some(format!("under `{under}` does not exist"));
        }
        if why.is_none() && (eq(&under, &at) || ps::starts_with(&under, &format!("{at}|")) || ps::starts_with(&under, &format!("{at}/"))) {
            why = Some("a Card cannot move under itself".into());
        }
        if let Some(why) = why {
            return Outcome::Refused(why);
        }
        match self.move_under(&at, &under) {
            Ok(to) => Outcome::Done(json!({ "decision": "move", "at": at, "to": to, "reason": field(m, "reason") })),
            Err(error) => Outcome::Refused(format!("{error:#}")),
        }
    }
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Work"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["DryRun"])?;
    let work = common::mandatory(&a, "Work")?;
    let dry_run = a.flag("DryRun");
    let decisions_file = ps::join(&work, "decisions.json");
    let mut decisions = common::read_json(&decisions_file)?;
    let mut o = Organizer { work: work.clone(), dry_run, map: CiMap::new(), zt: Lazy::new() };
    let mut done: Vec<Value> = Vec::new();
    let mut refused: Vec<Value> = Vec::new();

    for m in ps::pipe(&decisions["merges"]).into_iter().cloned().collect::<Vec<_>>() {
        match o.merge(&m) {
            Outcome::Done(entry) => done.push(entry),
            Outcome::Refused(reason) => refused.push(json!({ "decision": "merge", "drop": field(&m, "drop"), "keep": field(&m, "keep"), "reason": reason })),
        }
    }
    for m in ps::pipe(&decisions["moves"]).into_iter().cloned().collect::<Vec<_>>() {
        match o.movement(&m) {
            Outcome::Done(entry) => done.push(entry),
            Outcome::Refused(reason) => refused.push(json!({ "decision": "move", "at": field(&m, "at"), "reason": reason })),
        }
    }
    // The notes name Cards by the addresses Opus read: translate them, so the card cycle finds them.
    let mut translated = 0;
    if !dry_run && !o.map.is_empty() && ps::ps_count(&decisions["notes"]) > 0 {
        let mut translate = |n: &mut Value| {
            let old = field(n, "address");
            let now = get_now(&o.map, &old);
            if !eq(&now, &old) {
                if let Some(map) = n.as_object_mut() {
                    map.insert("address".into(), Value::String(now));
                }
                translated += 1;
            }
        };
        match decisions.get_mut("notes") {
            Some(Value::Array(list)) => list.iter_mut().for_each(&mut translate),
            Some(one) => translate(one),
            None => {}
        }
        util::write_text(&decisions_file, &ps::json_pretty(&decisions))?;
    }
    let mut moved = Map::new();
    for (k, v) in o.map.iter() {
        moved.insert(k.clone(), Value::String(v.clone()));
    }
    // The staging manifests that name owned Cards (owned-gNN.txt, src-gNN.txt) follow the moves, so a later card
    // cycle reads every owned Card by its present address. On grossman2002integration trial 11 a moved Topic Card kept
    // its old address there and the notes cycle stopped at "incomplete Card read". The ledger's recorded outputs keep
    // the addresses their completions were checked at.
    let manifests = if !dry_run && !o.map.is_empty() {
        let mapping: std::collections::BTreeMap<String, String> = o.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        match std::path::Path::new(&work).parent() {
            Some(staging) => crate::ledger::relocate_manifests(staging, &mapping)?,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let count = |what: &str| done.iter().filter(|d| eq(&field(d, "decision"), what)).count();
    let summary = json!({ "merges": count("merge"), "moves": count("move"), "refused": refused.len(), "renamed": o.map.len() });
    let result = json!({ "dry_run": dry_run, "done": done, "refused": refused, "moved": moved, "notes_translated": translated, "manifests": manifests });
    util::write_text(ps::join(&work, "applied.json"), &ps::json_pretty(&result))?;
    util::print_line(&ps::json_compact(&summary))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_follow_the_renames_longest_first() {
        let mut map = CiMap::new();
        map.insert("15/1", "12/1|a".to_string());
        map.insert("15/1|b", "12/3|a".to_string());
        assert_eq!(get_now(&map, "15/1|b|a"), "12/3|a|a");
        assert_eq!(get_now(&map, "15/1/2"), "12/1|a/2");
        assert_eq!(get_now(&map, "15/1"), "12/1|a");
        assert_eq!(get_now(&map, "15/10"), "15/10");
        assert_eq!(get_now(&map, ""), "");
    }

    #[test]
    fn the_next_side_slot_counts_letters_as_digits() {
        let tree = |list: &[&str]| list.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(free_side("12/1", &tree(&["12/0", "12/1", "12/2|a"])), "12/1|a");
        assert_eq!(free_side("12/1", &tree(&["12/1|a", "12/1|b|a", "12/1|c/1", "12/10|z"])), "12/1|c");
        assert_eq!(free_side("12/1", &tree(&["12/1|z"])), "12/1|aa");
        assert_eq!(free_side("12/1", &tree(&["12/1|az"])), "12/1|ba");
        assert_eq!(free_side("12/1", &tree(&["12/1|1"])), "12/1|a");
    }

    #[test]
    fn movable_cards() {
        assert_eq!(test_movable(None, "`x`").unwrap(), "`x` does not exist");
        let card = |w: &str, t: &str, k: &str, tree: &str| json!({ "workspace": w, "type": t, "kind": k, "tree": tree });
        assert_eq!(test_movable(Some(&card("human", "idea", "regular", "3")), "`x`").unwrap(), "`x` is not in the agent Workspace");
        assert_eq!(test_movable(Some(&card("agent", "model", "regular", "3")), "`x`").unwrap(), "`x` is a model Card");
        assert_eq!(test_movable(Some(&card("agent", "idea", "regular", "Key2024")), "`x`").unwrap(), "`x` is in a Literature tree");
        assert!(test_movable(Some(&card("agent", "idea", "regular", "3")), "`x`").is_none());
    }
}
