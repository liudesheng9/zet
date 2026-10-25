//! Model structure: one Model Card and the Model parts that belong to it.
//!
//! A whole model is presented by its Model Card (its entry point: the question
//! and what the model shows) and by Model parts: Model Cards that each carry a
//! Model facet, a short label the author chooses (`setting`, `动态`, `a worked
//! example`). A part may also carry an Emphasis: `core` for the parts a reader
//! must see, `aside` for detail a reader may skip.
//!
//! Following Luhmann, where a Card is filed says nothing about what it belongs
//! to: a part belongs to the Model Card it is filed under, if any, and to every
//! Model Card that links it, so a part may sit wherever it was first written
//! and serve several models. Inside a model the structure is a graph of Links,
//! not the tree its Cards are filed in, and each Link says what it is: the
//! words before it (`依赖`, `例示`, `对照`, `放宽`, `对应`, `反驳`, …) give it a
//! [`RelationKind`]. Links between the model and Cards outside it belong to the
//! structure too. Facets and emphasis are stored on the Card row like the Card
//! type; everything else is computed from Locations, facets, and Links, never
//! stored, so it follows the Cards when they move.

use anyhow::{Result, bail};
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use crate::link::{LinkBody, LinkForm};
use crate::{Card, CardType};

/// Which part of its model a Model part presents: a short, one-line label.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct Facet(String);

/// Facets many formal models use, with what a part of each holds. They are
/// stored in one spelling whatever their case, and are no list to fill: a
/// model has the parts its author names, in no fixed order.
pub(crate) const COMMON_FACETS: [(&str, &str); 8] = [
    ("setting", "agents, assets, actions, and payoffs"),
    (
        "assumptions",
        "what the model assumes, and what each assumption buys",
    ),
    ("timing", "the order of moves and who knows what when"),
    (
        "dynamics",
        "how the state evolves: laws of motion, best responses, feedback",
    ),
    (
        "solution",
        "the equilibrium or optimum and how it is derived",
    ),
    ("results", "propositions and comparative statics"),
    ("intuition", "why the results hold, in words"),
    ("limits", "where the model stops applying, and its critics"),
];

impl Facet {
    /// The longest facet, in characters.
    pub(crate) const MAX_CHARS: usize = 32;

    /// The stored and typed label.
    pub(crate) fn id(&self) -> &str {
        &self.0
    }

    /// A label: trimmed, its inner whitespace collapsed, one line of at most
    /// [`Facet::MAX_CHARS`] characters. A common facet in any case is stored in
    /// lower case.
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let label = value.split_whitespace().collect::<Vec<_>>().join(" ");
        if label.is_empty() {
            bail!("a Model facet cannot be empty");
        }
        if label.chars().count() > Self::MAX_CHARS {
            bail!(
                "Model facet `{label}` is too long; a facet is a short label of at most {} characters",
                Self::MAX_CHARS
            );
        }
        if label.chars().any(char::is_control) || label.contains("[[") || label.contains("]]") {
            bail!("Model facet `{label}` must be plain text on one line, without Links");
        }
        let common = COMMON_FACETS
            .iter()
            .find(|(id, _)| id.eq_ignore_ascii_case(&label))
            .map(|(id, _)| (*id).to_string());
        Ok(Facet(common.unwrap_or(label)))
    }

    /// Read a stored value; an empty or unreadable one is no facet.
    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| Facet::parse(value).ok())
    }
}

impl std::fmt::Display for Facet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// How much of a reader's attention a Model part asks for. A part without an
/// Emphasis is a regular part.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Emphasis {
    /// The heart of the model: what a reader must see first.
    Core,
    /// Detail a reader may skip.
    Aside,
}

impl Emphasis {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Emphasis::Core => "core",
            Emphasis::Aside => "aside",
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "core" => Ok(Emphasis::Core),
            "aside" => Ok(Emphasis::Aside),
            other => bail!("unknown emphasis `{other}`; expected core, aside, or empty for none"),
        }
    }

    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| Emphasis::parse(value).ok())
    }
}

impl std::fmt::Display for Emphasis {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// How fully a model is elaborated. Only a Model Card (a model's entry point,
/// without a facet) has a level; a Model Card without one has not been
/// classified, and its build report applies the formal rules.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum Level {
    /// Level 1: a concept model. The Model Card and a few parts state the
    /// question, the mechanism, and the result in words; the model has no
    /// Derivation parts.
    Concept,
    /// Level 2: a formal model. A self-sufficient model built by the full
    /// model build: a derivation ledger, the main line in build order, and
    /// Derivation parts for every step the book skips.
    Formal,
}

impl Level {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Level::Concept => "concept",
            Level::Formal => "formal",
        }
    }

    /// The level's number: 1 for a concept model, 2 for a formal model.
    pub(crate) fn number(self) -> u8 {
        match self {
            Level::Concept => 1,
            Level::Formal => 2,
        }
    }

    pub(crate) fn parse(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "concept" | "1" | "l1" | "概念" | "概念模型" => Ok(Level::Concept),
            "formal" | "2" | "l2" | "形式" | "形式模型" | "形式化" => Ok(Level::Formal),
            other => bail!(
                "unknown elaboration level `{other}`; expected concept (1), formal (2), or empty for none"
            ),
        }
    }

    pub(crate) fn from_stored(value: Option<&str>) -> Option<Self> {
        value.and_then(|value| Level::parse(value).ok())
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// Parse an optional level argument: empty or `none` clears it.
pub(crate) fn parse_optional_level(value: &str) -> Result<Option<Level>> {
    if clears(value) {
        Ok(None)
    } else {
        Level::parse(value).map(Some)
    }
}

pub(crate) const LEVEL_NEEDS_MODEL_CARD: &str =
    "only a Model Card (a whole model, without a facet) has an elaboration level";

/// Store a Model Card's elaboration level; `None` clears it.
pub(crate) fn set_level(conn: &Connection, card: &Card, level: Option<Level>) -> Result<()> {
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
    if level.is_some() && (card.card_type != CardType::Model || card.facet.is_some()) {
        bail!(LEVEL_NEEDS_MODEL_CARD);
    }
    conn.execute(
        "UPDATE cards SET model_level = ?1 WHERE row_id = ?2",
        params![level.map(Level::id), card.row_id],
    )?;
    Ok(())
}

/// What makes a new Card a Model part: its facet and, if any, its emphasis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Part {
    pub(crate) facet: Facet,
    pub(crate) emphasis: Option<Emphasis>,
}

impl Part {
    /// A part from optional arguments; an emphasis without a facet is refused.
    pub(crate) fn from_args(
        facet: Option<Facet>,
        emphasis: Option<Emphasis>,
    ) -> Result<Option<Self>> {
        match (facet, emphasis) {
            (Some(facet), emphasis) => Ok(Some(Part { facet, emphasis })),
            (None, Some(_)) => bail!(EMPHASIS_NEEDS_FACET),
            (None, None) => Ok(None),
        }
    }
}

const EMPHASIS_NEEDS_FACET: &str =
    "only Model parts can have an emphasis; give the Card a Model facet first";

fn clears(value: &str) -> bool {
    let value = value.trim();
    value.is_empty() || value.eq_ignore_ascii_case("none")
}

/// Parse an optional facet argument: an empty string or `none` clears the facet.
pub(crate) fn parse_optional(value: &str) -> Result<Option<Facet>> {
    if clears(value) {
        Ok(None)
    } else {
        Facet::parse(value).map(Some)
    }
}

/// Parse an optional emphasis argument: empty, `none`, or `regular` clears it.
pub(crate) fn parse_optional_emphasis(value: &str) -> Result<Option<Emphasis>> {
    if clears(value) || value.trim().eq_ignore_ascii_case("regular") {
        Ok(None)
    } else {
        Emphasis::parse(value).map(Some)
    }
}

/// The Model Card a Card filed at `start` sits under: walk up through Model
/// parts until a Model Card without a facet. `lookup` tells whether the Card at
/// an address is a Model Card and whether it has a facet. Filing is one way a
/// part joins a model; a Link from a Model Card is the other (see [`heads`]).
pub(crate) fn find_head(
    start: Option<String>,
    lookup: impl Fn(&str) -> Option<(CardType, bool)>,
) -> Option<String> {
    let mut at = start;
    while let Some(address) = at {
        match lookup(&address)? {
            (CardType::Model, false) => return Some(address),
            (CardType::Model, true) => at = crate::parent_location(&address),
            _ => return None,
        }
    }
    None
}

fn db_lookup(conn: &Connection) -> impl Fn(&str) -> Option<(CardType, bool)> + '_ {
    |address| {
        crate::load_card(conn, address)
            .ok()
            .map(|card| (card.card_type, card.facet.is_some()))
    }
}

/// Check the type, facet, and emphasis a Card would have. Only Model Cards have
/// a facet and only parts have an emphasis. A part may be filed anywhere: it
/// belongs to the Model Card it is filed under, if any, and to every Model Card
/// that links it. A Model Card with parts cannot stop being their Model Card.
pub(crate) fn ensure_allowed(
    conn: &Connection,
    current: Option<&Card>,
    card_type: CardType,
    facet: Option<&Facet>,
    emphasis: Option<Emphasis>,
) -> Result<()> {
    if facet.is_some() {
        if card_type != CardType::Model {
            bail!("only Model Cards can have a Model facet");
        }
    } else if emphasis.is_some() {
        bail!(EMPHASIS_NEEDS_FACET);
    }
    if let Some(card) = current
        && card.card_type == CardType::Model
        && card.facet.is_none()
        && (card_type != CardType::Model || facet.is_some())
    {
        let parts = parts_in_db(conn, card)?;
        if !parts.is_empty() {
            bail!(
                "`{}` is the Model Card of {} Model part(s) ({}); change or move them first",
                card.address,
                parts.len(),
                parts.join(", ")
            );
        }
    }
    Ok(())
}

/// The parts of the Model Card `head`: those filed under it and those it links.
fn parts_in_db(conn: &Connection, head: &Card) -> Result<Vec<String>> {
    let linked = crate::parse_card_text(&head.text)
        .ok()
        .and_then(|parsed| {
            LinkBody::parse(&parsed.body).ok().map(|body| {
                body.occurrences()
                    .iter()
                    .map(|occurrence| occurrence.target.to_string())
                    .collect::<BTreeSet<_>>()
            })
        })
        .unwrap_or_default();
    let cards = crate::load_cards(conn)?;
    let lookup = db_lookup(conn);
    Ok(cards
        .iter()
        .filter(|card| card.card_type == CardType::Model && card.facet.is_some())
        .filter(|card| {
            linked.contains(&card.address)
                || find_head(crate::parent_location(&card.address), &lookup).as_deref()
                    == Some(head.address.as_str())
        })
        .map(|card| card.address.clone())
        .collect())
}

/// Store a Card's type, facet, and emphasis. A Card that stops being a Model
/// Card (it changes type or becomes a part) loses its elaboration level, and a
/// Card that stops being a Term Card loses its term name in the same statement,
/// so the table's own check never sees a name on another type.
pub(crate) fn set(
    conn: &Connection,
    card: &Card,
    card_type: CardType,
    facet: Option<&Facet>,
    emphasis: Option<Emphasis>,
) -> Result<()> {
    crate::tree_lock::ensure_unlocked(conn, crate::tree_lock::Change::Edit(card.row_id))?;
    crate::card_type::ensure_allowed(card, card_type)?;
    ensure_allowed(conn, Some(card), card_type, facet, emphasis)?;
    conn.execute(
        "UPDATE cards SET card_type = ?1, model_facet = ?2, model_emphasis = ?3,
             model_level = CASE WHEN ?1 = 'model' AND ?2 IS NULL THEN model_level END,
             math_form = CASE WHEN ?1 = 'model' AND ?2 IS NULL THEN math_form END,
             hub_level = CASE WHEN ?1 = 'hub' THEN hub_level END,
             term_name = CASE WHEN ?1 = 'term' THEN term_name END
         WHERE row_id = ?4",
        params![
            card_type.id(),
            facet.map(Facet::id),
            emphasis.map(Emphasis::id),
            card.row_id
        ],
    )?;
    Ok(())
}

/// Folgezettel order: numbers by value, side letters by length then
/// alphabet, so `3/2|b` comes before `3/2|aa` and `3/10` after `3/9`.
pub(crate) fn address_order(a: &str, b: &str) -> Ordering {
    fn segments(address: &str) -> Vec<(u8, u64, &str)> {
        address
            .split(['/', '|'])
            .map(|segment| match segment.parse::<u64>() {
                Ok(number) => (0, number, ""),
                Err(_) => (1, segment.len() as u64, segment),
            })
            .collect()
    }
    segments(a).cmp(&segments(b)).then_with(|| a.cmp(b))
}

/// What a Link says about the Card it names. ZT reads it from the words its
/// author wrote before the Link in the same sentence (`依赖设定 [[…]]`,
/// `relaxes [[…]]`), so the reason a Link gives is also its kind. A Link with
/// no such word is a plain reference; a Link that follows another in the same
/// sentence with no word of its own (`依赖 [[a]] 与 [[b]]`) shares its kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) enum RelationKind {
    /// The linking Card rests on the linked one: a premise or a derivation step.
    RestsOn,
    /// An example, case, or numerical illustration.
    Illustrates,
    /// A comparison or a difference.
    Contrasts,
    /// Relaxes, extends, or generalizes the linked Card.
    Relaxes,
    /// A correspondence, analogy, or equivalence.
    Corresponds,
    /// An objection, critique, or counterexample.
    Objects,
    /// A special case: every object of the linking Card is one of the linked Card.
    Specializes,
    /// A plain reference.
    Refers,
}

/// Each announced kind with what it means and the words that announce it.
/// English words match at the start of a word, so `relax` covers `relaxes`.
/// The last row names words that announce a plain reference, so a Link after
/// them does not share the kind of a Link before them in the sentence.
pub(crate) const RELATION_KINDS: [(RelationKind, &str, &[&str]); 8] = [
    (
        RelationKind::RestsOn,
        "rests on the linked Card: a premise or a derivation step",
        &[
            "依赖",
            "依据",
            "基于",
            "建立在",
            "取自",
            "沿用",
            "承接",
            "用到",
            "借用",
            "代入",
            "推导自",
            "由此推出",
            "按定义",
            "substitut",
            "rests on",
            "builds on",
            "depends on",
            "relies on",
            "derived from",
            "follows from",
            "uses",
            "cites",
            "by definition",
            "by theorem",
            "by lemma",
            "by proposition",
            "by corollary",
            "by axiom",
        ],
    ),
    (
        RelationKind::Illustrates,
        "an example, case, or numerical illustration of it",
        &[
            "例示",
            "举例",
            "例子",
            "示例",
            "演示",
            "例证",
            "实例",
            "案例",
            "印证",
            "illustrat",
            "example of",
            "instance of",
            "for example",
            "e.g.",
        ],
    ),
    (
        RelationKind::Contrasts,
        "a comparison with it, or how it differs",
        &[
            "对照",
            "对比",
            "相比",
            "不同于",
            "区别",
            "相反",
            "逆命题",
            "contrast",
            "unlike",
            "compared with",
            "compare",
            "versus",
            "differs from",
        ],
    ),
    (
        RelationKind::Relaxes,
        "relaxes, extends, or generalizes it",
        &[
            "放宽",
            "放松",
            "推广",
            "扩展",
            "拓展",
            "一般化",
            "relax",
            "extend",
            "generaliz",
        ],
    ),
    (
        RelationKind::Corresponds,
        "a correspondence, analogy, or equivalence with it",
        &[
            "对应",
            "类比",
            "同构",
            "相当于",
            "等价",
            "参照",
            "correspond",
            "maps onto",
            "analog",
            "equivalent",
            "mirrors",
        ],
    ),
    (
        RelationKind::Objects,
        "an objection, critique, or counterexample to it",
        &[
            "反驳",
            "批评",
            "质疑",
            "反例",
            "否定",
            "挑战",
            "objection",
            "objects to",
            "critici",
            "refut",
            "counterexample",
            "challeng",
        ],
    ),
    (
        RelationKind::Specializes,
        "a special case of it: every object of this Card is an object of the linked one",
        &["特例", "特殊情形", "special case", "specializ"],
    ),
    (
        RelationKind::Refers,
        "a plain reference: 引出 points the other way (the linked Card builds on this one), and that Card's 承接 carries the relation",
        &["引出"],
    ),
];

impl RelationKind {
    pub(crate) fn id(self) -> &'static str {
        match self {
            RelationKind::RestsOn => "rests-on",
            RelationKind::Illustrates => "illustrates",
            RelationKind::Contrasts => "contrasts",
            RelationKind::Relaxes => "relaxes",
            RelationKind::Corresponds => "corresponds",
            RelationKind::Objects => "objects",
            RelationKind::Specializes => "specializes",
            RelationKind::Refers => "refers",
        }
    }

    /// The kind announced last in `words`, the text just before a Link.
    fn announced_in(words: &str) -> Option<Self> {
        let lower = words.to_lowercase();
        let mut best: Option<(usize, usize, RelationKind)> = None;
        for (kind, _, cues) in RELATION_KINDS {
            for cue in cues {
                let word = cue.starts_with(|ch: char| ch.is_ascii_alphabetic());
                for (at, _) in lower.match_indices(cue) {
                    let inside_word = lower[..at]
                        .chars()
                        .next_back()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric());
                    if word && inside_word {
                        continue;
                    }
                    let found = (at + cue.len(), cue.len(), kind);
                    if best.is_none_or(|(end, len, _)| (found.0, found.1) > (end, len)) {
                        best = Some(found);
                    }
                }
            }
        }
        best.map(|(_, _, kind)| kind)
    }
}

impl std::fmt::Display for RelationKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.id())
    }
}

/// A sentence ends at CJK or ASCII end punctuation, a line break, or a period
/// followed by a space.
fn ends_sentence(body: &str, index: usize, ch: char) -> bool {
    ['。', '！', '？', '!', '?', '；', ';', '\n'].contains(&ch)
        || (ch == '.' && body[index + 1..].starts_with(char::is_whitespace))
}

/// Where the sentence holding byte `at` of `body` starts.
fn sentence_start(body: &str, at: usize) -> usize {
    body[..at]
        .char_indices()
        .rev()
        .find(|&(index, ch)| ends_sentence(body, index, ch))
        .map_or(0, |(index, ch)| index + ch.len_utf8())
}

/// The sentence of `body` around the byte range `at`, with every Link in it
/// replaced by `name(target)`, clipped for a tooltip.
fn sentence_around(
    body: &str,
    at: std::ops::Range<usize>,
    name: impl Fn(&str) -> String,
) -> String {
    let start = sentence_start(body, at.start);
    let end = body[at.end..]
        .char_indices()
        .find(|&(index, ch)| ends_sentence(body, at.end + index, ch))
        .map_or(body.len(), |(index, ch)| {
            at.end + index + if ch == '\n' { 0 } else { ch.len_utf8() }
        });
    let sentence = &body[start..end];
    // The shared lexical scan names each macro's target, so a Term use shows its head's address
    // rather than its authored `label@address`, and a name in a code span is no Link.
    let mut text = String::new();
    let mut offset = 0;
    for occurrence in crate::link::LinkBody::for_rendering(sentence).occurrences() {
        text.push_str(&sentence[offset..occurrence.macro_range.start]);
        text.push('「');
        text.push_str(&name(occurrence.target));
        text.push('」');
        offset = occurrence.macro_range.end;
    }
    text.push_str(&sentence[offset..]);
    let text = text
        .trim()
        .trim_start_matches(['-', '*', '>', '#', ' '])
        .replace("**", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = text.chars();
    let clipped: String = chars.by_ref().take(140).collect();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

/// One Link of a body, read: its target, its kind, the sentence that holds
/// it, and whether it is an ordinary Link or a Term use. A Term use names its
/// Term head directly and takes part in no reasoning kind (design 10.2).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LinkRead {
    pub(crate) target: String,
    pub(crate) kind: RelationKind,
    pub(crate) note: String,
    pub(crate) form: LinkForm,
}

/// Every Link of `body`, in order, read with its kind and sentence.
pub(crate) fn read_links(body: &str, name: impl Fn(&str) -> String) -> Vec<LinkRead> {
    let Ok(parsed) = LinkBody::parse(body) else {
        return Vec::new();
    };
    let mut previous: Option<(usize, RelationKind)> = None;
    parsed
        .occurrences()
        .iter()
        .map(|occurrence| {
            let range = occurrence.macro_range.clone();
            let note = sentence_around(body, range.clone(), &name);
            if occurrence.form == LinkForm::Term {
                // A Term use is no announced relation and never changes the ordinary Link state
                // a following ordinary Link may share.
                return LinkRead {
                    target: occurrence.target.to_string(),
                    kind: RelationKind::Refers,
                    note,
                    form: LinkForm::Term,
                };
            }
            let start = sentence_start(body, range.start);
            // After a Link earlier in the same sentence, only the words since it count.
            let shared = previous.filter(|&(end, _)| end > start);
            let from = shared.map_or(start, |(end, _)| end);
            let kind = RelationKind::announced_in(&body[from..range.start])
                .or(shared.map(|(_, kind)| kind))
                .unwrap_or(RelationKind::Refers);
            previous = Some((range.end, kind));
            LinkRead {
                target: occurrence.target.to_string(),
                kind,
                note,
                form: LinkForm::Ordinary,
            }
        })
        .collect()
}

/// Every model a Card belongs to, by Model Card address. A Model Card is its
/// own model. A Model part belongs to the Model Card it is filed under, if
/// any, then to every other Model Card that links it, in Folgezettel order.
/// Any other Card belongs to no model.
pub(crate) fn heads(
    address: &str,
    by_address: &BTreeMap<String, Card>,
    inbound: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<String> {
    let Some(card) = by_address.get(address) else {
        return Vec::new();
    };
    if card.card_type != CardType::Model {
        return Vec::new();
    }
    if card.facet.is_none() {
        return vec![address.to_string()];
    }
    let lookup = |at: &str| {
        by_address
            .get(at)
            .map(|card| (card.card_type, card.facet.is_some()))
    };
    let filed = find_head(crate::parent_location(address), lookup);
    let mut linking = inbound
        .get(address)
        .into_iter()
        .flatten()
        .filter(|source| filed.as_ref() != Some(*source))
        .filter(|source| {
            by_address
                .get(*source)
                .is_some_and(|card| card.card_type == CardType::Model && card.facet.is_none())
        })
        .cloned()
        .collect::<Vec<_>>();
    linking.sort_by(|a, b| address_order(a, b));
    filed.into_iter().chain(linking).collect()
}

/// The parts of the model whose Model Card is `head`: first the parts it
/// links, in the order it names them, then the parts filed under it that it
/// does not name, in Folgezettel order.
pub(crate) fn parts_of<'a>(
    head: &str,
    by_address: &'a BTreeMap<String, Card>,
    parsed: &BTreeMap<String, crate::ParsedCard>,
) -> Vec<&'a Card> {
    let is_part = |card: &Card| card.card_type == CardType::Model && card.facet.is_some();
    let mut parts = Vec::new();
    let mut named = BTreeSet::new();
    let body = parsed.get(head).map_or("", |parsed| parsed.body.as_str());
    if let Ok(links) = LinkBody::parse(body) {
        for occurrence in links.occurrences() {
            if let Some(card) = by_address.get(occurrence.target)
                && is_part(card)
                && named.insert(card.address.as_str())
            {
                parts.push(card);
            }
        }
    }
    let lookup = |at: &str| {
        by_address
            .get(at)
            .map(|card| (card.card_type, card.facet.is_some()))
    };
    let tree = crate::tree_id(head);
    let mut filed = by_address
        .values()
        .filter(|card| is_part(card) && !named.contains(card.address.as_str()))
        .filter(|card| crate::is_tree_member(&card.address, tree))
        .filter(|card| {
            find_head(crate::parent_location(&card.address), lookup).as_deref() == Some(head)
        })
        .collect::<Vec<_>>();
    filed.sort_by(|a, b| address_order(&a.address, &b.address));
    parts.extend(filed);
    parts
}

/// The structure of the model a Card belongs to, computed over a set of
/// Cards. `None` when the Card is neither a Model Card nor a Model part. A
/// part in several models is shown in the first of them; `models` lists all.
///
/// `relations` lists every Link from one part to another, once per pair, with
/// its kind and the sentence that makes it; `loose` lists the parts in no
/// relation. `neighbours` lists the Links between the model's Cards and Cards
/// outside it (tree roots aside), either way, with their kind and sentence:
/// the model's place in the rest of the slip-box.
pub(crate) fn structure(
    address: &str,
    by_address: &BTreeMap<String, Card>,
    parsed: &BTreeMap<String, crate::ParsedCard>,
    inbound: &BTreeMap<String, BTreeSet<String>>,
) -> Option<Value> {
    let card = by_address.get(address)?;
    if card.card_type != CardType::Model {
        return None;
    }
    let title = |at: &str| {
        parsed
            .get(at)
            .map(|parsed| parsed.title.clone())
            .unwrap_or_default()
    };
    let body_of = |at: &str| {
        parsed
            .get(at)
            .map(|parsed| parsed.body.as_str())
            .unwrap_or_default()
    };
    let reference = |at: &str| json!({ "address": at, "title": title(at) });
    let models = heads(address, by_address, inbound);
    let Some(head) = models.first().cloned() else {
        return Some(json!({
            "head": null,
            "level": null,
            "form": null,
            "math": null,
            "models": [],
            "facet": card.facet.as_ref().map(Facet::id),
            "emphasis": card.emphasis.map(Emphasis::id),
            "parts": [],
            "relations": [],
            "loose": [],
            "neighbours": [],
            "discussion": 0,
            "issues": [format!(
                "`{}` is a Model part ({}) but belongs to no model: file it under a Model Card, or link it from one",
                card.address,
                card.facet.as_ref().map(Facet::id).unwrap_or_default()
            )],
        }));
    };
    let parts = parts_of(&head, by_address, parsed);
    let facet_of = parts
        .iter()
        .map(|part| {
            (
                part.address.as_str(),
                part.facet.as_ref().map(Facet::id).unwrap_or_default(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let members = std::iter::once(head.clone())
        .chain(parts.iter().map(|part| part.address.clone()))
        .collect::<BTreeSet<_>>();
    let name = |at: &str| {
        facet_of.get(at).map_or_else(
            || {
                if at == head {
                    "模型".to_string()
                } else {
                    at.to_string()
                }
            },
            |facet| (*facet).to_string(),
        )
    };
    // Outside Cards worth showing beside the model: not its own, not a tree root.
    let outside = |at: &str| {
        !members.contains(at)
            && by_address
                .get(at)
                .is_some_and(|card| !crate::is_tree_root(card))
    };
    let neighbour = |way: &str, from: &str, to: &str, read: &LinkRead| {
        let other = if way == "in" { from } else { to };
        let card = &by_address[other];
        json!({
            "way": way,
            "from": from,
            "to": to,
            "kind": read.kind.id(),
            "note": read.note,
            "card": {
                "address": other,
                "title": title(other),
                "type": card.card_type.id(),
                "facet": card.facet.as_ref().map(Facet::id),
            },
        })
    };
    let mut relations = Vec::new();
    let mut related = BTreeSet::new();
    let mut neighbours = Vec::new();
    let mut met = BTreeSet::new();
    let head_reads = read_links(body_of(&head), name);
    for read in &head_reads {
        if outside(&read.target) && met.insert((head.clone(), read.target.clone())) {
            neighbours.push(neighbour("out", &head, &read.target, read));
        }
    }
    let mut part_values = Vec::new();
    for part in &parts {
        let reads = read_links(body_of(&part.address), name);
        let mut seen = BTreeSet::new();
        for read in &reads {
            if read.target == part.address {
                continue;
            }
            if facet_of.contains_key(read.target.as_str()) {
                if seen.insert(read.target.as_str()) {
                    related.insert(part.address.clone());
                    related.insert(read.target.clone());
                    relations.push(json!({
                        "from": part.address,
                        "to": read.target,
                        "kind": read.kind.id(),
                        "note": read.note,
                    }));
                }
            } else if outside(&read.target)
                && met.insert((part.address.clone(), read.target.clone()))
            {
                neighbours.push(neighbour("out", &part.address, &read.target, read));
            }
        }
        let filed_under = find_head(crate::parent_location(&part.address), |at| {
            by_address
                .get(at)
                .map(|card| (card.card_type, card.facet.is_some()))
        });
        part_values.push(json!({
            "address": part.address,
            "title": title(&part.address),
            "facet": part.facet.as_ref().map(Facet::id),
            "emphasis": part.emphasis.map(Emphasis::id),
            "filed": filed_under.as_deref() == Some(head.as_str()),
            "also": heads(&part.address, by_address, inbound)
                .iter()
                .filter(|other| **other != head)
                .map(|other| reference(other))
                .collect::<Vec<_>>(),
            "links": reads
                .iter()
                .map(|read| read.target.as_str())
                .filter(|target| members.contains(*target) && *target != part.address)
                .collect::<BTreeSet<_>>(),
        }));
    }
    // Links into the model, in the words of the Cards that write them.
    for member in std::iter::once(&head).chain(parts.iter().map(|part| &part.address)) {
        for source in inbound.get(member).into_iter().flatten() {
            if !outside(source) {
                continue;
            }
            let reads = read_links(body_of(source), name);
            if let Some(read) = reads.iter().find(|read| read.target == *member)
                && met.insert((source.clone(), member.clone()))
            {
                neighbours.push(neighbour("in", source, member, read));
            }
        }
    }
    let loose = parts
        .iter()
        .filter(|part| !related.contains(&part.address))
        .map(|part| part.address.clone())
        .collect::<Vec<_>>();
    // Cards outside the model that link any of its Cards: its discussion.
    let discussion = members
        .iter()
        .flat_map(|member| inbound.get(member).into_iter().flatten())
        .filter(|source| !members.contains(*source))
        .collect::<BTreeSet<_>>();
    let level = by_address[&head].level;
    let form = by_address[&head].form;
    let build = build_report(&head, level, &parts, by_address, parsed, inbound, &name);
    let view = crate::math::View {
        by_address,
        parsed,
        inbound,
    };
    let math = match form {
        Some(crate::math::MathForm::Theory) => crate::math::theory_report(&view, &head),
        Some(form) => crate::math::unit_report(
            &view,
            &head,
            form,
            level,
            build["order"].as_array().map_or(&[][..], Vec::as_slice),
        ),
        None => Value::Null,
    };
    Some(json!({
        "head": reference(&head),
        "level": level.map(Level::id),
        "form": form.map(crate::math::MathForm::id),
        "math": math,
        "models": models.iter().map(|at| reference(at)).collect::<Vec<_>>(),
        "facet": card.facet.as_ref().map(Facet::id),
        "emphasis": card.emphasis.map(Emphasis::id),
        "head_links": head_reads
            .iter()
            .map(|read| read.target.as_str())
            .filter(|target| members.contains(*target) && *target != head)
            .collect::<BTreeSet<_>>(),
        "parts": part_values,
        "relations": relations,
        "loose": loose,
        "neighbours": neighbours,
        "discussion": discussion.len(),
        "issues": [],
        "build": build,
    }))
}

/// Whether a facet names a Derivation part: one step of a derivation.
pub(crate) fn is_derivation(facet: &Facet) -> bool {
    let id = facet.id();
    id.starts_with("推导") || id.to_lowercase().starts_with("derivation")
}

/// Put the shared parts that `at` rests on into `out`, each after the shared
/// parts it rests on itself (a depth-first walk over rests-on Links among
/// shared parts, in address order). `placed` holds what is already in the
/// build order, so each part comes once and a cycle ends the walk.
fn take_in_shared(
    at: &str,
    premises: &BTreeMap<String, BTreeSet<String>>,
    shared: &BTreeSet<String>,
    placed: &mut BTreeSet<String>,
    out: &mut Vec<String>,
) {
    for premise in premises.get(at).into_iter().flatten() {
        if shared.contains(premise) && placed.insert(premise.clone()) {
            take_in_shared(premise, premises, shared, placed, out);
            out.push(premise.clone());
        }
    }
}

/// How a Card outside a model that the model rests on can come in (rule 5 of
/// the self-sufficient model): the fix a closure fault names.
fn closure_fix(target: &Card, head: &str, target_models: &[String]) -> String {
    let at = &target.address;
    if target.card_type == CardType::Model && target.facet.is_none() {
        format!(
            "rests on the Model Card `{at}`, which is not in this model: a model rests on parts, not on a whole model; name the parts the step uses, take them in, and rest on them"
        )
    } else if let Some(home) = target_models.first() {
        format!(
            "rests on `{at}`, which is not in this model: it is a part of `{home}`; take it in with a Link from this Model Card's 结构 list"
        )
    } else if target.workspace == crate::Workspace::Agent
        && crate::tree_id(at) == crate::tree_id(head)
    {
        format!(
            "rests on `{at}`, which is not in this model: move it into the range at its build position (move_card with `before`) and give it a facet"
        )
    } else {
        format!(
            "rests on `{at}`, which is not in this model: restate it in a part of this model, and keep `{at}` as a neighbour"
        )
    }
}

/// The build report of the model whose Model Card is `head`: its members in
/// build order, and the faults that keep a reader from building it from
/// nothing. ZT computes it from Locations, facets, and Links; it stores
/// nothing and enforces nothing.
///
/// - **Range**: the Model Card and the parts filed under it.
/// - **Main line**: the first Side successor of the Model Card (the setting)
///   and its Direct successors in the range. A part below a main-line part
///   through a Side successor hangs from it (`side`); any other part in the
///   range stands outside the build (`off`). A shared part (a member filed
///   outside the range) stands just before the first main-line step that
///   rests on it, directly or through another shared part, and after the
///   shared parts it rests on itself.
/// - **Chains**: runs of Derivation parts next to each other in the build
///   order, with the members just before and after each run. One step that is
///   too long for one Card takes a chain of Derivation parts.
/// - **Faults**: `closure` (a rests-on Link from the range to a Card that is
///   not a member), `forward` (a main-line part rests on a later member), and
///   `unanchored` (a main-line part with no rests-on path back to the
///   setting). `tolerated` lists the premises of shared parts that stay in
///   their home model; they are not faults.
/// - **Level**: a concept model (level 1) needs no Derivation parts; each one
///   in its range is listed in `beyond_level`. A formal model (level 2) and a
///   model with no level take the rules above and nothing more.
fn build_report(
    head: &str,
    level: Option<Level>,
    parts: &[&Card],
    by_address: &BTreeMap<String, Card>,
    parsed: &BTreeMap<String, crate::ParsedCard>,
    inbound: &BTreeMap<String, BTreeSet<String>>,
    name: &impl Fn(&str) -> String,
) -> Value {
    let lookup = |at: &str| {
        by_address
            .get(at)
            .map(|card| (card.card_type, card.facet.is_some()))
    };
    let title = |at: &str| {
        parsed
            .get(at)
            .map(|parsed| parsed.title.clone())
            .unwrap_or_default()
    };
    let in_range = parts
        .iter()
        .filter(|part| {
            find_head(crate::parent_location(&part.address), lookup).as_deref() == Some(head)
        })
        .map(|part| part.address.clone())
        .collect::<BTreeSet<_>>();
    let members = std::iter::once(head.to_string())
        .chain(parts.iter().map(|part| part.address.clone()))
        .collect::<BTreeSet<_>>();
    let rests_on = |at: &str| -> Vec<LinkRead> {
        let body = parsed.get(at).map_or("", |parsed| parsed.body.as_str());
        read_links(body, name)
            .into_iter()
            .filter(|read| read.kind == RelationKind::RestsOn && read.target != at)
            .collect()
    };

    // The main line: the first Side successor of the Model Card, then its Direct successors.
    let setting = in_range
        .iter()
        .filter_map(|at| {
            crate::immediate_side_label(at, head)
                .filter(|label| *at == format!("{head}|{label}"))
                .map(|label| (crate::side_label_to_number(&label), at.clone()))
        })
        .min()
        .map(|(_, at)| at);
    let mut main = Vec::new();
    let mut next = setting.clone();
    while let Some(at) = next.filter(|at| in_range.contains(at)) {
        next = crate::direct_successor(&at).ok();
        main.push(at);
    }
    let main_set = main.iter().cloned().collect::<BTreeSet<_>>();
    // Every other part of the range hangs from the main-line part above it, or stands off.
    let mut sides: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut off = Vec::new();
    for at in &in_range {
        if main_set.contains(at) {
            continue;
        }
        let mut up = crate::parent_location(at);
        let mut hangs = None;
        while let Some(above) = up {
            if main_set.contains(&above) {
                hangs = Some(above);
                break;
            }
            if above == head {
                break;
            }
            up = crate::parent_location(&above);
        }
        match hangs {
            Some(from) => sides.entry(from).or_default().push(at.clone()),
            None => off.push(at.clone()),
        }
    }
    for list in sides.values_mut() {
        list.sort_by(|a, b| address_order(a, b));
    }
    off.sort_by(|a, b| address_order(a, b));

    // Each shared part stands before the first main-line step that needs it, directly or
    // through another shared part, and after every shared part it rests on: a shared chain
    // (a building block taken in from its home model, such as a derivation) keeps its order.
    let shared = parts
        .iter()
        .filter(|part| !in_range.contains(&part.address))
        .map(|part| part.address.clone())
        .collect::<Vec<_>>();
    let shared_set = shared.iter().cloned().collect::<BTreeSet<_>>();
    let premises = members
        .iter()
        .map(|at| {
            (
                at.clone(),
                rests_on(at)
                    .into_iter()
                    .map(|read| read.target)
                    .collect::<BTreeSet<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut placed = BTreeSet::new();
    let mut before: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for step in &main {
        let mut list = Vec::new();
        for at in std::iter::once(step).chain(sides.get(step).into_iter().flatten()) {
            take_in_shared(at, &premises, &shared_set, &mut placed, &mut list);
        }
        if !list.is_empty() {
            before.insert(step.clone(), list);
        }
    }
    // Shared parts that no step needs come after the main line, premises first.
    let mut unplaced = Vec::new();
    for part in &shared {
        if placed.insert(part.clone()) {
            take_in_shared(part, &premises, &shared_set, &mut placed, &mut unplaced);
            unplaced.push(part.clone());
        }
    }

    let mut order: Vec<(String, &'static str, Option<String>)> = Vec::new();
    for step in &main {
        for part in before.get(step).into_iter().flatten() {
            order.push((part.clone(), "shared", None));
        }
        order.push((step.clone(), "main", None));
        for part in sides.get(step).into_iter().flatten() {
            order.push((part.clone(), "side", Some(step.clone())));
        }
    }
    for part in unplaced {
        order.push((part, "shared", None));
    }
    for part in off {
        order.push((part, "off", None));
    }
    let position = order
        .iter()
        .enumerate()
        .map(|(at, (address, _, _))| (address.clone(), at))
        .collect::<BTreeMap<_, _>>();

    let homes = |at: &str| heads(at, by_address, inbound);
    let card_ref = |at: &str| {
        let card = &by_address[at];
        json!({
            "address": at,
            "title": title(at),
            "type": card.card_type.id(),
            "facet": card.facet.as_ref().map(Facet::id),
            "models": homes(at),
        })
    };
    let order_values = order
        .iter()
        .map(|(at, line, from)| {
            let card = &by_address[at];
            let mut value = json!({
                "address": at,
                "title": title(at),
                "facet": card.facet.as_ref().map(Facet::id),
                "emphasis": card.emphasis.map(Emphasis::id),
                "line": line,
            });
            if let Some(from) = from {
                value["from"] = json!(from);
            }
            if *line == "shared" {
                value["home"] = json!(homes(at).into_iter().find(|model| model != head));
            }
            if card.facet.as_ref().is_some_and(is_derivation) {
                value["derivation"] = json!(true);
            }
            value
        })
        .collect::<Vec<_>>();

    // Faults: what keeps a reader from building the model from its own Cards.
    let outside = |at: &str| {
        !members.contains(at)
            && by_address
                .get(at)
                .is_some_and(|card| !crate::is_tree_root(card))
    };
    let mut closure = Vec::new();
    let mut cited = Vec::new();
    let mut reach_in = Vec::new();
    let view = crate::math::View {
        by_address,
        parsed,
        inbound,
    };
    let mut range = std::iter::once(head.to_string())
        .chain(in_range.iter().cloned())
        .collect::<Vec<_>>();
    range.sort_by(|a, b| address_order(a, b));
    for from in &range {
        let mut seen = BTreeSet::new();
        for read in rests_on(from) {
            if outside(&read.target) && seen.insert(read.target.clone()) {
                let target = &by_address[&read.target];
                let models = homes(&read.target);
                // A proof rests on the statement of an earlier unit, or on the book's notation table.
                if crate::math::is_unit_head(target)
                    || crate::math::is_notation_table(&title(&read.target))
                {
                    cited.push(json!({
                        "from": from,
                        "to": read.target,
                        "note": read.note,
                        "form": target.form.map(crate::math::MathForm::id),
                        "title": title(&read.target),
                    }));
                    continue;
                }
                // A step inside another unit: cite its statement, or make the step a lemma.
                if let Some(unit) = view.unit_of(&read.target) {
                    reach_in.push(json!({
                        "from": from,
                        "to": read.target,
                        "unit": unit,
                        "note": read.note,
                        "fix": format!(
                            "`{from}` rests on `{}`, a part inside the unit `{unit}`: rest on the statement `{unit}` instead, or make the step a lemma that both proofs cite",
                            read.target
                        ),
                    }));
                    continue;
                }
                closure.push(json!({
                    "from": from,
                    "to": read.target,
                    "note": read.note,
                    "card": card_ref(&read.target),
                    "fix": closure_fix(target, head, &models),
                }));
            }
        }
    }
    let mut forward = Vec::new();
    for step in &main {
        let here = position[step];
        let mut seen = BTreeSet::new();
        for read in rests_on(step) {
            if let Some(&there) = position.get(&read.target)
                && there > here
                && seen.insert(read.target.clone())
            {
                forward.push(json!({
                    "from": step,
                    "to": read.target,
                    "note": read.note,
                    "fix": format!(
                        "`{step}` rests on `{}`, which comes later in the build: move one of them so that every premise comes first",
                        read.target
                    ),
                }));
            }
        }
    }
    let mut tolerated = Vec::new();
    for part in &shared {
        let mut seen = BTreeSet::new();
        for read in rests_on(part) {
            if outside(&read.target) && seen.insert(read.target.clone()) {
                let mut value = json!({
                    "part": part,
                    "home": homes(part).into_iter().find(|model| model != head),
                    "premise": card_ref(&read.target),
                    "note": read.note,
                });
                // Not a fault, but a reader who follows a shared derivation would leave the model.
                if by_address[&read.target]
                    .facet
                    .as_ref()
                    .is_some_and(is_derivation)
                {
                    value["fix"] = json!(format!(
                        "`{}` is a step of the derivation that `{part}` continues: take it in too, so the whole derivation reads in this model",
                        read.target
                    ));
                }
                tolerated.push(value);
            }
        }
    }
    // A main-line part is anchored when a chain of rests-on Links among members leads to the setting.
    // A shared part is anchored in its home model.
    let mut anchored = setting
        .iter()
        .chain(&shared)
        .cloned()
        .collect::<BTreeSet<_>>();
    loop {
        let grown = premises
            .iter()
            .filter(|(at, targets)| {
                !anchored.contains(*at) && targets.iter().any(|target| anchored.contains(target))
            })
            .map(|(at, _)| at.clone())
            .collect::<Vec<_>>();
        if grown.is_empty() {
            break;
        }
        anchored.extend(grown);
    }
    let unanchored = main
        .iter()
        .filter(|step| !anchored.contains(*step))
        .map(|step| {
            json!({
                "address": step,
                "title": title(step),
                "fix": format!(
                    "`{step}` has no rests-on path back to the setting: say which earlier part it rests on (`依赖 [[…]]`)"
                ),
            })
        })
        .collect::<Vec<_>>();
    let beyond_level = if level == Some(Level::Concept) {
        in_range
            .iter()
            .filter(|at| by_address[*at].facet.as_ref().is_some_and(is_derivation))
            .map(|at| {
                json!({
                    "address": at,
                    "title": title(at),
                    "fix": format!(
                        "`{at}` is a Derivation part, but `{head}` is a concept model (level 1): fold the step into the part that uses it, or raise the model to formal (level 2)"
                    ),
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    // Derivation chains: runs of Derivation parts that follow each other in the build order
    // (a part beside a step does not break a run). One long step may take several
    // Derivation parts; a chain shows which, between which two members.
    let derivation_at = |at: &str| by_address[at].facet.as_ref().is_some_and(is_derivation);
    let mut chains = Vec::new();
    let mut run: Vec<String> = Vec::new();
    let mut after: Option<String> = None;
    let mut last: Option<String> = None;
    let chain = |run: &[String], after: Option<String>, before: Option<String>| json!({ "parts": run, "length": run.len(), "after": after, "before": before });
    for (at, line, _) in &order {
        match *line {
            "side" => continue,
            "main" | "shared" if derivation_at(at) => {
                if run.is_empty() {
                    after = last.clone();
                }
                run.push(at.clone());
            }
            _ => {
                if !run.is_empty() {
                    let before = (*line != "off").then(|| at.clone());
                    chains.push(chain(&run, after.take(), before));
                    run.clear();
                }
            }
        }
        last = Some(at.clone());
    }
    if !run.is_empty() {
        chains.push(chain(&run, after.take(), None));
    }
    json!({
        "level": level.map(Level::id),
        "level_number": level.map(Level::number),
        "setting": setting,
        "order": order_values,
        "chains": chains,
        "closure": closure,
        "cited": cited,
        "reach_in": reach_in,
        "forward": forward,
        "tolerated": tolerated,
        "unanchored": unanchored,
        "self_sufficient": closure.is_empty()
            && reach_in.is_empty()
            && forward.is_empty()
            && unanchored.is_empty(),
        "beyond_level": beyond_level,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn facets_are_short_free_labels_and_common_facets_are_normalized() {
        assert_eq!(Facet::parse(" Dynamics ").unwrap().id(), "dynamics");
        assert_eq!(Facet::parse("动态").unwrap().id(), "动态");
        assert_eq!(
            Facet::parse("  a   worked\texample ").unwrap().id(),
            "a worked example"
        );
        assert_eq!(
            Facet::parse("  ").unwrap_err().to_string(),
            "a Model facet cannot be empty"
        );
        assert!(
            Facet::parse(&"长".repeat(33))
                .unwrap_err()
                .to_string()
                .contains("at most 32 characters")
        );
        assert!(Facet::parse(&"长".repeat(32)).is_ok());
        assert!(
            Facet::parse("see [[3/1]]")
                .unwrap_err()
                .to_string()
                .contains("without Links")
        );
        assert_eq!(Facet::from_stored(Some("")), None);
        assert_eq!(Facet::from_stored(None), None);
        assert_eq!(parse_optional("").unwrap(), None);
        assert_eq!(parse_optional("none").unwrap(), None);
        assert_eq!(parse_optional("均衡").unwrap(), Some(Facet("均衡".into())));
    }

    #[test]
    fn emphasis_is_core_aside_or_none() {
        assert_eq!(Emphasis::parse(" Core").unwrap(), Emphasis::Core);
        assert_eq!(
            parse_optional_emphasis("aside").unwrap(),
            Some(Emphasis::Aside)
        );
        assert_eq!(parse_optional_emphasis("").unwrap(), None);
        assert_eq!(parse_optional_emphasis("regular").unwrap(), None);
        assert_eq!(
            Emphasis::parse("loud").unwrap_err().to_string(),
            "unknown emphasis `loud`; expected core, aside, or empty for none"
        );
    }

    #[test]
    fn the_head_is_the_nearest_model_card_above_a_chain_of_parts() {
        let cards: BTreeMap<&str, (CardType, bool)> = [
            ("3/1", (CardType::Idea, false)),
            ("3/2", (CardType::Model, false)),
            ("3/2|a", (CardType::Model, true)),
            ("3/2|a|1", (CardType::Model, true)),
            ("3/3", (CardType::Model, true)),
            ("3/1|a", (CardType::Model, true)),
        ]
        .into_iter()
        .collect();
        let lookup = |at: &str| cards.get(at).copied();
        let head = |at: &str| find_head(crate::parent_location(at), lookup);
        assert_eq!(head("3/2|a").as_deref(), Some("3/2"));
        assert_eq!(head("3/2|a|1").as_deref(), Some("3/2"));
        assert_eq!(head("3/3").as_deref(), Some("3/2"));
        assert_eq!(head("3/1|a"), None);
        assert_eq!(head("3/2"), None);
    }

    #[test]
    fn addresses_sort_in_folgezettel_order() {
        let mut addresses = vec!["3/10", "3/2|aa", "3/2|b", "3/9", "3/2|a|1", "3/2"];
        addresses.sort_by(|a, b| address_order(a, b));
        assert_eq!(
            addresses,
            ["3/2", "3/2|a|1", "3/2|b", "3/2|aa", "3/9", "3/10"]
        );
    }

    #[test]
    fn a_relation_note_is_the_sentence_that_makes_the_link() {
        let body = "本卡是模型 [[3/2]] 的动态。依赖时序 [[3/2|a|1]]：投资发生在**议价**之前，所以投资不足。另一句。";
        let at = body.find("[[3/2|a|1]]").unwrap();
        let note = sentence_around(body, at..at + "[[3/2|a|1]]".len(), |target| {
            if target == "3/2|a|1" {
                "时序".into()
            } else {
                target.into()
            }
        });
        assert_eq!(
            note,
            "依赖时序 「时序」：投资发生在议价之前，所以投资不足。"
        );
        let body = "- uses [[3/2|a]] for the payoffs\n- other";
        let at = body.find("[[").unwrap();
        assert_eq!(
            sentence_around(body, at..at + 11, |_| "setting".into()),
            "uses 「setting」 for the payoffs"
        );
    }

    /// S10 (#151): a Term use's note names the head it reaches, not the authored macro; an
    /// ordinary Link keeps its target.
    #[test]
    fn a_term_use_note_names_its_head_and_never_the_authored_macro() {
        let reads = read_links("均衡 [[均衡@2/1|a]] 决定价格。", |at| at.to_string());
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0].target, "2/1|a");
        assert_eq!(reads[0].form, LinkForm::Term);
        assert_eq!(reads[0].note, "均衡 「2/1|a」 决定价格。");
        let ordinary = read_links("见 [[2/1|a]] 的正文。", |at| at.to_string());
        assert_eq!(ordinary[0].note, "见 「2/1|a」 的正文。");
    }

    #[test]
    fn a_link_takes_its_kind_from_the_words_before_it_in_its_sentence() {
        let kinds = |body: &str| {
            read_links(body, |at| at.to_string())
                .into_iter()
                .map(|read| (read.target, read.kind.id()))
                .collect::<Vec<_>>()
        };
        let pair = |target: &str, kind: &'static str| (target.to_string(), kind);
        // The last announcing word before a Link names its kind; a Link right
        // after another in the same sentence shares it; a new sentence starts over.
        assert_eq!(
            kinds("依赖可承诺的还款 [[a]] 与清算比例 [[b]]。本卡是模型 [[m]] 的设定。"),
            [
                pair("a", "rests-on"),
                pair("b", "rests-on"),
                pair("m", "refers")
            ]
        );
        assert_eq!(
            kinds("依赖设定 [[s]]：区别在触发条件——[[x]] 里由状态触发，回应了 [[y]] 的不足。"),
            [
                pair("s", "rests-on"),
                pair("x", "contrasts"),
                pair("y", "contrasts")
            ]
        );
        assert_eq!(
            kinds("放宽“两期”：多期推广见 [[p]]；例 1 例示 [[q]]；反驳 [[r]]；类比 [[t]]。"),
            [
                pair("p", "relaxes"),
                pair("q", "illustrates"),
                pair("r", "objects"),
                pair("t", "corresponds")
            ]
        );
        // English words match at the start of a word only.
        assert_eq!(
            kinds("It relaxes [[a]]. This causes [[b]]. It uses [[c]]. A counterexample to [[d]]."),
            [
                pair("a", "relaxes"),
                pair("b", "refers"),
                pair("c", "rests-on"),
                pair("d", "objects")
            ]
        );
        // 比较静态 is no contrast.
        assert_eq!(kinds("均衡与比较静态见 [[e]]。"), [pair("e", "refers")]);
    }

    #[test]
    fn digest_words_for_corroboration_and_reference_have_kinds() {
        let kinds = |body: &str| {
            read_links(body, |at| at.to_string())
                .into_iter()
                .map(|read| (read.target, read.kind.id()))
                .collect::<Vec<_>>()
        };
        let pair = |target: &str, kind: &'static str| (target.to_string(), kind);
        // 印证 states a case that bears a claim out and 参照 a parallel point. 来自 is
        // an ordinary verb inside a clause (`只来自观察`), so it announces nothing.
        assert_eq!(
            kinds("结论直接来自 [[a]] 的导数公式。这印证 [[b]] 的判断。参照 [[c]]：均衡路径可能不唯一。"),
            [
                pair("a", "refers"),
                pair("b", "illustrates"),
                pair("c", "corresponds")
            ]
        );
        // 引出 points the other way (the linked Card builds on this one), so it
        // stays a plain reference; the later Card's 承接 carries the relation.
        assert_eq!(
            kinds("本部分承接 [[p]]，引出 [[n]] 的融资问题。"),
            [pair("p", "rests-on"), pair("n", "refers")]
        );
    }
}
