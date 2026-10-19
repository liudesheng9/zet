//! Spreads: named sets of Cards a reader laid out together on the GUI's Table.
//!
//! A Spread is not a Link and changes no Card. It records which Cards lie on
//! the Table, where each lies, and which lies on top (the last listed); a Spread
//! kept from an answered question also keeps that whole question, while its
//! display name stays within the schema's own limit. The archive keeps every
//! Spread in its `metadata` table, so a Spread survives a restart and a new
//! port, and it follows its Cards: a move, a Citation-key rename, or a
//! side-successor compaction rewrites its addresses in the same transaction,
//! and a deleted Card leaves every Spread.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

const KEY: &str = "gui_spreads";
const MAX_SPREADS: usize = 64;
const MAX_CARDS: usize = 400;
const MAX_NAME: usize = 80;
const MAX_QUESTION: usize = 512;
const MAX_ID: usize = 40;
const MAX_COORD: f64 = 1.0e6;

/// Every Spread, as `{ "current": <id>, "spreads": [{ "id", "name", "question"?, "cards": [{ "at", "x", "y" }] }] }`.
/// `question` is the whole question an Ask Spread was kept from, when the reader asked one.
pub(crate) fn load(conn: &Connection) -> Result<Value> {
    let stored: Option<String> = conn
        .query_row("SELECT value FROM metadata WHERE key = ?1", [KEY], |row| {
            row.get(0)
        })
        .optional()?;
    Ok(stored
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| normalize(&value).ok())
        .unwrap_or_else(empty))
}

/// Replace every Spread with `value`, checked and normalized; returns what was kept.
pub(crate) fn save(conn: &Connection, value: &Value) -> Result<Value> {
    let normalized = normalize(value)?;
    store(conn, &normalized)?;
    Ok(normalized)
}

/// Keep every Spread on its Cards after a change of addresses: `mapping`
/// renames addresses (old to new) and `deleted` Cards leave every Spread.
pub(crate) fn follow(
    conn: &Connection,
    mapping: &BTreeMap<String, String>,
    deleted: &BTreeSet<String>,
) -> Result<()> {
    if mapping.is_empty() && deleted.is_empty() {
        return Ok(());
    }
    let mut value = load(conn)?;
    let mut changed = false;
    for spread in value["spreads"].as_array_mut().into_iter().flatten() {
        let Some(cards) = spread["cards"].as_array_mut() else {
            continue;
        };
        let before = cards.len();
        cards.retain(|card| !card["at"].as_str().is_some_and(|at| deleted.contains(at)));
        changed |= cards.len() != before;
        for card in cards.iter_mut() {
            if let Some(new) = card["at"].as_str().and_then(|at| mapping.get(at)) {
                card["at"] = Value::String(new.clone());
                changed = true;
            }
        }
    }
    if changed {
        store(conn, &normalize(&value)?)?;
    }
    Ok(())
}

fn store(conn: &Connection, value: &Value) -> Result<()> {
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![KEY, value.to_string()],
    )?;
    Ok(())
}

fn empty() -> Value {
    json!({ "current": Value::Null, "spreads": [] })
}

/// Check the shape and bounds of a Spread document. A Card listed twice in a
/// Spread keeps its last place, which is the one on top.
fn normalize(value: &Value) -> Result<Value> {
    let spreads = value
        .get("spreads")
        .and_then(Value::as_array)
        .context("spreads must be a list")?;
    if spreads.len() > MAX_SPREADS {
        bail!("at most {MAX_SPREADS} spreads may be kept");
    }
    let mut ids = BTreeSet::new();
    let mut kept = Vec::with_capacity(spreads.len());
    for spread in spreads {
        let id = text(spread, "id", MAX_ID)?;
        if id.is_empty() || !ids.insert(id.clone()) {
            bail!("every spread needs its own id");
        }
        let name = display_name(spread)?;
        let cards = spread
            .get("cards")
            .and_then(Value::as_array)
            .context("a spread's cards must be a list")?;
        if cards.len() > MAX_CARDS {
            bail!("a spread holds at most {MAX_CARDS} cards");
        }
        let mut last = BTreeMap::new();
        for (index, card) in cards.iter().enumerate() {
            let at = text(card, "at", 512)?;
            if at.is_empty() {
                bail!("a card on a spread needs an address");
            }
            last.insert(at, index);
        }
        let mut placed = Vec::with_capacity(last.len());
        for (index, card) in cards.iter().enumerate() {
            let at = text(card, "at", 512)?;
            if last.get(&at) != Some(&index) {
                continue;
            }
            placed.push(json!({ "at": at, "x": coord(card, "x")?, "y": coord(card, "y")? }));
        }
        let mut entry = Map::new();
        entry.insert("id".into(), Value::String(id));
        entry.insert("name".into(), Value::String(name));
        entry.insert("cards".into(), Value::Array(placed));
        // A kept question is durable; a Spread saved before the field existed simply has none.
        if let Some(question) = question(spread)? {
            entry.insert("question".into(), Value::String(question));
        }
        kept.push(Value::Object(entry));
    }
    let current = value
        .get("current")
        .and_then(Value::as_str)
        .filter(|id| ids.contains(*id))
        .map(str::to_string)
        .or_else(|| {
            kept.first()
                .and_then(|spread| spread["id"].as_str())
                .map(str::to_string)
        });
    Ok(json!({ "current": current, "spreads": kept }))
}

fn text(value: &Value, field: &str, limit: usize) -> Result<String> {
    let text = value
        .get(field)
        .and_then(Value::as_str)
        .with_context(|| format!("`{field}` must be text"))?
        .trim()
        .to_string();
    if text.chars().count() > limit {
        bail!("`{field}` is longer than {limit} characters");
    }
    Ok(text)
}

/// The display name: trimmed and cut to the schema's own 80 characters. A Spread kept from a
/// question takes the question as its name, and only the tail is cut there; the whole question
/// stays in `question` (D233).
fn display_name(value: &Value) -> Result<String> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .context("`name` must be text")?
        .trim();
    Ok(name.chars().take(MAX_NAME).collect())
}

/// The whole question a Spread was kept from (D233, D228): optional, trimmed, absent when blank,
/// and at most the question path's own 512 characters counted as characters.
fn question(value: &Value) -> Result<Option<String>> {
    let Some(raw) = value.get("question") else {
        return Ok(None);
    };
    let text = match raw {
        Value::Null => return Ok(None),
        Value::String(text) => text.trim(),
        _ => bail!("`question` must be text"),
    };
    if text.is_empty() {
        return Ok(None);
    }
    if text.chars().count() > MAX_QUESTION {
        bail!("a spread's question is longer than {MAX_QUESTION} characters");
    }
    Ok(Some(text.to_string()))
}

fn coord(value: &Value, field: &str) -> Result<f64> {
    let number = value
        .get(field)
        .and_then(Value::as_f64)
        .filter(|number| number.is_finite())
        .with_context(|| format!("`{field}` must be a number"))?;
    Ok((number.clamp(-MAX_COORD, MAX_COORD) * 10.0).round() / 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);")
            .unwrap();
        conn
    }

    fn spread(cards: Value) -> Value {
        json!({ "current": "a", "spreads": [{ "id": "a", "name": " Firms ", "cards": cards }] })
    }

    #[test]
    fn an_archive_without_spreads_has_none() {
        assert_eq!(load(&conn()).unwrap(), empty());
    }

    #[test]
    fn a_saved_spread_loads_back_with_its_last_place_for_a_repeated_card() {
        let conn = conn();
        save(
            &conn,
            &spread(json!([
                { "at": "1/1", "x": 0, "y": 0 },
                { "at": "1/2", "x": 10.04, "y": -5 },
                { "at": "1/1", "x": 300, "y": 40 },
            ])),
        )
        .unwrap();
        let loaded = load(&conn).unwrap();
        assert_eq!(loaded["current"], "a");
        assert_eq!(loaded["spreads"][0]["name"], "Firms");
        assert_eq!(
            loaded["spreads"][0]["cards"],
            json!([{ "at": "1/2", "x": 10.0, "y": -5.0 }, { "at": "1/1", "x": 300.0, "y": 40.0 }])
        );
    }

    #[test]
    fn malformed_spreads_are_refused() {
        let conn = conn();
        assert!(save(&conn, &json!({ "spreads": "no" })).is_err());
        assert!(
            save(
                &conn,
                &spread(json!([{ "at": "1/1", "x": "left", "y": 0 }]))
            )
            .is_err()
        );
        let twice = json!({ "spreads": [
            { "id": "a", "name": "", "cards": [] },
            { "id": "a", "name": "", "cards": [] },
        ] });
        assert!(save(&conn, &twice).is_err());
    }

    /// D233, S16-AC02: a question longer than the display-name limit is kept whole in `question`,
    /// while only the name is cut to the Spread schema's 80 characters — by characters, not bytes.
    #[test]
    fn a_kept_question_survives_while_only_the_display_name_is_cut() {
        let conn = conn();
        let question = "为什么学习可能造成多重均衡？".repeat(9);
        assert!(question.chars().count() > MAX_NAME);
        assert!(question.chars().count() <= MAX_QUESTION);
        let saved = save(
            &conn,
            &json!({ "current": "a", "spreads": [{
                "id": "a",
                "name": question,
                "question": question,
                "cards": [{ "at": "1/1", "x": 0, "y": 0 }],
            }] }),
        )
        .unwrap();
        let name = saved["spreads"][0]["name"].as_str().unwrap();
        assert_eq!(name.chars().count(), MAX_NAME, "{name}");
        assert!(question.starts_with(name), "only the tail is cut: {name}");
        assert_eq!(saved["spreads"][0]["question"], json!(question));
        assert_eq!(load(&conn).unwrap()["spreads"][0]["question"], json!(question));
    }

    /// D233, S16-AC03: a Spread written before the field existed loads and saves byte for byte,
    /// and a blank question is absent rather than stored as an empty string.
    #[test]
    fn a_legacy_spread_without_a_question_stays_as_it_was() {
        let conn = conn();
        let legacy = json!({ "current": "a", "spreads": [{
            "id": "a",
            "name": "Work",
            "cards": [{ "at": "1/1", "x": 0.0, "y": 0.0 }],
        }] });
        assert_eq!(save(&conn, &legacy).unwrap(), legacy);
        assert_eq!(load(&conn).unwrap(), legacy);
        let blank = save(
            &conn,
            &json!({ "current": "a", "spreads": [{
                "id": "a", "name": "Work", "question": "   ", "cards": [],
            }] }),
        )
        .unwrap();
        assert!(blank["spreads"][0].get("question").is_none(), "{blank}");
    }

    /// A question is bounded by the question path's own 512 characters (D228), counted as
    /// characters, and a value of the wrong type is a malformed document.
    #[test]
    fn a_question_is_bounded_and_typed() {
        let conn = conn();
        let held = |question: Value| {
            json!({ "current": "a", "spreads": [{
                "id": "a", "name": "Q", "question": question, "cards": [],
            }] })
        };
        let longest = "学".repeat(MAX_QUESTION);
        let saved = save(&conn, &held(json!(longest))).unwrap();
        assert_eq!(saved["spreads"][0]["question"], json!(longest));
        assert!(save(&conn, &held(json!("学".repeat(MAX_QUESTION + 1)))).is_err());
        assert!(save(&conn, &held(json!(7))).is_err());
        let absent = save(&conn, &held(Value::Null)).unwrap();
        assert!(absent["spreads"][0].get("question").is_none(), "{absent}");
    }

    /// D233, S16-AC05: following a move or a delete rewrites addresses in place and keeps the
    /// kept question, so a Spread never loses why it was gathered.
    #[test]
    fn a_spreads_question_survives_a_move_and_a_delete() {
        let conn = conn();
        save(
            &conn,
            &json!({ "current": "a", "spreads": [{
                "id": "a",
                "name": "为什么学习可能造成多重均衡？",
                "question": "为什么学习可能造成多重均衡？",
                "cards": [
                    { "at": "1/1", "x": 0, "y": 0 },
                    { "at": "Smith2024/1", "x": 10, "y": 10 },
                ],
            }] }),
        )
        .unwrap();
        let mapping = BTreeMap::from([("Smith2024/1".to_string(), "Smith2025/1".to_string())]);
        let deleted = BTreeSet::from(["1/1".to_string()]);
        follow(&conn, &mapping, &deleted).unwrap();
        let spread = &load(&conn).unwrap()["spreads"][0];
        assert_eq!(spread["question"], json!("为什么学习可能造成多重均衡？"));
        assert_eq!(spread["name"], json!("为什么学习可能造成多重均衡？"));
        assert_eq!(
            spread["cards"],
            json!([{ "at": "Smith2025/1", "x": 10.0, "y": 10.0 }])
        );
    }

    #[test]
    fn spreads_follow_moved_cards_and_drop_deleted_ones() {
        let conn = conn();
        save(
            &conn,
            &spread(json!([
                { "at": "1/1", "x": 0, "y": 0 },
                { "at": "1/2", "x": 1, "y": 1 },
                { "at": "Smith2024/1", "x": 2, "y": 2 },
            ])),
        )
        .unwrap();
        let mapping = BTreeMap::from([("Smith2024/1".to_string(), "Smith2025/1".to_string())]);
        let deleted = BTreeSet::from(["1/2".to_string()]);
        follow(&conn, &mapping, &deleted).unwrap();
        let cards = &load(&conn).unwrap()["spreads"][0]["cards"];
        assert_eq!(
            cards,
            &json!([{ "at": "1/1", "x": 0.0, "y": 0.0 }, { "at": "Smith2025/1", "x": 2.0, "y": 2.0 }])
        );
    }
}
