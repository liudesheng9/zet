//! The `zt mcp` client: zt-check's, so both tools see the same answers, plus this tool's own
//! contract around it: the service is resolved before archive work (R320) and every invocation
//! records the run in the archive's active-run marker (R318).

use anyhow::Result;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub use zt_check::mcp::{ServiceFacts, Zt};

/// The staging folder this invocation was given, which ties its marker to one prepared run.
static STAGING: OnceLock<Option<PathBuf>> = OnceLock::new();
/// The unit this invocation names, when it names one, for the marker's heartbeat facts.
static UNIT: OnceLock<Option<String>> = OnceLock::new();

/// Reads the invocation's staging folder and unit from its own arguments, before any work runs.
pub fn note_invocation(argv: &[String]) {
    let value = |name: &str| {
        argv.iter()
            .position(|argument| argument.eq_ignore_ascii_case(name))
            .and_then(|at| argv.get(at + 1))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty() && !value.starts_with('-'))
    };
    let _ = STAGING.set(value("-Out").map(PathBuf::from));
    let _ = UNIT.set(value("-Unit"));
}

/// The staging folder of this invocation, when it names one.
pub fn staging() -> Option<&'static Path> {
    STAGING.get().and_then(|value| value.as_deref())
}

/// The service CLI a command must resolve before its archive work, or `None` when the client is
/// pointed at a stand-in (a test seam with another name) rather than the installed `zt`.
pub fn service_program() -> Option<String> {
    let program = std::env::var("ZT_CHECK_ZT").unwrap_or_else(|_| "zt".to_string());
    let stem = Path::new(&program)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_ascii_lowercase());
    matches!(stem.as_deref(), Some("zt")).then_some(program)
}

/// Starts the archive client. The service is resolved first (R320), so a restarting service is
/// waited for within its bound and a service that stays down is one infrastructure fault; then
/// this invocation refreshes the archive's active-run marker, which the daemon's heartbeat
/// reports while a digest run is active (R318).
pub fn start() -> Result<Zt> {
    if let Some(program) = service_program() {
        zt_check::mcp::ensure_service(&program, zt_check::mcp::SERVICE_WAIT)?;
    }
    let client = Zt::start()?;
    if let Some(facts) = zt_check::mcp::observed_service() {
        renew_marker(&facts);
    }
    Ok(client)
}

/// Writes the marker for the archive the resolved service serves, when this invocation names a
/// staging folder. A command outside a prepared run (a version check, say) writes nothing.
fn renew_marker(facts: &ServiceFacts) {
    let Some(root) = facts.archive_root.as_deref() else {
        return;
    };
    let Some(out) = staging() else {
        return;
    };
    let unit = UNIT.get().and_then(|value| value.clone());
    renew_active_run(root, out, unit.as_deref());
}

/// The marker's lifetime: three missed minute heartbeats still count as active, and a killed run
/// stops renewing at once, so it never stays active indefinitely (R318).
pub const ACTIVE_RUN_TTL_SECONDS: u64 = 300;

/// Refreshes `<archive>/zt.digest.run`, the marker the daemon reads before it appends a
/// heartbeat. It is published atomically and carries nothing an agent wrote: the run identity,
/// the staging folder, this process's pid, the renewal time and the marker's own lifetime.
pub fn renew_active_run(root: &Path, out: &Path, unit: Option<&str>) {
    let marker = json!({
        "schema_version": 1,
        "run_id": ledger_run_id(out).unwrap_or_else(|| "unknown".to_string()),
        "out": out.display().to_string(),
        "unit": unit.unwrap_or("run"),
        "pid": std::process::id(),
        "renewed": now_seconds(),
        "ttl_s": ACTIVE_RUN_TTL_SECONDS,
    });
    let path = root.join("zt.digest.run");
    let temporary = root.join("zt.digest.run.tmp");
    if std::fs::write(&temporary, marker.to_string()).is_ok() {
        let _ = std::fs::rename(&temporary, &path);
    }
}

/// The prepared run's own identity, read from its ledger without starting a client or writing.
pub fn ledger_run_id(out: &Path) -> Option<String> {
    let ledger = out.join("ledger.sqlite3");
    if !ledger.exists() {
        return None;
    }
    let conn = rusqlite::Connection::open_with_flags(
        ledger,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let value: String = conn
        .query_row("SELECT value FROM meta WHERE key='run_id'", [], |row| row.get(0))
        .ok()?;
    match serde_json::from_str::<Value>(&value) {
        Ok(Value::String(run)) => Some(run),
        _ => Some(value),
    }
}

pub fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
