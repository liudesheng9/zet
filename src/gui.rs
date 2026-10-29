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
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::link::{BrokenReason, ClassifiedLink, LinkBody, LinkForm, LinkStatus};
use crate::model::{Emphasis, Facet};
use crate::ops::{self, Snapshot, TypeChange};
use crate::{Access, CardType, Workspace};

mod document;

/// The GUI is the human's own tool and may change both Workspaces.
const ACCESS: Access = Access::Owner;

/// How long a GUI request waits for the edit lock before it reports contention.
const EDIT_PATIENCE: Duration = crate::ops::EDIT_PATIENCE;

const INDEX_HTML: &str = include_str!("gui/index.html");
const APP_CSS: &str = include_str!("gui/app.css");
const APP_JS: &str = include_str!("gui/app.js");
const DRAFT_JS: &str = include_str!("gui/draft.js");
const ASK_JS: &str = include_str!("gui/ask.js");
const FEATURES_JS: &str = include_str!("gui/features.js");
const LOCK_JS: &str = include_str!("gui/lock.js");
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
/// The most current candidates the optional Bridge report projection may return (§12.2, D293).
const BRIDGE_CANDIDATES_MAX: usize = 20;
/// How long the optional installed helper may run before it is cancelled (§12.2, D293).
const BRIDGE_HELPER_LIFETIME: Duration = Duration::from_secs(10);
/// The most report projections the server keeps: one per report content, archive revision, and
/// Card.
const BRIDGE_PROJECTION_CACHE: usize = 32;
/// The largest Bridge report the reader may select: a report reviews one frozen plan.
const BRIDGE_REPORT_MAX_BYTES: u64 = 16 * 1024 * 1024;
/// The longest report path the reader may select.
const BRIDGE_REPORT_MAX_PATH: usize = 4096;
/// Names the installed `zt-digest` helper when it is not on `PATH` (tests name one exactly).
const BRIDGE_HELPER_ENV: &str = "ZT_DIGEST_HELPER";
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
        projections: Projections::installed(),
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
    /// The optional Bridge report projection behind `/api/distant` (§10.3, S20).
    projections: Projections,
}

/// The optional Bridge-report projection behind `/api/distant` (§10.3, D257-D261; S20).
///
/// The reader selects one existing immutable report once; this keeps that choice's projection in
/// memory, keyed by the report file's content hash, the archive revision, and the Card, and reads
/// it through the installed `zt-digest` helper with a fixed argument vector. It never scans a
/// folder, never assembles a shell command from the path, and never starts a helper per repaint:
/// the first read of a key starts one run in the background, so a slow or stuck helper never
/// holds the reader, and the next read of that key uses the answer. A run past its lifetime is
/// cancelled. A missing helper, missing report, invalid report, stale endpoint, or timeout leaves
/// the reader with its ordinary term and model evidence and a small report status.
struct Projections {
    /// The installed helper, or the bare name to find on `PATH`.
    helper: PathBuf,
    /// How long one run may live before it is cancelled.
    lifetime: Duration,
    /// The run behind the seam; the unit tests answer without starting a process.
    run: Arc<dyn Fn(&Path, &[String], Duration) -> HelperRun + Send + Sync>,
    /// The answers and the runs in flight, oldest first; bounded.
    cache: Arc<Mutex<VecDeque<(String, CachedProjection)>>>,
}

/// One report projection, or the run still making it.
enum CachedProjection {
    Pending { started: Instant },
    Ready(Projection),
}

/// One answer of a selected report's projection: only current candidates for one Card.
#[derive(Clone, Debug, Default)]
struct Projection {
    status: String,
    hash: String,
    report_id: String,
    candidates: Vec<crate::entry::ReportTarget>,
    counts: Value,
    errors: Vec<String>,
    /// Whether this answer came from the in-memory cache rather than a helper run in flight.
    cached: bool,
}

impl Projection {
    fn pending(hash: &str) -> Self {
        Self {
            status: "pending".to_string(),
            hash: hash.to_string(),
            ..Self::default()
        }
    }

    fn failed(status: &str, hash: &str, error: impl Into<String>) -> Self {
        Self {
            status: status.to_string(),
            hash: hash.to_string(),
            errors: vec![error.into()],
            ..Self::default()
        }
    }

    /// What the reader's small status line shows and what a client may report: the selected path,
    /// the report's content hash, the status, the exact report id when the helper answered, the
    /// candidate count, and the helper's own errors.
    fn json(&self, selected: &str) -> Value {
        json!({
            "selected": selected,
            "hash": self.hash,
            "status": self.status,
            "report_id": self.report_id,
            "candidates": self.candidates.len(),
            "counts": self.counts,
            "errors": self.errors,
            "cached": self.cached,
        })
    }
}

/// One run of the installed helper, as the seam returns it.
enum HelperRun {
    Output(String),
    Missing,
    Failed(String),
    Timeout,
}

impl Projections {
    /// The installed helper: `ZT_DIGEST_HELPER` names one exactly, else `zt-digest` on `PATH`.
    fn installed() -> Self {
        let helper = std::env::var_os(BRIDGE_HELPER_ENV)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("zt-digest"));
        Self::new(helper, BRIDGE_HELPER_LIFETIME, Arc::new(run_helper))
    }

    fn new(helper: PathBuf, lifetime: Duration, run: Arc<dyn Fn(&Path, &[String], Duration) -> HelperRun + Send + Sync>) -> Self {
        Self {
            helper,
            lifetime,
            run,
            cache: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// The projection of one selected report for one Card. The report's content hash and the
    /// archive revision make the key, so a changed report or archive is read again; a key already
    /// answered is reused, and a key with a run in flight reports `pending` instead of starting a
    /// second run.
    fn read(&self, selected: &str, at: &str, revision: &str) -> Projection {
        let selected = selected.trim();
        let Some((path, hash)) = report_file(selected) else {
            return Projection::failed(
                "missing_report",
                "",
                "the selected Bridge report is not a readable file, so nothing was read",
            );
        };
        let key = format!("{hash}\u{0}{revision}\u{0}{at}");
        let cached = {
            let cache = self.cache.lock().unwrap_or_else(|poison| poison.into_inner());
            cache
                .iter()
                .rev()
                .find(|(seen, _)| seen == &key)
                .map(|(_, cached)| match cached {
                    CachedProjection::Ready(projection) => Ok(projection.clone()),
                    CachedProjection::Pending { started } => Err(started.elapsed()),
                })
        };
        match cached {
            Some(Ok(projection)) => return Projection { cached: true, ..projection },
            Some(Err(elapsed)) => {
                let mut pending = Projection::pending(&hash);
                pending.counts = json!({ "elapsed_ms": elapsed.as_millis() as u64 });
                return pending;
            }
            None => {}
        }
        let args = projection_args(&path, at);
        let helper = self.helper.clone();
        let lifetime = self.lifetime;
        let run = Arc::clone(&self.run);
        let cache = Arc::clone(&self.cache);
        let (worker_key, worker_hash) = (key.clone(), hash.clone());
        {
            let mut cache = self.cache.lock().unwrap_or_else(|poison| poison.into_inner());
            cache.push_back((key, CachedProjection::Pending { started: Instant::now() }));
            while cache.len() > BRIDGE_PROJECTION_CACHE {
                cache.pop_front();
            }
        }
        thread::spawn(move || {
            let outcome = panic::catch_unwind(AssertUnwindSafe(|| run(&helper, &args, lifetime)))
                .unwrap_or_else(|_| HelperRun::Failed("the Bridge report helper runner stopped unexpectedly".to_string()));
            let projection = projection_from_run(outcome, &worker_hash);
            let mut cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());
            if let Some((_, slot)) = cache.iter_mut().find(|(seen, _)| seen == &worker_key) {
                *slot = CachedProjection::Ready(projection);
            }
        });
        let mut pending = Projection::pending(&hash);
        pending.counts = json!({ "elapsed_ms": 0 });
        pending
    }
}

/// The fixed argument vector of one projection: the `bridge` tool, its `project` step, and the
/// selected path as one argument. No shell sees the path, so a space, `&`, `;`, or quote in a
/// file name is never interpreted (§10.3, D258; S20-AC03).
fn projection_args(report: &Path, at: &str) -> Vec<String> {
    vec![
        "bridge".to_string(),
        "-Step".to_string(),
        "project".to_string(),
        "-Report".to_string(),
        report.display().to_string(),
        "-At".to_string(),
        at.to_string(),
    ]
}

/// The selected report's path and content hash, or `None` when the reader named no readable
/// file. The exact path is used; no folder is listed and no guessed latest report is chosen.
fn report_file(selected: &str) -> Option<(PathBuf, String)> {
    use sha2::{Digest, Sha256};
    if selected.is_empty() || selected.chars().count() > BRIDGE_REPORT_MAX_PATH {
        return None;
    }
    let path = PathBuf::from(selected);
    if !path.is_absolute() {
        return None;
    }
    let meta = fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > BRIDGE_REPORT_MAX_BYTES {
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    let digest = Sha256::digest(&bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    Some((path, format!("sha256:{hex}")))
}

/// One helper answer as a projection status (S20-AC06, D309): only the helper's own successful
/// projection is used; every other answer is a small status the reader can show while keeping the
/// ordinary term and model evidence.
fn projection_from_run(run: HelperRun, hash: &str) -> Projection {
    let text = match run {
        HelperRun::Missing => {
            return Projection::failed("missing_helper", hash, "the Bridge report helper (zt-digest) is not installed")
        }
        HelperRun::Timeout => {
            return Projection::failed("timeout", hash, "the Bridge report helper did not answer within ten seconds and was cancelled")
        }
        HelperRun::Failed(message) => return Projection::failed("failed", hash, message),
        HelperRun::Output(text) => text,
    };
    let Some(line) = text.lines().rev().map(str::trim).find(|line| line.starts_with('{')) else {
        return Projection::failed("invalid_report", hash, "the Bridge report helper printed no projection");
    };
    let value: Value = match serde_json::from_str(line) {
        Ok(value) => value,
        Err(error) => {
            return Projection::failed("invalid_report", hash, format!("the Bridge report helper printed an unreadable projection: {error}"))
        }
    };
    if value["ok"] != json!(true) {
        let errors: Vec<String> = value["errors"]
            .as_array()
            .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_else(|| vec!["the Bridge report helper refused the selected report".to_string()]);
        let status = match value["status"].as_str() {
            Some("invalid_report") => "invalid_report",
            _ => "failed",
        };
        return Projection {
            status: status.to_string(),
            hash: hash.to_string(),
            errors,
            ..Projection::default()
        };
    }
    let status = match value["status"].as_str() {
        Some(status @ ("done" | "stale" | "not_in_report")) => status.to_string(),
        _ => return Projection::failed("invalid_report", hash, "the Bridge report helper returned an unknown projection status"),
    };
    let mut candidates = Vec::new();
    for item in value["candidates"].as_array().into_iter().flatten().take(BRIDGE_CANDIDATES_MAX) {
        let (Some(report), Some(proposal), Some(target)) = (item["report"].as_str(), item["proposal"].as_str(), item["target"].as_str()) else {
            return Projection::failed("invalid_report", hash, "the Bridge report helper returned a malformed candidate");
        };
        candidates.push(crate::entry::ReportTarget {
            report: report.to_string(),
            proposal: proposal.to_string(),
            target: target.to_string(),
            relation: item["relation"].as_str().filter(|relation| !relation.is_empty()).map(str::to_string),
        });
    }
    Projection {
        status,
        hash: hash.to_string(),
        report_id: value["report_id"].as_str().unwrap_or_default().to_string(),
        candidates,
        counts: value["counts"].clone(),
        errors: Vec::new(),
        cached: false,
    }
}

/// Run the installed helper with a fixed argument vector, bounded by `lifetime`. Its output goes
/// to temporary files, so a helper that writes too much cannot block on a full pipe; on the
/// deadline the child is killed and reaped.
fn run_helper(program: &Path, args: &[String], lifetime: Duration) -> HelperRun {
    let folder = match tempfile::Builder::new().prefix("zt-bridge-").tempdir() {
        Ok(folder) => folder,
        Err(error) => return HelperRun::Failed(format!("a temporary folder is unavailable: {error}")),
    };
    let out = folder.path().join("stdout");
    let err = folder.path().join("stderr");
    let (Ok(stdout), Ok(stderr)) = (fs::File::create(&out), fs::File::create(&err)) else {
        return HelperRun::Failed("a temporary file is unavailable".to_string());
    };
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;
        // The GUI lives inside the daemon, which has no console to lend the helper.
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return HelperRun::Missing,
        Err(error) => return HelperRun::Failed(format!("the Bridge report helper did not start: {error}")),
    };
    let deadline = Instant::now() + lifetime;
    let mut pause = Duration::from_millis(2);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let text = fs::read_to_string(&out).unwrap_or_default();
                if text.lines().any(|line| line.trim_start().starts_with('{')) {
                    return HelperRun::Output(text);
                }
                let message = fs::read_to_string(&err).unwrap_or_default();
                let message: String = message.chars().take(500).collect();
                return HelperRun::Failed(format!("the Bridge report helper failed ({status}): {}", message.trim()));
            }
            Ok(None) => {}
            Err(error) => {
                let _ = child.kill();
                return HelperRun::Failed(format!("the Bridge report helper could not be observed: {error}"));
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return HelperRun::Timeout;
        }
        thread::sleep(pause);
        pause = (pause * 2).min(Duration::from_millis(50));
    }
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

/// One failed GUI request. A Tree Lock refusal keeps its structured facts, so the page can name the
/// Locked tree, the Lock and the pinned sources instead of printing one opaque line; a Card-check
/// refusal keeps its typed contract the same way, so the desk can show the fault lines and keep
/// the draft (R0164, R0176).
struct ApiError {
    message: String,
    refusal: Option<Value>,
    check: Option<Value>,
}

impl ApiError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            refusal: None,
            check: None,
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(error: anyhow::Error) -> Self {
        if let Some(refusal) = error.downcast_ref::<crate::tree_lock::Refusal>() {
            return Self {
                message: refusal.to_string(),
                refusal: Some(refusal.value()),
                check: None,
            };
        }
        if let Some(refusal) = error.downcast_ref::<crate::tree_lock::IndexRefusal>() {
            return Self {
                message: refusal.to_string(),
                refusal: Some(refusal.value()),
                check: None,
            };
        }
        if let Some(refusal) = error.downcast_ref::<crate::gate::Refusal>() {
            return Self {
                message: refusal.to_string(),
                refusal: None,
                check: Some(refusal.value()),
            };
        }
        if let Some(incomplete) = error.downcast_ref::<crate::gate::Incomplete>() {
            return Self {
                message: incomplete.to_string(),
                refusal: None,
                check: Some(incomplete.value()),
            };
        }
        Self::new(format!("{error:#}"))
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new(error.to_string())
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
            (Method::Get, "/draft.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    DRAFT_JS.to_string(),
                );
                return;
            }
            (Method::Get, "/ask.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    ASK_JS.to_string(),
                );
                return;
            }
            (Method::Get, "/features.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    FEATURES_JS.to_string(),
                );
                return;
            }
            (Method::Get, "/lock.js") => {
                respond_text(
                    request,
                    200,
                    "text/javascript; charset=utf-8",
                    LOCK_JS.to_string(),
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
        // A reader feature the release turned off answers one clear refusal; every other route
        // (Cards, trees, the map, the Table, saved Spreads) keeps working.
        if let Some(key) = crate::features::Features::load().gate(&path) {
            respond_json(
                request,
                409,
                &json!({
                    "error": format!(
                        "the {} feature is disabled in the GUI; run `zt config set {key} on`",
                        crate::features::label(key)
                    ),
                    "disabled": key,
                }),
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
            Err(error) => {
                let mut value = json!({ "error": error.message });
                let status = match error.refusal {
                    Some(refusal) => {
                        value["refusal"] = refusal;
                        409
                    }
                    None => {
                        if let Some(check) = error.check {
                            value["check"] = check;
                        }
                        400
                    }
                };
                respond_json(request, status, &value);
            }
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
                .ok_or_else(|| ApiError::new(format!("missing `{name}` parameter")))
        };
        match (method, path) {
            // The page's heartbeat: this service is still up and its token still holds.
            (Method::Get, "/api/ping") => Ok(json!({ "ok": true })),
            (Method::Get, "/api/overview") => self.overview(),
            (Method::Get, "/api/tree") => self.tree(&param("id")?),
            (Method::Get, "/api/card") => self.card(&param("at")?),
            (Method::Get, "/api/graph") => self.graph(),
            (Method::Get, "/api/register") => self.register(query),
            (Method::Post, "/api/ask") => self.ask(body),
            (Method::Post, "/api/hub/draft") => self.hub_draft(body),
            (Method::Post, "/api/hub/check") => self.hub_check(body),
            (Method::Get, "/api/distant") => self.distant(
                &param("at")?,
                query.get("table").map(String::as_str),
                query.get("report").map(String::as_str),
            ),
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
            (Method::Post, "/api/reply") => self.reply(body),
            (Method::Get, "/api/locks") => self.locks(),
            (Method::Post, "/api/lock") => self.lock(body),
            (Method::Post, "/api/unlock") => self.unlock(body),
            (Method::Post, "/api/lock/note") => self.lock_note(body),
            (Method::Get, "/api/artifact") => self.artifact(&param("sha")?),
            (Method::Get, "/api/scholar/works") => self.scholar_works(&param("q")?),
            (Method::Get, "/api/scholar/artifacts") => self.scholar_artifacts(&param("work_id")?),
            (Method::Get, "/api/scholar/files") => self.scholar_files(&param("work_id")?),
            (Method::Post, "/api/scholar/preview") => self.scholar_preview(body),
            (Method::Post, "/api/scholar/artifact") => self.scholar_keep(body),
            (Method::Get, "/api/spreads") => self.spreads(),
            (Method::Post, "/api/spreads") => self.save_spreads(body),
            _ => Err(ApiError::new(format!("unknown endpoint {path}"))),
        }
    }

    fn snapshot(&self) -> Result<Arc<Snapshot>> {
        self.snapshots.get()
    }

    fn overview(&self) -> ApiResult {
        let mut overview = self.snapshots.overview(|snapshot| {
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
                "revision": snapshot.revision(),
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
        })?;
        // The switches are read per answer, not cached with the Snapshot, so turning one off or on
        // takes effect for the next request without a restart.
        overview["features"] = crate::features::Features::load().json();
        Ok(overview)
    }

    fn tree(&self, tree: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        let root = crate::tree_root_address(tree);
        let Some(root_card) = snapshot.by_address.get(&root) else {
            return Err(ApiError::new(format!("tree `{tree}` does not exist")));
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
    /// `at` names one Card whose accepted terms the answer carries (`terms_at`, `terms`).
    fn register(&self, query: &BTreeMap<String, String>) -> ApiResult {
        let workspace = query
            .get("ws")
            .filter(|value| !value.is_empty() && *value != "all")
            .map(|value| crate::Workspace::parse(value))
            .transpose()?
            .ok_or_else(|| ApiError::new("missing `ws` parameter (human or agent)"))?;
        let number = |name: &str| -> Result<Option<u64>, ApiError> {
            match query.get(name) {
                None => Ok(None),
                Some(value) => value
                    .parse::<u64>()
                    .map(Some)
                    .map_err(|_| ApiError::new(format!("`{name}` must be a whole number"))),
            }
        };
        let args = json!({
            "entry_offset": number("entry_offset")?,
            "entry_limit": number("entry_limit")?,
        });
        let entries = crate::entry::EntryArgs::from_args(&args).map_err(ApiError::from)?;
        let terms_for = query.get("at").filter(|value| !value.is_empty());
        Ok(crate::read::register(
            &self.root,
            workspace,
            &entries,
            terms_for.map(String::as_str),
        )?)
    }

    /// The unsaved Spread proposal of one question (§9.5, D226-D231; `/api/ask` in D279): the same
    /// retrieval function the MCP server's `ask_box` calls, from one coherent read, so both
    /// transports answer one question identically. Read-only: it writes no Card and no Spread.
    fn ask(&self, body: &Value) -> ApiResult {
        let args = crate::entry::AskArgs::from_args(body).map_err(ApiError::from)?;
        Ok(crate::read::ask(&self.root, &args)?)
    }

    /// The plan of the agent Hub Card that files a Spread (S17, #118; design §9.6, D235-D237):
    /// where the draft would be filed, at what level, the Markdown it opens with, every legal
    /// destination, and the precheck of that draft. Read-only: filing needs the separate Save.
    fn hub_draft(&self, body: &Value) -> ApiResult {
        let snapshot = self.snapshot()?;
        Ok(crate::hub::draft(&snapshot, body)?)
    }

    /// The draft precheck (§9.6, S17-AC04): the hub report's own rules run over a hub body that is
    /// not saved yet, so the desk can refuse a draft before it writes one. Read-only.
    fn hub_check(&self, body: &Value) -> ApiResult {
        let snapshot = self.snapshot()?;
        Ok(crate::hub::precheck(&snapshot, body)?)
    }

    /// The one distant Card of the Card the reader has open or holds (§10.2, D249-D256; `/api/distant`
    /// in D280): at most one candidate, the Cards already on the Table as exclusions, and one line of
    /// evidence. When the reader has selected a Bridge report, its validated current proposals are
    /// the first candidate source (§10.3, D257-D261; S20): the projection is read through the
    /// installed helper with a fixed argument vector, cached by report hash and archive revision,
    /// and reported as a small status even when it fails. Read-only: it writes no Card, Link, or
    /// viewing record, and it never applies a proposal.
    fn distant(&self, at: &str, table: Option<&str>, report: Option<&str>) -> ApiResult {
        let table = table
            .map(|list| {
                list.split(',')
                    .map(str::trim)
                    .filter(|at| !at.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let snapshot = self.snapshot()?;
        let mut args = crate::entry::DistantArgs::new(at);
        args.table = table;
        let mut status = Value::Null;
        if let Some(selected) = report.map(str::trim).filter(|selected| !selected.is_empty()) {
            let projection = self
                .projections
                .read(selected, at.trim(), &snapshot.fingerprint());
            args.reports = projection.candidates.clone();
            status = projection.json(selected);
        }
        let mut value = crate::entry::distant(&snapshot, &args)?;
        if !status.is_null() {
            value["report"] = status;
        }
        Ok(value)
    }

    fn card(&self, target: &str) -> ApiResult {
        let snapshot = self.snapshot()?;
        let card = snapshot
            .by_address
            .get(target)
            .ok_or_else(|| ApiError::new(format!("card `{target}` does not exist")))?;
        let parsed = &snapshot.parsed[&card.address];
        let context = if crate::is_tree_root(card) {
            "tree-root"
        } else {
            "regular"
        };
        let mut value = json!({
            "address": card.address,
            "tree": crate::tree_id(&card.address),
            "workspace": card.workspace.id(),
            "kind": ops::kind(card),
            "type": card.card_type.id(),
            "term": card.term_name,
            // The computed Term role and list, so the reader can name them quietly beside the
            // head and keep the generated use list a separate, countable list (S13, #156).
            "term_facts": snapshot.term_facts(&card.address)?,
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
            // The authored identity a Reply captures and re-checks before it saves (S18, #99).
            "authored_fingerprint": crate::card::authored_fingerprint(card)?,
            // The derived navigation role of an accepted restatement entry and the exact statement
            // fingerprint (§4.5, D086-D087): both computed from the authored text, stored nowhere.
            "restatement": crate::entry::is_restatement_entry(&parsed.body),
            "statement_fingerprint": crate::card::statement_fingerprint(&parsed.body),
            "html": render_markdown(&parsed.body, &snapshot, &self.charts),
            "bibtex": card.bibtex,
            "citation_key": card.citation_key,
            "parent": snapshot.nav(crate::parent_location(&card.address)),
            "direct": snapshot.nav(crate::direct_successor(&card.address).ok()),
            "sides": snapshot.sides(card),
            "inbound": snapshot.inbound_refs(&card.address),
            // Ordinary Reverse links and Term uses are separate lists, as their generated sections
            // are (S11, #153; design 10.3). `inbound` keeps both for existing navigation.
            "reverse_links": snapshot.reverse_links(&card.address),
            "term_uses": snapshot.term_uses(&card.address),
            "outbound": snapshot.outbound(&card.address),
        });
        // The Card's own Lock facts (R201-R204, S03-AC01): the reader learns here whether an active
        // Lock governs this Card and which Locked trees pin it. `writable` is the GUI's own Owner
        // access and no active Lock.
        snapshot.decorate(&mut value, card.row_id);
        value["writable"] = json!(snapshot.lock().writable_for(
            card.row_id,
            ACCESS,
            card.workspace,
        ));
        Ok(value)
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
            .map_err(|_| ApiError::new("work_id must be a number"))?;
        let mut bridge = crate::scholar::Bridge::connect()?;
        Ok(bridge.call("list_work_artifacts", &json!({ "work_id": work_id }))?)
    }

    /// A Work's PDF and Markdown documents, each with the Document link that opens it from a
    /// Card. Like its figures, only the documents the Work shows are listed when it shows any:
    /// raw OCR copies beside them would read as duplicates.
    fn scholar_files(&self, work_id: &str) -> ApiResult {
        let work_id = work_id
            .parse::<u64>()
            .map_err(|_| ApiError::new("work_id must be a number"))?;
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
            .ok_or_else(|| ApiError::new("missing `file_id`"))?;
        Ok((file_id, str_field(body, "path")?))
    }

    /// A figure to look at before choosing it; nothing is kept.
    fn scholar_preview(&self, body: &Value) -> ApiResult {
        let (file_id, path) = Self::artifact_target(body)?;
        let mut bridge = crate::scholar::Bridge::connect()?;
        let (bytes, _) = bridge.artifact(file_id, &path)?;
        let media_type = crate::artifact::sniff(&bytes)
            .map(|(_, media_type)| media_type)
            .ok_or_else(|| ApiError::new("the artifact is not an image"))?;
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
                return Err(ApiError::new(
                    "only Model Cards can have a Model facet".to_string(),
                ));
            }
            (_, Some(_)) => CardType::Model,
            (card_type, None) => card_type.unwrap_or_default(),
        };
        let (level, hub_level) = ops::parse_level_arg(body.get("level"))?;
        let (level, hub_level) = (level.flatten(), hub_level.flatten());
        if level.is_some() && (card_type != CardType::Model || part.is_some()) {
            return Err(ApiError::new(crate::model::LEVEL_NEEDS_MODEL_CARD.to_string()));
        }
        if hub_level.is_some() && card_type != CardType::Hub {
            return Err(ApiError::new(crate::hub::LEVEL_NEEDS_HUB_CARD.to_string()));
        }
        let form = form_field(body)?.flatten();
        if form.is_some() && (card_type != CardType::Model || part.is_some()) {
            return Err(ApiError::new(crate::math::FORM_NEEDS_MODEL_CARD.to_string()));
        }
        let term = term_for_type(body, card_type)?;
        let (location, mut accepted) = match kind.as_str() {
            "topic" if card_type != CardType::Idea => {
                return Err(ApiError::new(
                    "only Regular Cards can be Event, Finding, Model, Hub, or Term Cards".to_string(),
                ));
            }
            "topic" => ops::create_topic(&self.root, ACCESS, workspace_field(body)?, &text)?,
            "direct" => ops::create_successor_with_term(
                &self.root,
                ACCESS,
                ops::Successor::Direct,
                &str_field(body, "at")?,
                card_type,
                part.clone(),
                term.as_deref(),
                &text,
            )?,
            "side" => ops::create_successor_with_term(
                &self.root,
                ACCESS,
                ops::Successor::Side,
                &str_field(body, "at")?,
                card_type,
                part,
                term.as_deref(),
                &text,
            )?,
            other => return Err(ApiError::new(format!("unknown card kind `{other}`"))),
        };
        if level.is_some() || hub_level.is_some() || form.is_some() {
            let change = TypeChange {
                level: level.map(Some),
                hub_level: hub_level.map(Some),
                form: form.map(Some),
                ..TypeChange::default()
            };
            let (_, checked) = ops::edit(&self.root, ACCESS, &location, None, change, None)?;
            if !checked.is_empty() {
                accepted = checked;
            }
        }
        Ok(accepted.value(&location))
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
        let (address, accepted) = ops::create_literature(
            &self.root,
            ACCESS,
            workspace_field(body)?,
            &str_field(body, "bibtex")?,
            &str_field(body, "text")?,
        )?;
        Ok(accepted.value(&address))
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
            term: term_field(body)?,
        };
        let text = if change.is_empty() {
            Some(str_field(body, "text")?)
        } else {
            body.get("text").and_then(Value::as_str).map(str::to_string)
        };
        let (address, accepted) = ops::edit(
            &self.root,
            ACCESS,
            &str_field(body, "at")?,
            text.as_deref(),
            change,
            base_field(body),
        )?;
        Ok(accepted.value(&address))
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

    /// Reply to an agent Card with one human Card (S18, #99). Owner-only, like every GUI write, and
    /// there is no MCP route that writes a human Card. A refusal is a 200 answer that says why, so
    /// the reader keeps the draft and sees the fresh context.
    fn reply(&self, body: &Value) -> ApiResult {
        let outcome = crate::reply::write(
            &self.root,
            &crate::reply::Request {
                target: &str_field(body, "at")?,
                base: &str_field(body, "base")?,
                text: &str_field(body, "text")?,
                topic: body.get("topic").and_then(Value::as_str),
            },
        )?;
        Ok(outcome.into_value())
    }

    /// The Lock panel: every Lock, its retained history and the current integrity result. Opening
    /// the panel is one of the design's integrity check points, so newly observed faults are recorded
    /// once under the edit lock; a healthy archive is only read (R292, R027).
    fn locks(&self) -> ApiResult {
        let mut conn = crate::open_db(&self.root)?;
        let integrity = crate::tree_lock::check(&conn)?;
        if integrity.needs_recording() {
            let _lock =
                crate::acquire_edit_lock_within(&self.root, EDIT_PATIENCE, "locks")?;
            let tx = conn.transaction()?;
            let fresh = crate::tree_lock::check(&tx)?;
            crate::tree_lock::record_faults_tx(&tx, &fresh)?;
            tx.commit()?;
            // Re-read so the answer carries the latch this open just recorded.
            let recorded = crate::tree_lock::check(&conn)?;
            return Ok(crate::tree_lock::export_with(&conn, &recorded)?);
        }
        Ok(crate::tree_lock::export_with(&conn, &integrity)?)
    }

    /// Plan a Lock (`{root, note?}`) and commit it only with the matching `confirm: "lock"` word,
    /// its stateless `plan_hash` and a freshly recomputed plan.
    fn lock(&self, body: &Value) -> ApiResult {
        let root = str_field(body, "root")?;
        let note = body.get("note").and_then(Value::as_str);
        let _lock = crate::acquire_edit_lock_within(&self.root, EDIT_PATIENCE, "lock")?;
        let mut conn = crate::open_db(&self.root)?;
        let tx = conn.transaction()?;
        let plan = crate::tree_lock::plan(&tx, &root, note)?;
        let confirmed = body.get("confirm").and_then(Value::as_str);
        let Some(confirmed) = confirmed else {
            drop(tx);
            return Ok(json!({
                "applied": false,
                "confirm": "lock",
                "plan": plan.value(),
                "lines": plan.lines(),
            }));
        };
        if confirmed != "lock" {
            return Err(ApiError::new(format!(
                "the lock confirmation word is `lock`, not `{confirmed}`"
            )));
        }
        let Some(hash) = body.get("plan_hash").and_then(Value::as_str) else {
            return Err(ApiError::new("a confirmed lock needs its plan_hash"));
        };
        if hash != plan.plan_hash {
            drop(tx);
            let fresh = crate::tree_lock::plan(&conn, &root, note)?;
            return Ok(json!({
                "applied": false,
                "plan_changed": true,
                "confirm": "lock",
                "plan": fresh.value(),
                "lines": fresh.lines(),
            }));
        }
        let lock = crate::tree_lock::lock_tx(&tx, &plan)?;
        tx.commit()?;
        let mut answer = json!({ "applied": true, "lock": lock.value() });
        if let Err(error) = crate::log_event(
            &self.root,
            &format!(
                "tree locked: {} (lock {}, seal {})",
                lock.root,
                lock.lock_id,
                lock.seal_display()
            ),
        ) {
            answer["log_error"] = json!(format!("{error:#}"));
        }
        Ok(answer)
    }

    /// Plan the release of one active Lock and end exactly that Lock on confirmation.
    fn unlock(&self, body: &Value) -> ApiResult {
        let lock_id = lock_id_field(body)?;
        let _lock = crate::acquire_edit_lock_within(&self.root, EDIT_PATIENCE, "unlock")?;
        let mut conn = crate::open_db(&self.root)?;
        let tx = conn.transaction()?;
        let plan = crate::tree_lock::existing_plan(&tx, lock_id, "unlock", None)?;
        check_lock_tree(body, &plan)?;
        let Some(confirmed) = body.get("confirm").and_then(Value::as_str) else {
            drop(tx);
            return Ok(json!({
                "applied": false,
                "confirm": "unlock",
                "plan": plan,
                "lines": unlock_lines(&plan),
            }));
        };
        if confirmed != "unlock" {
            return Err(ApiError::new(format!(
                "the unlock confirmation word is `unlock`, not `{confirmed}`"
            )));
        }
        let Some(hash) = body.get("plan_hash").and_then(Value::as_str) else {
            return Err(ApiError::new("a confirmed unlock needs its plan_hash"));
        };
        if Some(hash) != plan["plan_hash"].as_str() {
            drop(tx);
            let fresh = crate::tree_lock::existing_plan(&conn, lock_id, "unlock", None)?;
            return Ok(json!({
                "applied": false,
                "plan_changed": true,
                "confirm": "unlock",
                "plan": fresh,
                "lines": unlock_lines(&fresh),
            }));
        }
        let lock = crate::tree_lock::unlock_tx(&tx, lock_id)?;
        tx.commit()?;
        let mut answer = json!({ "applied": true, "lock": lock.value() });
        if let Err(error) = crate::log_event(
            &self.root,
            &format!("tree unlocked: {} (lock {})", lock.root, lock.lock_id),
        ) {
            answer["log_error"] = json!(format!("{error:#}"));
        }
        Ok(answer)
    }

    /// Plan a note change for one active Lock and apply it on confirmation. A changed note never
    /// reseals the tree.
    fn lock_note(&self, body: &Value) -> ApiResult {
        let lock_id = lock_id_field(body)?;
        let note = body.get("note").and_then(Value::as_str);
        let _lock = crate::acquire_edit_lock_within(&self.root, EDIT_PATIENCE, "lock note")?;
        let mut conn = crate::open_db(&self.root)?;
        let tx = conn.transaction()?;
        let plan = crate::tree_lock::existing_plan(&tx, lock_id, "note", note)?;
        check_lock_tree(body, &plan)?;
        let Some(confirmed) = body.get("confirm").and_then(Value::as_str) else {
            drop(tx);
            return Ok(json!({
                "applied": false,
                "confirm": "note",
                "plan": plan,
                "lines": note_lines(&plan),
            }));
        };
        if confirmed != "note" {
            return Err(ApiError::new(format!(
                "the note confirmation word is `note`, not `{confirmed}`"
            )));
        }
        let Some(hash) = body.get("plan_hash").and_then(Value::as_str) else {
            return Err(ApiError::new("a confirmed note change needs its plan_hash"));
        };
        if Some(hash) != plan["plan_hash"].as_str() {
            drop(tx);
            let fresh = crate::tree_lock::existing_plan(&conn, lock_id, "note", note)?;
            return Ok(json!({
                "applied": false,
                "plan_changed": true,
                "confirm": "note",
                "plan": fresh,
                "lines": note_lines(&fresh),
            }));
        }
        let changed = plan["note"] != plan["previous_note"];
        let lock = crate::tree_lock::note_tx(&tx, lock_id, note)?;
        tx.commit()?;
        let mut answer = json!({ "applied": true, "lock": lock.value() });
        // R105: one concise line mirrors the committed event, after the commit. An identical note
        // wrote no event, so it mirrors nothing; a log that cannot be opened is only reported.
        if changed
            && let Err(error) = crate::log_event(
                &self.root,
                &format!(
                    "tree lock note changed: {} (lock {}), note {}",
                    lock.root,
                    lock.lock_id,
                    if lock.note.is_some() { "set" } else { "cleared" }
                ),
            )
        {
            answer["log_error"] = json!(format!("{error:#}"));
        }
        Ok(answer)
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
            _ => return Err(ApiError::new("give either `to` or `before`".to_string())),
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
                return Err(ApiError::new(
                    "only Model Cards can have a Model facet".to_string(),
                ));
            }
            (_, Some(_)) => CardType::Model,
            (card_type, None) => card_type.unwrap_or_default(),
        };
        let term = term_for_type(body, card_type)?;
        Ok(ops::insert_card_with_term(
            &self.root,
            ACCESS,
            &str_field(body, "before")?,
            card_type,
            part,
            term.as_deref(),
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
/// names with `[[address]]` or a Term `[[label@address]]`. The store keeps this
/// small template; [`serve_chart_page`] fills in the theme and inlines the scripts.
fn chart_page(source: &str, snapshot: &Snapshot) -> String {
    let mut links = serde_json::Map::new();
    // The one lexical scan: a name inside a code span is no Link there either, and a Term use
    // names the head's address.
    for occurrence in crate::link::LinkBody::for_rendering(source).occurrences() {
        let target = occurrence.target;
        if !target.is_empty() && target.len() < 200 && !target.contains(['\n', '[', ']']) {
            let exists = snapshot.by_address.contains_key(target);
            links.insert(
                target.to_string(),
                json!({ "title": snapshot.title(target), "exists": exists }),
            );
        }
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
        events.push(Event::InlineHtml(link_html(&link, snapshot).into()));
        offset = link.macro_range.end;
    }
    if offset < text.len() {
        events.push(Event::Text(text[offset..].to_string().into()));
    }
    events
}

/// One Link occurrence as HTML in the reader's prose (S13, #156; design 15.2). A Term use is the
/// authored word alone, in the prose font, carrying the list row's short meaning and the target
/// address for hover and focus, and the existing encoded Card route for click or keyboard. An
/// ordinary Link keeps its address/title chip. Every authored value is escaped here, while the
/// route is URL-encoded separately, so a label, meaning or target stays data and never becomes
/// markup or a second attribute.
fn link_html(link: &ClassifiedLink, snapshot: &Snapshot) -> String {
    let target = escape_html(link.target);
    let route = percent_encode(link.target);
    if link.form == LinkForm::Term {
        let label = escape_html(term_label(link));
        let Some(reason) = link.reason else {
            let meaning = snapshot.term_meaning(link.target).unwrap_or_default();
            let title = if meaning.is_empty() {
                link.target.to_string()
            } else {
                format!("{meaning} · {}", link.target)
            };
            return format!(
                "<a class=\"zt-link zt-term\" href=\"#/card/{route}\" data-target=\"{target}\" data-meaning=\"{}\" title=\"{}\">{label}</a>",
                escape_html(meaning),
                escape_html(&title),
            );
        };
        let note = term_broken_note(reason);
        return format!(
            "<span class=\"zt-link zt-term broken\" data-target=\"{target}\" data-reason=\"{}\" title=\"Broken Term link: {target} ({note})\">{label} <span class=\"zt-term-note\">{note}</span></span>",
            reason.id(),
        );
    }
    match link.status {
        // A Model part is named by its facet, the label a reader understands inside the model;
        // its address stays one hover away.
        LinkStatus::Valid => match snapshot.by_address.get(link.target).and_then(|card| card.facet.as_ref()) {
            Some(facet) => format!(
                "<a class=\"zt-link\" href=\"#/card/{route}\" data-target=\"{target}\" title=\"{target}\"><span class=\"zt-link-addr zt-link-facet\">{}</span><span class=\"zt-link-title\">{}</span></a>",
                escape_html(facet.id()),
                escape_html(&snapshot.title(link.target)),
            ),
            None => format!(
                "<a class=\"zt-link\" href=\"#/card/{route}\" data-target=\"{target}\"><span class=\"zt-link-addr\">{target}</span><span class=\"zt-link-title\">{}</span></a>",
                escape_html(&snapshot.title(link.target)),
            ),
        },
        LinkStatus::Broken => format!(
            "<span class=\"zt-link broken\" title=\"Broken link\"><span class=\"zt-link-addr\">{target}</span><span class=\"zt-link-title\">missing</span></span>"
        ),
    }
}

/// The word a Term use shows: its authored label, or the target address when the stored macro has
/// no label, so the reader always has an accessible word rather than an invisible Link.
fn term_label<'a>(link: &ClassifiedLink<'a>) -> &'a str {
    link.label
        .filter(|label| !label.trim().is_empty())
        .unwrap_or(link.target)
}

/// Why a broken Term use is unusable, in the reader's words (S13, #156; design 15.2).
fn term_broken_note(reason: BrokenReason) -> &'static str {
    match reason {
        BrokenReason::Missing => "missing",
        BrokenReason::NotHead => "not a Term head",
    }
}

fn str_field(body: &Value, name: &str) -> Result<String> {
    body.get(name)
        .and_then(Value::as_str)
        .map(str::to_string)
        .with_context(|| format!("missing `{name}` field"))
}

/// The exact Lock a Lock action names. A Lock id is required; a server-side Lock id is never guessed.
fn lock_id_field(body: &Value) -> Result<i64> {
    body.get("lock_id")
        .and_then(Value::as_i64)
        .with_context(|| "missing `lock_id` field".to_string())
}

/// A Lock action may name its tree; when it does, it must name the Lock's own tree.
fn check_lock_tree(body: &Value, plan: &Value) -> Result<()> {
    let Some(tree) = body.get("tree").and_then(Value::as_str) else {
        return Ok(());
    };
    if plan["root"].as_str() == Some(tree) {
        Ok(())
    } else {
        bail!(
            "lock {} belongs to tree `{}`, not `{tree}`",
            plan["lock_id"],
            plan["root"].as_str().unwrap_or_default()
        )
    }
}

fn unlock_lines(plan: &Value) -> Vec<String> {
    vec![
        format!(
            "unlock `{}` (lock {})",
            plan["root"].as_str().unwrap_or_default(),
            plan["lock_id"]
        ),
        format!("seal: {}", plan["seal_display"].as_str().unwrap_or_default()),
        format!(
            "releases {} cards and {} pinned targets",
            plan["member_count"], plan["pin_count"]
        ),
        "history stays in the archive; the tree takes new cards again".to_string(),
    ]
}

fn note_lines(plan: &Value) -> Vec<String> {
    vec![
        format!(
            "note on `{}` (lock {})",
            plan["root"].as_str().unwrap_or_default(),
            plan["lock_id"]
        ),
        format!(
            "previous note: {}",
            plan["previous_note"].as_str().unwrap_or("none")
        ),
        format!("new note: {}", plan["note"].as_str().unwrap_or("none")),
        "a note change never reseals the tree".to_string(),
    ]
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

/// An optional Term name: absent preserves, null or a trimmed-empty string clears, a string sets.
fn term_field(body: &Value) -> Result<Option<Option<String>>> {
    crate::term::parse_field(body.get("term"))
}

/// A Term name belongs only to a Term Card; a supplied name on another type is an invalid field.
fn term_for_type(body: &Value, card_type: CardType) -> Result<Option<String>> {
    let term = term_field(body)?.flatten();
    crate::term::ensure_on_type(term.as_deref(), card_type)?;
    Ok(term)
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

    /// This module's own source text, so a page test can pin the route table it mounts.
    const GUI_RS: &str = include_str!("gui.rs");

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

    /// Run one of the page's DOM-free modules (`src/gui/draft.js`, `src/gui/ask.js`) under Node and
    /// answer the JSON its expression prints, the module bound to `bind`. The project already needs
    /// Node for its wiki check and its workflow tools; when Node is missing this answers `None` and
    /// the test says so, so `cargo test` itself needs no extra program.
    fn page_module_js(module: &str, source: &str, bind: &str, expression: &str) -> Option<Value> {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join(module);
        fs::write(&file, source).expect("page module");
        let driver = dir.path().join("driver.mjs");
        let script = format!(
            "import {{ createRequire }} from 'node:module';\n\
             const require = createRequire(import.meta.url);\n\
             const {bind} = require(process.argv[2]);\n\
             process.stdout.write(JSON.stringify(({expression})));\n"
        );
        fs::write(&driver, script).expect("driver");
        let output = match std::process::Command::new("node")
            .arg(&driver)
            .arg(&file)
            .output()
        {
            Ok(output) => output,
            Err(_) => return None,
        };
        assert!(
            output.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).expect("UTF-8 output");
        Some(
            serde_json::from_str(&text)
                .unwrap_or_else(|error| panic!("driver printed {text:?}: {error}")),
        )
    }

    fn draft_js(expression: &str) -> Option<Value> {
        page_module_js("draft.js", DRAFT_JS, "draft", expression)
    }

    /// S16 (#116): the Ask rules — question bounds, the temporary Table, Keep, and the endpoint
    /// recheck — are the page's own DOM-free code (`src/gui/ask.js`), run here rather than described.
    fn ask_js(expression: &str) -> Option<Value> {
        page_module_js("ask.js", ASK_JS, "ZtAsk", expression)
    }

    /// S28 (#121): the feature-switch rules — what the page takes from `/api/overview.features`,
    /// which command and key belong to which feature — are the page's own DOM-free code
    /// (`src/gui/features.js`), run here rather than described.
    fn features_js(expression: &str) -> Option<Value> {
        page_module_js("features.js", FEATURES_JS, "ZtFeatures", expression)
    }

    /// S01 (#124): the Tree Lock page rules — the seal's short form, the note bound, when a dialog's
    /// plan is still current, what a route's answer means, how a refusal reads, and the panel's
    /// grouping — are the page's own DOM-free code (`src/gui/lock.js`), run here rather than described.
    fn lock_js(expression: &str) -> Option<Value> {
        page_module_js("lock.js", LOCK_JS, "ZtLock", expression)
    }

    #[test]
    fn the_lock_page_shows_twelve_seal_characters_and_checks_the_note_like_the_server() {
        let Some(value) = lock_js(
            "(() => {
               const seal = 'abcdef0123456789'.repeat(4);
               return {
                 short: ZtLock.sealShort(seal),
                 none: ZtLock.sealShort(null),
                 blank: ZtLock.noteCheck('   '),
                 kept: ZtLock.noteCheck(' reviewed 2026-10-21 '),
                 limit: ZtLock.noteCheck('x'.repeat(500)),
                 over: ZtLock.noteCheck('x'.repeat(501)),
                 emoji: ZtLock.noteCheck('\\u{1F512}'.repeat(500)),
                 nul: ZtLock.noteCheck('a\\u0000b'),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["short"], "abcdef012345");
        assert_eq!(value["none"], "");
        assert_eq!(value["blank"]["ok"], true);
        assert_eq!(value["blank"]["note"], Value::Null, "a blank note is no note");
        assert_eq!(value["kept"]["note"], " reviewed 2026-10-21 ", "an accepted note is kept as typed");
        assert_eq!(value["limit"]["ok"], true);
        assert_eq!(value["over"]["ok"], false);
        assert_eq!(value["over"]["count"], 501);
        assert_eq!(value["emoji"]["ok"], true, "a note is counted in Unicode scalar values");
        assert_eq!(value["emoji"]["count"], 500);
        assert_eq!(value["nul"]["ok"], false);
    }

    #[test]
    fn a_lock_dialog_confirms_only_the_plan_for_the_note_in_its_field() {
        let Some(value) = lock_js(
            "(() => {
               const plan = { plan_hash: 'h', note: 'checked' };
               const bare = { plan_hash: 'h', note: null };
               return {
                 same: ZtLock.planCurrent(plan, 'lock', 'checked'),
                 edited: ZtLock.planCurrent(plan, 'lock', 'checked twice'),
                 cleared: ZtLock.planCurrent(plan, 'note', ''),
                 blank: ZtLock.planCurrent(bare, 'lock', '   '),
                 missing: ZtLock.planCurrent(null, 'lock', ''),
                 unhashed: ZtLock.planCurrent({ note: null }, 'unlock', null),
                 unlock: ZtLock.planCurrent({ plan_hash: 'u' }, 'unlock', null),
                 over: ZtLock.planCurrent({ plan_hash: 'h', note: null }, 'lock', 'x'.repeat(501)),
                 applied: ZtLock.answer({ applied: true, lock: {} }),
                 changed: ZtLock.answer({ applied: false, plan_changed: true, plan: {} }),
                 planned: ZtLock.answer({ applied: false, confirm: 'lock', plan: {} }),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["same"], true);
        assert_eq!(value["edited"], false, "a changed note needs a renewed plan");
        assert_eq!(value["cleared"], false);
        assert_eq!(value["blank"], true, "a blank field is the plan's null note");
        assert_eq!(value["missing"], false);
        assert_eq!(value["unhashed"], false, "a plan without its hash is never confirmable");
        assert_eq!(value["unlock"], true);
        assert_eq!(value["over"], false);
        assert_eq!(value["applied"], "applied");
        assert_eq!(value["changed"], "changed");
        assert_eq!(value["planned"], "plan");
    }

    #[test]
    fn a_lock_refusal_reads_with_its_code_and_names_its_tree() {
        let Some(value) = lock_js(
            "{
               locked: ZtLock.refusalText({ code: 'locked', error: 'card `3/1` is in locked tree `3/0` (lock 2); unlock the tree in the GUI first', locked_tree: '3/0', lock_id: 2, pinned_by: [] }),
               given: ZtLock.refusalText({ code: 'locked_tree', locked_tree: '3/0' }, 'outer message'),
               plain: ZtLock.refusalText(null, 'missing `text` field'),
               tree: ZtLock.refusalTree({ code: 'locked', locked_tree: '3/0' }),
               pinned: ZtLock.refusalTree({ code: 'pinned', locked_tree: null, pinned_by: [{ tree: 'Smith2024', lock_id: 4, sources: ['Smith2024/1'] }] }),
               none: ZtLock.refusalTree(null),
             }",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        let locked = value["locked"].as_str().expect("refusal text");
        assert!(locked.starts_with("locked (locked card): card `3/1`"), "{locked}");
        assert!(locked.contains("unlock the tree in the GUI first"), "{locked}");
        assert_eq!(value["given"], "locked_tree (locked tree): outer message");
        assert_eq!(value["plain"], Value::Null, "an ordinary error is not dressed as a refusal");
        assert_eq!(value["tree"], "3/0");
        assert_eq!(value["pinned"], "Smith2024");
        assert_eq!(value["none"], Value::Null);
    }

    #[test]
    fn the_lock_panel_shows_faults_beside_their_lock_without_hiding_any() {
        let Some(value) = lock_js(
            "(() => {
               const exported = {
                 version: 1, schema_version: '1',
                 locks: [
                   { lock_id: 3, root: '3/0', seal: 'c'.repeat(64), unlocked_at: null,
                     members: [{ address: '3/0', row_id: 30 }, { address: '3/1', row_id: 31 }],
                     pins: [{ target: '5/1', target_row: 51, source_row: 30 }, { target: '5/1', target_row: 51, source_row: 31 }, { target: 'Smith2024', target_row: 70, source_row: 31 }],
                     events: [] },
                   { lock_id: 1, root: '3/0', seal: 'a'.repeat(64), unlocked_at: '2026-10-21T10:00:00.000Z', members: [], pins: [], events: [] },
                   { lock_id: 2, root: 'Smith2024', seal: 'b'.repeat(64), unlocked_at: null, members: [], pins: [], events: [] },
                 ],
                 integrity: { ok: false, faults: [
                   { id: 'lock-3:changed:3/1', lock_id: 3, kind: 'changed', card: '3/1', detail: 'fingerprint differs' },
                   { id: 'lock-schema:x', lock_id: null, kind: 'schema', card: 'tree_lock', detail: '' },
                 ] },
               };
               const read = ZtLock.panel(exported);
               return {
                 active: read.active.map((lock) => lock.lock_id),
                 ended: read.ended.map((lock) => lock.lock_id),
                 faultsOf3: (read.faultsOf[3] || []).map((fault) => fault.card),
                 faultsOf2: read.faultsOf[2] || [],
                 loose: read.loose.map((fault) => fault.kind),
                 ok: read.ok,
                 line: ZtLock.faultLine(exported),
                 healthy: ZtLock.faultLine({ locks: [], integrity: { ok: true, faults: [] } }),
                 empty: ZtLock.panel(null),
                 active3: ZtLock.activeLock(exported, '3/0').lock_id,
                 noneActive: ZtLock.activeLock(exported, '9/0'),
                 pins: ZtLock.pinGroups(exported.locks[0]),
                 planPins: ZtLock.pinGroups({ pins: [{ target: '5/1', target_row: 51, sources: ['3/0', '3/1'] }] }),
                 lockEvent: ZtLock.eventLine({ action: 'lock', at: '2026-10-21T09:00:00.123Z', detail: { member_count: 2, seal: 'c'.repeat(64) } }),
                 noteEvent: ZtLock.eventLine({ action: 'note', at: '2026-10-21T09:05:00Z', detail: { previous_note: null, note: 'final' } }),
                 lockOne: ZtLock.eventLine({ action: 'lock', at: '2026-10-21T09:00:00Z', detail: { member_count: 1, seal: 'c'.repeat(64) } }),
                 unlockOne: ZtLock.eventLine({ action: 'unlock', at: '2026-10-21T10:00:00Z', detail: { member_count: 1, pin_count: 1 } }),
                 unlockMany: ZtLock.eventLine({ action: 'unlock', at: '2026-10-21T10:00:00Z', detail: { member_count: 2, pin_count: 3 } }),
                 unlockUnknown: ZtLock.eventLine({ action: 'unlock', at: '2026-10-21T10:00:00Z', detail: {} }),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["active"], json!([2, 3]), "active Locks in creation order");
        assert_eq!(value["ended"], json!([1]), "an ended Lock stays in the history");
        assert_eq!(value["faultsOf3"], json!(["3/1"]));
        assert_eq!(value["faultsOf2"], json!([]));
        assert_eq!(value["loose"], json!(["schema"]), "a fault of no listed Lock is still shown");
        assert_eq!(value["ok"], false);
        assert_eq!(value["line"], "2 lock faults");
        assert_eq!(value["healthy"], Value::Null);
        assert_eq!(value["empty"]["ok"], true);
        assert_eq!(value["active3"], 3, "the active Lock, not the ended one of the same root");
        assert_eq!(value["noneActive"], Value::Null);
        assert_eq!(
            value["pins"],
            json!([
                {"target": "5/1", "target_row": 51, "sources": ["3/0", "3/1"]},
                {"target": "Smith2024", "target_row": 70, "sources": ["3/1"]}
            ]),
            "pins are counted by distinct target, with every locked source"
        );
        assert_eq!(value["planPins"], json!([{"target": "5/1", "target_row": 51, "sources": ["3/0", "3/1"]}]));
        assert_eq!(value["lockEvent"], "2026-10-21 09:00:00Z · locked 2 cards · seal cccccccccccc");
        assert_eq!(value["noteEvent"], "2026-10-21 09:05:00Z · note: none → final");
        assert_eq!(value["lockOne"], "2026-10-21 09:00:00Z · locked 1 card · seal cccccccccccc", "a count of one is singular");
        assert_eq!(value["unlockOne"], "2026-10-21 10:00:00Z · unlocked · released 1 card and 1 pinned target");
        assert_eq!(value["unlockMany"], "2026-10-21 10:00:00Z · unlocked · released 2 cards and 3 pinned targets");
        assert_eq!(value["unlockUnknown"], "2026-10-21 10:00:00Z · unlocked · released ? cards and ? pinned targets");
    }

    /// S06 (#129, S06-AC08, AC05): a latched fault breaks its tree's seal exactly as a current one
    /// does and is labelled with when it was recorded; a current fault and its own record are one
    /// row, and a schema fault latched on every Lock counts once while it is current. The one
    /// page-level line counts current faults plus the latches no current fault repeats.
    #[test]
    fn a_latched_fault_breaks_the_seal_like_a_current_one_and_counts_once() {
        let Some(value) = lock_js(
            "(() => {
               const seal3 = 'c'.repeat(64);
               const exported = {
                 version: 1, schema_version: '1',
                 locks: [
                   { lock_id: 3, root: '3/0', seal: seal3, unlocked_at: null, seal_intact: false, members: [], pins: [], events: [] },
                   { lock_id: 2, root: 'Smith2024', seal: 'b'.repeat(64), unlocked_at: null, seal_intact: false, members: [], pins: [], events: [] },
                   { lock_id: 4, root: '4/0', seal: 'd'.repeat(64), unlocked_at: null, seal_intact: true, members: [], pins: [], events: [] },
                 ],
                 integrity: { ok: false, faulted: true, summary: 'integrity faulted: 2 current faults', faults: [
                   { id: 'lock-3:changed:3/1', lock_id: 3, kind: 'changed', card: '3/1', detail: 'fingerprint differs', expected: 'aa', actual: 'bb' },
                   { id: 'lock-schema:trigger-missing:zt_lock_guard', lock_id: null, kind: 'trigger-missing', card: 'zt_lock_guard', detail: 'the trigger is gone' },
                 ], latched: [
                   { lock_id: 3, id: 'lock-3:changed:3/1', kind: 'changed', card: '3/1', at: '2026-10-21T09:00:00.000Z', detail: 'fingerprint differs' },
                   { lock_id: 3, id: 'lock-3:trigger-missing:zt_lock_guard', kind: 'trigger-missing', card: 'zt_lock_guard', at: '2026-10-21T09:00:00.000Z', detail: 'the trigger is gone' },
                   { lock_id: 2, id: 'lock-2:trigger-missing:zt_lock_guard', kind: 'trigger-missing', card: 'zt_lock_guard', at: '2026-10-21T09:00:00.000Z', detail: 'the trigger is gone' },
                   { lock_id: 2, id: 'lock-2:missing:Smith2024/4', kind: 'missing', card: 'Smith2024/4', at: '2026-10-20T08:30:00.000Z', detail: 'member row is gone' },
                 ] },
               };
               const read = ZtLock.panel(exported);
               const rows = (id) => (read.faultsOf[id] || []).map((row) => ({ kind: row.kind, current: row.current, at: row.at, text: ZtLock.faultText(row) }));
               // The same Lock after a clean comparison: the current faults are gone, the latches stay.
               const clean = { locks: [{ lock_id: 2, root: 'Smith2024', seal: 'b'.repeat(64), unlocked_at: null, members: [], pins: [], events: [] }],
                 integrity: { ok: false, faulted: false, faults: [], latched: [exported.integrity.latched[3]] } };
               const after = ZtLock.panel(clean);
               return {
                 rows3: rows(3), rows2: rows(2), rows4: rows(4),
                 loose: read.loose.map((row) => [row.kind, row.current, row.at]),
                 broken: read.broken, count: read.faultCount, latched: read.latchedCount, ok: read.ok,
                 line: ZtLock.faultLine(exported),
                 seal3: read.active.find((lock) => lock.lock_id === 3).seal === seal3,
                 afterBroken: after.broken, afterLine: ZtLock.faultLine(clean), afterOk: after.ok,
                 afterRows: after.faultsOf[2].map((row) => ZtLock.faultText(row).when),
                 bare: ZtLock.faultText({ kind: 'extra', card: '3/9', current: false }).when,
                 recorded: ZtLock.eventLine({ action: 'integrity-fault', at: '2026-10-21T09:00:00Z', detail: { fault: 'lock-3:changed:3/1', evidence: { kind: 'changed', card: '3/1', detail: 'fingerprint differs' } } }),
                 recordedBare: ZtLock.eventLine({ action: 'integrity-fault', at: '2026-10-21T09:00:00Z', detail: { fault: 'lock-3:changed:3/1' } }),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(
            value["rows3"],
            json!([
                {"kind": "changed", "current": true, "at": "2026-10-21T09:00:00.000Z",
                 "text": {"kind": "changed", "where": "3/1 · fingerprint differs", "when": "now · recorded 2026-10-21 09:00 UTC"}},
                {"kind": "trigger-missing", "current": true, "at": "2026-10-21T09:00:00.000Z",
                 "text": {"kind": "trigger-missing", "where": "zt_lock_guard · the trigger is gone", "when": "now · recorded 2026-10-21 09:00 UTC"}}
            ]),
            "a current fault and its record are one row; the schema latch is the current schema fault"
        );
        assert_eq!(
            value["rows2"][1],
            json!({"kind": "missing", "current": false, "at": "2026-10-20T08:30:00.000Z",
                   "text": {"kind": "missing", "where": "Smith2024/4 · member row is gone", "when": "latched · recorded 2026-10-20 08:30 UTC"}}),
            "a latch alone is labelled with when it was recorded"
        );
        assert_eq!(value["rows4"], json!([]));
        assert_eq!(value["loose"], json!([["trigger-missing", true, "2026-10-21T09:00:00.000Z"]]), "the schema fault is listed once");
        assert_eq!(value["broken"], json!({"2": true, "3": true, "4": true}), "a current schema fault breaks every seal");
        assert_eq!(value["count"], 3, "two current faults and one latch no current fault repeats");
        assert_eq!(value["latched"], 1);
        assert_eq!(value["ok"], false);
        assert_eq!(value["line"], "3 lock faults (1 latched)");
        assert_eq!(value["seal3"], true, "the shown seal is the Lock's own");
        assert_eq!(value["afterBroken"], json!({"2": true}), "a latch alone breaks the seal like a current fault");
        assert_eq!(value["afterLine"], "1 lock fault (1 latched)");
        assert_eq!(value["afterOk"], false, "a later clean comparison never makes a latched Lock healthy");
        assert_eq!(value["afterRows"], json!(["latched · recorded 2026-10-20 08:30 UTC"]));
        assert_eq!(value["bare"], "latched · recorded earlier");
        assert_eq!(value["recorded"], "2026-10-21 09:00:00Z · integrity fault · changed · 3/1 · fingerprint differs");
        assert_eq!(value["recordedBare"], "2026-10-21 09:00:00Z · integrity fault · lock-3:changed:3/1");
    }

    /// S06 (#129, S06-AC09): the owner's recovery stays reachable. An active Lock whose root Card
    /// is gone, or which is faulted, still offers `Edit note…` and `Unlock tree…` in the panel
    /// list; only `Open tree` needs the root. A faulted entry keeps reading, its members, its pins
    /// and its whole history, shows the Lock's own seal, and an ended Lock keeps its recorded
    /// integrity faults in its history under its own Lock id.
    #[test]
    fn unlock_stays_reachable_from_the_panel_for_a_lock_whose_root_is_gone() {
        let Some(value) = lock_js(
            "(() => {
               const lock = { lock_id: 7, root: '3/0', seal: 'c'.repeat(64), unlocked_at: null, seal_intact: false };
               const ids = (items) => items.map((item) => item.id);
               return {
                 gone: ids(ZtLock.entryActions(lock, false)),
                 present: ids(ZtLock.entryActions(lock, true)),
                 danger: ZtLock.entryActions(lock, false).find((item) => item.id === 'unlock'),
                 ended: ZtLock.entryActions({ ...lock, unlocked_at: '2026-10-21T10:00:00Z' }, true),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["gone"], json!(["note", "unlock"]), "the root is gone: unlock stays");
        assert_eq!(value["present"], json!(["open", "note", "unlock"]));
        assert_eq!(value["danger"], json!({"id": "unlock", "label": "Unlock tree…", "danger": true}));
        assert_eq!(value["ended"], json!([]));
        // The panel draws exactly those actions, and the unlock dialog needs no root Card.
        let panel = app_function("lockPanel");
        assert!(panel.contains("ZtLock.entryActions(lock, Boolean(card)).map((item) =>"), "{panel}");
        assert!(panel.contains("unlock: act(() => unlockTree(lock)),"));
        assert!(!panel.contains("card ? h('button', { class: 'btn danger'"), "no unlock depends on the root Card");
        assert!(app_function("existingLockView").contains("card ? card.title : plan.root"), "the unlock plan reads without the root");
        // A faulted entry keeps everything and shows the reviewed seal.
        assert!(panel.contains("seal: lock.seal, broken, note: lock.note, since: lock.locked_at, lockId: lock.lock_id,"));
        assert!(panel.contains("lockFold('Members', view.members.map((member) =>"));
        assert!(panel.contains("lockFold('Pinned targets', pinRows(view.pins), 'lock-scroll'),"));
        assert!(panel.contains("digestFold(lock.seal),"));
        // S04 (#127): active and ended Locks are drawn by the one entry, so both keep their history,
        // and an ended Lock counts its recorded faults and names its own id on the card.
        assert_eq!(panel.matches("history(view)").count(), 1, "one entry draws every history");
        assert!(panel.contains("read.ended.map((lock) => entry(lock, faultsOf(lock), false))"), "an ended Lock is the same entry");
        assert!(panel.contains("view.ended && view.faultsRecorded"), "an ended Lock counts its recorded faults");
        assert!(LOCK_JS.contains("faultsRecorded: events.filter((event) => event && event.action === 'integrity-fault').length,"));
        assert!(panel.contains("lockId: lock.lock_id,"), "an ended Lock names its own id");
        assert!(app_function("lockCardView").contains("text: `lock ${lockId}`"), "a relock shows its new Lock id");
        // A latched fault breaks the seal wherever the seal is stamped.
        assert!(app_function("lockLine").contains("sealStamp(lock.seal, broken)"));
        assert!(app_function("existingLockView").contains("ZtLock.panel(state.locks).broken[plan.lock_id]"));
        assert_eq!(APP_JS.matches("faultsOf[").count(), 1, "only the panel list reads the rows; the seal reads `broken`");
    }

    /// S04 (#127, S04-AC06): an ended Lock reads like an active one in the panel. Its entry shows
    /// the note, the 12-character seal and the full digest, every member the retained `lock` event
    /// captured, the pins grouped by target with their source addresses, the Broken-link warnings
    /// and the whole history in event-id order, none of them cut; it offers no action. The panel
    /// draws both with one entry, names an ended root by its captured address only, and lets every
    /// list scroll in its fold instead of truncating it.
    #[test]
    fn an_ended_lock_shows_its_retained_snapshot_whole_and_offers_no_action() {
        let Some(value) = lock_js(
            "(() => {
               const members = Array.from({ length: 40 }, (_, i) => ({ address: i ? `3/${i}` : '3/0', row_id: 300 + i, fingerprint: String(i).padStart(2, '0').repeat(32) }));
               const ended = {
                 lock_id: 1, root: '3/0', workspace: 'human', note: 'reviewed 2026-10-20', seal: 'a'.repeat(64),
                 member_count: 40, locked_at: '2026-10-20T08:00:00Z', unlocked_at: '2026-10-21T10:00:00Z',
                 members,
                 pins: [{ target: '5/1', target_row: 51, source_row: 301, source: '3/1' }, { target: '5/1', target_row: 51, source_row: 302, source: '3/2' }, { target: 'Smith2024', target_row: 70, source_row: 302, source: '3/2' }],
                 warnings: [{ source: '3/3', target: '9/9' }],
                 events: [
                   { event_id: 12, action: 'unlock', at: '2026-10-21T10:00:00Z', detail: { lock_id: 1, member_count: 40, pin_count: 2 } },
                   { event_id: 10, action: 'lock', at: '2026-10-20T08:00:00Z', detail: { member_count: 40, seal: 'a'.repeat(64) } },
                   { event_id: 11, action: 'integrity-fault', at: '2026-10-20T09:00:00Z', detail: { fault: 'lock-1:changed:3/1' } },
                 ],
               };
               const view = ZtLock.entryView(ended, true);
               const active = ZtLock.entryView({ ...ended, lock_id: 2, unlocked_at: null, note: null, events: [] }, true);
               return {
                 ended: view.ended, note: view.note, short: view.sealShort, seal: view.seal,
                 members: view.members.length, last: view.members[39],
                 pins: view.pins, warnings: view.warnings,
                 history: view.history.map((row) => row.action), firstLine: view.history[0].text,
                 recorded: view.faultsRecorded, actions: view.actions,
                 activeEnded: active.ended, activeNote: active.note, activeActions: active.actions.map((item) => item.id),
                 activeWarnings: active.warnings && active.warnings.rows,
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["ended"], true);
        assert_eq!(value["note"], "reviewed 2026-10-20");
        assert_eq!(value["short"], "aaaaaaaaaaaa", "the seal's 12 characters");
        assert_eq!(value["seal"], "a".repeat(64), "and the full digest");
        assert_eq!(value["members"], 40, "every captured member, none cut");
        assert_eq!(value["last"], json!({"address": "3/39", "fingerprint": "39".repeat(32)}));
        assert_eq!(
            value["pins"],
            json!([
                {"target": "5/1", "target_row": 51, "sources": ["3/1", "3/2"]},
                {"target": "Smith2024", "target_row": 70, "sources": ["3/2"]}
            ])
        );
        assert_eq!(value["warnings"]["rows"], json!([{"source": "3/3", "target": "9/9"}]));
        assert_eq!(value["history"], json!(["lock", "integrity-fault", "unlock"]), "history in event-id order");
        assert_eq!(value["firstLine"], "2026-10-20 08:00:00Z · locked 40 cards · seal aaaaaaaaaaaa");
        assert_eq!(value["recorded"], 1);
        assert_eq!(value["actions"], json!([]), "an ended Lock exposes its facts without write actions");
        assert_eq!(value["activeEnded"], false);
        assert_eq!(value["activeNote"], Value::Null, "no note reads as none");
        assert_eq!(value["activeActions"], json!(["open", "note", "unlock"]));
        assert_eq!(value["activeWarnings"], json!([{"source": "3/3", "target": "9/9"}]), "an active entry gains its warnings");
        // The panel draws every Lock with that one entry: an ended root is its captured address,
        // never today's Card at it (R104), and it is dated at both ends.
        let panel = app_function("lockPanel");
        assert!(panel.contains("const view = ZtLock.entryView(lock, false);"), "{panel}");
        assert!(panel.contains("const card = view.ended ? null : state.byAddress.get(lock.root);"));
        assert!(panel.contains("until: lock.unlocked_at, noteless: true,"));
        assert!(panel.contains("warningFold(view.warnings),"), "active and ended entries show their warnings");
        assert!(panel.contains("actions.length ? h('div', { class: 'lock-entry-actions' }, actions) : null,"));
        let card = app_function("lockCardView");
        assert!(card.contains("until ? h('span', { class: 'lock-sub', text: `unlocked ${dateOf(until)}` }) : null,"));
        assert!(card.contains("noteless ? h('p', { class: 'lock-note-text lock-note-empty', text: 'no note' }) : null"));
        // Every list stays whole: it scrolls in its fold, and nothing slices a list.
        assert_eq!(panel.matches("'lock-scroll'").count(), 3, "members, pins and history scroll");
        assert!(APP_CSS.contains(".lock-fold.lock-scroll > .lock-list, .lock-fold.warn > .lock-list { max-height: 15em; overflow-y: auto;"));
        assert_eq!(panel.matches(".slice(0, ").count(), 1, "only a fingerprint is shortened, its whole value in its title");
        assert!(panel.contains("title: member.fingerprint, text: member.fingerprint.slice(0, 12)"));
        assert_eq!(LOCK_JS.matches(".slice(0, ").count(), 1, "only the seal's short form cuts, and the digest stays beside it");
        assert!(APP_CSS.contains(".lock-entry.lock-ended .lock-card {"), "an ended Lock recedes a step");
    }

    /// S04 (#127): a pin names its source by the most direct fact it carries: a plan's `sources`,
    /// then a stored row's `source` address (active or ended), and only then the Lock's own member
    /// rows. A source the members do not name stays visible as its row.
    #[test]
    fn lock_pins_prefer_plan_sources_then_the_stored_source_then_the_member_rows() {
        let Some(value) = lock_js(
            "(() => {
               const members = [{ address: '3/0', row_id: 30 }, { address: '3/1', row_id: 31 }];
               return {
                 plan: ZtLock.pinGroups({ members, pins: [{ target: '5/1', target_row: 51, sources: ['3/0'], source: '3/1', source_row: 31 }] }),
                 stored: ZtLock.pinGroups({ members, pins: [{ target: '5/1', target_row: 51, source: '3/7', source_row: 31 }] }),
                 rows: ZtLock.pinGroups({ members, pins: [{ target: '5/1', target_row: 51, source_row: 31 }, { target: '5/1', target_row: 51, source_row: 99 }] }),
                 mixed: ZtLock.pinGroups({ members, pins: [{ target: '5/1', target_row: 51, source: '3/0', source_row: 30 }, { target: '5/1', target_row: 51, source_row: 31 }] }),
                 none: ZtLock.pinGroups(null),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["plan"], json!([{"target": "5/1", "target_row": 51, "sources": ["3/0"]}]), "a plan's sources first");
        assert_eq!(
            value["stored"],
            json!([{"target": "5/1", "target_row": 51, "sources": ["3/7"]}]),
            "the stored source address, not today's member row"
        );
        assert_eq!(value["rows"], json!([{"target": "5/1", "target_row": 51, "sources": ["3/1", "row 99"]}]), "the member row map last");
        assert_eq!(value["mixed"], json!([{"target": "5/1", "target_row": 51, "sources": ["3/0", "3/1"]}]));
        assert_eq!(value["none"], json!([]));
    }

    /// S04 (#127): the Broken Links a Lock accepted fold the same way in the plan and in the
    /// panel, every row kept; no warnings is no fold.
    #[test]
    fn the_lock_warnings_fold_keeps_every_broken_link_in_the_plan_and_the_panel() {
        let Some(value) = lock_js(
            "(() => {
               const warnings = Array.from({ length: 25 }, (_, i) => ({ source: `3/${i + 1}`, target: `9/${i + 1}` }));
               return {
                 plan: ZtLock.warningsFold(warnings, 'plan'),
                 lock: ZtLock.warningsFold(warnings.slice(0, 1), 'lock'),
                 empty: ZtLock.warningsFold([], 'lock'),
                 missing: ZtLock.warningsFold(undefined, 'plan'),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["plan"]["rows"].as_array().map(Vec::len), Some(25), "every warning, none cut");
        assert_eq!(value["plan"]["rows"][24], json!({"source": "3/25", "target": "9/25"}));
        assert_eq!(
            value["plan"]["label"],
            "Already broken links · 25 — they create no pin; if one resolves later, unlock and lock again to include it"
        );
        assert_eq!(
            value["lock"]["label"],
            "Broken links when locked · 1 — they hold no pin; unlock and lock again to include one that resolves"
        );
        assert_eq!(value["empty"], Value::Null);
        assert_eq!(value["missing"], Value::Null);
        let fold = app_function("warningFold");
        assert!(fold.contains("h('details', { class: 'lock-fold warn' },"), "{fold}");
        assert!(fold.contains("fold.rows.map((warning) =>"));
        assert!(fold.contains("text: `${warning.target} (missing)`"));
        assert_eq!(APP_JS.matches("warningFold(").count(), 3, "the helper, the plan and the panel");
    }

    /// S04 (#127, R105): a committed Lock, unlock or note whose after-commit `zt.log` mirror failed
    /// shows the action's own line and then one separate warning line that never says the action
    /// was undone; an answer without `log_error` adds nothing.
    #[test]
    fn a_lock_log_error_shows_as_one_separate_warning_line() {
        let Some(value) = lock_js(
            "({
               failed: ZtLock.logWarning({ applied: true, lock: { lock_id: 4 }, log_error: 'open zt.log\\n\\nCaused by:\\n    Access is denied. (os error 5)' }),
               clean: ZtLock.logWarning({ applied: true, lock: { lock_id: 4 } }),
               blank: ZtLock.logWarning({ applied: true, lock: {}, log_error: '  ' }),
               none: ZtLock.logWarning(null),
             })",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        let failed = value["failed"].as_str().expect("a warning");
        assert_eq!(
            failed,
            "Saved in the archive, but its zt.log line was not written: open zt.log Caused by: Access is denied. (os error 5)"
        );
        assert!(!failed.contains('\n'), "one line");
        assert!(!failed.contains("rolled back") && !failed.contains("not saved"), "the committed action is never undone");
        assert_eq!(value["clean"], Value::Null);
        assert_eq!(value["blank"], Value::Null);
        assert_eq!(value["none"], Value::Null);
        // The dialog hands back the whole committed answer; each flow passes it on, and the
        // warning is its own toast after the action's line.
        let dialog = app_function("lockDialog");
        assert!(dialog.contains("if (ZtLock.answer(answer) === 'applied') { finish(answer); return; }"), "{dialog}");
        let after = app_function("afterLock");
        let said = after.find("toast(message);").expect("the action's line");
        let warned = after.find("if (warning) toast(warning, 'warn');").expect("the separate warning");
        assert!(said < warned && after.contains("const warning = ZtLock.logWarning(answer);"), "{after}");
        assert!(app_function("editLockNote").contains("await afterLock(`note changed on ${noted.root}`, done);"));
        assert!(app_function("lockTree").contains(", done);"));
        assert!(app_function("unlockTree").contains("await afterLock(`unlocked ${ended.root}`, done);"));
        assert!(app_function("toast").contains("kind === 'warn' ? ' warn' : ''"));
        assert!(APP_CSS.contains(".toast.warn { border-left: 4px solid var(--broken); }"));
    }

    /// S01 (#124): the page's Lock dialogs send nothing on Cancel, confirm only with the plan's own
    /// word and hash, keep the note through a renewed plan, and a refused Save keeps the draft.
    #[test]
    fn the_lock_dialogs_confirm_with_the_plan_hash_and_cancel_writes_nothing() {
        let lock_js = INDEX_HTML.find("<script src=\"/lock.js\" defer></script>").expect("lock.js");
        let app = INDEX_HTML.find("<script src=\"/app.js\" defer></script>").expect("app.js");
        assert!(lock_js < app, "the rules load before the page");
        assert!(GUI_RS.contains("(Method::Get, \"/lock.js\") => {"));
        let dialog = app_function("lockDialog");
        assert!(
            dialog.contains("api(path, { ...body(noteValue()), confirm: action, plan_hash: plan.plan_hash })"),
            "a confirmation carries the word and the reviewed plan's hash: {dialog}"
        );
        assert!(dialog.contains("cancel.addEventListener('click', () => finish(null));"), "Cancel only closes");
        assert!(dialog.contains("ok.disabled = busy || !fresh || input.value.trim() !== action;"));
        assert!(dialog.contains("const fresh = ZtLock.planCurrent(plan, action, noteValue());"));
        assert_eq!(dialog.matches("note.value =").count(), 1, "only the opening sets the note");
        assert_eq!(dialog.matches("api(").count(), 2, "one plan read and one confirmation");
        let changed = dialog.find("show(answer);").expect("a renewed plan replaces the old one");
        assert!(changed > dialog.find("ZtLock.answer(answer) === 'applied'").expect("applied check"));
        // Every Lock route is reached only through the dialog; nothing else confirms a Lock.
        assert_eq!(APP_JS.matches("plan_hash: ").count(), 1, "one place sends a plan hash");
        assert!(APP_JS.contains("path: 'lock', verb: 'Lock tree'"));
        assert!(APP_JS.contains("path: 'unlock', verb: 'Unlock tree'"));
        assert!(APP_JS.contains("path: 'lock/note', verb: 'Change note'"));
        assert!(APP_JS.contains("body: () => ({ lock_id: lock.lock_id, tree: lock.root })"), "an unlock names its exact Lock");
        // A refusal keeps its facts on the error, and the writing desk keeps the draft.
        let api = app_function("api");
        assert!(api.contains("if (data.refusal) error.refusal = data.refusal;"), "{api}");
        assert!(api.contains("ZtLock.refusalText(data.refusal, data.error)"));
        let editor = app_function("openEditor");
        // A Card-check refusal (S04, #146) shows its own headline and offers no Lock panel.
        let refused = editor.find("if (error.refusal && !refused) {").expect("refusal branch");
        assert!(editor[refused..].contains("lockPanel(ZtLock.refusalTree(error.refusal))"), "the panel opens at the refused tree");
        let catch = &editor[..refused];
        assert!(catch.rfind("setMsg(refused ? refused.headline : error.message, 'err');").is_some());
        // The panel is an integrity check point: it reads the Locks every time it opens.
        assert!(app_function("lockPanel").contains("await loadLocks();"));
    }

    /// S02 (#125, S02-AC06): the lock plan dialog says, right under what the Lock freezes, what
    /// the seal promise does not cover, in the same words as the plan's own last line; the
    /// already-broken-links fold stays as it was.
    #[test]
    fn the_lock_plan_names_what_the_seal_promise_leaves_out() {
        let exclusion = "its meaning, and any external document, artifact or cache availability, stay outside the seal promise";
        assert!(
            include_str!("tree_lock.rs").contains(&format!("pins keep a target's address and existence only: {exclusion}")),
            "the API's plan lines carry the same exclusion"
        );
        let view = app_function("lockPlanView");
        let what = view.find("class: 'lock-what', text: `Freezes the root and").expect("the freeze line");
        let limit = view
            .find(&format!("class: 'lock-limit', text: 'Pins keep a target\\'s address and existence only: {exclusion}.'"))
            .expect("the exclusion sentence");
        let digest = view.find("digestFold(plan.seal)").expect("the digest fold");
        assert!(what < limit && limit < digest, "the exclusion sits right under the freeze line: {view}");
        // S04 (#127): the plan and the panel share one warnings fold; the plan's words are unchanged.
        assert!(view.contains("warningFold(ZtLock.warningsFold(plan.warnings, 'plan')),"), "{view}");
        assert!(
            LOCK_JS.contains("`Already broken links · ${rows.length} — they create no pin; if one resolves later, unlock and lock again to include it`"),
            "the broken-link fold is unchanged"
        );
        assert!(APP_CSS.contains(".lock-limit { margin: 0 0 4px; font-size: 12px;"), "the limit recedes below the freeze line");
        assert_eq!(APP_JS.matches("lock-limit").count(), 1);
    }

    /// The text of one top-level `function <name>` of `app.js`, up to its closing brace (the
    /// first line that starts with two spaces and `}`), so a page test can say where in that
    /// function a guard stands.
    fn app_function(name: &str) -> &'static str {
        let start = APP_JS
            .find(&format!("function {name}("))
            .unwrap_or_else(|| panic!("app.js has no function {name}"));
        let end = APP_JS[start..]
            .find("\n  }")
            .unwrap_or_else(|| panic!("function {name} does not close"));
        &APP_JS[start..start + end]
    }

    /// S03 (#126): a Card's own `lock` and `pinned_by` read as one quiet line. A member names its
    /// Locked tree and since; a Pinned target names each pinning root once and keeps editing open;
    /// a Card can be both; an unlocked, unpinned Card has no line.
    #[test]
    fn a_cards_lock_facts_read_as_one_quiet_line_with_its_tree_and_pins() {
        let Some(value) = lock_js(
            "(() => {
               const since = '2026-10-21T09:00:00Z';
               return {
                 free: ZtLock.cardState({ address: '3/1', lock: null, pinned_by: [] }),
                 old: ZtLock.cardState({ address: '3/1' }),
                 member: ZtLock.cardState({ address: '3/1', lock: { lock_id: 2, tree: '3/0', since }, pinned_by: [] }),
                 root: ZtLock.cardState({ address: '3/0', lock: { lock_id: 2, tree: '3/0', since }, pinned_by: [] }),
                 pinned: ZtLock.cardState({ address: '5/1', lock: null, pinned_by: ['3/0', '3/0'] }),
                 both: ZtLock.cardState({ address: '7/1', lock: { lock_id: 4, tree: '7/0', since }, pinned_by: ['3/0', 'Smith2024'] }),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["free"], Value::Null, "an unlocked, unpinned Card has no lock line");
        assert_eq!(value["old"], Value::Null, "a Card read without the fields has no lock line");
        assert_eq!(value["member"]["locked"], true);
        assert_eq!(value["member"]["root"], false);
        assert_eq!(value["member"]["tree"], "3/0");
        assert_eq!(value["member"]["since"], "2026-10-21T09:00:00Z");
        assert!(value["member"]["lockedWhy"].as_str().unwrap().contains("until tree 3/0 is unlocked"));
        assert_eq!(value["member"]["pinnedBy"], json!([]));
        assert_eq!(value["root"]["root"], true, "the root's own lock line already says it");
        assert_eq!(value["pinned"]["locked"], false);
        assert_eq!(value["pinned"]["pinnedBy"], json!(["3/0"]), "one root once");
        assert_eq!(value["pinned"]["pinnedText"], "Pinned by locked tree 3/0");
        assert!(value["pinned"]["pinnedWhy"].as_str().unwrap().starts_with("Editing stays open."));
        assert_eq!(value["both"]["locked"], true);
        assert_eq!(value["both"]["pinnedBy"], json!(["3/0", "Smith2024"]));
        assert_eq!(value["both"]["pinnedText"], "Pinned by locked trees 3/0, Smith2024");
    }

    /// S03 (#126): a Lock-only change repaints through the existing refresh. The 4 s watch compares
    /// the overview's fingerprint (which now carries the archive revision) and calls `refresh`;
    /// `refresh` reads the Locks with the overview, so no second poll exists. The reader shows the
    /// Card's own lock facts, and the cabinet keeps a Locked tree's drawer and says `locked`.
    #[test]
    fn a_lock_only_change_repaints_through_the_existing_refresh() {
        let refresh = app_function("refresh");
        assert!(refresh.contains("api('locks').catch(() => null)"), "refresh reads the Locks");
        assert!(refresh.contains("state.locks = locks;"));
        assert!(refresh.contains("paintLocks();"));
        let watch = app_function("watchForChanges");
        assert!(watch.contains("if (overview.fingerprint !== state.fingerprint) await refresh();"));
        assert!(!watch.contains("api('locks')"), "the 4 s poll itself reads only the overview");
        assert_eq!(APP_JS.matches("setInterval(").count(), 2, "no new polling loop");
        assert!(APP_JS.contains("setInterval(watchForChanges, 4000);"));
        assert!(APP_JS.contains("      cardLockLine(card),"), "the reader shows the Card's lock line");
        let rail = app_function("renderRail");
        assert!(rail.contains("(tree.locked ? ' locked' : '')"), "a Locked tree's drawer is marked");
        assert!(rail.contains("`${tree.count} · locked`"), "in words, not only by colour");
        assert!(!rail.contains("filter((tree) => !tree.locked"), "a Locked tree is never hidden");
    }

    /// S05 (#128, S05-AC04/AC05): a Card in a Locked tree (its root included) offers no write, and
    /// each explanation names the Locked tree and its unlock. A Pinned target keeps editing, type
    /// changes and new successors; only moving, inserting in its place and deleting are refused,
    /// naming the pinning tree and the source Cards that hold the Links.
    #[test]
    fn a_locked_card_offers_no_write_and_a_pin_refuses_only_movement() {
        let Some(value) = lock_js(
            "(() => {
               const exported = { locks: [{ lock_id: 2, root: '3/0', seal: 'ab'.repeat(32), unlocked_at: null,
                 members: [{ address: '3/0', row_id: 30 }, { address: '3/1', row_id: 31 }, { address: '3/2', row_id: 32 }],
                 pins: [{ target: '5/1', target_row: 51, source_row: 31 }, { target: '5/1', target_row: 51, source_row: 32 }] }] };
               const member = { address: '3/1', lock: { lock_id: 2, tree: '3/0', since: 'x' }, pinned_by: [] };
               const root = { address: '3/0', lock: { lock_id: 2, tree: '3/0', since: 'x' }, pinned_by: [] };
               const pinned = { address: '5/1', lock: null, pinned_by: ['3/0'] };
               const free = { address: '6/1', lock: null, pinned_by: [] };
               const all = Object.keys(ZtLock.WRITES);
               const each = (card) => Object.fromEntries(all.map((action) => [action, ZtLock.writeGate(card, action, exported)]));
               let unknown = null;
               try { ZtLock.writeGate(free, 'publish', exported); } catch (error) { unknown = error.message; }
               return {
                 all, member: each(member), root: each(root), pinned: each(pinned), free: each(free),
                 unread: ZtLock.writeGate(pinned, 'delete', null),
                 commands: ZtLock.COMMAND_WRITES, unknown,
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        let all: Vec<&str> = value["all"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(
            all,
            ["edit", "bibtex", "type", "level", "form", "direct", "side", "insert", "move", "delete"],
            "every write the reader, the keys and the command bar offer"
        );
        for action in &all {
            for card in ["member", "root"] {
                let gate = &value[card][action];
                assert_eq!(gate["allowed"], false, "{card} {action}");
                assert_eq!(gate["code"], "locked");
                assert_eq!(gate["tree"], "3/0");
                let why = gate["why"].as_str().unwrap();
                assert!(why.contains("locked with tree 3/0") && why.contains("Unlock it from the tree's lock panel"), "{why}");
            }
            assert_eq!(value["free"][action]["allowed"], true, "an unlocked, unpinned Card keeps {action}");
        }
        for open in ["edit", "bibtex", "type", "level", "form", "direct", "side"] {
            assert_eq!(value["pinned"][open]["allowed"], true, "a pin is not a whole-tree Lock: {open} stays open");
        }
        for refused in ["insert", "move", "delete"] {
            let gate = &value["pinned"][refused];
            assert_eq!(gate["allowed"], false, "{refused}");
            assert_eq!(gate["code"], "pinned");
            assert_eq!(gate["holders"], json!([{"tree": "3/0", "lockId": 2, "sources": ["3/1", "3/2"]}]));
            let why = gate["why"].as_str().unwrap();
            assert!(why.contains("pinned by locked tree 3/0 (Links from 3/1, 3/2)"), "{why}");
            assert!(why.contains("Editing it and adding successors stay open"), "{why}");
        }
        assert_eq!(value["unread"]["allowed"], false, "the Card's own pin facts decide, even before the Locks are read");
        assert_eq!(value["unread"]["holders"], json!([{"tree": "3/0", "lockId": null, "sources": []}]));
        assert_eq!(
            value["commands"],
            json!({"e": "edit", "ty": "type", "lv": "level", "mf": "form", "n": "direct", "b": "side", "ins": "insert", "mv": "move", "del": "delete"})
        );
        assert_eq!(value["unknown"], "unknown write `publish`");
    }

    /// S05 (#128, S05-AC04): every entry point to a write asks the same gate — the reader's buttons
    /// (visible but disabled, titled with the reason), the keys, the command bar (listed, marked and
    /// explained) and the functions they all reach — and a refused write offers the tree's lock
    /// panel. Reading, the Table and Reply are never gated (S05-AC06).
    #[test]
    fn every_write_entry_point_asks_the_same_lock_gate() {
        for (name, action) in [
            ("editCard", "gateWrite(card, 'edit');"),
            ("editMetadata", "gateWrite(card, 'bibtex');"),
            ("setCardType", "gateWrite(card, 'type');"),
            ("deleteCard", "gateWrite(card, 'delete');"),
            ("moveCard", "gateWrite(card, 'move');"),
            ("insertCard", "gateWrite(card, 'insert');"),
            ("newCard", "if (kind !== 'topic') gateWrite(parent, kind);"),
        ] {
            let body = app_function(name);
            let gate = body.find(action).unwrap_or_else(|| panic!("{name} asks the gate: {body}"));
            if let Some(write) = body.find("api(") {
                assert!(gate < write, "{name} asks before any request");
            }
            if let Some(desk) = body.find("openEditor(") {
                assert!(gate < desk, "{name} asks before the writing desk opens");
            }
        }
        let command = app_function("runCommand");
        let gate = command
            .find("if (state.card && ZtLock.COMMAND_WRITES[name]) gateWrite(state.card, ZtLock.COMMAND_WRITES[name]);")
            .expect("the command bar asks the gate for every write word");
        assert!(gate < command.find("switch (name) {").unwrap(), "before any usage check");
        assert!(command.contains("gateWrite(state.card, 'level');") && command.contains("gateWrite(state.card, 'form');"));
        let keys = app_function("installKeys");
        for key in [
            "case 'n': return run(() => runCommand('n'));",
            "case 'b': return run(() => runCommand('b'));",
            "case 'e': return run(() => runCommand('e'));",
            "case 'm': return card ? run(() => moveCard()) : undefined;",
            "case 'i': return card && card.context === 'regular' ? run(insertCard) : undefined;",
            "case 'Delete': return card ? run(deleteCard) : undefined;",
        ] {
            assert!(keys.contains(key), "each write key reaches a gated function: {key}");
        }
        let reader = app_function("renderReader");
        assert!(reader.contains("const gate = ZtLock.writeGate(card, action, state.locks);"));
        assert!(reader.contains("{ ...base, disabled: true, title: gate.why, cls: [base.cls, 'gated'].filter(Boolean).join(' ') }"));
        for button in [
            "actionButton('Edit', 'e', editCard, writeOpts('edit', { primary: true })),",
            "actionButton('Direct', 'n', () => newCard('direct'), writeOpts('direct',",
            "actionButton('Side', 'b', () => newCard('side'), regular ? writeOpts('side',",
            "actionButton('Insert', 'i', insertCard, regular ? writeOpts('insert',",
            "actionButton('Move', 'm', () => moveCard(), regular ? writeOpts('move',",
            "actionButton('Delete', 'del', deleteCard, writeOpts('delete', { danger: true })),",
        ] {
            assert!(reader.contains(button), "the button stays visible and asks the gate: {button}");
        }
        assert!(reader.contains("actionButton('Reply', 'r', replyTo, {"), "Reply is never gated");
        assert!(reader.contains("actionButton(spreads.has(card.address) ? 'On the Table ✓' : 'To the Table', 'p'"), "the Table is never gated");
        assert_eq!(APP_JS.matches("writeOpts(").count(), 6, "the six write buttons, and no other");
        // The command bar lists a gated write, marks it and explains it at once.
        let bar = app_function("installCommandBar");
        assert!(bar.contains("desc: gated ? `unavailable — ${gated.short}` : desc"));
        assert!(bar.contains("fail(Object.assign(new Error(item.gate.why), { gate: item.gate }));"));
        // A refusal — the page's own gate or the server's — offers the tree's lock panel.
        let fail = &APP_JS[APP_JS.find("const fail = (error) => {").expect("fail")..];
        let fail = &fail[..fail.find("\n  };").expect("fail ends")];
        assert!(fail.contains("error.gate ? error.gate.tree : error.refusal ? ZtLock.refusalTree(error.refusal) : null"), "{fail}");
        assert!(fail.contains("{ label: 'Lock panel', run: () => lockPanel(tree).catch(fail) }"));
        assert!(APP_CSS.contains(".btn.gated:disabled { border-style: dashed; }"), "a held slot, not only a paler colour");
    }

    /// S05 (#128, S05-AC06/AC07): a human Topic inside a Locked tree is shown and explained but
    /// not choosable for a Reply; a remembered locked Topic stays chosen with the reason, never
    /// replaced. A Save refused because the destination was locked meanwhile keeps the draft, names
    /// the refusal, offers the lock panel and is never retried on its own.
    #[test]
    fn a_locked_reply_destination_is_explained_and_never_replaced() {
        let Some(value) = lock_js(
            "{
               locked: ZtLock.destinationLock({ address: '1/0', lock: { lock_id: 3, tree: '1/0', since: 'x' }, pinned_by: [] }),
               pinned: ZtLock.destinationLock({ address: '2/0', lock: null, pinned_by: ['3/0'] }),
               free: ZtLock.destinationLock({ address: '4/0', lock: null, pinned_by: [] }),
             }",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        assert_eq!(value["locked"]["tree"], "1/0");
        let why = value["locked"]["why"].as_str().unwrap();
        assert!(why.contains("Topic 1/0 is in locked tree 1/0") && why.contains("your draft stays"), "{why}");
        assert_eq!(value["pinned"], Value::Null, "a pin leaves a Topic open for a reply");
        assert_eq!(value["free"], Value::Null);
        let editor = app_function("openEditor");
        assert!(editor.contains("value: String(index), disabled: Boolean(locked), title: locked ? locked.why : null,"));
        assert!(editor.contains("text: `${topic.title || '(untitled)'} · ${topic.address}${locked ? ' · locked' : ''}`"));
        assert!(editor.contains("paintDestination();"), "a remembered locked Topic is explained as the desk opens");
        assert!(
            !app_function("replyDestinations").contains("lock"),
            "the destination list is not filtered, so the chosen Topic is never silently replaced"
        );
        let reply = app_function("replyTo");
        assert!(reply.contains("if (result.reason === 'locked_destination' && result.refusal) error.refusal = result.refusal;"));
        assert!(
            !reply.contains("unlockTree(") && !reply.contains("api('unlock'") && !reply.contains("api('lock"),
            "a refused destination never unlocks anything"
        );
        // The desk saves only on Save or Ctrl+S: no retry after a refusal or an unlock.
        assert_eq!(editor.matches("doSave").count(), 3, "defined once, wired to the button and to Ctrl+S");
        assert!(editor.contains("saveBtn.addEventListener('click', doSave);"));
        let refused = editor.find("if (error.refusal && !refused) {").expect("the refusal branch");
        assert!(editor[refused..].contains("text: 'Lock panel'"));
    }

    /// S05 (#128, S05-AC01/AC03): each tree's cabinet menu offers Lock tree, or the active Lock's
    /// panel, note and unlock; a Locked tree wears a paper band with its seal in words on the desk
    /// and a strap in the cabinet; a Lock action repaints through the normal refresh (R222).
    #[test]
    fn the_cabinet_menu_and_band_follow_each_trees_lock() {
        let Some(value) = lock_js(
            "(() => {
               const exported = { locks: [{ lock_id: 5, root: '3/0', seal: '0123456789abcdef'.repeat(4), unlocked_at: null, members: [], pins: [] },
                                           { lock_id: 1, root: '7/0', seal: 'f'.repeat(64), unlocked_at: '2026-10-21T10:00:00Z', members: [], pins: [] }] };
               const ids = (items) => items.map((item) => item.id);
               return {
                 locked: ZtLock.treeActions('3/0', true, exported),
                 ended: ids(ZtLock.treeActions('7/0', false, exported)),
                 unread: ids(ZtLock.treeActions('3/0', true, null)),
                 free: ids(ZtLock.treeActions('9/0', false, exported)),
                 band: ZtLock.bandText(exported.locks[0]),
                 bare: ZtLock.bandText(null),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the lock rules were not run");
            return;
        };
        let locked: Vec<&str> = value["locked"].as_array().unwrap().iter().map(|item| item["id"].as_str().unwrap()).collect();
        assert_eq!(locked, ["open", "panel", "note", "unlock"]);
        assert_eq!(value["locked"][1]["note"], "seal 0123456789ab");
        assert_eq!(value["locked"][3]["label"], "Unlock tree…");
        assert_eq!(value["ended"], json!(["open", "lock"]), "an ended Lock leaves the tree unlocked");
        assert_eq!(value["unread"], json!(["open", "panel"]), "unread Locks: the panel reads them");
        assert_eq!(value["free"], json!(["open", "lock"]));
        assert_eq!(value["band"], "locked · seal 0123456789ab", "the band says it in words");
        assert_eq!(value["bare"], "locked");
        // The menu opens the same dialogs as the lock line and the panel, on drawers and stacks.
        let menu = &APP_JS[APP_JS.find("const treeMenu = (() => {").expect("tree menu")..];
        let menu = &menu[..menu.find("\n  })();").expect("menu end")];
        for call in ["await lockTree(tree.root);", "await lockPanel(tree.root);", "await editLockNote(lock);", "await unlockTree(lock);"] {
            assert!(menu.contains(call), "{call}");
        }
        for entry in ["'contextmenu'", "event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')", "}, 500);"] {
            assert!(menu.contains(entry), "right-click, the menu key and a long press: {entry}");
        }
        let arrows = menu.find("if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {").expect("the menu's arrows");
        let walk = menu[arrows..].find("items[(at").expect("walk");
        assert!(
            menu[arrows..arrows + walk].contains("event.stopPropagation();"),
            "the menu's arrows never reach the page's keys, which would walk the Map"
        );
        let rail = app_function("renderRail");
        assert!(rail.contains("lockBand('drawer', tree, lock),") && rail.contains("treeMenu.wire(el, tree);"));
        let desk = app_function("renderDesk");
        assert!(desk.contains("const stack = (tree) => treeMenu.wire(h('button', {"));
        assert!(desk.contains("lockBand('desk', tree, tree.locked ? ZtLock.activeLock(state.locks, tree.root) : null),"));
        let band = app_function("lockBand");
        assert!(band.contains("h('span', { class: 'lbl', text: 'locked' })"), "visible text, not colour alone");
        assert!(
            band.contains("class: `lock-band band-${where}` + (motion ? ` band-${motion}` : '')"),
            "the band's own class names: `.drawer`, `.desk`, `.on` and `.off` belong to the cabinet and desk"
        );
        assert!(!APP_CSS.contains(".lock-band.drawer") && !APP_CSS.contains(".lock-band.desk"));
        assert!(band.contains("text: `seal ${ZtLock.sealShort(lock.seal)}`"));
        assert!(app_function("paintLocks").contains("bands.take(state.locks);"), "motion only for a changed Lock");
        let bands = &APP_JS[APP_JS.find("const bands = {").expect("bands")..];
        let bands = &bands[..bands.find("\n  };").expect("bands end")];
        assert!(
            bands.contains("requestAnimationFrame(") && bands.contains("for (const done of this.painting) this.played.add(done);"),
            "a refresh paints the rail twice in one frame; the motion survives to the frame the eye sees"
        );
        let after = app_function("afterLock");
        assert!(after.contains("await refresh().catch(fail);"), "the normal refresh repaints every copy: {after}");
        assert!(!after.contains("renderReader"));
        for rule in [".lock-band.band-desk {", ".lock-band.band-drawer {", ".stack.locked:hover .face { transform: none;", "@keyframes band-wrap", "@keyframes band-off"] {
            assert!(APP_CSS.contains(rule), "{rule}");
        }
        assert_eq!(APP_CSS.matches("--band: ").count(), 3, "light and both dark blocks");
        assert_eq!(APP_CSS.matches("--wax: ").count(), 3);
    }

    #[test]
    fn the_page_reads_each_feature_switch_from_the_overview_and_keeps_it_on_by_default() {
        let Some(value) = features_js(
            "{
               on: ZtFeatures.read(null),
               older: ZtFeatures.read({ fingerprint: 'x' }),
               one: ZtFeatures.read({ features: { search: false, ask: true, reply: true } }),
               all: ZtFeatures.read({ features: { search: false, ask: false, reply: false } }),
               odd: ZtFeatures.read({ features: { search: 'off', ask: null } }),
               same: ZtFeatures.changed(ZtFeatures.read(null), ZtFeatures.read({ features: { search: true, ask: true, reply: true } })),
               moved: ZtFeatures.changed(ZtFeatures.read(null), ZtFeatures.read({ features: { reply: false } })),
             }",
        ) else {
            eprintln!("node is not installed; the feature rules were not run");
            return;
        };
        let all_on = serde_json::json!({"search": true, "ask": true, "reply": true});
        assert_eq!(value["on"], all_on, "before any answer every feature is on");
        assert_eq!(value["older"], all_on, "a service without `features` leaves them on");
        assert_eq!(value["one"], serde_json::json!({"search": false, "ask": true, "reply": true}));
        assert_eq!(value["all"], serde_json::json!({"search": false, "ask": false, "reply": false}));
        assert_eq!(value["odd"], all_on, "only an exact `false` turns a feature off");
        assert_eq!(value["same"], false);
        assert_eq!(value["moved"], true);
    }

    #[test]
    fn each_command_and_key_belongs_to_its_own_feature() {
        let Some(value) = features_js(
            "{
               slash: ZtFeatures.forKey('/', false),
               ctrlK: ZtFeatures.forKey('k', true),
               ctrlShiftK: ZtFeatures.forKey('K', true),
               r: ZtFeatures.forKey('r', false),
               ctrlR: ZtFeatures.forKey('r', true),
               n: ZtFeatures.forKey('n', false),
               k: ZtFeatures.forKey('k', false),
               ask: ZtFeatures.forCommand('ask'),
               reply: ZtFeatures.forCommand('reply'),
               go: ZtFeatures.forCommand('go'),
               offAsk: ZtFeatures.allows({ search: true, ask: false, reply: true }, 'ask'),
               offOther: ZtFeatures.allows({ search: true, ask: false, reply: true }, 'reply'),
               none: ZtFeatures.allows({ search: false, ask: false, reply: false }, null),
               note: ZtFeatures.offNote('ask'),
             }",
        ) else {
            eprintln!("node is not installed; the feature rules were not run");
            return;
        };
        assert_eq!(value["slash"], "search");
        assert_eq!(value["ctrlK"], "search");
        assert_eq!(value["ctrlShiftK"], "search");
        assert_eq!(value["r"], "reply");
        assert_eq!(value["ctrlR"], Value::Null, "Ctrl+R still reloads the page");
        assert_eq!(value["n"], Value::Null);
        assert_eq!(value["k"], Value::Null);
        assert_eq!(value["ask"], "ask");
        assert_eq!(value["reply"], "reply");
        assert_eq!(value["go"], Value::Null);
        assert_eq!(value["offAsk"], false);
        assert_eq!(value["offOther"], true, "one switch gates only its own feature");
        assert_eq!(value["none"], true, "a key of no feature is never gated");
        let note = value["note"].as_str().expect("note");
        assert!(note.contains("zt config set gui_ask on"), "{note}");
    }

    #[test]
    fn a_switch_turned_back_on_shows_on_the_next_overview_poll() {
        let Some(value) = features_js(
            "(() => {
               const off = ZtFeatures.read({ features: { search: false, ask: false, reply: false } });
               const back = ZtFeatures.read({ features: { search: true, ask: true, reply: true } });
               return { moved: ZtFeatures.changed(off, back), back,
                        search: ZtFeatures.allows(back, ZtFeatures.forKey('/', false)),
                        reply: ZtFeatures.allows(back, ZtFeatures.forCommand('reply')) };
             })()",
        ) else {
            eprintln!("node is not installed; the feature rules were not run");
            return;
        };
        assert_eq!(value["moved"], true);
        assert_eq!(value["search"], true);
        assert_eq!(value["reply"], true);
        // The page takes the states on boot, on every refresh, and on every 4 s overview poll —
        // before the fingerprint check, so a switch shows although no Card changed.
        let watch = app_function("watchForChanges");
        let applied = watch.find("applyFeatures(overview);").expect("the poll applies the switches");
        let compared = watch.find("overview.fingerprint !== state.fingerprint").expect("fingerprint check");
        assert!(applied < compared, "{watch}");
        assert!(APP_JS.contains("setInterval(watchForChanges, 4000);"));
        assert!(app_function("refresh").contains("applyFeatures(overview);"));
        assert!(app_function("boot").contains("applyFeatures(overview);"));
        let apply = app_function("applyFeatures");
        assert!(apply.contains("if (!ZtFeatures.changed(state.features, next)) return;"), "{apply}");
        assert!(apply.contains("paintFeatures();"));
        assert!(apply.contains("renderReader(); paintDistant();"), "the Reply actions come and go with the switch");
    }

    #[test]
    fn the_front_end_removes_a_switched_off_feature_whole_and_asks_nothing() {
        // The module loads before the page and is served on its own route.
        let features = INDEX_HTML.find("<script src=\"/features.js\" defer></script>").expect("features.js");
        let app = INDEX_HTML.find("<script src=\"/app.js\" defer></script>").expect("app.js");
        assert!(features < app);
        assert!(GUI_RS.contains("(Method::Get, \"/features.js\") => {"));
        // Search: the rail's search field is removed with its row, and the tray (the only caller
        // of `/api/search`) opens nothing, whether `/`, Ctrl+K, the field or the command bar asked.
        let paint = app_function("paintFeatures");
        assert!(paint.contains("$('#search-open').closest('.rail-search').hidden = !on('search');"), "{paint}");
        assert!(paint.contains("$('#ask-open').hidden = !on('ask');"), "{paint}");
        assert_eq!(APP_JS.matches("api(`search?").count(), 1, "one search call, inside the tray");
        let open = &APP_JS[APP_JS.find("    function open(text) {").expect("finder.open")..];
        let guard = open.find("if (!ZtFeatures.allows(state.features, 'search')) return;").expect("search guard");
        assert!(guard < open.find("if (!built) build();").expect("build"), "the tray is not even built");
        assert!(APP_JS.contains("if (ZtFeatures.allows(state.features, 'search') && trimmed &&"), "no `search \"…\"` suggestion");
        assert!(app_function("applyFeatures").contains("if (!next.search && finder.isOpen) finder.close();"));
        // Keys: one gate before every key of the switch, Ctrl+K included.
        let keys = app_function("installKeys");
        let gate = keys
            .find("if (!ZtFeatures.allows(state.features, ZtFeatures.forKey(event.key, event.ctrlKey || event.metaKey))) return;")
            .expect("key gate");
        assert!(gate < keys.find("finder.open();").expect("Ctrl+K"), "{keys}");
        // Ask: the button is hidden, the command neither offered nor run, and no question is asked.
        let ask = app_function("askBox");
        let ask_guard = ask.find("if (!ZtFeatures.allows(state.features, 'ask')) throw new Error(ZtFeatures.offNote('ask'));").expect("ask guard");
        assert!(ask_guard < ask.find("api('ask'").expect("ask call"));
        let reasons = app_function("askReasons");
        assert!(
            reasons.find("if (!ZtFeatures.allows(state.features, 'ask')) return {};").expect("reasons guard")
                < reasons.find("api('ask'").expect("reasons call")
        );
        assert_eq!(APP_JS.matches("api('ask'").count(), 2, "no other question call");
        assert!(APP_JS.contains("const offered = (syntax, scope) => available(scope)"));
        assert!(APP_JS.contains("if (!offered(syntax, scope)) continue;"), "the menu offers only what is on");
        assert!(app_function("showHelp").contains("COMMANDS.filter(([syntax, , scope]) => offered(syntax, scope))"));
        let run = app_function("runCommand");
        assert!(
            run.find("if (!ZtFeatures.allows(state.features, feature)) throw new Error(ZtFeatures.offNote(feature));").expect("command gate")
                < run.find("case 'ask': return askBox(rest.join(' '));").expect("ask case")
        );
        // Reply: the reader's and the distant slot's Reply are not drawn, and no reply is saved.
        assert!(APP_JS.contains("card.workspace === 'agent' && ZtFeatures.allows(state.features, 'reply') ? actionButton('Reply', 'r', replyTo, {"));
        assert!(APP_JS.contains("agent && ZtFeatures.allows(state.features, 'reply') ? h('button', {\n          class: 'distant-reply'")
            || APP_JS.contains("agent && ZtFeatures.allows(state.features, 'reply') ? h('button', {\r\n          class: 'distant-reply'"));
        let reply = app_function("replyTo");
        let reply_guard = reply.find("if (!ZtFeatures.allows(state.features, 'reply')) throw new Error(ZtFeatures.offNote('reply'));").expect("reply guard");
        assert!(reply_guard < reply.find("api('reply'").expect("reply call"));
        assert_eq!(APP_JS.matches("api('reply'").count(), 1, "no other reply call");
    }

    /// The shipped page scripts parse: a broken statement in `app.js` or `layout.js` ships as a
    /// page that never boots, and no other test reads the whole file as a program. Node is the
    /// project's own checker (the wiki and the draft tests use it); without Node the check says so.
    #[test]
    fn the_page_scripts_parse_as_javascript() {
        let dir = tempfile::tempdir().expect("temp dir");
        for (name, source) in [
            ("app.js", APP_JS),
            ("layout.js", LAYOUT_JS),
            ("draft.js", DRAFT_JS),
            ("ask.js", ASK_JS),
            ("features.js", FEATURES_JS),
            ("lock.js", LOCK_JS),
        ] {
            let file = dir.path().join(name);
            fs::write(&file, source).expect("write the script");
            let output = match std::process::Command::new("node")
                .arg("--check")
                .arg(&file)
                .output()
            {
                Ok(output) => output,
                Err(_) => {
                    eprintln!("node is not installed; the page scripts were not parsed");
                    return;
                }
            };
            assert!(
                output.status.success(),
                "{name} does not parse: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    /// S04 (#146, S04-AC06, US014): an accepted write's save note names the stage and the advice
    /// count, puts the advice this save added before the advice the Card already carried, shows
    /// three and folds the rest; an answer that ran no check has no note.
    #[test]
    fn the_save_note_shows_new_advice_first_and_folds_the_rest() {
        let Some(out) = draft_js(
            "(() => {
               const check = { complete: true, rule_version: '0.4.1', policy_hash: 'abcdef0123456789ff', stage: 'advisory' };
               const warn = (rule, legacy) => ({ rule, fault: 'f', message: `${rule}  message\\n${'字'.repeat(200)}`, legacy });
               return {
                 none: draft.checkNote({ address: '1/1' }),
                 clean: draft.checkNote({ address: '1/1', warnings: [], check }),
                 mixed: draft.checkNote({ address: '1/1', warnings: [warn('R2', true), warn('R3', false), warn('R4', false), warn('R5', true), warn('R6', false)], check }),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(out["none"], Value::Null);
        assert_eq!(out["clean"]["summary"], json!("check · advisory · no advice"));
        assert_eq!(out["clean"]["shown"], json!([]));
        let mixed = &out["mixed"];
        assert_eq!(mixed["summary"], json!("check · advisory · 5 advice · 3 new, 2 already there"));
        assert_eq!(mixed["versions"], json!("zt-check 0.4.1 · policy abcdef012345"));
        let rules = |key: &str| -> Vec<String> {
            mixed[key].as_array().expect(key).iter().map(|line| line["rule"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(rules("shown"), ["R3", "R4", "R6"], "new advice first, three shown");
        assert_eq!(rules("folded"), ["R2", "R5"], "the advice already there folds under +N");
        let text = mixed["shown"][0]["text"].as_str().unwrap();
        assert!(text.starts_with("R3 message ") && text.ends_with('…'), "one clipped line: {text}");
        assert_eq!(text.chars().count(), 140);
    }

    /// S04 (#146, S04-AC06, US015): a refused write's slip lists the blocking lines first and folds
    /// the advice; each line finds its sentence in the draft (also after a measure prefix); a check
    /// that could not finish names its reason; any other error is not a Card check.
    #[test]
    fn a_card_check_refusal_lists_the_blocking_lines_and_finds_them_in_the_draft() {
        let Some(out) = draft_js(
            "(() => {
               const refused = draft.checkRefusal({
                 error: 'card_check', address: '1/1', blocking: 2,
                 faults: ['1/1 R3 丙；丁。'],
                 diagnostics: [
                   { rule: 'R9', fault: 'old', key: '旧的句子', message: '旧的句子', severity: 'advise', legacy: true },
                   { rule: 'R3', fault: 'semicolon', key: '丙；丁。', message: '丙；丁。', severity: 'block', legacy: false },
                   { rule: 'R2', fault: 'sentence_length', key: '(30 字 > 25) 这是一个很长的句子。', message: '30 字 > 25: 这是一个很长的句子。', severity: 'block', legacy: false },
                 ],
               });
               const text = '# 标题\\n\\n丙；丁。这是一个很长的句子。\\n';
               return {
                 refused,
                 found: refused.blocking.map((line) => draft.locateFault(text, line)),
                 missing: draft.locateFault('# 别的\\n', refused.blocking[0]),
                 lines: draft.checkRefusal({ error: 'card_check', blocking: 1, faults: ['1/1 R3 丙；丁。', '1/1 R4 (advisory) 被看见。'] }),
                 incomplete: draft.checkRefusal({ error: 'check_incomplete', reason: 'target_budget', message: 'check_incomplete: too many Link targets' }),
                 other: draft.checkRefusal({ error: 'card_locked' }),
                 absent: draft.checkRefusal(undefined),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        let refused = &out["refused"];
        assert_eq!(refused["incomplete"], json!(false));
        let rules = |value: &Value| -> Vec<String> {
            value.as_array().unwrap().iter().map(|line| line["rule"].as_str().unwrap().to_string()).collect()
        };
        assert_eq!(rules(&refused["blocking"]), ["R3", "R2"]);
        assert_eq!(rules(&refused["advice"]), ["R9"], "legacy advice is folded, never blocking");
        assert!(refused["headline"].as_str().unwrap().starts_with("2 blocking faults: nothing was saved and the draft is kept."));
        assert_eq!(out["found"], json!([{ "start": 6, "end": 10 }, { "start": 10, "end": 20 }]));
        assert_eq!(out["missing"], Value::Null);
        assert_eq!(rules(&out["lines"]["blocking"]), ["R3"], "the human lines stand in for missing diagnostics");
        assert_eq!(out["lines"]["advice"][0]["text"], json!("被看见。"));
        assert_eq!(out["incomplete"]["incomplete"], json!(true));
        assert_eq!(out["incomplete"]["detail"], json!("check_incomplete: too many Link targets"));
        assert_eq!(out["other"], Value::Null);
        assert_eq!(out["absent"], Value::Null);
    }

    /// S04 (#146, S04-AC06): every desk save hands its answer to the save note, and every refusal
    /// reaches the desk's correction slip: `api()` keeps the 400's `check`, a Reply's
    /// `saved:false, reason:card_check` raises it, the slip lies beside Save, and a check refusal
    /// offers no Lock panel and saves nothing again on its own.
    #[test]
    fn every_gui_write_shows_its_check_beside_the_save_result() {
        assert!(APP_JS.contains("if (data.check) error.check = data.check;"));
        assert!(APP_JS.contains("if (result.reason === 'card_check' && result.check) error.check = result.check;"));
        // create, Literature create, edit, insert: one `.then(keep)` each.
        assert_eq!(APP_JS.matches(".then(keep)").count(), 4);
        // the hub draft's edit and create, the metadata save, the Reply.
        assert_eq!(APP_JS.matches("return keep(saved);").count(), 2);
        assert_eq!(APP_JS.matches("return keep(result);").count(), 2);
        assert!(APP_JS.contains("const note = ZtDraft.checkNote(result || accepted);"));
        assert_eq!(APP_JS.matches("`, result);").count(), 3, "ty, lv and mf pass their answer");
        assert!(APP_JS.contains("const refused = ZtDraft.checkRefusal(error.check);"));
        assert!(APP_JS.contains("if (error.refusal && !refused) {"), "no Lock panel for a check refusal");
        assert!(APP_JS.contains("        deskCheck,\n        h('div', { class: 'dlg-foot' }, msg, cancelBtn, saveBtn),"));
        assert_eq!(APP_JS.matches("doSave()").count(), 1, "only Ctrl+S saves again; nothing retries");
        for class in [".desk-check {", ".desk-check-line.advice", ".toast-check-line.legacy"] {
            assert!(APP_CSS.contains(class), "{class}");
        }
    }

    /// S18 (#99, D241): the desk opens with an empty claim title, then the supplied relation prefix
    /// in the body, then the generated-section marker. The title is the first line, so the selector
    /// can find the prefix where the desk put it.
    #[test]
    fn the_reply_desk_starts_with_the_title_then_the_supplied_prefix() {
        let Some(out) = draft_js("{ fresh: draft.replyDraft('印证', '5/1|a') }") else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(
            out["fresh"],
            json!("# \n\n印证 [[5/1|a]]：\n\n<!-- zt:reverse-links -->\n")
        );
    }

    /// S18 (#99, AC02/D244): the relation selector replaces the supplied prefix where the desk put
    /// it and leaves every sentence the reader composed byte for byte.
    #[test]
    fn the_relation_selector_replaces_only_the_supplied_prefix() {
        let Some(out) = draft_js(
            "{\
               swapped: draft.replySwapPrefix(draft.replyDraft('印证', '5/1|a'), '印证 [[5/1|a]]：', '反驳 [[5/1|a]]：'),\
               composed: draft.replySwapPrefix('# 融资约束\\n\\n印证 [[5/1|a]]：外部融资越贵，剩余控制权越值钱。\\n', '印证 [[5/1|a]]：', '放宽 [[5/1|a]]：'),\
               twice: draft.replySwapPrefix(draft.replySwapPrefix(draft.replyDraft('印证', '5/1|a'), '印证 [[5/1|a]]：', '反驳 [[5/1|a]]：'), '反驳 [[5/1|a]]：', '对应 [[5/1|a]]：')\
             }",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(
            out["swapped"],
            json!("# \n\n反驳 [[5/1|a]]：\n\n<!-- zt:reverse-links -->\n"),
            "the supplied prefix is replaced, and the title stays first"
        );
        assert_eq!(
            out["composed"],
            json!("# 融资约束\n\n放宽 [[5/1|a]]：外部融资越贵，剩余控制权越值钱。\n"),
            "the reader's own sentence is untouched"
        );
        assert_eq!(
            out["twice"],
            json!("# \n\n对应 [[5/1|a]]：\n\n<!-- zt:reverse-links -->\n"),
            "a second change leaves one prefix, not two"
        );
    }

    /// S18 (#99, AC02): a prefix the reader removed comes back at the start of the body, after the
    /// title heading, never above it — the desk cannot put the title out of its first line.
    #[test]
    fn a_removed_prefix_comes_back_after_the_title() {
        let Some(out) = draft_js(
            "{\
               removed: draft.replySwapPrefix('# 融资约束\\n\\n外部融资越贵。\\n', '印证 [[5/1|a]]：', '对应 [[5/1|a]]：'),\
               tight: draft.replySwapPrefix('# 融资约束\\n外部融资越贵。', '印证 [[5/1|a]]：', '对应 [[5/1|a]]：'),\
               title_only: draft.replySwapPrefix('# 融资约束', '印证 [[5/1|a]]：', '对应 [[5/1|a]]：')\
             }",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(
            out["removed"],
            json!("# 融资约束\n\n对应 [[5/1|a]]：\n\n外部融资越贵。\n"),
            "the title stays first and the reader's sentence follows the prefix"
        );
        assert_eq!(
            out["tight"],
            json!("# 融资约束\n\n对应 [[5/1|a]]：\n\n外部融资越贵。"),
            "a body line without a blank line after the title still gets the prefix after it"
        );
        assert_eq!(
            out["title_only"],
            json!("# 融资约束\n\n对应 [[5/1|a]]：\n\n"),
            "a title-only draft gets the prefix in its empty body"
        );
    }

    /// S18 (#99, AC03/AC04/D243/D246): the desk starts on the human Topic the reader replied in
    /// last — even when a Topic titled `回应` exists — and offers the default `回应` only when there
    /// is no usable recent Topic. A preference whose Topic is gone is never followed by address.
    #[test]
    fn the_reply_destination_is_the_last_topic_then_the_default() {
        let Some(out) = draft_js(
            "{\
               recent: draft.replyDestination([{address:'3/0',title:'项目'},{address:'5/0',title:'回应'}], '3/0', '回应'),\
               fallback: draft.replyDestination([{address:'3/0',title:'项目'},{address:'5/0',title:'回应'}], null, '回应'),\
               none: draft.replyDestination([{address:'3/0',title:'项目'}], null, '回应'),\
               gone: draft.replyDestination([{address:'3/0',title:'项目'}], '9/0', '回应'),\
               empty: draft.replyDestination([], null, '回应'),\
               spaced: draft.replyDestination([{address:'5/0',title:' 回应 '}], null, '回应')\
             }",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        let address = |name: &str| out[name].get("address").cloned().unwrap_or(Value::Null);
        assert_eq!(address("recent"), json!("3/0"), "the Topic used last wins over 回应");
        assert_eq!(address("fallback"), json!("5/0"), "the default Topic when nothing was used");
        assert_eq!(address("none"), Value::Null, "no Topic yet: the desk offers to create 回应");
        assert_eq!(address("gone"), Value::Null, "a vanished preference is not followed");
        assert_eq!(address("empty"), Value::Null);
        assert_eq!(address("spaced"), json!("5/0"), "a padded 回应 title is the default Topic");
    }

    /// S18 (#99): the reader's Reply action, its `r` key, the four offered relation words, the
    /// destination selector, the browser-local preference and the refusal path all exist in the
    /// shipped page, and the page never posts a human write through any other route.
    #[test]
    fn the_front_end_replies_to_an_agent_card_from_the_human_workspace() {
        let reply = &APP_JS[APP_JS.find("const REPLY_KINDS = [").expect("REPLY_KINDS")..];
        let reply = &reply[..reply.find("];").expect("end of REPLY_KINDS")];
        for word in ["印证", "反驳", "放宽", "对应"] {
            assert!(reply.contains(word), "the reply selector offers {word}: {reply}");
        }
        assert!(
            APP_JS.contains("async function replyTo(card = state.card)"),
            "the page has one reply path, which names its Card: {APP_JS}"
        );
        assert!(
            APP_JS.contains("case 'r': return card && card.workspace === 'agent' ? run(replyTo) : undefined;"),
            "r opens Reply only for an agent Card, and only outside an input"
        );
        assert!(APP_JS.contains("actionButton('Reply', 'r', replyTo"), "the reader shows the Reply action");
        assert!(APP_JS.contains("api('reply'"), "Reply saves through the GUI-only route");
        assert!(APP_JS.contains("store.set('zt-reply-topic'"), "the destination is remembered after a save");
        assert!(APP_JS.contains("store.get('zt-reply-topic')"), "a stale destination is resolved, not followed");
        assert!(APP_JS.contains("new Topic ${reply.fallback}"), "an empty archive offers the default Topic");
        assert!(APP_JS.contains("target.fingerprint = result.target.authored_fingerprint;"), "a changed target refreshes the captured fingerprint");
        assert!(APP_JS.contains("'agent-card'"), "the command bar offers reply only for an agent Card");
        // The desk's draft rules live in the DOM-free module the page loads, so the title stays
        // first and the relation selector changes only the supplied prefix.
        assert!(INDEX_HTML.contains("<script src=\"/draft.js\" defer></script>"), "the page loads the draft module");
        for used in [
            "ZtDraft.replyDraft(",
            "ZtDraft.replySwapPrefix(",
            "ZtDraft.replyDestination(",
        ] {
            assert!(APP_JS.contains(used), "the reply desk uses {used}");
        }
        assert!(
            !APP_JS.contains("text.value.startsWith(before)"),
            "the supplied prefix is found inside the draft, not matched against the whole text"
        );
        // `r` acts only in reader navigation: the desk, the finder and an input keep their keys.
        let keys = &APP_JS[APP_JS.find("function installKeys()").expect("installKeys")..];
        let r = keys.find("case 'r':").expect("the r key");
        for guard in [
            "if (state.editing || finder.isOpen) return;",
            "if (study.isOpen()) { study.key(event); return; }",
            "if (isTyping(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;",
        ] {
            let at = keys
                .find(guard)
                .unwrap_or_else(|| panic!("the key handler keeps its guard: {guard}"));
            assert!(at < r, "`{guard}` is checked before the r key");
        }
        // Cancelling the desk performs no write: the one reply POST is inside the desk's save, and
        // a desk that resolves without an address stores nothing and opens nothing.
        let reply_to = &APP_JS[APP_JS
            .find("async function replyTo(card = state.card)")
            .expect("replyTo")..];
        let save = reply_to.find("save: async (text) =>").expect("the reply save");
        assert!(reply_to[save..].contains("api('reply'"), "the save posts the reply");
        assert!(!reply_to[..save].contains("api('reply'"), "opening or cancelling the desk posts nothing");
        assert!(reply_to.contains("if (!address) return;"), "a cancelled desk saves nothing");
        let saved = reply_to.find("if (result.saved)").expect("the saved branch");
        let remembered = reply_to.find("store.set('zt-reply-topic'").expect("the preference");
        assert!(saved < remembered, "the destination is remembered only after a successful save");
        // No MCP tool writes a human Card, and Reply adds no route of its own beyond /api/reply.
        assert_eq!(APP_JS.matches("api('reply'").count(), 1);
        assert!(!include_str!("mcp.rs").contains("\"reply\""), "Reply is not an MCP tool");
    }

    /// S11 (#153, US040): the reader shows ordinary Reverse links under `Referred by` and Term
    /// uses in a second row of their own, each from its own list, and hides the second row when
    /// there is none; the broken-links dialog names a broken Term use's word and its reason.
    #[test]
    fn the_front_end_shows_term_uses_apart_from_reverse_links_and_names_broken_term_facts() {
        let reader = &APP_JS[APP_JS.find("function renderReader()").expect("the reader")..];
        let reader = &reader[..reader.find("card.outbound.length ?").expect("the links-out row")];
        assert!(
            reader.contains("referredBy(card).length ? [h('div', { class: 'section-label', text: `Referred by · ${referredBy(card).length}` })")
                && reader.contains("referredBy(card).map((link) => chip(link))"),
            "Referred by is built from the ordinary Reverse links: {reader}"
        );
        assert!(
            reader.contains("termUses(card).length ? h('details', { class: 'term-uses' },")
                && reader.contains("h('summary', { class: 'section-label', text: `Term uses · ${termUses(card).length}` })")
                && reader.contains("termUses(card).map((link) => termUseChip(link))")
                && reader.contains(": null,"),
            "Term uses is its own folded row, absent when the list is empty: {reader}"
        );
        assert!(!reader.contains("card.inbound.map"), "the reader no longer lists the merged inbound set");
        assert!(APP_JS.contains(
            "function referredBy(card) { return Array.isArray(card.reverse_links) ? card.reverse_links : card.inbound; }"
        ));
        assert!(APP_JS.contains(
            "function termUses(card) { return Array.isArray(card.term_uses) ? card.term_uses : []; }"
        ));
        // A Term-use chip is the ordinary chip (it opens the source Card) with the authored word.
        let term_chip = &APP_JS[APP_JS.find("function termUseChip(link)").expect("the term chip")..];
        let term_chip = &term_chip[..term_chip.find("function actionButton").expect("end")];
        assert!(term_chip.contains("chip(link, 'term-use')"), "{term_chip}");
        assert!(term_chip.contains("class: 'term-word'") && term_chip.contains("text: link.label"), "{term_chip}");
        // The broken dialog keeps the ordinary text and adds the term fact only for a Term use.
        let broken = &APP_JS[APP_JS.find("async function showBroken()").expect("the dialog")..];
        let broken = &broken[..broken.find("// ------------------------------------------------------------ tree locks").expect("end")];
        assert!(broken.contains("h('span', { class: 't', text: `${item.title} → ${item.target}` })"));
        assert!(broken.contains("brokenTermFact(item) ? h('span', { class: 'term-fact', text: brokenTermFact(item) }) : null"));
        assert!(broken.contains("if (item.label == null) return '';"), "an ordinary Link has no term fact");
        assert!(
            broken.contains("item.reason === 'not_head' ? 'not a Term head' : 'missing'"),
            "both reasons are named: {broken}"
        );
        // Run the shipped fact rule itself under Node when it is installed.
        let start = broken.find("function brokenTermFact(item)").expect("the fact rule");
        let rule = &broken[start..];
        let rule = &rule[..rule.find("\n  }\n").expect("end of the rule") + 4];
        let script = format!(
            "{rule}\nconsole.log(JSON.stringify([\
             brokenTermFact({{label: null, reason: 'missing'}}),\
             brokenTermFact({{label: '需求', reason: 'missing'}}),\
             brokenTermFact({{label: '需求', reason: 'not_head'}})]));"
        );
        if let Ok(out) = std::process::Command::new("node").arg("-e").arg(&script).output() {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                r#"["","“需求” · missing","“需求” · not a Term head"]"#
            );
        }
        let css = &APP_CSS[APP_CSS.find(".chip .term-word {").expect("the term word style")..];
        let css = &css[..css.find('\n').unwrap()];
        assert!(css.contains("var(--ink-soft)") && !css.contains('#'), "theme ink only: {css}");
        assert!(APP_CSS.contains(".chip .term-fact { flex: none;"), "the fact is never cut off");
    }

    /// The CSS rule that starts with `selector`, up to its closing brace.
    fn css_rule<'a>(selector: &str) -> &'a str {
        let start = APP_CSS
            .find(&format!("\n{selector} {{"))
            .unwrap_or_else(|| panic!("no rule {selector}"));
        let rule = &APP_CSS[start + 1..];
        &rule[..=rule.find('}').expect("end of rule")]
    }

    /// The `app.js` function that starts with `head`, up to the next function at the same depth.
    fn js_function<'a>(head: &str) -> &'a str {
        let start = APP_JS.find(head).unwrap_or_else(|| panic!("no function {head}"));
        let body = &APP_JS[start..];
        &body[..body.find("\n  }\n").expect("end of function") + 4]
    }

    /// S13 (#156, US047/US048, S13-AC01..AC03): a quiet Term link is the authored word in the
    /// prose's own font, size and ink with a restrained dotted line; its meaning and address show
    /// on a slip on hover and on keyboard focus by CSS alone; Enter and Space follow it through
    /// the ordinary Card route; a cross-Workspace Term link keeps its meaning and adds the
    /// Workspace instead of losing its title; and a broken one is pencilled in the broken ink with
    /// its reason as visible text.
    #[test]
    fn the_front_end_writes_term_links_quietly_with_a_slip_on_hover_and_focus() {
        let term = css_rule(".zt-link.zt-term");
        for part in [
            "font: inherit",
            "color: inherit",
            "background: none",
            "box-shadow: none",
            "padding: 0",
            "text-decoration-style: dotted",
        ] {
            assert!(term.contains(part), "a Term link is prose, not a chip ({part}): {term}");
        }
        assert!(!term.contains('#'), "theme ink only: {term}");
        let shown = css_rule(".zt-term:hover .zt-term-tip, .zt-term:focus-visible .zt-term-tip");
        assert!(shown.contains("opacity: 1") && shown.contains("visibility: visible"), "{shown}");
        let tip = css_rule(".zt-term-tip");
        assert!(
            tip.contains("position: absolute") && tip.contains("visibility: hidden") && tip.contains("pointer-events: none"),
            "the slip lies hidden under the word until it is asked for: {tip}"
        );
        assert!(tip.contains("ease-in-out") && !tip.contains('#'), "{tip}");
        for ws in ["agent", "human"] {
            let cross = css_rule(&format!(".zt-link.zt-term.cross.{ws}"));
            assert!(
                cross.contains("background: none") && cross.contains(&format!("var(--{ws})")),
                "a cross-Workspace Term link keeps the Workspace colour in its line: {cross}"
            );
        }
        let broken = css_rule(".zt-link.zt-term.broken");
        assert!(broken.contains("var(--broken)") && broken.contains("dashed"), "{broken}");
        let note = css_rule(".zt-term-note");
        assert!(note.contains("display: inline-block") && note.contains("var(--broken)"), "{note}");

        // The page wires Term links in wireMarkdown, and a cross-Workspace Term link no longer
        // loses its meaning to the Workspace title.
        let wire = js_function("function wireMarkdown(root)");
        assert!(
            wire.contains("if (anchor.classList.contains('zt-term')) wireTermLink(anchor, cross);")
                && wire.contains("else if (cross) anchor.title = anchor.title ? `${wsLabel(cross)} workspace · ${anchor.title}` : `${wsLabel(cross)} workspace`;"),
            "{wire}"
        );
        assert!(
            wire.contains("anchor.addEventListener('click', (event) => { event.preventDefault(); go(anchor.dataset.target); });"),
            "the ordinary encoded Card route stays the click: {wire}"
        );
        let link = js_function("function wireTermLink(anchor, cross)");
        assert!(link.contains("class: 'zt-term-tip', id, 'aria-hidden': 'true'"), "{link}");
        assert!(link.contains("anchor.setAttribute('aria-describedby', id);"), "{link}");
        assert!(link.contains("anchor.removeAttribute('title');"), "{link}");
        assert!(
            link.contains("anchor.style.setProperty('--tip-shift'")
                && link.contains("anchor.style.setProperty('--tip-max'")
                && tip.contains("var(--tip-shift, 0px)")
                && tip.contains("var(--tip-max, 24em)"),
            "the slip is narrowed to the card and slid back onto it near the right edge: {link}"
        );
        assert!(
            link.contains("event.key !== ' '") && link.contains("go(anchor.dataset.target);"),
            "Space follows the link as Enter does: {link}"
        );
        assert!(
            !link.contains("innerHTML") && !link.contains("html:"),
            "the slip's words are text, never HTML: {link}"
        );
        // No new route, popover service or Term page.
        assert!(!APP_JS.contains("/api/term"), "the reader asks no new route");

        let rule = js_function("function termTipParts(meaning, target, workspaceLabel)");
        let script = format!(
            "{rule}\nconsole.log(JSON.stringify([\
             termTipParts('双方都不改变策略', '0/1|a', ''),\
             termTipParts(undefined, '0/1|a', 'Agent')]));"
        );
        if let Ok(out) = std::process::Command::new("node").arg("-e").arg(&script).output() {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                r#"[{"meaning":"双方都不改变策略","address":"0/1|a","workspace":""},{"meaning":"","address":"0/1|a","workspace":"Agent workspace"}]"#
            );
        }
    }

    /// S13 (#156, US049, S13-AC05/AC06): a Term Card is read in the ordinary reader: one quiet
    /// line under the title names its computed role and the list it is filed in (the list's name
    /// opens the list through the ordinary route), and its generated Term uses fold to their count
    /// under a normal `<details>` disclosure. No second page or board is added.
    #[test]
    fn the_front_end_names_term_facts_quietly_and_folds_term_uses_to_their_count() {
        let reader = js_function("function renderReader()");
        assert!(
            reader.contains("h('h1', { class: 'title' }, titleNodes(card.title || '(untitled)')),\n        termFactsLine(card),"),
            "the Term facts line sits under the title in the card head: {reader}"
        );
        let line = js_function("function termFactsLine(card)");
        assert!(line.contains("h('p', { class: 'term-facts' },") && line.contains("onclick: () => go(facts.list.address)"), "{line}");
        assert!(!line.contains("innerHTML") && !line.contains("html:"), "{line}");
        let css = css_rule(".card .term-facts");
        assert!(css.contains("var(--ink-faint)") && css.contains("12px") && !css.contains('#'), "quiet: {css}");
        let marker = css_rule("details.term-uses > summary::before");
        assert!(marker.contains("transition: transform"), "{marker}");
        assert!(APP_CSS.contains("details.term-uses[open] > summary::before { transform: rotate(90deg); }"));

        let rule = js_function("function termFactsText(facts)");
        let script = format!(
            "{rule}\nconst list = {{address: '0/1', title: '术语表：测试'}};\
             console.log(JSON.stringify([\
             termFactsText(null),\
             termFactsText({{role: 'list', list}}),\
             termFactsText({{role: 'card', list}}),\
             termFactsText({{role: 'part', list}}),\
             termFactsText({{role: 'unfiled', list: null}})]));"
        );
        if let Ok(out) = std::process::Command::new("node").arg("-e").arg(&script).output() {
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert_eq!(
                String::from_utf8_lossy(&out.stdout).trim(),
                concat!(
                    r#"[null,{"role":"Term list","list":null,"note":""},"#,
                    r#"{"role":"Term head","list":{"address":"0/1","title":"术语表：测试"},"note":"filed in"},"#,
                    r#"{"role":"Term part","list":{"address":"0/1","title":"术语表：测试"},"note":"filed in"},"#,
                    r#"{"role":"Unfiled Term","list":null,"note":"in no Term list"}]"#
                )
            );
        }
    }

    /// S14 (#161, US051/US052, S14-AC02/AC03): `[[` completion offers a Term use only for a Term
    /// head, found by its term name and told apart from a same-named head by its book and address;
    /// it inserts exactly `name@address` (or the reader's own word before `@`), never an address it
    /// made up. The ordinary reasoned Link to the same Term Card stays offered, first in its group.
    #[test]
    fn term_completion_offers_heads_by_name_with_their_book_and_keeps_the_ordinary_citation() {
        let Some(out) = draft_js(
            "(() => {
               const nodes = [
                 { address: 'Book1990', tree: 'Book1990', edge: 'root', title: 'A Book' },
                 { address: 'Other2000', tree: 'Other2000', edge: 'root', title: 'Another Book' },
                 { address: 'Book1990/1', tree: 'Book1990', type: 'term', term: null, term_role: 'list', title: '术语表' },
                 { address: 'Book1990/1|b', tree: 'Book1990', type: 'term', term: '一般均衡', term_role: 'card', title: '一般均衡' },
                 { address: 'Book1990/1|a', tree: 'Book1990', type: 'term', term: '均衡', term_role: 'card', title: '定义' },
                 { address: 'Book1990/1|a1', tree: 'Book1990', type: 'term', term: '均衡', term_role: 'part', title: '均衡（续）' },
                 { address: 'Other2000/3|a', tree: 'Other2000', type: 'term', term: '均衡', term_role: 'card', title: '均衡' },
                 { address: 'Other2000/4', tree: 'Other2000', type: 'term', term: '均衡', term_role: 'unfiled', title: '均衡？' },
                 { address: 'Book1990/2', tree: 'Book1990', type: 'idea', title: '均衡价格的形成' },
               ];
               const rows = (typed) => draft.linkCompletions(nodes, typed, 8).map((row) => [row.kind, row.insert, row.book.tree, row.book.title]);
               return {
                 name: rows('均衡'),
                 word: rows('均衡价格@Other'),
                 bare: rows('均衡@'),
                 address: rows('Book1990/2'),
                 none: rows(''),
                 typed: [draft.linkTyped('见 [[均衡'), draft.linkTyped('[[general eq'), draft.linkTyped('[[a]] b'), draft.linkTyped('no link')],
               };
             })()",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(
            out["name"],
            json!([
                ["term", "均衡@Book1990/1|a", "Book1990", "A Book"],
                ["term", "均衡@Other2000/3|a", "Other2000", "Another Book"],
                ["term", "一般均衡@Book1990/1|b", "Book1990", "A Book"],
                ["link", "Book1990/1|a", "Book1990", "A Book"],
                ["link", "Other2000/3|a", "Other2000", "Another Book"],
                ["link", "Book1990/1|b", "Book1990", "A Book"],
                ["link", "Book1990/1|a1", "Book1990", "A Book"],
                ["link", "Other2000/4", "Other2000", "Another Book"],
                ["link", "Book1990/2", "Book1990", "A Book"],
            ]),
            "heads only, exact name first, each with its book; every offered head is also citable, first"
        );
        assert_eq!(
            out["word"],
            json!([["term", "均衡价格@Other2000/3|a", "Other2000", "Another Book"]]),
            "the reader's own word stays the label; after `@` the book narrows; no citation rows"
        );
        assert_eq!(
            out["bare"],
            json!([
                ["term", "均衡@Book1990/1|a", "Book1990", "A Book"],
                ["term", "均衡@Other2000/3|a", "Other2000", "Another Book"],
                ["term", "均衡@Book1990/1|b", "Book1990", "A Book"],
            ])
        );
        assert_eq!(out["address"], json!([["link", "Book1990/2", "Book1990", "A Book"]]), "an address is an ordinary Link only");
        let none = out["none"].as_array().unwrap();
        assert!(!none.is_empty() && none.iter().all(|row| row[0] == "link"), "no Term use before a word: {none:?}");
        assert_eq!(out["typed"], json!(["均衡", "general eq", null, null]));
    }

    /// S29 (#171, the GUI check's defect B): `[[` completion ranks both groups on one scale, so the
    /// first row, the one Enter takes, is the closest match: an exact address or Citation key, an
    /// exact term name, an address prefix, a term-name prefix, then a title match. A human Card
    /// whose title only mentions `TermBook2026` no longer comes before the book's own Card, and the
    /// inserted forms stay `label@address` for a Term use and `address` for an ordinary Card.
    #[test]
    fn link_completion_ranks_an_exact_or_prefixed_address_before_a_title_match() {
        let Some(out) = draft_js(
            "(() => {
               const nodes = [
                 { address: '1', tree: '1', edge: 'root', title: '读记' },
                 { address: '1/1', tree: '1', type: 'idea', title: '读 TermBook2026 的第一章' },
                 { address: 'TermBook2026', tree: 'TermBook2026', edge: 'root', title: 'Term Book 2026' },
                 { address: 'TermBook2026/1', tree: 'TermBook2026', type: 'idea', title: '第一章：均衡' },
                 { address: 'TermBook2026/1|a', tree: 'TermBook2026', type: 'term', term: null, term_role: 'list', title: '术语表' },
                 { address: 'TermBook2026/1|a|a', tree: 'TermBook2026', type: 'term', term: '均衡', term_role: 'card', title: '均衡' },
                 { address: 'TermBook2026/1|a|b', tree: 'TermBook2026', type: 'term', term: '1/1', term_role: 'card', title: '一个怪名字' },
                 { address: '0/2', tree: '0', type: 'idea', title: '均衡是重复博弈的稳定结果' },
               ];
               const rows = (typed) => draft.linkCompletions(nodes, typed, 8).map((row) => [row.kind, row.insert]);
               return {
                 prefix: rows('TermB'),
                 key: rows('termbook2026').slice(0, 1),
                 address: rows('1/1'),
                 term: rows('均').slice(0, 3),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(
            out["prefix"],
            json!([
                ["link", "TermBook2026"],
                ["link", "TermBook2026/1"],
                ["link", "TermBook2026/1|a"],
                ["link", "TermBook2026/1|a|a"],
                ["link", "TermBook2026/1|a|b"],
                ["link", "1/1"],
            ]),
            "the Citation key prefix first, in address order; the title match last"
        );
        assert_eq!(out["key"], json!([["link", "TermBook2026"]]), "an exact Citation key, any case, comes first");
        assert_eq!(
            out["address"],
            json!([["link", "1/1"], ["link", "TermBook2026/1|a|b"], ["term", "1/1@TermBook2026/1|a|b"]]),
            "an exact address beats a Term head of the same name, and its group is listed first"
        );
        assert_eq!(
            out["term"],
            json!([["term", "均衡@TermBook2026/1|a|a"], ["link", "TermBook2026/1|a|a"], ["link", "TermBook2026/1"]]),
            "a term-name prefix keeps the Term use first, its head cited next"
        );
    }

    /// S29 (#171, the GUI check's defect C): the `[[` menu stands beside the caret's line and never
    /// over it: below when it fits, above when the room below is short, shrunk to the larger side
    /// otherwise, inside the window; and the chosen row is scrolled into view.
    #[test]
    fn the_completion_menu_never_covers_the_caret_line_and_keeps_its_row_in_view() {
        let Some(out) = draft_js(
            "(() => {
               const view = { viewWidth: 1000, viewHeight: 800, menuWidth: 380 };
               const place = (line, menuHeight, left = 100) => draft.menuPlace({ ...view, lineTop: line, lineBottom: line + 20, left, menuHeight });
               return {
                 below: place(100, 300),
                 above: place(700, 300),
                 squeezed: place(390, 600),
                 right: place(100, 100, 900),
                 scroll: [
                   draft.rowScroll({ rowTop: 400, rowHeight: 30, scrollTop: 0, viewHeight: 200 }),
                   draft.rowScroll({ rowTop: 20, rowHeight: 30, scrollTop: 100, viewHeight: 200 }),
                   draft.rowScroll({ rowTop: 120, rowHeight: 30, scrollTop: 100, viewHeight: 200 }),
                 ],
               };
             })()",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(out["below"]["above"], json!(false));
        assert_eq!(out["below"]["top"], json!(124), "4 px under the line that ends at 120");
        assert_eq!(out["above"]["above"], json!(true));
        assert_eq!(out["above"]["top"], json!(396), "300 tall, ending 4 px above the line at 700");
        assert_eq!(out["squeezed"]["above"], json!(false), "the larger side (below) when it fits neither");
        assert_eq!(out["squeezed"]["top"], json!(414));
        assert_eq!(out["squeezed"]["maxHeight"], json!(378), "it shrinks to the room below and scrolls");
        assert_eq!(out["right"]["left"], json!(612), "kept 8 px inside the window's right edge");
        assert_eq!(out["scroll"], json!([230, 20, 100]));

        let desk = js_function("function openEditor(");
        assert!(desk.contains("const line = caretLine(text, text.selectionStart);"), "{desk}");
        assert!(desk.contains("const place = ZtDraft.menuPlace({"), "{desk}");
        assert!(desk.contains("completion.scrollTop = ZtDraft.rowScroll({"), "{desk}");
        assert!(!desk.contains("scrollIntoView"), "no ancestor scroll: {desk}");
        assert!(desk.contains("if (layer && completion.parentElement !== layer) layer.append(completion);"), "off the desk's transform: {desk}");
        assert!(desk.contains("options[0].kind === 'link' ? [...linkGroup, ...termGroup] : [...termGroup, ...linkGroup]"), "the best row's group first");
        let caret = js_function("function caretLine(area, index)");
        assert!(caret.contains("whiteSpace: 'pre-wrap'") && caret.contains("mirror.remove();"), "{caret}");
    }

    /// S29 (#171, the GUI check's defect D): a list sheet (Broken links among them) is a named modal
    /// dialog: focus moves in and Tab stays inside, Escape and a route change close it, and focus
    /// returns to what opened it.
    #[test]
    fn a_list_dialog_is_a_named_modal_that_closes_on_escape_and_route_and_returns_focus() {
        let dialog = js_function("function listDialog(title, rows)");
        assert!(dialog.contains("role: 'dialog', 'aria-modal': 'true', 'aria-labelledby': id"), "{dialog}");
        assert!(dialog.contains("h('h3', { id, text: title })"), "the heading names it: {dialog}");
        assert!(dialog.contains("if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); return; }"));
        assert!(dialog.contains("window.addEventListener('hashchange', close);") && dialog.contains("window.removeEventListener('hashchange', close);"));
        assert!(dialog.contains("const opener = document.activeElement;") && dialog.contains("opener.focus();"), "{dialog}");
        assert!(dialog.contains("last.focus();") && dialog.contains("first.focus();"), "Tab wraps inside: {dialog}");
        assert!(APP_JS.contains("    listDialog(`Broken links · ${broken.length}`, (close) =>"), "the broken list uses it");
    }

    /// S29 (#171, the GUI check's defect G): the folded `Term uses` summary is a control, so its
    /// quiet label keeps at least 4.5:1 against the desk in the light and both dark theme blocks.
    #[test]
    fn the_term_uses_summary_keeps_text_contrast_in_both_themes() {
        let rule = css_rule("details.term-uses > summary");
        assert!(rule.contains("color: color-mix(in srgb, var(--ink-soft) 75%, var(--ink-faint));"), "{rule}");
        assert!(css_rule("details.term-uses > summary:hover, details.term-uses > summary:focus-visible").contains("var(--ink)"));
        let token = |block: &str, name: &str| -> [f64; 3] {
            let start = APP_CSS.find(block).unwrap_or_else(|| panic!("no block {block}"));
            let body = &APP_CSS[start..];
            let body = &body[..body.find('}').unwrap()];
            let at = body.find(&format!("  --{name}: #")).unwrap_or_else(|| panic!("no --{name} in {block}"));
            let hex = &body[at + name.len() + 7..at + name.len() + 13];
            [0, 2, 4].map(|i| f64::from(u8::from_str_radix(&hex[i..i + 2], 16).unwrap()))
        };
        let luminance = |rgb: [f64; 3]| {
            let c = rgb.map(|v| {
                let v = v / 255.0;
                if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
            });
            0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
        };
        for block in [":root {", ":root[data-theme=\"dark\"] {", ":root:not([data-theme=\"light\"]) {"] {
            let (soft, faint) = (token(block, "ink-soft"), token(block, "ink-faint"));
            let ink = [0, 1, 2].map(|i| soft[i] * 0.75 + faint[i] * 0.25);
            for ground in ["desk", "panel", "stock"] {
                let (a, b) = (luminance(ink), luminance(token(block, ground)));
                let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                assert!(ratio >= 4.5, "{block} --{ground}: {ratio:.2}");
            }
        }
    }

    /// S14 (#161, US050, S14-AC01): the term-name field follows the server's own name rule, and a
    /// save sends a `term` only for a Term Card — a create none when the name is empty (a Term list),
    /// an edit always, so an emptied field clears the name; no other type ever sends one.
    #[test]
    fn the_desk_sends_a_term_name_only_for_a_term_card_and_checks_it_like_the_server() {
        let Some(out) = draft_js(
            "(() => ({
               ok: [draft.termNameIssue(''), draft.termNameIssue('  均衡  '), draft.termNameIssue('字'.repeat(40))],
               refused: [draft.termNameIssue('a\\nb'), draft.termNameIssue('字'.repeat(41)), draft.termNameIssue('a@b'), draft.termNameIssue('[x]'), draft.termNameIssue('a|b'), draft.termNameIssue('a`b')],
               create: [draft.termField('term', ' 均衡 ', 'create'), draft.termField('term', '  ', 'create') === undefined],
               edit: [draft.termField('term', ' 均衡 ', 'edit'), draft.termField('term', '', 'edit')],
               other: ['idea', 'event', 'finding', 'model', 'hub'].map((type) => draft.termField(type, '均衡', 'edit') === undefined),
               max: draft.TERM_NAME_MAX,
             }))()",
        ) else {
            eprintln!("node is not installed; the draft module was not run");
            return;
        };
        assert_eq!(out["ok"], json!([null, null, null]));
        let refused = out["refused"].as_array().unwrap();
        assert!(refused.iter().all(Value::is_string), "{refused:?}");
        assert_eq!(out["create"], json!(["均衡", true]));
        assert_eq!(out["edit"], json!(["均衡", ""]));
        assert_eq!(out["other"], json!([true, true, true, true, true]));
        assert_eq!(out["max"], json!(crate::term::MAX_NAME_CHARS));
    }

    /// S14 (#161, S14-AC01/AC02/AC04): the page knows the sixth Card type everywhere it lists types
    /// (the desk's type picker, the search chips, the stack tallies, `ty`), shows the term-name
    /// field only while Term is chosen, sends the name through every desk save, and completes `[[`
    /// from the shared rules; the Term ink is a theme variable in all three theme blocks.
    #[test]
    fn the_front_end_authors_term_cards_in_the_existing_desk() {
        assert!(APP_JS.contains("    term: { label: 'Term', note: "), "CARD_TYPES names the sixth type");
        let desk = js_function("function openEditor(");
        assert!(desk.contains("class: 'term-name', 'aria-label': 'Term name', maxlength: ZtDraft.TERM_NAME_MAX"), "{desk}");
        assert!(desk.contains("const termPick = cardType ? h('span', { class: 'term-pick', hidden: cardType !== 'term' },"));
        assert!(desk.contains("facetPick.hidden = id !== 'model';\n            termPick.hidden = id !== 'term';"), "one type's field at a time");
        assert!(desk.contains("chosenType === 'term' ? chosenTerm.trim() : ''"), "only a Term Card's save carries a name");
        assert!(desk.contains("ZtDraft.termNameIssue(chosenTerm)"), "the name is checked before Save");
        assert!(desk.contains("options = ZtDraft.linkCompletions(state.nodes, typed, 8);"));
        assert!(desk.contains("text: 'Use the term'") && desk.contains("'Cite the card (a reasoned Link)'"), "two groups");
        assert!(desk.contains("text.setRangeText(option.insert + closing, at - typed, at, 'end');"), "the row's exact insert");
        assert!(!desk.contains(r"/\[\[([^\]\s]*)$/"), "one `[[` rule, the shared one");
        // The desk's refusal surfaces are unchanged: the draft stays and the slip names the faults.
        assert!(desk.contains("paintDeskCheck(refused);") && desk.contains("const refused = ZtDraft.checkRefusal(error.check);"));

        let create = js_function("async function newCard(");
        assert!(create.contains("term: ZtDraft.termField(type, term, 'create')"), "{create}");
        let edit = js_function("async function editCard()");
        assert!(edit.contains("term: card.term,") && edit.contains("term: ZtDraft.termField(type, term, 'edit'),") && edit.contains("base: card.text"), "{edit}");
        let insert = js_function("async function insertCard()");
        assert!(insert.contains("term: ZtDraft.termField(type, term, 'create'),"), "{insert}");
        let retype = js_function("async function setCardType(");
        assert!(retype.contains("term: ZtDraft.termField(type, name, 'edit')") && retype.contains("ZtDraft.termNameIssue(name)"), "{retype}");
        assert!(APP_JS.contains("'ty <idea|event|finding|model|hub|term> [<facet|name>]'"));

        assert!(APP_JS.contains("tally('term', tree.terms),"), "the stack tallies its Term Cards");
        assert!(
            APP_JS.contains("...Object.keys(CARD_TYPES).map((type) => chip('type', type, CARD_TYPES[type].label, types[type] || 0))"),
            "the search chips list every type, Term included"
        );

        assert_eq!(APP_CSS.matches("  --term: #").count(), 3, "light, dark and system-dark Term ink");
        assert!(css_rule(".type-stamp.term").contains("var(--term)"));
        assert!(css_rule(".type-pick button.on.term").contains("var(--term)"));
        assert!(css_rule(".term-pick[hidden]").contains("display: none"));
        let field = css_rule(".term-pick .term-name");
        assert!(field.contains("var(--serif)") && field.contains("border-bottom: 1px solid var(--line)") && !field.contains('#'), "{field}");
        assert!(css_rule(".command-menu button.cmp-term .cmp-word").contains("var(--serif)"));
        assert!(css_rule(".command-menu button.cmp-term .cmp-tree").contains("var(--ink-faint)"));
    }

    /// S19 (#108, AC04, AC05, AC06): the reader and the Table carry one low-salience slot, it asks
    /// the one read-only route once per read, it never posts, and its actions are the ordinary
    /// open, lay, and Reply.
    #[test]
    fn the_front_end_offers_one_distant_card_beside_the_neighbours() {
        let section = &APP_JS[APP_JS
            .find("const distant = { key: null, value: null, pending: null, poll: null };")
            .expect("the distant cache")..];
        let section = &section[..section
            .find("// ----------------------------------------------------------------- reader")
            .expect("end of the distant section")];
        assert_eq!(
            section.matches("api('distant?").count(),
            1,
            "the slot has one read: {section}"
        );
        assert!(
            section.contains("if (distant.key === key || distant.pending === key) return;"),
            "a repaint or hover reuses the read instead of asking again"
        );
        assert!(
            !section.contains("Math.random") && !section.contains("'telemetry'"),
            "the slot never rotates or records a view: {section}"
        );
        assert!(
            section.contains("distantKey(at)"),
            "the answer is keyed by the Card, the read, and the Table"
        );
        for where_ in ["distantSlot('reader')", "distantSlot('table')"] {
            assert!(APP_JS.contains(where_), "the page has no slot: {where_}");
        }
        // The Table's box appears for a distant Card alone, and only a repaint of the slips comes
        // with a late answer, so the Card held keeps its place and motion.
        assert!(APP_JS.contains("const away = focus ? !!distantValue(focus) : false;"));
        assert!(APP_JS.contains("if (!list.length && !away)"));
        assert!(APP_JS.contains("refreshReach() { if (state.mode === 'table') renderReach(); }"));
        // Opening, laying, and replying are the ordinary actions; Reply only for an agent Card.
        assert!(section.contains("onclick: () => go(candidate.address)"));
        assert!(section.contains("toggleTable(candidate.address)"));
        assert!(section.contains("const agent = candidate.workspace === 'agent';"));
        assert!(section.contains("onclick: () => replyTo(candidate).catch(fail)"));
        assert!(
            !section.contains("api('edit'") && !section.contains("api('create'") && !section.contains("api('reply'"),
            "the slot itself writes nothing: {section}"
        );
        // The slot is faint until the reader points at it or tabs into it, and it lifts nothing.
        let css = &APP_CSS[APP_CSS.find(".distant {").expect("the distant style")..];
        let css = &css[..css.find(".distant-open {").expect("the slot's title button")];
        assert!(css.contains("opacity: .58"), "the slot is faint at rest: {css}");
        assert!(css.contains(".distant:hover, .distant:focus-within"), "focus and hover ink it");
        assert!(!css.contains("box-shadow"), "the slot takes no lift from the held Card: {css}");
        assert!(!css.contains("animation"), "the slot never animates: {css}");
        // The suggestion is a GUI read, not an MCP tool.
        assert!(
            !include_str!("mcp.rs").contains("\"distant\""),
            "a distant suggestion is no MCP tool"
        );
    }

    /// S20 (#112, AC01, AC03, AC04, AC05, AC06, AC07): one reader action selects one existing
    /// Bridge report for this browser session, the route is asked again with that exact path as
    /// one argument, a pending helper answer is polled instead of blocking a repaint, and every
    /// failure is one small status line beside the slot. Nothing in the page applies a proposal.
    #[test]
    fn the_front_end_uses_one_selected_bridge_report() {
        let start = APP_JS
            .find("// ------------------------------------------------------- bridge report (S20, #112)")
            .expect("the bridge report section");
        let section = &APP_JS[start..];
        let section = &section[..section
            .find("// ----------------------------------------------------------------- reader")
            .expect("end of the bridge report section")];
        // The reader action names the exact file and keeps the choice in the browser session.
        assert!(APP_JS.contains("actionButton('Bridge report', null, useBridgeReport"));
        assert!(section.contains("const BRIDGE_SELECTION = 'zt-bridge-report';"));
        assert!(section.contains("sessionStorage.getItem(BRIDGE_SELECTION)"));
        assert!(section.contains("sessionStorage.setItem(BRIDGE_SELECTION, path)"));
        assert!(section.contains("sessionStorage.removeItem(BRIDGE_SELECTION)"));
        assert!(
            section.contains("await promptValue('Use a Bridge report'"),
            "the reader chooses one existing report path"
        );
        // The path travels as one encoded query argument; nothing scans a folder for a report.
        assert!(APP_JS.contains("(report ? '&report=' + encodeURIComponent(report) : '')"));
        assert!(!APP_JS.contains("readdir") && !APP_JS.contains("latest report"));
        // The answer is keyed by the selection, so changing it reads again; a repaint does not.
        assert!(APP_JS.contains("return [at, state.fingerprint || '', lying, bridgeReport()].join('|');"));
        assert!(section.contains("if (!report || report.status !== 'pending') return;"));
        assert!(
            APP_JS.contains("if (value && value.report && value.report.status === 'pending') scheduleBridgePoll(card.address);"),
            "a helper answer that is still pending is asked for again, never awaited in a paint"
        );
        // Every helper answer the service can report has a line, and the line is not a tray.
        for phrase in [
            "Bridge report: reading the selected report",
            "current proposal",
            "its endpoints changed, so no proposal is current",
            "no proposal names this Card",
            "Bridge report not found at the selected path",
            "Bridge report helper is not installed",
            "Bridge report helper took longer than ten seconds",
            "Bridge report is not a valid immutable report",
        ] {
            assert!(section.contains(phrase), "the status line never says `{phrase}`");
        }
        assert!(APP_JS.contains("card.context === 'regular' ? bridgeStatusSlot() : null"));
        assert!(section.contains("class: 'bridge-status', 'data-bridge-status': 'reader'"));
        // Reading or selecting a report never applies a Bridge: no write route and no proposal
        // acceptance lives in the page's bridge section.
        assert!(
            !section.contains("api('edit'") && !section.contains("api('create'") && !section.contains("api('apply'"),
            "the bridge section writes nothing: {section}"
        );
        assert!(APP_JS.contains("no proposal is applied"));
        let css = &APP_CSS[APP_CSS.find(".bridge-status {").expect("the status style")..];
        let css = &css[..css.find(".actions {").expect("the actions style")];
        assert!(css.contains(".bridge-status[hidden] { display: none; }"));
        assert!(css.contains(".bridge-status .bridge-clear:focus-visible"));
        assert!(!css.contains("animation") && !css.contains("box-shadow"), "the line is not a tray: {css}");
    }

    /// The search tray draws a folded row: the statement it stands for, its `+N`, and every member
    /// with what it is and whether it matched (S14-AC03, S14-AC05). The tray counts rows and Cards
    /// apart, a fold is a display choice of the open tray, and expanding it writes nothing.
    #[test]
    fn the_search_tray_renders_a_folded_row_and_every_member() {
        let slip = &APP_JS[APP_JS.find("function slip(hit, words) {").expect("slip")..];
        let slip = &slip[..slip.find("function guide(").expect("the end of slip")];
        assert!(
            slip.contains("hit.representative && hit.representative !== hit.address"),
            "a matching member names its canonical Card: {slip}"
        );
        assert!(
            slip.contains("dataset: { address: hit.representative }"),
            "the canonical reference is a slip the keyboard can open: {slip}"
        );
        assert!(
            slip.contains("choose(hit.representative)"),
            "the canonical reference opens that Card"
        );
        assert!(
            slip.contains("foldMarker(hit, words)"),
            "a row with members draws its fold marker"
        );
        let marker = &APP_JS[APP_JS.find("function foldMarker(hit, words) {").expect("foldMarker")..];
        let marker = &marker[..marker.find("function memberLine(").expect("the end of foldMarker")];
        for wanted in [
            "hit.members || []",
            "foldedOpen.has(hit.address)",
            "`+${behind} more stating the same statement`",
            "toggleMembers(hit.address)",
            "members.map((member) => memberLine(member, hit, words))",
        ] {
            assert!(marker.contains(wanted), "the fold marker has no `{wanted}`: {marker}");
        }
        let member = &APP_JS[APP_JS.find("function memberLine(member, hit, words) {").expect("memberLine")..];
        let member = &member[..member.find("function toggleMembers(").expect("the end of memberLine")];
        for wanted in [
            "dataset: { address: member.address }",
            "choose(member.address)",
            "member.canonical ? 'statement'",
            "member.matched",
            "filtered out",
        ] {
            assert!(member.contains(wanted), "a member line has no `{wanted}`: {member}");
        }
        // The toggle re-renders the tray from the answer already in hand: no second request, and no
        // Card is written by opening a fold.
        let toggle = &APP_JS[APP_JS.find("function toggleMembers(at) {").expect("toggleMembers")..];
        let toggle = &toggle[..toggle.find("function render() {").expect("the end of toggleMembers")];
        assert!(!toggle.contains("api("), "expanding a fold asks the server nothing: {toggle}");
        assert!(!toggle.contains("api('edit'") && !toggle.contains("api('create'"));
        // Rows and Cards are counted apart, and the weaker-row note uses the row total.
        assert!(APP_JS.contains("const rows = data.row_total == null ? data.total : data.row_total;"));
        assert!(APP_JS.contains("`best ${shown} of ${rows} rows · ${cards} cards`"));
        assert!(APP_JS.contains("`${rows} rows · ${cards} cards`"));
        assert!(APP_JS.contains("`${rows - shown} weaker rows are not shown; add a word to narrow the search.`"));
        // A member is reachable from the keyboard like a slip, and Enter opens it.
        assert!(APP_JS.contains("'.fslip, .ffold, .fslip-member, .fslip-canon'"));
        assert!(APP_JS.contains("else if (item.dataset.fold) toggleMembers(item.dataset.fold);"));
        // The fold's own styles exist, and the tray resets both kinds of fold together.
        for rule in [".fslip-canon {", ".fslip-fold-toggle {", ".fslip-members {", ".fslip-member {"] {
            assert!(APP_CSS.contains(rule), "app.css has no `{rule}`");
        }
        assert!(APP_JS.contains("const clearFolds = () => { unfolded = new Set(); foldedOpen.clear(); };"));
        assert_eq!(
            APP_JS.matches("clearFolds();").count(),
            4,
            "a new query, a new filter, a reopened tray and Esc all reset both kinds of fold"
        );
    }

    /// S16 (#158): the graph views' edge rules — which Links are Term edges, what the Map's lens
    /// shows, the Constellation's bundles — are the page's own DOM-free code (`src/gui/layout.js`),
    /// run here rather than described.
    fn layout_js(expression: &str) -> Option<Value> {
        page_module_js("layout.js", LAYOUT_JS, "L", expression)
    }

    /// #200/#201 (B2, B3; docs/model-board-blocks-proposal.md §3a): the block board's attention and
    /// ribbons. One focus block, at most four supporting blocks (previous, next, the two most linked),
    /// every other block at one receded level; one ribbon per block pair and kind, refers only behind
    /// the switch, lanes that never overlap where they share blocks, an upward rests-on ribbon marked
    /// as the forward fault, and ports per Card for the Links that leave its block.
    #[test]
    fn the_block_board_spends_attention_on_one_focus_and_bundles_links_between_blocks() {
        let Some(out) = layout_js(
            "(() => {
               const blocks = ['b0','b1','b2','b3','b4','b5','b6','b7'].map((id) => ({ id }));
               const e = (from_block, to_block, kind, count, pairs) => ({ from_block, to_block, kind, count, pairs: pairs || [] });
               const edges = [
                 e('b3', 'b1', 'rests-on', 3, [{ from: 'x3', to: 'x1' }, { from: 'x3b', to: 'x1' }]),
                 e('b3', 'b6', 'rests-on', 1, [{ from: 'y3', to: 'y6' }]),
                 e('b3', 'b0', 'rests-on', 9),
                 e('b3', 'b7', 'contrasts', 1),
                 e('b5', 'b3', 'refers', 4),
                 e('b2', 'b1', 'rests-on', 2),
               ];
               const off = L.blockLayout(blocks, edges, 'b3', { refers: false });
               const on = L.blockLayout(blocks, edges, 'b3', { refers: true });
               const head = L.blockLayout(blocks, edges, null, { refers: false });
               const only = L.blockLayout(blocks, edges, 'b3', { refers: false, focusOnly: true });
               const headOnly = L.blockLayout(blocks, edges, null, { refers: false, focusOnly: true });
               const lanes = off.ribbons.map((r) => [r.from_block, r.to_block, r.kind, r.lane, r.up, r.hot]);
               return {
                 levels: off.trays.map((t) => t.level),
                 headLevels: [...new Set(head.trays.map((t) => t.level))],
                 ribbonsOff: off.ribbons.length, ribbonsOn: on.ribbons.length,
                 ribbonsOnly: only.ribbons.length, ribbonsHeadOnly: headOnly.ribbons.length,
                 lanes,
                 portsX3: off.ports['x3'], portsX1: off.ports['x1'],
               };
             })()",
        ) else {
            return;
        };
        // b3 is the focus; b2 and b4 are its neighbours; b0 (9) and b1 (3) are its most linked.
        assert_eq!(
            out["levels"],
            json!(["support", "support", "support", "focus", "support", "far", "far", "far"]),
            "{out}"
        );
        assert_eq!(out["headLevels"], json!(["near"]), "with no focus every tray is at one level: {out}");
        assert_eq!(out["ribbonsOff"], json!(5), "refers hidden by default: {out}");
        assert_eq!(out["ribbonsOn"], json!(6), "the switch adds the refers ribbon: {out}");
        assert_eq!(out["ribbonsOnly"], json!(4), "an open block draws only its own ribbons: {out}");
        assert_eq!(out["ribbonsHeadOnly"], json!(5), "with no open block every ribbon is drawn: {out}");
        let lanes = out["lanes"].as_array().unwrap();
        let up = lanes.iter().find(|l| l[0] == "b3" && l[1] == "b6").unwrap();
        assert_eq!(up[4], json!(true), "resting on a later block points up: the forward fault: {out}");
        let down = lanes.iter().find(|l| l[0] == "b3" && l[1] == "b1").unwrap();
        assert_eq!(down[4], json!(false), "{out}");
        // Two ribbons that share blocks never share a lane.
        let span = |l: &Value| {
            let i = |s: &Value| s.as_str().unwrap()[1..].parse::<i64>().unwrap();
            (i(&l[0]).min(i(&l[1])), i(&l[0]).max(i(&l[1])), l[3].as_i64().unwrap())
        };
        for a in lanes {
            for b in lanes {
                if a == b {
                    continue;
                }
                let ((a0, a1, la), (b0, b1, lb)) = (span(a), span(b));
                if a0 < b1 && b0 < a1 {
                    assert_ne!(la, lb, "overlapping ribbons in one lane: {out}");
                }
            }
        }
        assert_eq!(out["portsX3"], json!([{ "dir": "out", "block": "b1", "count": 1 }]), "{out}");
        assert_eq!(out["portsX1"], json!([{ "dir": "in", "block": "b3", "count": 2 }]), "{out}");
    }

    /// #201 (B3; docs/model-board-blocks-proposal.md §5.2, §5.5, §5.6): with no open block the
    /// overview draws only a backbone (each block's heaviest rests-on ribbon to an earlier block,
    /// and every forward-fault ribbon), so the column stays readable; pointing at a block draws its
    /// own ribbons. A chapter block with no rests-on ribbon to an earlier block is unanchored. Each
    /// ribbon has its own slot where it meets a tray, so ribbons that end at one tray do not merge.
    /// Layout stays linear: a 500-part model lays out well under 50 ms.
    #[test]
    fn the_block_overview_draws_a_backbone_and_marks_unanchored_blocks() {
        let Some(out) = layout_js(
            "(() => {
               const blocks = [{ id: 'b0', kind: 'setting' }, ...['b1','b2','b3','b4','b5'].map((id) => ({ id, kind: 'chapter' })), { id: 'b6', kind: 'aside' }];
               const e = (from_block, to_block, kind, count) => ({ from_block, to_block, kind, count, pairs: [] });
               const edges = [
                 e('b1', 'b0', 'rests-on', 1),
                 e('b2', 'b1', 'rests-on', 3), e('b2', 'b0', 'rests-on', 5),
                 e('b3', 'b2', 'rests-on', 1), e('b3', 'b0', 'rests-on', 2),
                 e('b4', 'b5', 'rests-on', 1),
                 e('b5', 'b3', 'rests-on', 2), e('b5', 'b1', 'contrasts', 1), e('b5', 'b2', 'refers', 6),
               ];
               const head = L.blockLayout(blocks, edges, null, { focusOnly: true, backbone: true });
               const hover = L.blockLayout(blocks, edges, null, { focusOnly: true, backbone: true, hover: 'b2' });
               const all = L.blockLayout(blocks, edges, null, { focusOnly: true });
               const slots = head.ribbons.filter((r) => r.to_block === 'b0').map((r) => r.toSlot).sort();
               // A model of 500 parts: 40 blocks, 1,500 relations across them.
               const big = Array.from({ length: 40 }, (_, i) => ({ id: 'c' + i, kind: i ? 'chapter' : 'setting' }));
               const many = Array.from({ length: 1500 }, (_, i) => { const from = 1 + (i * 7) % 39; return e('c' + from, 'c' + ((i * 13) % from), i % 5 ? 'rests-on' : 'refers', 1 + (i % 3)); });
               const t0 = performance.now();
               for (let k = 0; k < 10; k += 1) L.blockLayout(big, many, null, { focusOnly: true, backbone: true });
               const ms = (performance.now() - t0) / 10;
               const bigHead = L.blockLayout(big, many, null, { focusOnly: true, backbone: true });
               return {
                 head: head.ribbons.map((r) => r.from_block + '>' + r.to_block).sort(),
                 hover: hover.ribbons.map((r) => r.from_block + '>' + r.to_block).sort(),
                 hoverLevels: [...new Set(hover.trays.map((t) => t.level))],
                 all: all.ribbons.length,
                 folded: head.folded,
                 unanchored: head.unanchored,
                 slots,
                 ms,
                 bigRibbons: bigHead.ribbons.length,
               };
             })()",
        ) else {
            return;
        };
        assert_eq!(out["head"], json!(["b1>b0", "b2>b0", "b3>b0", "b4>b5", "b5>b3"]), "the backbone and the forward fault: {out}");
        assert_eq!(out["hover"], json!(["b2>b0", "b2>b1", "b3>b2"]), "pointing at a block draws its own ribbons: {out}");
        assert_eq!(out["hoverLevels"], json!(["near"]), "pointing moves no tray: {out}");
        assert_eq!(out["all"], json!(8), "without the backbone every ribbon but refers: {out}");
        assert_eq!(out["folded"], json!(3), "the overview says how many ribbons it folds: {out}");
        assert_eq!(out["unanchored"], json!(["b4"]), "b4 rests only on a later block: {out}");
        assert_eq!(out["slots"], json!([0, 1, 2]), "three ribbons end at b0 in three slots: {out}");
        assert!(out["bigRibbons"].as_u64().unwrap() <= 40, "one backbone ribbon per block at most, plus faults: {out}");
        assert!(out["ms"].as_f64().unwrap() < 50.0, "layout is linear: {out}");
    }

    /// The model network (#200, #201): each block hangs under the earlier block it rests on most
    /// (ties go to the later block), so a spine model reads as the tree of what each chapter builds
    /// on; a block with no earlier premise is a root (the setting, or an unanchored chapter). The
    /// tree is laid out top to bottom: one layer per depth, siblings in reading order, a parent
    /// centred over its children, no two cards overlapping, the same model always the same way.
    #[test]
    fn the_model_network_hangs_each_block_under_its_main_premise_and_lays_the_tree_top_down() {
        let Some(out) = layout_js(
            "(() => {
               const blocks = ['b0','b1','b2','b3','b4','b5','b6'].map((id, i) => ({ id, kind: i ? 'chapter' : 'setting' }));
               const e = (from_block, to_block, kind, count) => ({ from_block, to_block, kind, count, pairs: [] });
               const edges = [
                 e('b1', 'b0', 'rests-on', 2),
                 e('b2', 'b1', 'rests-on', 1), e('b2', 'b0', 'rests-on', 1),
                 e('b3', 'b2', 'rests-on', 5), e('b4', 'b2', 'rests-on', 7), e('b4', 'b3', 'rests-on', 3),
                 e('b5', 'b4', 'refers', 9), e('b5', 'b6', 'rests-on', 2),
                 e('b6', 'b4', 'rests-on', 1),
               ];
               const tree = L.blockTree(blocks, edges);
               const size = (id) => (id === 'b4' ? { w: 160, h: 80 } : { w: 100, h: 50 });
               const lay = L.treeLayout(tree.roots, tree.children, size, { gapX: 10, gapY: 30 });
               const again = L.treeLayout(tree.roots, tree.children, size, { gapX: 10, gapY: 30 });
               const boxes = Object.values(lay.pos);
               let overlap = false;
               for (const a of boxes) for (const b of boxes) if (a !== b && a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h) overlap = true;
               return { parent: tree.parent, roots: tree.roots, children: tree.children, pos: lay.pos, width: lay.width, height: lay.height, overlap, same: JSON.stringify(lay) === JSON.stringify(again) };
             })()",
        ) else {
            return;
        };
        assert_eq!(
            out["parent"],
            json!({ "b0": null, "b1": "b0", "b2": "b1", "b3": "b2", "b4": "b2", "b5": null, "b6": "b4" }),
            "heaviest earlier premise, ties to the later block; refers and later blocks never parent: {out}"
        );
        assert_eq!(out["roots"], json!(["b0", "b5"]), "{out}");
        assert_eq!(out["children"]["b2"], json!(["b3", "b4"]), "siblings in reading order: {out}");
        let pos = &out["pos"];
        let cx = |id: &str| pos[id]["cx"].as_f64().unwrap();
        let y = |id: &str| pos[id]["y"].as_f64().unwrap();
        assert!(y("b0") < y("b1") && y("b1") < y("b2") && y("b2") < y("b3") && y("b3") < y("b6"), "top to bottom by depth: {out}");
        assert_eq!(pos["b3"]["depth"], pos["b4"]["depth"], "siblings share a layer: {out}");
        assert!(cx("b3") < cx("b4"), "reading order left to right: {out}");
        assert!((cx("b2") - (cx("b3") + cx("b4")) / 2.0).abs() < 0.5, "a parent centred over its children: {out}");
        assert!((cx("b4") - cx("b6")).abs() < 0.5, "an only child under its parent: {out}");
        assert!(cx("b5") > cx("b0"), "a second root stands to the right: {out}");
        assert_eq!(out["overlap"], json!(false), "no two cards overlap: {out}");
        assert_eq!(out["same"], json!(true), "deterministic: {out}");
        // A small subtree tucks in beside a wide one where their layers do not meet (contours).
        let Some(tuck) = layout_js(
            "(() => {
               const children = { r: ['a', 'b'], b: ['c'], c: ['d', 'e', 'f'] };
               const lay = L.treeLayout(['r'], children, () => ({ w: 100, h: 40 }), { gapX: 14, gapY: 20 });
               return { width: lay.width, a: lay.pos.a, b: lay.pos.b, d: lay.pos.d };
             })()",
        ) else {
            return;
        };
        assert_eq!(tuck["width"], json!(328), "three cards and two gaps, not the naive 442: {tuck}");
        assert!(tuck["a"]["x"].as_f64().unwrap() + 100.0 + 14.0 <= tuck["b"]["x"].as_f64().unwrap() + 0.01, "a beside b: {tuck}");
    }

    /// A transit line that skips layers of the board never runs behind a card: it drops into the gap
    /// below its premise, runs along the free corridor between the cards of the layers it passes
    /// (the corridor nearest its own line), and enters its dependent from the gap above it. A thread
    /// between neighbouring layers stays one smooth curve.
    #[test]
    fn a_thread_that_skips_layers_runs_in_a_free_corridor_not_behind_a_card() {
        let Some(out) = layout_js(
            "(() => {
               const between = [{ x: 60, y: 100, w: 80, h: 40 }, { x: 160, y: 100, w: 80, h: 40 }];
               const routed = L.threadPath({ x: 100, y: 40 }, { x: 100, y: 240 }, between, { half: 17 });
               const up = L.threadPath({ x: 100, y: 240 }, { x: 100, y: 40 }, between, { half: 17 });
               const near = L.threadPath({ x: 100, y: 40 }, { x: 180, y: 90 }, [], { half: 17 });
               return { routed, up, near };
             })()",
        ) else {
            return;
        };
        let lane = out["routed"]["lane"].as_f64().unwrap();
        assert!((lane - 150.0).abs() < 0.01, "the corridor between the two cards, nearest the line: {out}");
        let d = out["routed"]["d"].as_str().unwrap();
        assert!(d.starts_with("M 100 40 V "), "down from the premise first: {d}");
        assert!(d.contains(" 57 ") || d.contains(" 57\n") || d.contains(",57"), "the gap below the premise (17 down): {d}");
        assert!(d.ends_with(" 240"), "into the dependent: {d}");
        assert!(out["up"]["d"].as_str().unwrap().contains(" 223"), "an upward thread leaves by the gap above its premise: {out}");
        assert!(out["near"]["d"].as_str().unwrap().contains(" C "), "neighbouring layers: one smooth curve: {out}");
        assert!(out["near"]["lane"].is_null(), "{out}");
    }

    /// A transit line turns with a smooth bend, as on a metro map: each corner of its route is
    /// rounded by the radius, or less where a leg is short, and the straight legs stay straight.
    #[test]
    fn a_transit_line_turns_with_smooth_bends() {
        let Some(out) = layout_js(
            "({ bend: L.roundedPath([[0, 0], [0, 40], [60, 40], [60, 80]], 9), short: L.roundedPath([[0, 0], [0, 6], [30, 6]], 9), straight: L.roundedPath([[0, 0], [0, 50]], 9) })",
        ) else {
            return;
        };
        assert_eq!(out["bend"], json!("M 0 0 V 31 Q 0 40 9 40 H 51 Q 60 40 60 49 V 80"), "{out}");
        assert_eq!(out["short"], json!("M 0 0 V 3 Q 0 6 3 6 H 30"), "a short leg takes a tighter bend: {out}");
        assert_eq!(out["straight"], json!("M 0 0 V 50"), "{out}");
    }

    /// The board (#201, mental mapping): a pinned card leans a little about its pin, -1.2° to 1.2°,
    /// and the same card always leans the same way, so the board looks pinned by hand and stays put.
    #[test]
    fn a_pinned_card_leans_a_little_and_always_the_same_way() {
        let Some(out) = layout_js("({ tilts: ['K/4|a|a|5', 'K/4|a|a|5|b', 'K/4|a|a|5|c', 'K/4|a|a|7', 'K/4|a'].map(L.tilt), again: L.tilt('K/4|a|a|5') })") else {
            return;
        };
        let tilts: Vec<f64> = out["tilts"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        assert!(tilts.iter().all(|t| t.abs() <= 1.2), "a small lean: {out}");
        assert!(tilts.windows(2).any(|w| w[0] != w[1]), "cards lean differently: {out}");
        assert_eq!(out["again"].as_f64().unwrap(), tilts[0], "the same card always leans the same way");
    }

    /// #201 (B3; P3): a flag opens its two blocks side by side: one row per Link. The premises (the Cards
    /// of the block rested on) stand on the left, the Cards that rest on them on the right, each
    /// once and in Folgezettel order, and one thread per Card pair joins them.
    #[test]
    fn the_two_block_view_puts_premises_left_and_dependents_right_with_one_thread_per_pair() {
        let Some(out) = layout_js(
            "L.bridgeLayout({ from_block: 'b5', to_block: 'b3', kind: 'rests-on', count: 3, pairs: [
               { from: 'K/5|b|a', to: 'K/3|a' }, { from: 'K/5|b', to: 'K/3|a' }, { from: 'K/5|b', to: 'K/3|b|a' } ] })",
        ) else {
            return;
        };
        assert_eq!(
            out,
            json!({ "left": ["K/3|a", "K/3|b|a"], "right": ["K/5|b", "K/5|b|a"], "threads": [[0, 1], [0, 0], [1, 0]] }),
            "{out}"
        );
    }

    /// #200–#202 (B2–B4; docs/model-board-blocks-proposal.md §9): the page builds the model board
    /// in the owner's two forms. Between blocks, a transit map: each block's card hangs below its
    /// station, placed by the tree of main premises; its Links are uniform lines with smooth bends
    /// (the tree's and the faults always, the open block's own inked), a line that skips layers in a
    /// free corridor; pointing at a line lists its Card pairs and clicking it opens the two-block
    /// view. Inside a block, nesting and flags: its cards tucked under its chapter card in a cascade,
    /// the open Card pulled out with page flags on its edges. Cards slide from where they stood
    /// (300 ms, ease-in-out); colour only for the open block's station and the faults.
    #[test]
    fn the_front_end_shows_blocks_as_a_transit_map_and_a_block_inside_as_tucked_cards_with_flags() {
        let board = app_function("modelBoard");
        assert!(board.contains("if (model.blocks && model.blocks.length > 1 && (model.blocks[0].chaptered || model.parts.length > 20)) return blockBoard(card, model);"));
        let blocks = app_function("blockBoard");
        assert!(blocks.contains("const tree = ZtLayout.blockTree(blocks, edges);"), "each block hangs under its main premise");
        assert!(blocks.contains("ZtLayout.treeLayout(tree.roots, tree.children, sizeOf, { gapX: 34, gapY: 30 })"));
        // The transit map between blocks.
        assert!(blocks.contains("const layer = h('div', { class: 'mbn-layer' }, lines, [...nodes.values()], stations);"), "lines under the cards, stations over them");
        assert!(blocks.contains("let route = ZtLayout.threadPath(p1, p2, between, { half: 15 });"), "a line that skips layers takes a free corridor");
        assert!(blocks.contains("d = ZtLayout.roundedPath(route.points, 9);"), "smooth bends");
        assert!(blocks.contains("for (const e of faults) if (!list.has(key(e))) list.set(key(e), { ...e, up: true });"), "the forward faults are always drawn");
        assert!(blocks.contains("stations.append(svg('circle', { cx: q.cx, cy: q.y, r: q.h < 40 ? 4.5 : 6, class: 'station' + state }));"), "a station on each card's upper edge");
        assert!(blocks.contains("hit.addEventListener('click', () => openBridge(flag));"), "clicking a line opens the two-block view");
        assert!(blocks.contains("showPairs(flag, x.from_block, x.to_block,"), "pointing at a line lists its Card pairs");
        assert!(blocks.contains("if (drawnFor !== b.id) draw(b.id);"), "pointing at a card draws its own lines");
        assert!(!blocks.contains("flagStrip"), "the cards on the map carry no flags: their lines show their Links");
        // Inside a block: nesting and flags.
        assert!(blocks.contains("class: 'mbn-tuck' + (pulled ? ' pulled' : '')"));
        assert!(blocks.contains("pulled ? null : h('span', { class: 'mbn-tuck-line' }"), "a tucked card shows its top strip");
        assert!(blocks.contains("cardFlags([c.address, ...((c.proof && c.proof.addresses) || [])])"), "the pulled-out Card carries its page flags");
        assert!(blocks.contains("const side = p.dir === 'out' ? 'left' : 'right';"), "left: the blocks it rests on; right: the blocks that rest on it");
        assert!(blocks.contains("h('ol', { class: 'mbn-proof', 'aria-label': 'Proof chain' }"), "a proof cascades under its result");
        assert!(blocks.contains("open.lead ? h('p', { class: 'mbk-lead' }, titleNodes(open.lead)) : null"), "the open block opens with its intuition");
        assert!(blocks.contains("node.style.setProperty('--tilt', `${ZtLayout.tilt(b.head)}deg`);"), "a card leans a little");
        assert!(blocks.contains("{ duration: 300, easing: 'ease-in-out' }"), "a moved card slides from where it stood");
        assert!(blocks.contains("layer.style.transform = `translateX(${place.offset}px) scale(${place.scale})`;"), "a wide tree scales as a whole");
        for key in ["'ArrowDown'", "'ArrowUp'", "'ArrowLeft'", "'ArrowRight'", "'Enter'", "'Escape'"] {
            assert!(blocks.contains(key), "the strip answers {key}");
        }
        assert!(APP_JS.contains("localStorage.getItem('zt-refers-edges') === 'on'"), "the Refers switch has its own key");
        assert!(blocks.contains("class: 'mbk-switch lens-toggle', 'aria-pressed': String(refersOn)"));
        assert!(blocks.contains("Unanchored: nothing in ${nameOf(b.id)} rests on an earlier block."), "an unanchored block carries its mark and fix");
        assert!(blocks.contains("if (bridgeOpen && (bridgeOpen.head !== model.head.address || !bridgeOpen.cards.includes(here))) bridgeOpen = null;"));
        assert!(APP_JS.contains("return { open, close, hits: foundCards, get isOpen() { return isOpen; } };"));
        assert!(blocks.contains("(holdsHit(b) ? ' found' : '')"), "a search marks the blocks that hold a hit");
        // The two-block view: one row per Link, the two Cards side by side.
        let bridge = app_function("bridgeView");
        assert!(bridge.contains("threads.map(([li, ri]) => [cardEl(left[li]), cardEl(right[ri])])"), "each pair side by side in one row");
        assert!(bridge.contains("if (event.key === 'Escape' && document.body.contains(view))"), "Esc puts the blocks back");
        // The look.
        let cork = css_rule(".model-board.blocks, .mbk-bridge");
        assert!(cork.contains("var(--frame)") && cork.contains("var(--cork)") && cork.contains("var(--cork-grain)"), "a cork board in a frame: {cork}");
        assert!(css_rule(".mbn-lines .line").contains("var(--line-ink)"), "uniform muted lines");
        assert!(css_rule(".mbn-lines .tree .line").contains("stroke-width: 4"), "the main lines are bolder");
        assert!(css_rule(".mbn-lines .up .line").contains("var(--broken)"), "the forward fault is the one red line");
        assert!(css_rule(".mbn-stations .station.here").contains("var(--pointer)"), "the open block's station is the pointer's colour");
        assert!(css_rule(".mbn-flags.left .mbn-flag").contains("var(--flag-premise)"));
        assert!(css_rule(".mbn-flags.right .mbn-flag").contains("var(--flag-dependent)"));
        assert!(css_rule(".mbn-tuck").contains("margin-top: -9px"), "tucked cards overlap like a fanned cascade");
        assert!(css_rule(".mbn-tuck.pulled").contains("var(--card-shadow-lift)"), "the open Card is pulled out and lifted");
        for token in ["--cork:", "--cork-grain:", "--frame:", "--line-ink:", "--line-hot:", "--flag-premise:", "--flag-dependent:"] {
            assert_eq!(APP_CSS.matches(token).count(), 3, "{token} in the light block and both dark blocks");
        }
    }

    /// #203 (B5): on the Map, a Model Card's Links to its own parts draw no pointer thread (they are
    /// its table of contents), and the node where a part of the open model starts carries the
    /// part's name, drawn above it so the lens moves no Card.
    #[test]
    fn the_map_names_the_parts_of_a_model_and_leaves_out_its_table_of_contents() {
        let map = APP_JS;
        assert!(map.contains("const ownParts = card.model && card.model.head && card.model.head.address === card.address ? new Set((card.model.parts || []).map((p) => p.address)) : null;"));
        assert!(map.contains(".filter((at) => !(ownParts && ownParts.has(at)));"));
        assert!(map.contains("if (partStarts.has(n.address)) el.dataset.part = partStarts.get(n.address);"));
        let label = css_rule(".node[data-part]::before");
        assert!(label.contains("content: attr(data-part)") && label.contains("position: absolute"), "{label}");
    }

    /// S16 (#158, S16-AC05; design 15.2): a default graph view draws no Term edge, the one switch
    /// draws them as their own edges, and neither state adds, removes or moves a Card or an
    /// ordinary edge. `A -> B` is a Term edge iff `A.term_use_targets` names `B`, inbound and
    /// outbound alike; a target also named with an announced relation keeps its ordinary edge.
    #[test]
    fn a_default_graph_view_leaves_term_edges_out_and_the_switch_draws_them_apart() {
        let Some(out) = layout_js(
            "(() => {
               const card = (address, parent, edge, links, kinds, terms) =>
                 ({ address, tree: address.split('/')[0], parent, edge, links, kinds, term_use_targets: terms });
               // 1/1 reaches 1/2 only with a Term use, rests on 1/3 and also uses its Term, refers to 1/4.
               const cards = [
                 card('1/0', null, 'root', [], {}, []),
                 card('1/1', '1/0', 'direct', ['1/2', '1/3', '1/4'], { '1/3': 'rests-on' }, ['1/2', '1/3']),
                 card('1/2', '1/1', 'direct', ['1/1'], {}, ['1/1']),
                 card('1/3', '1/2', 'direct', [], {}, []),
                 card('1/4', '1/3', 'direct', ['1/1'], { '1/1': 'illustrates' }, []),
                 card('1/5', '1/4', 'direct', [], {}, []),
               ];
               const by = new Map(cards.map((c) => [c.address, c]));
               const split = (at, on) => L.edgeSplit(by.get(at), on);
               const outline = L.outline(cards, '1/0');
               const edges = (on) => L.focusEdges((at) => by.get(at), '1/1', ['1/2', '1/3', '1/4'], ['1/2', '1/4'], on);
               const lensOf = (on) => L.focusLens(outline, '1/1', { links: edges(on).lens })
                 .nodes.map((n) => [n.address, n.x, n.y, n.w, n.h, n.role, n.lod]);
               // Two trees: 1/1 -> 2/1 is a Term use only, 1/4 -> 2/1 an ordinary Link.
               const sky = [
                 ...cards,
                 card('2/0', null, 'root', [], {}, []),
                 card('2/1', '2/0', 'direct', [], {}, []),
               ];
               sky[1] = { ...sky[1], links: [...sky[1].links, '2/1'], term_use_targets: [...sky[1].term_use_targets, '2/1'] };
               sky[4] = { ...sky[4], links: [...sky[4].links, '2/1'] };
               const plain = sky.map((c) => ({ ...c, term_use_targets: [] }));
               const ring = L.constellation(sky, []);
               const ringPlain = L.constellation(plain, []);
               return {
                 off: split('1/1', false), on: split('1/1', true),
                 older: L.edgeSplit({ links: ['1/2'], kinds: {} }, true),
                 odd: L.edgeSplit({ links: ['1/2'], kinds: { '1/2': 'term' }, term_use_targets: ['1/2'] }, false),
                 none: L.edgeSplit(undefined, true),
                 inbound: [L.isTermEdge(by.get('1/2'), '1/1'), L.isTermEdge(by.get('1/4'), '1/1'), L.isTermEdge(by.get('1/1'), '1/2')],
                 mapOff: edges(false), mapOn: edges(true),
                 lensSame: JSON.stringify(lensOf(false)) === JSON.stringify(lensOf(true)),
                 lensCards: lensOf(false).map((n) => n[0]),
                 bundles: ring.bundles, termBundles: ring.termBundles,
                 plainBundles: ringPlain.bundles, plainTermBundles: ringPlain.termBundles,
                 ringSame: JSON.stringify([...ring.pos]) === JSON.stringify([...ringPlain.pos]),
               };
             })()",
        ) else {
            eprintln!("node is not installed; the layout module was not run");
            return;
        };
        // Default: the Term-only target 1/2 has no edge; 1/3 keeps its ordinary edge, and so does
        // the plain reference 1/4.
        assert_eq!(out["off"], json!({ "links": ["1/3", "1/4"], "terms": [] }));
        // On: the Term edges come in as their own list, and the ordinary edges are the same.
        assert_eq!(out["on"], json!({ "links": ["1/3", "1/4"], "terms": ["1/2", "1/3"] }));
        // A summary without `term_use_targets` keeps every Link; a stray `term` kind or a missing
        // summary does not throw.
        assert_eq!(out["older"], json!({ "links": ["1/2"], "terms": [] }));
        assert_eq!(out["odd"], json!({ "links": ["1/2"], "terms": [] }));
        assert_eq!(out["none"], json!({ "links": [], "terms": [] }));
        // Inbound applies the same rule: 1/2 -> 1/1 is a Term edge, 1/4 -> 1/1 is not, 1/1 -> 1/2 is.
        assert_eq!(out["inbound"], json!([true, false, true]));
        // The Map: no Term arc by default, both directions with the switch on, the ordinary arcs
        // the same in both states.
        assert_eq!(out["mapOff"]["out"], json!(["1/3", "1/4"]));
        assert_eq!(out["mapOff"]["in"], json!(["1/4"]));
        assert_eq!(out["mapOff"]["termOut"], json!([]));
        assert_eq!(out["mapOff"]["termIn"], json!([]));
        assert_eq!(out["mapOn"]["out"], out["mapOff"]["out"]);
        assert_eq!(out["mapOn"]["in"], out["mapOff"]["in"]);
        assert_eq!(out["mapOn"]["termOut"], json!(["1/2", "1/3"]));
        assert_eq!(out["mapOn"]["termIn"], json!(["1/2"]));
        // The lens shows the same Cards in the same places whether the switch is on or off.
        assert_eq!(out["lensSame"], json!(true));
        for at in ["1/1", "1/2", "1/3", "1/4"] {
            assert!(out["lensCards"].as_array().unwrap().contains(&json!(at)), "{at} stays in the lens");
        }
        // The Constellation: the Term-only Link between the trees is bundled apart, the ordinary one
        // counts as before, and the ring does not move.
        assert_eq!(out["bundles"], json!([{ "a": "1", "b": "2", "count": 1 }]));
        assert_eq!(out["termBundles"], json!([{ "a": "1", "b": "2", "count": 1 }]));
        assert_eq!(out["plainBundles"], json!([{ "a": "1", "b": "2", "count": 2 }]));
        assert_eq!(out["plainTermBundles"], json!([]));
        assert_eq!(out["ringSame"], json!(true));
    }

    /// S16 (#158, S16-AC05): the page itself — one switch for the Map, the Table and the
    /// Constellation, off by default and kept in this browser, with a label, a title, a pressed
    /// state and a focus ring; Term edges drawn only behind it, dotted pencil with their own word
    /// and tip; the Model board and the reader untouched by it.
    #[test]
    fn the_front_end_draws_term_edges_only_behind_one_switch() {
        // One switch, a real button with its label and pressed state, in the stage bar.
        let button = &INDEX_HTML[INDEX_HTML.find("id=\"term-toggle\"").expect("the switch")..];
        let button = &button[..button.find("</button>").expect("end of the switch")];
        assert!(INDEX_HTML.contains("<button class=\"lens-toggle term-toggle\" id=\"term-toggle\" type=\"button\" aria-pressed=\"false\" hidden>"));
        assert!(button.contains("<span>Term uses</span>"));
        assert_eq!(INDEX_HTML.matches("id=\"term-toggle\"").count(), 1, "one switch for every view");
        let bar = &INDEX_HTML[INDEX_HTML.find("<div class=\"stage-bar\">").unwrap()..INDEX_HTML.find("<div class=\"viewport\"").unwrap()];
        assert!(bar.contains("id=\"term-toggle\""), "the switch sits in the view controls");
        assert!(INDEX_HTML.contains("<span class=\"lg-term-row\" hidden><i class=\"lg-term\"></i>term use</span>"));
        // Off by default, kept like the other view options, shown in all three graph views.
        assert!(APP_JS.contains("termEdges: store.get('zt-term-edges') === '1',"));
        let set = app_function("setTermEdges");
        assert!(set.contains("store.set('zt-term-edges', on ? '1' : '0');") && set.contains("renderStage();"));
        let stage = app_function("renderStage");
        assert!(stage.contains("terms.hidden = !(inTree || onTableMode || state.mode === 'constellation');"));
        assert!(stage.contains("terms.setAttribute('aria-pressed', state.termEdges ? 'true' : 'false');"));
        assert!(stage.contains("terms.title = state.termEdges"));
        assert!(APP_JS.contains("$('#term-toggle').addEventListener('click', () => setTermEdges(!state.termEdges));"));
        // The Map draws Term arcs only from the switch's lists, and its lens does not follow the switch.
        assert!(APP_JS.contains("ZtLayout.focusEdges((at) => outline.card.get(at), card.address, outbound, inbound, state.termEdges);"));
        assert!(APP_JS.contains("links: arcs.lens, expandAll: state.expandAll,"));
        assert!(APP_JS.contains("put('t' + address, ZtLayout.linkArc(focus, b, 18), 'e-term out');"));
        assert!(APP_JS.contains("put('u' + address, ZtLayout.linkArc(b, focus, 18), 'e-term in');"));
        assert!(APP_JS.contains("ZtLayout.edgeSplit(state.outline.card.get(hovered), state.termEdges);"));
        assert_eq!(APP_JS.matches("'e-term").count(), 3, "three Term arc kinds and no other");
        // The Table: threads from the same split, a Term thread named `term` with a dot for a tip.
        assert!(APP_JS.contains("const edgesOf = (address) => ZtLayout.edgeSplit(state.byAddress.get(address), state.termEdges);"));
        assert!(APP_JS.contains("const kind = term ? 'term' : (node.kinds && node.kinds[target]) || 'refers';"));
        assert!(APP_JS.contains("word.textContent = term ? 'term' : KINDS[kind].fwd;"));
        assert!(APP_JS.contains("`${card.at} uses the term ${target}`"));
        let related = app_function("relatedTo");
        assert!(related.contains("const split = edgesOf(address);"));
        assert!(related.contains("if (state.termEdges) for (const source of state.termInboundOf.get(address) || []) related.add(source);"));
        // The Constellation: Term bundles and single Term lines only behind the switch.
        assert!(APP_JS.contains("if (state.termEdges) {\n        ctx.setLineDash([1.5 * px, 4 * px]);\n        for (const bundle of layout.termBundles) {")
            || APP_JS.contains("if (state.termEdges) {\r\n        ctx.setLineDash([1.5 * px, 4 * px]);\r\n        for (const bundle of layout.termBundles) {"));
        assert!(APP_JS.contains("const split = ZtLayout.edgeSplit(from, state.termEdges);"));
        // The Model board and the reader do not read the switch: Model semantics stay with it on.
        for name in ["modelBoard", "renderReader", "theoryBoard"] {
            let body = app_function(name);
            for word in ["termEdges", "edgesOf", "term_use_targets", "edgeSplit"] {
                assert!(!body.contains(word), "{name} reads `{word}`");
            }
        }
        // KINDS stays the announced kinds: a Term use is no relation kind.
        let kinds = &APP_JS[APP_JS.find("const KINDS = {").unwrap()..];
        let kinds = &kinds[..kinds.find("};").unwrap()];
        assert!(!kinds.contains("term"), "KINDS names no Term kind: {kinds}");
        // The style: dotted pencil in theme ink, not colour alone, with a focus ring on the switch.
        let arc = css_rule(".edges .e-term");
        assert!(arc.contains("stroke: var(--ink-soft)") && arc.contains("stroke-dasharray: .2 4.2") && !arc.contains('#'), "{arc}");
        let thread = css_rule(".thread.kind-term .line");
        assert!(thread.contains("stroke-dasharray: .2 4"), "{thread}");
        assert!(css_rule(".thread.hot.kind-term .line").contains("var(--ink-soft)"), "a hot Term thread stays pencil");
        assert!(css_rule(".term-toggle:focus-visible").contains("outline: 2px solid var(--accent)"));
        assert!(css_rule(".legend .lg-term").contains("dotted"));
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

    // --- the optional Bridge report projection (S20, #112, design §10.3) ---

    /// Poll a projection until its run answers, as the page's next read would.
    fn wait_projection(projections: &Projections, selected: &str, at: &str, revision: &str) -> Projection {
        for _ in 0..300 {
            let projection = projections.read(selected, at, revision);
            if projection.status != "pending" {
                return projection;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("the Bridge projection never left pending");
    }

    /// S20-AC06, D309: every helper answer becomes a status the reader can show beside the
    /// ordinary term and model evidence; only a successful projection supplies candidates.
    #[test]
    fn a_bridge_projection_reports_every_helper_answer() {
        let hash = "sha256:abc";
        let status = |run| projection_from_run(run, hash);
        assert_eq!(status(HelperRun::Missing).status, "missing_helper");
        assert_eq!(status(HelperRun::Timeout).status, "timeout");
        assert_eq!(status(HelperRun::Failed("boom".into())).status, "failed");
        assert_eq!(status(HelperRun::Output("no projection here".into())).status, "invalid_report");
        assert_eq!(status(HelperRun::Output("{not json".into())).status, "invalid_report");
        let refused = status(HelperRun::Output(
            r#"{"ok":false,"step":"project","status":"invalid_report","errors":["invalid published report schema or fingerprint"]}"#.into(),
        ));
        assert_eq!(refused.status, "invalid_report");
        assert_eq!(refused.errors, vec!["invalid published report schema or fingerprint"]);
        assert_eq!(
            status(HelperRun::Output(r#"{"ok":false,"status":"failed","errors":["zt mcp failed"]}"#.into())).status,
            "failed",
            "an operational failure is not described as an invalid report"
        );
        // A successful status with no candidate is still a status, not an error.
        let stale = status(HelperRun::Output(
            r#"{"ok":true,"step":"project","status":"stale","report_id":"r-1","candidates":[],"stale":[{"proposal":"b001","why":"changed endpoint"}],"counts":{"at":1,"current":0}}"#.into(),
        ));
        assert_eq!(stale.status, "stale");
        assert!(stale.candidates.is_empty());
        assert_eq!(stale.report_id, "r-1");
        assert_eq!(
            status(HelperRun::Output(r#"{"ok":true,"status":"not_in_report","candidates":[]}"#.into())).status,
            "not_in_report"
        );
        let done = status(HelperRun::Output(
            r#"{"ok":true,"step":"project","status":"done","report_id":"r-1","candidates":[{"report":"r-1","proposal":"b003","target":"Old/1|a","relation":"contrasts"}],"counts":{"at":1,"current":1}}"#.into(),
        ));
        assert_eq!(done.status, "done");
        assert_eq!(done.candidates.len(), 1);
        assert_eq!(done.candidates[0].proposal, "b003");
        assert_eq!(done.candidates[0].target, "Old/1|a");
        assert_eq!(done.candidates[0].relation.as_deref(), Some("contrasts"));
        assert_eq!(done.counts["current"], json!(1));
        // A malformed candidate is a schema mismatch, never a silent suggestion.
        assert_eq!(
            status(HelperRun::Output(r#"{"ok":true,"status":"done","candidates":[{"proposal":"b001"}]}"#.into())).status,
            "invalid_report"
        );
        // The twenty-candidate bound holds at this seam too.
        let many: Vec<String> = (1..=25)
            .map(|i| format!(r#"{{"report":"r-1","proposal":"b{i:03}","target":"Old/{i}"}}"#))
            .collect();
        let bounded = status(HelperRun::Output(format!(
            r#"{{"ok":true,"status":"done","candidates":[{}]}}"#,
            many.join(",")
        )));
        assert_eq!(bounded.candidates.len(), BRIDGE_CANDIDATES_MAX);
    }

    /// S20-AC03, AC04: one fixed argument vector with the path as a single argument, one helper run
    /// per report content, archive revision, and Card, and a new report or revision reads again.
    #[test]
    fn the_bridge_projection_runs_once_per_report_revision_and_card() {
        let temp = tempfile::tempdir().expect("temp dir");
        let report = temp.path().join("odd & del; report.json");
        fs::write(&report, r#"{"schema":1,"report_id":"r-1"}"#).unwrap();
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let seen = Arc::clone(&calls);
        let projections = Projections::new(
            PathBuf::from("zt-digest-test"),
            Duration::from_secs(5),
            Arc::new(move |program, args, _| {
                seen.lock().unwrap().push(
                    std::iter::once(program.display().to_string())
                        .chain(args.iter().cloned())
                        .collect(),
                );
                HelperRun::Output(
                    r#"{"ok":true,"step":"project","status":"done","report_id":"r-1","candidates":[{"report":"r-1","proposal":"b001","target":"Old/1|a","relation":"contrasts"}],"counts":{"at":1,"current":1}}"#
                        .into(),
                )
            }),
        );
        let selected = report.display().to_string();
        // The first read starts the run and answers pending: the reader keeps its Card and its
        // ordinary term and model evidence while the helper works.
        assert_eq!(projections.read(&selected, "0/1", "revision-1").status, "pending");
        let ready = wait_projection(&projections, &selected, "0/1", "revision-1");
        assert_eq!(ready.status, "done");
        assert_eq!(ready.candidates[0].target, "Old/1|a");
        assert_eq!(calls.lock().unwrap().len(), 1);
        // The same read is answered from memory: no helper per repaint or hover.
        let reused = projections.read(&selected, "0/1", "revision-1");
        assert_eq!(reused.status, "done");
        assert!(reused.cached, "a repeated read is answered from memory");
        assert_eq!(calls.lock().unwrap().len(), 1);
        // The exact fixed argument vector, with the whole path in one argument.
        assert_eq!(
            calls.lock().unwrap()[0],
            vec!["zt-digest-test", "bridge", "-Step", "project", "-Report", &selected, "-At", "0/1"]
        );
        assert!(
            calls.lock().unwrap()[0].iter().any(|arg| arg == &selected),
            "the path is one argument, never part of a command line"
        );
        // A changed archive revision invalidates the endpoint evidence: the next read revalidates.
        assert_eq!(projections.read(&selected, "0/1", "revision-2").status, "pending");
        assert_eq!(wait_projection(&projections, &selected, "0/1", "revision-2").status, "done");
        assert_eq!(calls.lock().unwrap().len(), 2);
        // A changed report file is a different report.
        fs::write(&report, r#"{"schema":1,"report_id":"r-2"}"#).unwrap();
        assert_eq!(projections.read(&selected, "0/1", "revision-2").status, "pending");
        assert_eq!(wait_projection(&projections, &selected, "0/1", "revision-2").status, "done");
        assert_eq!(calls.lock().unwrap().len(), 3);
    }

    /// S20-AC01, AC06: no folder is scanned and no guessed report is read; a path that names no
    /// readable file costs no helper run.
    #[test]
    fn a_bridge_projection_without_a_readable_report_starts_no_helper() {
        let temp = tempfile::tempdir().expect("temp dir");
        let calls = Arc::new(Mutex::new(0usize));
        let seen = Arc::clone(&calls);
        let projections = Projections::new(
            PathBuf::from("zt-digest-test"),
            Duration::from_secs(5),
            Arc::new(move |_, _, _| {
                *seen.lock().unwrap() += 1;
                HelperRun::Missing
            }),
        );
        let missing = temp.path().join("missing.json").display().to_string();
        let folder = temp.path().display().to_string();
        for selected in ["", "relative/proposals.json", &missing, &folder] {
            let projection = projections.read(selected, "0/1", "revision-1");
            assert_eq!(projection.status, "missing_report", "{selected}");
            assert!(projection.candidates.is_empty());
        }
        assert_eq!(*calls.lock().unwrap(), 0, "an unreadable report starts no helper");
    }

    /// S20-AC03, D260: a helper past its lifetime is cancelled instead of holding the reader.
    #[test]
    fn a_bridge_helper_past_its_lifetime_is_cancelled() {
        #[cfg(windows)]
        let (program, args) = ("cmd.exe", vec!["/c".to_string(), "ping -n 30 127.0.0.1 >nul".to_string()]);
        #[cfg(not(windows))]
        let (program, args) = ("/bin/sh", vec!["-c".to_string(), "sleep 30".to_string()]);
        let started = Instant::now();
        let run = run_helper(Path::new(program), &args, Duration::from_millis(300));
        assert!(matches!(run, HelperRun::Timeout), "a stuck helper is cancelled");
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "the cancelled helper returns at its deadline"
        );
    }

    /// S16 (#116, D232, D233): a question is trimmed and bounded by the question path's own 512
    /// characters, counted as characters rather than UTF-16 units, and the kept Spread's display
    /// name is cut to the schema's 80 characters while the whole question stays in `question`.
    #[test]
    fn the_ask_question_is_trimmed_and_bounded_and_only_the_name_is_cut() {
        let Some(out) = ask_js(
            "{\
               plain: ZtAsk.askQuestion('  为什么学习  '),\
               blank: ZtAsk.askQuestion('   '),\
               missing: ZtAsk.askQuestion(undefined),\
               longest: ZtAsk.askQuestion('学'.repeat(512)),\
               too_long: ZtAsk.askQuestion('学'.repeat(513)),\
               name: ZtAsk.askName('为什么学习可能造成多重均衡？'.repeat(9)),\
               emoji: Array.from(ZtAsk.askName('😀'.repeat(100))).length,\
               emoji_units: ZtAsk.askName('😀'.repeat(100)).length\
             }",
        ) else {
            eprintln!("node is not installed; the ask module was not run");
            return;
        };
        assert_eq!(out["plain"]["ok"], json!(true), "{out}");
        assert_eq!(out["plain"]["question"], json!("为什么学习"));
        assert_eq!(out["plain"]["name"], json!("为什么学习"));
        assert_eq!(out["blank"]["ok"], json!(false), "{out}");
        assert!(
            out["blank"]["error"]
                .as_str()
                .is_some_and(|error| error.contains("nonblank")),
            "{out}"
        );
        assert_eq!(out["missing"]["ok"], json!(false), "{out}");
        assert_eq!(out["longest"]["ok"], json!(true), "{out}");
        let longest = out["longest"]["question"].as_str().expect("question");
        assert_eq!(longest.chars().count(), 512);
        assert_eq!(out["too_long"]["ok"], json!(false), "{out}");
        assert!(
            out["too_long"]["error"]
                .as_str()
                .is_some_and(|error| error.contains("512")),
            "{out}"
        );
        assert_eq!(out["name"].as_str().expect("name").chars().count(), 80);
        assert_eq!(out["emoji"], json!(80), "the cut counts characters, not UTF-16 units");
        assert_eq!(out["emoji_units"], json!(160), "{out}");
    }

    /// S16 (#116, D232, D234): an answered question becomes a temporary Table — the proposed Cards
    /// at their suggested places, each with the reason it was chosen and the fingerprint the Keep
    /// check uses — and a second question decks a different Table. Nothing here is a saved Spread.
    #[test]
    fn a_proposal_becomes_a_temporary_table_with_its_reasons() {
        let question = "为什么学习可能造成多重均衡？".repeat(9);
        let proposal = json!({
            "question": question.clone(),
            "held": "bookA/1",
            "sparse": false,
            "truncated": true,
            "matched_terms": ["多重均衡"],
            "ambiguous_terms": ["学习"],
            "spread": {
                "name": question.chars().take(80).collect::<String>(),
                "question": question.clone(),
                "cards": [
                    { "at": "bookA/1", "x": 0, "y": 0 },
                    { "at": "bookB/1", "x": 266, "y": 0 },
                ],
            },
            "cards": [
                {
                    "at": "bookA/1",
                    "title": "多重均衡的陈述",
                    "reason": "States the accepted term 多重均衡.",
                    "authored_fingerprint": "aa",
                },
                {
                    "at": "bookB/1",
                    "title": "策略互补",
                    "reason": "Reached in 1 semantic step(s) from the entry Card for 多重均衡.",
                    "authored_fingerprint": "bb",
                },
            ],
        });
        let other = json!({
            "question": "策略互补说明了什么？",
            "held": Value::Null,
            "spread": { "name": "策略互补说明了什么？", "question": "策略互补说明了什么？", "cards": [] },
            "cards": [],
        });
        let Some(out) = ask_js(&format!(
            "({{ preview: ZtAsk.askPreview({proposal}), other: ZtAsk.askPreview({other}) }})"
        )) else {
            eprintln!("node is not installed; the ask module was not run");
            return;
        };
        let preview = &out["preview"];
        assert_eq!(preview["question"], json!(question));
        assert_eq!(
            preview["name"].as_str().expect("name").chars().count(),
            80,
            "the display name is cut; the question is not: {preview}"
        );
        assert_eq!(preview["held"], json!("bookA/1"));
        assert_eq!(preview["cards"][0]["at"], json!("bookA/1"));
        assert_eq!(preview["cards"][0]["x"].as_f64(), Some(0.0));
        assert_eq!(preview["cards"][1]["x"].as_f64(), Some(266.0));
        assert_eq!(
            preview["cards"][1]["reason"],
            json!("Reached in 1 semantic step(s) from the entry Card for 多重均衡.")
        );
        assert_eq!(preview["cards"][1]["fingerprint"], json!("bb"));
        assert_eq!(preview["cards"][1]["title"], json!("策略互补"));
        assert_eq!(preview["sparse"], json!(false));
        assert_eq!(preview["truncated"], json!(true));
        assert_eq!(preview["matched_terms"], json!(["多重均衡"]));
        assert_eq!(preview["ambiguous_terms"], json!(["学习"]));
        assert_eq!(
            preview["id"], json!(format!("ask:{question}")),
            "the Table is decked for this question"
        );
        assert_ne!(preview["id"], out["other"]["id"], "another question decks another Table");
        assert_eq!(out["other"]["cards"], json!([]), "zero cards is a valid answer");
        assert_eq!(out["other"]["held"], Value::Null);
    }

    /// S16 (#116, D232): the reason of one proposed Card is readable by address, and an address the
    /// question did not propose has none.
    #[test]
    fn the_temporary_table_names_why_each_proposed_card_is_there() {
        let proposal = json!({
            "question": "多重均衡",
            "held": "bookA/1",
            "spread": { "name": "多重均衡", "question": "多重均衡", "cards": [
                { "at": "bookA/1", "x": 0, "y": 0 },
                { "at": "bookB/1", "x": 10, "y": 0 },
            ] },
            "cards": [
                { "at": "bookA/1", "title": "甲", "reason": "States the accepted term 多重均衡.", "authored_fingerprint": "aa" },
            ],
        });
        let Some(out) = ask_js(&format!(
            "(() => {{ const p = ZtAsk.askPreview({proposal}); \
               return {{ held: ZtAsk.askReason(p, 'bookA/1'), plain: ZtAsk.askReason(p, 'bookB/1'), \
                         unknown: ZtAsk.askReason(p, '9/9') }}; }})()"
        )) else {
            eprintln!("node is not installed; the ask module was not run");
            return;
        };
        assert_eq!(out["held"], json!("States the accepted term 多重均衡."));
        assert_eq!(out["plain"], json!(""), "a Card with no explanation is not given one");
        assert_eq!(out["unknown"], json!(""));
    }

    /// S16 (#116, D232, D233, D282): Keep writes one ordinary named Spread into the Spread document
    /// through the existing save shape: the earlier Spreads stay exactly as they were (questions
    /// included), the new one carries its id, cut name, whole question and positions — and none of
    /// the transient explanation or fingerprint — and the kept document is a new value, never a
    /// mutation of the document the page holds.
    #[test]
    fn keeping_a_proposal_adds_one_ordinary_spread_and_leaves_the_others_as_they_were() {
        let doc = json!({
            "current": "old",
            "spreads": [
                { "id": "old", "name": "Work", "cards": [{ "at": "1/1", "x": 0, "y": 0 }] },
                { "id": "asked", "name": "上一个问题", "question": "上一个问题",
                  "cards": [{ "at": "2/1", "x": 10, "y": 10 }] },
            ],
        });
        let question = "为什么学习可能造成多重均衡？".repeat(9);
        let proposal = json!({
            "question": question.clone(),
            "held": "bookA/1",
            "spread": {
                "name": question.chars().take(80).collect::<String>(),
                "question": question.clone(),
                "cards": [
                    { "at": "bookA/1", "x": 0, "y": 0 },
                    { "at": "bookB/1", "x": 266.4, "y": -1 },
                ],
            },
            "cards": [
                { "at": "bookA/1", "title": "甲", "reason": "States the accepted term.", "authored_fingerprint": "aa" },
                { "at": "bookB/1", "title": "乙", "reason": "One step from it.", "authored_fingerprint": "bb" },
            ],
        });
        let Some(out) = ask_js(&format!(
            "(() => {{ const doc = {doc}; const p = ZtAsk.askPreview({proposal}); \
               const kept = ZtAsk.askKeep(doc, p, 'kept-1'); \
               return {{ kept, doc, plain: ZtAsk.askKeep(doc, {{ name: 'Q', question: '', cards: p.cards }}, 'kept-2') }}; }})()"
        )) else {
            eprintln!("node is not installed; the ask module was not run");
            return;
        };
        let kept = &out["kept"];
        assert_eq!(kept["current"], json!("kept-1"), "{kept}");
        assert_eq!(kept["spreads"].as_array().expect("spreads").len(), 3);
        assert_eq!(kept["spreads"][0], doc_spread(&out["doc"], 0), "an earlier Spread is untouched");
        assert_eq!(kept["spreads"][1], doc_spread(&out["doc"], 1), "a kept question is not lost");
        let spread = &kept["spreads"][2];
        assert_eq!(spread["id"], json!("kept-1"));
        assert_eq!(spread["name"].as_str().expect("name").chars().count(), 80);
        assert_eq!(spread["question"], json!(question));
        assert_eq!(
            spread["cards"],
            json!([
                { "at": "bookA/1", "x": 0, "y": 0 },
                { "at": "bookB/1", "x": 266.4, "y": -1 },
            ]),
            "only the Spread schema's own fields are saved"
        );
        assert!(spread.get("reason").is_none() && spread["cards"][0].get("fingerprint").is_none());
        // The document the page held is unchanged: Keep builds a new one.
        assert_eq!(out["doc"]["current"], json!("old"));
        assert_eq!(out["doc"]["spreads"].as_array().expect("spreads").len(), 2);
        assert_eq!(out["doc"]["spreads"][1]["question"], json!("上一个问题"));
        assert!(
            out["plain"]["spreads"][2].get("question").is_none(),
            "a Spread without a question stores no question: {}",
            out["plain"]
        );
    }

    fn doc_spread(doc: &Value, index: usize) -> Value {
        doc["spreads"][index].clone()
    }

    /// S16 (#116, D234): before Keep, every proposed endpoint is re-read and its authored
    /// fingerprint compared with the one the answer captured. An unchanged Card is unchanged, a
    /// changed one is named with both titles, a vanished one is missing, and a new occupant of an
    /// old address is a change — never silently the Card the question selected. A proposal that
    /// captured no fingerprint cannot be trusted either.
    #[test]
    fn a_changed_or_missing_endpoint_is_named_before_keep() {
        let proposal = json!({
            "question": "多重均衡",
            "held": "bookA/1",
            "spread": { "name": "多重均衡", "question": "多重均衡", "cards": [
                { "at": "bookA/1", "x": 0, "y": 0 },
                { "at": "bookB/1", "x": 10, "y": 0 },
            ] },
            "cards": [
                { "at": "bookA/1", "title": "多重均衡的陈述", "reason": "States it.", "authored_fingerprint": "aa" },
                { "at": "bookB/1", "title": "策略互补", "reason": "One step.", "authored_fingerprint": "bb" },
            ],
        });
        let Some(out) = ask_js(&format!(
            "(() => {{ const p = ZtAsk.askPreview({proposal}); const a = {{ authored_fingerprint: 'aa', title: '多重均衡的陈述' }}; \
               return {{\
                 same: ZtAsk.askChanged(p, {{ 'bookA/1': a, 'bookB/1': {{ authored_fingerprint: 'bb', title: '策略互补' }} }}),\
                 edited: ZtAsk.askChanged(p, {{ 'bookA/1': a, 'bookB/1': {{ authored_fingerprint: 'cc', title: '策略互补（改）' }} }}),\
                 gone: ZtAsk.askChanged(p, {{ 'bookA/1': a }}),\
                 occupant: ZtAsk.askChanged(p, {{ 'bookA/1': a, 'bookB/1': {{ authored_fingerprint: 'dd', title: '现在占着这个地址的卡' }} }}),\
                 untyped: ZtAsk.askChanged({{ cards: [{{ at: 'bookB/1', title: '策略互补', fingerprint: '' }}] }}, {{ 'bookB/1': {{ authored_fingerprint: 'bb', title: '策略互补' }} }}),\
                 trimmed: ZtAsk.askWithout(p, ['bookB/1'])\
               }}; }})()"
        )) else {
            eprintln!("node is not installed; the ask module was not run");
            return;
        };
        assert_eq!(out["same"]["changed"], json!([]), "{}", out["same"]);
        assert_eq!(out["same"]["missing"], json!([]), "{}", out["same"]);
        assert_eq!(out["same"]["unchanged"], json!(["bookA/1", "bookB/1"]));
        assert_eq!(
            out["edited"]["changed"],
            json!([{ "at": "bookB/1", "then": "策略互补", "now": "策略互补（改）" }]),
            "{}",
            out["edited"]
        );
        assert_eq!(out["edited"]["unchanged"], json!(["bookA/1"]));
        assert_eq!(
            out["gone"]["missing"],
            json!([{ "at": "bookB/1", "then": "策略互补" }]),
            "{}",
            out["gone"]
        );
        assert_eq!(
            out["occupant"]["changed"][0]["now"],
            json!("现在占着这个地址的卡"),
            "a new occupant is a change, not the previously selected Card: {}",
            out["occupant"]
        );
        assert_eq!(out["occupant"]["unchanged"], json!(["bookA/1"]));
        assert_eq!(
            out["untyped"]["changed"][0]["at"],
            json!("bookB/1"),
            "a proposal with no fingerprint is never trusted silently: {}",
            out["untyped"]
        );
        // Keep saves only what is left, and the question and held Card stay with the Table.
        assert_eq!(out["trimmed"]["cards"].as_array().expect("cards").len(), 1);
        assert_eq!(out["trimmed"]["cards"][0]["at"], json!("bookA/1"));
        assert_eq!(out["trimmed"]["question"], json!("多重均衡"));
        assert_eq!(out["trimmed"]["held"], json!("bookA/1"));
    }

    /// S16 (#116, D232, D234; S16-AC01, S16-AC04, S16-AC06, S16-AC07): the shipped page offers the
    /// `ask` command, decks the answer on the temporary Table with its reasons, keeps it through the
    /// one existing Spread save path, cancels back to the previous Table, re-reads every endpoint
    /// before Keep, and guards the keys and the Table gestures while a proposal lies there. A
    /// proposal is never written: the temporary Table's changes return before the save.
    #[test]
    fn the_front_end_asks_the_box_and_keeps_only_on_the_readers_choice() {
        assert!(
            APP_JS.contains("['ask [<question>]'"),
            "the command bar offers ask: {APP_JS}"
        );
        assert!(APP_JS.contains("case 'ask': return askBox(rest.join(' '));"));
        assert!(APP_JS.contains("api('ask', { question: checked.question })"));
        assert_eq!(
            APP_JS.matches("api('spreads', body)").count(),
            2,
            "Keep sends the one Spread document to the one existing save route"
        );
        assert!(
            APP_JS.contains("const sent = saving.catch(() => {}).then(() => api('spreads', body));"),
            "Keep confirms the save instead of the debounced Table save"
        );
        assert!(
            !APP_JS.contains("api('spreads/") && !APP_JS.contains("api('spread'"),
            "no second write route was added for keeping"
        );
        let ask = APP_JS.find("api('ask', { question: checked.question })").expect("ask call");
        let begin = APP_JS.find("spreads.begin(ZtAsk.askPreview(proposal));").expect("begin");
        assert!(ask < begin, "a proposal is decked only after the box answered");
        assert!(
            APP_JS.contains("the question could not be answered"),
            "a failed question says so"
        );
        assert!(
            APP_JS.contains("if (preview) { renderTableCount(); return; }"),
            "a temporary Table's changes are never saved"
        );
        assert!(
            APP_JS.contains("spreads.previewing() ? spreads.held() : state.pointer"),
            "the answer's held Card is squared up and lifted on the temporary Table"
        );
        assert!(APP_JS.contains("el.classList.toggle('proposed', proposed);"));
        assert!(APP_JS.contains("const why = proposed ? ZtAsk.askReason(spreads.now(), card.at) : '';"));
        assert!(APP_JS.contains("async function keepAskSpread()"));
        assert!(
            APP_JS.contains("fresh[card.at] = await api('card?at=' + enc(card.at));"),
            "Keep re-reads each proposed endpoint"
        );
        let changed = APP_JS
            .find("const changed = ZtAsk.askChanged(preview, fresh);")
            .expect("the endpoint check");
        let kept = APP_JS
            .find("const kept = ZtAsk.askKeep(spreads.doc, keep, spreads.newId());")
            .expect("the kept document");
        assert!(changed < kept, "the recheck runs before Keep writes");
        assert!(APP_JS.contains("if (!(await askChangedDialog(changed))) return;"));
        assert!(APP_JS.contains(
            "keep = ZtAsk.askWithout(preview, changed.missing.map((card) => card.at));"
        ));
        // The changed-set dialog belongs to the keyboard too: Esc is the way back, never Keep, and
        // the dialog consumes it so the page's own Esc does not also fire.
        assert!(APP_JS.contains("// Esc is the way back, as in every other dialog: it never keeps anything."));
        assert!(APP_JS.contains("if (event.key !== 'Escape') return;"));
        assert!(
            APP_JS.contains("if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(); resolve(null); }"),
            "the Ask dialog consumes its own Escape"
        );
        assert_eq!(
            APP_JS.matches("finish(true)").count(),
            1,
            "only the Keep button keeps from the changed-set dialog"
        );
        assert_eq!(
            APP_JS.matches("finish(false)").count(),
            2,
            "Esc and Back both go back, never Keep"
        );
        assert!(APP_JS.contains("await spreads.adopt(kept);"));
        assert!(APP_JS.contains("function cancelAskSpread()"));
        assert!(
            APP_JS.contains("the proposal is gone; the previous Spread is back on the Table"),
            "Cancel says the previous Table is back"
        );
        // The Table's own gestures do not change a proposal's set, and filing waits for Keep.
        for guard in [
            "the proposal is a temporary Table: Keep or Cancel it to change the Spread",
            "the Table holds a question’s proposal: Keep or Cancel it before changing the Spread",
            "the Table holds a question’s proposal: Keep or Cancel it before laying other cards",
            "a question’s proposal is not a saved Spread yet: Keep it, then file it as a card",
        ] {
            assert!(APP_JS.contains(guard), "the page guards the preview: {guard}");
        }
        // The keys belong to the page until the reader answers: y keeps, Esc cancels, and both are
        // behind the same input/desk guard as every other page key.
        assert!(APP_JS.contains(
            "case 'y': return spreads.previewing() ? run(keepAskSpread) : undefined;"
        ));
        assert!(APP_JS.contains(
            "spreads.previewing() ? cancelAskSpread() : pullout.node ? dropBack() : openRail(false)"
        ));
        let keys = &APP_JS[APP_JS.find("function installKeys()").expect("installKeys")..];
        assert!(
            keys.find("if (state.editing || finder.isOpen) return;")
                < keys.find("case 'y':"),
            "the ask keys never fire while a dialog or the search tray owns the keyboard"
        );
        assert!(
            keys.find("if (isTyping(event.target) || event.ctrlKey") < keys.find("case 'y':"),
            "the ask keys never fire inside an input"
        );
        // The page loads the module, and the bar is an element of the stage row, not a dialog over
        // the Cards: while it shows, the title and the Table's tools stand down.
        assert!(INDEX_HTML.contains("<script src=\"/ask.js\" defer></script>"));
        for id in ["ask-bar", "ask-q", "ask-terms", "ask-note", "ask-keep", "ask-cancel", "ask-open"] {
            assert!(
                INDEX_HTML.contains(&format!("id=\"{id}\"")),
                "the page carries #{id}"
            );
        }
        assert!(INDEX_HTML.contains("Ask the box"));
        let stage_bar = INDEX_HTML.find("class=\"stage-bar\"").expect("stage bar");
        let bar = INDEX_HTML.find("id=\"ask-bar\"").expect("ask bar");
        let viewport = INDEX_HTML.find("id=\"viewport\"").expect("viewport");
        assert!(
            stage_bar < bar && bar < viewport,
            "the ask bar sits in the stage row, not over the Cards"
        );
        for line in [
            "const previewing = spreads.previewing();",
            "$('#stage-title').hidden = onTableMode && previewing;",
            "$('#table-tools').hidden = !onTableMode || previewing;",
        ] {
            assert!(
                APP_JS.contains(line),
                "the title and the Table's tools stand down for the bar: {line}"
            );
        }
        // Light and dark: the bar and the reasons take their ink from the page's theme variables,
        // so both themes get the same bar; Keep and Cancel are ordinary buttons, which draw the
        // page's own focus ring.
        for rule in [".ask-bar {", ".tcard.proposed {", ".tcard .why {", ".ask-changed {"] {
            assert!(APP_CSS.contains(rule), "the stylesheet has {rule}");
        }
        let start = APP_CSS.find(".ask-bar {").expect("ask-bar rule");
        let block = &APP_CSS[start..start + APP_CSS[start..].find("\n}").expect("rule end")];
        assert!(
            !block.contains('#') && block.contains("var(--"),
            "the bar is painted from the theme, not a fixed colour: {block}"
        );
        assert!(APP_CSS.contains(
            ":focus-visible { outline: 2px solid color-mix(in srgb, var(--accent) 70%, transparent)"
        ));
        assert!(
            APP_CSS.contains(".tcard.proposed .put-back { display: none; }"),
            "a proposed Card cannot be put back before Keep or Cancel"
        );
        let why = APP_CSS.find(".tcard .why {").expect("the reason band");
        let why = &APP_CSS[why..why + APP_CSS[why..].find("\n}").expect("rule end")];
        assert!(
            why.contains("position: absolute") && why.contains("background:") && why.contains("var(--paper)"),
            "the reason is its own legible band over the ruled paper: {why}"
        );
    }

    /// S17 (#118, S17-AC01, S17-AC04, S17-AC05, S17-AC06): the shipped page files a kept Ask Spread
    /// as one agent Hub Card draft through the existing composer — `fileSpread` reads the plan, the
    /// desk states the agent Workspace and the Hub type, the placement controls choose the level and
    /// the destination, the hub precheck gates Save, and the one existing create (or guarded edit)
    /// route writes the Card. Filing itself writes nothing, and Cancel writes nothing.
    #[test]
    fn the_front_end_files_a_kept_spread_as_an_agent_hub_draft() {
        // The routes exist and read through the cached Snapshot, like `/api/ask`.
        assert!(GUI_RS.contains("(Method::Post, \"/api/hub/draft\") => self.hub_draft(body),"));
        assert!(GUI_RS.contains("(Method::Post, \"/api/hub/check\") => self.hub_check(body),"));
        assert!(GUI_RS.contains("crate::hub::draft(&snapshot, body)"));
        assert!(GUI_RS.contains("crate::hub::precheck(&snapshot, body)"));
        // `fileSpread` branches on the Spread's kept question and opens the plan, unchanged for the
        // Spreads that were never a question.
        assert!(APP_JS.contains("if (spread && spread.question) return fileAskSpread(spread, cards);"));
        assert!(APP_JS.contains("async function fileAskSpread(spread, cards)"));
        assert!(
            APP_JS.contains("plan = await api('hub/draft', { cards: addresses, question: spread.question, title: spread.name, reasons });"),
            "the page asks for the plan, and passes the reasons the answer gave"
        );
        assert!(
            APP_JS.contains("const proposal = await api('ask', { question });"),
            "a Spread kept in an earlier session reads its own question again for its reasons"
        );
        assert!(
            APP_JS.contains("if (wanted.has(card.at) && card.reason) reasons[card.at] = card.reason;"),
            "only the reasons of the Spread's own Cards are used"
        );
        // The desk states the agent Workspace and the Hub type, whatever the cabinet shows, and the
        // one create route writes a side successor at the chosen level.
        assert!(APP_JS.contains("heading: `Hub draft · ${spread.name || 'Spread'} · agent workspace`"));
        assert!(APP_JS.contains("cardType: 'hub',"));
        assert!(APP_JS.contains("h('span', { class: 'hub-ws', text: 'agent workspace'"));
        assert!(APP_JS.contains(
            "api('create', { kind: 'side', at: placement.at, text, type: 'hub', level: placement.level })"
        ));
        assert!(APP_JS.contains(
            "api('edit', { at: placement.at, text, base: placement.base })"
        ));
        assert!(
            APP_JS.contains("if (!placement || !placement.at) throw new Error('choose where the hub is filed first');"),
            "no placement is no Save"
        );
        // The placement controls: the level select and the destination select, filled from the
        // plan's legal choices, a rewrite never offered a second hub.
        assert!(APP_JS.contains("'aria-label': 'Hub level'"));
        assert!(APP_JS.contains("'aria-label': 'Destination of the hub draft'"));
        assert!(
            APP_JS.contains("const hubOptions = !hub ? [] : hub.destination && hub.destination.how === 'edit'")
                && APP_JS.contains("? [hub.destination]")
                && APP_JS.contains(": (hub.choices || []).filter((choice) => choice.how === 'side');"),
            "a rewrite goes back to the one hub it was drafted from, and a new draft to a create"
        );
        assert!(APP_JS.contains("const fillDestinations = (keepEmpty = false) => {"));
        assert!(
            APP_JS.contains("{ value: '', text: '— where is it filed? —' }"),
            "a plan with no destination asks for one inside the editor"
        );
        // The hub precheck runs with every edit and gates Save.
        assert!(APP_JS.contains("hubCheck = await hub.check(text.value, hubPlaced.level, hubPlaced.how, hubPlaced.at);"));
        assert!(APP_JS.contains("await hubPrecheck();"), "every edit rechecks the draft");
        assert!(APP_JS.contains("saveBtn.disabled = faults.length > 0 || !hubCheck;"));
        assert!(APP_JS.contains("if (hub && !(hubCheck && hubCheck.ok)) {"));
        assert!(APP_JS.contains("the hub precheck must pass before Save; fix the named faults"));
        assert!(APP_JS.contains("h('div', { class: 'hub-fault', dataset: { fault: fault.fault } }"));
        // Cancel and an empty plan write nothing: the Save is the one write, behind `if (!address)`.
        assert!(APP_JS.contains("if (!address) return;"), "Cancel returns without writing");
        assert!(APP_JS.contains("await afterChange(address, `filed the hub draft as ${address}`);"));
        assert!(
            !APP_JS.contains("api('hub/draft', { cards: addresses, question: spread.question, title: spread.name, reasons }).then"),
            "the plan's answer is checked before the desk opens"
        );
        // The plan and the precheck are read-only: no write route is added for them.
        for route in ["api('hub/create'", "api('hub/save'", "api('hub/file'"] {
            assert!(!APP_JS.contains(route), "no second composer or write route: {route}");
        }
        // The placement bar and the fault panel are painted from the theme's own variables.
        for rule in [".hub-bar {", ".hub-ws {", ".hub-level, .hub-dest {", ".hub-fault {", ".hub-ok {"] {
            assert!(APP_CSS.contains(rule), "the stylesheet has {rule}");
        }
        let start = APP_CSS.find(".hub-bar {").expect("hub-bar rule");
        let block = &APP_CSS[start..start + APP_CSS[start..].find("\n}").expect("rule end")];
        assert!(
            !block.contains('#') && block.contains("var(--"),
            "the hub bar is painted from the theme: {block}"
        );
        // The desk's own rules are one composer: the reply, create, and insert paths still use it.
        assert_eq!(
            APP_JS.matches("function openEditor(").count(),
            1,
            "one writing desk serves every composer"
        );
    }

    /// S24 (#120, S24-AC05 check 4): the one writing desk serves Edit, a new Card, Insert and Reply
    /// as well as the hub draft, and only the hub draft has a placement (`hubPlaced` is null for
    /// every other desk). A statement the desk runs as it opens may read the placement only under
    /// `if (hub)`; any other read sits inside a handler or helper that only the hub draft's own
    /// controls call. The browser pass found `if (hubPlaced.at)` at the desk's top level, which
    /// threw on every non-hub desk (Reply and Edit showed "Cannot read properties of null").
    #[test]
    fn every_desk_but_the_hub_draft_opens_without_reading_a_hub_placement() {
        // A Windows checkout may hold the page with CRLF line ends; the desk is read line by line.
        let page = APP_JS.replace("\r\n", "\n");
        let start = page.find("function openEditor(").expect("the writing desk");
        let body = &page[start..];
        let body = &body[..body.find("\n  }\n").expect("the end of openEditor")];
        // The desk's own top level is the Promise body, six spaces in.
        for line in body.lines().filter(|line| line.contains("hubPlaced.")) {
            let indent = line.len() - line.trim_start().len();
            if indent != 6 {
                continue;
            }
            let code = line.trim_start();
            assert!(
                code.starts_with("if (hub) ") || code.contains("=> "),
                "a desk statement reads the hub placement without the hub guard: {code}"
            );
        }
        assert!(
            !page.contains("      if (hubPlaced.at) fillDestinations();"),
            "the unguarded destination fill is gone"
        );
        assert!(page.contains(
            "      if (hub) {\n        hubLevelPick.value = hubPlaced.level;\n        fillDestinations(!hubPlaced.at);\n      }"
        ));
    }

    /// S24 (#120, S24-AC05 check 2; CONTEXT "Spread"): a Spread kept from a question shows that
    /// whole question on the Table, not only its cut display name. The question lies on the Table
    /// plane as a slip at the head of the cards — above their top row, so it covers no card and
    /// moves with them — painted faint and without card stock so the held Card keeps the eye; a long
    /// question folds to three lines and the slip unfolds it. A proposal names its question in the
    /// ask bar, so the slip stands down while one lies on the Table, and fitting the view takes the
    /// slip in with the cards.
    #[test]
    fn a_spread_kept_from_a_question_shows_the_whole_question_on_the_table() {
        let plane = INDEX_HTML.find("id=\"table-plane\"").expect("the Table plane");
        let slip = INDEX_HTML.find("id=\"spread-question\"").expect("the question slip");
        let cards = INDEX_HTML.find("id=\"tcards\"").expect("the cards");
        assert!(plane < slip && slip < cards, "the slip lies on the Table plane, under the cards");
        assert!(INDEX_HTML.contains("id=\"spread-question-text\""));
        assert!(APP_JS.contains(
            "const question = !spreads.previewing() && cards.length && spread && spread.question ? spread.question : '';"
        ));
        assert!(
            APP_JS.contains("$('#spread-question-text').textContent = question;"),
            "the slip carries the whole question, never the cut name"
        );
        assert!(APP_JS.contains("slip.title = question;"));
        assert!(
            APP_JS.contains("slip.style.top = (Math.min(...cards.map((card) => card.y)) - QUESTION_GAP) + 'px';"),
            "the slip sits above the top row of cards"
        );
        let render = APP_JS.find("      paintQuestion(cards);").expect("the Table paints the slip");
        let fit = APP_JS[render..].find("if (fresh) fit(false);").expect("the first fit");
        assert!(fit > 0, "the slip is painted before the view is fitted around it");
        assert!(APP_JS.contains("const asked = questionRoom();"));
        assert!(APP_JS.contains("b.y -= asked;") && APP_JS.contains("b.h += asked;"));
        // A heading, not a card: faint ink, no stock or shadow, no blur, eased motion.
        let start = APP_CSS.find(".spread-question {").expect("the slip rule");
        let block = &APP_CSS[start..start + APP_CSS[start..].find("\n}").expect("rule end")];
        assert!(block.contains("transform: translateY(-100%);"), "it grows upward: {block}");
        assert!(block.contains("background: none;") && block.contains("color: var(--ink-soft);"));
        assert!(!block.contains("box-shadow") && !block.contains("blur("), "no lift or blur: {block}");
        assert!(block.contains("cubic-bezier(.45, 0, .25, 1)"), "it moves with ease-in-out: {block}");
        assert!(APP_CSS.contains(".spread-question .sq-text {") && APP_CSS.contains("-webkit-line-clamp: 3;"));
        assert!(APP_CSS.contains(".spread-question.open .sq-text { display: block; -webkit-line-clamp: unset; }"));
        // The Map's classes stay its own.
        assert!(!INDEX_HTML.contains("class=\"slip") && !INDEX_HTML.contains("class=\"fold"));
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

    /// A book with one Term list and its published head `均衡`, whose list row carries the short
    /// meaning a quiet Term link offers on hover or focus (S13, #156).
    fn term_snapshot() -> (tempfile::TempDir, Snapshot, String) {
        term_snapshot_with_meaning("双方都不改变策略")
    }

    /// The same book, with the exact `意思` cell the reader's hover text comes from.
    fn term_snapshot_with_meaning(meaning: &str) -> (tempfile::TempDir, Snapshot, String) {
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
            "# The first chapter\n\nbody\n",
        )
        .unwrap();
        let table = format!(
            "# 术语表：测试\n\n## 术语表\n\n| 术语 | 不用 | 意思 | 首次定义 |\n| --- | --- | --- | --- |\n| 均衡 | 平衡 | {meaning} | §1.1 |\n"
        );
        let (list, _) = ops::create_successor_with_term(
            root,
            Access::Owner,
            Successor::Side,
            "0/1",
            CardType::Term,
            None,
            None,
            &table,
        )
        .unwrap();
        let (head, _) = ops::create_successor_with_term(
            root,
            Access::Owner,
            Successor::Side,
            &list,
            CardType::Term,
            None,
            Some("均衡"),
            "# 均衡\n\n## 释义\n\n两个参与者都不愿单方面改变策略。\n",
        )
        .unwrap();
        let snapshot = Snapshot::read(root).expect("snapshot");
        (temp, snapshot, head)
    }

    /// S13 (#156, US047/US048, S13-AC01/AC02): a Term use renders only its authored label in the
    /// prose font, offers the list row's short meaning and the target address, and navigates
    /// through the existing encoded Card route instead of the ordinary address/title chip.
    #[test]
    fn a_term_use_renders_only_its_label_with_the_list_row_meaning_and_encoded_route() {
        let (_temp, snapshot, head) = term_snapshot();
        let html = render_markdown(
            &format!("均衡 [[均衡@{head}]] 决定价格。"),
            &snapshot,
            &Charts::default(),
        );
        let route = head.replace('|', "%7C");
        assert!(
            html.contains(&format!(
                "<a class=\"zt-link zt-term\" href=\"#/card/{route}\" data-target=\"{head}\" data-meaning=\"双方都不改变策略\" title=\"双方都不改变策略 · {head}\">均衡</a>"
            )),
            "a Term link is its label, its list-row meaning, and its encoded route: {html}"
        );
        assert!(
            !html.contains("zt-link-addr"),
            "a Term link shows no address chip: {html}"
        );
    }

    /// S13 (#156, US048, S13-AC03): a broken Term use keeps its authored word, says in its own
    /// text why it cannot be followed (no hover needed), and offers no route; an ordinary Broken
    /// Link keeps the chip it always had.
    #[test]
    fn a_broken_term_use_keeps_its_label_and_explains_why_it_cannot_be_followed() {
        let (_temp, snapshot, _head) = term_snapshot();
        let missing = render_markdown("幽灵 [[幽灵@0/9]] 不决定价格。", &snapshot, &Charts::default());
        assert!(
            missing.contains(
                "<span class=\"zt-link zt-term broken\" data-target=\"0/9\" data-reason=\"missing\" title=\"Broken Term link: 0/9 (missing)\">幽灵 <span class=\"zt-term-note\">missing</span></span>"
            ),
            "a missing Term target is explained without hover: {missing}"
        );
        assert!(!missing.contains("#/card/"), "a broken Term use has no route: {missing}");

        let not_head = render_markdown("均衡 [[均衡@0/1]] 不决定价格。", &snapshot, &Charts::default());
        assert!(
            not_head.contains(
                "<span class=\"zt-link zt-term broken\" data-target=\"0/1\" data-reason=\"not_head\" title=\"Broken Term link: 0/1 (not a Term head)\">均衡 <span class=\"zt-term-note\">not a Term head</span></span>"
            ),
            "an existing Card that is no Term head is explained in its own words: {not_head}"
        );

        let ordinary = render_markdown("见 [[Gone]]。", &snapshot, &Charts::default());
        assert!(
            ordinary.contains("<span class=\"zt-link broken\" title=\"Broken link\">"),
            "an ordinary Broken Link keeps its chip: {ordinary}"
        );
        assert!(!ordinary.contains("zt-term"), "{ordinary}");
    }

    /// S13 (#156, S13-AC04/R0470): a term label, a list-row meaning, a target and every attribute
    /// value are data. Markup in any of them is escaped, a forged target cannot open a second
    /// attribute or a route, and the Card route is URL-encoded separately from the attributes.
    #[test]
    fn term_labels_meanings_targets_and_attributes_are_escaped_and_the_route_is_encoded() {
        let (_temp, snapshot, head) =
            term_snapshot_with_meaning("双方都\"不\"改变策略 & <b>均衡</b>");
        let hostile = "<img src=x onerror=alert(1)>均衡";
        let html = render_markdown(&format!("[[{hostile}@{head}]]"), &snapshot, &Charts::default());
        assert!(
            html.contains(&format!("href=\"#/card/{}\"", head.replace('|', "%7C"))),
            "the route is URL-encoded and names the target: {html}"
        );
        assert!(
            html.contains(&format!("data-target=\"{head}\"")),
            "the attribute keeps the authored address: {html}"
        );
        assert!(
            html.contains(&format!(
                ">{}</a>",
                escape_html(hostile)
            )),
            "a term label is text, never HTML: {html}"
        );
        assert!(
            !html.contains("<img") && html.contains("&lt;img src=x onerror=alert(1)&gt;"),
            "a hostile label runs nothing: {html}"
        );
        assert!(
            html.contains(
                "data-meaning=\"双方都&quot;不&quot;改变策略 &amp; &lt;b&gt;均衡&lt;/b&gt;\""
            ),
            "the list-row meaning is escaped: {html}"
        );
        assert!(
            html.contains(&format!(
                "title=\"双方都&quot;不&quot;改变策略 &amp; &lt;b&gt;均衡&lt;/b&gt; · {head}\""
            )),
            "the hover and focus title is escaped: {html}"
        );
        assert!(!html.contains("<b>均衡</b>"), "{html}");

        let forged = render_markdown(
            "[[均衡@0/1\" onmouseover=\"alert(1)]]",
            &snapshot,
            &Charts::default(),
        );
        assert!(
            forged.contains("data-target=\"0/1&quot; onmouseover=&quot;alert(1)\""),
            "a forged target stays inside its attribute: {forged}"
        );
        assert!(
            !forged.contains(" onmouseover=\"alert(1)\"") && !forged.contains("<script>"),
            "no attribute or element is injected: {forged}"
        );
        assert!(forged.contains("data-reason=\"missing\""), "{forged}");
        assert!(!forged.contains("#/card/"), "an unusable target has no route: {forged}");
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

    /// A Link to a Model part names the part by its facet, the label a reader understands, and
    /// keeps the address in the tooltip; a Link to any other Card keeps its address chip.
    #[test]
    fn a_link_to_a_model_part_shows_its_facet_and_keeps_the_address_as_a_tooltip() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path();
        crate::initialize_database(root).expect("initialize database");
        ops::create_topic(root, Access::Owner, Workspace::Human, "# Firms\n").unwrap();
        let (head, _) = ops::create_successor(
            root,
            Access::Owner,
            Successor::Direct,
            "0/0",
            CardType::Model,
            None,
            "# 一个模型\n\n问题：工厂选多少职业？\n",
        )
        .unwrap();
        let part = crate::model::Part {
            facet: crate::model::Facet::parse("推导：利润 (13)").unwrap(),
            emphasis: None,
        };
        let (setting, _) = ops::create_successor(
            root,
            Access::Owner,
            Successor::Side,
            &head,
            CardType::Model,
            Some(part),
            "# 推导：把 (12) 代入 (11) 得利润 (13)\n\n利润。\n",
        )
        .unwrap();
        let snapshot = Snapshot::read(root).expect("snapshot");
        let html = render_markdown(
            &format!("依赖 [[{setting}]] 的结果；参照 [[0/0]]。"),
            &snapshot,
            &Charts::default(),
        );
        assert!(
            html.contains(&format!(
                "data-target=\"{setting}\" title=\"{setting}\"><span class=\"zt-link-addr zt-link-facet\">推导：利润 (13)</span>"
            )),
            "a Model part's chip shows its facet and keeps the address as the tooltip: {html}"
        );
        assert!(
            html.contains("<span class=\"zt-link-addr\">0/0</span>"),
            "another Card keeps its address chip: {html}"
        );
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

    /// S10 (#151): a chart names its Cards by the shared lexical scan, so an ordinary Link and a
    /// Term use both name the target address, while a name in a code span is no Link.
    #[test]
    fn a_chart_names_ordinary_and_term_links_by_their_targets() {
        let (_temp, snapshot) = snapshot();
        let page = chart_page("<p>[[0/1]] [[均衡@0/1]] `[[0/9]]` [[0/8]]</p>", &snapshot);
        let links = page
            .split("window.ZT_LINKS = ")
            .nth(1)
            .and_then(|rest| rest.split(';').next())
            .expect("the chart's links map");
        let links: Value = serde_json::from_str(links).expect("the links map is JSON");
        assert_eq!(links["0/1"]["exists"], json!(true));
        assert_eq!(links["0/1"]["title"], json!("Ownership matters"));
        assert_eq!(links["0/8"]["exists"], json!(false));
        assert!(links.get("0/9").is_none(), "a code span is no Link: {links}");
        assert!(
            links.get("均衡@0/1").is_none(),
            "a Term use names the address, not the macro: {links}"
        );
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
