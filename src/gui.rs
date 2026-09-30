//! `zt gui`: a local, browser-based view of the card space.
//!
//! The server binds to the loopback interface only, answers API calls that
//! carry the per-launch token embedded in its page, and performs every change
//! through the same Card operations as the shell commands and the Session.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::Card;
use crate::link::{LinkBody, LinkLifecycle, LinkStatus};

const INDEX_HTML: &str = include_str!("gui/index.html");
const APP_CSS: &str = include_str!("gui/app.css");
const APP_JS: &str = include_str!("gui/app.js");
const DEFAULT_PORT: u16 = 4717;
const TOKEN_HEADER: &str = "X-ZT-Token";

struct GuiOptions {
    port: Option<u16>,
    open: bool,
}

fn parse_options(args: &[String]) -> Result<GuiOptions> {
    let mut options = GuiOptions {
        port: None,
        open: true,
    };
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--no-open" => options.open = false,
            "--port" if index + 1 < args.len() => {
                index += 1;
                options.port = Some(
                    args[index]
                        .parse()
                        .with_context(|| format!("invalid port `{}`", args[index]))?,
                );
            }
            value => {
                bail!("unexpected gui argument `{value}`; expected [--port <port>] [--no-open]")
            }
        }
        index += 1;
    }
    Ok(options)
}

pub(crate) fn run(args: &[String]) -> Result<()> {
    let options = parse_options(args)?;
    let root = crate::require_service_up()?;
    let server = bind(options.port)?;
    let port = server
        .server_addr()
        .to_ip()
        .context("GUI server is not bound to an IP address")?
        .port();
    let token = random_token()?;
    let url = format!("http://127.0.0.1:{port}/");
    crate::session::register_session(&root)?;
    let _guard = crate::session::SessionGuard { root: root.clone() };
    crate::log_event(&root, &format!("gui started on {url}")).ok();
    println!("zt gui: {url}");
    println!("press Ctrl+C or use the power button in the window to stop");
    if options.open {
        open_window(&url);
    }
    let app = App { root, token, port };
    for request in server.incoming_requests() {
        if app.handle(request) == Flow::Quit {
            break;
        }
    }
    crate::log_event(&app.root, "gui stopped").ok();
    println!("zt gui stopped");
    Ok(())
}

fn bind(port: Option<u16>) -> Result<Server> {
    let attempt = |port: u16| Server::http(("127.0.0.1", port));
    match port {
        Some(port) => attempt(port)
            .map_err(|error| anyhow::anyhow!("cannot listen on 127.0.0.1:{port}: {error}")),
        None => attempt(DEFAULT_PORT)
            .or_else(|_| attempt(0))
            .map_err(|error| anyhow::anyhow!("cannot start the GUI server: {error}")),
    }
}

fn random_token() -> Result<String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).context("failed to read OS randomness")?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Prefer a chromeless app window; fall back to the default browser.
fn open_window(url: &str) {
    use std::process::{Command, Stdio};
    let quiet = |command: &mut Command| {
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
    };
    #[cfg(windows)]
    {
        let candidates = [
            (
                "ProgramFiles(x86)",
                r"Microsoft\Edge\Application\msedge.exe",
            ),
            ("ProgramFiles", r"Microsoft\Edge\Application\msedge.exe"),
            ("ProgramFiles", r"Google\Chrome\Application\chrome.exe"),
            ("LOCALAPPDATA", r"Google\Chrome\Application\chrome.exe"),
        ];
        for (base, relative) in candidates {
            if let Some(base) = std::env::var_os(base) {
                let exe = PathBuf::from(base).join(relative);
                if exe.exists() && quiet(Command::new(exe).arg(format!("--app={url}"))) {
                    return;
                }
            }
        }
        quiet(Command::new("cmd").args(["/C", "start", "", url]));
    }
    #[cfg(target_os = "macos")]
    {
        quiet(Command::new("open").arg(url));
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        quiet(Command::new("xdg-open").arg(url));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Flow {
    Continue,
    Quit,
}

struct App {
    root: PathBuf,
    token: String,
    port: u16,
}

struct ApiError(String);

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        Self(format!("{error:#}"))
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        Self(error.to_string())
    }
}

type ApiResult = std::result::Result<Value, ApiError>;

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("static header is valid")
}

impl App {
    fn handle(&self, mut request: Request) -> Flow {
        let url = request.url().to_string();
        let (path, query) = url.split_once('?').unwrap_or((&url, ""));
        let path = path.to_string();
        let query = parse_query(query);
        if !self.host_allowed(&request) {
            respond_text(
                request,
                421,
                "text/plain",
                "misdirected request".to_string(),
            );
            return Flow::Continue;
        }
        let method = request.method().clone();
        match (&method, path.as_str()) {
            (Method::Get, "/") | (Method::Get, "/index.html") => {
                let page = INDEX_HTML.replace("__ZT_TOKEN__", &self.token);
                respond_text(request, 200, "text/html; charset=utf-8", page);
                return Flow::Continue;
            }
            (Method::Get, "/app.css") => {
                respond_text(request, 200, "text/css; charset=utf-8", APP_CSS.to_string());
                return Flow::Continue;
            }
            (Method::Get, "/app.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    APP_JS.to_string(),
                );
                return Flow::Continue;
            }
            _ => {}
        }
        if !path.starts_with("/api/") {
            respond_text(request, 404, "text/plain", "not found".to_string());
            return Flow::Continue;
        }
        let authorized = request
            .headers()
            .iter()
            .any(|h| h.field.equiv(TOKEN_HEADER) && h.value.as_str() == self.token);
        if !authorized {
            respond_json(
                request,
                403,
                &json!({ "error": "missing or invalid GUI token" }),
            );
            return Flow::Continue;
        }
        let body = if method == Method::Post {
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                respond_json(
                    request,
                    400,
                    &json!({ "error": "request body must be UTF-8" }),
                );
                return Flow::Continue;
            }
            match serde_json::from_str::<Value>(if raw.is_empty() { "{}" } else { &raw }) {
                Ok(value) => value,
                Err(error) => {
                    respond_json(
                        request,
                        400,
                        &json!({ "error": format!("invalid JSON: {error}") }),
                    );
                    return Flow::Continue;
                }
            }
        } else {
            Value::Null
        };
        let flow = if (&method, path.as_str()) == (&Method::Post, "/api/quit") {
            Flow::Quit
        } else {
            Flow::Continue
        };
        let result = self.route(&method, &path, &query, &body);
        match result {
            Ok(value) => respond_json(request, 200, &value),
            Err(ApiError(message)) => respond_json(request, 400, &json!({ "error": message })),
        }
        flow
    }

    /// Reject DNS-rebinding requests whose Host is not this loopback server.
    fn host_allowed(&self, request: &Request) -> bool {
        let Some(host) = request
            .headers()
            .iter()
            .find(|h| h.field.equiv("Host"))
            .map(|h| h.value.as_str().to_ascii_lowercase())
        else {
            return false;
        };
        [
            format!("127.0.0.1:{}", self.port),
            format!("localhost:{}", self.port),
        ]
        .contains(&host)
    }

    fn route(
        &self,
        method: &Method,
        path: &str,
        query: &BTreeMap<String, String>,
        body: &Value,
    ) -> ApiResult {
        let param = |name: &str| -> std::result::Result<String, ApiError> {
            query
                .get(name)
                .cloned()
                .ok_or_else(|| ApiError(format!("missing `{name}` parameter")))
        };
        match (method, path) {
            (Method::Get, "/api/overview") => self.overview(),
            (Method::Get, "/api/tree") => self.tree(&param("id")?),
            (Method::Get, "/api/card") => self.card(&param("at")?),
            (Method::Get, "/api/graph") => self.graph(),
            (Method::Get, "/api/search") => self.search(&param("q")?),
            (Method::Get, "/api/broken") => self.broken(),
            (Method::Post, "/api/render") => self.render(body),
            (Method::Post, "/api/create") => self.create(body),
            (Method::Post, "/api/literature/check") => self.literature_check(body),
            (Method::Post, "/api/literature") => self.create_literature(body),
            (Method::Post, "/api/edit") => self.edit(body),
            (Method::Post, "/api/metadata") => self.edit_metadata(body),
            (Method::Post, "/api/delete") => self.delete(body),
            (Method::Post, "/api/move") => self.move_card(body),
            (Method::Post, "/api/quit") => Ok(json!({ "ok": true })),
            _ => Err(ApiError(format!("unknown endpoint {path}"))),
        }
    }

    fn snapshot(&self) -> Result<Snapshot> {
        crate::ensure_markdown_cards(&self.root)?;
        let conn = crate::open_db(&self.root)?;
        Snapshot::load(&conn)
    }

    fn overview(&self) -> ApiResult {
        let snapshot = self.snapshot()?;
        let counts = crate::card_counts(&self.root)?;
        let mut trees = Vec::new();
        for card in snapshot
            .cards
            .iter()
            .filter(|card| crate::is_tree_root(card))
        {
            let tree = crate::tree_id(&card.address);
            let members = snapshot
                .cards
                .iter()
                .filter(|member| crate::is_tree_member(&member.address, tree))
                .count();
            trees.push(json!({
                "tree": tree,
                "root": card.address,
                "kind": if card.is_lit { "literature" } else { "topic" },
                "title": snapshot.title(&card.address),
                "excerpt": excerpt(&snapshot.parsed[&card.address].body),
                "count": members,
            }));
        }
        let broken = snapshot.lifecycle.broken_links()?.len();
        Ok(json!({
            "fingerprint": snapshot.fingerprint(),
            "archive_root": self.root.display().to_string(),
            "stats": {
                "total": counts.total,
                "topics": counts.topics,
                "regular": counts.regular,
                "literature": counts.literature,
            },
            "broken": broken,
            "trees": trees,
        }))
    }

    fn tree(&self, tree: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        let root = crate::tree_root_address(tree);
        if !snapshot.by_address.contains_key(&root) {
            return Err(ApiError(format!("tree `{tree}` does not exist")));
        }
        let cards = snapshot
            .cards
            .iter()
            .filter(|card| crate::is_tree_member(&card.address, tree))
            .map(|card| snapshot.summary(card))
            .collect::<Vec<_>>();
        Ok(json!({
            "tree": tree,
            "root": root,
            "kind": if snapshot.by_address[&root].is_lit { "literature" } else { "topic" },
            "title": snapshot.title(&root),
            "cards": cards,
        }))
    }

    fn graph(&self) -> ApiResult {
        let snapshot = self.snapshot()?;
        let nodes = snapshot
            .cards
            .iter()
            .map(|card| snapshot.summary(card))
            .collect::<Vec<_>>();
        Ok(json!({ "nodes": nodes }))
    }

    fn card(&self, target: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        let card = snapshot
            .by_address
            .get(target)
            .ok_or_else(|| ApiError(format!("card `{target}` does not exist")))?;
        let parsed = &snapshot.parsed[&card.address];
        let nav = |address: Option<String>| {
            address
                .filter(|address| snapshot.by_address.contains_key(address))
                .map(|address| json!({ "address": address, "title": snapshot.title(&address) }))
        };
        let sides = if crate::is_tree_root(card) {
            Vec::new()
        } else {
            snapshot
                .cards
                .iter()
                .filter(|other| {
                    crate::immediate_side_label(&other.address, &card.address)
                        .is_some_and(|label| other.address == format!("{}|{label}", card.address))
                })
                .map(|other| json!({ "address": other.address, "title": snapshot.title(&other.address) }))
                .collect()
        };
        let inbound = snapshot
            .inbound
            .get(&card.address)
            .into_iter()
            .flatten()
            .map(|source| json!({ "address": source, "title": snapshot.title(source) }))
            .collect::<Vec<_>>();
        let context = if crate::is_tree_root(card) {
            "tree-root"
        } else {
            "regular"
        };
        Ok(json!({
            "address": card.address,
            "tree": crate::tree_id(&card.address),
            "kind": kind(card),
            "context": context,
            "title": parsed.title,
            "text": card.text,
            "body": parsed.body,
            "html": render_markdown(&parsed.body, &snapshot),
            "bibtex": card.bibtex,
            "citation_key": card.citation_key,
            "parent": nav(crate::parent_location(&card.address)),
            "direct": nav(crate::direct_successor(&card.address).ok()),
            "sides": sides,
            "inbound": inbound,
            "outbound": snapshot.outbound(&card.address),
        }))
    }

    fn search(&self, query: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(json!({ "results": [] }));
        }
        let results = snapshot
            .cards
            .iter()
            .filter_map(|card| {
                let parsed = &snapshot.parsed[&card.address];
                let haystack = format!("{}\n{}\n{}", card.address, parsed.title, parsed.body);
                let lower = haystack.to_lowercase();
                let at = lower.find(&needle)?;
                let line = lower[..at].matches('\n').count();
                let snippet = haystack.lines().nth(line).unwrap_or_default();
                Some(json!({
                    "address": card.address,
                    "title": parsed.title,
                    "kind": kind(card),
                    "snippet": clip(snippet.trim(), 160),
                }))
            })
            .take(60)
            .collect::<Vec<_>>();
        Ok(json!({ "results": results }))
    }

    fn broken(&self) -> ApiResult {
        let snapshot = self.snapshot()?;
        let broken = snapshot
            .lifecycle
            .broken_links()?
            .into_iter()
            .map(|broken| {
                json!({
                    "source": broken.source_address,
                    "title": broken.source_title,
                    "target": broken.target,
                    "line": broken.source_line,
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({ "broken": broken }))
    }

    fn render(&self, body: &Value) -> ApiResult {
        let text = str_field(body, "text")?;
        let literature = body
            .get("literature")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let snapshot = self.snapshot()?;
        let parsed = if literature {
            crate::parse_literature_edit_text(&text)
        } else {
            crate::parse_card_text(&text)
        };
        Ok(match parsed {
            Ok(parsed) => {
                let issue = LinkBody::parse(&parsed.body)
                    .err()
                    .map(|error| error.to_string());
                json!({
                    "title": parsed.title,
                    "html": render_markdown(&parsed.body, &snapshot),
                    "error": issue,
                })
            }
            Err(error) => json!({
                "title": "",
                "html": render_markdown(&text, &snapshot),
                "error": error.to_string(),
            }),
        })
    }

    fn create(&self, body: &Value) -> ApiResult {
        let kind = str_field(body, "kind")?;
        let text = str_field(body, "text")?;
        let _lock = crate::acquire_edit_lock(&self.root)?;
        let mut conn = crate::open_db(&self.root)?;
        let location = match kind.as_str() {
            "topic" => {
                let location = crate::next_topic_location(&conn)?;
                crate::insert_card(&mut conn, &location, true, &text)?;
                crate::bump_next_topic_id(&conn, &location)?;
                location
            }
            "direct" => {
                let parent = crate::load_card(&conn, &str_field(body, "at")?)?;
                let location = crate::direct_successor(&parent.address)?;
                if crate::location_exists(&conn, &location)? {
                    return Err(ApiError(format!(
                        "direct successor already exists: {location}"
                    )));
                }
                crate::insert_card(&mut conn, &location, false, &text)?;
                location
            }
            "side" => {
                let parent = crate::load_card(&conn, &str_field(body, "at")?)?;
                crate::ensure_side_parent(&parent)?;
                let location =
                    crate::next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
                crate::insert_card(&mut conn, &location, false, &text)?;
                location
            }
            other => return Err(ApiError(format!("unknown card kind `{other}`"))),
        };
        Ok(json!({ "address": location }))
    }

    fn literature_check(&self, body: &Value) -> ApiResult {
        let bibtex = str_field(body, "bibtex")?;
        let metadata = crate::literature::parse_metadata(&bibtex)?;
        let conn = crate::open_db(&self.root)?;
        let exists = crate::citation_key_exists_case_insensitive(&conn, &metadata.citation_key)?;
        Ok(json!({
            "citation_key": metadata.citation_key,
            "title": metadata.title,
            "exists": exists,
        }))
    }

    fn create_literature(&self, body: &Value) -> ApiResult {
        let bibtex = str_field(body, "bibtex")?;
        let edited = str_field(body, "text")?;
        let _lock = crate::acquire_edit_lock(&self.root)?;
        let mut conn = crate::open_db(&self.root)?;
        let metadata = crate::literature::parse_metadata(&bibtex)?;
        let parsed = crate::parse_literature_edit_text(&edited)?;
        let text = crate::compose_card_text(&metadata.title, &parsed.body, "");
        crate::insert_literature_card(&mut conn, &metadata.citation_key, &bibtex, &text)?;
        Ok(json!({ "address": metadata.citation_key }))
    }

    fn edit(&self, body: &Value) -> ApiResult {
        let at = str_field(body, "at")?;
        let text = str_field(body, "text")?;
        let _lock = crate::acquire_edit_lock(&self.root)?;
        let mut conn = crate::open_db(&self.root)?;
        let card = self.unchanged_card(&conn, &at, body)?;
        if card.is_lit {
            crate::update_literature_text(&mut conn, &card, &text)?;
        } else {
            crate::update_card_text(&mut conn, &card, &text)?;
        }
        Ok(json!({ "address": card.address }))
    }

    fn edit_metadata(&self, body: &Value) -> ApiResult {
        let at = str_field(body, "at")?;
        let bibtex = str_field(body, "bibtex")?;
        let _lock = crate::acquire_edit_lock(&self.root)?;
        let mut conn = crate::open_db(&self.root)?;
        let card = self.unchanged_card(&conn, &at, body)?;
        if !card.is_lit {
            return Err(ApiError(
                "only Literature Cards have BibTeX metadata".to_string(),
            ));
        }
        let old_key = card
            .citation_key
            .clone()
            .context("Literature Card is missing its Citation key")?;
        let metadata = crate::literature::parse_metadata(&bibtex)?;
        if metadata.citation_key != old_key {
            if metadata.citation_key.eq_ignore_ascii_case(&old_key)
                || crate::citation_key_conflicts(&conn, &metadata.citation_key, card.row_id)?
            {
                return Err(ApiError(format!(
                    "Citation key `{}` conflicts with an existing Literature Card",
                    metadata.citation_key
                )));
            }
            if confirm_field(body) != Some("move") {
                let lines = crate::citation_rename_lines(&conn, &old_key, &metadata.citation_key)?;
                return Ok(json!({ "confirm": "move", "lines": lines }));
            }
        }
        crate::update_literature_metadata(
            &mut conn,
            &card,
            &metadata.citation_key,
            &bibtex,
            &metadata.title,
        )?;
        Ok(json!({ "address": metadata.citation_key }))
    }

    fn delete(&self, body: &Value) -> ApiResult {
        let at = str_field(body, "at")?;
        let _lock = crate::acquire_edit_lock(&self.root)?;
        let mut conn = crate::open_db(&self.root)?;
        let plan = crate::delete_plan(&conn, &at)?;
        if confirm_field(body) != Some(plan.confirmation.as_str()) {
            return Ok(json!({
                "confirm": plan.confirmation,
                "lines": crate::delete_verification_lines(&plan),
            }));
        }
        let result = crate::apply_delete_plan(&mut conn, &at, plan)?;
        let next = crate::parent_location(&at)
            .filter(|parent| crate::target_exists(&conn, parent).unwrap_or(false));
        Ok(json!({
            "deleted": result.deleted_count,
            "moved": result.moved,
            "address": next,
        }))
    }

    fn move_card(&self, body: &Value) -> ApiResult {
        let at = str_field(body, "at")?;
        let to = str_field(body, "to")?;
        let _lock = crate::acquire_edit_lock(&self.root)?;
        let mut conn = crate::open_db(&self.root)?;
        let plan = crate::move_plan(&conn, &at, &to)?;
        if confirm_field(body) != Some("move") {
            return Ok(json!({
                "confirm": "move",
                "lines": crate::move_verification_lines(&plan),
            }));
        }
        crate::apply_move_plan(&mut conn, &plan)?;
        Ok(json!({ "address": to, "moved": plan.mapping }))
    }

    /// Load a Card for saving, refusing if it changed since the GUI opened it.
    fn unchanged_card(&self, conn: &Connection, at: &str, body: &Value) -> Result<Card> {
        let card = crate::load_card(conn, at)?;
        if let Some(base) = body.get("base").and_then(Value::as_str)
            && base != card.text
        {
            bail!("card `{at}` changed since it was opened; reload it and try again");
        }
        Ok(card)
    }
}

/// Every Card with its parsed text and Link graph, read once per request.
struct Snapshot {
    cards: Vec<Card>,
    by_address: BTreeMap<String, Card>,
    parsed: BTreeMap<String, crate::ParsedCard>,
    lifecycle: LinkLifecycle,
    inbound: BTreeMap<String, BTreeSet<String>>,
}

impl Snapshot {
    fn load(conn: &Connection) -> Result<Self> {
        let cards = crate::load_cards(conn)?;
        let lifecycle = crate::link_lifecycle_from_cards(&cards)?;
        let mut parsed = BTreeMap::new();
        for card in &cards {
            parsed.insert(card.address.clone(), crate::parse_card_text(&card.text)?);
        }
        let by_address: BTreeMap<String, Card> = cards
            .iter()
            .map(|card| (card.address.clone(), card.clone()))
            .collect();
        let mut inbound: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for card in &cards {
            for occurrence in LinkBody::parse(&parsed[&card.address].body)?.occurrences() {
                if by_address.contains_key(occurrence.target) {
                    inbound
                        .entry(occurrence.target.to_string())
                        .or_default()
                        .insert(card.address.clone());
                }
            }
        }
        Ok(Self {
            cards,
            by_address,
            parsed,
            lifecycle,
            inbound,
        })
    }

    /// Changes whenever any Card changes, so open windows can follow edits
    /// made from the shell or a Session.
    fn fingerprint(&self) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        for card in &self.cards {
            hasher.update(card.address.as_bytes());
            hasher.update([0]);
            hasher.update(card.text.as_bytes());
            hasher.update([0]);
            hasher.update(card.bibtex.as_deref().unwrap_or_default().as_bytes());
            hasher.update([0]);
        }
        hasher
            .finalize()
            .iter()
            .take(12)
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn title(&self, address: &str) -> String {
        self.parsed
            .get(address)
            .map(|parsed| parsed.title.clone())
            .unwrap_or_default()
    }

    fn outbound(&self, address: &str) -> Vec<Value> {
        let Some(parsed) = self.parsed.get(address) else {
            return Vec::new();
        };
        let mut seen = BTreeSet::new();
        LinkBody::parse(&parsed.body)
            .map(|body| {
                body.occurrences()
                    .iter()
                    .filter(|occurrence| seen.insert(occurrence.target.to_string()))
                    .map(|occurrence| {
                        let exists = self.by_address.contains_key(occurrence.target);
                        json!({
                            "address": occurrence.target,
                            "title": self.title(occurrence.target),
                            "broken": !exists,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    fn summary(&self, card: &Card) -> Value {
        let parent = crate::parent_location(&card.address);
        let edge = if crate::is_tree_root(card) {
            "root"
        } else if crate::valid_side_label(crate::last_segment(&card.address)) {
            "side"
        } else {
            "direct"
        };
        let links = self
            .outbound(&card.address)
            .into_iter()
            .filter(|link| link["broken"] == false)
            .filter_map(|link| link["address"].as_str().map(str::to_string))
            .collect::<Vec<_>>();
        json!({
            "address": card.address,
            "tree": crate::tree_id(&card.address),
            "title": self.title(&card.address),
            "kind": kind(card),
            "edge": edge,
            "parent": parent,
            "excerpt": excerpt(&self.parsed[&card.address].body),
            "links": links,
            "inbound": self.inbound.get(&card.address).map_or(0, BTreeSet::len),
        })
    }
}

fn kind(card: &Card) -> &'static str {
    if card.is_lit {
        "literature"
    } else if card.is_topic {
        "topic"
    } else {
        "regular"
    }
}

/// Render Card body Markdown to HTML. Raw HTML is shown as text, and
/// `[[target]]` Links become navigable chips classified like the Session does.
fn render_markdown(body: &str, snapshot: &Snapshot) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut events: Vec<Event> = Vec::new();
    let mut pending = String::new();
    let mut in_code_block = false;
    let flush = |pending: &mut String, events: &mut Vec<Event>, in_code_block: bool| {
        if pending.is_empty() {
            return;
        }
        let text = std::mem::take(pending);
        if in_code_block {
            events.push(Event::Text(text.into()));
        } else {
            events.extend(link_events(&text, snapshot));
        }
    };
    for event in Parser::new_ext(body, options) {
        match event {
            Event::Text(text) => pending.push_str(&text),
            Event::Html(raw) | Event::InlineHtml(raw) => pending.push_str(&raw),
            other => {
                flush(&mut pending, &mut events, in_code_block);
                match &other {
                    Event::Start(Tag::CodeBlock(_)) => in_code_block = true,
                    Event::End(TagEnd::CodeBlock) => in_code_block = false,
                    _ => {}
                }
                events.push(other);
            }
        }
    }
    flush(&mut pending, &mut events, in_code_block);
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

fn link_events<'a>(text: &str, snapshot: &Snapshot) -> Vec<pulldown_cmark::Event<'a>> {
    use pulldown_cmark::Event;
    let mut events = Vec::new();
    let mut offset = 0;
    for link in snapshot.lifecycle.classify_for_rendering(text) {
        if link.macro_range.start > offset {
            events.push(Event::Text(
                text[offset..link.macro_range.start].to_string().into(),
            ));
        }
        let target = escape_html(link.target);
        let chip = match link.status {
            LinkStatus::Valid => format!(
                "<a class=\"zt-link\" href=\"#/card/{}\" data-target=\"{target}\"><span class=\"zt-link-addr\">{target}</span><span class=\"zt-link-title\">{}</span></a>",
                escape_html(&percent_encode(link.target)),
                escape_html(&snapshot.title(link.target)),
            ),
            LinkStatus::Broken => format!(
                "<span class=\"zt-link broken\" title=\"Broken link\"><span class=\"zt-link-addr\">{target}</span><span class=\"zt-link-title\">missing</span></span>"
            ),
        };
        events.push(Event::InlineHtml(chip.into()));
        offset = link.macro_range.end;
    }
    if offset < text.len() {
        events.push(Event::Text(text[offset..].to_string().into()));
    }
    events
}

fn excerpt(body: &str) -> String {
    let fenced = crate::link::fenced_code_ranges(body);
    let mut start = 0;
    for raw in body.split_inclusive('\n') {
        let line_start = start;
        start += raw.len();
        if fenced.iter().any(|range| range.contains(&line_start)) {
            continue;
        }
        let line = raw
            .trim()
            .trim_start_matches(['#', '>', '-', '*', '+', ' '])
            .trim();
        if !line.is_empty() {
            let plain = ["**", "__", "~~", "`", "[[", "]]"]
                .iter()
                .fold(line.to_string(), |text, mark| text.replace(mark, ""));
            return clip(&plain, 140);
        }
    }
    String::new()
}

fn clip(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        text.to_string()
    } else {
        let mut clipped = text.chars().take(max_chars - 1).collect::<String>();
        clipped.push('…');
        clipped
    }
}

fn str_field(body: &Value, name: &str) -> Result<String> {
    body.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing `{name}` field"))
}

fn confirm_field(body: &Value) -> Option<&str> {
    body.get("confirm").and_then(Value::as_str)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn parse_query(query: &str) -> BTreeMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => decoded.push(b' '),
            b'%' if index + 3 <= bytes.len() => {
                match std::str::from_utf8(&bytes[index + 1..index + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok())
                {
                    Some(byte) => {
                        decoded.push(byte);
                        index += 2;
                    }
                    None => decoded.push(b'%'),
                }
            }
            byte => decoded.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

fn respond_text(request: Request, status: u16, content_type: &str, body: String) {
    let response = Response::from_string(body)
        .with_status_code(status)
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header(
            "Content-Security-Policy",
            "default-src 'self'; img-src 'self' https: data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'self'; frame-ancestors 'none'",
        ));
    request.respond(response).ok();
}

fn respond_json(request: Request, status: u16, value: &Value) {
    respond_text(
        request,
        status,
        "application/json; charset=utf-8",
        value.to_string(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_strings_decode_percent_escapes_and_plus_signs() {
        let query = parse_query("at=0%2F1%7Ca&q=hello+world&bad=%zz");
        assert_eq!(query["at"], "0/1|a");
        assert_eq!(query["q"], "hello world");
        assert_eq!(query["bad"], "%zz");
    }

    #[test]
    fn excerpts_skip_headings_markers_and_code() {
        assert_eq!(
            excerpt("```\ncode\n```\n\n> ## Quote line\nrest"),
            "Quote line"
        );
        assert_eq!(
            excerpt("A **bold** `code` see [[0/1]]"),
            "A bold code see 0/1"
        );
        assert_eq!(excerpt(""), "");
    }

    #[test]
    fn options_accept_port_and_no_open() {
        let options = parse_options(&["--port".into(), "9000".into(), "--no-open".into()]).unwrap();
        assert_eq!((options.port, options.open), (Some(9000), false));
        assert!(parse_options(&["--bogus".into()]).is_err());
    }
}
