//! `zt-digest backup -Out <path> [-Source <db>] [-Force]`:
//!
//! Creates a consistent SQLite backup of the archive using `VACUUM INTO` into a
//! sibling temporary file, verifies it with `PRAGMA integrity_check`, and only
//! then atomically publishes it to `-Out`. Never deletes `-Source` or destroys
//! an existing backup on failure.

use crate::args::Args;
use crate::util;
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OpenFlags};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupReport {
    pub source: PathBuf,
    pub out: PathBuf,
    pub integrity: String,
    pub bytes: u64,
}

impl BackupReport {
    pub fn to_json(&self) -> Value {
        json!({
            "ok": true,
            "source": self.source.display().to_string(),
            "out": self.out.display().to_string(),
            "integrity": self.integrity,
            "bytes": self.bytes,
        })
    }
}

fn norm_identity_key(path: &Path) -> Option<String> {
    let raw_abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(path)
    };
    // Lexically normalize `.` and `..` before ancestor canonicalization so even
    // `nonexistent_dir/../zt.sqlite3-journal` resolves against its real parent.
    let mut abs = PathBuf::new();
    for comp in raw_abs.components() {
        match comp {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                abs.pop();
            }
            other => abs.push(other.as_os_str()),
        }
    }
    let resolved = if let Ok(c) = std::fs::canonicalize(&abs) {
        c
    } else {
        // Walk up to the deepest existing ancestor (typically the parent directory) so two
        // different spellings of an absent path (e.g. relative vs absolute, or differing case on
        // Windows) canonicalize through the same real directory before joining trailing segments.
        let mut tail: Vec<std::ffi::OsString> = Vec::new();
        let mut cur = abs.as_path();
        let mut base = None;
        loop {
            if let Ok(c) = std::fs::canonicalize(cur) {
                base = Some(c);
                break;
            }
            let Some(name) = cur.file_name() else {
                break;
            };
            tail.push(name.to_os_string());
            let Some(parent) = cur.parent() else {
                break;
            };
            cur = parent;
        }
        let mut out = base.unwrap_or_else(|| abs.clone());
        for seg in tail.into_iter().rev() {
            out.push(seg);
        }
        out
    };
    let mut s = resolved.to_string_lossy().into_owned();
    if cfg!(windows) {
        if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
            s = format!(r"\\{stripped}");
        } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
            s = stripped.to_string();
        }
        s = s.replace('/', "\\").to_lowercase();
    }
    Some(s)
}

fn same_path_identity(source: &Path, out: &Path) -> bool {
    if let (Some(a), Some(b)) = (norm_identity_key(source), norm_identity_key(out)) {
        return a == b;
    }
    source == out
}

fn same_or_sqlite_sidecar_for(source_paths: &[PathBuf], out: &Path) -> bool {
    for src in source_paths {
        if same_path_identity(src, out) {
            return true;
        }
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut s = src.as_os_str().to_os_string();
            s.push(suffix);
            if same_path_identity(Path::new(&s), out) {
                return true;
            }
        }
    }
    false
}

/// Checks `out` against both the supplied `source` path and its resolved canonical path (H4),
/// so a file symlink/alias `alias.db -> real.db` protects both `alias.db-{wal,shm,journal}` and
/// `real.db-{wal,shm,journal}`.
fn same_or_sqlite_sidecar(source: &Path, out: &Path) -> bool {
    let mut sources = vec![source.to_path_buf()];
    if let Ok(canon) = std::fs::canonicalize(source)
        && canon != source
    {
        sources.push(canon);
    }
    same_or_sqlite_sidecar_for(&sources, out)
}

/// Refuses destinations that already have live SQLite recovery sidecars (`-wal`, `-shm`, `-journal`)
/// so `-Force` replacement can never leave a hot journal beside a freshly published backup (H4).
fn check_destination_sidecars(out: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut s = out.as_os_str().to_os_string();
        s.push(suffix);
        let sidecar = PathBuf::from(s);
        match std::fs::symlink_metadata(&sidecar) {
            Ok(_) => bail!(
                "refusing to write backup to {}: SQLite recovery sidecar {} exists (leave or recover it separately)",
                out.display(),
                sidecar.display()
            ),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to inspect backup destination sidecar {}",
                        sidecar.display()
                    )
                });
            }
        }
    }
    Ok(())
}

fn finish_with_explicit_cleanup<T>(
    result: Result<T>,
    out: &Path,
    tmp_dir: &Path,
    remove_fn: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<T> {
    let cleanup = remove_fn(tmp_dir);
    match (result, cleanup) {
        (Ok(val), Ok(())) => Ok(val),
        (Ok(_), Err(err)) => bail!(
            "published backup to {}, but failed to remove temporary backup folder {}: {err}",
            out.display(),
            tmp_dir.display()
        ),
        (Err(orig), Ok(())) => Err(orig),
        (Err(orig), Err(err)) => Err(orig.context(format!(
            "also failed to remove temporary backup folder {}: {err}",
            tmp_dir.display()
        ))),
    }
}

fn create_exclusive_sibling_dir(out: &Path) -> Result<PathBuf> {
    let parent = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let stem = out
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "backup.sqlite3".to_string());
    for _ in 0..16 {
        let mut rnd = [0u8; 8];
        getrandom::fill(&mut rnd)
            .map_err(|e| anyhow::anyhow!("failed to obtain random bytes for backup temp dir: {e}"))?;
        let hex: String = rnd.iter().map(|b| format!("{b:02x}")).collect();
        let dir = parent.join(format!(".{stem}.{}.{hex}.tmp", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to create temporary backup folder {}", dir.display())
                });
            }
        }
    }
    bail!(
        "failed to allocate unique temporary backup folder beside {}",
        out.display()
    )
}

fn ensure_regular_or_absent(out: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(out) {
        Ok(meta) => {
            let ft = meta.file_type();
            if ft.is_symlink() || ft.is_dir() || !ft.is_file() {
                bail!(
                    "backup destination must be a regular file, not a directory or symlink: {}",
                    out.display()
                );
            }
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect backup destination {}", out.display()))
        }
    }
}

/// Publishes `src` to `dst` without ever overwriting an existing destination.
/// Refuses to fall back to a racy check-then-rename when the filesystem cannot provide
/// atomic no-replace linking.
fn publish_no_clobber(src: &Path, dst: &Path) -> Result<()> {
    if ensure_regular_or_absent(dst)? {
        bail!(
            "backup destination already exists: {} (pass -Force to overwrite)",
            dst.display()
        );
    }
    match std::fs::hard_link(src, dst) {
        Ok(()) => {
            std::fs::remove_file(src).with_context(|| {
                format!(
                    "published backup to {}, but failed to remove temporary file {}",
                    dst.display(),
                    src.display()
                )
            })?;
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            bail!(
                "backup destination already exists: {} (pass -Force to overwrite)",
                dst.display()
            );
        }
        Err(err) if err.kind() == std::io::ErrorKind::Unsupported => {
            bail!(
                "cannot publish backup to {} without clobbering: filesystem does not support atomic no-replace links (pass -Force to overwrite)",
                dst.display()
            );
        }
        Err(err) => Err(err).with_context(|| format!("failed to publish backup to {}", dst.display())),
    }
}

/// Runs `VACUUM INTO` from `source` into an exclusively created sibling temp folder,
/// verifies it with `PRAGMA integrity_check`, closes it, and only then publishes it to `out`.
pub fn backup_sqlite(source: &Path, out: &Path, force: bool) -> Result<BackupReport> {
    if !source.is_file() {
        bail!("source archive database does not exist: {}", source.display());
    }
    if same_or_sqlite_sidecar(source, out) {
        bail!(
            "backup destination cannot be the same file as source or its SQLite sidecars: {}",
            out.display()
        );
    }
    if ensure_regular_or_absent(out)? && !force {
        bail!(
            "backup destination already exists: {} (pass -Force to overwrite)",
            out.display()
        );
    }
    check_destination_sidecars(out)?;
    if let Some(parent) = out.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create backup directory {}", parent.display()))?;
    }

    let tmp_dir = create_exclusive_sibling_dir(out)?;
    struct RemoveDirOnDrop(Option<PathBuf>);
    impl Drop for RemoveDirOnDrop {
        fn drop(&mut self) {
            if let Some(dir) = self.0.take() {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }
    let mut guard = RemoveDirOnDrop(Some(tmp_dir.clone()));
    let tmp_path = tmp_dir.join("backup.sqlite3");

    let run_result: Result<BackupReport> = (|| {
        let conn = Connection::open_with_flags(
            source,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("failed to open source database {}", source.display()))?;

        let tmp_str = tmp_path
            .to_str()
            .with_context(|| format!("backup path is not valid UTF-8: {}", tmp_path.display()))?;
        conn.execute("VACUUM INTO ?1", [tmp_str])
            .with_context(|| format!("VACUUM INTO failed for {}", out.display()))?;
        drop(conn);

        // Verify integrity of the freshly created backup independently before publishing.
        let verify = Connection::open_with_flags(
            &tmp_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .with_context(|| format!("failed to reopen backup {}", tmp_path.display()))?;
        let mut stmt = verify
            .prepare("PRAGMA integrity_check")
            .context("failed to prepare PRAGMA integrity_check")?;
        let rows: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed to read PRAGMA integrity_check")?;
        drop(stmt);
        drop(verify);
        if rows.len() != 1 || rows[0] != "ok" {
            bail!(
                "backup integrity check failed for {}: {}",
                out.display(),
                rows.join("; ")
            );
        }

        if same_or_sqlite_sidecar(source, out) {
            bail!(
                "backup destination cannot be the same file as source or its SQLite sidecars: {}",
                out.display()
            );
        }
        check_destination_sidecars(out)?;

        // Publish without clobbering when `!force`, or atomically replace an existing regular file with `-Force`.
        if !force {
            publish_no_clobber(&tmp_path, out)?;
        } else {
            replace_file(&tmp_path, out)?;
        }

        let bytes = std::fs::metadata(out)
            .with_context(|| format!("failed to stat backup {}", out.display()))?
            .len();

        Ok(BackupReport {
            source: source.to_path_buf(),
            out: out.to_path_buf(),
            integrity: "ok".to_string(),
            bytes,
        })
    })();

    guard.0 = None;
    finish_with_explicit_cleanup(run_result, out, &tmp_dir, |p| std::fs::remove_dir_all(p))
}

fn replace_file(src: &Path, dst: &Path) -> Result<()> {
    replace_file_with(src, dst, |a, b| std::fs::rename(a, b))
}

/// Atomically replaces `dst` with `src` in a single rename operation, never moving `dst` aside first
/// so a crash or failed rename always leaves any pre-existing backup intact at `dst`.
fn replace_file_with(
    src: &Path,
    dst: &Path,
    mut rename_fn: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    ensure_regular_or_absent(dst)?;
    rename_fn(src, dst).with_context(|| {
        format!(
            "failed to atomically replace backup at {} (existing file left intact)",
            dst.display()
        )
    })
}

pub fn cli(argv: &[String]) -> Result<i32> {
    let a = Args::parse(argv, &["Out", "Source"], &["Force"])?;
    let out = a.required("Out")?;
    let source = if a.has("Source") {
        PathBuf::from(a.required("Source")?)
    } else {
        let mut zt = crate::zt::start()?;
        let reply = zt.call("get_cards", json!({ "addresses": [], "view": "text" }))?;
        let root = reply["archive_identity"]["path"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("could not read archive_identity.path from zt service")?;
        PathBuf::from(root).join("zt.sqlite3")
    };
    let report = backup_sqlite(&source, Path::new(&out), a.flag("Force"))?;
    util::print_line(&util::compact(&report.to_json()))?;
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_db(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE cards (id INTEGER PRIMARY KEY, body TEXT);
             INSERT INTO cards (body) VALUES ('hello zt');",
        )
        .unwrap();
        path
    }

    #[test]
    fn vacuum_into_creates_verified_copy_and_refuses_existing_without_force() {
        let dir = TempDir::new().unwrap();
        let src = make_db(dir.path(), "zt.sqlite3");
        let dst = dir.path().join("backups").join("zt-before-test.sqlite3");

        let report = backup_sqlite(&src, &dst, false).unwrap();
        assert_eq!(report.integrity, "ok");
        assert!(report.bytes > 0);
        assert!(dst.is_file());

        let copy = Connection::open_with_flags(&dst, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let body: String = copy
            .query_row("SELECT body FROM cards WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(body, "hello zt");
        drop(copy);

        let err = backup_sqlite(&src, &dst, false).unwrap_err();
        assert!(format!("{err:#}").contains("already exists"), "{err:#}");

        let report2 = backup_sqlite(&src, &dst, true).unwrap();
        assert_eq!(report2.integrity, "ok");
    }

    #[test]
    fn force_never_deletes_same_file_source_or_existing_backup_on_failure() {
        let dir = TempDir::new().unwrap();
        let src = make_db(dir.path(), "zt.sqlite3");

        // Same-file source & destination with -Force must be refused without touching source!
        let err = backup_sqlite(&src, &src, true).unwrap_err();
        assert!(format!("{err:#}").contains("same file as source"), "{err:#}");
        assert!(src.is_file());

        // Existing valid backup must survive if a new source is corrupted / unreadable.
        let good_backup = dir.path().join("last-good.sqlite3");
        backup_sqlite(&src, &good_backup, false).unwrap();
        let good_bytes = std::fs::read(&good_backup).unwrap();

        let bad_src = dir.path().join("corrupt.sqlite3");
        std::fs::write(&bad_src, b"not a sqlite database").unwrap();
        assert!(backup_sqlite(&bad_src, &good_backup, true).is_err());
        assert_eq!(std::fs::read(&good_backup).unwrap(), good_bytes);
    }

    #[test]
    fn missing_source_database_fails_cleanly() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("no-such.sqlite3");
        let dst = dir.path().join("out.sqlite3");
        let err = backup_sqlite(&missing, &dst, false).unwrap_err();
        assert!(format!("{err:#}").contains("does not exist"), "{err:#}");
        assert!(!dst.exists());
    }

    #[test]
    fn rejects_directory_target_and_surfaces_rollback_and_cleanup_errors() {
        let dir = TempDir::new().unwrap();
        let src = make_db(dir.path(), "zt.sqlite3");
        let target_dir = dir.path().join("existing-dir");
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::write(target_dir.join("keep.txt"), "stay").unwrap();

        // Directory target must be refused both with and without -Force, leaving it intact.
        let err_no_force = backup_sqlite(&src, &target_dir, false).unwrap_err();
        assert!(
            format!("{err_no_force:#}").contains("regular file"),
            "{err_no_force:#}"
        );
        let err_force = backup_sqlite(&src, &target_dir, true).unwrap_err();
        assert!(
            format!("{err_force:#}").contains("regular file"),
            "{err_force:#}"
        );
        assert!(target_dir.is_dir());
        assert_eq!(
            std::fs::read_to_string(target_dir.join("keep.txt")).unwrap(),
            "stay"
        );

        // No-clobber publish must refuse if destination already exists without touching it.
        let existing = dir.path().join("existing.sqlite3");
        std::fs::write(&existing, b"original").unwrap();
        let staged = dir.path().join("staged.tmp");
        std::fs::write(&staged, b"replacement").unwrap();
        let nc_err = publish_no_clobber(&staged, &existing).unwrap_err();
        assert!(format!("{nc_err:#}").contains("already exists"), "{nc_err:#}");
        assert_eq!(std::fs::read(&existing).unwrap(), b"original");

        // H4: absent SQLite sidecars must be refused even under differing path spellings
        // (e.g. non-canonical `sub/../zt.sqlite3-journal` or differing Windows case).
        let sub = dir.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        let absent_journal_spelling = sub.join("..").join("zt.sqlite3-journal");
        assert!(same_or_sqlite_sidecar(&src, &absent_journal_spelling));
        let err_absent = backup_sqlite(&src, &absent_journal_spelling, true).unwrap_err();
        assert!(format!("{err_absent:#}").contains("sidecar"), "{err_absent:#}");
        if cfg!(windows) {
            let upper_wal = dir.path().join("ZT.SQLITE3-WAL");
            assert!(same_or_sqlite_sidecar(&src, &upper_wal));
        }
        // H4: when source is an alias/symlink to a real database file, sidecars of BOTH the alias
        // and the resolved target (`real.db-journal`) must be protected.
        let alias_src = dir.path().join("alias.db");
        let real_src = dir.path().join("real.db");
        let real_journal = dir.path().join("real.db-journal");
        let alias_journal = dir.path().join("alias.db-journal");
        assert!(same_or_sqlite_sidecar_for(&[alias_src.clone(), real_src.clone()], &real_journal));
        assert!(same_or_sqlite_sidecar_for(&[alias_src, real_src], &alias_journal));

        // SQLite recovery sidecars (-wal, -shm, -journal) must be refused even with -Force (H4).
        for suffix in ["-wal", "-shm", "-journal"] {
            let mut sidecar = src.as_os_str().to_os_string();
            sidecar.push(suffix);
            let sidecar_path = PathBuf::from(sidecar);
            std::fs::write(&sidecar_path, b"wal-bytes").unwrap();
            let err = backup_sqlite(&src, &sidecar_path, true).unwrap_err();
            assert!(format!("{err:#}").contains("sidecar"), "{err:#}");
            assert_eq!(std::fs::read(&sidecar_path).unwrap(), b"wal-bytes");
        }

        // H4: destination with its own existing SQLite recovery sidecar (-journal/-wal) must be
        // refused even with -Force without deleting its recovery file.
        let dst_with_journal = dir.path().join("prior-backup.sqlite3");
        let dst_journal = dir.path().join("prior-backup.sqlite3-journal");
        std::fs::write(&dst_with_journal, b"prior").unwrap();
        std::fs::write(&dst_journal, b"hot-journal").unwrap();
        let j_err = backup_sqlite(&src, &dst_with_journal, true).unwrap_err();
        assert!(format!("{j_err:#}").contains("recovery sidecar"), "{j_err:#}");
        assert_eq!(std::fs::read(&dst_journal).unwrap(), b"hot-journal");
        assert_eq!(std::fs::read(&dst_with_journal).unwrap(), b"prior");

        // L1: explicit temp-dir cleanup failures report the retained directory path on both Ok and Err paths.
        let fake_tmp = dir.path().join(".fake.tmp");
        let ok_clean_err = finish_with_explicit_cleanup(Ok(()), &dst_with_journal, &fake_tmp, |_| {
            Err(std::io::Error::other("busy"))
        })
        .unwrap_err();
        assert!(format!("{ok_clean_err:#}").contains(".fake.tmp"), "{ok_clean_err:#}");
        let err_clean_err = finish_with_explicit_cleanup::<()>(
            Err(anyhow::anyhow!("vacuum failed")),
            &dst_with_journal,
            &fake_tmp,
            |_| Err(std::io::Error::other("busy")),
        )
        .unwrap_err();
        let err_clean_msg = format!("{err_clean_err:#}");
        assert!(err_clean_msg.contains(".fake.tmp"), "{err_clean_msg}");
        assert!(err_clean_msg.contains("vacuum failed"), "{err_clean_msg}");

        // Atomic replace failure leaves pre-existing backup intact at dst without moving it aside (H3).
        let dst = dir.path().join("dst.sqlite3");
        let tmp = dir.path().join("src.tmp");
        std::fs::write(&dst, b"old").unwrap();
        std::fs::write(&tmp, b"new").unwrap();
        let rep_err = replace_file_with(&tmp, &dst, |_a, _b| {
            Err(std::io::Error::other("rename denied"))
        })
        .unwrap_err();
        let msg = format!("{rep_err:#}");
        assert!(msg.contains("rename denied"), "{msg}");
        assert!(msg.contains("existing file left intact"), "{msg}");
        assert_eq!(std::fs::read(&dst).unwrap(), b"old");
    }
}
