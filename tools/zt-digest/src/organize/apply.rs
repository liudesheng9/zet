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
//! is written again. Prints {"merges","moves","refused","blocked","renamed","complete"}.
//!
//! Tree Lock (#139, S16): the stage reads a Locked tree like any other. Before a decision writes,
//! it asks the archive for the plan of its own move (`move_card` without `confirm`), which runs the
//! real Lock, pin, destination and Link-rewrite checks and answers their typed `card_locked`
//! refusal; a merge also checks the emptied root it would delete and every Link the `into`
//! re-pointing would rewrite. A blocked decision keeps its Cards in place, is reported with the
//! archive's own refusal, and no effect of it is dropped to make the run succeed: a decision whose
//! addresses belong to a blocked one is reported as its dependent. A Lock that appears after the
//! plan stops the affected write (the archive's refusal, `applied.json` written, exit 2) and keeps
//! the receipts already committed. `-DryRun` previews every mapping, into re-pointing and Side
//! compaction without writing.

use super::common::{Lazy, prop, pstr};
use crate::args::Args;
use crate::calls;
use crate::lockcheck;
use crate::ps::{self, CiMap, eq, field};
use crate::util;
use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

static ROOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d+/0$").unwrap());
static ROOT_END: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/0$").unwrap());
static DIGITS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^\d+$").unwrap());
static LABEL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^[a-z]+$").unwrap());

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

/// What one checked effect of a decision came to.
enum Check {
    /// The effect may run, with its previewed structural facts.
    Ok(Value),
    /// An active Lock keeps it in place: the archive's own refusal, or the plan's own facts.
    Blocked(Value),
    /// The decision breaks an ordinary rule (a Model Card, a Literature Card, a missing Card).
    Refused(String),
}

/// One plan item the run left in place, with the addresses it owns: a later decision that names
/// one of them waits for this one instead of being checked on its own facts.
struct Withheld {
    decision: String,
    address: String,
    /// The tree a blocked merge owns (its dropped Topic), or "" for a move.
    tree: String,
    /// The addresses a blocked move would have renamed (`mapping` keys and values).
    owns: Vec<String>,
    refusal: Value,
}

struct Organizer {
    work: String,
    dry_run: bool,
    /// Old address → address now ([ordered]@{}: keys ignore case).
    map: CiMap<String>,
    zt: Lazy,
    /// `get_card` answers, by exact address (an address is case significant once read).
    cards: HashMap<String, Option<Value>>,
    /// `get_tree` rows by tree id.
    trees: HashMap<String, Vec<Value>>,
    /// The Lock and pin facts every read answered, one read each.
    facts: HashMap<String, lockcheck::Facts>,
    done: Vec<Value>,
    refused: Vec<Value>,
    blocked: Vec<Value>,
    preview: Vec<Value>,
    withheld: Vec<Withheld>,
    stopped: Option<Value>,
}

impl Organizer {
    fn new(work: String, dry_run: bool) -> Self {
        Self {
            work,
            dry_run,
            map: CiMap::new(),
            zt: Lazy::new(),
            cards: HashMap::new(),
            trees: HashMap::new(),
            facts: HashMap::new(),
            done: Vec::new(),
            refused: Vec::new(),
            blocked: Vec::new(),
            preview: Vec::new(),
            withheld: Vec::new(),
            stopped: None,
        }
    }

    /// `Get-Card`: the Card, or None when it cannot be read.
    fn get_card(&mut self, at: &str) -> Option<Value> {
        if let Some(hit) = self.cards.get(at) {
            return hit.clone();
        }
        let value = match self.zt.get() {
            Ok(zt) => match super::common::read_first(zt, "get_card", json!({ "at": at })) {
                Ok(Value::Null) | Err(_) => None,
                Ok(card) => Some(card),
            },
            Err(_) => None,
        };
        self.cards.insert(at.to_string(), value.clone());
        value
    }

    /// The Cards of one tree, read once. Every row's Lock and pin facts are kept with the read, so
    /// a member or a pinned target is known without another call.
    fn tree_cards(&mut self, tree: &str) -> Vec<Value> {
        if let Some(hit) = self.trees.get(tree) {
            return hit.clone();
        }
        let cards: Vec<Value> = match self.zt.get() {
            Ok(zt) => match super::common::read_first(zt, "get_tree", json!({ "tree": tree })) {
                Ok(value) => ps::each(prop(&value, "cards")).into_iter().cloned().collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        for card in &cards {
            self.facts.insert(pstr(card, "address"), lockcheck::facts_in(card));
        }
        self.trees.insert(tree.to_string(), cards.clone());
        cards
    }

    /// One Card's Lock and pin facts.
    fn facts(&mut self, at: &str) -> lockcheck::Facts {
        if let Some(hit) = self.facts.get(at) {
            return hit.clone();
        }
        let facts = self.get_card(at).map(|card| lockcheck::facts_in(&card)).unwrap_or_default();
        self.facts.insert(at.to_string(), facts.clone());
        facts
    }

    /// The addresses of `at` and every Card below or beside it: a Card's successors are the Cards
    /// The addresses of the Cards that stay and link `at`: the Links an `into` re-pointing would
    /// rewrite.
    fn inbound(&mut self, at: &str) -> Vec<String> {        self.get_card(at).map(|card| lockcheck::strings(&card["inbound"])).unwrap_or_default()
    }

    /// The next free Side successor slot of `under`: this tool's own destination.
    fn destination(&mut self, under: &str) -> String {
        let tree = lockcheck::tree_id(under);
        let addresses: Vec<String> = self.tree_cards(&tree).iter().map(|card| pstr(card, "address")).collect();
        free_side(under, &addresses)
    }

    /// The complete mapping of moving `at` to `to`: `at` lands at `to` and each of its successors
    /// keeps its path from the moved root (a Direct successor is the next number or `|1`, a Side
    /// successor keeps its label), exactly as the archive's own move plan renames them.
    fn move_mapping(&mut self, at: &str, to: &str) -> Map<String, Value> {
        let tree = lockcheck::tree_id(at);
        let rows = self.tree_cards(&tree);
        let mut mapping = Map::new();
        mapping.insert(at.to_string(), Value::String(to.to_string()));
        let mut queue = vec![(at.to_string(), to.to_string())];
        while let Some((old_parent, new_parent)) = queue.pop() {
            for row in &rows {
                let address = pstr(row, "address");
                if !eq(&pstr(row, "parent"), &old_parent) || mapping.contains_key(&address) {
                    continue;
                }
                let new = if eq(&pstr(row, "edge"), "side") {
                    match address.strip_prefix(&format!("{old_parent}|")) {
                        Some(rest) => format!("{new_parent}|{}", rest.split('|').next().unwrap_or_default()),
                        None => continue,
                    }
                } else {
                    next_direct(&new_parent)
                };
                mapping.insert(address.clone(), Value::String(new.clone()));
                queue.push((address, new));
            }
        }
        mapping
    }

    /// One read-only plan call: the archive answers the plan or its typed `card_locked` refusal.
    /// Nothing is written by a plan.
    fn plan_call(&mut self, tool: &str, arguments: Value, what: &str) -> Result<Check> {
        let call = json!({ "tool": tool, "arguments": arguments });
        let (done, failure) = calls::run(self.zt.get()?, &[call], &mut calls::Options::new())?;
        if let Some(failure) = failure {
            match failure.refusal.clone().filter(is_lock) {
                Some(refusal) => return Ok(Check::Blocked(refusal)),
                None => return Ok(Check::Refused(format!("{what}: {}", failure.lines().first().cloned().unwrap_or_default()))),
            }
        }
        Ok(Check::Ok(done.first().and_then(|entry| entry.value.clone()).unwrap_or(Value::Null)))
    }

    /// Sends one write batch. `Ok(None)` when it committed; `Ok(Some(refusal))` when a Lock
    /// appeared after the plan and the archive refused it; `Err` for any other failure.
    fn write(&mut self, calls: Vec<Value>) -> Result<Option<Value>> {
        let moves_file = ps::join(&self.work, "moves.jsonl");
        if ps::exists(&moves_file) {
            std::fs::remove_file(&moves_file)?;
        }
        let mut options = calls::Options::new();
        options.moves = moves_file.clone();
        let (_, failure) = calls::run(self.zt.get()?, &calls, &mut options)?;
        if let Some(failure) = failure {
            match failure.refusal.clone().filter(is_lock) {
                Some(refusal) => return Ok(Some(refusal)),
                None => anyhow::bail!("zt: {}", failure.lines().first().cloned().unwrap_or_default()),
            }
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
        // The commit changed addresses and bodies: every read taken for the plan is stale.
        self.cards.clear();
        self.trees.clear();
        self.facts.clear();
        Ok(None)
    }

    fn refuse(&mut self, decision: &str, address: &str, reason: String) {
        self.refused.push(entry_of(decision, address, json!({ "reason": reason })));
        self.preview.push(entry_of(decision, address, json!({ "refused": reason })));
    }

    /// Records a decision kept in place: the archive's (or the plan's) own refusal, its complete
    /// effects, whether it is a dependent of another blocked decision, and the addresses it owns.
    fn block(&mut self, decision: &str, address: &str, refusal: Value, effect: Value, tree: &str, owns: Vec<String>) {
        let mut entry = entry_of(
            decision,
            address,
            json!({ "refusal": refusal, "dependent": false, "depends_on": Value::Null }),
        );
        for key in ["drop", "keep", "at", "to", "step"] {
            if let Some(value) = effect.get(key).filter(|value| !value.is_null()) {
                entry[key] = value.clone();
            }
        }
        self.withheld.push(Withheld {
            decision: decision.to_string(),
            address: address.to_string(),
            tree: tree.to_string(),
            owns,
            refusal,
        });
        self.blocked.push(entry);
    }

    /// The blocked decision a later decision waits for, when it names an address that decision
    /// owns. A move in a blocked merge's dropped Topic is a dependent, not a checked plan.
    fn dependent_of(&self, addresses: &[String]) -> Option<Value> {
        for withheld in &self.withheld {
            for address in addresses {
                let owns = if withheld.decision == "merge" {
                    !withheld.tree.is_empty() && eq(&lockcheck::tree_id(address), &withheld.tree)
                } else {
                    withheld.owns.iter().any(|owned| lockcheck::inside(address, owned))
                };
                if owns {
                    return Some(entry_of(
                        &withheld.decision,
                        &withheld.address,
                        json!({ "refusal": withheld.refusal }),
                    ));
                }
            }
        }
        None
    }

    fn block_dependent(&mut self, decision: &str, address: &str, depends_on: Value) {
        let mut refusal = depends_on["refusal"].clone();
        if let Some(object) = refusal.as_object_mut() {
            object.insert("blocked_by".into(), json!("dependent"));
        }
        let entry = entry_of(
            decision,
            address,
            json!({ "refusal": refusal, "dependent": true, "depends_on": depends_on }),
        );
        self.blocked.push(entry);
    }

    /// The delete step of a merge: the emptied root, the Links `into` re-points, and the Side
    /// compaction (a Topic root keeps none).
    fn delete_preview(&mut self, at: &str, into: &str) -> Check {
        let facts = self.facts(at);
        let inbound = self.inbound(at);
        let effect = json!({
            "at": at, "into": into,
            "repointed_in": inbound,
            "side_compaction": [],
        });
        if let Some(lock) = facts.lock {
            return Check::Blocked(lockcheck::blocked_card("delete", at, &lock));
        }
        if !facts.pinned_by.is_empty() {
            return Check::Blocked(lockcheck::blocked_pin("pinned-delete", at, &facts.pinned_by, None));
        }
        for source in &inbound {
            let facts = self.facts(source);
            if let Some(lock) = facts.lock {
                return Check::Blocked(lockcheck::blocked_card("edit", source, &lock));
            }
            match self.get_card(source) {
                Some(card) if !eq(&pstr(&card, "workspace"), "agent") => {
                    return Check::Refused(format!(
                        "the Link from human Card `{source}` cannot be re-pointed to `{into}`"
                    ));
                }
                _ => {}
            }
        }
        Check::Ok(effect)
    }

    fn merge(&mut self, m: &Value) {
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
        if why.is_none() && !eq(&lockcheck::tree_id(&under), &lockcheck::tree_id(&keep)) {
            why = Some(format!("under `{under}` is not in the kept Topic"));
        }
        if why.is_none() && self.get_card(&under).is_none() {
            why = Some(format!("under `{under}` does not exist"));
        }
        if let Some(why) = why {
            self.refuse("merge", &drop, why);
            return;
        }
        let first = ROOT_END.replace(&drop, "/1").into_owned();
        if let Some(depends_on) = self.dependent_of(&[drop.clone(), keep.clone()]) {
            self.block_dependent("merge", &drop, depends_on);
            return;
        }
        let tree = lockcheck::tree_id(&drop);
        let mut effect = json!({ "decision": "merge", "drop": drop, "keep": keep, "under": under });
        // 1. The move of the dropped Topic's first Card with its successors. The archive's own plan
        //    runs the Lock, pin, destination and Link-rewrite checks; a refusal blocks the whole
        //    merge before any write.
        let mut move_target = Value::Null;
        if self.get_card(&first).is_some() {
            let to = self.destination(&under);
            let mapping = self.move_mapping(&first, &to);
            match self.plan_call("move_card", json!({ "at": first, "to": to }), "the move") {
                Ok(Check::Blocked(refusal)) => {
                    effect["step"] = json!("move");
                    effect["move"] = json!({ "at": first, "under": under, "to": to, "mapping": mapping });
                    self.block("merge", &drop, refusal, effect, &tree, vec![drop.clone(), to]);
                    return;
                }
                Ok(Check::Refused(reason)) => {
                    self.refuse("merge", &drop, reason);
                    return;
                }
                Ok(Check::Ok(plan)) => {
                    effect["move"] = json!({
                        "at": first, "under": under, "to": to, "mapping": mapping,
                        "lines": plan["lines"].clone(),
                    });
                    move_target = Value::String(to);
                }
                Err(error) => {
                    self.refuse("merge", &drop, format!("{error:#}"));
                    return;
                }
            }
        }
        // 2. The delete of the emptied root: its own Lock and pins, and every Link `into` would
        //    re-point. Nothing is written when either is refused.
        let delete = match self.delete_preview(&drop, &keep) {
            Check::Blocked(refusal) => {
                effect["step"] = json!("delete");
                effect["delete"] = json!({ "at": drop, "into": keep, "repointed_in": [], "side_compaction": [] });
                self.block("merge", &drop, refusal, effect, &tree, vec![drop.clone(), keep.clone()]);
                return;
            }
            Check::Refused(reason) => {
                self.refuse("merge", &drop, reason);
                return;
            }
            Check::Ok(effect_delete) => effect_delete,
        };
        effect["delete"] = delete.clone();
        self.preview.push(effect.clone());
        if self.dry_run {
            self.done.push(json!({ "decision": "merge", "drop": drop, "keep": keep, "moved_to": move_target, "reason": field(m, "reason") }));
            return;
        }
        // 3. Apply in order: the move, then the emptied root.
        if let Some(to) = move_target.as_str() {
            match self.write(vec![json!({ "tool": "move_card", "arguments": { "at": first, "to": to, "confirm": "move" } })]) {
                Ok(None) => {}
                Ok(Some(refusal)) => {
                    self.stop("merge", &drop, "move", refusal);
                    return;
                }
                Err(error) => {
                    self.refuse("merge", &drop, format!("{error:#}"));
                    return;
                }
            }
            if !self.map.contains(&first) {
                self.map.insert(&first, to.to_string());
            }
        }
        let left = self.tree_cards(&tree).iter().filter(|c| !eq(&pstr(c, "address"), &drop)).count();
        if left > 0 {
            self.refuse("merge", &drop, format!("the dropped Topic still holds {left} Cards; its root stays"));
            return;
        }
        match self.write(vec![json!({ "tool": "delete_card", "arguments": { "at": drop, "into": keep, "confirm": drop } })]) {
            Ok(None) => {}
            Ok(Some(refusal)) => {
                self.stop("merge", &drop, "delete", refusal);
                return;
            }
            Err(error) => {
                self.refuse("merge", &drop, format!("{error:#}"));
                return;
            }
        }
        self.done.push(json!({ "decision": "merge", "drop": drop, "keep": keep, "moved_to": move_target, "reason": field(m, "reason") }));
    }

    fn movement(&mut self, m: &Value) {
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
            self.refuse("move", &at, why);
            return;
        }
        if let Some(depends_on) = self.dependent_of(&[at.clone()]) {
            self.block_dependent("move", &at, depends_on);
            return;
        }
        let to = self.destination(&under);
        let mapping = self.move_mapping(&at, &to);
        let mut effect = json!({
            "decision": "move", "at": at, "under": under, "to": to,
            "mapping": mapping, "side_compaction": [],
        });
        match self.plan_call("move_card", json!({ "at": at, "to": to }), "the move") {
            Ok(Check::Blocked(refusal)) => {
                effect["step"] = json!("move");
                self.block("move", &at, refusal.clone(), effect, "", vec![at.clone(), to.clone()]);
                return;
            }
            Ok(Check::Refused(reason)) => {
                self.refuse("move", &at, reason);
                return;
            }
            Ok(Check::Ok(plan)) => effect["lines"] = plan["lines"].clone(),
            Err(error) => {
                self.refuse("move", &at, format!("{error:#}"));
                return;
            }
        }
        self.preview.push(effect);
        if self.dry_run {
            self.done.push(json!({ "decision": "move", "at": at, "to": to, "reason": field(m, "reason") }));
            return;
        }
        match self.write(vec![json!({ "tool": "move_card", "arguments": { "at": at, "to": to, "confirm": "move" } })]) {
            Ok(None) => {}
            Ok(Some(refusal)) => {
                self.stop("move", &at, "move", refusal);
                return;
            }
            Err(error) => {
                self.refuse("move", &at, format!("{error:#}"));
                return;
            }
        }
        if !self.map.contains(&at) {
            self.map.insert(&at, to.clone());
        }
        self.done.push(json!({ "decision": "move", "at": at, "to": to, "reason": field(m, "reason") }));
    }

    /// A Lock that appeared after the plan stops the affected write and every later decision: the
    /// archive's refusal, never a retry, and the receipts already committed stay.
    fn stop(&mut self, decision: &str, address: &str, step: &str, refusal: Value) {
        self.block(decision, address, refusal.clone(), json!({ "step": step }), "", Vec::new());
        if self.stopped.is_none() {
            self.stopped = Some(entry_of(
                decision,
                address,
                json!({ "step": step, "refusal": refusal }),
            ));
        }
    }
}

/// The next Direct successor address of one changed address: the next number, or `|1` after a
/// Side label (`A/1|a` → `A/1|a|1`), exactly as the archive renumbers it.
fn next_direct(address: &str) -> String {
    let last = address.rsplit_once('|').map(|(_, segment)| segment).or_else(|| address.rsplit_once('/').map(|(_, segment)| segment)).unwrap_or(address);
    match last.parse::<u64>() {
        Ok(number) => match address.rsplit_once('|') {
            Some((prefix, _)) => format!("{prefix}|{}", number + 1),
            None => match address.rsplit_once('/') {
                Some((topic, _)) => format!("{topic}/{}", number + 1),
                None => (number + 1).to_string(),
            },
        },
        Err(_) => format!("{address}|1"),
    }
}

/// The field that names a decision's own address.
fn address_key(decision: &str) -> &'static str {
    if decision == "merge" { "drop" } else { "at" }
}

/// One JSON object carrying a decision's name, its own address and `fields`.
fn entry_of(decision: &str, address: &str, fields: Value) -> Value {
    let mut object = Map::new();
    object.insert("decision".into(), json!(decision));
    object.insert(address_key(decision).into(), json!(address));
    if let Value::Object(fields) = fields {
        for (key, value) in fields {
            object.insert(key, value);
        }
    }
    Value::Object(object)
}

fn is_lock(refusal: &Value) -> bool {
    refusal["code"] == json!("card_locked") || refusal["error"] == json!("card_locked")
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let names = ["Work"];
    let a = Args::parse(&util::bind_positional(argv, &names)?, &names, &["DryRun"])?;
    let work = super::common::mandatory(&a, "Work")?;
    let dry_run = a.flag("DryRun");
    let decisions_file = ps::join(&work, "decisions.json");
    let mut decisions = super::common::read_json(&decisions_file)?;
    let mut o = Organizer::new(work.clone(), dry_run);

    for m in ps::pipe(&decisions["merges"]).into_iter().cloned().collect::<Vec<_>>() {
        if o.stopped.is_some() {
            break;
        }
        o.merge(&m);
    }
    for m in ps::pipe(&decisions["moves"]).into_iter().cloned().collect::<Vec<_>>() {
        if o.stopped.is_some() {
            break;
        }
        o.movement(&m);
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
        let mapping: BTreeMap<String, String> = o.map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        match std::path::Path::new(&work).parent() {
            Some(staging) => crate::ledger::relocate_manifests(staging, &mapping)?,
            None => Vec::new(),
        }
    } else {
        Vec::new()
    };
    let count = |what: &str| o.done.iter().filter(|d| eq(&field(d, "decision"), what)).count();
    let complete = o.stopped.is_none() && o.blocked.is_empty();
    let summary = json!({
        "merges": count("merge"), "moves": count("move"),
        "refused": o.refused.len(), "blocked": o.blocked.len(),
        "renamed": o.map.len(), "complete": complete,
    });
    let result = json!({
        "schema_version": 1,
        "dry_run": dry_run,
        "complete": complete,
        "done": o.done,
        "refused": o.refused,
        "blocked": o.blocked,
        "preview": o.preview,
        "stopped": o.stopped,
        "moved": moved,
        "notes_translated": translated,
        "manifests": manifests,
    });
    util::write_text(ps::join(&work, "applied.json"), &ps::json_pretty(&result))?;
    if let Some(stopped) = o.stopped.clone() {
        return lockcheck::stop("organize", &stopped["refusal"]);
    }
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

    #[test]
    fn a_merge_names_its_own_address_and_a_move_names_its_at() {
        assert_eq!(address_key("merge"), "drop");
        assert_eq!(address_key("move"), "at");
        assert!(is_lock(&json!({"code": "card_locked"})));
        assert!(is_lock(&json!({"error": "card_locked"})));
        assert!(!is_lock(&json!({"error": "read_limit"})));
        assert!(!is_lock(&Value::Null));
    }
}
