//! The one authored field of a Term Card: its exact term name (S07, #149).
//!
//! A Term Card stores the term it defines in `cards.term_name`, a nullable single-line string of
//! at most 40 Unicode scalar values. No name means the Card is a candidate Term list; a name is
//! the definition it carries. The name is authored identity, so it travels in the Lock
//! fingerprint. One parser decides what a supplied name normalizes to, and the client field
//! protocol (`None` absent, `Some(None)` clear, `Some(Some(name))` set) is shared by every route.

use anyhow::{Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use crate::CardType;

/// The longest term name, and the same limit a Term-link label gets: Unicode scalar values. The
/// shared text module owns it, so the name parser and the lexical Link scanner cannot disagree.
pub(crate) const MAX_NAME_CHARS: usize = zt_check::text::MAX_TERM_NAME;

/// The refusal for a name supplied beside another Card type.
pub(crate) const NAME_NEEDS_TERM_CARD: &str =
    "only Term Cards can have a term name; set `type` to `term` first";

/// The characters a term name cannot hold: Link macro syntax and the Term-link separator.
const FORBIDDEN: [char; 5] = ['[', ']', '@', '|', '`'];

/// Normalize one supplied term name: trim the outer whitespace, treat an empty result as no name,
/// and refuse more than one line, more than [`MAX_NAME_CHARS`] scalar values, or macro syntax.
pub(crate) fn normalize_name(raw: &str) -> Result<Option<String>> {
    let name = raw.trim();
    if name.is_empty() {
        return Ok(None);
    }
    if name.contains(['\n', '\r']) {
        bail!("a term name must be one line");
    }
    if name.chars().count() > MAX_NAME_CHARS {
        bail!("a term name holds at most {MAX_NAME_CHARS} characters");
    }
    if name.contains(FORBIDDEN) {
        bail!("a term name cannot contain `[`, `]`, `@`, `|`, or a backtick");
    }
    Ok(Some(name.to_string()))
}

/// The client field protocol: absent leaves the stored name alone, `null` or a trimmed-empty
/// string clears it, and a string sets it.
pub(crate) fn parse_field(value: Option<&Value>) -> Result<Option<Option<String>>> {
    match value {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(raw)) => Ok(Some(normalize_name(raw)?)),
        Some(_) => bail!("argument `term` must be a string"),
    }
}

/// Refuse a supplied name whose Card type is not `term`.
pub(crate) fn ensure_on_type(name: Option<&str>, card_type: CardType) -> Result<()> {
    if name.is_some() && card_type != CardType::Term {
        bail!(NAME_NEEDS_TERM_CARD);
    }
    Ok(())
}

/// Write one Card's term name inside the caller's transaction. The caller has already checked the
/// final type and the Lock gate; the table's own check backs the same rule.
pub(crate) fn set_name(conn: &Connection, row_id: i64, name: Option<&str>) -> Result<()> {
    conn.execute(
        "UPDATE cards SET term_name = ?1 WHERE row_id = ?2",
        params![name, row_id],
    )?;
    Ok(())
}

// --- the computed Term structure (S08, #150) --------------------------------------------------

/// How a Term Card takes part in one Term list (design 9.2). The role is computed from the Card
/// type, its exact name and its placement; no column and no ownership table stores it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Role {
    /// A Term Card with no name: the list itself, holding the four-column table.
    List,
    /// A named Term Card that is an immediate Side successor of a list. Its wire role is `card`.
    Head,
    /// A named Term Card that continues a same-named head or part as its Direct successor.
    Part,
    /// A named Term Card in no valid list/head/part placement.
    Unfiled,
}

impl Role {
    /// The wire name of the role. A head is `card`, as the design names it, so no client mistakes
    /// the computed role for the Card type.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Role::List => "list",
            Role::Head => "card",
            Role::Part => "part",
            Role::Unfiled => "unfiled",
        }
    }
}

/// The longest placement chain a Term Card may have. Location parsing cannot cycle
/// (`parent_location` always answers a shorter or earlier address), so the bound and the visited
/// set are defensive: a Card whose chain exceeds either fails the read visibly rather than being
/// silently cut (S08-AC04).
pub(crate) const MAX_CHAIN: usize = 128;

/// A placement chain that cannot be resolved: it is too long, or it revisited an address.
#[derive(Debug)]
pub(crate) struct Placement {
    pub(crate) address: String,
    pub(crate) reason: &'static str,
}

impl Placement {
    fn depth(address: &str) -> Self {
        Self { address: address.to_string(), reason: "depth" }
    }

    fn cycle(address: &str) -> Self {
        Self { address: address.to_string(), reason: "cycle" }
    }

    /// The typed read error, so a client sees a limit rather than a truncated structure.
    pub(crate) fn value(&self) -> Value {
        json!({
            "error": self.to_string(),
            "code": "term_placement",
            "reason": self.reason,
            "address": self.address,
        })
    }
}

impl std::fmt::Display for Placement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.reason {
            "cycle" => write!(
                f,
                "the Term placement chain above `{}` is a cycle; the archive holds an impossible Location",
                self.address
            ),
            _ => write!(
                f,
                "the Term placement chain above `{}` is longer than {MAX_CHAIN} steps; the structure cannot be read",
                self.address
            ),
        }
    }
}

impl std::error::Error for Placement {}

/// One card's stored facts a placement needs: its type and exact name, never its text.
type Stored = (CardType, Option<String>);

/// The placement of one named Term Card.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Placed {
    pub(crate) role: Role,
    /// The address of the list the Card belongs to.
    pub(crate) list: String,
    /// The head this Card continues: the Card itself for a head.
    pub(crate) head: String,
}

/// Resolve one named Term Card's placement by walking its ancestor chain (design 9.2):
///
/// - a Side successor is a head only as an immediate Side child of a list, and a part only when
///   the chain up to a head continues the same name;
/// - a numbered Direct successor is a part only when its predecessor is a same-named head or
///   valid part;
/// - anything else has no valid placement.
///
/// The lookup answers one address's stored type and name, `None` when it does not exist. A missing
/// ancestor, another Card type or another name ends the walk with no placement.
pub(crate) fn place(
    address: &str,
    name: &str,
    lookup: &dyn Fn(&str) -> Result<Option<Stored>>,
) -> Result<Option<Placed>> {
    place_with(address, name, lookup, &crate::parent_location)
}

/// [`place`] with the parent function passed in, so a unit test can prove the cycle and depth
/// guards. Every ordinary caller uses the Location parser, whose answers always get shorter.
fn place_with(
    address: &str,
    name: &str,
    lookup: &dyn Fn(&str) -> Result<Option<Stored>>,
    parent_of: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Placed>> {
    let mut current = address.to_string();
    let mut part = false;
    let mut seen = BTreeSet::new();
    for _ in 0..MAX_CHAIN {
        if !seen.insert(current.clone()) {
            return Err(Placement::cycle(address).into());
        }
        let Some(parent) = parent_of(&current) else {
            return Ok(None);
        };
        let Some((parent_type, parent_name)) = lookup(&parent)? else {
            return Ok(None);
        };
        if parent_type != CardType::Term {
            return Ok(None);
        }
        if !crate::valid_positive_number(crate::last_segment(&current)) {
            // A Side successor: a head under a list, or the head a part continues.
            return Ok(parent_name.is_none().then(|| Placed {
                role: if part { Role::Part } else { Role::Head },
                list: parent,
                head: current,
            }));
        }
        if parent_name.as_deref() != Some(name) {
            return Ok(None);
        }
        part = true;
        current = parent;
    }
    Err(Placement::depth(address).into())
}

/// Read one address's stored type and name inside the caller's read.
fn stored(conn: &Connection, at: &str) -> Result<Option<Stored>> {
    let mut statement = conn.prepare_cached(
        "SELECT card_type, term_name FROM cards WHERE location = ?1 OR citation_key = ?1",
    )?;
    let row = statement
        .query_row([at], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        })
        .optional()?;
    Ok(row.map(|(card_type, name)| (CardType::from_stored(&card_type), name)))
}

/// The computed role of one stored Card (S08-AC01): `None` for a Card that is not a Term Card,
/// else its role and its owning list. A list's owning list is itself.
pub(crate) fn role(
    conn: &Connection,
    address: &str,
    card_type: CardType,
    name: Option<&str>,
) -> Result<Option<(Role, Option<String>)>> {
    if card_type != CardType::Term {
        return Ok(None);
    }
    let Some(name) = name else {
        return Ok(Some((Role::List, Some(address.to_string()))));
    };
    let lookup = |at: &str| stored(conn, at);
    Ok(match place(address, name, &lookup)? {
        Some(placed) => Some((placed.role, Some(placed.list))),
        None => Some((Role::Unfiled, None)),
    })
}

/// The Term facts of one stored Card, as every Card read publishes them (S08-AC02): the name is
/// already in the read, and these are its computed `term_role`, the `term_list` it belongs to and
/// the number of stored Term uses that name it. A Card of another type has no such facts.
pub(crate) fn facts(conn: &Connection, card: &crate::Card) -> Result<Option<Value>> {
    let Some((role, list)) = role(
        conn,
        &card.address,
        card.card_type,
        card.term_name.as_deref(),
    )? else {
        return Ok(None);
    };
    Ok(Some(json!({
        "term_role": role.id(),
        "term_list": list,
        "term_uses": uses_to(conn, &card.address)?,
    })))
}

/// How many stored Term uses (`links.kind = 'term'`) name one Card.
fn uses_to(conn: &Connection, at: &str) -> Result<u64> {
    let mut statement =
        conn.prepare_cached("SELECT COUNT(*) FROM links WHERE target = ?1 AND kind = 'term'")?;
    let count: i64 = statement.query_row([at], |row| row.get(0))?;
    Ok(count as u64)
}

/// Whether an address is a valid Term head: a named Term Card that is an immediate Side successor
/// of a Term list. Answers the list it is a head of; `None` for a missing Card, a list, a part, or
/// another type, which is exactly what `term_link_target` reports and what a new Term use must
/// pass (S10, #151).
fn head_of(conn: &Connection, at: &str) -> Result<Option<String>> {
    let Some((card_type, name)) = stored(conn, at)? else {
        return Ok(None);
    };
    if card_type != CardType::Term || name.is_none() {
        return Ok(None);
    }
    if crate::valid_positive_number(crate::last_segment(at)) {
        return Ok(None);
    }
    let Some(parent) = crate::parent_location(at) else {
        return Ok(None);
    };
    let Some((parent_type, parent_name)) = stored(conn, &parent)? else {
        return Ok(None);
    };
    Ok((parent_type == CardType::Term && parent_name.is_none()).then_some(parent))
}

/// Whether one address is a Term head right now: the one role a new Term use may target (S10-AC03).
pub(crate) fn is_head(conn: &Connection, at: &str) -> Result<bool> {
    Ok(head_of(conn, at)?.is_some())
}

/// The list a `get_terms {at}` read names: the Card itself when it is a list, else the nearest list
/// on its placement chain, else the one Term list of its tree (a book-level selection with no
/// silent first-by-address pick). Every other Card is refused visibly (S08-AC04).
pub(crate) fn resolve_list(conn: &Connection, at: &str) -> Result<crate::Card> {
    let card = crate::load_card(conn, at)?;
    if card.card_type == CardType::Term && card.term_name.is_none() {
        return Ok(card);
    }
    let mut current = crate::parent_location(&card.address);
    let mut steps = 0;
    while let Some(parent) = current {
        steps += 1;
        if steps > MAX_CHAIN {
            return Err(Placement::depth(at).into());
        }
        let Some((card_type, name)) = stored(conn, &parent)? else {
            break;
        };
        if card_type == CardType::Term && name.is_none() {
            return crate::load_card(conn, &parent);
        }
        current = crate::parent_location(&parent);
    }
    if crate::is_tree_root(&card) {
        let tree = crate::tree_id(&card.address);
        let lists = tree_term_lists(conn, tree)?;
        return match lists.as_slice() {
            [one] => crate::load_card(conn, one),
            [] => bail!("tree `{tree}` has no Term list"),
            many => bail!(
                "tree `{tree}` has {} Term lists ({}); name the one to read",
                many.len(),
                many.join(", ")
            ),
        };
    }
    bail!(
        "`{at}` is not a Term list and is not under one; pass a Term list or a tree that holds exactly one"
    )
}

/// The Term lists of one tree, in natural address order.
fn tree_term_lists(conn: &Connection, tree: &str) -> Result<Vec<String>> {
    let lower = format!("{tree}/");
    let upper = format!("{tree}0");
    let mut statement = conn.prepare_cached(
        "SELECT COALESCE(location, citation_key) FROM cards
         WHERE location >= ?1 AND location < ?2 AND card_type = 'term' AND term_name IS NULL",
    )?;
    let rows = statement
        .query_map([&lower, &upper], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut lists = rows
        .into_iter()
        .filter(|address| crate::is_tree_member(address, tree))
        .collect::<Vec<_>>();
    lists.sort_by(|a, b| crate::model::address_order(a, b));
    Ok(lists)
}

/// One Card of a list's subtree, as the structure needs it: its stored identity and its title.
struct Subtree {
    card_type: CardType,
    name: Option<String>,
    title: String,
}

/// One stored Term use of one head: the source Card, the label it wrote, and its occurrences.
struct Use {
    source: String,
    source_title: String,
    target: String,
    label: Option<String>,
    count: u64,
}

/// The authored label of the first Term use of `target` in one body: the shared lexical scan gives
/// the label and the address, so this reader never splits `label@address` on its own (S10-AC06).
fn label_of(body: &str, target: &str) -> Option<String> {
    for occurrence in crate::link::LinkBody::for_rendering(body).occurrences() {
        if occurrence.form != crate::link::LinkForm::Term || occurrence.target != target {
            continue;
        }
        if let Some(label) = occurrence.label.filter(|label| !label.is_empty()) {
            return Some(label.to_string());
        }
    }
    None
}

/// The computed structure of one Term list (S08, #150; design 9.2-9.3): the rows of its table,
/// its heads, parts and unfiled Cards, its Term uses, its coverage and its seven faults, read
/// inside the caller's transaction and bound to that revision by the caller.
pub(crate) fn structure(conn: &Connection, at: &str) -> Result<Value> {
    let list = resolve_list(conn, at)?;
    let parsed = crate::parse_card_text(&list.text)?;
    let rows = zt_check::text::term_table(&parsed.body);
    let raw_rows = zt_check::text::term_rows_raw(&parsed.body);

    // The list and every descendant, so a placement walk never leaves the map it is read from.
    // Locations have no wildcard characters, and `|` is the one separator after the tree id, so
    // one prefix range names exactly the subtree.
    let lower = format!("{}|", list.address);
    let upper = format!("{}}}", list.address);
    let mut statement = conn.prepare_cached(
        "SELECT COALESCE(location, citation_key), card_type, term_name, text FROM cards
         WHERE (location >= ?1 AND location < ?2) OR COALESCE(location, citation_key) = ?3
         ORDER BY COALESCE(location, citation_key)",
    )?;
    let subtree = statement
        .query_map(params![lower, upper, list.address], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .map(|row| {
            let (address, card_type, name, text) = row?;
            let title = crate::parse_card_text(&text)?.title;
            Ok((
                address,
                Subtree {
                    card_type: CardType::from_stored(&card_type),
                    name,
                    title,
                },
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    drop(statement);

    let lookup = |address: &str| -> Result<Option<Stored>> {
        Ok(subtree
            .get(address)
            .map(|card| (card.card_type, card.name.clone())))
    };

    let mut heads: Vec<(String, String)> = Vec::new();
    let mut parts: Vec<(String, String, String)> = Vec::new();
    let mut unfiled: Vec<(String, String)> = Vec::new();
    let mut faults: Vec<(u8, String, Value)> = Vec::new();
    for (address, card) in &subtree {
        if card.card_type != CardType::Term {
            continue;
        }
        let Some(name) = card.name.as_deref() else {
            continue;
        };
        match place(address, name, &lookup)? {
            Some(placed) if placed.list == list.address => match placed.role {
                Role::Head => heads.push((address.clone(), name.to_string())),
                Role::Part => parts.push((address.clone(), name.to_string(), placed.head)),
                _ => {}
            },
            Some(_) => {}
            None => {
                // An unfiled Card is also a continuation whose name differs from the head or part
                // it continues; name that fault when the predecessor is a valid one. Only a
                // Direct successor (a numbered placement) continues anything.
                let numbered = crate::valid_positive_number(crate::last_segment(address));
                let differs = match (numbered, crate::parent_location(address)) {
                    (true, Some(parent)) => match subtree.get(&parent) {
                        Some(predecessor)
                            if predecessor.card_type == CardType::Term
                                && predecessor.name.as_deref().is_some_and(|known| known != name) =>
                        {
                            let known = predecessor.name.clone().unwrap_or_default();
                            match place(&parent, &known, &lookup)? {
                                Some(placed) if placed.list == list.address => {
                                    Some((known, placed.role))
                                }
                                _ => None,
                            }
                        }
                        _ => None,
                    },
                    _ => None,
                };
                match differs {
                    Some((expected, role)) => faults.push((
                        4,
                        address.clone(),
                        json!({
                            "fault": "term_part_name",
                            "address": address,
                            "term": name,
                            "expected": expected,
                            "continues": crate::parent_location(address),
                            "role": role.id(),
                        }),
                    )),
                    None => {
                        unfiled.push((address.clone(), name.to_string()));
                        faults.push((
                            0,
                            address.clone(),
                            json!({"fault": "term_unfiled", "address": address, "term": name}),
                        ));
                    }
                }
            }
        }
    }
    heads.sort_by(|a, b| crate::model::address_order(&a.0, &b.0));
    parts.sort_by(|a, b| crate::model::address_order(&a.0, &b.0));
    unfiled.sort_by(|a, b| crate::model::address_order(&a.0, &b.0));

    // The row each head publishes, by the exact row spelling: no case folding or normalization
    // decides which row a head belongs to (S08-AC06).
    let head_rows = |name: &str| -> Vec<usize> {
        rows.iter()
            .enumerate()
            .filter(|(_, row)| row.term == name)
            .map(|(index, _)| index + 1)
            .collect()
    };
    let mut row_values = Vec::new();
    let mut published = 0usize;
    for (index, row) in rows.iter().enumerate() {
        let matching = heads
            .iter()
            .filter(|(_, name)| name == &row.term)
            .map(|(address, _)| address.clone())
            .collect::<Vec<_>>();
        if matching.is_empty() {
            faults.push((
                2,
                format!("{:010}", index + 1),
                json!({
                    "fault": "term_row_without_card",
                    "rule": "T8",
                    "row": index + 1,
                    "term": row.term,
                }),
            ));
        } else {
            published += 1;
        }
        row_values.push(json!({
            "row": index + 1,
            "term": row.term,
            "avoid": row.avoid,
            "meaning": row.meaning,
            "defined": row.defined,
            "heads": matching,
            "published": !matching.is_empty(),
        }));
    }
    let mut duplicate_terms: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (address, name) in &heads {
        if head_rows(name).is_empty() {
            faults.push((
                3,
                address.clone(),
                json!({
                    "fault": "term_card_without_row",
                    "rule": "T8",
                    "address": address,
                    "term": name,
                }),
            ));
        }
        duplicate_terms
            .entry(name.clone())
            .or_default()
            .push(address.clone());
    }
    for (term, addresses) in &duplicate_terms {
        if addresses.len() > 1 {
            faults.push((
                1,
                term.clone(),
                json!({
                    "fault": "term_duplicate",
                    "rule": "T8",
                    "term": term,
                    "heads": addresses,
                }),
            ));
        }
    }

    // The parts each head continues, and the Term uses of each head.
    let mut head_values = Vec::new();
    let mut head_uses: Vec<(String, Vec<Use>)> = Vec::new();
    for (address, name) in &heads {
        let parts_of = parts
            .iter()
            .filter(|(_, _, head)| head == address)
            .map(|(part, _, _)| part.clone())
            .collect::<Vec<_>>();
        let uses = uses_of(conn, address)?;
        let total = uses.iter().map(|use_row| use_row.count).sum::<u64>();
        head_uses.push((address.clone(), uses));
        head_values.push(json!({
            "address": address,
            "term": name,
            "title": subtree.get(address).map(|card| card.title.clone()).unwrap_or_default(),
            "role": Role::Head.id(),
            "parts": parts_of,
            "uses": total,
        }));
    }
    let part_values = parts
        .iter()
        .map(|(address, name, head)| {
            json!({
                "address": address,
                "term": name,
                "title": subtree.get(address).map(|card| card.title.clone()).unwrap_or_default(),
                "role": Role::Part.id(),
                "head": head,
            })
        })
        .collect::<Vec<_>>();
    let unfiled_values = unfiled
        .iter()
        .map(|(address, name)| {
            json!({
                "address": address,
                "term": name,
                "title": subtree.get(address).map(|card| card.title.clone()).unwrap_or_default(),
                "role": Role::Unfiled.id(),
            })
        })
        .collect::<Vec<_>>();

    // Uses of this list's heads, from anywhere in the archive. A use whose authored label differs
    // from the head's exact name is advice about this list whoever writes it.
    let tree = crate::tree_id(&list.address);
    let mut use_values = Vec::new();
    let mut use_total = 0u64;
    for ((head, name), (_, uses)) in heads.iter().zip(&head_uses) {
        for use_row in uses {
            use_total += use_row.count;
            use_values.push(json!({
                "source": use_row.source,
                "source_title": use_row.source_title,
                "target": use_row.target,
                "label": use_row.label,
                "count": use_row.count,
            }));
            if let Some(label) = use_row.label.as_deref()
                && label != name
            {
                faults.push((
                    5,
                    format!("{} {head}", use_row.source),
                    json!({
                        "fault": "term_label_mismatch",
                        "source": use_row.source,
                        "source_title": use_row.source_title,
                        "target": head,
                        "label": label,
                        "term": name,
                    }),
                ));
            }
        }
    }
    // The Term-link faults of this list's own tree: a Term use whose target is missing or is not a
    // head. A foreign book's bad target is that book's debt, not this list's.
    let mut statement =
        conn.prepare("SELECT source, target FROM links WHERE kind = 'term' ORDER BY source, ord")?;
    let occurrences = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let mut bodies: BTreeMap<String, (String, String)> = BTreeMap::new();
    for (source, target) in occurrences {
        if !crate::is_tree_member(&source, tree) {
            continue;
        }
        if head_of(conn, &target)?.is_some() {
            continue;
        }
        if !bodies.contains_key(&source) {
            let card = crate::load_card(conn, &source)?;
            let parsed = crate::parse_card_text(&card.text)?;
            bodies.insert(source.clone(), (parsed.title, parsed.body));
        }
        let (title, body) = &bodies[&source];
        let label = label_of(body, &target);
        let reason = if stored(conn, &target)?.is_none() {
            "missing"
        } else {
            "not_head"
        };
        faults.push((
            6,
            format!("{source} {target}"),
            json!({
                "fault": "term_link_target",
                "source": source,
                "source_title": title,
                "target": target,
                "label": label,
                "reason": reason,
            }),
        ));
    }

    let duplicates = zt_check::text::term_row_duplicates(&parsed.body)
        .into_iter()
        .map(|group| {
            json!({
                "term": group.term,
                "spellings": group.spellings,
                "rows": group.rows,
            })
        })
        .collect::<Vec<_>>();
    table_shape_faults(&parsed.body, &mut faults);
    faults.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    Ok(json!({
        "list": list.address,
        "title": parsed.title,
        "workspace": list.workspace.id(),
        "tree": tree,
        "rows": row_values,
        "row_duplicates": duplicates,
        "heads": head_values,
        "parts": part_values,
        "unfiled": unfiled_values,
        "uses": use_values,
        "faults": faults.into_iter().map(|(_, _, fault)| fault).collect::<Vec<_>>(),
        "coverage": {
            "rows": rows.len(),
            "raw_rows": raw_rows.len(),
            "published": published,
            "missing": rows.len() - published,
            "heads": heads.len(),
            "parts": parts.len(),
            "unfiled": unfiled.len(),
            "uses": use_total,
        },
        "complete": true,
    }))
}

/// The table-shape faults of one Term list (R0233, T8): a row with more than the four template
/// columns, and an HTML comment in the body other than the generated Reverse marker. The list's
/// four-column table stays the table R6 reads; no fifth column or hidden comment rides in it.
fn table_shape_faults(body: &str, faults: &mut Vec<(u8, String, Value)>) {
    let mut fenced = false;
    let mut data_row = 0usize;
    for (index, raw) in body.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if trimmed.starts_with('|') {
            let cells = trimmed
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            let rule = !cells.is_empty()
                && cells
                    .iter()
                    .all(|cell| !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':'));
            let header = cells.first().is_some_and(|cell| *cell == "术语");
            if !rule && !header && cells.len() > 4 {
                data_row += 1;
                faults.push((
                    7,
                    format!("column:{:010}", index + 1),
                    json!({
                        "fault": "extra_column",
                        "rule": "T8",
                        "row": data_row,
                        "columns": cells.len(),
                    }),
                ));
            } else if !rule && !header {
                data_row += 1;
            }
        }
        // An HTML comment is not authored content; only the generated Reverse marker is known.
        if let Some(start) = raw.find("<!--")
            && let Some(end) = raw[start + 4..].find("-->")
        {
            let comment = raw[start + 4..start + 4 + end].trim();
            if comment != "zt:reverse-links" {
                faults.push((
                    7,
                    format!("comment:{:010}", index + 1),
                    json!({
                        "fault": "hidden_comment",
                        "rule": "T8",
                        "line": index + 1,
                        "comment": comment,
                    }),
                ));
            }
        }
    }
}


/// The stored Term uses of one head, one row per source Card with its occurrence count.
fn uses_of(conn: &Connection, target: &str) -> Result<Vec<Use>> {
    let mut statement = conn.prepare_cached(
        "SELECT l.source, c.text, COUNT(*) FROM links l
         JOIN cards c ON c.location = l.source OR c.citation_key = l.source
         WHERE l.kind = 'term' AND l.target = ?1
         GROUP BY l.source ORDER BY l.source",
    )?;
    let rows = statement
        .query_map([target], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut uses = Vec::new();
    for (source, text, count) in rows {
        let parsed = crate::parse_card_text(&text)?;
        uses.push(Use {
            source,
            source_title: parsed.title,
            target: target.to_string(),
            label: label_of(&parsed.body, target),
            count: count as u64,
        });
    }
    Ok(uses)
}


#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_name_is_trimmed_and_an_empty_one_means_no_name() {
        assert_eq!(normalize_name(" 均衡 ").unwrap().as_deref(), Some("均衡"));
        assert_eq!(normalize_name("a b").unwrap().as_deref(), Some("a b"));
        for empty in ["", "   ", "\t\n "] {
            assert_eq!(normalize_name(empty).unwrap(), None, "{empty:?}");
        }
    }

    #[test]
    fn a_name_is_one_line_of_at_most_forty_scalar_values() {
        let forty = "概".repeat(40);
        assert_eq!(normalize_name(&forty).unwrap().as_deref(), Some(forty.as_str()));
        let forty_one = "概".repeat(41);
        assert_eq!(
            normalize_name(&forty_one).unwrap_err().to_string(),
            "a term name holds at most 40 characters"
        );
        // Counting is by Unicode scalar value, not bytes: 40 two-byte characters still fit.
        assert!(normalize_name(&"é".repeat(40)).is_ok());
        assert!(normalize_name(&"é".repeat(41)).is_err());
        assert_eq!(
            normalize_name("two\nlines").unwrap_err().to_string(),
            "a term name must be one line"
        );
        assert_eq!(
            normalize_name("carriage\rreturn").unwrap_err().to_string(),
            "a term name must be one line"
        );
    }

    #[test]
    fn a_name_holds_no_macro_or_term_link_syntax() {
        for name in ["[均衡]", "a]b", "a@b", "a|b", "a`b"] {
            assert_eq!(
                normalize_name(name).unwrap_err().to_string(),
                "a term name cannot contain `[`, `]`, `@`, `|`, or a backtick",
                "{name}"
            );
        }
        // Other Markdown punctuation is ordinary term text.
        for name in ["(均衡)", "均衡*", "a-b", "a/b", "a_b"] {
            assert!(normalize_name(name).is_ok(), "{name}");
        }
    }

    #[test]
    fn the_client_field_distinguishes_omitted_cleared_and_set() {
        assert_eq!(parse_field(None).unwrap(), None);
        assert_eq!(parse_field(Some(&Value::Null)).unwrap(), Some(None));
        assert_eq!(parse_field(Some(&json!(""))).unwrap(), Some(None));
        assert_eq!(parse_field(Some(&json!("   "))).unwrap(), Some(None));
        assert_eq!(
            parse_field(Some(&json!(" 均衡 "))).unwrap(),
            Some(Some("均衡".to_string()))
        );
        assert_eq!(
            parse_field(Some(&json!(5))).unwrap_err().to_string(),
            "argument `term` must be a string"
        );
    }

    #[test]
    fn a_name_is_only_valid_on_a_term_card() {
        assert!(ensure_on_type(Some("均衡"), CardType::Term).is_ok());
        assert!(ensure_on_type(None, CardType::Idea).is_ok());
        assert!(ensure_on_type(None, CardType::Term).is_ok());
        for card_type in [
            CardType::Idea,
            CardType::Event,
            CardType::Finding,
            CardType::Model,
            CardType::Hub,
        ] {
            assert_eq!(
                ensure_on_type(Some("均衡"), card_type)
                    .unwrap_err()
                    .to_string(),
                NAME_NEEDS_TERM_CARD,
                "{card_type}"
            );
        }
    }

    /// S08-AC01/S08-AC04: the role contract, and the two guards that make a malformed chain a
    /// visible refusal instead of a loop or an unbounded read. Location parsing cannot answer a
    /// cycle, so the guard is proved with the parent function the walk is given.
    #[test]
    fn a_placement_walk_answers_each_role_and_refuses_a_cycle_or_a_too_long_chain() {
        let term = |name: Option<&str>| (CardType::Term, name.map(str::to_string));
        let cards = |rows: &[(&str, Option<&str>)]| {
            let map = rows
                .iter()
                .map(|(at, name)| (at.to_string(), term(*name)))
                .collect::<BTreeMap<_, _>>();
            move |at: &str| Ok(map.get(at).cloned())
        };
        let parent = |at: &str| crate::parent_location(at);

        // `2/1|a` is a list; `2/1|a|a` is its head; `2/1|a|a|1` and `...|2` are its parts.
        let lookup = cards(&[
            ("2/1", Some("均衡")),
            ("2/1|a", None),
            ("2/1|a|a", Some("均衡")),
            ("2/1|a|a|1", Some("均衡")),
            ("2/1|a|a|2", Some("均衡")),
            ("2/1|a|a|a", Some("均衡")),
            ("2/1|a|b", Some("弹性")),
        ]);
        assert_eq!(
            place_with("2/1|a|a", "均衡", &lookup, &parent).unwrap(),
            Some(Placed { role: Role::Head, list: "2/1|a".into(), head: "2/1|a|a".into() })
        );
        for part in ["2/1|a|a|1", "2/1|a|a|2"] {
            assert_eq!(
                place_with(part, "均衡", &lookup, &parent).unwrap(),
                Some(Placed { role: Role::Part, list: "2/1|a".into(), head: "2/1|a|a".into() }),
                "{part}"
            );
        }
        // A Side successor of a head is no placement, and neither is another name.
        assert_eq!(place_with("2/1|a|a|a", "均衡", &lookup, &parent).unwrap(), None);
        assert_eq!(place_with("2/1|a|a|1", "弹性", &lookup, &parent).unwrap(), None);

        // A parent function that answers a cycle is refused, not followed forever.
        let always_named = |_at: &str| Ok(Some((CardType::Term, Some("均衡".to_string()))));
        let cycle = |_at: &str| Some("1".to_string());
        let error = place_with("1", "均衡", &always_named, &cycle).unwrap_err();
        assert_eq!(
            error.downcast_ref::<Placement>().map(|p| p.reason),
            Some("cycle")
        );

        // A chain longer than the bound is refused with the depth reason.
        let deep = |at: &str| Some(format!("{at}|1"));
        let error = place_with("2/1|a|1", "均衡", &always_named, &deep).unwrap_err();
        let placement = error.downcast_ref::<Placement>().expect("a typed placement error");
        assert_eq!(placement.reason, "depth");
        assert!(placement.to_string().contains("longer than 128 steps"));
    }
}
