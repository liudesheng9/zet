//! #245: the one run watcher, and the ledger checkpoint.
//!
//! `ledger watch -Out <dir> [-Journal <path>] [-Every <s>] [-MaxHours <h>]` reads the run's counts
//! in-process every few minutes and prints a line when they change. It holds `<dir>/watch.lock`
//! for its whole life, so a second watcher of the same run refuses; the operating system releases
//! the lock when the watcher ends, however it ends, so a dead watcher never blocks the next. It
//! ends by itself when the run has no open unit, when the Workflow journal says the run halted, or
//! after `-MaxHours` (12 by default): no watcher outlives its run by more than one poll, and none
//! lives longer than half a day. On 2026-11-02, 54 forgotten shell watchers of one finished run
//! each started a `ledger status` process per poll, up to 19 at once, and locked the machine.
//!
//! `ledger checkpoint -Out <dir>` folds the WAL back into the ledger and cuts the file to zero.

use super::*;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Instant;

const LOCK: &str = "watch.lock";

/// What one watch looks at, and for how long.
pub(super) struct Watch {
    pub(super) out: PathBuf,
    pub(super) journal: Option<PathBuf>,
    pub(super) every: Duration,
    pub(super) max: Duration,
}

/// Take the run's watch lock, or refuse with `watch_running` while another watcher holds it.
pub(super) fn hold(out: &Path) -> Result<File> {
    let path = out.join(LOCK);
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            bail!("watch_running: another `ledger watch` holds {}", path.display())
        }
        Err(std::fs::TryLockError::Error(error)) => {
            return Err(error).with_context(|| format!("lock {}", path.display()));
        }
    }
    file.set_len(0)?;
    writeln!(file, "{}", json!({ "pid": std::process::id(), "started": canon::utc_now_o() }))?;
    Ok(file)
}

/// The counts a watch reports: every bucket but `claimed`, which moves on every claim and renewal.
fn key_of(counts: &Value) -> String {
    ["total", "done", "failed", "blocked", "retry", "ready", "registered", "refused", "noop"]
        .iter()
        .map(|name| format!("{name} {}", counts[*name].as_u64().unwrap_or(0)))
        .collect::<Vec<_>>()
        .join(" ")
}

fn halted(journal: &Option<PathBuf>) -> bool {
    journal
        .as_ref()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .is_some_and(|text| text.contains("\"halted\":true"))
}

/// Watch one run until it settles, halts, or the time runs out; every line goes to `emit`. Answers
/// why it ended: `settled` (no open unit at two polls in a row), `halted`, `gone` (no ledger), or
/// `timeout`.
pub(super) fn watch(w: &Watch, emit: &mut dyn FnMut(Value)) -> Result<&'static str> {
    let started = Instant::now();
    let mut last = None;
    let mut polls = 0_u64;
    let mut idle = 0;
    let mut last_error: Option<String> = None;
    loop {
        if halted(&w.journal) {
            emit(json!({ "event": "halted" }));
            return Ok("halted");
        }
        if !w.out.join("ledger.sqlite3").is_file() {
            emit(json!({ "event": "gone" }));
            return Ok("gone");
        }
        match status_counts(&w.out) {
            Ok(read) => {
                last_error = None;
                let counts = &read["counts"];
                let key = key_of(counts);
                polls += 1;
                if last.as_ref() != Some(&key) {
                    emit(json!({ "event": "change", "counts": key, "claimed": counts["claimed"] }));
                    last = Some(key);
                } else if polls % 8 == 0 {
                    emit(json!({ "event": "beat", "counts": key, "claimed": counts["claimed"] }));
                }
                if counts["open"] == json!(0) {
                    idle += 1;
                    if idle >= 2 {
                        // The full status, once: whether the settled run is complete.
                        let complete = status_lean(&w.out, "", None)
                            .map(|full| status_summary(&full)["complete"].clone())
                            .unwrap_or(Value::Null);
                        emit(json!({ "event": "settled", "complete": complete }));
                        return Ok("settled");
                    }
                } else {
                    idle = 0;
                }
            }
            // A read that fails (a writer mid-migration, a locked file) is reported once and retried.
            Err(error) => {
                let message = format!("{error:#}");
                if last_error.as_ref() != Some(&message) {
                    emit(json!({ "event": "error", "message": message }));
                    last_error = Some(message);
                }
            }
        }
        let spent = started.elapsed();
        if spent >= w.max {
            emit(json!({ "event": "timeout", "hours": w.max.as_secs_f64() / 3600.0 }));
            return Ok("timeout");
        }
        std::thread::sleep(w.every.min(w.max - spent));
    }
}

pub(super) fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(&argv[1..], &["Out", "Journal", "Every", "MaxHours"], &[])?;
    let out = PathBuf::from(a.required("Out")?);
    let every = a.int_or("Every", 240)?;
    if !(1..=3600).contains(&every) {
        bail!("-Every must be 1 to 3600 seconds");
    }
    let hours: f64 = if a.has("MaxHours") {
        a.str("MaxHours").parse().context("-MaxHours must be a number of hours")?
    } else {
        12.0
    };
    if !(hours > 0.0 && hours <= 48.0) {
        bail!("-MaxHours must be more than 0 and at most 48");
    }
    let _lock = match hold(&out) {
        Ok(lock) => lock,
        Err(error) => {
            util::print_line(&json!({ "ok": false, "code": "watch_running", "error": format!("{error:#}") }).to_string())?;
            return Ok(2);
        }
    };
    let journal = a.has("Journal").then(|| PathBuf::from(a.str("Journal")));
    let w = Watch {
        out: out.clone(),
        journal,
        every: Duration::from_secs(every as u64),
        max: Duration::from_secs_f64(hours * 3600.0),
    };
    let reason = watch(&w, &mut |line| {
        let _ = util::print_line(&line.to_string());
    })?;
    drop(_lock);
    let _ = std::fs::remove_file(out.join(LOCK));
    util::print_line(&json!({ "ok": true, "ended": reason }).to_string())?;
    Ok(0)
}

/// Fold the WAL back into the ledger and cut its file to zero (#245), waiting up to ten seconds
/// for readers to leave. Answers the WAL sizes before and after and whether a reader kept it.
pub(super) fn checkpoint(out: &Path) -> Result<Value> {
    let path = out.join("ledger.sqlite3");
    if !path.is_file() {
        bail!("no ledger at {}", path.display());
    }
    let wal = out.join("ledger.sqlite3-wal");
    let size = || std::fs::metadata(&wal).map(|meta| meta.len()).unwrap_or(0);
    let before = size();
    let conn = connect(&path, true)?;
    let (busy, frames, folded): (i64, i64, i64) =
        conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    drop(conn);
    Ok(json!({ "ok": busy == 0, "busy": busy != 0, "frames": frames, "folded": folded, "wal_before": before, "wal_after": size() }))
}

pub(super) fn checkpoint_cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(&argv[1..], &["Out"], &[])?;
    let answer = checkpoint(Path::new(&a.required("Out")?))?;
    util::print_line(&answer.to_string())?;
    Ok(if answer["ok"] == true { 0 } else { 3 })
}

#[cfg(test)]
mod tests {
    use super::super::lean_status_tests::ledger;
    use super::*;

    fn run(w: &Watch) -> (&'static str, Vec<Value>) {
        let mut lines = Vec::new();
        let reason = watch(w, &mut |line| lines.push(line)).unwrap();
        (reason, lines)
    }

    fn quick(out: &Path, journal: Option<PathBuf>, max_ms: u64) -> Watch {
        Watch { out: out.to_path_buf(), journal, every: Duration::from_millis(10), max: Duration::from_millis(max_ms) }
    }

    #[test]
    fn a_second_watch_of_one_run_refuses_until_the_first_ends() {
        let dir = ledger();
        let first = hold(dir.path()).unwrap();
        let second = hold(dir.path()).unwrap_err();
        assert!(format!("{second:#}").starts_with("watch_running"), "{second:#}");
        // The holder's PID is on record.
        let text = std::fs::read_to_string(dir.path().join(LOCK)).unwrap_or_default();
        drop(first);
        // A watcher that ends, however it ends, leaves the lock to the next.
        let again = hold(dir.path()).unwrap();
        drop(again);
        let record = std::fs::read_to_string(dir.path().join(LOCK)).unwrap();
        assert!(record.contains(&format!("\"pid\":{}", std::process::id())), "{record} / {text}");
    }

    #[test]
    fn a_watch_reports_the_counts_once_and_ends_after_its_time() {
        let dir = ledger();
        let (reason, lines) = run(&quick(dir.path(), None, 60));
        assert_eq!(reason, "timeout");
        let changes: Vec<&Value> = lines.iter().filter(|line| line["event"] == "change").collect();
        assert_eq!(changes.len(), 1, "unchanged counts are reported once: {lines:?}");
        assert!(changes[0]["counts"].as_str().unwrap().contains("ready 1"), "{lines:?}");
        assert_eq!(lines.last().unwrap()["event"], "timeout");
    }

    #[test]
    fn a_watch_ends_when_the_run_has_no_open_unit() {
        let dir = ledger();
        let conn = connect(&dir.path().join("ledger.sqlite3"), true).unwrap();
        conn.execute("DELETE FROM units WHERE state='ready'", []).unwrap();
        drop(conn);
        let (reason, lines) = run(&quick(dir.path(), None, 60_000));
        assert_eq!(reason, "settled", "{lines:?}");
        assert_eq!(lines.last().unwrap()["event"], "settled");
    }

    #[test]
    fn a_watch_ends_when_the_journal_says_the_run_halted() {
        let dir = ledger();
        let journal = dir.path().join("journal.jsonl");
        std::fs::write(&journal, "{\"phase\":\"Cards\"}\n{\"halted\":true}\n").unwrap();
        let (reason, lines) = run(&quick(dir.path(), Some(journal), 60_000));
        assert_eq!(reason, "halted");
        assert_eq!(lines, vec![json!({ "event": "halted" })]);
    }

    #[test]
    fn a_checkpoint_cuts_the_wal_to_nothing() {
        let dir = ledger();
        let conn = connect(&dir.path().join("ledger.sqlite3"), true).unwrap();
        for n in 0..200 {
            conn.execute("INSERT INTO meta(key,value) VALUES(?1,'1')", [format!("pad/{n}")]).unwrap();
        }
        drop(conn);
        let answer = checkpoint(dir.path()).unwrap();
        assert_eq!(answer["ok"], true, "{answer}");
        assert_eq!(answer["wal_after"], 0, "{answer}");
    }
}
