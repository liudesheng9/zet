//! `zt gui`: a local, browser-based view of the card space.
//!
//! The server binds to the loopback interface only, answers API calls that
//! carry the per-launch token embedded in its page, and performs every change
//! through the same Card operations as the shell commands and the Session.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::Mutex;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::link::{LinkBody, LinkStatus};
use crate::model::Facet;
use crate::ops::{self, Snapshot, TypeChange};
use crate::{Access, CardType, Workspace};

/// The GUI is the human's own tool and may change both Workspaces.
const ACCESS: Access = Access::Owner;

const INDEX_HTML: &str = include_str!("gui/index.html");
const APP_CSS: &str = include_str!("gui/app.css");
const APP_JS: &str = include_str!("gui/app.js");
const LAYOUT_JS: &str = include_str!("gui/layout.js");
const CHART_JS: &str = include_str!("gui/chart-frame.js");
const KATEX_JS: &str = include_str!("gui/vendor/katex/katex.min.js");
const KATEX_CSS: &str = include_str!("gui/vendor/katex/katex.min.css");

/// KaTeX's fonts, served next to its stylesheet so math renders offline.
macro_rules! katex_fonts {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!("gui/vendor/katex/fonts/", $name)))),*]
    };
}
const KATEX_FONTS: &[(&str, &[u8])] = katex_fonts![
    "KaTeX_AMS-Regular.woff2",
    "KaTeX_Caligraphic-Bold.woff2",
    "KaTeX_Caligraphic-Regular.woff2",
    "KaTeX_Fraktur-Bold.woff2",
    "KaTeX_Fraktur-Regular.woff2",
    "KaTeX_Main-Bold.woff2",
    "KaTeX_Main-BoldItalic.woff2",
    "KaTeX_Main-Italic.woff2",
    "KaTeX_Main-Regular.woff2",
    "KaTeX_Math-BoldItalic.woff2",
    "KaTeX_Math-Italic.woff2",
    "KaTeX_SansSerif-Bold.woff2",
    "KaTeX_SansSerif-Italic.woff2",
    "KaTeX_SansSerif-Regular.woff2",
    "KaTeX_Script-Regular.woff2",
    "KaTeX_Size1-Regular.woff2",
    "KaTeX_Size2-Regular.woff2",
    "KaTeX_Size3-Regular.woff2",
    "KaTeX_Size4-Regular.woff2",
    "KaTeX_Typewriter-Regular.woff2",
];

/// How many rendered charts the server keeps for `/chart/<id>`.
const CHART_CAPACITY: usize = 512;
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
    let app = App {
        root,
        token,
        port,
        charts: Charts::default(),
    };
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
    charts: Charts,
}

/// Chart pages this server rendered, by content id. A Card's HTML chart runs
/// in a sandboxed frame loaded from `/chart/<id>`: an opaque origin with its
/// own policy, so a chart's scripts never reach the GUI page or its token.
/// Only charts of Cards the page has read are kept, and an id is the hash of
/// the chart page, so it cannot be guessed.
#[derive(Default)]
struct Charts(Mutex<(BTreeMap<String, String>, VecDeque<String>)>);

impl Charts {
    fn add(&self, page: String) -> String {
        use sha2::{Digest, Sha256};
        let id = Sha256::digest(page.as_bytes())
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut charts = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        let (pages, order) = &mut *charts;
        if pages.insert(id.clone(), page).is_none() {
            order.push_back(id.clone());
            while order.len() > CHART_CAPACITY {
                if let Some(old) = order.pop_front() {
                    pages.remove(&old);
                }
            }
        }
        id
    }

    fn get(&self, id: &str) -> Option<String> {
        let charts = self.0.lock().unwrap_or_else(|poison| poison.into_inner());
        charts.0.get(id).cloned()
    }
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
            (Method::Get, "/layout.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    LAYOUT_JS.to_string(),
                );
                return Flow::Continue;
            }
            (Method::Get, "/vendor/katex/katex.min.js") => {
                respond_asset(
                    request,
                    "text/javascript; charset=utf-8",
                    KATEX_JS.as_bytes(),
                );
                return Flow::Continue;
            }
            (Method::Get, "/vendor/katex/katex.min.css") => {
                respond_asset(request, "text/css; charset=utf-8", KATEX_CSS.as_bytes());
                return Flow::Continue;
            }
            (Method::Get, path) if path.starts_with("/vendor/katex/fonts/") => {
                let name = &path["/vendor/katex/fonts/".len()..];
                match KATEX_FONTS.iter().find(|(font, _)| *font == name) {
                    Some((_, bytes)) => respond_asset(request, "font/woff2", bytes),
                    None => respond_text(request, 404, "text/plain", "not found".to_string()),
                }
                return Flow::Continue;
            }
            (Method::Get, path) if path.starts_with("/chart/") => {
                let id = &path["/chart/".len()..];
                match self.charts.get(id) {
                    Some(page) => {
                        let theme = match query.get("theme").map(String::as_str) {
                            Some("dark") => "dark",
                            _ => "light",
                        };
                        respond_chart(request, self.port, serve_chart_page(&page, theme));
                    }
                    None => respond_text(
                        request,
                        404,
                        "text/plain",
                        "chart expired; reopen the Card".to_string(),
                    ),
                }
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
        Snapshot::read(&self.root)
    }

    fn overview(&self) -> ApiResult {
        let snapshot = self.snapshot()?;
        let counts = crate::card_counts(&self.root)?;
        let workspaces = Workspace::ALL
            .into_iter()
            .map(|workspace| {
                let counts = crate::workspace::counts(&snapshot.cards, workspace);
                json!({
                    "id": workspace.id(),
                    "label": workspace.label(),
                    "topic_trees": counts.topic_trees,
                    "literature_trees": counts.literature_trees,
                    "cards": counts.cards,
                    "agent_writable": Access::Agent.can_write(workspace),
                })
            })
            .collect::<Vec<_>>();
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
            "workspaces": workspaces,
            "trees": snapshot.trees(),
        }))
    }

    fn tree(&self, tree: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        let root = crate::tree_root_address(tree);
        let Some(root_card) = snapshot.by_address.get(&root) else {
            return Err(ApiError(format!("tree `{tree}` does not exist")));
        };
        let cards = snapshot
            .cards
            .iter()
            .filter(|card| crate::is_tree_member(&card.address, tree))
            .map(|card| snapshot.summary(card))
            .collect::<Vec<_>>();
        Ok(json!({
            "tree": tree,
            "root": root,
            "workspace": root_card.workspace.id(),
            "kind": if root_card.is_lit { "literature" } else { "topic" },
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
        let context = if crate::is_tree_root(card) {
            "tree-root"
        } else {
            "regular"
        };
        Ok(json!({
            "address": card.address,
            "tree": crate::tree_id(&card.address),
            "workspace": card.workspace.id(),
            "kind": ops::kind(card),
            "type": card.card_type.id(),
            "facet": card.facet.map(Facet::id),
            "model": snapshot.model(&card.address),
            "context": context,
            "title": parsed.title,
            "text": card.text,
            "body": parsed.body,
            "html": render_markdown(&parsed.body, &snapshot, &self.charts),
            "bibtex": card.bibtex,
            "citation_key": card.citation_key,
            "parent": snapshot.nav(crate::parent_location(&card.address)),
            "direct": snapshot.nav(crate::direct_successor(&card.address).ok()),
            "sides": snapshot.sides(card),
            "inbound": snapshot.inbound_refs(&card.address),
            "outbound": snapshot.outbound(&card.address),
        }))
    }

    fn search(&self, query: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        Ok(json!({ "results": snapshot.search(query, None, None, 60) }))
    }

    fn broken(&self) -> ApiResult {
        let snapshot = self.snapshot()?;
        Ok(json!({ "broken": snapshot.broken(None)? }))
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
                    "html": render_markdown(&parsed.body, &snapshot, &self.charts),
                    "error": issue,
                })
            }
            Err(error) => json!({
                "title": "",
                "html": render_markdown(&text, &snapshot, &self.charts),
                "error": error.to_string(),
            }),
        })
    }

    fn create(&self, body: &Value) -> ApiResult {
        let kind = str_field(body, "kind")?;
        let text = str_field(body, "text")?;
        let facet = facet_field(body)?.flatten();
        let card_type = match (type_field(body)?, facet) {
            (Some(card_type), Some(_)) if card_type != CardType::Model => {
                return Err(ApiError(
                    "only Model Cards can have a Model facet".to_string(),
                ));
            }
            (_, Some(_)) => CardType::Model,
            (card_type, None) => card_type.unwrap_or_default(),
        };
        let location = match kind.as_str() {
            "topic" if card_type != CardType::Idea => {
                return Err(ApiError(
                    "only Regular Cards can be Event or Model Cards".to_string(),
                ));
            }
            "topic" => ops::create_topic(&self.root, ACCESS, workspace_field(body)?, &text)?,
            "direct" => ops::create_successor(
                &self.root,
                ACCESS,
                ops::Successor::Direct,
                &str_field(body, "at")?,
                card_type,
                facet,
                &text,
            )?,
            "side" => ops::create_successor(
                &self.root,
                ACCESS,
                ops::Successor::Side,
                &str_field(body, "at")?,
                card_type,
                facet,
                &text,
            )?,
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
        let address = ops::create_literature(
            &self.root,
            ACCESS,
            workspace_field(body)?,
            &str_field(body, "bibtex")?,
            &str_field(body, "text")?,
        )?;
        Ok(json!({ "address": address }))
    }

    /// Save a Card's text, its type and facet, or both.
    fn edit(&self, body: &Value) -> ApiResult {
        let change = TypeChange {
            card_type: type_field(body)?,
            facet: facet_field(body)?,
        };
        let text = if change.is_empty() {
            Some(str_field(body, "text")?)
        } else {
            body.get("text").and_then(Value::as_str).map(str::to_string)
        };
        let address = ops::edit(
            &self.root,
            ACCESS,
            &str_field(body, "at")?,
            text.as_deref(),
            change,
            base_field(body),
        )?;
        Ok(json!({ "address": address }))
    }

    fn edit_metadata(&self, body: &Value) -> ApiResult {
        Ok(ops::edit_metadata(
            &self.root,
            ACCESS,
            &str_field(body, "at")?,
            &str_field(body, "bibtex")?,
            base_field(body),
            confirm_field(body),
        )?
        .into_value())
    }

    fn delete(&self, body: &Value) -> ApiResult {
        Ok(ops::delete(
            &self.root,
            ACCESS,
            &str_field(body, "at")?,
            confirm_field(body),
        )?
        .into_value())
    }

    fn move_card(&self, body: &Value) -> ApiResult {
        Ok(ops::move_card(
            &self.root,
            ACCESS,
            &str_field(body, "at")?,
            &str_field(body, "to")?,
            confirm_field(body),
        )?
        .into_value())
    }
}

/// Render Card body Markdown to HTML.
///
/// - `[[target]]` Links become navigable chips classified like the Session does.
/// - TeX in `$...$`, `$$...$$`, `\(...\)`, and `\[...\]` becomes math spans that
///   the page typesets with KaTeX.
/// - A ```` ```chart ```` fence, or a raw HTML block that opens with a chart
///   element (`<figure>`, `<div>`, `<svg>`, `<table>`, …), becomes an HTML
///   chart: a sandboxed frame served from `/chart/<id>`.
/// - Other raw HTML is shown as text, except a few plain inline tags
///   (`<sub>`, `<sup>`, `<br>`, `<kbd>`, `<mark>`, …).
fn render_markdown(body: &str, snapshot: &Snapshot, charts: &Charts) -> String {
    use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH;
    let source = fence_html_charts(&normalize_math_delimiters(body));
    let mut events: Vec<Event> = Vec::new();
    let mut pending = String::new();
    let mut in_code_block = false;
    let mut chart: Option<String> = None;
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
    for event in Parser::new_ext(&source, options) {
        if let Some(chart_source) = chart.as_mut() {
            match event {
                Event::Text(text) => chart_source.push_str(&text),
                Event::End(TagEnd::CodeBlock) => {
                    let page = chart_page(chart_source, snapshot);
                    let id = charts.add(page);
                    events.push(Event::Html(
                        format!(
                            "<figure class=\"zt-chart\"><iframe class=\"zt-chart-frame\" data-chart=\"{id}\" loading=\"lazy\" title=\"Chart\"></iframe></figure>\n"
                        )
                        .into(),
                    ));
                    chart = None;
                }
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) if is_chart_info(&info) => {
                flush(&mut pending, &mut events, in_code_block);
                chart = Some(String::new());
            }
            Event::Text(text) => pending.push_str(&text),
            Event::InlineHtml(raw) if is_plain_inline_tag(&raw) => {
                flush(&mut pending, &mut events, in_code_block);
                events.push(Event::InlineHtml(raw.to_ascii_lowercase().into()));
            }
            // An HTML comment is a note to editors, not text.
            Event::Html(raw) | Event::InlineHtml(raw) if is_html_comment(&raw) => {}
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

/// A fence opens a chart when its info string is `chart` or `html chart`.
fn is_chart_info(info: &str) -> bool {
    let mut words = info.split_whitespace();
    match words.next() {
        Some("chart") => true,
        Some("html") => words.any(|word| word == "chart"),
        _ => false,
    }
}

fn is_html_comment(raw: &str) -> bool {
    let raw = raw.trim();
    raw.starts_with("<!--") && raw.ends_with("-->")
}

/// `<sub>`, `</sup>`, `<br/>`: an inline tag without attributes that only
/// styles text.
fn is_plain_inline_tag(raw: &str) -> bool {
    const TAGS: [&str; 14] = [
        "sub", "sup", "br", "kbd", "mark", "small", "u", "ins", "del", "s", "b", "i", "em",
        "strong",
    ];
    let Some(inner) = raw
        .trim()
        .strip_prefix('<')
        .and_then(|rest| rest.strip_suffix('>'))
    else {
        return false;
    };
    let inner = inner.strip_prefix('/').unwrap_or(inner);
    let inner = inner.strip_suffix('/').unwrap_or(inner).trim();
    TAGS.iter().any(|tag| inner.eq_ignore_ascii_case(tag))
}

/// Rewrite `\(...\)` as `$...$` and `\[...\]` as `$$...$$` in Markdown prose,
/// so TeX written with LaTeX delimiters renders like TeX written with dollars.
fn normalize_math_delimiters(body: &str) -> String {
    if !body.contains("\\(") && !body.contains("\\[") {
        return body.to_string();
    }
    let code = crate::link::code_ranges(body);
    let mut output = String::with_capacity(body.len());
    let mut offset = 0;
    for range in code
        .iter()
        .chain(std::iter::once(&(body.len()..body.len())))
    {
        if range.start > offset {
            output.push_str(&convert_tex_delimiters(&body[offset..range.start]));
        }
        output.push_str(&body[range.start.max(offset)..range.end]);
        offset = offset.max(range.end);
    }
    output
}

fn convert_tex_delimiters(prose: &str) -> String {
    let mut output = String::with_capacity(prose.len());
    let mut rest = prose;
    loop {
        let inline = rest.find("\\(");
        let display = rest.find("\\[");
        let (start, open, close, dollars) = match (inline, display) {
            (Some(i), Some(d)) if d < i => (d, "\\[", "\\]", "$$"),
            (Some(i), _) => (i, "\\(", "\\)", "$"),
            (None, Some(d)) => (d, "\\[", "\\]", "$$"),
            (None, None) => break,
        };
        // `\\(` is an escaped backslash followed by a parenthesis, not TeX.
        if start > 0 && rest[..start].ends_with('\\') {
            output.push_str(&rest[..start + open.len()]);
            rest = &rest[start + open.len()..];
            continue;
        }
        let inner_start = start + open.len();
        let Some(length) = rest[inner_start..].find(close) else {
            break;
        };
        let inner = rest[inner_start..inner_start + length].trim();
        output.push_str(&rest[..start]);
        if dollars == "$$" {
            output.push_str(&format!("$${inner}$$"));
        } else {
            output.push_str(&format!("${inner}$"));
        }
        rest = &rest[inner_start + length + close.len()..];
    }
    output.push_str(rest);
    output
}

/// Elements that open an HTML chart when a raw HTML block starts with them.
const CHART_TAGS: [&str; 9] = [
    "figure", "div", "svg", "table", "section", "canvas", "script", "style", "iframe",
];

/// Wrap raw HTML charts in ```` ```chart ```` fences. A chart runs from a line
/// that opens a chart element to the line that closes it, so blank lines
/// inside it do not cut it the way they cut a Markdown HTML block; charts
/// separated only by blank lines (a `<div>` and its `<script>`) are one chart.
fn fence_html_charts(body: &str) -> String {
    let lines: Vec<&str> = body.split_inclusive('\n').collect();
    let fenced = crate::link::fenced_code_ranges(body);
    let mut starts = Vec::with_capacity(lines.len());
    let mut at = 0;
    for line in &lines {
        starts.push(at);
        at += line.len();
    }
    let in_fence = |index: usize| fenced.iter().any(|range| range.contains(&starts[index]));
    let mut output = String::with_capacity(body.len() + 32);
    let mut index = 0;
    while index < lines.len() {
        if in_fence(index) || chart_tag(lines[index]).is_none() {
            output.push_str(lines[index]);
            index += 1;
            continue;
        }
        let mut end = chart_end(&lines, index);
        // Join the next chart element when only blank lines come between.
        loop {
            let mut next = end;
            while next < lines.len() && lines[next].trim().is_empty() {
                next += 1;
            }
            if next < lines.len() && chart_tag(lines[next]).is_some() && !in_fence(next) {
                end = chart_end(&lines, next);
            } else {
                break;
            }
        }
        let chart = lines[index..end].concat();
        let ticks = longest_backtick_run(&chart).max(2) + 1;
        let fence = "`".repeat(ticks);
        output.push_str(&format!(
            "{fence}chart\n{}",
            chart.trim_end_matches(['\n', '\r'])
        ));
        output.push_str(&format!("\n{fence}\n"));
        index = end;
    }
    output
}

/// The chart element a line opens, if it starts (after at most three spaces
/// of indent) with `<tag` for a chart tag.
fn chart_tag(line: &str) -> Option<&'static str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.trim_start().strip_prefix('<')?;
    CHART_TAGS.into_iter().find(|tag| {
        rest.len() >= tag.len()
            && rest[..tag.len()].eq_ignore_ascii_case(tag)
            && rest[tag.len()..]
                .chars()
                .next()
                .is_none_or(|ch| ch.is_whitespace() || ch == '>' || ch == '/')
    })
}

/// The index after the line that closes the chart element opened at `start`.
/// An element that never closes ends at the first blank line, as a Markdown
/// HTML block would.
fn chart_end(lines: &[&str], start: usize) -> usize {
    let tag = chart_tag(lines[start]).expect("a chart line");
    let open = format!("<{tag}");
    let close = format!("</{tag}");
    let mut depth: isize = 0;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let lower = line.to_ascii_lowercase();
        depth += count_tags(&lower, &open) as isize;
        depth -= lower.matches(close.as_str()).count() as isize;
        // `<svg ... />` closes itself.
        if index == start && lower.trim_end().ends_with("/>") && depth == 1 {
            depth = 0;
        }
        if depth <= 0 {
            return index + 1;
        }
    }
    lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| line.trim().is_empty())
        .map_or(lines.len(), |(index, _)| index)
}

/// Occurrences of `<tag` followed by a space, `>`, `/`, or the line end.
fn count_tags(lower: &str, open: &str) -> usize {
    lower
        .match_indices(open)
        .filter(|(at, _)| {
            lower[at + open.len()..]
                .chars()
                .next()
                .is_none_or(|ch| ch.is_whitespace() || ch == '>' || ch == '/')
        })
        .count()
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    longest
}

/// The page a chart frame loads: the chart's HTML in the card's typography
/// and colors, KaTeX when the chart holds TeX, and the titles of the Cards it
/// names with `[[address]]`. The store keeps this small template;
/// [`serve_chart_page`] fills in the theme and inlines the scripts.
fn chart_page(source: &str, snapshot: &Snapshot) -> String {
    let mut links = serde_json::Map::new();
    let mut rest = source;
    while let Some(start) = rest.find("[[") {
        let Some(length) = rest[start + 2..].find("]]") else {
            break;
        };
        let target = &rest[start + 2..start + 2 + length];
        if !target.is_empty() && target.len() < 200 && !target.contains(['\n', '[', ']']) {
            let exists = snapshot.by_address.contains_key(target);
            links.insert(
                target.to_string(),
                json!({ "title": snapshot.title(target), "exists": exists }),
            );
        }
        rest = &rest[start + 2 + length + 2..];
    }
    // `</` would end the script element the data sits in.
    let links = Value::Object(links).to_string().replace("</", "<\\/");
    let katex = if ["$", "\\(", "\\["].iter().any(|mark| source.contains(mark)) {
        CHART_KATEX
    } else {
        ""
    };
    format!(
        "<!doctype html>\n<html lang=\"en\" data-theme=\"{CHART_THEME}\">\n<head>\n<meta charset=\"utf-8\">\n<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n{katex}\n<script>window.ZT_LINKS = {links};</script>\n{CHART_FRAME}\n</head>\n<body>\n<div id=\"zt-chart\">\n{source}\n</div>\n</body>\n</html>\n"
    )
}

const CHART_THEME: &str = "__ZT_THEME__";
const CHART_KATEX: &str = "<!--zt:katex-->";
const CHART_FRAME: &str = "<!--zt:frame-->";

/// A stored chart page as served: its theme, and its scripts inlined. Some
/// embedded browsers refuse every subresource of a sandboxed page, so the
/// page carries the frame script and KaTeX (fonts included) itself.
fn serve_chart_page(page: &str, theme: &str) -> String {
    let page = page.replacen(CHART_THEME, theme, 1);
    let page = page.replacen(CHART_FRAME, &format!("<script>{CHART_JS}</script>"), 1);
    if page.contains(CHART_KATEX) {
        page.replacen(
            CHART_KATEX,
            &format!(
                "<style>{}</style>\n<script>{KATEX_JS}</script>",
                katex_inline_css()
            ),
            1,
        )
    } else {
        page
    }
}

/// KaTeX's stylesheet with its WOFF2 fonts as data URIs.
fn katex_inline_css() -> &'static str {
    static CSS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CSS.get_or_init(|| {
        KATEX_FONTS
            .iter()
            .fold(KATEX_CSS.to_string(), |css, (name, bytes)| {
                let base = name.trim_end_matches(".woff2");
                css.replace(
                    &format!(
                        "url(fonts/{base}.woff2) format(\"woff2\"),url(fonts/{base}.woff) format(\"woff\"),url(fonts/{base}.ttf) format(\"truetype\")"
                    ),
                    &format!("url(data:font/woff2;base64,{}) format(\"woff2\")", base64(bytes)),
                )
            })
    })
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, byte)| n | (u32::from(*byte) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[(n >> (18 - 6 * i)) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
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

fn str_field(body: &Value, name: &str) -> Result<String> {
    body.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing `{name}` field"))
}

fn base_field(body: &Value) -> Option<&str> {
    body.get("base").and_then(Value::as_str)
}

/// The Workspace for a new tree: the human's unless the request names one.
fn workspace_field(body: &Value) -> Result<Workspace> {
    body.get("workspace")
        .and_then(Value::as_str)
        .map_or(Ok(Workspace::Human), Workspace::parse)
}

/// An optional Card type: `idea`, `event`, or `model`.
fn type_field(body: &Value) -> Result<Option<CardType>> {
    body.get("type")
        .and_then(Value::as_str)
        .map(CardType::parse)
        .transpose()
}

/// An optional Model facet: absent leaves it alone, `""` or null clears it.
fn facet_field(body: &Value) -> Result<Option<Option<Facet>>> {
    match body.get("facet") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(value)) => crate::model::parse_optional(value).map(Some),
        Some(_) => bail!("`facet` must be a string"),
    }
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

/// A static asset of the GUI page, such as KaTeX and its fonts.
fn respond_asset(request: Request, content_type: &str, body: &[u8]) {
    let response = Response::from_data(body.to_vec())
        .with_header(header("Content-Type", content_type))
        .with_header(header("Cache-Control", "max-age=86400"))
        .with_header(header("X-Content-Type-Options", "nosniff"));
    request.respond(response).ok();
}

/// A chart page, framed only by the GUI page. It may run its own scripts and
/// load libraries over HTTPS, but its policy sandboxes it into an opaque
/// origin: it cannot reach the GUI page or its token, and the API answers no
/// preflights. The sandbox travels with the response rather than the frame's
/// `sandbox` attribute, which some embedded browsers refuse to load.
fn respond_chart(request: Request, port: u16, page: String) {
    let local = format!("http://127.0.0.1:{port} http://localhost:{port}");
    let response = Response::from_string(page)
        .with_header(header("Content-Type", "text/html; charset=utf-8"))
        .with_header(header("Cache-Control", "no-store"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header(
            "Content-Security-Policy",
            &format!(
                "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' {local} https:; style-src 'unsafe-inline' {local} https:; font-src {local} https: data:; img-src {local} https: data: blob:; connect-src https:; frame-ancestors {local}; form-action 'none'; base-uri 'none'"
            ),
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
    fn options_accept_port_and_no_open() {
        let options = parse_options(&["--port".into(), "9000".into(), "--no-open".into()]).unwrap();
        assert_eq!((options.port, options.open), (Some(9000), false));
        assert!(parse_options(&["--bogus".into()]).is_err());
    }
}

#[cfg(test)]
mod render_tests {
    use super::*;
    use crate::ops::Successor;

    fn snapshot() -> (tempfile::TempDir, Snapshot) {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        crate::initialize_database(root).expect("initialize database");
        ops::create_topic(root, Access::Owner, Workspace::Human, "# Firms\n").unwrap();
        ops::create_successor(
            root,
            Access::Owner,
            Successor::Direct,
            "0/0",
            CardType::Idea,
            None,
            "# Ownership matters\n\nbody\n",
        )
        .unwrap();
        let snapshot = Snapshot::read(root).expect("snapshot");
        (temp, snapshot)
    }

    #[test]
    fn tex_in_dollars_and_latex_delimiters_becomes_math_spans() {
        let (_temp, snapshot) = snapshot();
        let charts = Charts::default();
        let html = render_markdown(
            "Under-investment $i<i^*$ with $x_i * y_i$ and\n\n$$R'(i^*)=1$$\n\nalso \\(a_b\\) and \\[\\sum_i c_i\\]",
            &snapshot,
            &charts,
        );
        assert!(
            html.contains(r#"<span class="math math-inline">i&lt;i^*</span>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="math math-inline">x_i * y_i</span>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="math math-display">R&#39;(i^*)=1</span>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="math math-inline">a_b</span>"#),
            "{html}"
        );
        assert!(
            html.contains(r#"<span class="math math-display">\sum_i c_i</span>"#),
            "{html}"
        );
        // Code keeps its dollars and backslashes.
        let code = render_markdown(r"`$x$` and `\(y\)`", &snapshot, &charts);
        assert!(code.contains("<code>$x$</code>"), "{code}");
        assert!(code.contains(r"<code>\(y\)</code>"), "{code}");
    }

    #[test]
    fn chart_fences_and_raw_html_charts_become_sandboxed_frames() {
        let (_temp, snapshot) = snapshot();
        let charts = Charts::default();
        let html = render_markdown(
            "```chart\n<div class=\"c\">[[0/1]] then [[9/9]]</div>\n<script>const d = [[1, 2], [3, 4]];</script>\n```",
            &snapshot,
            &charts,
        );
        assert_eq!(html.matches("zt-chart-frame").count(), 1, "{html}");
        let id = html
            .split("data-chart=\"")
            .nth(1)
            .and_then(|rest| rest.split('"').next())
            .expect("chart id");
        let page = charts.get(id).expect("stored chart page");
        assert!(page.contains("<div class=\"c\">[[0/1]] then [[9/9]]</div>"));
        assert!(page.contains("const d = [[1, 2], [3, 4]];"));
        assert!(
            page.contains(r#""0/1":{"exists":true,"title":"Ownership matters"}"#),
            "{page}"
        );
        assert!(
            page.contains(r#""9/9":{"exists":false,"title":""}"#),
            "{page}"
        );
        assert!(page.contains("data-theme=\"__ZT_THEME__\""));
        // No TeX in this chart, so it is served without KaTeX.
        let served = serve_chart_page(&page, "dark");
        assert!(served.contains("data-theme=\"dark\"") && served.contains("postMessage"));
        assert!(!served.contains("KaTeX_Main"));

        // A raw HTML chart runs to its closing tag across blank lines, and takes the
        // script that follows it into the same frame.
        let html = render_markdown(
            "Intro\n\n<figure>\n<svg viewBox=\"0 0 10 10\"></svg>\n\n<figcaption>Timing</figcaption>\n</figure>\n\n<script>draw()</script>\n\nAfter",
            &snapshot,
            &charts,
        );
        assert_eq!(html.matches("zt-chart-frame").count(), 1, "{html}");
        assert!(
            html.contains("<p>Intro</p>") && html.contains("<p>After</p>"),
            "{html}"
        );
        assert!(!html.contains("figcaption"), "{html}");

        // A fenced html block without `chart` stays code.
        let code = render_markdown("```html\n<div>x</div>\n```", &snapshot, &charts);
        assert!(code.contains("&lt;div&gt;x&lt;/div&gt;"), "{code}");
        assert!(!code.contains("zt-chart"), "{code}");
    }

    #[test]
    fn plain_inline_tags_render_other_html_shows_as_text_and_comments_vanish() {
        let (_temp, snapshot) = snapshot();
        let html = render_markdown(
            "H<sub>2</sub>O<br> <span onclick=\"x\">s</span> [[0/1]] <!-- editor note -->",
            &snapshot,
            &Charts::default(),
        );
        assert!(html.contains("H<sub>2</sub>O<br>"), "{html}");
        assert!(
            html.contains("&lt;span onclick=\"x\"&gt;s&lt;/span&gt;"),
            "{html}"
        );
        assert!(html.contains("class=\"zt-link\""), "{html}");
        assert!(!html.contains("editor note"), "{html}");
    }

    #[test]
    fn latex_delimiters_skip_escaped_backslashes_and_code() {
        assert_eq!(
            normalize_math_delimiters(r"a \(x\) b ```\(c\)``` \\(d)"),
            r"a $x$ b ```\(c\)``` \\(d)"
        );
        assert_eq!(normalize_math_delimiters(r"\[ y \]"), "$$y$$");
        assert_eq!(normalize_math_delimiters(r"open \( only"), r"open \( only");
    }

    #[test]
    fn served_chart_pages_inline_katex_with_its_fonts_when_the_chart_holds_tex() {
        let (_temp, snapshot) = snapshot();
        let served = serve_chart_page(&chart_page("<p>$x^2$</p>", &snapshot), "light");
        assert!(served.contains("katex"));
        assert!(served.contains("url(data:font/woff2;base64,"));
        assert!(!served.contains("url(fonts/"));
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }

    #[test]
    fn chart_store_forgets_the_oldest_pages_past_its_capacity() {
        let charts = Charts::default();
        let first = charts.add("page 0".to_string());
        assert_eq!(charts.add("page 0".to_string()), first);
        for n in 1..=CHART_CAPACITY {
            charts.add(format!("page {n}"));
        }
        assert!(charts.get(&first).is_none());
        assert!(charts.get(&charts.add("page 1".to_string())).is_some());
    }
}
