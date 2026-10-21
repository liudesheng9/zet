//! Reply: the Owner's answer to an agent Card, written as a human Card (S18, #99; design §10.1).
//!
//! A Reply is a GUI action, not an MCP tool: the page sends the target it read, the target's authored
//! fingerprint and the reply text, and this module writes exactly one human Card inside one
//! transaction — creating the default Topic `回应` when the reader has no human Topic to reply in.
//! Nothing is appended to the agent Card: the reply Links back to it, and the ordinary Reverse link
//! regeneration makes it visible there.

use crate::{Access, CardType, Workspace};
use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::path::Path;

/// The Topic a reply falls back to when the reader has no usable human Topic (D246).
pub(crate) const DEFAULT_TOPIC: &str = "回应";
/// The body the default Topic is created with: one sentence, so it is a Card like any other.
const DEFAULT_TOPIC_BODY: &str = "对代理工作区卡片的人类回应。";
/// A reply carries its own thought: a bare Link cannot be saved as a substantive reply (D244).
pub(crate) const MIN_SUBSTANCE: usize = 8;

/// What the page asks for: the agent Card it read, that Card's authored fingerprint, the reply text
/// and the human Topic the reader last used (absent: the default).
pub(crate) struct Request<'a> {
    pub(crate) target: &'a str,
    pub(crate) base: &'a str,
    pub(crate) text: &'a str,
    pub(crate) topic: Option<&'a str>,
}

/// The result: a saved human Card, or the exact reason it was not saved and the fresh context the
/// reader needs to decide again. A refusal keeps the reader's draft, so it is not an error.
pub(crate) enum Outcome {
    Saved(Value),
    Refused(Value),
}

impl Outcome {
    pub(crate) fn into_value(self) -> Value {
        match self {
            Outcome::Saved(value) | Outcome::Refused(value) => value,
        }
    }
}

/// Write one reply Card. Owner access only: the human Workspace is the person's own. A refused
/// reply writes nothing at all: the transaction is rolled back rather than committed, so a fallback
/// Topic created on the way is not left behind.
pub(crate) fn write(root: &Path, request: &Request) -> Result<Outcome> {
    let _lock = crate::acquire_edit_lock_within(root, std::time::Duration::from_secs(20), "reply")?;
    let mut conn = crate::open_db(root)?;
    let tx = conn.transaction()?;
    let outcome = write_tx(&tx, request)?;
    match &outcome {
        Outcome::Saved(value) => {
            crate::link_index::source_changed(
                &tx,
                value["address"]
                    .as_str()
                    .context("reply result names no address")?,
            )?;
            tx.commit()?;
        }
        Outcome::Refused(_) => tx.rollback()?,
    }
    Ok(outcome)
}

fn write_tx(conn: &Connection, request: &Request) -> Result<Outcome> {
    if request.base.trim().is_empty() {
        bail!("a reply needs the target's authored fingerprint");
    }
    let target = crate::load_card(conn, request.target)
        .with_context(|| format!("card `{}` does not exist", request.target))?;
    let fingerprint = crate::card::authored_fingerprint(&target)?;
    let title = crate::parse_card_text(&target.text)?.title;
    let context = json!({
        "address": target.address,
        "title": title,
        "workspace": target.workspace.id(),
        "authored_fingerprint": fingerprint,
    });
    if target.workspace != Workspace::Agent {
        return Ok(Outcome::Refused(json!({
            "saved": false,
            "reason": "target_not_agent",
            "detail": format!("`{}` is in the {} Workspace; a Reply answers an agent Card", target.address, target.workspace.id()),
            "target": context,
        })));
    }
    if fingerprint != request.base.trim() {
        return Ok(Outcome::Refused(json!({
            "saved": false,
            "reason": "target_changed",
            "detail": "the agent Card changed since you opened Reply; read it again and save once more",
            "target": context,
        })));
    }
    let parsed = crate::parse_card_text(request.text)?;
    let link = crate::model::read_links(&parsed.body, |at| at.to_string())
        .into_iter()
        .find(|link| link.target == target.address);
    let Some(link) = link else {
        return Ok(Outcome::Refused(json!({
            "saved": false,
            "reason": "target_link_missing",
            "detail": format!("the reply must link `{}` in its own sentence", target.address),
            "target": context,
        })));
    };
    if link.kind == crate::model::RelationKind::Refers {
        return Ok(Outcome::Refused(json!({
            "saved": false,
            "reason": "link_unkinded",
            "detail": "put one relation word right before the Link to the agent Card: 印证, 反驳, 放宽, or 对应",
            "target": context,
        })));
    }
    let substance = substance_of(&parsed.body, &target.address);
    if substance < MIN_SUBSTANCE {
        return Ok(Outcome::Refused(json!({
            "saved": false,
            "reason": "reply_bare",
            "detail": "a bare Link is not a reply: write the thought and the conditions it rests on",
            "target": context,
            "substance": substance,
        })));
    }
    let destination = destination(conn, request.topic)?;
    let (parent, edge) = placement(conn, &destination.address)?;
    // A locked human destination is a kept refusal, not an error: the reader's draft stays in the
    // desk, the refusal names the Locked tree, and the transaction is rolled back (R061, T09).
    let address = match crate::ops::create_successor_tx(
        conn,
        Access::Owner,
        edge,
        &parent,
        CardType::Idea,
        None,
        request.text,
    ) {
        Ok(address) => address,
        Err(error) => match error.downcast_ref::<crate::tree_lock::Refusal>() {
            Some(refusal) => {
                return Ok(Outcome::Refused(json!({
                    "saved": false,
                    "reason": "locked_destination",
                    "detail": format!(
                        "the reply was not saved: {}; choose another human Topic",
                        refusal
                    ),
                    "refusal": refusal.value(),
                    "target": context,
                    "destination": {
                        "topic": destination.address,
                        "title": destination.title,
                    },
                })));
            }
            None => return Err(error),
        },
    };
    Ok(Outcome::Saved(json!({
        "saved": true,
        "address": address,
        "kind": link.kind.id(),
        "target": context,
        "destination": {
            "topic": destination.address,
            "title": destination.title,
            "created_topic": destination.created,
            "parent": parent,
            "edge": if edge == crate::ops::Successor::Direct { "direct" } else { "side" },
        },
    })))
}

/// The letters and digits a reply says beyond its Link and the relation word before it. The Link's
/// own target and the words naming the relation are the supplied skeleton, not the reader's thought.
fn substance_of(body: &str, target: &str) -> usize {
    let macro_text = format!("[[{target}]]");
    let mut text = body.replace(&macro_text, " ");
    for word in ["印证", "反驳", "放宽", "对应"] {
        text = text.replace(word, " ");
    }
    text.chars().filter(|character| character.is_alphanumeric()).count()
}

/// The human Topic a reply goes into: the one the reader used, when it still exists; else a human
/// Topic named `回应`; else that Topic, created now. A vanished address falls back rather than
/// following a stale numeric occupant (D247).
struct Destination {
    address: String,
    title: String,
    created: bool,
}

fn destination(conn: &Connection, wanted: Option<&str>) -> Result<Destination> {
    if let Some(at) = wanted.map(str::trim).filter(|at| !at.is_empty()) {
        if let Some(card) = crate::load_card(conn, at).ok().filter(|card| {
            card.is_topic && card.workspace == Workspace::Human && !card.is_lit
        }) {
            let title = crate::parse_card_text(&card.text)?.title;
            return Ok(Destination {
                address: card.address,
                title,
                created: false,
            });
        }
    }
    let mut statement = conn.prepare(
        "SELECT COALESCE(location,citation_key) FROM cards WHERE workspace=?1 AND is_topic=1 AND is_lit=0 ORDER BY COALESCE(location,citation_key)",
    )?;
    let topics = statement
        .query_map([Workspace::Human.id()], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    for address in topics {
        let card = crate::load_card(conn, &address)?;
        let title = crate::parse_card_text(&card.text)?.title;
        if title.trim() == DEFAULT_TOPIC {
            return Ok(Destination {
                address,
                title,
                created: false,
            });
        }
    }
    let text = format!("# {DEFAULT_TOPIC}\n\n{DEFAULT_TOPIC_BODY}\n");
    let address = crate::ops::create_topic_tx(conn, Access::Owner, Workspace::Human, &text)?;
    Ok(Destination {
        address,
        title: DEFAULT_TOPIC.to_string(),
        created: true,
    })
}

/// Where in the Topic the reply is filed, computed fresh at save time: a Side successor of the
/// Topic's last Regular Card in Folgezettel order, or the first Direct successor of an empty Topic
/// (D245).
fn placement(conn: &Connection, topic: &str) -> Result<(String, crate::ops::Successor)> {
    let tree = crate::tree_id(topic);
    let mut statement = conn.prepare("SELECT COALESCE(location,citation_key) FROM cards")?;
    let addresses = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let mut members: Vec<String> = addresses
        .into_iter()
        .filter(|at| at != topic && crate::is_tree_member(at, tree))
        .collect();
    if members.is_empty() {
        return Ok((topic.to_string(), crate::ops::Successor::Direct));
    }
    members.sort_by(|a, b| crate::model::address_order(a, b));
    let last = members.last().cloned().expect("members is not empty");
    let card = crate::load_card(conn, &last)?;
    if card.is_topic || card.is_lit {
        // A Literature tree never holds a reply; a Topic member that is still a tree root (no
        // Regular Card yet) takes the reply as its first Direct successor.
        return Ok((topic.to_string(), crate::ops::Successor::Direct));
    }
    Ok((last, crate::ops::Successor::Side))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reply_needs_its_own_thought_beyond_the_link() {
        // The relation word and the Link are the supplied skeleton, not the reader's thought.
        assert_eq!(substance_of("印证 [[1/2]]", "1/2"), 0);
        assert_eq!(substance_of("对应 [[1/2]]", "1/2"), 0);
        assert!(substance_of("印证 [[1/2]]", "1/2") < MIN_SUBSTANCE);
        assert!(substance_of("印证 [[1/2]]：C 违约时 E 清算资产。", "1/2") >= MIN_SUBSTANCE);
        assert!(substance_of("反驳 [[3/1|a]]：这个结论只在完全契约下成立。", "3/1|a") >= MIN_SUBSTANCE);
        // A sentence that names another Card counts by its own words, not by the Link text.
        assert!(substance_of("印证 [[1/2]]：参看 [[4/1]] 对谈判地位的推论。", "1/2") >= MIN_SUBSTANCE);
    }
}
