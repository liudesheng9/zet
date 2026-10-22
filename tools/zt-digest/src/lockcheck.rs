//! The digest's Lock precheck (Tree Lock §15.2, R327-R334; #137).
//!
//! A digest mode is either read-only for the archive or it writes one book's Literature tree. A
//! read-only audit, checker, model report or export reads a Locked tree like any other Card and
//! must not ask about Locks at all (R327). A mode that rewrites the tree (`cards`, `models`, math,
//! events, findings, figures, skeleton, metadata) or that allocates its jobs must know first
//! whether that tree is Locked: it stops before the first write and before any job file or job
//! dispatch, with one exact message (R328/R329/R333/R334):
//!
//! ```text
//! the Literature tree <key> is locked by the owner
//! ```
//!
//! The check is one read through `zt mcp` (`get_cards` of the tree root, whose projection carries
//! the Lock facts). It writes nothing, so it never changes a Lock, a revision or an access rule.
//! It is advisory: the storage gate stays authoritative, so a tree that becomes Locked during the
//! run still refuses the write it would have taken (§15.3), and the run keeps the staged drafts and
//! the receipts it already committed.
//!
//! The check belongs to the *destination* only. A Locked book used as a source, or an unrelated
//! Locked tree in the same archive, never stops a mode: those Cards stay readable and linkable.

//! A planner that reads the Lock facts itself (the Organize and Hub stages, S16/#139) answers in
//! the same vocabulary for the decision it keeps in place: `card_locked`, the blocking Locked
//! tree's root, the affected Cards, one reason (`edit`, `move`, `delete`, `create-inside`,
//! `move-inside`, `pinned-move`, `pinned-delete`), the owner's hint and every blocking Lock. A
//! pin keeps a Card's address and existence frozen, never its authored text (R336), so a planner
//! reads `pinned_by` only to refuse a move or a delete.

use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::Result;
use serde_json::{Value, json};

/// The exit code of a writing mode that stopped because its destination Literature tree is Locked.
/// It is the refusal exit code of the digest tools (`-Token` refusals answer it too), never a crash.
pub const LOCKED_EXIT: i32 = 2;

/// The one owner-facing sentence (R334). The key is the Citation key of the book's Literature tree.
pub fn message(key: &str) -> String {
    format!("the Literature tree {key} is locked by the owner")
}

/// One active Lock of a tree, as the read projection states it: the Lock id, its tree's root
/// address and the moment it was taken.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Lock {
    pub lock_id: i64,
    pub tree: String,
    pub since: String,
}

/// The Lock facts of a read Card (`lock` = `{lock_id, tree, since}`), or None.
pub fn lock_fact(card: &Value) -> Option<Lock> {
    lock_in(&card["lock"])
}

/// The Lock fact of one projected `lock` object.
pub fn lock_in(lock: &Value) -> Option<Lock> {
    let lock = lock.as_object()?;
    Some(Lock {
        lock_id: lock.get("lock_id")?.as_i64()?,
        tree: lock.get("tree")?.as_str()?.to_string(),
        since: lock.get("since").and_then(Value::as_str).unwrap_or_default().to_string(),
    })
}

/// The typed refusal a writing mode answers when its destination tree is Locked (R333). `mode` is
/// the digest tool and `step` its step, so the caller reads which part of the run stopped. The
/// fields are the MCP refusal's own vocabulary (`code`, `tree`, `lock_id`): a caller recognizes a
/// Lock without matching English text, and never retries it as a provider fault.
pub fn refusal(mode: &str, step: &str, key: &str, lock: &Lock) -> Value {
    json!({
        "ok": false,
        "code": "card_locked",
        "error": message(key),
        "mode": mode,
        "step": step,
        "tree": lock.tree,
        "lock_id": lock.lock_id,
        "locked_at": lock.since,
        "hint": "unlock the tree in the GUI first; its Cards stay readable as sources",
    })
}

/// The book's Literature-tree key from a staging folder's `manifest.json`, or "" when the folder
/// holds no manifest (the step's own read of the manifest reports that) or no Citation key.
pub fn key_from_staging(staging: &str) -> String {
    if staging.trim().is_empty() {
        return String::new();
    }
    let path = ps::join(staging, "manifest.json");
    if !std::path::Path::new(&path).exists() {
        return String::new();
    }
    util::read_json(&path)
        .ok()
        .and_then(|manifest| manifest.get("citation_key").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default()
}

/// Reads one address's active Lock through the MCP read projection. A missing Card, an address
/// outside the archive or an unlocked Card all answer `None`: none of them can be a Locked tree.
pub fn lock_of(zt: &mut Zt, at: &str) -> Result<Option<Lock>> {
    let reply = zt.request("get_cards", json!({ "addresses": [at], "view": "text" }))?;
    if zt_check::mcp::reply_error(&reply).is_some() {
        // A read the archive refuses (a limit, a closed view) is not proof of a Lock.
        return Ok(None);
    }
    let text = zt_check::mcp::reply_text(&reply);
    let value: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    let card = value["cards"].as_array().and_then(|cards| cards.first());
    Ok(card.and_then(lock_fact))
}

/// The precheck itself: `Some(refusal)` when the destination Literature tree is Locked. An empty
/// key names no book (a step that has no destination to check) and always passes.
pub fn preflight(zt: &mut Zt, mode: &str, step: &str, key: &str) -> Result<Option<Value>> {
    if key.trim().is_empty() {
        return Ok(None);
    }
    Ok(lock_of(zt, key)?.map(|lock| refusal(mode, step, key, &lock)))
}

// ---------------------------------------------------------------------------------------------
// The Lock facts a planner reads (S16)
// ---------------------------------------------------------------------------------------------

/// One Card's active Lock and pin facts, exactly as every read projection states them: `lock` is
/// the active Lock of the Card's tree (null when the tree is not Locked) and `pinned_by` names the
/// roots of the active Locks whose Cards link this one.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Facts {
    pub lock: Option<Lock>,
    pub pinned_by: Vec<String>,
}

/// The Lock and pin facts of one read Card.
pub fn facts_in(card: &Value) -> Facts {
    Facts {
        lock: lock_fact(card),
        pinned_by: strings(&card["pinned_by"]),
    }
}

/// The string items of a JSON array (a read's `pinned_by`, `inbound` or `mapping` keys).
pub fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default()
}

/// `tree`'s id: `<n>` for a Topic tree, the Citation key for a Literature tree.
pub fn tree_id(address: &str) -> String {
    address.split('/').next().unwrap_or_default().to_string()
}

/// Whether `address` is `prefix`, lies below it (`prefix/1`, `prefix|a|1`) or beside it
/// (`prefix|a`): the subtree a decision owns.
pub fn inside(address: &str, prefix: &str) -> bool {
    !prefix.is_empty()
        && (address == prefix
            || address.starts_with(&format!("{prefix}/"))
            || address.starts_with(&format!("{prefix}|")))
}

/// The actionable sentence for a reason, exactly as the archive's own refusal gives it.
pub fn hint_for(reason: &str) -> &'static str {
    match reason {
        "pinned-move" | "pinned-delete" => {
            "write a new Card in another tree and link the locked Card, or unlock the tree in the GUI first"
        }
        _ => "unlock the tree in the GUI first",
    }
}

/// The one sentence of a blocked plan item. `cards` are the affected Cards, `trees` the blocking
/// Locked trees' roots; a pin names the pinned target, and the sources that pin it stay readable.
fn blocked_message(reason: &str, cards: &[String], trees: &[String]) -> String {
    let quoted = |values: &[String]| values.iter().map(|v| format!("`{v}`")).collect::<Vec<_>>().join(", ");
    let card = quoted(cards);
    let tree = quoted(trees);
    match reason {
        "move-inside" => format!("{card} is inside locked tree {tree}; a locked tree takes no moved-in Cards"),
        "create-inside" => format!("{card} is inside locked tree {tree}; a locked tree takes no new Cards"),
        "pinned-move" => format!("card {card} cannot be moved: it is a pinned target of locked tree {tree}"),
        "pinned-delete" => format!("card {card} cannot be deleted: it is a pinned target of locked tree {tree}"),
        "delete" => format!("card {card} cannot be deleted: it is in locked tree {tree}"),
        "edit" => format!("card {card} is in locked tree {tree}; locked Cards can be linked but not changed"),
        _ => format!("card {card} cannot be moved: it is in locked tree {tree}"),
    }
}

/// The typed refusal of one blocked plan item (S16-AC05), in the archive's own vocabulary: the
/// reason the item is kept in place, the affected Cards, the blocking Locked trees and their Lock
/// ids, and — for a pin — the locked source Cards whose Links pin the target. A planner reports
/// every item; it never drops one to make the plan succeed.
pub fn blocked(reason: &str, cards: &[String], trees: &[String], locks: &[Lock], sources: &[String]) -> Value {
    let mut cards = cards.to_vec();
    cards.sort();
    cards.dedup();
    let mut trees = trees.to_vec();
    trees.sort();
    trees.dedup();
    let mut value = json!({
        "error": "card_locked",
        "code": "card_locked",
        "message": blocked_message(reason, &cards, &trees),
        "tree": trees.first(),
        "cards": cards,
        "reason": reason,
        "hint": hint_for(reason),
    });
    if !sources.is_empty() {
        let mut sources = sources.to_vec();
        sources.sort();
        sources.dedup();
        value["sources"] = json!(sources);
    }
    if !locks.is_empty() {
        let mut blocks: Vec<Value> = locks
            .iter()
            .map(|lock| json!({"lock_id": lock.lock_id, "tree": lock.tree}))
            .collect();
        blocks.sort_by_key(|block| {
            (
                block["lock_id"].as_i64().unwrap_or(0),
                block["tree"].as_str().unwrap_or_default().to_string(),
            )
        });
        blocks.dedup();
        value["locks"] = Value::Array(blocks);
    }
    value
}

/// The blocked refusal of one Card whose Locked tree keeps it in place: a member of a Locked tree
/// (its authored text, address or existence) or a pinned target (its address or existence).
pub fn blocked_card(reason: &str, card: &str, lock: &Lock) -> Value {
    blocked(reason, &[card.to_string()], std::slice::from_ref(&lock.tree), std::slice::from_ref(lock), &[])
}

/// The blocked refusal of a destination inside a Locked tree: a Card none of the plan may create
/// or move into it.
pub fn blocked_destination(reason: &str, address: &str, lock: &Lock) -> Value {
    blocked(reason, &[address.to_string()], std::slice::from_ref(&lock.tree), std::slice::from_ref(lock), &[])
}

/// The blocked refusal of a pinned target: the Locked trees that pin it are `pinned_by` roots; the
/// Lock itself is read from the pinned Card's facts when it is available.
pub fn blocked_pin(reason: &str, card: &str, pinned_by: &[String], lock: Option<&Lock>) -> Value {
    let locks: Vec<Lock> = lock.cloned().into_iter().collect();
    blocked(reason, &[card.to_string()], pinned_by, &locks, &[])
}

/// The archive refused a write the plan had allowed: a Lock that appeared after the plan, kept as
/// its typed `card_locked` refusal instead of an English sentence. A stage that receives it stops
/// the affected write, keeps the receipts it already committed and answers the refusal's own JSON.
#[derive(Debug)]
pub struct Stopped(pub Value);

impl std::fmt::Display for Stopped {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let said = self.0["message"].as_str().or_else(|| self.0["error"].as_str()).unwrap_or_default();
        write!(f, "zt: {} {}", self.0["error"].as_str().unwrap_or("card_locked"), said)?;
        write!(f, " {}", util::compact(&self.0))
    }
}

impl std::error::Error for Stopped {}

/// `Some(refusal)` when an error is the archive's typed Lock refusal (or a planner's own blocked
/// refusal), so a stage recognizes it without matching English text (R338, S16-AC05).
pub fn stopped_refusal(error: &anyhow::Error) -> Option<Value> {
    error.downcast_ref::<Stopped>().map(|stopped| stopped.0.clone())
}

/// Reports a stopped step: the typed refusal as its last stdout line (so a caller reads the same
/// JSON it reads from every step), the exact sentence on stderr, and the refusal exit code. It
/// writes no file, allocates no job and changes no Card.
pub fn stop(mode: &str, refusal: &Value) -> Result<i32> {
    util::print_line(&util::compact(refusal))?;
    // The preflight refusal keeps the owner sentence in `error`; an archive refusal keeps its code
    // there and the sentence in `message`. Either way the person reads what stopped the stage.
    let said = refusal["message"]
        .as_str()
        .filter(|message| !message.trim().is_empty())
        .or_else(|| refusal["error"].as_str())
        .unwrap_or_default();
    eprintln!("zt-digest {mode}: {said}");
    Ok(LOCKED_EXIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock() -> Lock {
        Lock { lock_id: 2, tree: "K".into(), since: "2026-10-21T10:00:00.000Z".into() }
    }

    #[test]
    fn the_message_is_the_one_owner_contract() {
        assert_eq!(message("K"), "the Literature tree K is locked by the owner");
        assert_eq!(message("Smith2024"), "the Literature tree Smith2024 is locked by the owner");
    }

    #[test]
    fn a_read_carries_the_lock_facts_or_none() {
        let locked = json!({"address": "K", "lock": {"lock_id": 2, "tree": "K", "since": "2026-10-21T10:00:00.000Z"}});
        assert_eq!(lock_fact(&locked), Some(lock()));
        assert_eq!(lock_fact(&json!({"address": "K", "lock": null})), None);
        assert_eq!(lock_fact(&json!({"address": "K"})), None);
        // A Lock fact without its id or tree is not a Lock this client may act on.
        assert_eq!(lock_fact(&json!({"lock": {"tree": "K"}})), None);
        assert_eq!(lock_fact(&json!({"lock": {"lock_id": 2}})), None);
    }

    #[test]
    fn the_refusal_names_the_mode_step_tree_and_locks_typed_code() {
        let value = refusal("skeleton", "plan", "K", &lock());
        assert_eq!(value["ok"], json!(false));
        assert_eq!(value["code"], json!("card_locked"));
        assert_eq!(value["error"], json!("the Literature tree K is locked by the owner"));
        assert_eq!(value["mode"], json!("skeleton"));
        assert_eq!(value["step"], json!("plan"));
        assert_eq!(value["tree"], json!("K"));
        assert_eq!(value["lock_id"], json!(2));
        assert_eq!(value["locked_at"], json!("2026-10-21T10:00:00.000Z"));
        // The hint is an owner action, never a bypass, a route name or a token.
        let hint = value["hint"].as_str().unwrap();
        assert!(hint.contains("unlock the tree in the GUI"), "{hint}");
        assert!(!hint.contains("/api/"), "{hint}");
        assert!(!hint.contains("token"), "{hint}");
        assert!(!hint.contains("Access"), "{hint}");
    }

    #[test]
    fn the_key_comes_from_the_staging_manifest_and_an_absent_one_is_no_destination() {
        let dir = std::env::temp_dir().join(format!("zt-lockcheck-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let staging = dir.to_string_lossy().into_owned();
        assert_eq!(key_from_staging(&staging), "");
        assert_eq!(key_from_staging(""), "");
        std::fs::write(dir.join("manifest.json"), r#"{"citation_key":"Smith2024","groups":[]}"#).unwrap();
        assert_eq!(key_from_staging(&staging), "Smith2024");
        std::fs::write(dir.join("manifest.json"), "{not json").unwrap();
        assert_eq!(key_from_staging(&staging), "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- S16 (#139): the planner's own blocked refusals -------------------------------------------

    #[test]
    fn a_cards_facts_are_its_lock_and_pin_roots() {
        let locked = json!({
            "lock": {"lock_id": 2, "tree": "4/0", "since": "2026-10-21T10:00:00.000Z"},
            "pinned_by": ["4/0", "7/0"],
        });
        let facts = facts_in(&locked);
        assert_eq!(
            facts.lock,
            Some(Lock { lock_id: 2, tree: "4/0".into(), since: "2026-10-21T10:00:00.000Z".into() })
        );
        assert_eq!(facts.pinned_by, vec!["4/0".to_string(), "7/0".to_string()]);
        assert_eq!(facts_in(&json!({"lock": null, "pinned_by": []})), Facts::default());
        assert_eq!(facts_in(&json!({})), Facts::default());
    }

    #[test]
    fn an_address_tree_id_and_inside_name_the_lock_unit_and_its_subtree() {
        assert_eq!(tree_id("4/2|a"), "4");
        assert_eq!(tree_id("Smith2024/1"), "Smith2024");
        assert!(inside("4/2|a", "4/2"));
        assert!(inside("4/2|a/1", "4/2"));
        assert!(inside("4/2", "4/2"));
        assert!(!inside("4/20", "4/2"));
        assert!(!inside("5/1", "4/2"));
        assert!(!inside("4/1", ""));
    }

    #[test]
    fn a_blocked_member_names_its_card_tree_reason_lock_and_hint() {
        let value = blocked_card("move", "15/2|a", &lock());
        assert_eq!(value["error"], json!("card_locked"));
        assert_eq!(value["code"], json!("card_locked"));
        assert_eq!(value["reason"], json!("move"));
        assert_eq!(value["tree"], json!("K"));
        assert_eq!(value["cards"], json!(["15/2|a"]));
        assert_eq!(value["locks"], json!([{"lock_id": 2, "tree": "K"}]));
        assert_eq!(value["hint"], json!("unlock the tree in the GUI first"));
        assert!(value["message"].as_str().unwrap().contains("cannot be moved"), "{value}");
        // A pin says it is a pinned target and gives the owner the other choice (R336).
        let pin = blocked_pin("pinned-move", "12/3", &["K".to_string()], None);
        assert_eq!(pin["reason"], json!("pinned-move"));
        assert_eq!(pin["tree"], json!("K"));
        assert!(pin["message"].as_str().unwrap().contains("pinned target"), "{pin}");
        assert!(pin["hint"].as_str().unwrap().contains("link the locked Card"), "{pin}");
        // A destination inside a Locked tree names the address that cannot be written.
        let inside = blocked_destination("move-inside", "12/1|a", &lock());
        assert_eq!(inside["reason"], json!("move-inside"));
        assert_eq!(inside["cards"], json!(["12/1|a"]));
        assert!(inside["message"].as_str().unwrap().contains("no moved-in Cards"), "{inside}");
    }

    #[test]
    fn a_blocked_refusal_sorts_and_deduplicates_its_facts() {
        let other = Lock { lock_id: 1, tree: "B/0".into(), since: String::new() };
        let value = blocked(
            "edit",
            &["z/1".to_string(), "a/1".to_string(), "z/1".to_string()],
            &["B/0".to_string(), "A/0".to_string(), "B/0".to_string()],
            &[lock_with(2, "A/0"), other.clone(), lock_with(2, "A/0")],
            &["s/2".to_string(), "s/1".to_string(), "s/1".to_string()],
        );
        assert_eq!(value["cards"], json!(["a/1", "z/1"]));
        assert_eq!(value["tree"], json!("A/0"));
        assert_eq!(value["sources"], json!(["s/1", "s/2"]));
        assert_eq!(value["locks"], json!([{"lock_id": 1, "tree": "B/0"}, {"lock_id": 2, "tree": "A/0"}]));
    }

    fn lock_with(id: i64, tree: &str) -> Lock {
        Lock { lock_id: id, tree: tree.into(), since: String::new() }
    }
}
