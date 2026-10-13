//! Spreads: named sets of Cards a reader laid out together on the GUI's Table.
//!
//! A Spread is not a Link and changes no Card. It records which Cards lie on
//! the Table, where each lies, and which lies on top (the last listed). The
//! archive keeps every Spread in its `metadata` table, so a Spread survives a
//! restart and a new port, and it follows its Cards: a move, a Citation-key
//! rename, or a side-successor compaction rewrites its addresses in the same
//! transaction, and a deleted Card leaves every Spread.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

const KEY: &str = "gui_spreads";
const MAX_SPREADS: usize = 64;
const MAX_CARDS: usize = 400;
const MAX_NAME: usize = 80;
const MAX_ID: usize = 40;
const MAX_COORD: f64 = 1.0e6;

/// Every Spread, as `{ "current": <id>, "spreads": [{ "id", "name", "cards": [{ "at", "x", "y" }] }] }`.
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
        let name = text(spread, "name", MAX_NAME)?;
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
