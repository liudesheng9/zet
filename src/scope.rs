//! The Write scope (CONTEXT.md, docs/agent-write-scope-proposal.md): the limit on which agent
//! Cards one `zt mcp` connection may change, on top of Access and Locks. It is declared once, when
//! the connection starts (`zt mcp --scope literature|open [--allow f] [--beneath f]`), and the
//! server checks it for every writer and every `apply_batch` item. A write outside it is refused
//! with the typed `scope_refused` and stores nothing. A connection that declares nothing is open,
//! with no allow list: exactly the behaviour before the Write scope existed.
//!
//! The same module holds the Topic title rule: an agent `create_topic` whose title equals or nearly
//! equals an agent Topic's title is refused with the typed `topic_exists`, in every scope.

use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use zt_check::scope::{is_topic_address, similarity, title_key, topic_links};

/// The metadata row that holds the Topic title rule's similarity threshold.
pub(crate) const SIMILARITY_KEY: &str = "scope.topic_similarity";
/// The threshold when the archive declares none.
pub(crate) const DEFAULT_SIMILARITY: f64 = 0.8;

/// The tools a connection under an allow list may not run: each makes a tree, renames Cards, or
/// deletes one, so its effect reaches Cards the list does not name.
const STRUCTURAL: [&str; 6] = [
    "create_topic",
    "create_literature",
    "edit_literature_metadata",
    "move_card",
    "insert_card",
    "delete_card",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Open,
    Literature,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Kind::Open => "open",
            Kind::Literature => "literature",
        }
    }
}

/// One connection's Write scope and the Cards this connection has made.
#[derive(Clone, Debug)]
pub(crate) struct WriteScope {
    kind: Kind,
    /// `None`: no allow list. `Some`: only these Cards, and the Cards this connection made.
    allow: Option<BTreeSet<String>>,
    /// Earlier Cards that may take one new Side successor, never an edit (S06, #103).
    beneath: BTreeSet<String>,
    made: BTreeSet<String>,
}

impl Default for WriteScope {
    fn default() -> Self {
        Self { kind: Kind::Open, allow: None, beneath: BTreeSet::new(), made: BTreeSet::new() }
    }
}

impl WriteScope {
    /// The scope the `zt mcp` arguments declare. An unknown argument or value is an error, so a
    /// client never runs in a wider scope than it asked for.
    pub(crate) fn from_args(args: &[String]) -> Result<Self> {
        let mut scope = Self::default();
        let mut rest = args.iter();
        while let Some(flag) = rest.next() {
            let mut value = || {
                rest.next()
                    .map(String::as_str)
                    .with_context(|| format!("`zt mcp {flag}` needs a value"))
            };
            match flag.as_str() {
                "--scope" => {
                    scope.kind = match value()? {
                        "open" => Kind::Open,
                        "literature" => Kind::Literature,
                        other => bail!("unknown Write scope `{other}`; expected literature or open"),
                    }
                }
                "--allow" => scope.allow = Some(read_list(value()?)?),
                "--beneath" => scope.beneath = read_list(value()?)?,
                other => bail!(
                    "unknown `zt mcp` argument `{other}`; expected --scope literature|open, --allow <file>, --beneath <file>"
                ),
            }
        }
        Ok(scope)
    }

    /// Why this write is outside the scope, or Ok. `made_here` holds the Cards the current request
    /// made before this item (an `apply_batch`), which count as this connection's own.
    pub(crate) fn check(
        &self,
        conn: &Connection,
        tool: &str,
        args: &Value,
        made_here: &BTreeSet<String>,
    ) -> Result<()> {
        if self.kind == Kind::Literature {
            self.check_literature(conn, tool, args)?;
        }
        if let Some(allowed) = &self.allow {
            self.check_allowed(tool, args, allowed, made_here)?;
        }
        Ok(())
    }

    /// Records the Cards a committed request made, so later requests of this connection may grow them.
    pub(crate) fn record<I: IntoIterator<Item = String>>(&mut self, addresses: I) {
        self.made.extend(addresses);
    }

    fn refuse(&self, tool: &str, at: &str, reason: &'static str, message: String) -> anyhow::Error {
        anyhow::Error::new(Refusal {
            tool: tool.to_string(),
            at: at.to_string(),
            scope: self.kind.id(),
            reason,
            message,
            item: None,
        })
    }

    fn check_literature(&self, conn: &Connection, tool: &str, args: &Value) -> Result<()> {
        let text = |name: &str| args.get(name).and_then(Value::as_str).unwrap_or_default();
        let scope = "this connection writes only Literature trees";
        let authored = || {
            let mut found = Vec::new();
            for name in ["title", "body", "text"] {
                found.extend(topic_links(text(name)));
            }
            if let Some(paragraph) = args["owned_paragraph"]["text"].as_str() {
                found.extend(topic_links(paragraph));
            }
            found
        };
        match tool {
            "create_topic" => {
                return Err(self.refuse(
                    tool,
                    "",
                    "topic_tree",
                    format!("{scope}: it creates no Topic tree; Topic trees belong to zt-idea-grow"),
                ));
            }
            "create_successor" | "edit_card" | "insert_card" | "move_card" | "delete_card"
            | "edit_literature_metadata" => {
                for name in ["at", "before", "to", "into"] {
                    let address = text(name);
                    if is_topic_address(address) {
                        return Err(self.refuse(
                            tool,
                            address,
                            "topic_tree",
                            format!("{scope}: `{address}` is a Topic-tree Card; Topic trees belong to zt-idea-grow"),
                        ));
                    }
                }
                if text("type").eq_ignore_ascii_case("hub") || self.names_hub(conn, tool, text("at"))? {
                    return Err(self.refuse(
                        tool,
                        text("at"),
                        "hub",
                        format!("{scope}: it makes or changes no Hub Card; hubs belong to zt-idea-grow"),
                    ));
                }
            }
            "create_literature" => {}
            _ => return Ok(()),
        }
        let links = authored();
        if !links.is_empty() {
            return Err(self.refuse(
                tool,
                text("at"),
                "topic_link",
                format!(
                    "{scope}: the text links Topic-tree Cards ({}); link Literature Cards only",
                    links.join(", ")
                ),
            ));
        }
        Ok(())
    }

    /// Whether the Card a change names (not a new Card's parent) is a Hub Card.
    fn names_hub(&self, conn: &Connection, tool: &str, at: &str) -> Result<bool> {
        if !matches!(tool, "edit_card" | "move_card" | "delete_card") || at.is_empty() {
            return Ok(false);
        }
        let card_type: Option<String> = conn
            .query_row(
                "SELECT card_type FROM cards WHERE location = ?1 OR citation_key = ?1",
                [at],
                |row| row.get(0),
            )
            .optional()?;
        Ok(card_type.as_deref() == Some("hub"))
    }

    fn check_allowed(
        &self,
        tool: &str,
        args: &Value,
        allowed: &BTreeSet<String>,
        made_here: &BTreeSet<String>,
    ) -> Result<()> {
        let at = args.get("at").and_then(Value::as_str).unwrap_or_default();
        if STRUCTURAL.contains(&tool) {
            return Err(self.refuse(
                tool,
                at,
                "structural",
                format!("{tool} is not allowed under this connection's allow list"),
            ));
        }
        if tool == "create_successor" && args.get("before").is_some_and(|b| !b.is_null()) {
            return Err(self.refuse(
                tool,
                at,
                "renumbers",
                "create_successor with `before` renumbers Side successors the allow list does not name"
                    .to_string(),
            ));
        }
        if !matches!(tool, "create_successor" | "edit_card") {
            return Ok(());
        }
        let own = allowed.contains(at) || self.made.contains(at) || made_here.contains(at);
        if own {
            return Ok(());
        }
        if self.beneath.contains(at) {
            if tool == "create_successor" && args["kind"].as_str() == Some("side") {
                return Ok(());
            }
            return Err(self.refuse(
                tool,
                at,
                "beside_only",
                format!(
                    "file a new Card beside `{at}` only as a Side successor; the earlier Card and its Direct chain are not this connection's"
                ),
            ));
        }
        Err(self.refuse(
            tool,
            at,
            "not_listed",
            format!("`{at}` is not one of this connection's Cards; it may change only its listed Cards and the Cards it made"),
        ))
    }
}

/// One address per line; blank lines are ignored.
fn read_list(path: &str) -> Result<BTreeSet<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read the Write scope list `{path}`"))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect())
}

/// The typed refusal of a write outside the connection's Write scope. Like `card_locked`, it is a
/// planning stop: the caller chooses another destination, it never retries the same write.
#[derive(Clone, Debug)]
pub(crate) struct Refusal {
    tool: String,
    at: String,
    scope: &'static str,
    reason: &'static str,
    message: String,
    item: Option<Value>,
}

impl Refusal {
    pub(crate) fn value(&self) -> Value {
        let mut value = json!({
            "error": "scope_refused",
            "message": self.message,
            "tool": self.tool,
            "at": self.at,
            "scope": self.scope,
            "reason": self.reason,
        });
        if let Some(item) = &self.item {
            value["item"] = item.clone();
        }
        value
    }

    /// The same refusal, naming the batch item that a real `apply_batch` stopped on.
    pub(crate) fn in_batch(mut self, index: usize, tool: &str, op_key: &str) -> Self {
        self.message = format!("batch item {index} ({tool}): {}", self.message);
        self.item = Some(json!({"index": index, "tool": tool, "op_key": op_key}));
        self
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scope_refused: {}", self.message)
    }
}

impl std::error::Error for Refusal {}

/// The typed refusal of an agent Topic whose title another agent Topic already has or nearly has.
#[derive(Clone, Debug)]
pub(crate) struct TopicExists {
    title: String,
    root: String,
    existing: String,
    similarity: f64,
}

impl TopicExists {
    pub(crate) fn value(&self) -> Value {
        json!({
            "error": "topic_exists",
            "message": self.to_string(),
            "title": self.title,
            "root": self.root,
            "existing_title": self.existing,
            "similarity": self.similarity,
        })
    }
}

impl std::fmt::Display for TopicExists {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "topic exists: `{}` {}. File the Card under that Topic instead of creating one.",
            self.root, self.existing
        )
    }
}

impl std::error::Error for TopicExists {}

/// The Topic title rule: refuse a new agent Topic whose title key equals an agent Topic's, or whose
/// bigram similarity with one reaches the archive's threshold. It runs inside the write
/// transaction under the edit lock, so two writers cannot both pass it.
pub(crate) fn check_topic_title(conn: &Connection, title: &str) -> Result<()> {
    let threshold = topic_similarity(conn)?;
    let key = title_key(title);
    let mut query = conn.prepare(
        "SELECT location, text FROM cards WHERE workspace = 'agent' AND is_topic = 1 ORDER BY location",
    )?;
    let topics = query
        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (root, text) in topics {
        let existing = crate::parse_card_text(&text)?.title;
        let other = title_key(&existing);
        let score = if other == key { 1.0 } else { similarity(&other, &key) };
        if score >= threshold {
            return Err(anyhow::Error::new(TopicExists {
                title: title.to_string(),
                root,
                existing,
                similarity: (score * 1000.0).round() / 1000.0,
            }));
        }
    }
    Ok(())
}

/// The archive's similarity threshold for the Topic title rule.
pub(crate) fn topic_similarity(conn: &Connection) -> Result<f64> {
    let value: Option<String> = conn
        .query_row("SELECT value FROM metadata WHERE key = ?1", [SIMILARITY_KEY], |row| row.get(0))
        .optional()?;
    match value {
        None => Ok(DEFAULT_SIMILARITY),
        Some(value) => parse_similarity(&value),
    }
}

fn parse_similarity(value: &str) -> Result<f64> {
    match value.trim().parse::<f64>() {
        Ok(number) if number > 0.0 && number <= 1.0 => Ok(number),
        _ => bail!("`{SIMILARITY_KEY}` must be a number above 0 and at most 1, or `default`; found `{value}`"),
    }
}

/// `zt config set scope.topic_similarity <0..1|default>`: one metadata row; `default` removes it.
pub(crate) fn set_topic_similarity(conn: &Connection, value: &str) -> Result<String> {
    if value.trim().eq_ignore_ascii_case("default") {
        conn.execute("DELETE FROM metadata WHERE key = ?1", [SIMILARITY_KEY])?;
        return Ok(format!("{SIMILARITY_KEY}=default"));
    }
    let number = parse_similarity(value)?;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [SIMILARITY_KEY, &number.to_string()],
    )?;
    Ok(format!("{SIMILARITY_KEY}={number}"))
}
