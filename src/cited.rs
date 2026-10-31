//! Cited Cards (#217): the cited literature of one book.
//!
//! A Card of type `cited` has one of two roles, computed from its placement and never stored. A
//! Cited list Card is a `cited` Card whose parent is not a `cited` Card; its body holds the
//! five-column table of the works the book cites (`编号`, `文献`, `年份`, `引用处`, `关系`). Its
//! immediate Side successors of type `cited` are its Cited Cards, matched to the table rows by
//! order: the first Side successor states row 1, the second row 2, and so on. A Cited Card names
//! its row in its source line (`来源：[[book]] 参考文献第 n 条`), so a Card filed at the wrong place
//! is reported. Any other `cited` Card under a list is unfiled. The faults are reported, never
//! refused.

use std::collections::BTreeMap;

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::CardType;

/// One row of a Cited list table.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Row {
    pub(crate) row: usize,
    pub(crate) entry: String,
    pub(crate) year: String,
    pub(crate) cited_at: String,
    pub(crate) relation: String,
}

/// The rows of a Cited list body: the first Markdown table whose header names `编号` and `文献`.
/// The other columns are read by their header names, so a missing column reads as empty.
pub(crate) fn table(body: &str) -> Vec<Row> {
    let mut header: Option<Vec<String>> = None;
    let mut rows = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if !line.starts_with('|') {
            if header.is_some() && !rows.is_empty() {
                break;
            }
            continue;
        }
        let cells = cells(line);
        match &header {
            None => {
                if cells.iter().any(|cell| cell == "编号") && cells.iter().any(|cell| cell == "文献") {
                    header = Some(cells);
                }
            }
            Some(names) => {
                if cells.iter().all(|cell| cell.chars().all(|ch| matches!(ch, '-' | ':' | ' '))) {
                    continue;
                }
                let get = |name: &str| {
                    names
                        .iter()
                        .position(|known| known == name)
                        .and_then(|index| cells.get(index))
                        .cloned()
                        .unwrap_or_default()
                };
                rows.push(Row {
                    row: get("编号").parse().unwrap_or(rows.len() + 1),
                    entry: get("文献"),
                    year: get("年份"),
                    cited_at: get("引用处"),
                    relation: get("关系"),
                });
            }
        }
    }
    rows
}

/// The trimmed cells of one table line, without the outer bars.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|').trim_end_matches('|');
    inner.split('|').map(|cell| cell.trim().to_string()).collect()
}

/// The list row a Cited Card names in its source line (`参考文献第 n 条`), if any.
pub(crate) fn named_row(body: &str) -> Option<usize> {
    let start = body.rfind("参考文献第")? + "参考文献第".len();
    let digits: String = body[start..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Whether `address` is a Side successor of its parent (its last segment is a letter label).
fn is_side(address: &str) -> bool {
    !crate::valid_positive_number(crate::last_segment(address))
}

/// The stored type of one Regular Card, if it exists.
fn stored_type(conn: &Connection, at: &str) -> Result<Option<CardType>> {
    let mut statement =
        conn.prepare_cached("SELECT card_type FROM cards WHERE location = ?1")?;
    let mut rows = statement.query([at])?;
    Ok(match rows.next()? {
        Some(row) => Some(CardType::from_stored(&row.get::<_, String>(0)?)),
        None => None,
    })
}

/// Whether the `cited` Card at `address` is a Cited list Card: its parent is not a `cited` Card.
fn is_list(conn: &Connection, address: &str) -> Result<bool> {
    Ok(match crate::parent_location(address) {
        Some(parent) => stored_type(conn, &parent)? != Some(CardType::Cited),
        None => true,
    })
}

/// The Cited list Card that `at` names: the list itself, a Card under it, or a tree that holds
/// exactly one list.
pub(crate) fn resolve_list(conn: &Connection, at: &str) -> Result<crate::Card> {
    let card = crate::load_card(conn, at)?;
    if card.card_type == CardType::Cited && is_list(conn, &card.address)? {
        return Ok(card);
    }
    let mut current = crate::parent_location(&card.address);
    while let Some(parent) = current {
        match stored_type(conn, &parent)? {
            Some(CardType::Cited) if is_list(conn, &parent)? => {
                return crate::load_card(conn, &parent);
            }
            Some(_) => current = crate::parent_location(&parent),
            None => break,
        }
    }
    if crate::is_tree_root(&card) {
        let tree = crate::tree_id(&card.address);
        let lists = tree_lists(conn, tree)?;
        return match lists.as_slice() {
            [one] => crate::load_card(conn, one),
            [] => bail!("tree `{tree}` has no Cited list"),
            many => bail!(
                "tree `{tree}` has {} Cited lists ({}); name the one to read",
                many.len(),
                many.join(", ")
            ),
        };
    }
    bail!(
        "`{at}` is not a Cited list and is not under one; pass a Cited list or a tree that holds exactly one"
    )
}

/// The Cited lists of one tree, in natural address order.
pub(crate) fn tree_lists(conn: &Connection, tree: &str) -> Result<Vec<String>> {
    let lower = format!("{tree}/");
    let upper = format!("{tree}0");
    let mut statement = conn.prepare_cached(
        "SELECT location FROM cards WHERE location >= ?1 AND location < ?2 AND card_type = 'cited'",
    )?;
    let cited = statement
        .query_map([&lower, &upper], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut lists = Vec::new();
    for address in cited {
        if crate::is_tree_member(&address, tree) && is_list(conn, &address)? {
            lists.push(address);
        }
    }
    lists.sort_by(|a, b| crate::model::address_order(a, b));
    Ok(lists)
}

/// The computed structure of one Cited list: its rows, its Cited Cards, the unfiled `cited` Cards
/// under it, the faults and the coverage.
pub(crate) fn structure(conn: &Connection, at: &str) -> Result<Value> {
    let list = resolve_list(conn, at)?;
    let parsed = crate::parse_card_text(&list.text)?;
    let rows = table(&parsed.body);

    // The list's subtree: one prefix range names exactly its descendants (see `term::structure`).
    let lower = format!("{}|", list.address);
    let upper = format!("{}}}", list.address);
    let mut statement = conn.prepare_cached(
        "SELECT location, card_type, text FROM cards
         WHERE location >= ?1 AND location < ?2 ORDER BY location",
    )?;
    let subtree = statement
        .query_map(params![lower, upper], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .filter(|(_, card_type, _)| CardType::from_stored(card_type) == CardType::Cited)
        .map(|(address, _, text)| Ok((address, crate::parse_card_text(&text)?)))
        .collect::<Result<BTreeMap<_, _>>>()?;
    drop(statement);

    let mut cards: Vec<(&String, &crate::ParsedCard)> = Vec::new();
    let mut unfiled: Vec<&String> = Vec::new();
    for (address, card) in &subtree {
        let immediate = crate::parent_location(address).as_deref() == Some(list.address.as_str());
        if immediate && is_side(address) {
            cards.push((address, card));
        } else {
            unfiled.push(address);
        }
    }
    cards.sort_by(|a, b| crate::model::address_order(a.0, b.0));
    unfiled.sort_by(|a, b| crate::model::address_order(a, b));

    let mut faults = Vec::new();
    for address in &unfiled {
        faults.push(json!({ "fault": "cited_unfiled", "address": address }));
    }
    let mut card_values = Vec::new();
    for (index, (address, card)) in cards.iter().enumerate() {
        let row = index + 1;
        let named = named_row(&card.body);
        if row > rows.len() {
            faults.push(json!({ "fault": "cited_card_without_row", "address": address, "row": row }));
        } else if named.is_some_and(|named| named != row) {
            faults.push(json!({
                "fault": "cited_row_mismatch",
                "address": address,
                "row": row,
                "names": named,
            }));
        }
        card_values.push(json!({
            "address": address,
            "title": card.title,
            "row": row,
            "names_row": named,
        }));
    }
    let mut row_values = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let card = cards.get(index).map(|(address, _)| (*address).clone());
        if card.is_none() {
            faults.push(json!({ "fault": "cited_row_without_card", "row": row.row, "entry": row.entry }));
        }
        row_values.push(json!({
            "row": row.row,
            "entry": row.entry,
            "year": row.year,
            "cited_at": row.cited_at,
            "relation": row.relation,
            "card": card,
        }));
    }
    let missing = rows.len().saturating_sub(cards.len());
    Ok(json!({
        "list": list.address,
        "title": parsed.title,
        "rows": row_values,
        "cards": card_values,
        "unfiled": unfiled,
        "faults": faults,
        "coverage": { "rows": rows.len(), "cards": cards.len(), "missing": missing },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_read_by_its_header_names_and_the_named_row_by_its_source_line() {
        let body = "本卡是参考文献表。\n\n| 编号 | 文献 | 年份 | 引用处 | 关系 |\n| --- | --- | --- | --- | --- |\n| 1 | A. Title. *J* | 2017 | §1 | 依赖 |\n| 2 | B. Other. | 2009a | 仅参考文献 | 背景 |\n\n来源：[[K]] 参考文献\n";
        let rows = table(body);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].entry, "A. Title. *J*");
        assert_eq!(rows[1].year, "2009a");
        assert_eq!(rows[1].relation, "背景");
        assert_eq!(named_row("来源：[[K]] 参考文献第 14 条"), Some(14));
        assert_eq!(named_row("来源：[[K]] 参考文献"), None);
    }
}
