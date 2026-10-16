use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod artifact;
mod autostart;
mod card;
mod card_type;
mod dump;
mod gui;
mod hub;
mod link;
mod literature;
mod math;
mod mcp;
mod model;
mod ops;
mod place;
mod scholar;
mod session;
mod shutdown;
mod spread;
mod workspace;

pub(crate) use card::{
    ParsedCard, compose_card_text, parse_card_text, parse_literature_edit_text, regular_template,
    topic_template,
};
pub(crate) use card_type::CardType;
pub(crate) use workspace::{Access, Workspace};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [] => {
            let root = require_service_up()?;
            session::run_session(root)
        }
        [cmd] if cmd == "version" => {
            println!("zt {VERSION}");
            Ok(())
        }
        [cmd] if cmd == "help" => cmd_help(),
        [cmd] if cmd == "up" => cmd_up(),
        [cmd] if cmd == "down" => cmd_down(),
        [cmd] if cmd == "status" => cmd_status(),
        [cmd] if cmd == "stats" => cmd_stats(),
        [cmd] if cmd == "lsbk" => cmd_lsbk(),
        [cmd] if cmd == "dp" => cmd_dump(),
        [cmd] if cmd == "clear" => cmd_clear(),
        [cmd] if cmd == "ws" => cmd_workspaces(),
        [cmd] if cmd == "mcp" => mcp::run(),
        [cmd, rest @ ..] if cmd == "scholar" => scholar::run_cli(rest),
        [cmd, rest @ ..] if cmd == "gui" => gui::run(rest),
        [cmd, rest @ ..] if cmd == "autostart" => autostart::run(rest),
        [cmd] if cmd == "__boot" => cmd_boot(),
        [cmd, root] if cmd == "__daemon" => cmd_daemon(PathBuf::from(root)),
        [cmd, sub] if cmd == "config" && sub == "show" => cmd_config_show(),
        [cmd, sub, key, value] if cmd == "config" && sub == "set" && key == "archive_root" => {
            cmd_config_set_archive_root(value)
        }
        [cmd, rest @ ..] if cmd == "t" => {
            let (workspace, title) = parse_workspace_option(rest)?;
            let title = title.join(" ");
            shell_create_topic(workspace, &title)
        }
        [cmd, rest @ ..] if cmd == "l" => {
            let (workspace, rest) = parse_workspace_option(rest)?;
            if !rest.is_empty() {
                bail!("unknown command");
            }
            shell_create_literature(workspace)
        }
        [cmd, rest @ ..] if cmd == "n" => {
            let at = parse_target_at(rest)?;
            shell_create_direct(&at)
        }
        [cmd, rest @ ..] if cmd == "b" => {
            let at = parse_at(rest)?;
            shell_create_side(&at)
        }
        [cmd, rest @ ..] if cmd == "e" => {
            let (at, part) = parse_edit_args(rest)?;
            shell_edit(&at, part)
        }
        [cmd, rest @ ..] if cmd == "del" => {
            let at = parse_target_at(rest)?;
            shell_delete(&at)
        }
        [cmd, rest @ ..] if cmd == "mv" => {
            let (at, target, alone) = parse_mv_args(rest)?;
            shell_move(&at, &target, alone)
        }
        [cmd, rest @ ..] if cmd == "ins" => {
            let at = parse_at(rest)?;
            shell_insert(&at)
        }
        _ => bail!("unknown command"),
    }
}

/// An optional leading `--ws <workspace>`; new trees go to the human
/// workspace unless another is named.
fn parse_workspace_option(args: &[String]) -> Result<(Workspace, &[String])> {
    match args {
        [flag, value, rest @ ..] if flag == "--ws" => Ok((Workspace::parse(value)?, rest)),
        [flag] if flag == "--ws" => bail!("expected --ws <workspace>"),
        rest => Ok((Workspace::Human, rest)),
    }
}

fn parse_at(args: &[String]) -> Result<String> {
    match args {
        [flag, value] if flag == "--at" => Ok(value.clone()),
        _ => bail!("expected --at <location>"),
    }
}

fn parse_target_at(args: &[String]) -> Result<String> {
    match args {
        [flag, value] if flag == "--at" => Ok(value.clone()),
        _ => bail!("expected --at <target>"),
    }
}

/// `--at <location>`, an optional `--alone`, then `<new-location>` or
/// `--before <location>`.
fn parse_mv_args(args: &[String]) -> Result<(String, place::Target, bool)> {
    let usage = "expected --at <location> [--alone] <new-location> or --at <location> [--alone] --before <location>";
    let [flag, at, rest @ ..] = args else {
        bail!(usage);
    };
    if flag != "--at" {
        bail!(usage);
    }
    let (alone, rest) = match rest {
        [flag, rest @ ..] if flag == "--alone" => (true, rest),
        rest => (false, rest),
    };
    let target = match rest {
        [flag, before] if flag == "--before" => place::Target::Before(before.clone()),
        [new_location] if !new_location.starts_with("--") => {
            place::Target::To(new_location.clone())
        }
        _ => bail!(usage),
    };
    Ok((at.clone(), target, alone))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EditPart {
    Metadata,
    Text,
}

fn parse_edit_args(args: &[String]) -> Result<(String, Option<EditPart>)> {
    let mut target = None;
    let mut part = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--at" if target.is_none() && index + 1 < args.len() => {
                target = Some(args[index + 1].clone());
                index += 2;
            }
            "--part" if part.is_none() && index + 1 < args.len() => {
                part = Some(match args[index + 1].as_str() {
                    "metadata" => EditPart::Metadata,
                    "text" => EditPart::Text,
                    value => bail!("invalid --part `{value}`; expected metadata or text"),
                });
                index += 2;
            }
            "--at" => bail!("duplicate or incomplete --at option"),
            "--part" => bail!("duplicate or incomplete --part option"),
            value => bail!("unexpected edit argument `{value}`"),
        }
    }
    Ok((target.context("expected --at <target>")?, part))
}

#[derive(Debug, Default)]
struct Config {
    archive_root: Option<PathBuf>,
}

impl Config {
    fn load() -> Result<Self> {
        let path = config_file()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config {}", path.display()))?;
        let mut config = Config::default();
        for line in raw.lines() {
            if let Some(value) = line.strip_prefix("archive_root=") {
                config.archive_root = Some(PathBuf::from(value));
            }
        }
        Ok(config)
    }

    fn save(&self) -> Result<()> {
        let path = config_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        // Only the archive_root line changes; every other line of the file stays as it was.
        let root_line = self
            .archive_root
            .as_ref()
            .map(|root| format!("archive_root={}\n", root.to_string_lossy()));
        let mut body = String::new();
        let mut placed = false;
        for line in fs::read_to_string(&path).unwrap_or_default().lines() {
            if line.starts_with("archive_root=") {
                if !placed {
                    body.push_str(root_line.as_deref().unwrap_or_default());
                    placed = true;
                }
                continue;
            }
            body.push_str(line);
            body.push('\n');
        }
        if !placed {
            body.push_str(root_line.as_deref().unwrap_or_default());
        }
        fs::write(&path, body).with_context(|| format!("failed to write config {}", path.display()))
    }
}

fn config_file() -> Result<PathBuf> {
    if let Some(dir) = env::var_os("ZT_CONFIG_DIR") {
        return Ok(PathBuf::from(dir).join("config.toml"));
    }
    #[cfg(windows)]
    {
        let appdata = env::var_os("APPDATA").context("APPDATA is not set")?;
        Ok(PathBuf::from(appdata).join("zt").join("config.toml"))
    }
    #[cfg(not(windows))]
    {
        let home = env::var_os("HOME").context("HOME is not set")?;
        Ok(PathBuf::from(home)
            .join(".config")
            .join("zt")
            .join("config.toml"))
    }
}

pub(crate) fn require_archive_root() -> Result<PathBuf> {
    Config::load()?
        .archive_root
        .context("archive_root is not configured; run `zt config set archive_root <path>`")
}

fn require_service_up() -> Result<PathBuf> {
    let root = require_archive_root()?;
    if !is_service_up(&root) {
        bail!("service is not up");
    }
    ensure_markdown_cards(&root)?;
    Ok(root)
}

fn db_path(root: &Path) -> PathBuf {
    root.join("zt.sqlite3")
}

fn log_path(root: &Path) -> PathBuf {
    root.join("zt.log")
}

fn service_path(root: &Path) -> PathBuf {
    root.join("zt.pid")
}

fn sessions_path(root: &Path) -> PathBuf {
    root.join("zt.sessions")
}

fn sessions_lock_path(root: &Path) -> PathBuf {
    root.join("zt.sessions.lock")
}

fn edit_lock_path(root: &Path) -> PathBuf {
    root.join("zt.edit.lock")
}

fn edit_guard_path(root: &Path) -> PathBuf {
    root.join("zt.edit.guard")
}

fn log_event(root: &Path, event: &str) -> Result<()> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(root))
        .with_context(|| format!("failed to open {}", log_path(root).display()))?;
    writeln!(file, "{seconds} {event}")?;
    Ok(())
}

fn daemon_lock_path(root: &Path) -> PathBuf {
    root.join("zt.daemon.lock")
}

/// Whether the service is up: its record names a daemon that still holds the daemon lock. A
/// daemon that ended without `zt down` (the machine lost power, or the daemon was killed) left
/// its record behind; the check clears it and starts the service again, so the service only
/// stays down after `zt down` or when the machine ends the session.
fn is_service_up(root: &Path) -> bool {
    if !service_path(root).exists() {
        return restart_in_progress(root) && wait_for_service(root);
    }
    if daemon_alive(root) != Some(false) {
        return true;
    }
    // The first process to find the dead daemon restarts the service; the others wait for it.
    let Some(_marker) = RestartMarker::claim(root) else {
        return wait_for_service(root);
    };
    // Another process may have restarted the service before this one claimed the restart.
    if !service_path(root).exists() {
        return false;
    }
    if daemon_alive(root) != Some(false) {
        return true;
    }
    match clear_stale_service(root) {
        Some(pid) => restart_service(root, &pid),
        None => wait_for_service(root),
    }
}

fn restart_marker_path(root: &Path) -> PathBuf {
    root.join("zt.restart")
}

/// A restart that started this long ago was abandoned (its process died): ignore its marker.
const RESTART_TIMEOUT: Duration = Duration::from_secs(30);

/// Whether a process is starting the service again after its daemon ended without `zt down`.
fn restart_in_progress(root: &Path) -> bool {
    fs::metadata(restart_marker_path(root))
        .and_then(|meta| meta.modified())
        .is_ok_and(|made| made.elapsed().unwrap_or_default() < RESTART_TIMEOUT)
}

/// The claim on a restart: made atomically, so one process restarts the service, and removed
/// when the restart is over, however it went.
struct RestartMarker(PathBuf);

impl RestartMarker {
    fn claim(root: &Path) -> Option<Self> {
        let path = restart_marker_path(root);
        if fs::metadata(&path).is_ok() && !restart_in_progress(root) {
            let _ = fs::remove_file(&path);
        }
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .ok()?;
        Some(Self(path))
    }
}

impl Drop for RestartMarker {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Whether the service is up, clearing a record left by a daemon that ended without `zt down`
/// but not starting the service again: for `zt up`, `zt down` and `zt config set`.
fn is_service_recorded_up(root: &Path) -> bool {
    if !service_path(root).exists() {
        return false;
    }
    if daemon_alive(root) != Some(false) {
        return true;
    }
    clear_stale_service(root);
    false
}

/// Starts the service again after daemon `pid` ended without `zt down`, and logs the outcome.
fn restart_service(root: &Path, pid: &str) -> bool {
    match start_service_at(root) {
        Ok(_) => {
            log_event(
                root,
                &format!("service restarted: daemon {pid} ended without `zt down`"),
            )
            .ok();
            true
        }
        Err(error) => {
            log_event(root, &format!("service did not restart: {error:#}")).ok();
            false
        }
    }
}

/// Waits for the restart another process is making, until its daemon is up or it is over.
fn wait_for_service(root: &Path) -> bool {
    let deadline = Instant::now() + RESTART_TIMEOUT;
    loop {
        let recorded = service_path(root).exists();
        if recorded && daemon_alive(root) != Some(false) {
            return true;
        }
        if !restart_in_progress(root) || Instant::now() >= deadline {
            return recorded && daemon_alive(root) != Some(false);
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Whether a daemon holds the daemon lock; `None` when no daemon has made one (a daemon from
/// zt 0.7 or earlier) or the lock cannot be tested.
fn daemon_alive(root: &Path) -> Option<bool> {
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(daemon_lock_path(root))
        .ok()?;
    match file.try_lock() {
        Ok(()) => {
            let _ = file.unlock();
            Some(false)
        }
        Err(fs::TryLockError::WouldBlock) => Some(true),
        Err(fs::TryLockError::Error(_)) => None,
    }
}

/// Clears the record of a daemon that ended without `zt down`; returns its pid when this
/// process removed the record, so only one of several processes that find it acts on it.
fn clear_stale_service(root: &Path) -> Option<String> {
    let pid = fs::read_to_string(service_path(root)).ok()?;
    let pid = pid.trim().to_string();
    gui::clear_record(root);
    fs::remove_file(service_path(root)).ok()?;
    log_event(
        root,
        &format!("service record of daemon {pid} cleared: the daemon ended without `zt down`"),
    )
    .ok();
    Some(pid)
}

/// A record from a daemon of zt 0.7 or earlier, which holds no daemon lock, is stale when no
/// process has its pid.
fn clear_stale_legacy_service(root: &Path) {
    if daemon_alive(root).is_some() {
        return;
    }
    let Ok(pid) = fs::read_to_string(service_path(root)) else {
        return;
    };
    if pid
        .trim()
        .parse::<u32>()
        .is_ok_and(|pid| !session::process_exists(pid))
    {
        clear_stale_service(root);
    }
}

fn cmd_config_show() -> Result<()> {
    let config = Config::load()?;
    match config.archive_root {
        Some(root) => println!("archive_root={}", root.display()),
        None => println!("archive_root=<unset>"),
    }
    Ok(())
}

fn cmd_config_set_archive_root(value: &str) -> Result<()> {
    let new_root = PathBuf::from(value);
    if let Ok(current_root) = require_archive_root()
        && is_service_recorded_up(&current_root)
    {
        bail!("cannot change archive_root while service is up");
    }
    let config = Config {
        archive_root: Some(new_root),
    };
    config.save()?;
    println!("archive_root={value}");
    Ok(())
}

fn cmd_up() -> Result<()> {
    println!("{}", start_service()?);
    print_gui_line()?;
    Ok(())
}

/// Where the service's GUI listens, or why it has none.
fn print_gui_line() -> Result<()> {
    let root = require_archive_root()?;
    match gui::recorded_url(&root) {
        Some(url) => println!("gui: {url}"),
        None => println!("gui: not running (see {})", log_path(&root).display()),
    }
    Ok(())
}

/// What `zt-boot` runs at sign-in: bring the service up and record how it went, since there
/// is no console to tell.
fn cmd_boot() -> Result<()> {
    let outcome = start_service();
    let line = match &outcome {
        Ok(message) => message.clone(),
        Err(error) => format!("error: {error:#}"),
    };
    if outcome.is_ok()
        && let Ok(root) = require_archive_root()
    {
        log_event(&root, &format!("login start: {line}")).ok();
    }
    autostart::record_boot(&line)?;
    outcome.map(|_| ())
}

fn start_service() -> Result<String> {
    let root = require_archive_root()?;
    // A second daemon would only wait for the daemon lock behind the one being restarted.
    if restart_in_progress(&root) {
        wait_for_service(&root);
    }
    start_service_at(&root)
}

fn start_service_at(root: &Path) -> Result<String> {
    fs::create_dir_all(root).with_context(|| format!("failed to create {}", root.display()))?;
    clear_stale_legacy_service(root);
    if is_service_recorded_up(root) {
        let pid = fs::read_to_string(service_path(root)).unwrap_or_else(|_| "unknown".to_string());
        return Ok(format!("service already running pid: {}", pid.trim()));
    }
    initialize_database(root)?;
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path(root))
        .with_context(|| format!("failed to open {}", log_path(root).display()))?;
    let child_id = spawn_daemon(root)?;
    // Generous, for a machine still busy signing in; a daemon that starts promptly ends the
    // wait at once.
    let deadline = Instant::now() + Duration::from_secs(15);
    while !service_path(root).exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    if !service_path(root).exists() {
        bail!("daemon did not start");
    }
    let pid = fs::read_to_string(service_path(root)).unwrap_or_else(|_| {
        child_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    });
    Ok(format!("service started pid: {}", pid.trim()))
}

#[cfg(windows)]
fn spawn_daemon(root: &Path) -> Result<Option<u32>> {
    let exe = env::current_exe()?;
    let script = format!(
        "Start-Process -WindowStyle Hidden -FilePath {} -ArgumentList @('__daemon', {})",
        powershell_quote(&exe),
        powershell_quote(root)
    );
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to launch daemon through PowerShell")?;
    if !status.success() {
        bail!("failed to launch daemon");
    }
    Ok(None)
}

#[cfg(windows)]
fn powershell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "''"))
}

#[cfg(not(windows))]
fn spawn_daemon(root: &Path) -> Result<Option<u32>> {
    let child = Command::new(env::current_exe()?)
        .arg("__daemon")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start daemon")?;
    Ok(Some(child.id()))
}

fn cmd_daemon(root: PathBuf) -> Result<()> {
    fs::create_dir_all(&root).with_context(|| format!("failed to create {}", root.display()))?;
    // Held for the daemon's whole life and released by the system however the daemon ends,
    // so a record whose daemon is gone reads as down. A daemon that is still stopping holds it
    // a moment longer; wait for it.
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(daemon_lock_path(&root))
        .with_context(|| format!("failed to open {}", daemon_lock_path(&root).display()))?;
    lock.lock()
        .with_context(|| format!("failed to lock {}", daemon_lock_path(&root).display()))?;
    log_event(&root, "daemon started")?;
    // The GUI lives as long as the daemon. It is up before the service record, so a service
    // that reads as up already shows where its GUI listens.
    let gui = gui::start(&root)
        .inspect_err(|error| {
            gui::clear_record(&root);
            log_event(&root, &format!("gui did not start: {error:#}")).ok();
        })
        .ok();
    let pid = std::process::id().to_string();
    fs::write(service_path(&root), &pid)
        .with_context(|| format!("failed to write {}", service_path(&root).display()))?;
    // Create the file if it is missing, without truncating one a Session wrote meanwhile.
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(sessions_path(&root))
        .ok();
    let ended = shutdown::watch(
        {
            let (root, pid) = (root.clone(), pid.clone());
            move || owns_service(&root, &pid)
        },
        {
            let (root, pid) = (root.clone(), pid.clone());
            move |end| stop_for_system(&root, &pid, end)
        },
    );
    if let Some(gui) = gui {
        gui.stop();
    }
    if ended.is_none() {
        log_event(&root, "daemon stopped")?;
    }
    drop(lock);
    Ok(())
}

fn owns_service(root: &Path, pid: &str) -> bool {
    matches!(fs::read_to_string(service_path(root)), Ok(current) if current.trim() == pid)
}

/// The machine is ending the user's session: stop the service as `zt down` does, without
/// waiting for open Sessions, which the system is ending too.
fn stop_for_system(root: &Path, pid: &str, end: shutdown::SystemEnd) {
    if owns_service(root, pid) {
        gui::clear_record(root);
        let _ = fs::remove_file(service_path(root));
    }
    let _ = log_event(root, &format!("service stopped: {}", end.describe()));
}

fn initialize_database(root: &Path) -> Result<()> {
    let conn = Connection::open(db_path(root))
        .with_context(|| format!("failed to open {}", db_path(root).display()))?;
    let integrity: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .context("failed to run SQLite integrity check")?;
    if integrity != "ok" {
        bail!("SQLite database is corrupted: {integrity}");
    }
    let cards_exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'cards')",
        [],
        |row| row.get(0),
    )?;
    if cards_exists {
        let mut stmt = conn.prepare("PRAGMA table_info(cards)")?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if columns == ["location", "is_topic", "text"] {
            bail!(
                "legacy cards schema detected; back up and manually migrate the database before starting this ZT version"
            );
        }
        let before_workspaces = [
            "row_id",
            "location",
            "citation_key",
            "is_topic",
            "is_lit",
            "bibtex",
            "text",
        ];
        let before_card_types = [&before_workspaces[..], &["workspace"]].concat();
        let before_facets = [&before_card_types[..], &["card_type"]].concat();
        let before_emphasis = [&before_facets[..], &["model_facet"]].concat();
        let before_level = [&before_emphasis[..], &["model_emphasis"]].concat();
        let before_hubs = [&before_level[..], &["model_level"]].concat();
        let before_math = [&before_hubs[..], &["hub_level"]].concat();
        let current = [&before_math[..], &["math_form"]].concat();
        if columns != before_workspaces
            && columns != before_card_types
            && columns != before_facets
            && columns != before_emphasis
            && columns != before_level
            && columns != before_hubs
            && columns != before_math
            && columns != current
        {
            bail!("unsupported cards schema; database was not modified");
        }
    }
    conn.execute_batch(&format!(
        "
        {}
        CREATE UNIQUE INDEX IF NOT EXISTS cards_citation_key_nocase
            ON cards(citation_key COLLATE NOCASE)
            WHERE citation_key IS NOT NULL;
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        ",
        cards_table("IF NOT EXISTS cards")
    ))?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    ensure_workspace_column(&conn, root)?;
    ensure_card_type_column(&conn, root)?;
    ensure_model_facet_column(&conn, root)?;
    ensure_open_model_parts(&conn, root)?;
    ensure_model_level_column(&conn, root)?;
    ensure_hub_cards(&conn, root)?;
    ensure_math_form_column(&conn, root)?;
    ensure_finding_cards(&conn, root)?;
    drop(conn);
    ensure_markdown_cards(root)
}

/// `CREATE TABLE <name> (…)` for the current cards schema. A Model facet is a
/// short label of the model's author; only a Model part has an emphasis, only
/// a Model Card (no facet) has an elaboration level or a Math form, and only a
/// Hub Card has a hub level.
fn cards_table(name: &str) -> String {
    format!(
        "CREATE TABLE {name} (
            row_id INTEGER PRIMARY KEY,
            location TEXT UNIQUE,
            citation_key TEXT UNIQUE,
            is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
            is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
            bibtex TEXT,
            text TEXT NOT NULL,
            workspace TEXT NOT NULL DEFAULT 'human' CHECK(workspace IN ('human', 'agent')),
            card_type TEXT NOT NULL DEFAULT 'idea' CHECK(card_type IN ('idea', 'event', 'finding', 'model', 'hub')),
            model_facet TEXT CHECK(model_facet IS NULL OR length(model_facet) BETWEEN 1 AND {}),
            model_emphasis TEXT CHECK(model_emphasis IS NULL OR model_emphasis IN ('core', 'aside')),
            model_level TEXT CHECK(model_level IS NULL OR model_level IN ('concept', 'formal')),
            hub_level TEXT CHECK(hub_level IS NULL OR hub_level IN ('line', 'topic', 'field', 'register')),
            math_form TEXT CHECK(math_form IS NULL OR math_form IN ({})),
            CHECK(
                (is_lit = 0 AND location IS NOT NULL AND citation_key IS NULL AND bibtex IS NULL)
                OR
                (is_lit = 1 AND is_topic = 0 AND location IS NULL AND citation_key IS NOT NULL AND bibtex IS NOT NULL)
            ),
            CHECK(model_emphasis IS NULL OR model_facet IS NOT NULL),
            CHECK(model_level IS NULL OR (card_type = 'model' AND model_facet IS NULL)),
            CHECK(hub_level IS NULL OR card_type = 'hub'),
            CHECK(math_form IS NULL OR (card_type = 'model' AND model_facet IS NULL))
        );",
        model::Facet::MAX_CHARS,
        math_form_values()
    )
}

/// The Math forms as an SQL list: `'theory', 'definition', …`.
fn math_form_values() -> String {
    math::MathForm::ALL
        .iter()
        .map(|form| format!("'{}'", form.id()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Archives written before Model Cards could state a mathematical unit: add the
/// `math_form` column with no form on any Model Card. A column added by ALTER
/// TABLE takes only column constraints, so the rule that only a Model Card has
/// a form is kept by `math::set_form` and `model::set`.
fn ensure_math_form_column(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'math_form')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(&format!(
        "ALTER TABLE cards ADD COLUMN math_form TEXT
             CHECK(math_form IS NULL OR math_form IN ({}));",
        math_form_values()
    ))
    .context("failed to add Math forms to the archive")?;
    log_event(root, "added math forms; no model card states a unit yet").ok();
    Ok(())
}

/// Archives created before Workspaces existed hold only the human's Cards:
/// add the `workspace` column with every existing Card in the human workspace.
fn ensure_workspace_column(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'workspace')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(
        "ALTER TABLE cards ADD COLUMN workspace TEXT NOT NULL DEFAULT 'human'
             CHECK(workspace IN ('human', 'agent'));",
    )
    .context("failed to add Workspaces to the archive")?;
    log_event(
        root,
        "added workspaces; existing cards are in the human workspace",
    )
    .ok();
    Ok(())
}

/// Archives created before Card types existed hold only Idea Cards: add the
/// `card_type` column with every existing Card an Idea Card.
fn ensure_card_type_column(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'card_type')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(
        "ALTER TABLE cards ADD COLUMN card_type TEXT NOT NULL DEFAULT 'idea'
             CHECK(card_type IN ('idea', 'event', 'finding', 'model', 'hub'));",
    )
    .context("failed to add Card types to the archive")?;
    log_event(root, "added card types; existing cards are idea cards").ok();
    Ok(())
}

/// Archives created before Model parts existed have no facets: add the
/// `model_facet` column with every existing Model Card a head.
fn ensure_model_facet_column(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'model_facet')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch("ALTER TABLE cards ADD COLUMN model_facet TEXT;")
        .context("failed to add Model facets to the archive")?;
    log_event(
        root,
        "added model facets; existing model cards are model heads",
    )
    .ok();
    Ok(())
}

/// Archives written before a model's author chose its facets allow only the
/// eight common facets and have no emphasis. SQLite cannot drop a CHECK, so
/// the cards table is rebuilt in one transaction after a backup copy; every
/// Card keeps its row id, facet, and text, and no part has an emphasis.
fn ensure_open_model_parts(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'model_emphasis')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    let cards: i64 = conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    if cards > 0 {
        let backup = root.join(format!(
            "zt.sqlite3.pre-open-models-{}.bak",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
            .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
    }
    let columns = "row_id, location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet";
    conn.execute_batch(&format!(
        "BEGIN IMMEDIATE;
         {}
         INSERT INTO cards_open_models ({columns}) SELECT {columns} FROM cards;
         DROP TABLE cards;
         ALTER TABLE cards_open_models RENAME TO cards;
         CREATE UNIQUE INDEX IF NOT EXISTS cards_citation_key_nocase
             ON cards(citation_key COLLATE NOCASE)
             WHERE citation_key IS NOT NULL;
         COMMIT;",
        cards_table("cards_open_models")
    ))
    .inspect_err(|_| {
        let _ = conn.execute_batch("ROLLBACK;");
    })
    .context("failed to let Model parts take any facet")?;
    log_event(
        root,
        "model facets are free labels and model parts may carry an emphasis",
    )
    .ok();
    Ok(())
}

/// Archives written before models had elaboration levels: add the
/// `model_level` column with no level on any Model Card. A column added by
/// ALTER TABLE takes only column constraints, so the rule that only a Model
/// Card has a level is kept by `model::set_level` and `model::set`.
fn ensure_model_level_column(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'model_level')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    conn.execute_batch(
        "ALTER TABLE cards ADD COLUMN model_level TEXT
             CHECK(model_level IS NULL OR model_level IN ('concept', 'formal'));",
    )
    .context("failed to add elaboration levels to the archive")?;
    log_event(
        root,
        "added model elaboration levels; no model has a level yet",
    )
    .ok();
    Ok(())
}

/// Archives written before Hub Cards existed allow only the Idea, Event, and
/// Model types and have no hub level. SQLite cannot change a CHECK, so the
/// cards table is rebuilt in one transaction after a backup copy; every Card
/// keeps its row id, type, facet, emphasis, level, and text, and no Card is a
/// Hub Card.
fn ensure_hub_cards(conn: &Connection, root: &Path) -> Result<()> {
    let has_column: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('cards') WHERE name = 'hub_level')",
        [],
        |row| row.get(0),
    )?;
    if has_column {
        return Ok(());
    }
    ensure_model_level_column(conn, root)?;
    let cards: i64 = conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    if cards > 0 {
        let backup = root.join(format!(
            "zt.sqlite3.pre-hub-cards-{}.bak",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
            .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
    }
    let columns = "row_id, location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level";
    conn.execute_batch(&format!(
        "BEGIN IMMEDIATE;
         {}
         INSERT INTO cards_hubs ({columns}) SELECT {columns} FROM cards;
         DROP TABLE cards;
         ALTER TABLE cards_hubs RENAME TO cards;
         CREATE UNIQUE INDEX IF NOT EXISTS cards_citation_key_nocase
             ON cards(citation_key COLLATE NOCASE)
             WHERE citation_key IS NOT NULL;
         COMMIT;",
        cards_table("cards_hubs")
    ))
    .inspect_err(|_| {
        let _ = conn.execute_batch("ROLLBACK;");
    })
    .context("failed to add Hub Cards to the archive")?;
    log_event(root, "added hub cards; no card is a hub card yet").ok();
    Ok(())
}

/// Archives written before Finding Cards existed allow only the Idea, Event,
/// Model, and Hub types. No column is added, so the stored table definition
/// tells whether `finding` is allowed. SQLite cannot change a CHECK, so the
/// cards table is rebuilt in one transaction after a backup copy; every Card
/// keeps its row id, type, facet, emphasis, levels, form, and text, and no
/// Card is a Finding Card.
fn ensure_finding_cards(conn: &Connection, root: &Path) -> Result<()> {
    let definition: String = conn.query_row(
        "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'cards'",
        [],
        |row| row.get(0),
    )?;
    if definition.contains("'finding'") {
        return Ok(());
    }
    ensure_math_form_column(conn, root)?;
    let cards: i64 = conn.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    if cards > 0 {
        let backup = root.join(format!(
            "zt.sqlite3.pre-finding-cards-{}.bak",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ));
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy()])
            .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
    }
    let columns = "row_id, location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form";
    conn.execute_batch(&format!(
        "BEGIN IMMEDIATE;
         {}
         INSERT INTO cards_findings ({columns}) SELECT {columns} FROM cards;
         DROP TABLE cards;
         ALTER TABLE cards_findings RENAME TO cards;
         CREATE UNIQUE INDEX IF NOT EXISTS cards_citation_key_nocase
             ON cards(citation_key COLLATE NOCASE)
             WHERE citation_key IS NOT NULL;
         COMMIT;",
        cards_table("cards_findings")
    ))
    .inspect_err(|_| {
        let _ = conn.execute_batch("ROLLBACK;");
    })
    .context("failed to add Finding Cards to the archive")?;
    log_event(root, "added finding cards; no card is a finding card yet").ok();
    Ok(())
}

const CARD_FORMAT: &str = "markdown";

/// Convert an archive written before Cards were Markdown documents. The
/// original database is first copied next to it as a backup.
fn ensure_markdown_cards(root: &Path) -> Result<()> {
    use rusqlite::OptionalExtension;
    let mut conn = open_db(root)?;
    ensure_workspace_column(&conn, root)?;
    ensure_card_type_column(&conn, root)?;
    ensure_model_facet_column(&conn, root)?;
    ensure_open_model_parts(&conn, root)?;
    ensure_hub_cards(&conn, root)?;
    let format: Option<String> = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'card_format'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if format.as_deref() == Some(CARD_FORMAT) {
        return Ok(());
    }
    let cards = load_cards(&conn)?;
    let converted = cards
        .iter()
        .filter_map(|card| Some((card.row_id, card::from_legacy_text(&card.text)?)))
        .collect::<Vec<_>>();
    if !converted.is_empty() {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let backup = root.join(format!("zt.sqlite3.pre-markdown-{seconds}.bak"));
        conn.execute("VACUUM INTO ?1", [backup.to_string_lossy().to_string()])
            .with_context(|| format!("failed to back up the archive to {}", backup.display()))?;
        log_event(
            root,
            &format!(
                "converting {} cards to Markdown; backup {}",
                converted.len(),
                backup.display()
            ),
        )
        .ok();
    }
    let tx = conn.transaction()?;
    for (row_id, text) in &converted {
        tx.execute(
            "UPDATE cards SET text = ?1 WHERE row_id = ?2",
            params![text, row_id],
        )?;
    }
    regenerate_reverse_links_tx(&tx).context("failed to convert Cards to Markdown")?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('card_format', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [CARD_FORMAT],
    )?;
    tx.commit()?;
    Ok(())
}

fn cmd_down() -> Result<()> {
    let root = match Config::load()?.archive_root {
        Some(root) => root,
        None => {
            println!("service already stopped");
            return Ok(());
        }
    };
    if !is_service_recorded_up(&root) {
        println!("service already stopped");
        return Ok(());
    }
    let sessions = session::session_count(&root);
    if sessions > 0 {
        bail!("cannot stop service while {sessions} session(s) are open");
    }
    fs::remove_file(service_path(&root))
        .with_context(|| format!("failed to remove {}", service_path(&root).display()))?;
    log_event(&root, "service stop requested").ok();
    println!("service stopped");
    Ok(())
}

fn cmd_status() -> Result<()> {
    let config = Config::load()?;
    let Some(root) = config.archive_root else {
        println!("state: down");
        println!("archive_root: <unset>");
        if let Some(line) = autostart::status_line() {
            println!("{line}");
        }
        return Ok(());
    };
    let up = is_service_up(&root);
    println!("state: {}", if up { "up" } else { "down" });
    if up {
        let pid = fs::read_to_string(service_path(&root)).unwrap_or_else(|_| "unknown".to_string());
        println!("pid: {}", pid.trim());
        if let Some(url) = gui::recorded_url(&root) {
            println!("gui: {url}");
        }
    }
    println!("archive_root: {}", root.display());
    println!("sqlite: {}", db_path(&root).display());
    println!(
        "cards: {}",
        card_counts(&root).map(|c| c.total).unwrap_or(0)
    );
    println!("sessions: {}", session::session_count(&root));
    if let Some(line) = autostart::status_line() {
        println!("{line}");
    }
    Ok(())
}

fn cmd_stats() -> Result<()> {
    let root = require_service_up()?;
    let counts = card_counts(&root)?;
    println!("total: {}", counts.total);
    println!("topics: {}", counts.topics);
    println!("regular: {}", counts.regular);
    println!("literature: {}", counts.literature);
    Ok(())
}

fn cmd_workspaces() -> Result<()> {
    let root = require_service_up()?;
    let conn = open_db(&root)?;
    let cards = load_cards(&conn)?;
    for workspace in Workspace::ALL {
        let counts = workspace::counts(&cards, workspace);
        println!(
            "{} ({}): {} topic trees, {} literature trees, {} cards; MCP access: {}",
            workspace.id(),
            workspace.label(),
            counts.topic_trees,
            counts.literature_trees,
            counts.cards,
            if Access::Agent.can_write(workspace) {
                "read-write"
            } else {
                "read-only"
            }
        );
    }
    Ok(())
}

fn cmd_dump() -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let conn = open_db(&root)?;
    let cards = load_cards(&conn)?
        .into_iter()
        .map(|card| {
            if card.is_lit {
                Ok(dump::DumpCard::literature(
                    card.address,
                    card.workspace,
                    card.bibtex
                        .context("Literature Card is missing its BibTeX metadata")?,
                    card.text,
                ))
            } else {
                Ok(
                    dump::DumpCard::new(card.address, card.workspace, card.card_type, card.text)
                        .with_part(card.facet, card.emphasis)
                        .with_level(card.level)
                        .with_form(card.form)
                        .with_hub_level(card.hub_level),
                )
            }
        })
        .collect::<Result<Vec<_>>>()?;
    dump::run(&root, &cards, next_topic_id(&conn)?)
}

fn cmd_clear() -> Result<()> {
    let root = require_service_up()?;
    let sessions = session::session_count(&root);
    if sessions > 0 {
        bail!("cannot clear while {sessions} session(s) are open");
    }
    let _lock = acquire_edit_lock(&root)?;
    print!("type clear to confirm: ");
    io::stdout().flush()?;
    let mut confirmation = String::new();
    io::stdin().read_line(&mut confirmation)?;
    if confirmation.trim_end_matches(['\r', '\n']) != "clear" {
        println!("clear canceled");
        return Ok(());
    }
    let mut conn = open_db(&root)?;
    let tx = conn.transaction()?;
    let count: i64 = tx.query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))?;
    tx.execute("DELETE FROM cards", [])?;
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('next_topic_id', '0')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    tx.commit()?;
    println!("cleared {count} cards; next topic id reset to 0");
    Ok(())
}

fn cmd_help() -> Result<()> {
    let up = Config::load()?
        .archive_root
        .as_ref()
        .map(|root| is_service_up(root))
        .unwrap_or(false);
    println!("zt commands:");
    println!("  zt up");
    println!("  zt down");
    println!("  zt status");
    println!("  zt autostart [on|off]");
    println!("  zt version");
    println!("  zt help");
    println!("  zt config show");
    println!("  zt config set archive_root <path>");
    println!("  zt scholar [status|pair [--rotate]|unpair|tools]");
    println!("  zt scholar call <tool> [<json-arguments>]");
    println!("  zt scholar sql [--db archive|vectors] [--limit <n>] <sql>");
    println!("  zt scholar file <file-id> [--out <path>]");
    println!("  zt scholar figures <work-id>");
    println!("  zt scholar figure <file-id> <path> [--caption <text>]");
    println!(
        "  zt scholar export [--scholar <id>] [--format json|csv|bibtex] [--view profile|combined] [--out <path>]"
    );
    if up {
        println!("  zt");
        println!("  zt gui [--open]");
        println!("  zt t [--ws <workspace>] <title>");
        println!("  zt l [--ws <workspace>]");
        println!("  zt n --at <target>");
        println!("  zt b --at <location>");
        println!("  zt e --at <location>");
        println!("  zt e --at <citation-key> --part metadata");
        println!("  zt e --at <citation-key> --part text");
        println!("  zt del --at <target>");
        println!("  zt mv --at <location> [--alone] <new-location>");
        println!("  zt mv --at <location> [--alone] --before <location>");
        println!("  zt ins --at <location>");
        println!("  zt stats");
        println!("  zt ws");
        println!("  zt mcp");
        println!("  zt lsbk");
        println!("  zt dp");
        println!("  zt clear");
    }
    Ok(())
}

#[derive(Default)]
struct Counts {
    total: i64,
    topics: i64,
    regular: i64,
    literature: i64,
}

fn card_counts(root: &Path) -> Result<Counts> {
    if !db_path(root).exists() {
        return Ok(Counts::default());
    }
    let conn = Connection::open(db_path(root))?;
    let total = conn
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap_or(0);
    let topics = conn
        .query_row("SELECT COUNT(*) FROM cards WHERE is_topic = 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    let literature = conn
        .query_row("SELECT COUNT(*) FROM cards WHERE is_lit = 1", [], |row| {
            row.get(0)
        })
        .unwrap_or(0);
    Ok(Counts {
        total,
        topics,
        regular: total - topics - literature,
        literature,
    })
}

fn open_db(root: &Path) -> Result<Connection> {
    Connection::open(db_path(root))
        .with_context(|| format!("failed to open {}", db_path(root).display()))
}

#[derive(Clone, Debug)]
struct Card {
    row_id: i64,
    address: String,
    citation_key: Option<String>,
    is_topic: bool,
    is_lit: bool,
    bibtex: Option<String>,
    text: String,
    workspace: Workspace,
    card_type: CardType,
    /// Set only on a Model part: which facet of its model it presents.
    facet: Option<model::Facet>,
    /// Set only on a Model part: how much of a reader's attention it asks for.
    emphasis: Option<model::Emphasis>,
    /// Set only on a Model Card: how fully its model is elaborated.
    level: Option<model::Level>,
    /// Set only on a Hub Card: how wide its subject is.
    hub_level: Option<hub::HubLevel>,
    /// Set only on a Model Card: the mathematical unit it states, if any.
    form: Option<math::MathForm>,
}

impl Card {
    /// The Card's level as clients see it: a Model Card's elaboration level
    /// or a Hub Card's hub level.
    fn level_id(&self) -> Option<&'static str> {
        self.level
            .map(model::Level::id)
            .or(self.hub_level.map(hub::HubLevel::id))
    }
}

fn link_lifecycle(conn: &Connection) -> Result<link::LinkLifecycle> {
    link_lifecycle_from_cards(&load_cards(conn)?)
}

fn link_lifecycle_from_cards(cards: &[Card]) -> Result<link::LinkLifecycle> {
    let snapshots = cards
        .iter()
        .map(|card| {
            let parsed = parse_card_text(&card.text)?;
            Ok(link::CardSnapshot {
                address: card.address.clone(),
                title: parsed.title,
                body: parsed.body,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(link::LinkLifecycle::from_cards(snapshots))
}

fn validate_card_text(
    conn: &Connection,
    _location: &str,
    text: &str,
    old_text: Option<&str>,
) -> Result<ParsedCard> {
    let parsed = parse_card_text(text)?;
    let old = old_text.map(parse_card_text).transpose()?;
    link_lifecycle(conn)?
        .validate_edit(&parsed.body, old.as_ref().map(|card| card.body.as_str()))?;
    Ok(parsed)
}

fn is_valid_location(location: &str) -> bool {
    is_valid_topic_location(location) || is_valid_regular_location(location)
}

fn is_valid_topic_location(location: &str) -> bool {
    let Some((topic, rest)) = location.split_once('/') else {
        return false;
    };
    rest == "0" && valid_topic_id(topic)
}

fn is_valid_regular_location(location: &str) -> bool {
    let Some((tree, rest)) = location.split_once('/') else {
        return false;
    };
    if !valid_tree_id(tree) {
        return false;
    }
    let mut segments = rest.split('|');
    let Some(first) = segments.next() else {
        return false;
    };
    if !valid_positive_number(first) {
        return false;
    }
    segments.all(|segment| valid_positive_number(segment) || valid_side_label(segment))
}

fn valid_topic_id(value: &str) -> bool {
    value == "0" || valid_positive_number(value)
}

/// A tree id is the Location text before `/`: a Topic number or a Citation key.
fn valid_tree_id(value: &str) -> bool {
    valid_topic_id(value) || is_literature_tree_id(value)
}

fn is_literature_tree_id(value: &str) -> bool {
    literature::validate_citation_key(value).is_ok()
}

/// The tree an address belongs to. A Citation key is its own Literature tree id.
fn tree_id(address: &str) -> &str {
    address
        .split_once('/')
        .map(|(tree, _)| tree)
        .unwrap_or(address)
}

fn tree_root_address(tree: &str) -> String {
    if valid_topic_id(tree) {
        format!("{tree}/0")
    } else {
        tree.to_string()
    }
}

fn is_tree_member(address: &str, tree: &str) -> bool {
    address == tree_root_address(tree) || address.starts_with(&format!("{tree}/"))
}

fn is_tree_root(card: &Card) -> bool {
    card.is_topic || card.is_lit
}

fn ensure_side_parent(card: &Card) -> Result<()> {
    if card.is_topic {
        bail!("side successors cannot start from a Topic Card");
    }
    if card.is_lit {
        bail!("side successors cannot start from a Literature Card");
    }
    Ok(())
}

fn valid_positive_number(value: &str) -> bool {
    if value.is_empty() || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    if value.len() > 1 && value.starts_with('0') {
        return false;
    }
    value != "0"
}

fn valid_side_label(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|ch| ch.is_ascii_lowercase())
}

fn topic_id(location: &str) -> Result<&str> {
    location
        .split_once('/')
        .map(|(topic, _)| topic)
        .context("invalid location")
}

fn location_exists(conn: &Connection, location: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1)",
        [location],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn citation_key_exists(conn: &Connection, citation_key: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE citation_key = ?1)",
        [citation_key],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn target_exists(conn: &Connection, target: &str) -> Result<bool> {
    if is_valid_location(target) {
        location_exists(conn, target)
    } else if literature::validate_citation_key(target).is_ok() {
        citation_key_exists(conn, target)
    } else {
        Ok(false)
    }
}

fn load_card(conn: &Connection, target: &str) -> Result<Card> {
    conn.query_row(
        "SELECT row_id, COALESCE(location, citation_key), citation_key,
                is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form
         FROM cards WHERE location = ?1 OR citation_key = ?1",
        [target],
        |row| {
            Ok(Card {
                row_id: row.get(0)?,
                address: row.get(1)?,
                citation_key: row.get(2)?,
                is_topic: row.get::<_, i64>(3)? == 1,
                is_lit: row.get::<_, i64>(4)? == 1,
                bibtex: row.get(5)?,
                text: row.get(6)?,
                workspace: Workspace::from_stored(&row.get::<_, String>(7)?),
                card_type: CardType::from_stored(&row.get::<_, String>(8)?),
                facet: model::Facet::from_stored(row.get::<_, Option<String>>(9)?.as_deref()),
                emphasis: model::Emphasis::from_stored(
                    row.get::<_, Option<String>>(10)?.as_deref(),
                ),
                level: model::Level::from_stored(row.get::<_, Option<String>>(11)?.as_deref()),
                hub_level: hub::HubLevel::from_stored(row.get::<_, Option<String>>(12)?.as_deref()),
                form: math::MathForm::from_stored(row.get::<_, Option<String>>(13)?.as_deref()),
            })
        },
    )
    .with_context(|| format!("card `{target}` does not exist"))
}

fn load_cards(conn: &Connection) -> Result<Vec<Card>> {
    let mut stmt = conn.prepare(
        "SELECT row_id, COALESCE(location, citation_key), citation_key,
                is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form
         FROM cards ORDER BY COALESCE(location, citation_key)",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Card {
            row_id: row.get(0)?,
            address: row.get(1)?,
            citation_key: row.get(2)?,
            is_topic: row.get::<_, i64>(3)? == 1,
            is_lit: row.get::<_, i64>(4)? == 1,
            bibtex: row.get(5)?,
            text: row.get(6)?,
            workspace: Workspace::from_stored(&row.get::<_, String>(7)?),
            card_type: CardType::from_stored(&row.get::<_, String>(8)?),
            facet: model::Facet::from_stored(row.get::<_, Option<String>>(9)?.as_deref()),
            emphasis: model::Emphasis::from_stored(row.get::<_, Option<String>>(10)?.as_deref()),
            level: model::Level::from_stored(row.get::<_, Option<String>>(11)?.as_deref()),
            hub_level: hub::HubLevel::from_stored(row.get::<_, Option<String>>(12)?.as_deref()),
            form: math::MathForm::from_stored(row.get::<_, Option<String>>(13)?.as_deref()),
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn last_segment(location: &str) -> &str {
    location
        .rsplit_once('|')
        .map(|(_, segment)| segment)
        .or_else(|| location.rsplit_once('/').map(|(_, segment)| segment))
        .unwrap_or(location)
}

fn replace_last_segment(location: &str, replacement: &str) -> String {
    if let Some((prefix, _)) = location.rsplit_once('|') {
        format!("{prefix}|{replacement}")
    } else if let Some((topic, _)) = location.rsplit_once('/') {
        format!("{topic}/{replacement}")
    } else {
        replacement.to_string()
    }
}

fn direct_successor(location: &str) -> Result<String> {
    if is_valid_topic_location(location) {
        let topic = topic_id(location)?;
        return Ok(format!("{topic}/1"));
    }
    if is_literature_tree_id(location) {
        return Ok(format!("{location}/1"));
    }
    if !is_valid_regular_location(location) {
        bail!("invalid regular card location `{location}`");
    }
    let last = last_segment(location);
    if valid_positive_number(last) {
        let next = last.parse::<u64>()? + 1;
        Ok(replace_last_segment(location, &next.to_string()))
    } else {
        Ok(format!("{location}|1"))
    }
}

fn parent_location(location: &str) -> Option<String> {
    if is_valid_topic_location(location) {
        return None;
    }
    if !is_valid_regular_location(location) {
        return None;
    }
    let last = last_segment(location);
    if valid_side_label(last) {
        return location
            .rsplit_once('|')
            .map(|(prefix, _)| prefix.to_string());
    }
    let number = last.parse::<u64>().ok()?;
    if number > 1 {
        return Some(replace_last_segment(location, &(number - 1).to_string()));
    }
    if let Some((prefix, _)) = location.rsplit_once('|') {
        Some(prefix.to_string())
    } else {
        Some(tree_root_address(tree_id(location)))
    }
}

fn immediate_side_label(location: &str, parent: &str) -> Option<String> {
    let prefix = format!("{parent}|");
    let rest = location.strip_prefix(&prefix)?;
    let label = rest.split('|').next()?;
    valid_side_label(label).then_some(label.to_string())
}

fn side_label_to_number(label: &str) -> u64 {
    label
        .bytes()
        .fold(0_u64, |acc, byte| acc * 26 + (byte - b'a' + 1) as u64)
}

fn number_to_side_label(mut value: u64) -> String {
    let mut chars = Vec::new();
    while value > 0 {
        value -= 1;
        chars.push((b'a' + (value % 26) as u8) as char);
        value /= 26;
    }
    chars.iter().rev().collect()
}

fn next_side_successor(
    conn: &Connection,
    parent: &str,
    excluding: &BTreeSet<String>,
) -> Result<String> {
    let mut max_label = 0;
    for card in load_cards(conn)? {
        if excluding.contains(&card.address) {
            continue;
        }
        if let Some(label) = immediate_side_label(&card.address, parent) {
            max_label = max_label.max(side_label_to_number(&label));
        }
    }
    Ok(format!("{parent}|{}", number_to_side_label(max_label + 1)))
}

fn descendant_locations(conn: &Connection, root: &str) -> Result<BTreeSet<String>> {
    let cards = load_cards(conn)?;
    let locations: BTreeSet<String> = cards.iter().map(|card| card.address.clone()).collect();
    let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for location in &locations {
        if let Some(parent) = parent_location(location)
            && locations.contains(&parent)
        {
            children.entry(parent).or_default().push(location.clone());
        }
    }
    let mut result = BTreeSet::new();
    let mut queue = VecDeque::from([root.to_string()]);
    while let Some(location) = queue.pop_front() {
        if !result.insert(location.clone()) {
            continue;
        }
        if let Some(children) = children.get(&location) {
            for child in children {
                queue.push_back(child.clone());
            }
        }
    }
    Ok(result)
}

fn next_topic_location(conn: &Connection) -> Result<String> {
    Ok(format!("{}/0", next_topic_id(conn)?))
}

/// The Topic number the next new Topic will take. Deleted Topic numbers are
/// never reused, so this can exceed the largest existing Topic number.
fn next_topic_id(conn: &Connection) -> Result<u64> {
    let next = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_else(|| {
            let max_existing: Option<u64> = load_cards(conn)
                .unwrap_or_default()
                .iter()
                .filter(|card| card.is_topic)
                .filter_map(|card| topic_id(&card.address).ok()?.parse::<u64>().ok())
                .max();
            max_existing.map(|value| value + 1).unwrap_or(0)
        });
    Ok(next)
}

fn bump_next_topic_id(conn: &Connection, topic_location: &str) -> Result<()> {
    let next = topic_id(topic_location)?.parse::<u64>()? + 1;
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('next_topic_id', ?1)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [next.to_string()],
    )?;
    Ok(())
}

enum EditOutcome {
    Saved(String),
    Canceled,
}

fn run_editor(initial_text: &str) -> Result<EditOutcome> {
    run_editor_with_suffix(initial_text, ".zt.md")
}

fn run_editor_with_suffix(initial_text: &str, suffix: &str) -> Result<EditOutcome> {
    let editor = env::var("EDITOR").context("EDITOR is not set")?;
    let mut temp = tempfile::Builder::new().suffix(suffix).tempfile()?;
    temp.write_all(initial_text.as_bytes())?;
    temp.flush()?;
    let temp_path = temp.into_temp_path();
    let path = temp_path.to_path_buf();
    let mut parts = editor.split_whitespace();
    let command = parts.next().context("EDITOR is empty")?;
    let args: Vec<&str> = parts.collect();
    let status = Command::new(command)
        .args(args)
        .arg(&path)
        .status()
        .with_context(|| format!("failed to run editor `{editor}`"))?;
    if !status.success() {
        return Ok(EditOutcome::Canceled);
    }
    let edited = fs::read_to_string(&path)?;
    drop(temp_path);
    Ok(EditOutcome::Saved(edited))
}

fn shell_create_literature(workspace: Workspace) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let mut metadata_buffer = String::new();
    let (bibtex, metadata) = loop {
        let EditOutcome::Saved(edited) = run_editor_with_suffix(&metadata_buffer, ".bib")? else {
            return Ok(());
        };
        match literature::parse_metadata(&edited) {
            Ok(metadata)
                if !citation_key_exists_case_insensitive(&conn, &metadata.citation_key)? =>
            {
                break (edited, metadata);
            }
            Ok(metadata) => {
                eprintln!("Literature Card `{}` already exists", metadata.citation_key);
            }
            Err(error) => eprintln!("{error:#}"),
        }
        metadata_buffer = edited;
    };
    let initial_text = card::card_template(&metadata.title);
    let EditOutcome::Saved(edited_text) = run_editor(&initial_text)? else {
        return Ok(());
    };
    let parsed = parse_literature_edit_text(&edited_text)?;
    let text = compose_card_text(&metadata.title, &parsed.body, "");
    insert_literature_card(&mut conn, &metadata.citation_key, workspace, &bibtex, &text)?;
    println!("{}", metadata.citation_key);
    Ok(())
}

fn citation_key_exists_case_insensitive(conn: &Connection, citation_key: &str) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM cards WHERE citation_key = ?1 COLLATE NOCASE)",
        [citation_key],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn citation_key_conflicts(
    conn: &Connection,
    citation_key: &str,
    excluding_row_id: i64,
) -> Result<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM cards
            WHERE citation_key = ?1 COLLATE NOCASE AND row_id != ?2
        )",
        params![citation_key, excluding_row_id],
        |row| row.get::<_, i64>(0),
    )? == 1)
}

fn insert_literature_card(
    conn: &mut Connection,
    citation_key: &str,
    workspace: Workspace,
    bibtex: &str,
    text: &str,
) -> Result<()> {
    literature::validate_citation_key(citation_key)?;
    if citation_key_exists_case_insensitive(conn, citation_key)? {
        bail!("Literature Card `{citation_key}` already exists");
    }
    validate_card_text(conn, citation_key, text, None)?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type)
         VALUES(NULL, ?1, 0, 1, ?2, ?3, ?4, ?5)",
        params![
            citation_key,
            bibtex,
            text,
            workspace.id(),
            CardType::Idea.id()
        ],
    )?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

/// The first line of an edit lock file whose holder also keeps the file's OS lock.
const EDIT_LOCK_MARKER: &str = "zt edit lock";

/// The edit lock: `zt.edit.lock`, created new by its holder and removed when the holder is done.
/// The holder also keeps the file's OS lock, which the system releases however the holder ends,
/// so the file of a holder that was killed is found stale and taken over. A lock file without
/// the marker comes from an older zt, which keeps no OS lock; it counts as held. The short-lived
/// `zt.edit.guard` lock orders taking over and removing, so no two processes hold the edit lock.
struct EditLock {
    root: PathBuf,
    file: Option<fs::File>,
}

impl Drop for EditLock {
    fn drop(&mut self) {
        // Remove the file before its OS lock is released, under the guard, so no process can
        // find it free and take it over in between.
        let guard = lock_edit_guard(&self.root).ok();
        fs::remove_file(edit_lock_path(&self.root)).ok();
        drop(self.file.take());
        drop(guard);
    }
}

fn lock_edit_guard(root: &Path) -> Result<fs::File> {
    let path = edit_guard_path(root);
    let guard = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    guard
        .lock()
        .with_context(|| format!("failed to lock {}", path.display()))?;
    Ok(guard)
}

fn acquire_edit_lock(root: &Path) -> Result<EditLock> {
    let path = edit_lock_path(root);
    let _guard = lock_edit_guard(root)?;
    // A second try follows only after a stale lock file was removed.
    for _ in 0..2 {
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path);
        match file {
            Ok(mut file) => {
                writeln!(file, "{EDIT_LOCK_MARKER}\npid {}", std::process::id())
                    .and_then(|()| file.lock())
                    .with_context(|| format!("failed to lock {}", path.display()))?;
                return Ok(EditLock {
                    root: root.to_path_buf(),
                    file: Some(file),
                });
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                if !remove_stale_edit_lock(&path)? {
                    break;
                }
            }
            Err(err) => {
                return Err(err).with_context(|| format!("failed to create {}", path.display()));
            }
        }
    }
    bail!("edit in progress")
}

/// Removes the lock file of a holder that ended without removing it: the file has the marker
/// and nobody keeps its OS lock. Returns whether the lock file is gone. The caller holds the guard.
fn remove_stale_edit_lock(path: &Path) -> Result<bool> {
    let mut file = match fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(true),
        Err(_) => return Ok(false),
    };
    if file.try_lock().is_err() {
        return Ok(false);
    }
    let mut text = String::new();
    io::Read::read_to_string(&mut file, &mut text).ok();
    if !text.starts_with(EDIT_LOCK_MARKER) {
        return Ok(false);
    }
    fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    Ok(true)
}

/// The edit lock for clients that make many short writes at once (the MCP server and the GUI):
/// wait up to `patience` for another short write to finish instead of refusing at once. A
/// Session holds the lock while its editor is open, so after `patience` the refusal stands.
fn acquire_edit_lock_within(root: &Path, patience: Duration) -> Result<EditLock> {
    let start = Instant::now();
    let mut pause = Duration::from_millis(20);
    loop {
        match acquire_edit_lock(root) {
            Err(err) if err.to_string() == "edit in progress" && start.elapsed() < patience => {
                thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(400));
            }
            other => return other,
        }
    }
}

fn shell_create_topic(workspace: Workspace, title: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    if title.trim().is_empty() || title.contains('\n') || title.contains('\r') {
        bail!("topic title must be non-empty single-line text");
    }
    let mut conn = open_db(&root)?;
    match run_editor(&topic_template(title))? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            let location = create_topic(&mut conn, workspace, &text)?;
            println!("{location}");
            Ok(())
        }
    }
}

fn shell_create_direct(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let parent = load_card(&conn, at)?;
    let location = direct_successor(&parent.address)?;
    if location_exists(&conn, &location)? {
        bail!("direct successor already exists: {location}");
    }
    match run_editor(&regular_template())? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            insert_card(
                &mut conn,
                &location,
                false,
                parent.workspace,
                CardType::Idea,
                None,
                &text,
            )?;
            println!("{location}");
            Ok(())
        }
    }
}

fn shell_create_side(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let parent = load_card(&conn, at)?;
    ensure_side_parent(&parent)?;
    let location = next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
    match run_editor(&regular_template())? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            insert_card(
                &mut conn,
                &location,
                false,
                parent.workspace,
                CardType::Idea,
                None,
                &text,
            )?;
            println!("{location}");
            Ok(())
        }
    }
}

fn shell_edit(at: &str, part: Option<EditPart>) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let card = load_card(&conn, at)?;
    if card.is_lit {
        return match part {
            Some(EditPart::Metadata) => shell_edit_literature_metadata(&mut conn, &card),
            Some(EditPart::Text) => shell_edit_literature_text(&mut conn, &card),
            None => bail!("Literature Card editing requires --part metadata or --part text"),
        };
    }
    if part.is_some() {
        bail!("--part is valid only when editing a Literature Card");
    }
    match run_editor(&card.text)? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => update_card_text(&mut conn, &card, &text),
    }
}

fn shell_edit_literature_text(conn: &mut Connection, card: &Card) -> Result<()> {
    match run_editor(&card.text)? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(edited) => update_literature_text(conn, card, &edited),
    }
}

fn update_literature_text(conn: &mut Connection, card: &Card, edited: &str) -> Result<()> {
    let metadata = literature::parse_metadata(
        card.bibtex
            .as_deref()
            .context("Literature Card is missing its BibTeX metadata")?,
    )?;
    let parsed = parse_literature_edit_text(edited)?;
    let text = compose_card_text(&metadata.title, &parsed.body, "");
    validate_card_text(conn, &card.address, &text, Some(&card.text))?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE cards SET text = ?1 WHERE row_id = ?2",
        params![text, card.row_id],
    )?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

fn shell_edit_literature_metadata(conn: &mut Connection, card: &Card) -> Result<()> {
    let citation_key = card
        .citation_key
        .as_deref()
        .context("Literature Card is missing its Citation key")?;
    let mut buffer = card
        .bibtex
        .clone()
        .context("Literature Card is missing its BibTeX metadata")?;
    loop {
        let EditOutcome::Saved(edited) = run_editor_with_suffix(&buffer, ".bib")? else {
            return Ok(());
        };
        let metadata = match literature::parse_metadata(&edited) {
            Ok(metadata) => metadata,
            Err(error) => {
                eprintln!("{error:#}");
                buffer = edited;
                continue;
            }
        };
        let renaming = metadata.citation_key != citation_key;
        if renaming
            && (metadata.citation_key.eq_ignore_ascii_case(citation_key)
                || citation_key_conflicts(conn, &metadata.citation_key, card.row_id)?)
        {
            eprintln!(
                "Citation key `{}` conflicts with an existing Literature Card",
                metadata.citation_key
            );
            buffer = edited;
            continue;
        }
        if renaming {
            let lines = citation_rename_lines(conn, citation_key, &metadata.citation_key)?;
            for line in lines {
                println!("{line}");
            }
            let stdin = io::stdin();
            let mut input = stdin.lock();
            read_confirmation(&mut input, "move")?;
        }
        update_literature_metadata(conn, card, &metadata.citation_key, &edited, &metadata.title)?;
        return Ok(());
    }
}

/// Renaming a Citation key renames its whole Literature tree: `Old -> New`
/// and every `Old/<suffix> -> New/<suffix>`.
fn citation_rename_mapping(
    conn: &Connection,
    old_key: &str,
    new_key: &str,
) -> Result<BTreeMap<String, String>> {
    let old_prefix = format!("{old_key}/");
    let new_prefix = format!("{new_key}/");
    let mut mapping = BTreeMap::from([(old_key.to_string(), new_key.to_string())]);
    for card in load_cards(conn)? {
        if card.address.starts_with(&new_prefix) {
            bail!(
                "Location `{}` already uses Citation key `{new_key}`",
                card.address
            );
        }
        if let Some(suffix) = card.address.strip_prefix(&old_prefix) {
            mapping.insert(card.address.clone(), format!("{new_prefix}{suffix}"));
        }
    }
    Ok(mapping)
}

fn citation_rename_lines(conn: &Connection, old_key: &str, new_key: &str) -> Result<Vec<String>> {
    let mapping = citation_rename_mapping(conn, old_key, new_key)?;
    let rewritten_links = count_rewritten_links(conn, &mapping)?;
    let mut lines = vec!["citation key rename:".to_string()];
    for (old, new) in &mapping {
        lines.push(format!("{old} -> {new}"));
    }
    lines.push(format!("renamed cards: {}", mapping.len()));
    lines.push(format!("link macros rewritten: {rewritten_links}"));
    Ok(lines)
}

fn update_literature_metadata(
    conn: &mut Connection,
    card: &Card,
    citation_key: &str,
    bibtex: &str,
    title: &str,
) -> Result<()> {
    let parsed = parse_card_text(&card.text)?;
    let text = compose_card_text(title, &parsed.body, "");
    let tx = conn.transaction()?;
    let mapping = if citation_key != card.address {
        citation_rename_mapping(&tx, &card.address, citation_key)?
    } else {
        BTreeMap::new()
    };
    tx.execute(
        "UPDATE cards SET citation_key = ?1, bibtex = ?2, text = ?3 WHERE row_id = ?4",
        params![citation_key, bibtex, text, card.row_id],
    )?;
    apply_location_mapping_tx(&tx, &mapping)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Insert a Topic or Regular Card. A Regular Card must name the Workspace of
/// the tree it joins; only a Regular Card may be an Event or Model Card.
fn insert_card(
    conn: &mut Connection,
    location: &str,
    is_topic: bool,
    workspace: Workspace,
    card_type: CardType,
    part: Option<&model::Part>,
    text: &str,
) -> Result<()> {
    if !is_valid_location(location) {
        bail!("invalid location `{location}`");
    }
    if is_topic && card_type != CardType::Idea {
        bail!("only Regular Cards can be Event, Finding, Model, or Hub Cards");
    }
    if location_exists(conn, location)? {
        bail!("card `{location}` already exists");
    }
    if !is_topic {
        let tree_workspace = workspace::tree_workspace(conn, location)?;
        if tree_workspace != workspace {
            bail!(
                "card `{location}` belongs to the {tree_workspace} workspace, not the {workspace} workspace"
            );
        }
    }
    let facet = part.map(|part| &part.facet);
    let emphasis = part.and_then(|part| part.emphasis);
    model::ensure_allowed(conn, None, card_type, facet, emphasis)?;
    validate_card_text(conn, location, text, None)?;
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis)
         VALUES(?1, NULL, ?2, 0, NULL, ?3, ?4, ?5, ?6, ?7)",
        params![
            location,
            if is_topic { 1 } else { 0 },
            text,
            workspace.id(),
            card_type.id(),
            facet.map(model::Facet::id),
            emphasis.map(model::Emphasis::id)
        ],
    )?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Create a Topic in `workspace` at the next Topic number.
fn create_topic(conn: &mut Connection, workspace: Workspace, text: &str) -> Result<String> {
    let location = next_topic_location(conn)?;
    insert_card(conn, &location, true, workspace, CardType::Idea, None, text)?;
    bump_next_topic_id(conn, &location)?;
    Ok(location)
}

fn update_card_text(conn: &mut Connection, card: &Card, text: &str) -> Result<()> {
    validate_card_text(conn, &card.address, text, Some(&card.text))?;
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE cards SET text = ?1 WHERE row_id = ?2",
        params![text, card.row_id],
    )?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

fn regenerate_reverse_links_tx(conn: &Connection) -> Result<()> {
    let cards = load_cards(conn)?;
    let reverse_sections = link_lifecycle_from_cards(&cards)?.reverse_sections()?;
    for card in &cards {
        let parsed = parse_card_text(&card.text)?;
        let reverse = reverse_sections
            .get(&card.address)
            .context("Link lifecycle omitted a Card Reverse link section")?;
        let text = compose_card_text(&parsed.title, &parsed.body, reverse);
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE row_id = ?2",
            params![text, card.row_id],
        )?;
    }
    Ok(())
}

fn replace_link_locations(text: &str, mapping: &BTreeMap<String, String>) -> Result<String> {
    let parsed = parse_card_text(text)?;
    let body = link::LinkBody::parse(&parsed.body)?
        .rewrite_targets(mapping)
        .body;
    Ok(compose_card_text(&parsed.title, &body, &parsed.reverse))
}

fn shell_delete(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let result = delete_card(&mut conn, at, |expected| {
        read_confirmation(&mut input, expected)
    })?;
    println!("deleted: {}", result.deleted_count);
    if !result.moved.is_empty() {
        println!("compacted:");
        for (old, new) in result.moved {
            println!("{old} -> {new}");
        }
    }
    Ok(())
}

struct DeleteResult {
    deleted_count: usize,
    moved: BTreeMap<String, String>,
}

struct DeletePlan {
    delete_set: BTreeSet<String>,
    confirmation: String,
    /// Links from Cards that stay to Cards that go, as (source, target): each breaks unless the
    /// plan re-points it with `into`.
    inbound: Vec<(String, String)>,
    /// The Card that takes over those Links: a deleted part's content now lives there.
    into: Option<String>,
    /// The later Side successors of the same parent, each one letter down (`side_compaction_mapping`).
    side_compaction: BTreeMap<String, String>,
}

impl DeletePlan {
    /// Re-point every Link into the deleted Cards to `into`, which must exist and stay.
    fn into(mut self, conn: &Connection, into: &str) -> Result<Self> {
        let target = load_card(conn, into)?;
        if self.delete_set.contains(&target.address) {
            bail!("`{into}` is deleted too; re-point the Links to a Card that stays");
        }
        self.into = Some(target.address);
        Ok(self)
    }

    /// Each deleted Card that a staying Card links, mapped to `into`.
    fn repoint_mapping(&self) -> BTreeMap<String, String> {
        self.into
            .as_ref()
            .map(|into| {
                self.inbound
                    .iter()
                    .map(|(_, target)| (target.clone(), into.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The renames the delete applies: the side compaction, unless a Link that breaks targets an
    /// address a renamed Card would take. Then every Card keeps its Location and the Link stays
    /// broken (new Side successors take the next label after the highest, so the gap stays).
    fn compaction(&self) -> BTreeMap<String, String> {
        if self.compaction_kept_out() {
            BTreeMap::new()
        } else {
            self.side_compaction.clone()
        }
    }

    fn compaction_kept_out(&self) -> bool {
        self.into.is_none()
            && self
                .inbound
                .iter()
                .any(|(_, target)| self.side_compaction.values().any(|new| new == target))
    }
}

/// Links from Cards outside `delete_set` to Cards in it, once per (source, target).
fn links_into(conn: &Connection, delete_set: &BTreeSet<String>) -> Result<Vec<(String, String)>> {
    let mut found = BTreeSet::new();
    for card in load_cards(conn)? {
        if delete_set.contains(&card.address) {
            continue;
        }
        let parsed = parse_card_text(&card.text)?;
        for occurrence in link::LinkBody::parse(&parsed.body)?.occurrences() {
            if delete_set.contains(occurrence.target) {
                found.insert((card.address.clone(), occurrence.target.to_string()));
            }
        }
    }
    Ok(found.into_iter().collect())
}

/// Deleting a Tree root removes its whole tree and must be confirmed by
/// typing the root's own address; any other Card is confirmed with `delete`.
fn delete_plan(conn: &Connection, at: &str) -> Result<DeletePlan> {
    let card = load_card(conn, at)?;
    let is_tree_root = is_tree_root(&card);
    let delete_set = if is_tree_root {
        let tree = tree_id(&card.address);
        load_cards(conn)?
            .into_iter()
            .filter(|member| is_tree_member(&member.address, tree))
            .map(|member| member.address)
            .collect::<BTreeSet<_>>()
    } else {
        descendant_locations(conn, &card.address)?
    };
    let confirmation = if is_tree_root {
        card.address.clone()
    } else {
        "delete".to_string()
    };
    let inbound = links_into(conn, &delete_set)?;
    let side_compaction = if is_tree_root {
        BTreeMap::new()
    } else {
        side_compaction_mapping(conn, &card.address, &delete_set)?
    };
    Ok(DeletePlan {
        delete_set,
        confirmation,
        inbound,
        into: None,
        side_compaction,
    })
}

fn delete_verification_lines(plan: &DeletePlan) -> Vec<String> {
    let mut lines = vec!["delete verification:".to_string()];
    for location in &plan.delete_set {
        lines.push(format!("delete {location}"));
    }
    lines.push(format!(
        "successors: {}",
        plan.delete_set.len().saturating_sub(1)
    ));
    if !plan.inbound.is_empty() {
        match &plan.into {
            Some(into) => {
                lines.push(format!("links re-pointed to {into}: {}", plan.inbound.len()));
                for (source, target) in &plan.inbound {
                    lines.push(format!("{source}: [[{target}]] -> [[{into}]]"));
                }
            }
            None => {
                lines.push(format!("links that will break: {}", plan.inbound.len()));
                for (source, target) in &plan.inbound {
                    lines.push(format!("{source} -> {target}"));
                }
                if plan.compaction_kept_out() {
                    lines.push(
                        "later side successors keep their locations, so no card takes over a broken link"
                            .to_string(),
                    );
                }
            }
        }
    }
    lines
}

fn print_delete_verification(plan: &DeletePlan) {
    for line in delete_verification_lines(plan) {
        println!("{line}");
    }
}

fn apply_delete_plan(conn: &mut Connection, plan: DeletePlan) -> Result<DeleteResult> {
    let compaction = plan.compaction();
    let repoint = plan.repoint_mapping();
    let tx = conn.transaction()?;
    // Re-point first, so a side compaction that renames `into` carries the new Links along.
    if !repoint.is_empty() {
        for card in load_cards(&tx)? {
            if plan.delete_set.contains(&card.address) {
                continue;
            }
            let text = replace_link_locations(&card.text, &repoint)?;
            if text != card.text {
                tx.execute(
                    "UPDATE cards SET text = ?1 WHERE row_id = ?2",
                    params![text, card.row_id],
                )?;
            }
        }
    }
    for address in &plan.delete_set {
        tx.execute(
            "DELETE FROM cards WHERE location = ?1 OR citation_key = ?1",
            [address],
        )?;
    }
    spread::follow(&tx, &BTreeMap::new(), &plan.delete_set)?;
    apply_location_mapping_tx(&tx, &compaction)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(DeleteResult {
        deleted_count: plan.delete_set.len(),
        moved: compaction,
    })
}

fn delete_card<F>(conn: &mut Connection, at: &str, mut confirm: F) -> Result<DeleteResult>
where
    F: FnMut(&str) -> Result<()>,
{
    let plan = delete_plan(conn, at)?;
    print_delete_verification(&plan);
    confirm(&plan.confirmation)?;
    apply_delete_plan(conn, plan)
}

fn read_confirmation<R: BufRead>(input: &mut R, expected: &str) -> Result<()> {
    println!("type `{expected}` to confirm:");
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim() != expected {
        bail!("confirmation did not match");
    }
    Ok(())
}

fn side_compaction_mapping(
    conn: &Connection,
    deleted_root: &str,
    delete_set: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let last = last_segment(deleted_root);
    if !valid_side_label(last) {
        return Ok(BTreeMap::new());
    }
    let Some(parent) = parent_location(deleted_root) else {
        return Ok(BTreeMap::new());
    };
    let deleted_rank = side_label_to_number(last);
    let cards = load_cards(conn)?;
    let mut labels = BTreeSet::new();
    for card in &cards {
        if delete_set.contains(&card.address) {
            continue;
        }
        if let Some(label) = immediate_side_label(&card.address, &parent)
            && side_label_to_number(&label) > deleted_rank
        {
            labels.insert(label);
        }
    }
    let mut prefix_map = BTreeMap::new();
    for label in labels {
        let rank = side_label_to_number(&label);
        prefix_map.insert(
            format!("{parent}|{label}"),
            format!("{parent}|{}", number_to_side_label(rank - 1)),
        );
    }
    let mut mapping = BTreeMap::new();
    for card in cards {
        if delete_set.contains(&card.address) {
            continue;
        }
        for (old_prefix, new_prefix) in &prefix_map {
            if card.address == *old_prefix || card.address.starts_with(&format!("{old_prefix}|")) {
                let suffix = &card.address[old_prefix.len()..];
                mapping.insert(card.address.clone(), format!("{new_prefix}{suffix}"));
                break;
            }
        }
    }
    Ok(mapping)
}

fn apply_location_mapping_tx(conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<()> {
    if mapping.is_empty() {
        return Ok(());
    }
    for old in mapping.keys() {
        conn.execute(
            "UPDATE cards SET location = ?1 WHERE location = ?2",
            params![format!("__moving__{old}"), old],
        )?;
    }
    for (old, new) in mapping {
        conn.execute(
            "UPDATE cards SET location = ?1 WHERE location = ?2",
            params![new, format!("__moving__{old}")],
        )?;
    }
    let cards = load_cards(conn)?;
    for card in cards {
        let text = replace_link_locations(&card.text, mapping)?;
        conn.execute(
            "UPDATE cards SET text = ?1 WHERE row_id = ?2",
            params![text, card.row_id],
        )?;
    }
    spread::follow(conn, mapping, &BTreeSet::new())
}

fn shell_move(at: &str, target: &place::Target, alone: bool) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let plan = place::move_plan(&conn, at, target, alone)?;
    print_move_verification(&plan);
    read_confirmation(&mut input, "move")?;
    apply_move_plan(&mut conn, &plan)?;
    println!("moved: {}", plan.mapping.len());
    for (old, new) in &plan.mapping {
        println!("{old} -> {new}");
    }
    Ok(())
}

/// Insert a new Card at `at`: confirm the renumbering, then write the Card.
fn shell_insert(at: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root)?;
    let mut conn = open_db(&root)?;
    let plan = place::insert_plan(&conn, at)?;
    for line in place::insert_verification_lines(&plan) {
        println!("{line}");
    }
    let stdin = io::stdin();
    read_confirmation(&mut stdin.lock(), "insert")?;
    match run_editor(&regular_template())? {
        EditOutcome::Canceled => Ok(()),
        EditOutcome::Saved(text) => {
            place::apply_insert_plan(&mut conn, &plan, CardType::Idea, None, &text)?;
            println!("{}", plan.location);
            Ok(())
        }
    }
}

struct MovePlan {
    mapping: BTreeMap<String, String>,
    rewritten_links: usize,
}

fn move_plan(conn: &Connection, at: &str, new_location: &str) -> Result<MovePlan> {
    let card = load_card(conn, at)?;
    if card.is_lit {
        bail!("Literature Card cannot be moved; edit metadata to change its Citation key");
    }
    if card.is_topic {
        bail!("Topic Cards cannot be moved");
    }
    if !is_valid_regular_location(new_location) {
        bail!("invalid move location `{new_location}`");
    }
    ensure_same_tree_kind_boundary(&card.address, new_location)?;
    workspace::ensure_same_workspace(conn, &card.address, new_location)?;
    let moved = descendant_locations(conn, at)?;
    if moved.contains(new_location) {
        bail!("cannot move a card into its own successors");
    }
    validate_move_target(conn, &moved, new_location)?;
    let mapping = move_mapping(conn, at, new_location, &moved)?;
    let conflicts: Vec<String> = mapping
        .values()
        .filter(|location| {
            location_exists(conn, location).unwrap_or(false) && !moved.contains(*location)
        })
        .cloned()
        .collect();
    if !conflicts.is_empty() {
        bail!(
            "destination locations already exist: {}",
            conflicts.join(", ")
        );
    }
    let rewritten_links = count_rewritten_links(conn, &mapping)?;
    Ok(MovePlan {
        mapping,
        rewritten_links,
    })
}

fn move_verification_lines(plan: &MovePlan) -> Vec<String> {
    let mut lines = vec!["move verification:".to_string()];
    for (old, new) in &plan.mapping {
        lines.push(format!("{old} -> {new}"));
    }
    lines.push(format!("moved cards: {}", plan.mapping.len()));
    lines.push(format!("link macros rewritten: {}", plan.rewritten_links));
    lines
}

fn print_move_verification(plan: &MovePlan) {
    for line in move_verification_lines(plan) {
        println!("{line}");
    }
}

fn apply_move_plan(conn: &mut Connection, plan: &MovePlan) -> Result<()> {
    let tx = conn.transaction()?;
    apply_location_mapping_tx(&tx, &plan.mapping)?;
    regenerate_reverse_links_tx(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Literature tree Cards stay in their originating Literature tree, and Topic
/// tree Cards never move into a Literature tree.
fn ensure_same_tree_kind_boundary(from: &str, to: &str) -> Result<()> {
    let from_tree = tree_id(from);
    let to_tree = tree_id(to);
    if from_tree == to_tree {
        return Ok(());
    }
    if is_literature_tree_id(from_tree) {
        bail!("Literature tree Cards cannot leave Literature tree `{from_tree}`");
    }
    if is_literature_tree_id(to_tree) {
        bail!("Topic tree Cards cannot move into Literature tree `{to_tree}`");
    }
    Ok(())
}

fn validate_move_target(
    conn: &Connection,
    moved: &BTreeSet<String>,
    new_location: &str,
) -> Result<()> {
    let parent = parent_location(new_location).context("move target has no parent")?;
    if !target_exists(conn, &parent)? || moved.contains(&parent) {
        bail!("move target parent does not exist outside the moved subtree");
    }
    let last = last_segment(new_location);
    if valid_side_label(last) {
        let expected = next_side_successor(conn, &parent, moved)?;
        if expected != new_location {
            bail!("side successor move target must be the next available label: {expected}");
        }
    } else if direct_successor(&parent)? != new_location {
        bail!("move target does not preserve direct successor rules");
    }
    Ok(())
}

fn move_mapping(
    conn: &Connection,
    old_root: &str,
    new_root: &str,
    moved: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let cards = load_cards(conn)?;
    let locations: BTreeSet<String> = cards.iter().map(|card| card.address.clone()).collect();
    let mut mapping = BTreeMap::new();
    for location in moved {
        let edges = path_edges(old_root, location, &locations)?;
        let mut target = new_root.to_string();
        for edge in edges {
            target = match edge {
                Edge::Direct => direct_successor(&target)?,
                Edge::Side(label) => format!("{target}|{label}"),
            };
        }
        mapping.insert(location.clone(), target);
    }
    Ok(mapping)
}

enum Edge {
    Direct,
    Side(String),
}

fn path_edges(root: &str, location: &str, existing: &BTreeSet<String>) -> Result<Vec<Edge>> {
    if root == location {
        return Ok(Vec::new());
    }
    let mut current = location.to_string();
    let mut reversed = Vec::new();
    while current != root {
        let parent = parent_location(&current).context("location has no parent")?;
        if !existing.contains(&parent) && parent != root {
            bail!("missing parent `{parent}`");
        }
        let last = last_segment(&current);
        if valid_side_label(last) {
            reversed.push(Edge::Side(last.to_string()));
        } else {
            reversed.push(Edge::Direct);
        }
        current = parent;
    }
    reversed.reverse();
    Ok(reversed)
}

fn count_rewritten_links(conn: &Connection, mapping: &BTreeMap<String, String>) -> Result<usize> {
    let mut count = 0;
    for card in load_cards(conn)? {
        let parsed = parse_card_text(&card.text)?;
        count += link::LinkBody::parse(&parsed.body)?
            .rewrite_targets(mapping)
            .count;
    }
    Ok(count)
}

fn cmd_lsbk() -> Result<()> {
    let root = require_service_up()?;
    let conn = open_db(&root)?;
    for broken in link_lifecycle(&conn)?.broken_links()? {
        println!(
            "{} {} -> {}: {}",
            broken.source_address, broken.source_title, broken.target, broken.source_line
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literature_tree_locations_use_the_citation_key_as_tree_id() {
        for valid in [
            "Smith2024/1",
            "Smith2024/2|a",
            "Smith2024/1|a|3|bb",
            "0/1|c",
        ] {
            assert!(is_valid_regular_location(valid), "{valid} should be valid");
            assert!(is_valid_location(valid));
        }
        for invalid in [
            "Smith2024/0",
            "Smith2024",
            "1Smith/1",
            "Smith 2024/1",
            "Smith2024/01",
            "CON/1",
        ] {
            assert!(!is_valid_location(invalid), "{invalid} should be invalid");
        }
    }

    #[test]
    fn tree_ids_roots_and_membership_cover_both_tree_kinds() {
        assert_eq!(tree_id("3/1|a"), "3");
        assert_eq!(tree_id("Smith2024/1|a"), "Smith2024");
        assert_eq!(tree_id("Smith2024"), "Smith2024");
        assert_eq!(tree_root_address("3"), "3/0");
        assert_eq!(tree_root_address("Smith2024"), "Smith2024");
        assert!(is_tree_member("3/0", "3"));
        assert!(is_tree_member("Smith2024", "Smith2024"));
        assert!(is_tree_member("Smith2024/1", "Smith2024"));
        assert!(!is_tree_member("Smith2024x/1", "Smith2024"));
        assert!(!is_tree_member("30/1", "3"));
    }

    #[test]
    fn literature_roots_have_direct_successors_and_parent_their_first_card() {
        assert_eq!(direct_successor("Smith2024").unwrap(), "Smith2024/1");
        assert_eq!(direct_successor("Smith2024/1").unwrap(), "Smith2024/2");
        assert_eq!(
            direct_successor("Smith2024/1|a").unwrap(),
            "Smith2024/1|a|1"
        );
        assert_eq!(parent_location("Smith2024/1").as_deref(), Some("Smith2024"));
        assert_eq!(
            parent_location("Smith2024/2").as_deref(),
            Some("Smith2024/1")
        );
        assert_eq!(
            parent_location("Smith2024/1|b").as_deref(),
            Some("Smith2024/1")
        );
        assert_eq!(parent_location("Smith2024"), None);
        assert_eq!(parent_location("0/1").as_deref(), Some("0/0"));
        assert_eq!(parent_location("0/0"), None);
    }

    #[test]
    fn move_boundary_keeps_literature_trees_closed() {
        ensure_same_tree_kind_boundary("Smith2024/1|a", "Smith2024/2").unwrap();
        ensure_same_tree_kind_boundary("0/1|a", "1/2").unwrap();
        assert_eq!(
            ensure_same_tree_kind_boundary("Smith2024/1", "Jones2025/1")
                .unwrap_err()
                .to_string(),
            "Literature tree Cards cannot leave Literature tree `Smith2024`"
        );
        assert_eq!(
            ensure_same_tree_kind_boundary("Smith2024/1", "0/2")
                .unwrap_err()
                .to_string(),
            "Literature tree Cards cannot leave Literature tree `Smith2024`"
        );
        assert_eq!(
            ensure_same_tree_kind_boundary("0/1", "Smith2024/2")
                .unwrap_err()
                .to_string(),
            "Topic tree Cards cannot move into Literature tree `Smith2024`"
        );
    }
}
