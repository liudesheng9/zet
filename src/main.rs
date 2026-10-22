use anyhow::{Context, Result, bail};
use rusqlite::{Connection, params};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(not(windows))]
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

mod artifact;
mod autostart;
mod batch;
mod card;
mod card_type;
mod dump;
mod entry;
mod features;
mod gui;
mod hub;
mod link;
mod link_index;
mod literature;
mod math;
mod mcp;
mod model;
mod ops;
mod place;
mod read;
mod reply;
mod scholar;
mod session;
mod shutdown;
mod spread;
mod tree_lock;
mod workspace;

pub(crate) use card::{
    ParsedCard, compose_card_text, parse_card_text, parse_literature_edit_text, regular_template,
    topic_template,
};
pub(crate) use card_type::CardType;
pub(crate) use workspace::{Access, Workspace};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    quit_quietly_when_stdout_has_no_reader();
    if let Err(err) = run() {
        eprintln!("{err:#}");
        std::process::exit(1);
    }
}

/// A consumer that closes the pipe early (`head`, `Select-Object -Last 1`) is not a command
/// failure: the work is done and only the last write had no reader. Rust's `println!` panics
/// on that write, so the default hook is kept for every other panic and this one case exits 0.
fn quit_quietly_when_stdout_has_no_reader() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let text = info
            .payload()
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| info.payload().downcast_ref::<&str>().copied())
            .unwrap_or_default();
        // "failed printing to stdout: <io error>", whose OS codes are 32 (EPIPE) and 232 (Windows).
        let closed = text.contains("failed printing to stdout")
            && (text.contains("os error 32)") || text.contains("os error 232)"));
        if closed {
            std::process::exit(0);
        }
        default(info);
    }));
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
        [cmd, flag] if cmd == "status" && flag == "--check-links" => link_index::command(false),
        [cmd, flag] if cmd == "status" && flag == "--check-locks" => cmd_status_check_locks(),
        [cmd] if cmd == "reindex" => link_index::command(true),
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
        [cmd, sub, key, value] if cmd == "config" && sub == "set" && features::is_key(key) => {
            cmd_config_set_feature(key, value)
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
            let command = parse_delete_args(rest)?;
            shell_delete(command)
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

/// The one delete command's selection (S12, #135 R237–R243): one Card or tree, one tree with its
/// root kept, or the whole archive.
#[derive(Clone, Debug, Eq, PartialEq)]
enum DeleteCommand {
    At { address: String, keep_root: bool },
    All,
}

const DELETE_USAGE: &str = "usage: zt del --at <target> [--keep-root] | zt del --all";

/// Parse `--at <target> [--keep-root]` or `--all` into the one delete selection. Exactly one
/// selector is required; a repeated selector or flag, an unknown argument, both selectors, and
/// `--keep-root` beside `--all` are all refused by the one usage line (R238).
fn parse_delete_args(args: &[String]) -> Result<DeleteCommand> {
    let mut address: Option<String> = None;
    let mut keep_root = false;
    let mut all = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--at"
                if address.is_none()
                    && !all
                    && args
                        .get(index + 1)
                        .is_some_and(|value| !value.starts_with("--")) =>
            {
                address = Some(args[index + 1].clone());
                index += 2;
                continue;
            }
            "--keep-root" if !keep_root => keep_root = true,
            "--all" if !all => all = true,
            _ => bail!(DELETE_USAGE),
        }
        index += 1;
    }
    match (address, all) {
        (Some(address), false) => Ok(DeleteCommand::At { address, keep_root }),
        (None, true) if !keep_root => Ok(DeleteCommand::All),
        _ => bail!(DELETE_USAGE),
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
    /// The declared GUI switches, in `features::KEYS` order: `Some(true)` on, `Some(false)` off,
    /// `None` when no line is written (on).
    gui_search: Option<bool>,
    gui_ask: Option<bool>,
    gui_reply: Option<bool>,
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
        config.gui_search = features::declared(&raw, features::KEYS[0]);
        config.gui_ask = features::declared(&raw, features::KEYS[1]);
        config.gui_reply = features::declared(&raw, features::KEYS[2]);
        Ok(config)
    }

    /// One declared switch, by its config key.
    fn switch(&self, key: &str) -> Option<bool> {
        match key {
            "gui_search" => self.gui_search,
            "gui_ask" => self.gui_ask,
            "gui_reply" => self.gui_reply,
            _ => None,
        }
    }

    fn set_switch(&mut self, key: &str, on: bool) {
        match key {
            "gui_search" => self.gui_search = Some(on),
            "gui_ask" => self.gui_ask = Some(on),
            "gui_reply" => self.gui_reply = Some(on),
            _ => {}
        }
    }

    fn save(&self) -> Result<()> {
        let path = config_file()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        // The lines this struct owns are rewritten from its fields; every other line stays as it
        // was. A key the struct holds no value for is dropped, so `archive_root` can still be
        // unset.
        let switch_line = |key: &str, value: Option<bool>| {
            value.map(|on| format!("{key}={}\n", if on { "on" } else { "off" }))
        };
        let owned: [(&str, Option<String>); 4] = [
            (
                "archive_root",
                self.archive_root
                    .as_ref()
                    .map(|root| format!("archive_root={}\n", root.to_string_lossy())),
            ),
            (features::KEYS[0], switch_line(features::KEYS[0], self.gui_search)),
            (features::KEYS[1], switch_line(features::KEYS[1], self.gui_ask)),
            (features::KEYS[2], switch_line(features::KEYS[2], self.gui_reply)),
        ];
        let mut body = String::new();
        let mut placed = [false; 4];
        for line in fs::read_to_string(&path).unwrap_or_default().lines() {
            let name = line
                .split_once('=')
                .map(|(name, _)| name.trim())
                .unwrap_or("");
            match owned.iter().position(|(key, _)| *key == name) {
                Some(index) => {
                    if !placed[index] {
                        placed[index] = true;
                        if let Some(value) = &owned[index].1 {
                            body.push_str(value);
                        }
                    }
                }
                None => {
                    body.push_str(line);
                    body.push('\n');
                }
            }
        }
        for (index, (_, value)) in owned.iter().enumerate() {
            if !placed[index]
                && let Some(value) = value
            {
                body.push_str(value);
            }
        }
        fs::write(&path, body).with_context(|| format!("failed to write config {}", path.display()))
    }
}

pub(crate) fn config_file() -> Result<PathBuf> {
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
    match &config.archive_root {
        Some(root) => println!("archive_root={}", root.display()),
        None => println!("archive_root=<unset>"),
    }
    for key in features::KEYS {
        let value = match config.switch(key) {
            Some(true) => "on",
            Some(false) => "off",
            None => "on",
        };
        println!("{key}={value}");
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
    let mut config = Config::load()?;
    config.archive_root = Some(new_root);
    config.save()?;
    println!("archive_root={value}");
    Ok(())
}

/// Turn one GUI reader feature on or off. The switch takes effect on the running service's next
/// request, so it never needs a restart; `archive_root` alone is refused while the service is up.
fn cmd_config_set_feature(key: &str, value: &str) -> Result<()> {
    let on = features::parse_switch(value)?;
    let mut config = Config::load()?;
    config.set_switch(key, on);
    config.save()?;
    println!("{key}={}", if on { "on" } else { "off" });
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
fn independent_service_flags() -> u32 {
    use windows_sys::Win32::System::Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NO_WINDOW};
    CREATE_NO_WINDOW
        | if env::var_os("ZT_MCP_OWNED_JOB").as_deref() == Some(std::ffi::OsStr::new("1")) {
            CREATE_BREAKAWAY_FROM_JOB
        } else {
            0
        }
}

#[cfg(windows)]
fn start_independent_service_command(program: &Path) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{
            CreateProcessW, GetExitCodeProcess, PROCESS_INFORMATION, STARTUPINFOW,
            TerminateProcess, WaitForSingleObject,
        },
    };
    let script = program
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    let executable = if script {
        PathBuf::from(
            env::var_os("COMSPEC").unwrap_or_else(|| "C:\\Windows\\System32\\cmd.exe".into()),
        )
    } else {
        program.to_path_buf()
    };
    // The only argument is the established service verb. Windows paths cannot
    // contain a quote; cmd needs one extra outer pair around a quoted script.
    let line = if script {
        format!("cmd /d /s /c \"\"{}\" start\"", program.display())
    } else {
        format!("\"{}\" start", program.display())
    };
    let exe: Vec<u16> = executable
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let mut line: Vec<u16> = line.encode_utf16().chain(Some(0)).collect();
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // A shared service must inherit neither a client's job nor capture pipes.
    if unsafe {
        CreateProcessW(
            exe.as_ptr(),
            line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            independent_service_flags(),
            std::ptr::null(),
            std::ptr::null(),
            &startup,
            &mut process,
        )
    } == 0
    {
        return Err(io::Error::last_os_error())
            .context("failed to launch independent service command");
    }
    unsafe {
        CloseHandle(process.hThread);
    }
    let waited = unsafe { WaitForSingleObject(process.hProcess, 20000) };
    let result = if waited == WAIT_OBJECT_0 {
        let mut code = 1;
        if unsafe { GetExitCodeProcess(process.hProcess, &mut code) } == 0 {
            Err(io::Error::last_os_error()).context("read service-start exit status")
        } else if code != 0 {
            Err(anyhow::anyhow!(
                "independent service start exited with status {code}"
            ))
        } else {
            Ok(())
        }
    } else {
        unsafe {
            TerminateProcess(process.hProcess, 1);
            WaitForSingleObject(process.hProcess, 2000);
        }
        Err(anyhow::anyhow!(
            "independent service start exceeded its 20-second deadline"
        ))
    };
    unsafe {
        CloseHandle(process.hProcess);
    }
    result
}

#[cfg(windows)]
fn spawn_daemon(root: &Path) -> Result<Option<u32>> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
    };

    let exe: Vec<u16> = env::current_exe()?
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // Windows argv parsing doubles a run of backslashes before a closing quote.
    // Quotes themselves cannot occur in a Windows directory name.
    let path: Vec<u16> = root.as_os_str().encode_wide().collect();
    let trailing = path
        .iter()
        .rev()
        .take_while(|&&unit| unit == b'\\' as u16)
        .count();
    let mut args: Vec<u16> = "zt __daemon \"".encode_utf16().collect();
    args.extend(path);
    args.extend(std::iter::repeat_n(b'\\' as u16, trailing));
    args.extend([b'"' as u16, 0]);
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();
    // A short-lived owned MCP may restart a disappeared service. The daemon
    // has an independent lifetime; it must not join that client's kill-on-close
    // job. Ordinary starts retain their existing flags.
    let flags = independent_service_flags();
    // No inherited handles: a long-lived daemon must not keep its caller's
    // capture pipes or file locks alive. All buffers live through this call.
    let started = unsafe {
        CreateProcessW(
            exe.as_ptr(),
            args.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            flags,
            std::ptr::null(),
            std::ptr::null(),
            &startup,
            &mut process,
        )
    };
    if started == 0 {
        return Err(io::Error::last_os_error()).context("failed to start daemon");
    }
    unsafe {
        CloseHandle(process.hThread);
        CloseHandle(process.hProcess);
    }
    Ok(Some(process.dwProcessId))
}

#[cfg(unix)]
fn start_independent_service_command(program: &Path) -> Result<()> {
    use std::os::unix::process::CommandExt;
    let mut child = Command::new(program)
        .arg("start")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .context("failed to launch independent service command")?;
    let until = std::time::Instant::now() + Duration::from_secs(20);
    loop {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            bail!("independent service start exited with status {status}");
        }
        if std::time::Instant::now() >= until {
            let _ = child.kill();
            let reap = std::time::Instant::now() + Duration::from_secs(2);
            while child.try_wait()?.is_none() && std::time::Instant::now() < reap {
                thread::sleep(Duration::from_millis(10));
            }
            bail!("independent service start exceeded its 20-second deadline");
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(not(windows))]
fn spawn_daemon(root: &Path) -> Result<Option<u32>> {
    let mut command = Command::new(env::current_exe()?);
    command
        .arg("__daemon")
        .arg(root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    if env::var_os("ZT_MCP_OWNED_JOB").as_deref() == Some(std::ffi::OsStr::new("1")) {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let child = command.spawn().context("failed to start daemon")?;
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
    // R318/R319: this run's record, and what the previous run left. A previous record without
    // a normal exit record is the abnormal disappearance, reported here with every preserved fact.
    let run = begin_daemon_run(&root);
    log_event(&root, &daemon_start_line(&run))?;
    install_daemon_panic_hook(&root);
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
    // An isolated test provokes the panic hook here, after `zt up` could see the service record.
    panic_probe();
    // R318: one heartbeat a minute while a ledger-aware digest command keeps a fresh
    // active-run marker, so a disappearance during a run leaves a trace in the log.
    STARTED_AT.store(unix_seconds(), std::sync::atomic::Ordering::Relaxed);
    let heartbeat = start_daemon_heartbeat(root.clone(), run.clone());
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
    stop_daemon_heartbeat(heartbeat);
    end_daemon_run(&root, &run, ended);
    drop(lock);
    Ok(())
}

/// The daemon's record of its own run: written at start, refreshed by each heartbeat, removed by
/// a normal exit. What is left behind is the evidence of an abnormal end (R318/R319).
fn daemon_run_path(root: &Path) -> PathBuf {
    root.join("zt.daemon.run")
}

/// The retained abnormal exit of the latest run, cleared by the next normal exit.
fn daemon_last_exit_path(root: &Path) -> PathBuf {
    root.join("zt.daemon.last")
}

/// The active-run marker that ledger-aware digest commands refresh while a digest invocation is
/// active; it expires after the renewals stop (R318).
fn active_run_path(root: &Path) -> PathBuf {
    root.join("zt.digest.run")
}

fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// A field of a parsed record for a log line, or `unknown` when the record lacks it.
fn record_field(record: &serde_json::Value, name: &str) -> String {
    match record.get(name) {
        Some(serde_json::Value::String(text)) => text.clone(),
        Some(serde_json::Value::Null) | None => "unknown".to_string(),
        Some(value) => value.to_string(),
    }
}

fn read_record(path: &Path) -> Option<serde_json::Value> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Publishes a small record atomically, so a reader never sees half of it.
fn write_record(path: &Path, record: &serde_json::Value) {
    let temporary = path.with_extension("tmp");
    if fs::write(&temporary, record.to_string()).is_ok() {
        let _ = fs::rename(&temporary, path);
    }
}

/// The archive's persisted identity and current revision for a diagnostic record. Diagnostics
/// never fail because the archive is unavailable: they report what they could observe.
fn archive_facts(root: &Path) -> (String, String) {
    let opened = Connection::open_with_flags(
        db_path(root),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    );
    match opened {
        Ok(conn) => {
            let id: Option<String> = conn
                .query_row(
                    "SELECT value FROM metadata WHERE key='archive_id'",
                    [],
                    |row| row.get(0),
                )
                .ok();
            let revision = link_index::revision(&conn)
                .map(|revision| revision.to_string())
                .unwrap_or_else(|_| "unknown".to_string());
            (id.unwrap_or_else(|| "unknown".to_string()), revision)
        }
        Err(_) => ("unavailable".to_string(), "unknown".to_string()),
    }
}

/// Writes this run's record and reports a previous record that has no normal exit record. The
/// report keeps every fact the previous run left and states plainly what the OS could not tell
/// us: a force kill or a power cut leaves no exit code (R319).
fn begin_daemon_run(root: &Path) -> serde_json::Value {
    if let Some(previous) = read_record(&daemon_run_path(root)) {
        let line = format!(
            "daemon abnormal exit: pid={} version={} archive={} revision={} started={} last_heartbeat={} panicked={} exit_info=none cause=unknown",
            record_field(&previous, "pid"),
            record_field(&previous, "version"),
            record_field(&previous, "archive_id"),
            record_field(&previous, "revision"),
            record_field(&previous, "started"),
            record_field(&previous, "last_heartbeat"),
            record_field(&previous, "panicked"),
        );
        let _ = log_event(root, &line);
        write_record(&daemon_last_exit_path(root), &previous);
    }
    let (archive_id, revision) = archive_facts(root);
    let record = serde_json::json!({
        "schema_version": 1,
        "pid": std::process::id(),
        "version": VERSION,
        "archive_id": archive_id,
        "revision": revision,
        "started": unix_seconds(),
        "last_heartbeat": serde_json::Value::Null,
        "panicked": serde_json::Value::Null,
    });
    write_record(&daemon_run_path(root), &record);
    record
}

fn daemon_start_line(run: &serde_json::Value) -> String {
    format!(
        "daemon started pid={} version={} archive={} revision={}",
        record_field(run, "pid"),
        record_field(run, "version"),
        record_field(run, "archive_id"),
        record_field(run, "revision"),
    )
}

/// The daemon's normal exit paths, all of them: `zt down`, the machine ending the session, and
/// the daemon finding another service record. Each removes this run's record so the next start
/// does not mistake it for a disappearance (R318/R319).
fn end_daemon_run(root: &Path, run: &serde_json::Value, ended: Option<shutdown::SystemEnd>) {
    let reason = match ended {
        None => "requested",
        Some(shutdown::SystemEnd::LogOff) => "system-logoff",
        Some(shutdown::SystemEnd::Shutdown) => "system-shutdown",
    };
    let (_, revision) = archive_facts(root);
    let started = STARTED_AT.load(std::sync::atomic::Ordering::Relaxed);
    let uptime = unix_seconds().saturating_sub(started);
    let _ = log_event(
        root,
        &format!(
            "daemon stopped pid={} version={} archive={} revision={} exit=normal reason={reason} uptime_s={uptime}",
            record_field(run, "pid"),
            record_field(run, "version"),
            record_field(run, "archive_id"),
            revision,
        ),
    );
    let _ = fs::remove_file(daemon_run_path(root));
    let _ = fs::remove_file(daemon_last_exit_path(root));
}

/// The daemon's start time, shared with the exit line for its uptime.
static STARTED_AT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Set when the daemon is stopping, so the heartbeat thread ends before the exit record.
static DAEMON_STOP: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// One heartbeat a minute while a digest run is active (R318). An isolated test shortens it.
fn daemon_heartbeat_interval() -> Duration {
    let declared = isolated_test_switch("ZT_DAEMON_TEST_HEARTBEAT_MS")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|millis| (50..=60_000).contains(millis));
    match declared {
        Some(millis) => Duration::from_millis(millis),
        None => Duration::from_secs(60),
    }
}

/// A switch that only an isolated test host may set: a temporary configuration directory and one
/// explicit test switch. An ordinary installation can never make the daemon panic or beat faster.
fn isolated_test_switch(name: &str) -> Option<String> {
    let config_dir = env::var_os("ZT_CONFIG_DIR")?;
    let config_dir = fs::canonicalize(&config_dir).ok()?;
    let temporary = fs::canonicalize(env::temp_dir()).ok()?;
    config_dir.starts_with(&temporary)
        .then(|| env::var(name).ok())
        .flatten()
}

/// The active-run marker a ledger-aware digest command refreshed, while its renewals are fresh.
fn fresh_active_run(root: &Path) -> Option<serde_json::Value> {
    let marker = read_record(&active_run_path(root))?;
    let renewed = marker
        .get("renewed")
        .or_else(|| marker.get("renewed_unix"))
        .and_then(serde_json::Value::as_u64)?;
    let ttl = marker
        .get("ttl_s")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(300)
        .clamp(30, 3600);
    (unix_seconds() <= renewed.saturating_add(ttl)).then_some(marker)
}

fn start_daemon_heartbeat(root: PathBuf, run: serde_json::Value) -> thread::JoinHandle<()> {
    DAEMON_STOP.store(false, std::sync::atomic::Ordering::Relaxed);
    let interval = daemon_heartbeat_interval();
    thread::spawn(move || {
        while !DAEMON_STOP.load(std::sync::atomic::Ordering::Relaxed) {
            // Sleep in slices, so a stopping daemon waits at most one slice for its exit record.
            let mut slept = Duration::ZERO;
            while slept < interval && !DAEMON_STOP.load(std::sync::atomic::Ordering::Relaxed) {
                let slice = Duration::from_millis(100).min(interval - slept);
                thread::sleep(slice);
                slept += slice;
            }
            if DAEMON_STOP.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            let Some(marker) = fresh_active_run(&root) else {
                continue;
            };
            let now = unix_seconds();
            let renewed = marker
                .get("renewed")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(now);
            let (archive_id, revision) = archive_facts(&root);
            let mut current = run.clone();
            current["last_heartbeat"] = serde_json::json!(now);
            write_record(&daemon_run_path(&root), &current);
            let _ = log_event(
                &root,
                &format!(
                    "daemon heartbeat pid={} version={} archive={archive_id} revision={revision} run={} unit={} marker_age_s={}",
                    record_field(&run, "pid"),
                    record_field(&run, "version"),
                    record_field(&marker, "run_id"),
                    record_field(&marker, "unit"),
                    now.saturating_sub(renewed),
                ),
            );
        }
    })
}

fn stop_daemon_heartbeat(handle: thread::JoinHandle<()>) {
    DAEMON_STOP.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = handle.join();
}

/// R318: a panic is logged with the daemon's identity and where it happened, the run record is
/// marked as panicked, and the default hook still runs so the message is not lost.
fn install_daemon_panic_hook(root: &Path) {
    let previous = std::panic::take_hook();
    let root = root.to_path_buf();
    std::panic::set_hook(Box::new(move |info| {
        let (archive_id, revision) = archive_facts(&root);
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|text| (*text).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".to_string());
        let location = info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let _ = log_event(
            &root,
            &format!(
                "daemon panic: pid={} version={} archive={archive_id} revision={revision} message={} location={location}",
                std::process::id(),
                VERSION,
                message.replace(['\n', '\r'], " "),
            ),
        );
        if let Some(mut run) = read_record(&daemon_run_path(&root)) {
            run["panicked"] = serde_json::json!(unix_seconds());
            write_record(&daemon_run_path(&root), &run);
        }
        previous(info);
    }));
}

/// An isolated test may provoke the daemon's panic hook; an ordinary host cannot.
fn panic_probe() {
    if isolated_test_switch("ZT_DAEMON_TEST_FAULT").as_deref() == Some("panic") {
        panic!("daemon fault probe");
    }
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
    // R302: an archive that already existed is backed up before this binary first installs the
    // Tree Lock schema, and a fresh one has nothing to preserve.
    let archive_existed = db_path(root).exists();
    let conn = Connection::open(db_path(root))
        .with_context(|| format!("failed to open {}", db_path(root).display()))?;
    tree_lock::register_sql_functions(&conn)?;
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
    // R303 (S07-AC02/AC04): the generation this archive declares is validated before anything is
    // repaired or migrated, so an unsupported version or a damaged lock table changes nothing.
    tree_lock::check_generation(&conn)?;
    // R051/R304 (S07-AC05): remember which canonical protection exists now, so the triggers a Card
    // rebuild below drops with the table are re-created instead of being reported as tampering.
    let protection = tree_lock::canonical_protection(&conn)?;
    // R293: a migration here can rebuild `cards`, change row identity or drop the protection
    // triggers, so an existing promise is compared and its evidence recorded before it runs.
    {
        let _lock = acquire_edit_lock(root, "migration integrity check")?;
        tree_lock::check_before_migration(&conn)?;
    }
    ensure_workspace_column(&conn, root)?;
    ensure_card_type_column(&conn, root)?;
    ensure_model_facet_column(&conn, root)?;
    ensure_open_model_parts(&conn, root)?;
    ensure_model_level_column(&conn, root)?;
    ensure_hub_cards(&conn, root)?;
    ensure_math_form_column(&conn, root)?;
    ensure_finding_cards(&conn, root)?;
    drop(conn);
    ensure_markdown_cards(root)?;
    // The Tree Lock tables and triggers are installed after every Card-table rebuild, so a
    // migration never drops the SQL backstop it just installed. The guarded startup comparison
    // inspects the protection catalog BEFORE repairing it, so an accidentally missing or altered
    // trigger is recorded and latched rather than silently restored (R290, R296–R298).
    let conn = open_db(root)?;
    {
        let _lock = acquire_edit_lock(root, "startup integrity check")?;
        // R051/R304 (S07-AC05): the rebuilds above dropped the triggers that belong to `cards`.
        // They are re-created here, after the final rename and before the guarded comparison, so an
        // authorized rebuild is not recorded as tampering. A trigger already missing before the
        // rebuild keeps its recorded evidence and is left to the guarded comparison.
        let restored = tree_lock::restore_rebuilt_protection(&conn, &protection)?;
        if !restored.is_empty() {
            log_event(
                root,
                &format!(
                    "tree lock rebuild: protection reinstalled after the Card rebuild: {}",
                    restored.join(", ")
                ),
            )
            .ok();
        }
        // R302 (S07-AC01): one consistent SQLite backup before the first installation. It is taken
        // before the guarded transaction, so a failed installation still leaves it for rollback.
        if let Some(backup) = tree_lock::backup_before_install(&conn, root, archive_existed)? {
            log_event(
                root,
                &format!("installing tree lock protection; backup {}", backup.display()),
            )
            .ok();
        }
        let guard = tree_lock::startup_guard(&conn)?;
        let integrity = &guard.integrity;
        if guard.recorded > 0 || !guard.repaired.is_empty() {
            log_event(
                root,
                &format!(
                    "tree lock startup: {} new fault event(s) recorded; protection reinstalled: {}",
                    guard.recorded,
                    if guard.repaired.is_empty() {
                        "none".to_string()
                    } else {
                        guard.repaired.join(", ")
                    }
                ),
            )
            .ok();
        }
        if !integrity.ok() {
            log_event(root, &format!("tree lock {}", integrity.summary())).ok();
        }
    }
    drop(conn);
    Ok(())
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
        return link_index::ensure(root, &mut conn);
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
    link_index::ensure(root, &mut conn)
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
    println!("revision: {}", archive_facts(&root).1);
    if let Some(run) = read_record(&daemon_run_path(&root)) {
        println!(
            "daemon: version={} started={} last_heartbeat={}",
            record_field(&run, "version"),
            record_field(&run, "started"),
            record_field(&run, "last_heartbeat"),
        );
    }
    // R319: the last run that ended without a normal exit record, with its preserved facts. A
    // normal exit clears this, so an operator sees the abnormal end until it is explained.
    if let Some(previous) = read_record(&daemon_last_exit_path(&root)) {
        println!(
            "last_exit: abnormal pid {} version {} revision {} started {} panicked {} (no normal exit record; see zt.log)",
            record_field(&previous, "pid"),
            record_field(&previous, "version"),
            record_field(&previous, "revision"),
            record_field(&previous, "started"),
            record_field(&previous, "panicked"),
        );
    }
    println!(
        "cards: {}",
        card_counts(&root).map(|c| c.total).unwrap_or(0)
    );
    println!("{}", lock_status_line(&root));
    println!("sessions: {}", session::session_count(&root));
    if let Some(line) = autostart::status_line() {
        println!("{line}");
    }
    Ok(())
}

/// The `locks:` line of ordinary status (R233–R235, S08-AC04): the active Locked trees, their member
/// snapshot rows and the distinct pinned target rows, with the full comparison that alone licenses
/// the words `integrity ok`. A database or read error is reported as unavailable, never converted
/// into a healthy zero-Lock result (R235).
fn lock_status_line(root: &Path) -> String {
    if !db_path(root).exists() {
        return "locks: unavailable (no archive database yet)".to_string();
    }
    read_lock_status(root).unwrap_or_else(|error| format!("locks: unavailable ({error:#})"))
}

fn read_lock_status(root: &Path) -> Result<String> {
    let conn = open_db(root)?;
    let scope = tree_lock::scope(&conn)?;
    let integrity = tree_lock::check(&conn)?;
    Ok(scope.line(&integrity))
}

/// `zt status --check-locks` (R291, S06-AC06): the full integrity comparison, with any newly
/// observed fault recorded once under the edit lock, then exit 1 on a current or latched fault of an
/// active Lock. A healthy archive is only read. A run that finds a fault but cannot hold the edit
/// lock says so and also exits 1: it may not report a health it did not record (R295). The status
/// check never queues behind a writer; rerun it when the writer is done. It first prints the active
/// Lock scope as its own `locks:` line (R236, S08-AC05).
fn cmd_status_check_locks() -> Result<()> {
    let config = Config::load()?;
    let Some(root) = config.archive_root else {
        println!("state: down");
        println!("archive_root: <unset>");
        println!("integrity: unavailable (archive_root is unset)");
        std::process::exit(1);
    };
    println!("archive_root: {}", root.display());
    let checked = (|| -> Result<(tree_lock::Scope, tree_lock::Integrity, usize, tree_lock::Integrity)> {
        let conn = open_db(&root)?;
        let scope = tree_lock::scope(&conn)?;
        let integrity = tree_lock::check(&conn)?;
        if !integrity.needs_recording() {
            // Nothing new to store: a latch is already stored evidence, so one comparison answers
            // both what was found and what the panel will show.
            let after = integrity.clone();
            return Ok((scope, integrity, 0, after));
        }
        let _lock = acquire_edit_lock(&root, "status --check-locks")?;
        let tx = conn.unchecked_transaction()?;
        let fresh = tree_lock::check(&tx)?;
        let recorded = tree_lock::record_faults_tx(&tx, &fresh)?;
        tx.commit()?;
        // The latches the service will show include the events this run just wrote.
        let after = tree_lock::check(&conn)?;
        Ok((scope, fresh, recorded, after))
    })();
    let (scope, integrity, recorded, after) = match checked {
        Ok(checked) => checked,
        Err(error) => {
            println!("integrity: unavailable ({error:#})");
            println!("recording: failed; no fault evidence was stored");
            std::process::exit(1);
        }
    };
    println!("{}", scope.counts());
    println!("integrity: {}", if after.ok() { "ok" } else { "faulted" });
    println!("recorded: {recorded} new fault event(s)");
    for fault in &integrity.faults {
        println!("fault: {}", fault_line(fault));
    }
    for latched in &after.latched {
        println!(
            "latched: lock {} {} {} recorded {}",
            latched.lock_id,
            latched.kind,
            latched.card.as_deref().unwrap_or("-"),
            latched.at
        );
    }
    // A latched fault stays unhealthy until the owner unlocks or relocks, so the exit code follows
    // the same rule the panel does (R289, S06-AC05).
    if !after.ok() {
        std::process::exit(1);
    }
    Ok(())
}

/// One current fault as a single line, with the expected and observed fact that explains it.
fn fault_line(fault: &tree_lock::Fault) -> String {
    let scope = match fault.lock_id {
        Some(lock_id) => format!("lock {lock_id}"),
        None => "schema".to_string(),
    };
    let mut line = format!("{scope} {} {}: {}", fault.kind, fault.subject, fault.detail);
    if let Some(expected) = &fault.expected {
        line.push_str(&format!("; expected {expected}"));
    }
    if let Some(actual) = &fault.actual {
        line.push_str(&format!("; found {actual}"));
    }
    line
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
    let _lock = acquire_edit_lock(&root, "dump")?;
    let conn = open_db(&root)?;
    // One captured view under the edit lock: the Cards, the next Topic id, the archive identity and
    // revision, and the Lock history with its integrity evidence all describe the same archive state
    // (R314, S13-AC01). The dump module only serializes what this view read.
    let view = conn.unchecked_transaction()?;
    let cards = load_cards(&view)?
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
    let next_topic_id = next_topic_id(&view)?;
    let locks = tree_lock::dump_export(&view)?;
    let _ = view.rollback();
    dump::run(&root, &cards, next_topic_id, &locks)
}

/// `zt clear` is now `zt del --all` (R245). The notice needs no service, asks nothing and deletes
/// nothing; the next release removes the handler and leaves an unknown command.
fn cmd_clear() -> Result<()> {
    bail!("zt clear is now `zt del --all`")
}

fn cmd_help() -> Result<()> {
    let up = Config::load()?
        .archive_root
        .as_ref()
        .map(|root| is_service_up(root))
        .unwrap_or(false);
    println!("zt commands:");
    println!("  zt status --check-links");
    println!("  zt status --check-locks");
    println!("  zt reindex");
    println!("  zt up");
    println!("  zt down");
    println!("  zt status");
    println!("  zt autostart [on|off]");
    println!("  zt version");
    println!("  zt help");
    println!("  zt config show");
    println!("  zt config set archive_root <path>");
    println!("  zt config set gui_search|gui_ask|gui_reply on|off");
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
        println!("  zt del --at <tree-root> --keep-root");
        println!("  zt del --all");
        println!("  zt mv --at <location> [--alone] <new-location>");
        println!("  zt mv --at <location> [--alone] --before <location>");
        println!("  zt ins --at <location>");
        println!("  zt stats");
        println!("  zt ws");
        println!("  zt mcp");
        println!("  zt lsbk");
        println!("  zt dp");
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
    let conn = open_db(root)?;
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
    let conn = Connection::open(db_path(root))
        .with_context(|| format!("failed to open {}", db_path(root).display()))?;
    tree_lock::register_sql_functions(&conn)?;
    Ok(conn)
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
    let authored_changed = old
        .as_ref()
        .is_none_or(|before| before.title != parsed.title || before.body != parsed.body);
    if authored_changed && card::body_starts_with_title(&parsed.title, &parsed.body) {
        bail!("duplicate leading title in Card body; keep the title only in its Card heading");
    }
    link_index::validate(
        conn,
        &parsed.body,
        old.as_ref().map(|card| card.body.as_str()),
    )?;
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

fn card_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Card> {
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
}

fn load_card(conn: &Connection, target: &str) -> Result<Card> {
    conn.query_row(
        "SELECT row_id, COALESCE(location, citation_key), citation_key,
                is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form
         FROM cards WHERE location = ?1 OR citation_key = ?1",
        [target],
        card_from_row,
    )
    .with_context(|| format!("card `{target}` does not exist"))
}

fn load_cards(conn: &Connection) -> Result<Vec<Card>> {
    let mut stmt = conn.prepare(
        "SELECT row_id, COALESCE(location, citation_key), citation_key,
                is_topic, is_lit, bibtex, text, workspace, card_type, model_facet, model_emphasis, model_level, hub_level, math_form
         FROM cards ORDER BY COALESCE(location, citation_key)",
    )?;
    let rows = stmt.query_map([], card_from_row)?;
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
    let _lock = acquire_edit_lock(&root, "create_literature")?;
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
    let tx = conn.transaction()?;
    insert_literature_card_tx(&tx, citation_key, workspace, bibtex, text)?;
    link_index::source_changed(&tx, citation_key)?;
    tx.commit()?;
    Ok(())
}

fn insert_literature_card_tx(
    conn: &Connection,
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
    tree_lock::ensure_unlocked(conn, tree_lock::Change::CreateAt(citation_key))?;
    let parsed = parse_card_text(text)?;
    let text = compose_card_text(&parsed.title, &parsed.body, &parsed.reverse);
    conn.execute(
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
    Ok(())
}

/// The first line of an edit lock file whose holder also keeps the file's OS lock.
const EDIT_LOCK_MARKER: &str = "zt edit lock";

/// A wait longer than this is worth one diagnostic line; the poll loop itself stays silent.
const EDIT_WAIT_LOG_AFTER: Duration = Duration::from_secs(5);

/// A hold longer than this is worth one line that attributes the write to its queue wait and
/// its lock-held duration, so a slow apply can be told apart from a slow wait (R317).
const EDIT_HOLD_LOG_AFTER: Duration = Duration::from_secs(5);

/// What the diagnostic line can observe of the process that keeps the edit lock: the marker and
/// pid of a zt holder, the marker of an older zt that kept no OS lock, or only the marker line.
fn observed_holder(text: &str) -> String {
    if !text.starts_with(EDIT_LOCK_MARKER) {
        return "legacy-marker".to_string();
    }
    match text.lines().find_map(|line| line.trim().strip_prefix("pid ")) {
        Some(pid) if !pid.is_empty() => format!("pid:{pid}"),
        _ => "marker-only".to_string(),
    }
}

/// Why an edit-lock attempt found the lock held: contention that is worth waiting for, as
/// distinct from every other failure (R314).
#[derive(Debug)]
enum EditConflict {
    /// The lock file of another holder is there; `holder` is what could be observed of it.
    Held { holder: String },
    /// The operating system's contention error while opening, reading or removing the lock
    /// file: Windows `ERROR_SHARING_VIOLATION` (os error 32) or the byte-range
    /// `ERROR_LOCK_VIOLATION` (os error 33) that a live holder's `LockFileEx` produces. The
    /// holder that could still be observed is kept, so the line names both facts.
    Sharing {
        code: i32,
        step: &'static str,
        holder: String,
    },
}

impl EditConflict {
    fn holder(&self) -> String {
        match self {
            EditConflict::Held { holder } => holder.clone(),
            EditConflict::Sharing { code, step, holder } => {
                format!("{holder}({step} refused with os error {code})")
            }
        }
    }
}

/// The operating system's contention codes. Only Windows defines them as sharing/lock
/// violations; elsewhere the same numbers mean unrelated errors, so they are never contention.
fn contention_code(error: &io::Error) -> Option<i32> {
    if !cfg!(windows) {
        return None;
    }
    match error.raw_os_error() {
        Some(code @ (32 | 33)) => Some(code),
        _ => None,
    }
}

/// A refused or failed edit-lock attempt. Contention is retryable within the patience; every
/// other error is a fault that is reported instead of being mistaken for a busy lock (R314).
enum EditLockError {
    Busy(EditConflict, Duration),
    Fault(anyhow::Error),
}

#[derive(Debug)]
struct EditBusy {
    operation: &'static str,
    holder: String,
    waited: Duration,
    patience: Option<Duration>,
}

impl std::fmt::Display for EditBusy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "edit in progress: {} is held by {}",
            self.operation, self.holder
        )?;
        if !self.waited.is_zero() {
            write!(f, " after {} ms of waiting", self.waited.as_millis())?;
        }
        if let Some(patience) = self.patience {
            write!(f, " (patience {} s)", patience.as_secs())?;
        }
        Ok(())
    }
}
impl std::error::Error for EditBusy {}

/// The edit lock: `zt.edit.lock`, created new by its holder and removed when the holder is done.
/// The holder also keeps the file's OS lock, which the system releases however the holder ends,
/// so the file of a holder that was killed is found stale and taken over. A lock file without
/// the marker comes from an older zt, which keeps no OS lock; it counts as held. The short-lived
/// `zt.edit.guard` lock orders taking over and removing, so no two processes hold the edit lock.
struct EditLock {
    root: PathBuf,
    file: Option<fs::File>,
    /// The operation the lock was taken for, so a slow hold names what it slowed down.
    operation: &'static str,
    /// How long the acquisition waited for another holder before it got the lock.
    queue_wait: Duration,
    acquired: Instant,
}

impl Drop for EditLock {
    fn drop(&mut self) {
        // Remove the file before its OS lock is released, under the guard, so no process can
        // find it free and take it over in between.
        let guard = lock_edit_guard(&self.root).ok();
        fs::remove_file(edit_lock_path(&self.root)).ok();
        drop(self.file.take());
        drop(guard);
        let held = self.acquired.elapsed();
        // One line per slow wait or slow hold (R316/R317): the fast path stays silent, and the
        // line records both durations so a slow apply is attributable to waiting or to writing.
        let line = if self.queue_wait > EDIT_WAIT_LOG_AFTER {
            Some(format!(
                "edit wait done: op={} waiter_pid={} outcome=acquired queue_wait_ms={} held_ms={}",
                self.operation,
                std::process::id(),
                self.queue_wait.as_millis(),
                held.as_millis()
            ))
        } else if held > EDIT_HOLD_LOG_AFTER {
            Some(format!(
                "edit lock held: op={} waiter_pid={} queue_wait_ms={} held_ms={}",
                self.operation,
                std::process::id(),
                self.queue_wait.as_millis(),
                held.as_millis()
            ))
        } else {
            None
        };
        if let Some(line) = line {
            let _ = log_event(&self.root, &line);
        }
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

/// Takes the edit lock at once, or reports why it could not: contention, which another client
/// may wait out, or a fault, which waiting cannot fix. The caller holds the guard across the
/// stale-file takeover and the caller's own `EditLock` drop holds it across removal.
fn try_acquire_edit_lock(
    root: &Path,
    operation: &'static str,
) -> std::result::Result<EditLock, EditLockError> {
    let path = edit_lock_path(root);
    let _guard = lock_edit_guard(root).map_err(EditLockError::Fault)?;
    // A second try follows only after a stale lock file was removed.
    for attempt in 0..2 {
        let opened = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path);
        match opened {
            Ok(mut file) => {
                let marked = writeln!(file, "{EDIT_LOCK_MARKER}\npid {}", std::process::id())
                    .and_then(|()| file.lock())
                    .with_context(|| format!("failed to lock {}", path.display()));
                return match marked {
                    Ok(()) => Ok(EditLock {
                        root: root.to_path_buf(),
                        file: Some(file),
                        operation,
                        queue_wait: Duration::ZERO,
                        acquired: Instant::now(),
                    }),
                    Err(error) => Err(EditLockError::Fault(error)),
                };
            }
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                match remove_stale_edit_lock(&path) {
                    Ok(StaleLock::Removed) if attempt == 0 => continue,
                    Ok(StaleLock::Removed) => {
                        return Err(EditLockError::Fault(anyhow::anyhow!(
                            "the edit lock file {} could not be taken over",
                            path.display()
                        )));
                    }
                    Ok(StaleLock::Held(conflict)) => return Err(EditLockError::Busy(conflict, Duration::ZERO)),
                    Err(EditLockError::Busy(conflict, waited)) => {
                        return Err(EditLockError::Busy(conflict, waited));
                    }
                    Err(EditLockError::Fault(error)) => return Err(EditLockError::Fault(error)),
                }
            }
            Err(err) => {
                return match contention_code(&err) {
                    Some(code) => Err(EditLockError::Busy(
                        EditConflict::Sharing {
                            code,
                            step: "create",
                            holder: "unobserved".to_string(),
                        },
                        Duration::ZERO,
                    )),
                    None => Err(EditLockError::Fault(
                        anyhow::Error::new(err).context(format!("failed to create {}", path.display())),
                    )),
                };
            }
        }
    }
    Err(EditLockError::Fault(anyhow::anyhow!(
        "the edit lock file {} could not be taken over",
        path.display()
    )))
}

/// Whether a held lock file was removed, or what keeps it: the caller holds the guard.
enum StaleLock {
    Removed,
    Held(EditConflict),
}

/// Removes the lock file of a holder that ended without removing it: the file has the marker
/// and nobody keeps its OS lock. A file that cannot be read, opened or removed for any reason
/// other than contention is a fault, not a busy lock (R314).
fn remove_stale_edit_lock(path: &Path) -> std::result::Result<StaleLock, EditLockError> {
    let mut file = match fs::OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(StaleLock::Removed),
        Err(err) => return Err(classify_lock_step(err, "open", path, "unobserved")),
    };
    // Read before taking the file's lock: a live holder's byte-range lock makes its own marker
    // unreadable, which is itself the observation that the holder is alive.
    let mut text = String::new();
    match io::Read::read_to_string(&mut file, &mut text) {
        Ok(_) => {}
        Err(err) if err.kind() == io::ErrorKind::InvalidData => {
            return Err(EditLockError::Fault(anyhow::anyhow!(
                "malformed edit lock file {}: it is not UTF-8 text, so it cannot be classified as a live or a stale holder; remove it by hand if no edit is open",
                path.display()
            )));
        }
        Err(err) => return Err(classify_lock_step(err, "read", path, "unreadable")),
    }
    let holder = observed_holder(&text);
    if file.try_lock().is_err() {
        return Ok(StaleLock::Held(EditConflict::Held { holder }));
    }
    if !text.starts_with(EDIT_LOCK_MARKER) {
        return Ok(StaleLock::Held(EditConflict::Held { holder }));
    }
    // The guard serializes stale cleanup; release this handle before Windows deletion.
    drop(file);
    match fs::remove_file(path) {
        Ok(()) => Ok(StaleLock::Removed),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(StaleLock::Removed),
        Err(err) => Err(classify_lock_step(err, "remove", path, &holder)),
    }
}

/// Times out of a wait for another client's short write instead of refusing at once. A Session
/// holds the lock while its editor is open, so after `patience` the refusal stands. A fault is
/// reported at once: waiting cannot repair it.
fn acquire_edit_lock_within(
    root: &Path,
    patience: Duration,
    operation: &'static str,
) -> Result<EditLock> {
    let start = Instant::now();
    let mut pause = Duration::from_millis(20);
    let mut logged = false;
    let mut last_holder = String::new();
    loop {
        match try_acquire_edit_lock(root, operation) {
            Ok(mut lock) => {
                let waited = start.elapsed();
                lock.queue_wait = waited;
                lock.acquired = Instant::now();
                // The last poll of a wait can succeed more than five seconds after the previous
                // refusal, so the crossing is also reported when the lock arrives: one wait
                // event, never one per poll, and never a wait that waited silently.
                if !logged && waited > EDIT_WAIT_LOG_AFTER {
                    let held_by = if last_holder.is_empty() { "unknown" } else { last_holder.as_str() };
                    let _ = log_event(
                        root,
                        &format!(
                            "edit wait: op={operation} waiter_pid={} elapsed_ms={} patience_s={} holder={held_by}",
                            std::process::id(),
                            waited.as_millis(),
                            patience.as_secs()
                        ),
                    );
                }
                return Ok(lock);
            }
            Err(EditLockError::Fault(error)) => return Err(error),
            Err(EditLockError::Busy(conflict, _)) => {
                let waited = start.elapsed();
                let holder = conflict.holder();
                last_holder = holder.clone();
                if waited >= patience {
                    // One timeout line, with the wait and the patience it overran.
                    let _ = log_event(
                        root,
                        &format!(
                            "edit wait done: op={operation} waiter_pid={} outcome=timeout waited_ms={} patience_s={} holder={holder}",
                            std::process::id(),
                            waited.as_millis(),
                            patience.as_secs()
                        ),
                    );
                    return Err(EditBusy {
                        operation,
                        holder,
                        waited,
                        patience: Some(patience),
                    }
                    .into());
                }
                if !logged && waited > EDIT_WAIT_LOG_AFTER {
                    // One wait event after five seconds, never one line per poll.
                    let _ = log_event(
                        root,
                        &format!(
                            "edit wait: op={operation} waiter_pid={} elapsed_ms={} patience_s={} holder={holder}",
                            std::process::id(),
                            waited.as_millis(),
                            patience.as_secs()
                        ),
                    );
                    logged = true;
                }
                thread::sleep(pause);
                pause = (pause * 2).min(Duration::from_millis(400));
            }
        }
    }
}

/// Turns a lock-file IO error into contention or a fault that names the file and the step.
fn classify_lock_step(
    err: io::Error,
    step: &'static str,
    path: &Path,
    holder: &str,
) -> EditLockError {
    match contention_code(&err) {
        Some(code) => EditLockError::Busy(
            EditConflict::Sharing {
                code,
                step,
                holder: holder.to_string(),
            },
            Duration::ZERO,
        ),
        None => EditLockError::Fault(
            anyhow::Error::new(err).context(format!("failed to {step} {}", path.display())),
        ),
    }
}

/// Takes the edit lock for a shell command or a Session edit, refusing at once when it is held.
fn acquire_edit_lock(root: &Path, operation: &'static str) -> Result<EditLock> {
    match try_acquire_edit_lock(root, operation) {
        Ok(lock) => Ok(lock),
        Err(EditLockError::Fault(error)) => Err(error),
        Err(EditLockError::Busy(conflict, waited)) => Err(EditBusy {
            operation,
            holder: conflict.holder(),
            waited,
            patience: None,
        }
        .into()),
    }
}

fn shell_create_topic(workspace: Workspace, title: &str) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root, "create_topic")?;
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
    let _lock = acquire_edit_lock(&root, "create_direct")?;
    let mut conn = open_db(&root)?;
    let parent = load_card(&conn, at)?;
    let location = direct_successor(&parent.address)?;
    if location_exists(&conn, &location)? {
        bail!("direct successor already exists: {location}");
    }
    tree_lock::ensure_unlocked(&conn, tree_lock::Change::CreateAt(&location))?;
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
    let _lock = acquire_edit_lock(&root, "create_side")?;
    let mut conn = open_db(&root)?;
    let parent = load_card(&conn, at)?;
    ensure_side_parent(&parent)?;
    let location = next_side_successor(&conn, &parent.address, &BTreeSet::new())?;
    tree_lock::ensure_unlocked(&conn, tree_lock::Change::CreateAt(&location))?;
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
    let _lock = acquire_edit_lock(&root, "edit")?;
    let mut conn = open_db(&root)?;
    let card = load_card(&conn, at)?;
    // The Lock refuses before an editor opens; the transaction writers check again.
    tree_lock::ensure_unlocked(&conn, tree_lock::Change::Edit(card.row_id))?;
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
    tree_lock::ensure_unlocked(conn, tree_lock::Change::Edit(card.row_id))?;
    let metadata = literature::parse_metadata(
        card.bibtex
            .as_deref()
            .context("Literature Card is missing its BibTeX metadata")?,
    )?;
    let parsed = parse_literature_edit_text(edited)?;
    let text = compose_card_text(&metadata.title, &parsed.body, "");
    validate_card_text(conn, &card.address, &text, Some(&card.text))?;
    let stored = parse_card_text(&card.text)?;
    if parsed.body == stored.body && metadata.title == stored.title {
        return Ok(());
    }
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE cards SET text = ?1 WHERE row_id = ?2",
        params![text, card.row_id],
    )?;
    link_index::source_changed(&tx, &card.address)?;
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
    tree_lock::ensure_unlocked(conn, tree_lock::Change::Edit(card.row_id))?;
    let parsed = parse_card_text(&card.text)?;
    if citation_key == card.address
        && card.bibtex.as_deref() == Some(bibtex)
        && title == parsed.title
    {
        return Ok(());
    }
    let text = compose_card_text(title, &parsed.body, "");
    let tx = conn.transaction()?;
    let mapping = if citation_key != card.address {
        citation_rename_mapping(&tx, &card.address, citation_key)?
    } else {
        BTreeMap::new()
    };
    tree_lock::ensure_unlocked(&tx, tree_lock::Change::Mapping(&mapping))?;
    tx.execute(
        "UPDATE cards SET citation_key = ?1, bibtex = ?2, text = ?3 WHERE row_id = ?4",
        params![citation_key, bibtex, text, card.row_id],
    )?;
    apply_location_mapping_tx(&tx, &mapping)?;
    if mapping.is_empty() {
        link_index::source_changed(&tx, citation_key)?;
    } else {
        regenerate_reverse_links_tx(&tx)?;
    }
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
    let tx = conn.transaction()?;
    insert_card_tx(&tx, location, is_topic, workspace, card_type, part, text)?;
    link_index::source_changed(&tx, location)?;
    tx.commit()?;
    Ok(())
}

/// Validate and insert within a caller-owned transaction. The caller finalizes
/// indexed sources, generated sections and the revision once before commit.
fn insert_card_tx(
    conn: &Connection,
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
    tree_lock::ensure_unlocked(conn, tree_lock::Change::CreateAt(location))?;
    let parsed = parse_card_text(text)?;
    let text = compose_card_text(&parsed.title, &parsed.body, &parsed.reverse);
    conn.execute(
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
    if is_topic {
        bump_next_topic_id(conn, location)?;
    }
    Ok(())
}

/// Create a Topic in `workspace` at the next Topic number.
fn create_topic(conn: &mut Connection, workspace: Workspace, text: &str) -> Result<String> {
    let location = next_topic_location(conn)?;
    insert_card(conn, &location, true, workspace, CardType::Idea, None, text)?;
    Ok(location)
}

fn update_card_text(conn: &mut Connection, card: &Card, text: &str) -> Result<()> {
    tree_lock::ensure_unlocked(conn, tree_lock::Change::Edit(card.row_id))?;
    validate_card_text(conn, &card.address, text, Some(&card.text))?;
    let before = parse_card_text(&card.text)?;
    let after = parse_card_text(text)?;
    if before.title == after.title && before.body == after.body {
        return Ok(());
    }
    let text = compose_card_text(&after.title, &after.body, &after.reverse);
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE cards SET text = ?1 WHERE row_id = ?2",
        params![text, card.row_id],
    )?;
    link_index::source_changed(&tx, &card.address)?;
    tx.commit()?;
    Ok(())
}

fn regenerate_reverse_links_tx(conn: &Connection) -> Result<()> {
    if link_index::ready(conn) {
        link_index::rebuild(conn)?;
        return link_index::bump_revision(conn);
    }
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

fn shell_delete(command: DeleteCommand) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root, "delete")?;
    // R270–R271: the whole-archive delete holds the Session-registration lock from the active-Session
    // check through its commit, so a Session that starts meanwhile waits to register. The edit lock
    // is always taken first, and Session registration never waits for the edit lock.
    let sessions = match command {
        DeleteCommand::All => Some(session::hold_registration(&root)?),
        DeleteCommand::At { .. } => None,
    };
    if let Some(guard) = &sessions {
        let open = guard.prune()?;
        if !open.is_empty() {
            bail!("cannot delete all while {} session(s) are open", open.len());
        }
    }
    let mut conn = open_db(&root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let plan = match &command {
        DeleteCommand::At { address, keep_root } => plan_delete(&conn, address, *keep_root)?,
        DeleteCommand::All => plan_delete_all(&conn)?,
    };
    // R244: the Lock check happens before the prompt, so no confirmation word is typed for a
    // deletion the archive already forbids.
    tree_lock::ensure_unlocked(&conn, plan.gate())?;
    print_delete_verification(&plan);
    read_confirmation(&mut input, &plan.confirmation)?;
    let result = apply_delete_plan(&mut conn, plan)?;
    println!("{}", delete_success_line(&result));
    if !result.moved.is_empty() {
        println!("compacted:");
        for (old, new) in result.moved {
            println!("{old} -> {new}");
        }
    }
    // `sessions` stays held through the commit above and releases when this function returns.
    Ok(())
}

/// The exact success line of a delete: `deleted: N`, for an emptied tree
/// `deleted: N; root kept: <root>` (R261), and for the whole archive
/// `deleted: N cards; next topic id reset to 0` (R274).
fn delete_success_line(result: &DeleteResult) -> String {
    match (&result.root_kept, result.next_topic_reset) {
        (Some(root), _) => format!("deleted: {}; root kept: {root}", result.deleted_count),
        (None, true) => format!(
            "deleted: {} cards; next topic id reset to 0",
            result.deleted_count
        ),
        (None, false) => format!("deleted: {}", result.deleted_count),
    }
}

struct DeleteResult {
    deleted_count: usize,
    moved: BTreeMap<String, String>,
    /// The Tree root a keep-root delete retained.
    root_kept: Option<String>,
    /// Whether this delete reset the Topic allocation to zero (the whole-archive delete).
    next_topic_reset: bool,
}

/// What a delete selects: one Card's successors, a whole tree, one tree's Regular Cards with its
/// root kept (S11, #134), or every Card of the archive (S12, #135).
#[derive(Clone, Debug, Eq, PartialEq)]
enum DeleteScope {
    /// The Card and its successors.
    Subtree,
    /// Every Card of the tree, its root included.
    Tree,
    /// Every Card of the tree except its root, which keeps its address and authored identity.
    KeepRoot { root: String, workspace: Workspace },
    /// Every Card of the archive, with the Topic allocation the delete resets (R264–R275).
    All { next_topic_id: u64 },
}

struct DeletePlan {
    scope: DeleteScope,
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
    /// The application gate this plan must pass before it prompts and again inside its
    /// transaction. The whole-archive delete refuses on any active Lock record, so an orphaned or
    /// damaged Locked tree still protects its promise (R272).
    fn gate(&self) -> tree_lock::Change<'_> {
        match self.scope {
            DeleteScope::All { .. } => tree_lock::Change::DeleteAll,
            DeleteScope::Subtree | DeleteScope::Tree | DeleteScope::KeepRoot { .. } => {
                tree_lock::Change::Delete(self)
            }
        }
    }

    /// The root a keep-root plan retains, with its Workspace.
    fn kept_root(&self) -> Option<(&str, Workspace)> {
        match &self.scope {
            DeleteScope::KeepRoot { root, workspace } => Some((root.as_str(), *workspace)),
            DeleteScope::Subtree | DeleteScope::Tree | DeleteScope::All { .. } => None,
        }
    }

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
    plan_delete(conn, at, false)
}

/// Emptying a Tree keeps its root: every other member goes, and the root's own
/// address confirms the request (S11, #134).
fn keep_root_delete_plan(conn: &Connection, at: &str) -> Result<DeletePlan> {
    plan_delete(conn, at, true)
}

fn plan_delete(conn: &Connection, at: &str, keep_root: bool) -> Result<DeletePlan> {
    let card = load_card(conn, at)?;
    let is_tree_root = is_tree_root(&card);
    if keep_root && !is_tree_root {
        bail!("keep root takes a Tree root (a Topic like 3/0 or a Citation key); `{at}` is a Regular Card");
    }
    let tree = tree_id(&card.address);
    let members = |conn: &Connection| -> Result<Vec<String>> {
        Ok(load_cards(conn)?
            .into_iter()
            .filter(|member| is_tree_member(&member.address, tree))
            .map(|member| member.address)
            .collect())
    };
    let (scope, delete_set) = if keep_root {
        (
            DeleteScope::KeepRoot {
                root: card.address.clone(),
                workspace: card.workspace,
            },
            members(conn)?
                .into_iter()
                .filter(|address| *address != card.address)
                .collect::<BTreeSet<_>>(),
        )
    } else if is_tree_root {
        (DeleteScope::Tree, members(conn)?.into_iter().collect())
    } else {
        (
            DeleteScope::Subtree,
            descendant_locations(conn, &card.address)?,
        )
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
        scope,
        delete_set,
        confirmation,
        inbound,
        into: None,
        side_compaction,
    })
}

/// The whole-archive delete selects every Card and resets the Topic allocation to zero; its
/// confirmation is `delete all` (S12, #135 R237–R243, R264–R275).
fn plan_delete_all(conn: &Connection) -> Result<DeletePlan> {
    Ok(DeletePlan {
        scope: DeleteScope::All {
            next_topic_id: next_topic_id(conn)?,
        },
        delete_set: load_cards(conn)?
            .into_iter()
            .map(|card| card.address)
            .collect(),
        confirmation: "delete all".to_string(),
        inbound: Vec::new(),
        into: None,
        side_compaction: BTreeMap::new(),
    })
}

fn delete_verification_lines(plan: &DeletePlan) -> Vec<String> {
    if let Some((root, workspace)) = plan.kept_root() {
        return keep_root_verification_lines(plan, root, workspace);
    }
    if let DeleteScope::All { next_topic_id } = plan.scope {
        return vec![
            "delete verification:".to_string(),
            format!("cards: {}", plan.delete_set.len()),
            format!("next topic id: {next_topic_id} -> 0"),
        ];
    }
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
                lines.push(format!(
                    "links re-pointed to {into}: {}",
                    plan.inbound.len()
                ));
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

/// How many breaking-Link pairs the keep-root plan prints before it states the exact remainder.
const BREAKING_LINK_PREVIEW: usize = 20;

/// The keep-root plan identifies the retained root, its Workspace, how many Regular Cards go,
/// and every Link from a staying Card that will break (R251).
fn keep_root_verification_lines(
    plan: &DeletePlan,
    root: &str,
    workspace: Workspace,
) -> Vec<String> {
    let kind = if is_valid_topic_location(root) {
        "Topic"
    } else {
        "Literature"
    };
    let mut lines = vec![
        "delete verification:".to_string(),
        format!("root kept: {root} ({kind}, {workspace} workspace)"),
        format!("delete regular cards: {}", plan.delete_set.len()),
    ];
    if plan.inbound.is_empty() {
        return lines;
    }
    lines.push(format!("links that will break: {}", plan.inbound.len()));
    for (source, target) in plan.inbound.iter().take(BREAKING_LINK_PREVIEW) {
        lines.push(format!("{source} -> {target}"));
    }
    if plan.inbound.len() > BREAKING_LINK_PREVIEW {
        lines.push(format!(
            "... and {} more",
            plan.inbound.len() - BREAKING_LINK_PREVIEW
        ));
    }
    lines
}

fn print_delete_verification(plan: &DeletePlan) {
    for line in delete_verification_lines(plan) {
        println!("{line}");
    }
}

fn apply_delete_plan(conn: &mut Connection, plan: DeletePlan) -> Result<DeleteResult> {
    if let DeleteScope::All { next_topic_id } = plan.scope {
        return apply_delete_all_plan(conn, plan, next_topic_id);
    }
    let root_kept = plan.kept_root().map(|(root, _)| root.to_string());
    let compaction = plan.compaction();
    let repoint = plan.repoint_mapping();
    if plan.delete_set.is_empty() && repoint.is_empty() && compaction.is_empty() {
        // An already empty unlocked Tree: the owner still confirmed the plan, and nothing is
        // written, so no revision moves and no derived data is rebuilt (R262).
        tree_lock::ensure_unlocked(&*conn, plan.gate())?;
        return Ok(DeleteResult {
            deleted_count: 0,
            moved: compaction,
            root_kept,
            next_topic_reset: false,
        });
    }
    let tx = conn.transaction()?;
    tree_lock::ensure_unlocked(&tx, plan.gate())?;
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
        root_kept,
        next_topic_reset: false,
    })
}

/// The whole-archive delete keeps today's `clear` effects on Cards, derived Links and the Topic
/// allocation, and leaves config, dumps, artifacts, other metadata, receipts, Spread metadata and
/// the ended Locks' history alone (R264–R269, R275).
fn apply_delete_all_plan(
    conn: &mut Connection,
    plan: DeletePlan,
    next_topic_id_before: u64,
) -> Result<DeleteResult> {
    // Only an already empty archive whose allocation is already zero has nothing to change, so the
    // confirmed request writes nothing and advances no revision (R275).
    if plan.delete_set.is_empty() && next_topic_id_before == 0 {
        tree_lock::ensure_unlocked(&*conn, tree_lock::Change::DeleteAll)?;
        return Ok(DeleteResult {
            deleted_count: 0,
            moved: BTreeMap::new(),
            root_kept: None,
            next_topic_reset: true,
        });
    }
    let tx = conn.transaction()?;
    tree_lock::ensure_unlocked(&tx, tree_lock::Change::DeleteAll)?;
    let deleted_count = tx.execute("DELETE FROM cards", [])?;
    if link_index::ready(&tx) {
        link_index::rebuild(&tx)?;
        link_index::bump_revision(&tx)?;
    }
    tx.execute(
        "INSERT INTO metadata(key, value) VALUES('next_topic_id', '0')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        [],
    )?;
    tx.commit()?;
    Ok(DeleteResult {
        deleted_count,
        moved: BTreeMap::new(),
        root_kept: None,
        next_topic_reset: true,
    })
}

/// Read exactly one confirmation line and remove only its line terminator, so a pipe, a wrong word
/// and EOF all cancel without a change (R244).
fn read_confirmation<R: BufRead>(input: &mut R, expected: &str) -> Result<()> {
    println!("type `{expected}` to confirm:");
    let mut line = String::new();
    input.read_line(&mut line)?;
    if line.trim_end_matches(['\r', '\n']) != expected {
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
    tree_lock::ensure_unlocked(conn, tree_lock::Change::Mapping(mapping))?;
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
        if text != card.text {
            conn.execute(
                "UPDATE cards SET text = ?1 WHERE row_id = ?2",
                params![text, card.row_id],
            )?;
        }
    }
    spread::follow(conn, mapping, &BTreeSet::new())
}

fn shell_move(at: &str, target: &place::Target, alone: bool) -> Result<()> {
    let root = require_service_up()?;
    let _lock = acquire_edit_lock(&root, "move")?;
    let mut conn = open_db(&root)?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let plan = place::move_plan(&conn, at, target, alone)?;
    tree_lock::ensure_unlocked(&conn, tree_lock::Change::Mapping(&plan.mapping))?;
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
    let _lock = acquire_edit_lock(&root, "insert")?;
    let mut conn = open_db(&root)?;
    let plan = place::insert_plan(&conn, at)?;
    tree_lock::ensure_unlocked(&conn, tree_lock::Change::Mapping(&plan.mapping))?;
    tree_lock::ensure_unlocked(&conn, tree_lock::Change::CreateAt(&plan.location))?;
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

    /// S11 (#134) S11-AC05: emptying a tree keeps the root's row identity, address, kind,
    /// Workspace, BibTeX and authored fingerprint; only its generated Reverse section changes.
    #[test]
    fn keep_root_keeps_the_root_authored_fingerprint_and_bibtex() {
        let temp = tempfile::tempdir().expect("temp dir");
        initialize_database(temp.path()).expect("initialize database");
        let mut conn = open_db(temp.path()).expect("open database");
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES\
             (NULL, 'KeepMe', 0, 1, '@book{KeepMe, title={Keep Me}}',\
              '# Keep Me\n\nbody\n\n<!-- zt:reverse-links -->\n'),\
             ('KeepMe/1', NULL, 0, 0, NULL, '# Child\n\nback [[KeepMe]]\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .expect("insert the Literature tree");
        let before = load_card(&conn, "KeepMe").expect("root");
        let fingerprint = card::authored_fingerprint(&before).expect("fingerprint");

        let plan = keep_root_delete_plan(&conn, "KeepMe").expect("keep-root plan");
        let result = apply_delete_plan(&mut conn, plan).expect("empty the tree");

        assert_eq!(result.deleted_count, 1);
        assert_eq!(result.root_kept.as_deref(), Some("KeepMe"));
        let after = load_card(&conn, "KeepMe").expect("the root stays");
        assert_eq!(after.row_id, before.row_id);
        assert_eq!(after.address, before.address);
        assert_eq!(after.citation_key, before.citation_key);
        assert_eq!(after.is_lit, before.is_lit);
        assert_eq!(after.workspace, before.workspace);
        assert_eq!(after.card_type, before.card_type);
        assert_eq!(after.bibtex, before.bibtex);
        assert_eq!(
            card::authored_fingerprint(&after).expect("fingerprint"),
            fingerprint,
            "the root keeps its authored identity"
        );
        assert!(load_card(&conn, "KeepMe/1").is_err(), "the Regular Card goes");
        assert!(
            !after.text.contains("- [[KeepMe/1]] Child"),
            "the generated Reverse section forgets the deleted child: {}",
            after.text
        );
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
