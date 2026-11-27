//! The `zt mcp` client: zt-check's, so both tools see the same answers, plus this tool's own
//! contract around it: the service is resolved before archive work (R320) and every invocation
//! records the run in the archive's active-run marker (R318).

use anyhow::Result;
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

pub use zt_check::mcp::ServiceFacts;

type PoolKey = (bool, u64);

/// The staging folder this invocation was given, which ties its marker to one prepared run.
static STAGING: Mutex<Option<PathBuf>> = Mutex::new(None);
/// The unit this invocation names, when it names one, for the marker's heartbeat facts.
static UNIT: Mutex<Option<String>> = Mutex::new(None);
/// Sequence counter for unique marker temporary filenames.
static MARKER_SEQ: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static POOL_ENABLED: Cell<bool> = const { Cell::new(false) };
    static POOL: RefCell<Option<(PoolKey, zt_check::mcp::Zt)>> = const { RefCell::new(None) };
}

fn current_pool_key() -> PoolKey {
    (
        zt_check::mcp::scope::literature_only(),
        zt_check::mcp::current_child_edit_patience(),
    )
}

/// Owned `zt mcp` handle that returns healthy default-scoped clients to the batch thread-local pool on Drop.
pub struct Zt {
    inner: Option<zt_check::mcp::Zt>,
    poolable: bool,
}

impl Deref for Zt {
    type Target = zt_check::mcp::Zt;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref().expect("zt client present")
    }
}

impl DerefMut for Zt {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.inner.as_mut().expect("zt client present")
    }
}

impl Drop for Zt {
    fn drop(&mut self) {
        let Some(mut client) = self.inner.take() else {
            return;
        };
        if self.poolable && POOL_ENABLED.with(Cell::get) && client.is_alive() {
            let key = current_pool_key();
            POOL.with(|cell| {
                *cell.borrow_mut() = Some((key, client));
            });
        }
    }
}

/// Enables thread-local `Zt` client pooling on the current batch dispatch thread.
pub fn begin_batch_pool() {
    POOL_ENABLED.with(|enabled| enabled.set(true));
}

/// Drains and shuts down any pooled `Zt` client and clears cached service observation.
pub fn drain_batch_pool() {
    POOL_ENABLED.with(|enabled| enabled.set(false));
    POOL.with(|cell| {
        drop(cell.borrow_mut().take());
    });
    zt_check::mcp::invalidate_observed_service();
}

/// Evicts any pooled `Zt` client and clears cached service observation (used before transport retry).
pub fn evict_pooled() {
    POOL.with(|cell| {
        drop(cell.borrow_mut().take());
    });
    zt_check::mcp::invalidate_observed_service();
}

fn extract_flag_value(argv: &[String], name: &str) -> Option<String> {
    for (idx, arg) in argv.iter().enumerate() {
        if arg.eq_ignore_ascii_case(name) {
            return argv
                .get(idx + 1)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty() && !v.starts_with('-'));
        }
        if let Some((flag, val)) = crate::util::split_flag_arg(arg)
            && flag.eq_ignore_ascii_case(name)
        {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

/// Reads the invocation's staging folder and unit from its own arguments, before any work runs.
/// Resets previous values so sequential invocations inside `zt-digest batch` never leak state.
pub fn note_invocation(argv: &[String]) {
    let out = extract_flag_value(argv, "-Out")
        .or_else(|| extract_flag_value(argv, "-Staging"))
        .map(PathBuf::from);
    let unit = extract_flag_value(argv, "-Unit");
    if let Ok(mut guard) = STAGING.lock() {
        *guard = out;
    }
    if let Ok(mut guard) = UNIT.lock() {
        *guard = unit;
    }
}

/// The staging folder of this invocation, when it names one.
pub fn staging() -> Option<PathBuf> {
    STAGING.lock().ok().and_then(|guard| guard.clone())
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
    start_with(&[])
}

/// Starts an archive client whose `zt mcp` also declares these Write scope lists (`--allow <file>`,
/// `--beneath <file>`): the server then refuses every write of this connection outside them.
pub fn start_with(declarations: &[String]) -> Result<Zt> {
    let pool_enabled = POOL_ENABLED.with(Cell::get) && declarations.is_empty();
    if pool_enabled {
        let wanted = current_pool_key();
        let cached = POOL.with(|cell| cell.borrow_mut().take());
        if let Some((key, mut client)) = cached {
            if key == wanted && client.is_alive() {
                if let Some(facts) = zt_check::mcp::observed_service() {
                    renew_marker(&facts);
                }
                return Ok(Zt {
                    inner: Some(client),
                    poolable: true,
                });
            }
            let alive = client.is_alive();
            drop(client);
            if !alive {
                zt_check::mcp::invalidate_observed_service();
            }
        }
    }

    if let Some(program) = service_program() {
        zt_check::mcp::ensure_service(&program, zt_check::mcp::SERVICE_WAIT)?;
    }
    let client = zt_check::mcp::Zt::start_with(declarations)?;
    if let Some(facts) = zt_check::mcp::observed_service() {
        renew_marker(&facts);
    }
    Ok(Zt {
        inner: Some(client),
        poolable: declarations.is_empty(),
    })
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
    let unit = UNIT.lock().ok().and_then(|value| value.clone());
    renew_active_run(root, &out, unit.as_deref());
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
    let seq = MARKER_SEQ.fetch_add(1, Ordering::Relaxed);
    let temporary = root.join(format!("zt.digest.run.{}.{seq}.tmp", std::process::id()));
    if std::fs::write(&temporary, marker.to_string()).is_ok()
        && std::fs::rename(&temporary, &path).is_err()
    {
        let _ = std::fs::remove_file(&temporary);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_invocation_resets_staging_and_unit_and_parses_inline_flags() {
        note_invocation(&["-Out".into(), "D:\\run1".into(), "-Unit".into(), "u1".into()]);
        assert_eq!(staging(), Some(PathBuf::from("D:\\run1")));
        assert_eq!(UNIT.lock().unwrap().as_deref(), Some("u1"));

        note_invocation(&["-Staging:D:\\run2".into(), "-Unit=u2".into()]);
        assert_eq!(staging(), Some(PathBuf::from("D:\\run2")));
        assert_eq!(UNIT.lock().unwrap().as_deref(), Some("u2"));

        note_invocation(&["-Step".into(), "plan".into()]);
        assert_eq!(staging(), None);
        assert_eq!(*UNIT.lock().unwrap(), None);

        // Multi-byte UTF-8 positional/content args longer than "-Out" must never panic.
        note_invocation(&["第一章".into(), "模型：结论".into(), "-Out:D:\\run3".into()]);
        assert_eq!(staging(), Some(PathBuf::from("D:\\run3")));

        // Inline "=" flag with Windows drive colon must split at "=", not "C:".
        note_invocation(&["-Out=C:\\box\\stage".into(), "-Unit=digest/g001@1".into()]);
        assert_eq!(staging(), Some(PathBuf::from("C:\\box\\stage")));
        assert_eq!(UNIT.lock().unwrap().as_deref(), Some("digest/g001@1"));
    }
}
