//! The GUI: a local, browser-based view of the card space.
//!
//! The service daemon runs the GUI server for its whole life, so the GUI is up exactly while
//! the service is; `zt gui` only tells where it listens. The server binds to the loopback
//! interface only, answers API calls that carry the per-service token embedded in its page,
//! and performs every change through the same Card operations as the shell commands and the
//! Session.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::link::{LinkBody, LinkStatus};
use crate::model::{Emphasis, Facet};
use crate::ops::{self, Snapshot, TypeChange};
use crate::{Access, CardType, Workspace};

mod document;

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
/// How many hits the search tray asks for, in either mode.
const SEARCH_LIMIT: usize = 80;
const DEFAULT_PORT: u16 = 4717;
/// Overrides the port the service's GUI listens on; `0` lets the system choose one.
const PORT_ENV: &str = "ZT_GUI_PORT";
const TOKEN_HEADER: &str = "X-ZT-Token";

/// `zt gui [--open]`: tell where the service's GUI listens. The GUI runs inside the service,
/// so it is up while the service is up; this command starts nothing but, with `--open`, a
/// window onto it.
pub(crate) fn run(args: &[String]) -> Result<()> {
    let open = match args {
        [] => false,
        [flag] if flag == "--open" => true,
        _ => bail!(
            "usage: zt gui [--open]; the GUI runs with the service, so `--port` and `--no-open` are gone (set {PORT_ENV} before `zt up` to choose the port)"
        ),
    };
    let root = crate::require_service_up()?;
    let Some(url) = recorded_url(&root) else {
        bail!(
            "the service has no GUI; it may predate zt 0.14 (restart it with `zt down` and `zt up`) or its GUI could not start (see {})",
            crate::log_path(&root).display()
        );
    };
    println!("zt gui: {url}");
    if let Some(port) = url_port(&url) {
        println!("port: {port}");
    }
    if open {
        open_window(&url);
    }
    Ok(())
}

/// The URL of the running service's GUI, as its daemon recorded it.
pub(crate) fn recorded_url(root: &Path) -> Option<String> {
    let url = fs::read_to_string(record_path(root)).ok()?;
    let url = url.trim();
    (!url.is_empty()).then(|| url.to_string())
}

fn url_port(url: &str) -> Option<u16> {
    url.strip_prefix("http://127.0.0.1:")?
        .trim_end_matches('/')
        .parse()
        .ok()
}

fn record_path(root: &Path) -> PathBuf {
    root.join("zt.gui")
}

/// Forget the GUI's address; the daemon that recorded it is gone or going.
pub(crate) fn clear_record(root: &Path) {
    let _ = fs::remove_file(record_path(root));
}

/// The GUI server a service daemon runs for its whole life.
pub(crate) struct Running {
    server: Arc<Server>,
    worker: JoinHandle<()>,
    root: PathBuf,
}

/// Start the GUI server inside the service daemon and record its URL in the archive.
pub(crate) fn start(root: &Path) -> Result<Running> {
    let server = Arc::new(bind(configured_port()?)?);
    let port = server
        .server_addr()
        .to_ip()
        .context("GUI server is not bound to an IP address")?
        .port();
    let url = format!("http://127.0.0.1:{port}/");
    let app = App {
        root: root.to_path_buf(),
        token: random_token()?,
        port,
        charts: Charts::default(),
        snapshots: Snapshots::new(root),
    };
    fs::write(record_path(root), format!("{url}\n"))
        .with_context(|| format!("failed to write {}", record_path(root).display()))?;
    let worker = thread::Builder::new()
        .name("zt-gui".to_string())
        .spawn({
            let server = Arc::clone(&server);
            move || {
                for request in server.incoming_requests() {
                    // A fault in one request must not take the GUI down with it.
                    let _ = panic::catch_unwind(AssertUnwindSafe(|| app.handle(request)));
                }
            }
        })
        .context("failed to start the GUI thread")?;
    crate::log_event(root, &format!("gui started on {url}")).ok();
    Ok(Running {
        server,
        worker,
        root: root.to_path_buf(),
    })
}

impl Running {
    /// Stop taking requests, let the one in hand finish, and forget the URL.
    pub(crate) fn stop(self) {
        clear_record(&self.root);
        self.server.unblock();
        let _ = self.worker.join();
        crate::log_event(&self.root, "gui stopped").ok();
    }
}

fn configured_port() -> Result<Option<u16>> {
    match std::env::var(PORT_ENV) {
        Ok(value) if !value.trim().is_empty() => value
            .trim()
            .parse()
            .map(Some)
            .with_context(|| format!("invalid {PORT_ENV} `{value}`")),
        _ => Ok(None),
    }
}

fn bind(port: Option<u16>) -> Result<Server> {
    let attempt = |port: u16| Server::http(("127.0.0.1", port));
    match port {
        Some(port) => attempt(port)
            .map_err(|error| anyhow::anyhow!("cannot listen on 127.0.0.1:{port}: {error}")),
        None => {
            // A service that just stopped may hold the default port a moment longer; wait for
            // it, so a restarted GUI keeps its address.
            for _ in 0..10 {
                if let Ok(server) = attempt(DEFAULT_PORT) {
                    return Ok(server);
                }
                thread::sleep(Duration::from_millis(100));
            }
            attempt(0).map_err(|error| anyhow::anyhow!("cannot start the GUI server: {error}"))
        }
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

struct App {
    root: PathBuf,
    token: String,
    port: u16,
    charts: Charts,
    snapshots: Snapshots,
}

/// The last Snapshot read, kept until the Cards change. Reading and parsing every Card takes
/// a few hundred milliseconds, too slow to repeat for each Card the page shows. A connection
/// held open for the purpose asks SQLite's `data_version`, which moves whenever any other
/// connection commits, so writes from the CLI, a Session, or the MCP server show up on the
/// next request just as the GUI's own do.
struct Snapshots {
    root: PathBuf,
    cached: Mutex<Option<Cached>>,
}

struct Cached {
    conn: rusqlite::Connection,
    version: i64,
    snapshot: Arc<Snapshot>,
    /// The overview of this Snapshot, which the page asks for every few seconds to notice
    /// changes; made on the first ask.
    overview: Option<Value>,
}

impl Snapshots {
    fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            cached: Mutex::new(None),
        }
    }

    fn get(&self) -> Result<Arc<Snapshot>> {
        let mut cached = self.lock();
        Ok(Arc::clone(&self.current(&mut cached)?.snapshot))
    }

    /// The overview `build` makes of the current Snapshot, built once per Snapshot.
    fn overview(&self, build: impl FnOnce(&Snapshot) -> Result<Value>) -> Result<Value> {
        let mut cached = self.lock();
        let current = self.current(&mut cached)?;
        if let Some(overview) = &current.overview {
            return Ok(overview.clone());
        }
        let overview = build(&current.snapshot)?;
        current.overview = Some(overview.clone());
        Ok(overview)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Cached>> {
        self.cached
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn current<'a>(&self, cached: &'a mut Option<Cached>) -> Result<&'a mut Cached> {
        let conn = match cached.take() {
            Some(held) => match data_version(&held.conn) {
                Ok(version) if version == held.version => return Ok(cached.insert(held)),
                Ok(_) => held.conn,
                // A connection that cannot answer is replaced, not trusted.
                Err(_) => crate::open_db(&self.root)?,
            },
            None => crate::open_db(&self.root)?,
        };
        // Ask before reading: a commit in between makes the next request read again rather
        // than keep a stale Snapshot.
        let version = data_version(&conn)?;
        let snapshot = Arc::new(Snapshot::read(&self.root)?);
        Ok(cached.insert(Cached {
            conn,
            version,
            snapshot,
            overview: None,
        }))
    }
}

fn data_version(conn: &rusqlite::Connection) -> rusqlite::Result<i64> {
    conn.query_row("PRAGMA data_version", [], |row| row.get(0))
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
    fn handle(&self, mut request: Request) {
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
            return;
        }
        let method = request.method().clone();
        match (&method, path.as_str()) {
            (Method::Get, "/") | (Method::Get, "/index.html") => {
                let page = INDEX_HTML.replace("__ZT_TOKEN__", &self.token);
                respond_text(request, 200, "text/html; charset=utf-8", page);
                return;
            }
            (Method::Get, "/app.css") => {
                respond_text(request, 200, "text/css; charset=utf-8", APP_CSS.to_string());
                return;
            }
            (Method::Get, "/app.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    APP_JS.to_string(),
                );
                return;
            }
            (Method::Get, "/layout.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    LAYOUT_JS.to_string(),
                );
                return;
            }
            (Method::Get, "/vendor/katex/katex.min.js") => {
                respond_asset(
                    request,
                    "text/javascript; charset=utf-8",
                    KATEX_JS.as_bytes(),
                );
                return;
            }
            (Method::Get, "/vendor/katex/katex.min.css") => {
                respond_asset(request, "text/css; charset=utf-8", KATEX_CSS.as_bytes());
                return;
            }
            (Method::Get, path) if path.starts_with("/vendor/katex/fonts/") => {
                let name = &path["/vendor/katex/fonts/".len()..];
                match KATEX_FONTS.iter().find(|(font, _)| *font == name) {
                    Some((_, bytes)) => respond_asset(request, "font/woff2", bytes),
                    None => respond_text(request, 404, "text/plain", "not found".to_string()),
                }
                return;
            }
            (Method::Get, path) if path.starts_with("/artifact/") => {
                match crate::artifact::read(&self.root, &path["/artifact/".len()..]) {
                    Some((bytes, media_type)) => respond_artifact(request, media_type, bytes),
                    None => respond_text(
                        request,
                        404,
                        "text/plain",
                        "artifact not in this archive".to_string(),
                    ),
                }
                return;
            }
            (Method::Get, "/doc.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    document::DOC_JS.to_string(),
                );
                return;
            }
            (Method::Get, path)
                if path.starts_with("/scholar/file/") || path.starts_with("/scholar/figure/") =>
            {
                document::serve(request, path.to_string(), query);
                return;
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
                return;
            }
            _ => {}
        }
        if !path.starts_with("/api/") {
            respond_text(request, 404, "text/plain", "not found".to_string());
            return;
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
            return;
        }
        let body = if method == Method::Post {
            let mut raw = String::new();
            if request.as_reader().read_to_string(&mut raw).is_err() {
                respond_json(
                    request,
                    400,
                    &json!({ "error": "request body must be UTF-8" }),
                );
                return;
            }
            match serde_json::from_str::<Value>(if raw.is_empty() { "{}" } else { &raw }) {
                Ok(value) => value,
                Err(error) => {
                    respond_json(
                        request,
                        400,
                        &json!({ "error": format!("invalid JSON: {error}") }),
                    );
                    return;
                }
            }
        } else {
            Value::Null
        };
        let result = self.route(&method, &path, &query, &body);
        match result {
            Ok(value) => respond_json(request, 200, &value),
            Err(ApiError(message)) => respond_json(request, 400, &json!({ "error": message })),
        }
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
            // The page's heartbeat: this service is still up and its token still holds.
            (Method::Get, "/api/ping") => Ok(json!({ "ok": true })),
            (Method::Get, "/api/overview") => self.overview(),
            (Method::Get, "/api/tree") => self.tree(&param("id")?),
            (Method::Get, "/api/card") => self.card(&param("at")?),
            (Method::Get, "/api/graph") => self.graph(),
            (Method::Get, "/api/register") => self.register(query),
            (Method::Get, "/api/search") => self.search(
                &param("q")?,
                query.get("ws").map(String::as_str),
                query.get("type").map(String::as_str),
                query.get("mode").map(String::as_str),
            ),
            (Method::Get, "/api/broken") => self.broken(),
            (Method::Post, "/api/render") => self.render(body),
            (Method::Post, "/api/create") => self.create(body),
            (Method::Post, "/api/literature/check") => self.literature_check(body),
            (Method::Post, "/api/literature") => self.create_literature(body),
            (Method::Post, "/api/edit") => self.edit(body),
            (Method::Post, "/api/metadata") => self.edit_metadata(body),
            (Method::Post, "/api/delete") => self.delete(body),
            (Method::Post, "/api/move") => self.move_card(body),
            (Method::Post, "/api/insert") => self.insert_card(body),
            (Method::Get, "/api/artifact") => self.artifact(&param("sha")?),
            (Method::Get, "/api/scholar/works") => self.scholar_works(&param("q")?),
            (Method::Get, "/api/scholar/artifacts") => self.scholar_artifacts(&param("work_id")?),
            (Method::Get, "/api/scholar/files") => self.scholar_files(&param("work_id")?),
            (Method::Post, "/api/scholar/preview") => self.scholar_preview(body),
            (Method::Post, "/api/scholar/artifact") => self.scholar_keep(body),
            (Method::Get, "/api/spreads") => self.spreads(),
            (Method::Post, "/api/spreads") => self.save_spreads(body),
            _ => Err(ApiError(format!("unknown endpoint {path}"))),
        }
    }

    fn snapshot(&self) -> Result<Arc<Snapshot>> {
        self.snapshots.get()
    }

    fn overview(&self) -> ApiResult {
        Ok(self.snapshots.overview(|snapshot| {
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
        })?)
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

    /// The concept register's flattened entry view (`src/entry.rs`): the same reading the MCP
    /// `get_register` tool returns, from one coherent read, so both clients show one contract.
    fn register(&self, query: &BTreeMap<String, String>) -> ApiResult {
        let workspace = query
            .get("ws")
            .filter(|value| !value.is_empty() && *value != "all")
            .map(|value| crate::Workspace::parse(value))
            .transpose()?
            .ok_or_else(|| ApiError("missing `ws` parameter (human or agent)".into()))?;
        let number = |name: &str| -> Result<Option<u64>, ApiError> {
            match query.get(name) {
                None => Ok(None),
                Some(value) => value
                    .parse::<u64>()
                    .map(Some)
                    .map_err(|_| ApiError(format!("`{name}` must be a whole number"))),
            }
        };
        let args = json!({
            "entry_offset": number("entry_offset")?,
            "entry_limit": number("entry_limit")?,
        });
        let entries = crate::entry::EntryArgs::from_args(&args).map_err(ApiError::from)?;
        Ok(crate::read::register(&self.root, workspace, &entries)?)
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
            "facet": card.facet.as_ref().map(Facet::id),
            "emphasis": card.emphasis.map(Emphasis::id),
            "level": card.level_id(),
            "form": card.form.map(crate::math::MathForm::id),
            "model": snapshot.model(&card.address),
            "hubs": crate::hub::hubs_of(&snapshot, &card.address),
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

    /// Search hits for the tray: the existing plain lexical order (`All matches`), or the entry-first
    /// ranking shared with the MCP server's `search_cards mode: entry` (D198, D278). Either way the
    /// answer names the effective `mode`. An empty query stays the existing listing: it is plain,
    /// whatever mode was asked for.
    fn search(
        &self,
        query: &str,
        workspace: Option<&str>,
        card_type: Option<&str>,
        mode: Option<&str>,
    ) -> ApiResult {
        let workspace = workspace
            .filter(|value| !value.is_empty() && *value != "all")
            .map(crate::Workspace::parse)
            .transpose()?;
        let card_type = card_type
            .filter(|value| !value.is_empty() && *value != "all")
            .map(crate::CardType::parse)
            .transpose()?;
        let mode = match mode.filter(|value| !value.is_empty()) {
            None => crate::entry::Mode::Plain,
            Some(value) => crate::entry::Mode::parse(value)?,
        };
        let snapshot = self.snapshot()?;
        if mode == crate::entry::Mode::Entry && !query.trim().is_empty() {
            return Ok(crate::entry::search(
                &snapshot,
                &crate::entry::SearchArgs {
                    query: query.to_string(),
                    workspace,
                    card_type,
                    form: None,
                    limit: SEARCH_LIMIT,
                },
            ));
        }
        let mut value = snapshot.find(query, workspace, card_type, SEARCH_LIMIT);
        value["mode"] = json!(crate::entry::Mode::Plain.id());
        Ok(value)
    }

    /// Where a kept artifact came from.
    fn artifact(&self, digest: &str) -> ApiResult {
        let held = crate::artifact::path(&self.root, digest).is_some();
        let source = crate::artifact::source(&self.root, digest);
        // Its place in the document it came from, which the reading page marks by file stem.
        let in_document = source.as_ref().and_then(|source| {
            let file_id = source["file_id"].as_u64()?;
            let path = source["path"].as_str()?;
            Some(format!(
                "/scholar/file/{file_id}#{}",
                document::figure_anchor(path)
            ))
        });
        Ok(json!({ "sha256": digest, "held": held, "source": source, "in_document": in_document }))
    }

    /// Works in the Scholar Archive with Markdown documents, whose figures a Card can show.
    fn scholar_works(&self, query: &str) -> ApiResult {
        let mut bridge = crate::scholar::Bridge::connect()?;
        let rows = bridge.sql(
            "SELECT w.id, w.title, w.year, w.authors, w.doi FROM works w \
             WHERE w.id IN (SELECT work_id FROM work_files WHERE kind='markdown' AND work_id IS NOT NULL) \
             AND (?1 = '' OR instr(lower(w.title), lower(?1)) > 0 OR instr(lower(coalesce(w.authors,'')), lower(?1)) > 0) \
             ORDER BY w.year DESC, w.title LIMIT 40",
            &json!([query.trim()]),
            40,
            "archive",
        )?;
        let works = rows["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| json!({ "work_id": row[0], "title": row[1], "year": row[2], "authors": row[3], "doi": row[4] }))
            .collect::<Vec<_>>();
        Ok(json!({ "works": works }))
    }

    fn scholar_artifacts(&self, work_id: &str) -> ApiResult {
        let work_id = work_id
            .parse::<u64>()
            .map_err(|_| ApiError("work_id must be a number".into()))?;
        let mut bridge = crate::scholar::Bridge::connect()?;
        Ok(bridge.call("list_work_artifacts", &json!({ "work_id": work_id }))?)
    }

    /// A Work's PDF and Markdown documents, each with the Document link that opens it from a
    /// Card. Like its figures, only the documents the Work shows are listed when it shows any:
    /// raw OCR copies beside them would read as duplicates.
    fn scholar_files(&self, work_id: &str) -> ApiResult {
        let work_id = work_id
            .parse::<u64>()
            .map_err(|_| ApiError("work_id must be a number".into()))?;
        let mut bridge = crate::scholar::Bridge::connect()?;
        let rows = bridge.sql(
            "SELECT id, relative_path, kind, size_bytes, shown, chinese_bilingual FROM work_files \
             WHERE work_id = ?1 AND availability = 'present' AND kind IN ('pdf', 'markdown') \
             ORDER BY shown DESC, kind DESC, chinese_bilingual DESC, id",
            &json!([work_id]),
            50,
            "archive",
        )?;
        let files = rows["rows"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|row| {
                let path = row[1].as_str().unwrap_or_default();
                let name = path.rsplit('/').next().unwrap_or(path);
                let file_id = row[0].as_u64().unwrap_or(0);
                json!({
                    "file_id": file_id, "name": name, "path": path, "kind": row[2], "bytes": row[3],
                    "shown": row[4] == 1, "bilingual": row[5] == 1,
                    "markdown": crate::scholar::document_markdown(name, file_id),
                })
            })
            .collect::<Vec<_>>();
        let shows_any = files.iter().any(|file| file["shown"] == true);
        let files = files
            .into_iter()
            .filter(|file| !shows_any || file["shown"] == true)
            .collect::<Vec<_>>();
        Ok(json!({ "files": files }))
    }

    fn artifact_target(body: &Value) -> std::result::Result<(u64, String), ApiError> {
        let file_id = body
            .get("file_id")
            .and_then(Value::as_u64)
            .ok_or_else(|| ApiError("missing `file_id`".into()))?;
        Ok((file_id, str_field(body, "path")?))
    }

    /// A figure to look at before choosing it; nothing is kept.
    fn scholar_preview(&self, body: &Value) -> ApiResult {
        let (file_id, path) = Self::artifact_target(body)?;
        let mut bridge = crate::scholar::Bridge::connect()?;
        let (bytes, _) = bridge.artifact(file_id, &path)?;
        let media_type = crate::artifact::sniff(&bytes)
            .map(|(_, media_type)| media_type)
            .ok_or_else(|| ApiError("the artifact is not an image".into()))?;
        Ok(json!({ "data_url": format!("data:{media_type};base64,{}", base64(&bytes)) }))
    }

    /// Keep a figure in this archive and return the Markdown that shows it.
    fn scholar_keep(&self, body: &Value) -> ApiResult {
        let (file_id, path) = Self::artifact_target(body)?;
        let caption = body.get("caption").and_then(Value::as_str);
        let mut bridge = crate::scholar::Bridge::connect()?;
        Ok(crate::scholar::keep_artifact(
            &mut bridge,
            &self.root,
            file_id,
            &path,
            caption,
        )?)
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
        let part = crate::model::Part::from_args(
            facet_field(body)?.flatten(),
            emphasis_field(body)?.flatten(),
        )?;
        let card_type = match (type_field(body)?, &part) {
            (Some(card_type), Some(_)) if card_type != CardType::Model => {
                return Err(ApiError(
                    "only Model Cards can have a Model facet".to_string(),
                ));
            }
            (_, Some(_)) => CardType::Model,
            (card_type, None) => card_type.unwrap_or_default(),
        };
        let (level, hub_level) = ops::parse_level_arg(body.get("level"))?;
        let (level, hub_level) = (level.flatten(), hub_level.flatten());
        if level.is_some() && (card_type != CardType::Model || part.is_some()) {
            return Err(ApiError(crate::model::LEVEL_NEEDS_MODEL_CARD.to_string()));
        }
        if hub_level.is_some() && card_type != CardType::Hub {
            return Err(ApiError(crate::hub::LEVEL_NEEDS_HUB_CARD.to_string()));
        }
        let form = form_field(body)?.flatten();
        if form.is_some() && (card_type != CardType::Model || part.is_some()) {
            return Err(ApiError(crate::math::FORM_NEEDS_MODEL_CARD.to_string()));
        }
        let location = match kind.as_str() {
            "topic" if card_type != CardType::Idea => {
                return Err(ApiError(
                    "only Regular Cards can be Event, Finding, Model, or Hub Cards".to_string(),
                ));
            }
            "topic" => ops::create_topic(&self.root, ACCESS, workspace_field(body)?, &text)?,
            "direct" => ops::create_successor(
                &self.root,
                ACCESS,
                ops::Successor::Direct,
                &str_field(body, "at")?,
                card_type,
                part.clone(),
                &text,
            )?,
            "side" => ops::create_successor(
                &self.root,
                ACCESS,
                ops::Successor::Side,
                &str_field(body, "at")?,
                card_type,
                part,
                &text,
            )?,
            other => return Err(ApiError(format!("unknown card kind `{other}`"))),
        };
        if level.is_some() || hub_level.is_some() || form.is_some() {
            let change = TypeChange {
                level: level.map(Some),
                hub_level: hub_level.map(Some),
                form: form.map(Some),
                ..TypeChange::default()
            };
            ops::edit(&self.root, ACCESS, &location, None, change, None)?;
        }
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

    /// Save a Card's text, its type, facet, emphasis, and level, or both.
    fn edit(&self, body: &Value) -> ApiResult {
        let (level, hub_level) = ops::parse_level_arg(body.get("level"))?;
        let change = TypeChange {
            card_type: type_field(body)?,
            facet: facet_field(body)?,
            emphasis: emphasis_field(body)?,
            level,
            hub_level,
            form: form_field(body)?,
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
            body.get("into").and_then(Value::as_str),
            confirm_field(body),
        )?
        .into_value())
    }

    /// The Spreads kept for the Table.
    fn spreads(&self) -> ApiResult {
        let conn = crate::open_db(&self.root)?;
        Ok(crate::spread::load(&conn)?)
    }

    /// Replace the Spreads kept for the Table; they change no Card.
    fn save_spreads(&self, body: &Value) -> ApiResult {
        let conn = crate::open_db(&self.root)?;
        Ok(crate::spread::save(&conn, body)?)
    }

    /// Move a Card to a free Location (`to`) or into another Card's place
    /// (`before`), with its successors or `alone`.
    fn move_card(&self, body: &Value) -> ApiResult {
        let target = match (
            body.get("to").and_then(Value::as_str),
            body.get("before").and_then(Value::as_str),
        ) {
            (Some(to), None) => ops::Target::To(to.to_string()),
            (None, Some(before)) => ops::Target::Before(before.to_string()),
            _ => return Err(ApiError("give either `to` or `before`".to_string())),
        };
        let alone = body.get("alone").and_then(Value::as_bool).unwrap_or(false);
        Ok(ops::move_card(
            &self.root,
            ACCESS,
            &str_field(body, "at")?,
            &target,
            alone,
            confirm_field(body),
        )?
        .into_value())
    }

    /// Insert a new Card at the place of `before`, which moves down.
    fn insert_card(&self, body: &Value) -> ApiResult {
        let part = crate::model::Part::from_args(
            facet_field(body)?.flatten(),
            emphasis_field(body)?.flatten(),
        )?;
        let card_type = match (type_field(body)?, &part) {
            (Some(card_type), Some(_)) if card_type != CardType::Model => {
                return Err(ApiError(
                    "only Model Cards can have a Model facet".to_string(),
                ));
            }
            (_, Some(_)) => CardType::Model,
            (card_type, None) => card_type.unwrap_or_default(),
        };
        Ok(ops::insert_card(
            &self.root,
            ACCESS,
            &str_field(body, "before")?,
            card_type,
            part,
            &str_field(body, "text")?,
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
    let mut figure: Option<(String, String)> = None;
    let mut in_document = false;
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
        if let Some((_, caption)) = figure.as_mut() {
            match event {
                Event::Text(text)
                | Event::Code(text)
                | Event::InlineMath(text)
                | Event::InlineHtml(text) => caption.push_str(&text),
                Event::End(TagEnd::Image) => {
                    let (digest, caption) = figure.take().unwrap_or_default();
                    events.push(Event::InlineHtml(figure_html(&digest, &caption).into()));
                }
                _ => {}
            }
            continue;
        }
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
            Event::Start(Tag::Image { dest_url, .. })
                if dest_url.starts_with(crate::artifact::SCHEME) =>
            {
                flush(&mut pending, &mut events, in_code_block);
                figure = Some((
                    dest_url[crate::artifact::SCHEME.len()..].to_string(),
                    String::new(),
                ));
            }
            Event::Start(Tag::Link { dest_url, .. })
                if dest_url.starts_with(crate::scholar::DOCUMENT_SCHEME) =>
            {
                flush(&mut pending, &mut events, in_code_block);
                in_document = true;
                events.push(Event::InlineHtml(document_link_open(&dest_url).into()));
            }
            Event::End(TagEnd::Link) if in_document => {
                flush(&mut pending, &mut events, in_code_block);
                in_document = false;
                events.push(Event::InlineHtml("</a>".into()));
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

/// An artifact shown in a Card: the image, framed like a print clipped to the
/// card, with its caption. Inline spans keep it valid inside a paragraph; the
/// page marks a figure whose image this archive does not hold.
fn figure_html(digest: &str, caption: &str) -> String {
    let caption = caption.trim();
    let digest = if crate::artifact::is_digest(digest) {
        digest
    } else {
        ""
    };
    format!(
        "<span class=\"zt-figure\" data-artifact=\"{digest}\"><img src=\"/artifact/{digest}\" alt=\"{alt}\" loading=\"lazy\">{caption}</span>",
        alt = escape_html(caption),
        caption = if caption.is_empty() {
            String::new()
        } else {
            format!(
                "<span class=\"zt-figcaption\">{}</span>",
                escape_html(caption)
            )
        },
    )
}

/// The opening tag of a Document link: a source document of the Scholar Archive, opened beside
/// the GUI. An address that names no document stays visible but leads nowhere.
fn document_link_open(dest: &str) -> String {
    match crate::scholar::document_target(dest) {
        Some((file_id, fragment)) => format!(
            "<a class=\"zt-doc\" href=\"/scholar/file/{file_id}{}\" data-doc=\"{file_id}\" title=\"Open the Scholar Archive document {file_id}\">",
            fragment
                .map(|fragment| format!("#{fragment}"))
                .unwrap_or_default()
        ),
        None => "<a class=\"zt-doc broken\" title=\"Not a Scholar Archive document address\">"
            .to_string(),
    }
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

/// An optional Card type: `idea`, `event`, `finding`, `model`, or `hub`.
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

/// An optional emphasis: absent leaves it alone, `""` or null clears it.
fn emphasis_field(body: &Value) -> Result<Option<Option<Emphasis>>> {
    match body.get("emphasis") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(value)) => crate::model::parse_optional_emphasis(value).map(Some),
        Some(_) => bail!("`emphasis` must be a string"),
    }
}

/// An optional Math form: absent leaves it alone, `""` or null clears it.
fn form_field(body: &Value) -> Result<Option<Option<crate::math::MathForm>>> {
    match body.get("form") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(value)) => crate::math::parse_optional_form(value).map(Some),
        Some(_) => bail!("`form` must be a string"),
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

/// An artifact's bytes. They never change (the address is their hash), and an
/// SVG opened on its own is sandboxed so it cannot script the GUI's origin.
fn respond_artifact(request: Request, media_type: &str, body: Vec<u8>) {
    let response = Response::from_data(body)
        .with_header(header("Content-Type", media_type))
        .with_header(header("Cache-Control", "max-age=31536000, immutable"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header(
            "Content-Security-Policy",
            "sandbox; default-src 'none'; style-src 'unsafe-inline'",
        ));
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
    fn the_recorded_url_names_its_port() {
        assert_eq!(url_port("http://127.0.0.1:4717/"), Some(4717));
        assert_eq!(url_port("http://example.com:4717/"), None);
        let temp = tempfile::tempdir().expect("temp dir");
        assert_eq!(recorded_url(temp.path()), None);
        fs::write(
            record_path(temp.path()),
            "http://127.0.0.1:5000/
",
        )
        .unwrap();
        assert_eq!(
            recorded_url(temp.path()).as_deref(),
            Some("http://127.0.0.1:5000/")
        );
        clear_record(temp.path());
        assert_eq!(recorded_url(temp.path()), None);
    }

    #[test]
    fn the_front_end_names_every_relation_kind() {
        use crate::model::RelationKind;
        let kinds = &APP_JS[APP_JS.find("const KINDS = {").expect("KINDS")..];
        let kinds = &kinds[..kinds.find("};").expect("end of KINDS")];
        for kind in [
            RelationKind::RestsOn,
            RelationKind::Illustrates,
            RelationKind::Contrasts,
            RelationKind::Relaxes,
            RelationKind::Corresponds,
            RelationKind::Objects,
            RelationKind::Specializes,
            RelationKind::Refers,
        ] {
            let id = kind.id();
            assert!(
                kinds.contains(&format!("'{id}':")) || kinds.contains(&format!("\n    {id}:")),
                "app.js KINDS has no `{id}`"
            );
        }
    }

    #[test]
    fn each_css_animation_name_is_defined_once() {
        let mut seen = std::collections::BTreeSet::new();
        for rest in APP_CSS.split("@keyframes ").skip(1) {
            let name = rest.split_whitespace().next().unwrap_or_default();
            assert!(seen.insert(name), "@keyframes {name} is defined twice in app.css");
        }
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
    fn a_snapshot_is_kept_until_another_connection_changes_the_cards() {
        let (temp, _) = snapshot();
        let snapshots = Snapshots::new(temp.path());
        let first = snapshots.get().unwrap();
        assert!(Arc::ptr_eq(&first, &snapshots.get().unwrap()));
        let fingerprint = |snapshots: &Snapshots| {
            snapshots
                .overview(|snapshot| Ok(json!(snapshot.fingerprint())))
                .unwrap()
        };
        let before = fingerprint(&snapshots);
        assert_eq!(
            snapshots
                .overview(|_| bail!("a kept overview is not built again"))
                .unwrap(),
            before
        );
        ops::edit(
            temp.path(),
            Access::Owner,
            "0/1",
            Some("# Ownership decides\n\nbody\n"),
            TypeChange::default(),
            None,
        )
        .unwrap();
        let after = snapshots.get().unwrap();
        assert!(!Arc::ptr_eq(&first, &after));
        assert_eq!(after.title("0/1"), "Ownership decides");
        assert!(Arc::ptr_eq(&after, &snapshots.get().unwrap()));
        assert_ne!(fingerprint(&snapshots), before);
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
    fn an_artifact_image_becomes_a_figure_with_its_caption() {
        let (_temp, snapshot) = snapshot();
        let digest = "ab".repeat(32);
        let html = render_markdown(
            &format!(
                "Before.\n\n![Figure 1: <GDP> & growth](artifact:{digest})\n\n![web](https://example.org/x.png)"
            ),
            &snapshot,
            &Charts::default(),
        );
        assert!(
            html.contains(&format!("<span class=\"zt-figure\" data-artifact=\"{digest}\"><img src=\"/artifact/{digest}\" alt=\"Figure 1: &lt;GDP&gt; &amp; growth\" loading=\"lazy\"><span class=\"zt-figcaption\">Figure 1: &lt;GDP&gt; &amp; growth</span></span>")),
            "{html}"
        );
        assert!(
            html.contains("<img src=\"https://example.org/x.png\""),
            "{html}"
        );
        let forged = render_markdown("![x](artifact:../../secret)", &snapshot, &Charts::default());
        assert!(forged.contains("data-artifact=\"\""), "{forged}");
    }

    #[test]
    fn a_document_link_opens_the_scholar_archive_document() {
        let (_temp, snapshot) = snapshot();
        let html = render_markdown(
            "原书：[MWG 中英对照](scholar:83)，第 41 页见 [PDF](scholar:85#page=41)；[坏](scholar:x)。",
            &snapshot,
            &Charts::default(),
        );
        assert!(
            html.contains("<a class=\"zt-doc\" href=\"/scholar/file/83\" data-doc=\"83\" title=\"Open the Scholar Archive document 83\">MWG 中英对照</a>"),
            "{html}"
        );
        assert!(html.contains("href=\"/scholar/file/85#page=41\""), "{html}");
        assert!(html.contains("<a class=\"zt-doc broken\""), "{html}");
        // An ordinary link is untouched.
        let plain = render_markdown("[web](https://example.org)", &snapshot, &Charts::default());
        assert!(
            plain.contains("<a href=\"https://example.org\">web</a>"),
            "{plain}"
        );
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
