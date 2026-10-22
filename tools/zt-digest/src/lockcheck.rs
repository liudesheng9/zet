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

/// The Lock facts of one Card read (`get_card`/`get_cards`): `lock` is the active Lock of the
/// Card's tree, or null. A Card read without the field is an unlocked Card.
pub fn lock_fact(card: &Value) -> Option<Lock> {
    let lock = card.get("lock")?.as_object()?;
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

/// Reports a stopped step: the typed refusal as its last stdout line (so a caller reads the same
/// JSON it reads from every step), the exact sentence on stderr, and the refusal exit code. It
/// writes no file, allocates no job and changes no Card.
pub fn stop(mode: &str, refusal: &Value) -> Result<i32> {
    util::print_line(&util::compact(refusal))?;
    eprintln!("zt-digest {mode}: {}", refusal["error"].as_str().unwrap_or_default());
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
}
