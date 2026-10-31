use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command as StdCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[path = "support/host_tools.rs"]
mod host_tools;

struct TestHome {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_dir = tmp.path().join("config");
        let archive_root = tmp.path().join("archive");
        Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        }
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn cmd_with_editor(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            concat!(
                "if ($env:ZT_EDITOR_PATH_OUT) { [IO.File]::WriteAllText($env:ZT_EDITOR_PATH_OUT, $args[0]) }\n",
                "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
                "if ($env:ZT_EDITOR_EXIT) { exit ([int]$env:ZT_EDITOR_EXIT) }\n",
            ),
        )
        .expect("editor script");
        let mut cmd = self.cmd();
        cmd.env(
            "EDITOR",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                script.display()
            ),
        );
        cmd.env("ZT_EDITOR_TEXT", text);
        cmd
    }

    fn cmd_with_canceling_editor(&self, text: &str) -> Command {
        let mut cmd = self.cmd_with_editor(text);
        cmd.env("ZT_EDITOR_EXIT", "1");
        cmd
    }

    fn cmd_with_editor_sequence(&self, texts: &[&str]) -> (Command, std::path::PathBuf) {
        let sequence_dir = self.config_dir.join("editor-sequence");
        let capture_dir = self.config_dir.join("editor-captures");
        let counter = self.config_dir.join("editor-counter.txt");
        let script = self.config_dir.join("fake-sequence-editor.ps1");
        let _ = fs::remove_dir_all(&sequence_dir);
        let _ = fs::remove_dir_all(&capture_dir);
        let _ = fs::remove_file(&counter);
        fs::create_dir_all(&sequence_dir).expect("editor sequence dir");
        fs::create_dir_all(&capture_dir).expect("editor capture dir");
        for (index, text) in texts.iter().enumerate() {
            fs::write(sequence_dir.join(format!("{index}.txt")), text)
                .expect("editor sequence text");
        }
        fs::write(
            &script,
            concat!(
                "$index = if (Test-Path -LiteralPath $env:ZT_EDITOR_COUNTER) { [int][IO.File]::ReadAllText($env:ZT_EDITOR_COUNTER) } else { 0 }\n",
                "$initial = [IO.File]::ReadAllText($args[0])\n",
                "[IO.File]::WriteAllText((Join-Path $env:ZT_EDITOR_CAPTURE_DIR \"$index.initial\"), $initial)\n",
                "[IO.File]::WriteAllText((Join-Path $env:ZT_EDITOR_CAPTURE_DIR \"$index.path\"), $args[0])\n",
                "$next = Join-Path $env:ZT_EDITOR_SEQUENCE_DIR \"$index.txt\"\n",
                "if (-not (Test-Path -LiteralPath $next)) { exit 1 }\n",
                "[IO.File]::WriteAllText($args[0], [IO.File]::ReadAllText($next))\n",
                "[IO.File]::WriteAllText($env:ZT_EDITOR_COUNTER, [string]($index + 1))\n",
            ),
        )
        .expect("sequence editor script");
        let mut cmd = self.cmd();
        cmd.env(
            "EDITOR",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                script.display()
            ),
        );
        cmd.env("ZT_EDITOR_SEQUENCE_DIR", &sequence_dir);
        cmd.env("ZT_EDITOR_CAPTURE_DIR", &capture_dir);
        cmd.env("ZT_EDITOR_COUNTER", &counter);
        (cmd, capture_dir)
    }

    fn configure_and_up(&self) {
        self.cmd()
            .args([
                "config",
                "set",
                "archive_root",
                self.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
        self.cmd().arg("up").assert().success();
    }

    fn card_text(&self, location: &str) -> String {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT text FROM cards WHERE location = ?1",
            [location],
            |row| row.get(0),
        )
        .expect("card text")
    }

    fn location_exists(&self, location: &str) -> bool {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1)",
            [location],
            |row| row.get::<_, i64>(0),
        )
        .expect("exists")
            == 1
    }

    fn literature_count(&self) -> i64 {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row("SELECT COUNT(*) FROM cards WHERE is_lit = 1", [], |row| {
            row.get(0)
        })
        .expect("literature count")
    }

    fn literature_data(&self, citation_key: &str) -> (String, String) {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT bibtex, text FROM cards WHERE citation_key = ?1",
            [citation_key],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("literature data")
    }

    fn revision(&self) -> i64 {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'archive_revision'",
            [],
            |row| row.get(0),
        )
        .expect("archive revision")
    }

    fn next_topic_id(&self) -> i64 {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT CAST(value AS INTEGER) FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .expect("next topic id")
    }

    /// A Card's stored identity: row identity, address, Citation key, kind flags, Card type,
    /// BibTeX, Workspace.
    fn card_row(
        &self,
        key: &str,
    ) -> (i64, String, Option<String>, i64, i64, String, Option<String>, String) {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT row_id, COALESCE(location, citation_key), citation_key, is_topic, is_lit,\
                    card_type, bibtex, workspace FROM cards WHERE location = ?1 OR citation_key = ?1",
            [key],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ))
            },
        )
        .expect("card row")
    }

    fn metadata(&self, key: &str) -> String {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row("SELECT value FROM metadata WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .expect("metadata value")
    }

    fn applied_ops(&self) -> i64 {
        let conn = Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        let created: i64 = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'applied_ops')",
                [],
                |row| row.get(0),
            )
            .expect("applied_ops table");
        if created == 0 {
            return 0;
        }
        conn.query_row("SELECT COUNT(*) FROM applied_ops", [], |row| row.get(0))
            .expect("applied ops")
    }

    fn create_topic_and_base(&self) {
        self.cmd_with_editor("# Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n")
            .args(["t", "Topic"])
            .assert()
            .success();
        self.cmd_with_editor("# Base\n\nbase\n\n<!-- zt:reverse-links -->\n")
            .args(["n", "--at", "0/0"])
            .assert()
            .success();
    }

    /// The pids recorded in `zt.sessions`: one line per registered Session process.
    fn session_count(&self) -> usize {
        fs::read_to_string(self.archive_root.join("zt.sessions"))
            .map(|body| {
                body.lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
            })
            .unwrap_or(0)
    }

    fn wait_until<F: Fn() -> bool>(&self, condition: F) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(condition(), "condition did not become true before timeout");
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

/// Read a live Session's output until `needle` has appeared anywhere in it, keeping everything
/// read so a later wait can match text that arrived in the same chunk.
fn wait_for_session_output(stdout: &mut impl BufRead, seen: &mut String, needle: &str) {
    let mut line = String::new();
    while !seen.contains(needle) {
        line.clear();
        let read = stdout.read_line(&mut line).expect("read session output");
        assert!(
            read > 0,
            "the Session ended before {needle:?}; it printed:\n{seen}"
        );
        seen.push_str(&line);
    }
}

#[cfg(windows)]
fn host_zip_backend_available() -> bool {
    StdCommand::new(host_tools::host_tar())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn host_tar_gz_backend_available() -> bool {
    let temp = tempfile::tempdir().expect("compression probe tempdir");
    let folder = temp.path().join("probe");
    fs::create_dir_all(&folder).expect("probe folder");
    fs::write(folder.join("mapping.json"), "{}").expect("probe payload");
    StdCommand::new(host_tools::host_tar())
        .args(["-a", "-cf"])
        .arg(temp.path().join("probe.tar.gz"))
        .arg("-C")
        .arg(temp.path())
        .arg("probe/mapping.json")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn host_tar_zst_backend_available() -> bool {
    let temp = tempfile::tempdir().expect("compression probe tempdir");
    let folder = temp.path().join("probe");
    fs::create_dir_all(&folder).expect("probe folder");
    fs::write(folder.join("mapping.json"), "{}").expect("probe payload");
    StdCommand::new(host_tools::host_tar())
        .args(["-a", "-cf"])
        .arg(temp.path().join("probe.tar.zst"))
        .arg("-C")
        .arg(temp.path())
        .arg("probe/mapping.json")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn host_tar_zst_backend_available() -> bool {
    StdCommand::new(host_tools::host_tar())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        && StdCommand::new("zstd")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

#[cfg(not(windows))]
fn host_tar_gz_backend_available() -> bool {
    StdCommand::new(host_tools::host_tar())
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
        && StdCommand::new("gzip")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
}

#[cfg(not(windows))]
fn host_zip_backend_available() -> bool {
    StdCommand::new("zip")
        .arg("-v")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn single_dump_archive(home: &TestHome, extension: &str) -> std::path::PathBuf {
    let dump_root = home.archive_root.join("dump");
    let archives: Vec<_> = fs::read_dir(&dump_root)
        .expect("dump directory")
        .map(|entry| entry.expect("dump entry").path())
        .filter(|path| path.is_file())
        .filter(|path| path.to_string_lossy().ends_with(extension))
        .collect();
    assert_eq!(archives.len(), 1, "expected one dump archive");
    archives.into_iter().next().unwrap()
}

fn expected_dump_filename(location: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(location.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("{}.md", &digest[..10])
}

fn expected_literature_dump_filename(citation_key: &str, bibtex: &str, text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(citation_key.as_bytes());
    hasher.update([0]);
    hasher.update(bibtex.as_bytes());
    hasher.update([0]);
    hasher.update(text.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("{}.md", &digest[..10])
}

/// The `locks.json` every dump carries, read from a ZIP archive (S13-AC02/S13-AC04).
fn zip_dump_locks(archive: &mut zip::ZipArchive<File>, archive_name: &str) -> serde_json::Value {
    let mut body = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{archive_name}/locks.json"))
            .expect("locks.json entry"),
        &mut body,
    )
    .expect("locks.json text");
    serde_json::from_str(&body).expect("locks.json document")
}

/// One entry of a dump archive in any format the host tar reads.
fn dump_entry_text(archive_path: &std::path::Path, entry: &str) -> String {
    let output = StdCommand::new(host_tools::host_tar())
        .arg("-xOf")
        .arg(archive_path)
        .arg(entry)
        .output()
        .expect("read dump entry");
    assert!(output.status.success(), "tar could not read {entry}");
    String::from_utf8(output.stdout).expect("entry text")
}

#[cfg(windows)]
fn build_failing_tar() -> TempDir {
    let output = tempfile::tempdir().expect("fake tar directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_tar.rs");
    let status = StdCommand::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(output.path().join("tar.exe"))
        .status()
        .expect("compile fake tar.exe");
    assert!(status.success());
    output
}

#[cfg(windows)]
fn build_probe_matrix() -> TempDir {
    let output = build_failing_tar();
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("fake_7z.rs");
    let seven_zip = output.path().join("7z.exe");
    let status = StdCommand::new("rustc")
        .arg(source)
        .arg("-o")
        .arg(&seven_zip)
        .status()
        .expect("compile fake 7z.exe");
    assert!(status.success());
    fs::copy(seven_zip, output.path().join("7zz.exe")).expect("copy fake 7zz.exe");
    output
}

/// PATH with the fake compressors in `directory` as the only ones: `zt dp` tries every
/// tar.exe, 7z.exe, and 7zz.exe on PATH, so the host's real ones are left out.
#[cfg(windows)]
fn path_with_only_fakes(directory: &std::path::Path) -> std::ffi::OsString {
    let mut paths = vec![directory.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).filter(|entry| {
            !["tar.exe", "7z.exe", "7zz.exe"]
                .iter()
                .any(|program| entry.join(program).is_file())
        }));
    }
    std::env::join_paths(paths).expect("test PATH")
}

#[cfg(windows)]
#[test]
fn service_starts_without_a_shell_and_preserves_archive_path() {
    let mut home = TestHome::new();
    home.archive_root = home._tmp.path().join("archive with spaces and 'quote'");
    home.cmd()
        .args(["config", "set", "archive_root"])
        .arg(&home.archive_root)
        .assert()
        .success();
    // A service launch needs only zt itself; no shell on PATH is required.
    let empty_path = home._tmp.path().join("empty-path");
    fs::create_dir(&empty_path).expect("empty PATH directory");
    home.cmd()
        .env("PATH", &empty_path)
        .arg("up")
        .assert()
        .success()
        .stdout(predicate::str::contains("service started pid:"));
    home.cmd()
        .env("PATH", &empty_path)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: up"));
    home.cmd()
        .env("PATH", &empty_path)
        .arg("down")
        .assert()
        .success()
        .stdout(predicate::str::contains("service stopped"));
}

#[test]
fn service_bootstraps_configured_archive_root_and_reports_status() {
    let home = TestHome::new();

    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("archive_root"));

    home.cmd()
        .arg("up")
        .assert()
        .success()
        .stdout(predicate::str::contains("service started"));

    home.cmd()
        .arg("up")
        .assert()
        .success()
        .stdout(predicate::str::contains("service already running"));

    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home._tmp.path().join("other").to_str().unwrap(),
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "cannot change archive_root while service is up",
        ));

    assert!(home.archive_root.join("zt.sqlite3").exists());
    assert!(home.archive_root.join("zt.log").exists());
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .expect("journal mode");
    assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
    let mut stmt = conn
        .prepare("PRAGMA table_info(cards)")
        .expect("table info");
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .expect("columns")
        .collect::<Result<Vec<_>, _>>()
        .expect("column rows");
    assert_eq!(
        columns,
        vec![
            "row_id",
            "location",
            "citation_key",
            "is_topic",
            "is_lit",
            "bibtex",
            "text",
            "workspace",
            "card_type",
            "model_facet",
            "model_emphasis",
            "model_level",
            "hub_level",
            "math_form",
            "term_name",
        ]
    );
    assert!(
        fs::read_to_string(home.archive_root.join("zt.log"))
            .expect("log")
            .contains("daemon started")
    );

    home.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: up"))
        .stdout(predicate::str::contains("cards: 0"))
        .stdout(predicate::str::contains("sessions: 0"));

    home.cmd()
        .arg("down")
        .assert()
        .success()
        .stdout(predicate::str::contains("service stopped"));

    home.cmd()
        .arg("down")
        .assert()
        .success()
        .stdout(predicate::str::contains("service already stopped"));
}

/// S28 (#170, US093): the executable publishes its own version and capability contract, so a
/// release check can tell the executable, rule, diagnostic and archive-schema contracts apart
/// without reading the source tree. `zt version` stays the one plain human line.
#[test]
fn version_publishes_the_executable_rule_schema_and_capability_contract() {
    let home = TestHome::new();
    home.cmd()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::diff(format!(
            "zt {}\n",
            env!("CARGO_PKG_VERSION")
        )));

    let out = home
        .cmd()
        .args(["version", "--json"])
        .output()
        .expect("zt version --json");
    assert!(
        out.status.success(),
        "zt version --json failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let value: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("zt version --json prints one JSON object");
    assert_eq!(value["zt"], env!("CARGO_PKG_VERSION"));
    assert_eq!(value["checker"], zt_check::VERSION);
    assert_eq!(value["rule_version"], zt_check::contract::RULE_VERSION);
    assert_eq!(
        value["diagnostic_schema"],
        zt_check::contract::DIAGNOSTIC_SCHEMA
    );
    assert_eq!(
        value["applicability_version"],
        zt_check::contract::APPLICABILITY_VERSION
    );
    // The Tree Lock schema generation this build installs and understands: the Term-aware
    // generation 2, whose older-binary floor is a real refusal, not a note.
    assert_eq!(value["schema"], "2");
    assert_eq!(value["capabilities"]["term_links"], true);
    assert_eq!(value["capabilities"]["term_cards"], true);
}

#[test]
fn literature_schema_enforces_card_kind_states_and_case_insensitive_identity() {
    let home = TestHome::new();
    home.configure_and_up();
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");

    for sql in [
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES(NULL, NULL, 0, 0, NULL, 'x')",
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES('0/1', 'Mixed', 0, 0, NULL, 'x')",
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES('0/1', 'Mixed', 0, 1, '@book{Mixed,title={X}}', 'x')",
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES(NULL, 'LitTopic', 1, 1, '@book{LitTopic,title={X}}', 'x')",
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES(NULL, 'NoBib', 0, 1, NULL, 'x')",
    ] {
        assert!(
            conn.execute(sql, []).is_err(),
            "invalid Card state was stored: {sql}"
        );
    }

    conn.execute(
        "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
         VALUES(NULL, 'ExactCase', 0, 1, '@book{ExactCase,title={X}}', '# X\n\n<!-- zt:reverse-links -->\n')",
        [],
    )
    .expect("valid Literature row");
    assert!(
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
             VALUES(NULL, 'exactcase', 0, 1, '@book{exactcase,title={Y}}', '# Y\n\n<!-- zt:reverse-links -->\n')",
            [],
        )
        .is_err(),
        "case-only duplicate Citation key was stored"
    );
    let exact: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE citation_key = 'ExactCase'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let wrong_case: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE citation_key = 'exactcase'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((exact, wrong_case), (1, 0));
}

#[test]
fn legacy_cards_schema_is_rejected_without_modifying_the_database() {
    let home = TestHome::new();
    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let db = home.archive_root.join("zt.sqlite3");
    let conn = Connection::open(&db).expect("legacy db");
    conn.execute_batch(
        "CREATE TABLE cards(location TEXT PRIMARY KEY, is_topic INTEGER NOT NULL, text TEXT NOT NULL);
         CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO cards VALUES('0/0', 1, '# Legacy\n\nexact body\n\n<!-- zt:reverse-links -->\n');
         INSERT INTO metadata VALUES('next_topic_id', '7');",
    )
    .expect("legacy fixture");
    drop(conn);
    let before = fs::read(&db).expect("legacy bytes before");

    home.cmd()
        .arg("up")
        .assert()
        .failure()
        .stderr(predicate::str::contains("legacy cards schema detected"));

    assert_eq!(fs::read(&db).expect("legacy bytes after"), before);
    assert!(!home.archive_root.join("zt.pid").exists());
    assert!(!home.archive_root.join("zt.log").exists());
    let conn = Connection::open(&db).expect("reopen legacy db");
    let row: (String, i64, String) = conn
        .query_row("SELECT location, is_topic, text FROM cards", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        row,
        (
            "0/0".into(),
            1,
            "# Legacy\n\nexact body\n\n<!-- zt:reverse-links -->\n".into()
        )
    );
    assert_eq!(next_topic_id, "7");
}

#[test]
fn service_rejects_corrupted_sqlite_without_rewriting_it() {
    let home = TestHome::new();
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let db = home.archive_root.join("zt.sqlite3");
    fs::write(&db, b"not a sqlite database").expect("corrupt db");

    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    home.cmd()
        .arg("up")
        .assert()
        .failure()
        .stderr(predicate::str::contains("SQLite").or(predicate::str::contains("database")));

    assert_eq!(
        fs::read(&db).expect("db bytes"),
        b"not a sqlite database".to_vec()
    );
}

#[test]
fn shell_create_edit_and_successor_locations() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd_with_editor("# Topic One\n\ndescription\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic One"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/0"));

    home.cmd_with_editor("# First\n\nbody\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/1"));

    home.cmd_with_editor("# No implicit pointer\n\nbody\n\n<!-- zt:reverse-links -->\n")
        .arg("n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected --at <target>"));

    home.cmd_with_editor("# Second\n\nnext\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/2"));

    home.cmd_with_editor("# Branch A\n\nside\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/1|a"));

    home.cmd_with_editor("# Branch B\n\nside\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/1|b"));

    home.cmd_with_editor("# Invalid\n\nside\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "side successors cannot start from a Topic Card",
        ));

    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total: 5"))
        .stdout(predicate::str::contains("topics: 1"))
        .stdout(predicate::str::contains("regular: 4"));
}

#[test]
fn shell_creates_literature_card_from_bibtex_then_card_text() {
    let home = TestHome::new();
    home.configure_and_up();
    let bibtex = concat!(
        "@article{Smith2024,\n",
        "  author = {Smith, Alice},\n",
        "  title = {{A}   Study},\n",
        "  year = {2024}\n",
        "}\n",
    );
    let (mut create, captures) = home.cmd_with_editor_sequence(&[
        bibtex,
        "# Client cannot replace this title\n\nmy literature notes\n\n<!-- zt:reverse-links -->\nclient reverse",
    ]);

    create
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::eq("Smith2024\n"));

    assert!(
        fs::read_to_string(captures.join("0.path"))
            .expect("metadata editor path")
            .ends_with(".bib")
    );
    assert!(
        fs::read_to_string(captures.join("1.path"))
            .expect("text editor path")
            .ends_with(".zt.md")
    );
    assert_eq!(
        fs::read_to_string(captures.join("1.initial")).expect("text editor initial content"),
        "# A Study\n\n\n<!-- zt:reverse-links -->\n"
    );

    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total: 1"))
        .stdout(predicate::str::contains("topics: 0"))
        .stdout(predicate::str::contains("regular: 0"))
        .stdout(predicate::str::contains("literature: 1"));
    home.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("cards: 1"));
}

#[test]
fn shell_literature_creation_reopens_invalid_metadata_with_content_intact() {
    let home = TestHome::new();
    home.configure_and_up();
    let invalid = "@comment{not a bibliographic work}\n";
    let valid = "@book{Repair2025, title = {Repaired Metadata}}\n";
    let (mut create, captures) = home.cmd_with_editor_sequence(&[
        invalid,
        valid,
        "# Repaired Metadata\n\nnotes\n\n<!-- zt:reverse-links -->\n",
    ]);

    create
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::eq("Repair2025\n"))
        .stderr(predicate::str::contains("ordinary bibliographic entry"));

    assert_eq!(
        fs::read_to_string(captures.join("1.initial")).expect("retried metadata"),
        invalid
    );
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("literature: 1"));
}

#[test]
fn literature_creation_cancellation_and_case_collisions_never_store_partial_rows() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd_with_canceling_editor("@book{CanceledMeta, title={Canceled Metadata}}")
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert_eq!(home.literature_count(), 0);

    let (mut cancel_body, _) =
        home.cmd_with_editor_sequence(&["@book{CanceledBody, title={Canceled Body}}\n"]);
    cancel_body
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert_eq!(home.literature_count(), 0);

    let invalid = "@book{0/7, title={Location Is Not A Citation Key}}";
    let (mut invalid_then_cancel, captures) = home.cmd_with_editor_sequence(&[invalid]);
    invalid_then_cancel
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("invalid Citation key"));
    assert_eq!(
        fs::read_to_string(captures.join("1.initial")).expect("invalid retry buffer"),
        invalid
    );
    assert_eq!(home.literature_count(), 0);

    let exact_bib = "@book{CaseKey, title={Exact Case}}\r\n";
    let (mut create, _) =
        home.cmd_with_editor_sequence(&[exact_bib, "# changed by client\nnotes\n"]);
    create
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::eq("CaseKey\n"));
    assert_eq!(home.literature_data("CaseKey").0, exact_bib);

    let (mut duplicate_then_cancel, duplicate_captures) =
        home.cmd_with_editor_sequence(&["@article{casekey, title={Case Collision}}"]);
    duplicate_then_cancel
        .arg("l")
        .assert()
        .success()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("already exists"));
    assert_eq!(
        fs::read_to_string(duplicate_captures.join("1.initial")).expect("duplicate retry buffer"),
        "@article{casekey, title={Case Collision}}"
    );
    assert_eq!(home.literature_count(), 1);
    home.cmd()
        .write_stdin("go casekey\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("target `casekey` does not exist"));

    let editor_path = home.config_dir.join("unexpected-editor.txt");
    home.cmd_with_editor("@book{Never, title={Never}}")
        .env("ZT_EDITOR_PATH_OUT", &editor_path)
        .args(["l", "unexpected"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown command"));
    assert!(
        !editor_path.exists(),
        "argument validation launched the editor"
    );
}

#[test]
fn shell_editor_failure_cancel_and_tempfile_cleanup_are_safe() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut no_editor = home.cmd();
    no_editor.env_remove("EDITOR");
    no_editor
        .args(["t", "No Editor"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("EDITOR is not set"));

    home.cmd_with_canceling_editor("# Canceled Topic\n\nnot saved\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Canceled Topic"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total: 0"));

    let path_out = home.config_dir.join("editor-path.txt");
    home.cmd_with_editor("# Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n")
        .env("ZT_EDITOR_PATH_OUT", &path_out)
        .args(["t", "Topic"])
        .assert()
        .success();
    let temp_path = fs::read_to_string(&path_out).expect("editor path");
    let temp_path = temp_path.trim();
    assert!(temp_path.ends_with(".zt.md"));
    assert!(!std::path::Path::new(&temp_path).exists());

    home.cmd_with_canceling_editor("# Canceled\n\nnot saved\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert!(!home.location_exists("0/1"));
}

#[test]
fn card_text_and_location_validation_reports_clear_errors() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd_with_editor("# Ignored\n\nignored\n\n<!-- zt:reverse-links -->\n")
        .args(["t", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("topic title"));

    home.cmd_with_editor("Malformed\nno title heading\n")
        .args(["t", "Malformed"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "must start with a `# Title` heading",
        ));

    home.cmd_with_editor(&format!("# Twice\n{0}\n{0}\n", "<!-- zt:reverse-links -->"))
        .args(["t", "Twice"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at most one"));

    home.cmd_with_editor("# Topic\n\nvalid\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/0"));

    home.cmd_with_editor("# Topic\n\ninvalid [[0/0]]\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/0"])
        .assert()
        .success();
    assert!(home.card_text("0/0").contains("- [[0/0]] Topic"));

    home.cmd_with_editor("# First\n\nbody\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();

    home.cmd_with_editor("# First\n\ninvalid [[0/01]]\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid link macro target"));
}

#[test]
fn reverse_links_and_broken_links_are_generated_from_card_text() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd_with_editor("# Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Base\n\nbase\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor("# Target\n\ntarget\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor(
        "# Source\n\nsee [[0/1|a]] and [[0/1|a]] plus [[0/0]]\n\n<!-- zt:reverse-links -->\n",
    )
    .args(["n", "--at", "0/1"])
    .assert()
    .success();

    let target_text = home.card_text("0/1|a");
    assert!(target_text.contains("- [[0/2]] Source"));
    assert_eq!(target_text.matches("- [[0/2]] Source").count(), 1);
    assert!(home.card_text("0/0").contains("- [[0/2]] Source"));

    home.cmd_with_editor("# Source\n\nnew broken [[0/99]]\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("does not exist"));

    home.cmd()
        .args(["del", "--at", "0/1|a"])
        .write_stdin("delete\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted: 1"));

    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::contains("0/2 Source -> 0/1|a"))
        .stdout(predicate::str::contains("see [[0/1|a]]"));

    home.cmd_with_editor("# Source\n\nsee [[0/1|a]] but keep editing\n\n<!-- zt:reverse-links -->\ndamaged reverse\n")
        .args(["e", "--at", "0/2"])
        .assert()
        .success();
}

#[test]
fn malformed_stored_link_text_remains_readable_and_validation_is_scoped_to_its_source() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let malformed = "# Base\n\nbefore [[0/0\n\n<!-- zt:reverse-links -->\n";
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute(
        "UPDATE cards SET text = ?1 WHERE location = '0/1'",
        [malformed],
    )
    .expect("inject malformed stored Link text");
    drop(conn);

    home.cmd()
        .write_stdin("go 0/1\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("before [[0/0"));

    home.cmd_with_editor("# Topic\n\nchanged\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/0"])
        .assert()
        .success();
    assert_eq!(home.card_text("0/1"), malformed);
    home.cmd().args(["status", "--check-links"]).assert().failure();
    home.cmd_with_editor("# Base\n\nstill [[0/0\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid link macro"));
    assert_eq!(home.card_text("0/1"), malformed);
    home.cmd_with_editor("# Base\n\nrepaired [[0/0]]\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .success();
    home.cmd().args(["status", "--check-links"]).assert().success();
}

#[test]
fn every_card_kind_can_link_to_literature_and_literature_can_link_to_locations() {
    let home = TestHome::new();
    home.configure_and_up();

    let (mut create_lit_a, _) = home.cmd_with_editor_sequence(&[
        "@article{LitA, title={Literature A}}",
        "# Literature A\n\nfirst work\n\n<!-- zt:reverse-links -->\n",
    ]);
    create_lit_a.arg("l").assert().success();

    home.cmd_with_editor("# Topic\n\nplain topic\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Regular\n\nplain regular\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor(
        "# Topic\n\ntopic cites [[LitA]] and [[0/1]]\n\n<!-- zt:reverse-links -->\n",
    )
    .args(["e", "--at", "0/0"])
    .assert()
    .success();
    home.cmd_with_editor("# Regular\n\nregular cites [[LitA]]\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .success();

    let (mut create_lit_b, _) = home.cmd_with_editor_sequence(&[
        "@book{LitB, title={Literature B}}",
        "# Literature B\n\nliterature cites [[LitA]], [[0/0]], and [[0/1]]\n\n<!-- zt:reverse-links -->\n",
    ]);
    create_lit_b.arg("l").assert().success();

    home.cmd()
        .write_stdin("go LitA\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("- [[0/0]] Topic"))
        .stdout(predicate::str::contains("- [[0/1]] Regular"))
        .stdout(predicate::str::contains("- [[LitB]] Literature B"));
    home.cmd()
        .write_stdin("go 0/0\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("- [[LitB]] Literature B"));
}

#[test]
fn session_root_topic_and_literature_lists_keep_their_locked_membership_and_order() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    for (key, title) in [("ZebraKey", "Zebra Work"), ("AlphaKey", "Alpha Work")] {
        let bib = format!("@book{{{key}, title={{{title}}}}}");
        let text = format!("# {title}\n\nnotes\n\n<!-- zt:reverse-links -->\n");
        let (mut create, _) = home.cmd_with_editor_sequence(&[&bib, &text]);
        create.arg("l").assert().success();
    }

    let automatic_root = home
        .cmd()
        .write_stdin("q\n")
        .output()
        .expect("automatic ROOT output");
    let automatic_root = String::from_utf8(automatic_root.stdout).unwrap();
    assert!(automatic_root.starts_with(
        "ROOT\nworkspace: human (switch with ws agent)\ntopics:\n0/0 Topic\nliterature:\nAlphaKey Alpha Work\nZebraKey Zebra Work\n"
    ));
    assert!(!automatic_root.contains("0/1 Base"));

    let root_ls = home
        .cmd()
        .write_stdin("ls\nq\n")
        .output()
        .expect("ROOT ls output");
    let root_ls = String::from_utf8(root_ls.stdout).unwrap();
    assert!(root_ls.contains("0/0 Topic\nAlphaKey Alpha Work\nZebraKey Zebra Work\n"));
    assert!(!root_ls.contains("0/1 Base"));

    let topic_ls = home
        .cmd()
        .write_stdin("go 0/0\nls\nq\n")
        .output()
        .expect("Topic ls output");
    let topic_ls = String::from_utf8(topic_ls.stdout).unwrap();
    // The initial ROOT view lists Literature Cards; the Topic listing must not.
    let topic_listing = &topic_ls[topic_ls
        .find("Topic tree 0/0: 2 cards\n")
        .expect("Topic tree listing")..];
    assert!(topic_listing.starts_with("Topic tree 0/0: 2 cards\n0/0 Topic\n0/1 Base\n"));
    assert!(!topic_listing.contains("AlphaKey Alpha Work"));
    assert!(!topic_listing.contains("ZebraKey Zebra Work"));

    let literature_ls = home
        .cmd()
        .write_stdin("go ZebraKey\nls\nq\n")
        .output()
        .expect("Literature ls output");
    let literature_ls = String::from_utf8(literature_ls.stdout).unwrap();
    assert!(literature_ls.contains("Literature tree ZebraKey: 1 cards\nZebraKey Zebra Work\n"));
    assert!(!literature_ls.contains("Literature tree ZebraKey: 1 cards\nAlphaKey"));
    assert_eq!(literature_ls.matches("0/0 Topic").count(), 1);
    assert!(!literature_ls.contains("0/1 Base"));
}

#[test]
fn empty_root_view_names_the_commands_that_create_topics_and_literature() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd()
        .write_stdin("q\n")
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "ROOT\nworkspace: human (switch with ws agent)\ntopics:\n  (none yet; create one with t <title>)\n\
             literature:\n  (none yet; create one with l)\n",
        ));
}

#[test]
fn shell_metadata_part_updates_raw_bibtex_and_generated_title_without_editing_body() {
    let home = TestHome::new();
    home.configure_and_up();
    home.cmd_with_editor("# Topic\n\ntarget\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@article{EditKey, title={Original Title}}",
        "# Original Title\n\nbody stays here [[0/0]]\n\n<!-- zt:reverse-links -->\n",
    ]);
    create.arg("l").assert().success();

    let editor_path = home.config_dir.join("metadata-editor-path.txt");
    home.cmd_with_editor("@article{EditKey, title={Updated {Title}}}\n")
        .env("ZT_EDITOR_PATH_OUT", &editor_path)
        .args(["e", "--at", "EditKey", "--part", "metadata"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert!(
        fs::read_to_string(editor_path)
            .expect("metadata editor path")
            .ends_with(".bib")
    );

    home.cmd()
        .write_stdin("go EditKey\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Updated Title"))
        .stdout(predicate::str::contains(
            "@article{EditKey, title={Updated {Title}}}",
        ))
        .stdout(predicate::str::contains("body stays here"));
    home.cmd()
        .write_stdin("go 0/0\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("- [[EditKey]] Updated Title"));
}

#[test]
fn shell_literature_edit_selector_errors_fail_before_editor_launch() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@book{SelectorKey, title={Selector Work}}",
        "# Selector Work\n\nbody\n\n<!-- zt:reverse-links -->\n",
    ]);
    create.arg("l").assert().success();
    home.cmd_with_editor("# Topic\n\nbody\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("  zt l [--ws <workspace>]\n"))
        .stdout(predicate::str::contains("  zt del --at <target>\n"))
        .stdout(predicate::str::contains(
            "  zt n --at <target> [--type <type>] [--term <name>]\n",
        ));

    let cases: &[(&[&str], &str)] = &[
        (&["e", "--at", "SelectorKey"], "requires --part"),
        (
            &["e", "--at", "SelectorKey", "--part", "unknown"],
            "invalid --part",
        ),
        (
            &[
                "e",
                "--at",
                "SelectorKey",
                "--part",
                "text",
                "--part",
                "metadata",
            ],
            "duplicate or incomplete --part",
        ),
        (
            &["e", "--at", "0/0", "--part", "text"],
            "valid only when editing a Literature Card",
        ),
    ];
    for (index, (args, expected)) in cases.iter().enumerate() {
        let marker = home
            .config_dir
            .join(format!("editor-must-not-run-{index}.txt"));
        home.cmd_with_editor("must not be used")
            .env("ZT_EDITOR_PATH_OUT", &marker)
            .args(*args)
            .assert()
            .failure()
            .stderr(predicate::str::contains(*expected));
        assert!(
            !marker.exists(),
            "invalid selector launched EDITOR: {args:?}"
        );
    }
}

#[test]
fn metadata_edit_cancel_collision_and_rejected_rename_leave_graph_unchanged() {
    let home = TestHome::new();
    home.configure_and_up();
    for (key, title) in [("KeepKey", "Keep Work"), ("TakenKey", "Taken Work")] {
        let bib = format!("@book{{{key}, title={{{title}}}}}\n");
        let text = format!("# {title}\n\nbody {key}\n\n<!-- zt:reverse-links -->\n");
        let (mut create, _) = home.cmd_with_editor_sequence(&[&bib, &text]);
        create.arg("l").assert().success();
    }
    home.cmd_with_editor("# Topic\n\nsource [[KeepKey]]\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    let original = home.literature_data("KeepKey");
    let original_source = home.card_text("0/0");

    home.cmd_with_canceling_editor("@book{CanceledRename, title={Canceled}}")
        .args(["e", "--at", "KeepKey", "--part", "metadata"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
    assert_eq!(home.literature_data("KeepKey"), original);

    let (mut collision, captures) =
        home.cmd_with_editor_sequence(&["@book{takenkey, title={Collision}}"]);
    collision
        .args(["e", "--at", "KeepKey", "--part", "metadata"])
        .assert()
        .success()
        .stderr(predicate::str::contains("conflicts with an existing"));
    assert_eq!(
        fs::read_to_string(captures.join("1.initial")).expect("collision retry buffer"),
        "@book{takenkey, title={Collision}}"
    );
    assert_eq!(home.literature_data("KeepKey"), original);

    home.cmd_with_editor("@book{RejectedRename, title={Rejected}}")
        .args(["e", "--at", "KeepKey", "--part", "metadata"])
        .write_stdin("not move\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("KeepKey -> RejectedRename"))
        .stderr(predicate::str::contains("confirmation did not match"));
    assert_eq!(home.literature_data("KeepKey"), original);
    assert_eq!(home.card_text("0/0"), original_source);
    assert_eq!(home.literature_count(), 2);
    home.cmd()
        .write_stdin("go RejectedRename\ngo keepkey\ngo KeepKey\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "target `RejectedRename` does not exist",
        ))
        .stdout(predicate::str::contains("target `keepkey` does not exist"))
        .stdout(predicate::str::contains("citation key: KeepKey"));
}

#[test]
fn shell_text_part_repairs_literature_title_and_reverse_links() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@book{TextEdit, title={Generated Title}}",
        "# Generated Title\n\nold body\n\n<!-- zt:reverse-links -->\n",
    ]);
    create.arg("l").assert().success();
    home.cmd_with_editor("# Topic\n\nsource [[TextEdit]]\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();

    home.cmd_with_editor(
        "\n# Client added another title\n\nnew body\n\n<!-- zt:reverse-links -->\nclient reverse must disappear",
    )
    .args(["e", "--at", "TextEdit", "--part", "text"])
    .assert()
    .success();

    home.cmd()
        .write_stdin("go TextEdit\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("# Generated Title"))
        .stdout(predicate::str::contains("new body"))
        .stdout(predicate::str::contains("- [[0/0]] Topic"))
        .stdout(predicate::str::contains("client reverse must disappear").not());
}

#[test]
fn citation_key_rename_requires_move_and_rewrites_links_from_every_card_kind() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create_old, _) = home.cmd_with_editor_sequence(&[
        "@book{OldKey, title={Old Work}}",
        "# Old Work\n\nwork\n\n<!-- zt:reverse-links -->\n",
    ]);
    create_old.arg("l").assert().success();
    home.cmd_with_editor("# Topic [[OldKey]]\n\ntopic [[OldKey]]\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Regular\n\nregular [[OldKey]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    let (mut create_source, _) = home.cmd_with_editor_sequence(&[
        "@misc{LitSource, title={Literature Source}, note={metadata [[OldKey]] stays literal}}",
        "# Literature Source\n\nliterature [[OldKey]]\n\n<!-- zt:reverse-links -->\n",
    ]);
    create_source.arg("l").assert().success();

    home.cmd_with_editor("@book{NewKey, title={Renamed Work}}\n")
        .args(["e", "--at", "OldKey", "--part", "metadata"])
        .write_stdin("move\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("OldKey -> NewKey"))
        .stdout(predicate::str::contains("link macros rewritten: 3"));

    home.cmd()
        .write_stdin("go OldKey\ngo NewKey\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("target `OldKey` does not exist"))
        .stdout(predicate::str::contains("citation key: NewKey"))
        .stdout(predicate::str::contains("# Renamed Work"));
    for target in ["0/0", "0/1", "LitSource"] {
        home.cmd()
            .write_stdin(format!("go {target}\nq\n"))
            .assert()
            .success()
            .stdout(predicate::str::contains("[[NewKey]]"));
    }
    home.cmd()
        .write_stdin("go LitSource\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "metadata [[OldKey]] stays literal",
        ));
    assert!(
        home.card_text("0/0")
            .starts_with("# Topic [[OldKey]]\n\ntopic [[NewKey]]\n"),
        "Citation-key rename must rewrite only actual body Links"
    );
}

#[test]
fn literature_delete_leaves_broken_inbound_links_and_refuses_location_topology() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@article{DeleteLit, title={Delete Literature}}",
        "# Delete Literature\n\nwork\n\n<!-- zt:reverse-links -->\n",
    ]);
    create.arg("l").assert().success();
    home.cmd_with_editor("# Topic\n\nsource [[DeleteLit]]\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Topic\n\nwrong case [[deletelit]]\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "link target `deletelit` does not exist",
        ));

    home.cmd_with_editor("# Never\n\ncreated\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "DeleteLit"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "side successors cannot start from a Literature Card",
        ));
    home.cmd()
        .args(["mv", "--at", "DeleteLit", "0/1"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Literature Card cannot be moved"));
    home.cmd_with_editor("# Tree Note\n\ninside the tree\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "DeleteLit"])
        .assert()
        .success()
        .stdout("DeleteLit/1\n");

    home.cmd()
        .args(["del", "--at", "DeleteLit"])
        .write_stdin("delete\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("type `DeleteLit` to confirm:"))
        .stderr(predicate::str::contains("confirmation did not match"));
    assert!(home.location_exists("DeleteLit/1"));
    home.cmd()
        .args(["del", "--at", "DeleteLit"])
        .write_stdin("DeleteLit\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("delete DeleteLit\n"))
        .stdout(predicate::str::contains("delete DeleteLit/1\n"))
        .stdout(predicate::str::contains("deleted: 2"))
        .stdout(predicate::str::contains("compacted:").not());
    assert!(!home.location_exists("DeleteLit/1"));

    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "0/0 Topic -> DeleteLit: source [[DeleteLit]]",
        ));
    home.cmd_with_editor("# Topic\n\nsource [[DeleteLit]] after deletion\n\n<!-- zt:reverse-links -->\nstale reverse")
        .args(["e", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor("# New Source\n\nnew [[DeleteLit]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "link target `DeleteLit` does not exist",
        ));
    home.cmd()
        .write_stdin("go DeleteLit\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "target `DeleteLit` does not exist",
        ));
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("literature: 0"))
        .stdout(predicate::str::contains("total: 1"));
}

#[test]
fn delete_side_successor_compacts_later_sides_and_rewrites_links() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    home.cmd_with_editor("# Side A\n\na\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("# Side B\n\nb\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("# Side C\n\nc\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("# Side C Child\n\nchild\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1|c"])
        .assert()
        .success();
    home.cmd_with_editor(
        "# Source\n\nlinks [[0/1|c]] and [[0/1|c|1]]\n\n<!-- zt:reverse-links -->\n",
    )
    .args(["n", "--at", "0/1"])
    .assert()
    .success();

    home.cmd()
        .args(["del", "--at", "0/1|b"])
        .write_stdin("delete\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("delete verification:"))
        .stdout(predicate::str::contains("successors: 0"))
        .stdout(predicate::str::contains("0/1|c -> 0/1|b"))
        .stdout(predicate::str::contains("0/1|c|1 -> 0/1|b|1"));

    assert!(!home.location_exists("0/1|c"));
    assert!(home.location_exists("0/1|b"));
    assert!(home.location_exists("0/1|b|1"));
    let source = home.card_text("0/2");
    assert!(source.contains("[[0/1|b]]"));
    assert!(source.contains("[[0/1|b|1]]"));

    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn config_set_rewrites_only_the_archive_root_line() {
    let home = TestHome::new();
    fs::create_dir_all(&home.config_dir).expect("config dir");
    let config = home.config_dir.join("config.toml");
    fs::write(&config, "# my note\narchive_root=C:/old\nother=1\n").expect("config");
    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        format!(
            "# my note\narchive_root={}\nother=1\n",
            home.archive_root.display()
        )
    );
}

#[test]
fn a_killed_session_does_not_leave_its_edit_lock_holding_every_write() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    // A Session holds the edit lock while its editor is open; then the process is killed.
    let mut editing = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()
        .expect("spawn editing session");
    editing
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"go 0/1\ne\n")
        .expect("enter edit");
    home.wait_until(|| home.archive_root.join("zt.edit.lock").exists());
    home.cmd_with_editor("# Blocked\n\nblocked\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("edit in progress"));
    editing.kill().expect("kill session");
    editing.wait().expect("session ends");

    home.cmd_with_editor("# After\n\nafter\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();
    assert!(home.card_text("0/2").starts_with("# After"));
    // The next writer finds the lock free again.
    home.cmd_with_editor("# Again\n\nagain\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/2"])
        .assert()
        .success();
}

#[test]
fn deleting_a_linked_side_successor_keeps_its_link_broken_instead_of_renaming_a_sibling_onto_it() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    for title in ["Side A", "Side B", "Side C"] {
        home.cmd_with_editor(&format!("# {title}\n\nx\n\n<!-- zt:reverse-links -->\n"))
            .args(["b", "--at", "0/1"])
            .assert()
            .success();
    }
    home.cmd_with_editor(
        "# Source\n\nlinks [[0/1|b]] and [[0/1|c]]\n\n<!-- zt:reverse-links -->\n",
    )
    .args(["n", "--at", "0/1"])
    .assert()
    .success();

    home.cmd()
        .args(["del", "--at", "0/1|b"])
        .write_stdin("delete\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("links that will break: 1"))
        .stdout(predicate::str::contains("0/2 -> 0/1|b"));

    // The Link to the deleted Card stays broken; Side C keeps its Location and its Link.
    assert!(!home.location_exists("0/1|b"));
    assert!(home.card_text("0/1|c").starts_with("# Side C"));
    let source = home.card_text("0/2");
    assert!(source.contains("links [[0/1|b]] and [[0/1|c]]"), "{source}");
    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::contains("0/2 Source -> 0/1|b"));
}

#[test]
fn move_subtree_validates_target_and_rewrites_links() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    home.cmd_with_editor("# Movable\n\nmove me\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("# Movable Child\n\nchild\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1|a"])
        .assert()
        .success();
    home.cmd_with_editor(
        "# Source\n\npoints [[0/1|a]] and [[0/1|a|1]]\n\n<!-- zt:reverse-links -->\n",
    )
    .args(["n", "--at", "0/1"])
    .assert()
    .success();

    home.cmd()
        .args(["mv", "--at", "0/0", "0/2|a"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Topic Cards cannot be moved"));

    home.cmd()
        .args(["mv", "--at", "0/1|a", "0/2|b"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("next available label"));

    home.cmd()
        .args(["mv", "--at", "0/1|a", "0/02"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid move location"));

    home.cmd()
        .args(["mv", "--at", "0/1|a", "0/1|a|1"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("own successors"));

    home.cmd()
        .args(["mv", "--at", "0/1|a", "0/2"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exist"));

    home.cmd()
        .args(["mv", "--at", "0/1|a", "0/2|a"])
        .write_stdin("move\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("move verification:"))
        .stdout(predicate::str::contains("0/1|a -> 0/2|a"))
        .stdout(predicate::str::contains("0/1|a|1 -> 0/2|a|1"))
        .stdout(predicate::str::contains("moved cards: 2"))
        .stdout(predicate::str::contains("link macros rewritten: 2"));

    assert!(!home.location_exists("0/1|a"));
    assert!(home.location_exists("0/2|a"));
    assert!(home.location_exists("0/2|a|1"));
    let source = home.card_text("0/2");
    assert!(source.contains("[[0/2|a]]"));
    assert!(source.contains("[[0/2|a|1]]"));
    assert!(home.card_text("0/2|a").contains("- [[0/2]] Source"));
}

#[test]
fn session_navigation_and_read_only_commands_use_the_session_pointer() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("# Second\n\nsecond\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();

    home.cmd()
        .write_stdin("  go 0/0  \nls\ngo 0/1\n\nfoo bar\nzt e\nzt\nroot\ngo 0/99\nstats\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("ROOT"))
        .stdout(predicate::str::contains("0/0 Topic"))
        .stdout(predicate::str::contains("location: 0/0"))
        .stdout(predicate::str::contains("direct: [[0/1]]"))
        .stdout(predicate::str::contains("0/1 Base"))
        .stdout(predicate::str::contains("location: 0/1"))
        .stdout(predicate::str::contains("unknown session command: foo"))
        .stdout(predicate::str::contains("unknown session command: zt"))
        .stdout(predicate::str::contains("session commands must start").not())
        .stdout(predicate::str::contains("target `0/99` does not exist"))
        .stdout(predicate::str::contains("total: 3"));
}

#[test]
fn line_session_bare_go_returns_to_root_and_is_idempotent() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let output = home
        .cmd()
        .write_stdin("go 0/1\ngo\ngo\ngo 0/1\nq\n")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("session output is UTF-8");

    assert_eq!(stdout.matches("ROOT\n").count(), 3);
    assert_eq!(stdout.matches("location: 0/1\n").count(), 2);
    assert!(!stdout.contains("unknown session command: go"));
}

#[test]
fn line_session_rejects_extra_go_arguments_without_moving_pointer() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    home.cmd()
        .write_stdin("go 0/1\ngo 0/0 extra\ne\n\x1bq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("usage: go [<target>]"))
        .stdout(predicate::str::contains("unknown session command: go").not())
        .stdout(predicate::str::contains(
            "edit mode\n# Base\n\nbase\n\n<!-- zt:reverse-links -->",
        ));
}

#[test]
fn line_session_help_shows_optional_go_target_and_root_alias() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd()
        .write_stdin("help\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("go [<target>] | root"))
        .stdout(predicate::str::contains("root | go <target>").not());
}

#[test]
fn go_remains_session_only_without_shell_or_initial_target_forms() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("zt go").not());
    for args in [vec!["go"], vec!["go", "--at", "0/1"], vec!["--at", "0/1"]] {
        home.cmd()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("unknown command"));
    }
    home.cmd()
        .write_stdin("q\n")
        .assert()
        .success()
        .stdout(predicate::str::starts_with("ROOT\n"));
}

#[test]
fn session_writes_use_pointer_tui_save_cancel_and_retry_validation() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd()
        .write_stdin(
            concat!(
                "t Session Topic\n",
                "# Session Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n",
                "\x13",
                "n\n",
                "# First\n\nbody\n\n<!-- zt:reverse-links -->\n",
                "\x13",
                "b\n",
                "# Side\n\nside body\n\n<!-- zt:reverse-links -->\n",
                "\x13",
                "q\n",
            )
            .as_bytes(),
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("edit mode"))
        .stdout(predicate::str::contains("location: 0/1|a"));

    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    assert!(home.location_exists("0/1|a"));
    assert!(home.card_text("0/1|a").contains("side body"));

    home.cmd()
        .write_stdin(
            concat!(
                "go 0/1\n",
                "n\n",
                "# Canceled\n\nnot saved\n\n<!-- zt:reverse-links -->\n",
                "\x1b",
                "q\n",
            )
            .as_bytes(),
        )
        .assert()
        .success();
    assert!(!home.location_exists("0/2"));

    home.cmd()
        .write_stdin(
            concat!(
                "go 0/1\n",
                "e\n",
                "Broken without a heading\n",
                "\x13",
                "# Fixed\n\nupdated body\n\n<!-- zt:reverse-links -->\ndamaged reverse\n",
                "\x13",
                "q\n",
            )
            .as_bytes(),
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("`# Title` heading"));
    assert!(home.card_text("0/1").contains("updated body"));
    assert!(!home.card_text("0/1").contains("damaged reverse"));
}

#[test]
fn line_session_creates_and_edits_literature_with_retry_and_numbered_selection() {
    let home = TestHome::new();
    home.configure_and_up();
    let mut input = String::new();
    input.push_str("l\n");
    input.push_str("@comment{invalid}");
    input.push('\x13');
    input.push_str("@book{LineLit, title={Line Literature}}");
    input.push('\x13');
    input.push_str("# client title\n\nline body\n\n<!-- zt:reverse-links -->\nclient reverse");
    input.push('\x13');
    input.push_str("e\n9\n1\n");
    input.push_str("@book{LineLit, title={Updated Line Literature}}");
    input.push('\x13');
    input.push_str("e\n2\n");
    input.push_str("\n# replacement title line\n\nupdated line body\n\n<!-- zt:reverse-links -->\nstale reverse");
    input.push('\x13');
    input.push_str("q\n");

    home.cmd()
        .write_stdin(input)
        .assert()
        .success()
        .stdout(predicate::str::contains("metadata edit"))
        .stdout(predicate::str::contains(
            "exactly one ordinary bibliographic entry",
        ))
        .stdout(predicate::str::contains("citation key: LineLit"))
        .stdout(predicate::str::contains("invalid edit option"))
        .stdout(predicate::str::contains("# Updated Line Literature"))
        .stdout(predicate::str::contains("updated line body"))
        .stdout(predicate::str::contains("stale reverse").not());

    assert_eq!(
        home.literature_data("LineLit"),
        (
            "@book{LineLit, title={Updated Line Literature}}".into(),
            "# Updated Line Literature\n\nupdated line body\n\n<!-- zt:reverse-links -->\n".into(),
        )
    );
}

#[test]
fn session_topic_command_keeps_title_validation() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd()
        .write_stdin("t\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "topic title must be non-empty single-line text",
        ));
}

#[test]
fn session_delete_and_move_use_pointer_confirmation_and_update_pointer() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("# Movable\n\nmove\n\n<!-- zt:reverse-links -->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("# Source\n\nsource [[0/1|a]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();

    home.cmd()
        .write_stdin(
            concat!(
                "go 0/1|a\n",
                "mv 0/2|a\n",
                "move\n",
                "del\n",
                "delete\n",
                "q\n",
            )
            .as_bytes(),
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("move verification:"))
        .stdout(predicate::str::contains("0/1|a -> 0/2|a"))
        .stdout(predicate::str::contains("delete verification:"))
        .stdout(predicate::str::contains("location: 0/2"));

    assert!(!home.location_exists("0/1|a"));
    assert!(!home.location_exists("0/2|a"));
    assert!(home.card_text("0/2").contains("[[0/2|a]]"));
    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::contains("0/2 Source -> 0/2|a"));
}

#[test]
fn topic_delete_removes_topic_and_topic_ids_are_not_reused() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    home.cmd()
        .args(["del", "--at", "0/0"])
        .write_stdin("0/0\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("delete verification:"))
        .stdout(predicate::str::contains("successors: 1"))
        .stdout(predicate::str::contains("deleted: 2"));

    assert!(!home.location_exists("0/0"));
    assert!(!home.location_exists("0/1"));
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total: 0"));

    home.cmd_with_editor("# Next Topic\n\nnext\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Next Topic"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1/0"));
    assert!(home.location_exists("1/0"));
}

/// D02 / R243 / S12-AC02: each of the four delete selections asks for its own word — `delete`, the
/// root address, the root address with keep-root, and `delete all` — and a wrong word or EOF cancels
/// without changing a Card, a Link or the archive revision. The exact word applies exactly that
/// selection, so D10's Topic allocation keeps advancing as before.
#[test]
fn delete_confirmations_refuse_a_wrong_word_or_eof_for_every_selection() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    for title in ["Second", "Keep"] {
        home.cmd_with_editor(&format!(
            "# {title}\n\n{title} body\n\n<!-- zt:reverse-links -->\n"
        ))
        .args(["t", title])
        .assert()
        .success();
    }
    home.cmd_with_editor("# Second base\n\nsecond base\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "1/0"])
        .assert()
        .success();
    home.cmd_with_editor("# Kept child\n\nkept child\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "2/0"])
        .assert()
        .success();

    // 1. A Regular Card's subtree asks for `delete`.
    let revision = home.revision();
    for input in ["nope\n", ""] {
        home.cmd()
            .args(["del", "--at", "0/1"])
            .write_stdin(input)
            .assert()
            .failure()
            .stdout(predicate::str::contains("type `delete` to confirm:"))
            .stderr(predicate::str::contains("confirmation did not match"));
        assert!(home.location_exists("0/1"), "a cancelled delete keeps the Card");
        assert_eq!(home.revision(), revision, "a cancelled delete moves no revision");
    }
    home.cmd()
        .args(["del", "--at", "0/1"])
        .write_stdin("delete\n")
        .assert()
        .success();
    assert!(!home.location_exists("0/1"));
    assert!(home.location_exists("0/0"), "the tree root stays");

    // 2. A whole tree asks for its root address; `delete` alone is the wrong word here.
    let revision = home.revision();
    for input in ["delete\n", ""] {
        home.cmd()
            .args(["del", "--at", "1/0"])
            .write_stdin(input)
            .assert()
            .failure()
            .stdout(predicate::str::contains("type `1/0` to confirm:"))
            .stderr(predicate::str::contains("confirmation did not match"));
        assert!(home.location_exists("1/0") && home.location_exists("1/1"));
        assert_eq!(home.revision(), revision, "a cancelled delete moves no revision");
    }
    home.cmd()
        .args(["del", "--at", "1/0"])
        .write_stdin("1/0\n")
        .assert()
        .success();
    assert!(!home.location_exists("1/0") && !home.location_exists("1/1"));

    // 3. Keep-root asks for the retained root's address; `delete` alone is the wrong word here too.
    let revision = home.revision();
    for input in ["delete\n", ""] {
        home.cmd()
            .args(["del", "--at", "2/0", "--keep-root"])
            .write_stdin(input)
            .assert()
            .failure()
            .stdout(predicate::str::contains("type `2/0` to confirm:"))
            .stderr(predicate::str::contains("confirmation did not match"));
        assert!(home.location_exists("2/0") && home.location_exists("2/1"));
        assert_eq!(home.revision(), revision, "a cancelled delete moves no revision");
    }
    home.cmd()
        .args(["del", "--at", "2/0", "--keep-root"])
        .write_stdin("2/0\n")
        .assert()
        .success();
    assert!(home.location_exists("2/0"), "keep-root retains the root");
    assert!(!home.location_exists("2/1"), "keep-root deletes the Regular Cards");

    // 4. The whole archive asks for `delete all`.
    let revision = home.revision();
    for input in ["delete\n", ""] {
        home.cmd()
            .args(["del", "--all"])
            .write_stdin(input)
            .assert()
            .failure()
            .stdout(predicate::str::contains("type `delete all` to confirm:"))
            .stderr(predicate::str::contains("confirmation did not match"));
        assert!(home.location_exists("0/0") && home.location_exists("2/0"));
        assert_eq!(home.revision(), revision, "a cancelled delete moves no revision");
    }
    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted: 2 cards; next topic id reset to 0"));
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total: 0"));
    assert_eq!(home.next_topic_id(), 0);
}

/// S12 (#135) R237–R238, D01: `zt del` takes exactly one selector. Both or neither selector, a
/// repeated selector or flag, an unknown argument, and `--keep-root` beside `--all` are refused by
/// the one usage line before the service is asked, a plan is made or a prompt is printed.
#[test]
fn delete_takes_exactly_one_selector_and_refuses_every_other_form() {
    // No configuration and no service: the syntax is decided by the command line alone.
    let home = TestHome::new();
    for args in [
        vec![],
        vec!["--at"],
        vec!["--at", "--all"],
        vec!["--at", "0/0", "--all"],
        vec!["--all", "--at", "0/0"],
        vec!["--all", "--all"],
        vec!["--at", "0/0", "--at", "0/1"],
        vec!["--keep-root"],
        vec!["--all", "--keep-root"],
        vec!["--at", "0/0", "--keep-root", "--keep-root"],
        vec!["--at", "0/0", "--keep-root", "extra"],
        vec!["--all", "extra"],
        vec!["--at", "0/0", "--force"],
    ] {
        home.cmd()
            .arg("del")
            .args(&args)
            .write_stdin("")
            .assert()
            .failure()
            .stdout(predicate::str::is_empty())
            .stderr(predicate::str::contains(
                "usage: zt del --at <target> [--keep-root] | zt del --all",
            ));
    }
}

/// S11 (#134) D03: `zt del --at <root> --keep-root` removes the Regular Cards of one Topic tree
/// and keeps its root Card, printing the exact success line.
#[test]
fn keep_root_empties_a_topic_tree_and_keeps_its_root() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    home.cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("0/0\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("deleted: 1; root kept: 0/0"));

    assert!(home.location_exists("0/0"), "the root stays");
    assert!(!home.location_exists("0/1"), "the Regular Card goes");
    assert_eq!(
        home.card_text("0/0"),
        "# Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n"
    );
}

/// S11 (#134) S11-AC03: the keep-root verification names the root, its Workspace, the number of
/// Regular Cards to remove, and the first 20 breaking-Link pairs with an exact remainder.
#[test]
fn keep_root_verification_names_the_root_workspace_count_and_breaking_links() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    // Twenty-four more Regular Cards (0/2 .. 0/25) through one Session process.
    let mut input = String::from("go 0/1\n");
    for _ in 0..24 {
        input.push_str("n\n# Card\n\nbody\n\n<!-- zt:reverse-links -->\n");
        input.push('\x13');
    }
    input.push_str("q\n");
    home.cmd().write_stdin(input.as_bytes()).assert().success();
    let links = (1..=25)
        .map(|index| format!("[[0/{index}]]"))
        .collect::<Vec<_>>()
        .join(" ");
    home.cmd_with_editor(&format!(
        "# Source\n\n{links}\n\n<!-- zt:reverse-links -->\n"
    ))
    .args(["t", "Source"])
    .assert()
    .success();

    let out = home
        .cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("0/0\n")
        .output()
        .expect("zt del --keep-root");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "zt del failed: {out:?}");
    for expected in [
        "delete verification:",
        "root kept: 0/0 (Topic, human workspace)",
        "delete regular cards: 25",
        "links that will break: 25",
        "1/0 -> 0/1",
        "1/0 -> 0/20",
        "... and 5 more",
        "deleted: 25; root kept: 0/0",
    ] {
        assert!(stdout.contains(expected), "missing {expected:?} in:\n{stdout}");
    }
    assert!(
        stdout.contains("1/0 -> 0/20"),
        "a later pair is still inside the first 20:\n{stdout}"
    );
    assert_eq!(
        stdout.matches("1/0 -> 0/").count(),
        20,
        "only the first 20 breaking pairs are printed:\n{stdout}"
    );
    assert!(home.location_exists("0/0"));
    assert!(!home.location_exists("0/25"));
}

/// S11 (#134) R262: an already empty unlocked tree still shows its plan and asks for the root's
/// address, then reports `deleted: 0` without a revision or a derived rebuild.
#[test]
fn keep_root_on_an_already_empty_tree_reports_zero_without_a_revision() {
    let home = TestHome::new();
    home.configure_and_up();
    home.cmd_with_editor("# Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    let before = home.revision();
    let text_before = home.card_text("0/0");

    home.cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("0/0\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("delete verification:"))
        .stdout(predicate::str::contains(
            "root kept: 0/0 (Topic, human workspace)",
        ))
        .stdout(predicate::str::contains("delete regular cards: 0"))
        .stdout(predicate::str::contains("deleted: 0; root kept: 0/0"));

    assert_eq!(home.revision(), before, "nothing changed, no revision");
    assert_eq!(home.card_text("0/0"), text_before);
    assert!(home.location_exists("0/0"));
}

/// S11 (#134) D03/D04/S11-AC05, S11-AC06, S11-AC09: emptying a Literature tree keeps the root's
/// row identity, address, kind, Workspace and BibTeX, keeps incoming Links to the root valid,
/// reports the root's and an outside Card's Links to the deleted children as breaking, drops the
/// deleted Spread positions, leaves the Topic allocation alone, reuses `<root>/1` and advances the
/// revision once.
#[test]
fn keep_root_keeps_the_literature_root_identity_links_and_allocation() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let (mut literature, _) = home.cmd_with_editor_sequence(&[
        "@book{KeepMe, title={Keep Me}}",
        "# Work\n\nwork\n\n<!-- zt:reverse-links -->\n",
    ]);
    literature.arg("l").assert().success();
    home.cmd_with_editor("# Child\n\nchild, back to [[KeepMe]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "KeepMe"])
        .assert()
        .success();
    home.cmd_with_editor("# Grandchild\n\ngrandchild\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "KeepMe/1"])
        .assert()
        .success();
    home.cmd_with_editor("continues [[KeepMe/1]]\n")
        .args(["e", "--at", "KeepMe", "--part", "text"])
        .assert()
        .success();
    home.cmd_with_editor("# Source\n\nroot [[KeepMe]] child [[KeepMe/1]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();
    // A Spread holding the root and one of its Regular Cards.
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('gui_spreads', ?1)",
        [r#"{"current":"s1","spreads":[{"id":"s1","name":"Spread","cards":[{"at":"KeepMe","x":0,"y":0},{"at":"KeepMe/1","x":40,"y":10}]}]}"#],
    )
    .expect("seed a Spread");
    drop(conn);
    let root_before = home.card_row("KeepMe");
    let revision_before = home.revision();
    let topics_before = home.next_topic_id();
    let ops_before = home.applied_ops();
    let text_before = home.literature_data("KeepMe").1;
    assert!(text_before.contains("- [[KeepMe/1]] Child"), "{text_before}");

    home.cmd()
        .args(["del", "--at", "KeepMe", "--keep-root"])
        .write_stdin("KeepMe\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("delete verification:"))
        .stdout(predicate::str::contains(
            "root kept: KeepMe (Literature, human workspace)",
        ))
        .stdout(predicate::str::contains("delete regular cards: 2"))
        .stdout(predicate::str::contains("links that will break: 2"))
        .stdout(predicate::str::contains("KeepMe -> KeepMe/1"))
        .stdout(predicate::str::contains("0/2 -> KeepMe/1"))
        .stdout(predicate::str::contains(
            "deleted: 2; root kept: KeepMe",
        ));

    // The root keeps its row identity, address, kind, Workspace and BibTeX.
    assert_eq!(home.card_row("KeepMe"), root_before);
    assert!(!home.location_exists("KeepMe/1"));
    assert!(!home.location_exists("KeepMe/2"));
    // Its authored body is untouched and its generated Reverse section names only the Card that
    // still links it, so the outside Link to the root stays valid while the child Link breaks.
    let authored = |text: &str| {
        text.split("<!-- zt:reverse-links -->")
            .next()
            .unwrap_or(text)
            .to_string()
    };
    let text = home.literature_data("KeepMe").1;
    assert_eq!(
        authored(&text),
        authored(&text_before),
        "the root's authored identity is unchanged"
    );
    assert!(text.contains("continues [[KeepMe/1]]"), "{text}");
    assert!(text.contains("- [[0/2]] Source"), "{text}");
    assert!(!text.contains("- [[KeepMe/1]] Child"), "{text}");
    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::contains("0/2 Source -> KeepMe/1"))
        .stdout(predicate::str::contains("KeepMe Keep Me -> KeepMe/1"))
        .stdout(predicate::str::contains("-> KeepMe:").not());
    home.cmd()
        .arg("status")
        .args(["--check-links"])
        .assert()
        .success();
    // The deleted positions left the Spread; the retained root kept its place.
    let spreads = home.metadata("gui_spreads");
    assert!(spreads.contains("\"at\":\"KeepMe\""), "{spreads}");
    assert!(!spreads.contains("KeepMe/1"), "{spreads}");
    // The global Topic allocation and the archive revision move once, exactly; the retained
    // operation receipts are untouched, so a rerun needs a fresh prefix rather than a replay.
    assert_eq!(home.next_topic_id(), topics_before);
    assert_eq!(home.revision(), revision_before + 1);
    assert_eq!(home.applied_ops(), ops_before);
    // The existing successor rule puts the next Regular Card back at `<root>/1`.
    home.cmd_with_editor("# Again\n\nagain\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "KeepMe"])
        .assert()
        .success()
        .stdout("KeepMe/1\n");
    assert!(home.location_exists("KeepMe/1"));
}

/// S11 (#134) D05 / S11-AC07: emptying a tree is allowed while a Session is open, and a live
/// Session whose Pointer was on a removed Regular Card recovers to the retained root.
#[test]
fn keep_root_is_allowed_while_a_session_holds_a_pointer_on_a_removed_child() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("start a line Session");
    let mut stdin = child.stdin.take().expect("session stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("session stdout"));
    let mut seen = String::new();
    stdin.write_all(b"go 0/1\n").expect("send go");
    stdin.flush().expect("flush");
    wait_for_session_output(&mut stdout, &mut seen, "location: 0/1");

    let emptied = home
        .cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("0/0\n")
        .output()
        .expect("zt del --keep-root");
    assert!(
        emptied.status.success(),
        "allowed while a Session is open: {emptied:?}"
    );
    assert!(!home.location_exists("0/1"));
    assert!(home.location_exists("0/0"));

    stdin.write_all(b"stats\n").expect("send stats");
    stdin.flush().expect("flush");
    wait_for_session_output(&mut stdout, &mut seen, "card `0/1` no longer exists");
    wait_for_session_output(&mut stdout, &mut seen, "location: 0/0");
    stdin.write_all(b"q\n").expect("send q");
    drop(stdin);
    assert!(child.wait().expect("wait for the Session").success());
}

/// S11 (#134) S11-AC01: a wrong kind, a malformed selector, a wrong word or EOF change nothing.
#[test]
fn keep_root_refuses_a_regular_card_and_its_cancellations_change_nothing() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let revision = home.revision();
    let root_text = home.card_text("0/0");
    let base_text = home.card_text("0/1");

    // A Regular Card is not a Tree root: the root-kind error comes before any prompt.
    home.cmd()
        .args(["del", "--at", "0/1", "--keep-root"])
        .write_stdin("")
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "keep root takes a Tree root (a Topic like 3/0 or a Citation key); `0/1` is a Regular Card",
        ));
    // A wrong word and an empty stdin cancel without a change.
    home.cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("delete\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("type `0/0` to confirm:"))
        .stderr(predicate::str::contains("confirmation did not match"));
    home.cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("")
        .assert()
        .failure();

    assert_eq!(home.revision(), revision, "no cancellation changed the archive");
    assert_eq!(home.card_text("0/0"), root_text);
    assert_eq!(home.card_text("0/1"), base_text);
}

/// S11 (#134) S11-AC04: a failure during the delete rolls every affected Card, Spread and revision
/// back, so the tree is untouched.
#[test]
fn keep_root_rolls_every_effect_back_when_the_delete_fails() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("# Second\n\nsecond\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();
    let spreads = r#"{"current":"s1","spreads":[{"id":"s1","name":"Spread","cards":[{"at":"0/0","x":0,"y":0},{"at":"0/1","x":10,"y":0}]}]}"#;
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('gui_spreads', ?1)",
        [spreads],
    )
    .expect("seed a Spread");
    conn.execute(
        "CREATE TRIGGER test_delete_failure BEFORE DELETE ON cards WHEN OLD.location = '0/2'\
         BEGIN SELECT RAISE(ABORT, 'test delete failure'); END",
        [],
    )
    .expect("a failing delete step");
    drop(conn);
    let revision = home.revision();

    home.cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("0/0\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("test delete failure"));

    // The first DELETE had already run inside the transaction; the failure undid it.
    for address in ["0/0", "0/1", "0/2"] {
        assert!(home.location_exists(address), "{address} was not rolled back");
    }
    assert_eq!(home.revision(), revision, "a failed delete moves no revision");
    assert_eq!(home.metadata("gui_spreads"), spreads, "the Spread is untouched");
}

/// D11 / S11-AC06: keep-root follows saved Spread positions, so the Spread keeps the retained
/// root's position and loses the deleted Regular Card's; all-archive instead keeps the stored
/// metadata bytes unchanged (see `delete_all_keeps_saved_spread_metadata_while_a_tree_delete_follows_positions`).
#[test]
fn keep_root_drops_a_deleted_card_from_saved_spread_positions() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let spreads = r#"{"current":"s1","spreads":[{"id":"s1","name":"Spread","cards":[{"at":"0/0","x":0,"y":0},{"at":"0/1","x":10,"y":0}]}]}"#;
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('gui_spreads', ?1)",
        [spreads],
    )
    .expect("seed a Spread");
    drop(conn);

    home.cmd()
        .args(["del", "--at", "0/0", "--keep-root"])
        .write_stdin("0/0\n")
        .assert()
        .success();

    let stored = home.metadata("gui_spreads");
    assert!(
        !stored.contains("\"at\":\"0/1\""),
        "the deleted Card's Spread position goes: {stored}"
    );
    assert!(
        stored.contains("\"at\":\"0/0\""),
        "the retained root's Spread position stays: {stored}"
    );
    assert!(home.location_exists("0/0"), "the retained root still exists");
}

#[test]
fn side_successor_labels_roll_over_from_z_to_aa() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut input = String::from("go 0/1\n");
    for index in 1..=27 {
        input.push_str("b\n");
        input.push_str(&format!(
            "# Side {index}\n\nbody {index}\n\n<!-- zt:reverse-links -->\n"
        ));
        input.push('\x13');
        input.push_str("go 0/1\n");
    }
    input.push_str("q\n");

    home.cmd().write_stdin(input).assert().success();
    assert!(home.location_exists("0/1|z"));
    assert!(home.location_exists("0/1|aa"));
    assert!(home.card_text("0/1|aa").contains("Side 27"));
}

#[test]
fn service_down_availability_help_and_edit_lock_rules_are_enforced() {
    let home = TestHome::new();

    home.cmd()
        .arg("version")
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^zt \d+\.\d+\.\d+\n$").unwrap());
    home.cmd()
        .args(["config", "show"])
        .assert()
        .success()
        .stdout(predicate::str::contains("archive_root=<unset>"));
    home.cmd()
        .arg("stats")
        .assert()
        .failure()
        .stderr(predicate::str::contains("archive_root is not configured"));

    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("zt n --at <target>").not());
    home.cmd()
        .arg("stats")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));
    home.cmd_with_editor("# Nope\n\nnope\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Nope"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));

    home.cmd().arg("up").assert().success();
    home.create_topic_and_base();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("zt n --at <target>"))
        .stdout(predicate::str::contains("zt lsbk"))
        .stdout(predicate::str::contains("zt stats"))
        .stdout(predicate::str::contains("zt help"))
        .stdout(predicate::str::contains("zt search").not());

    fs::write(home.archive_root.join("zt.edit.lock"), "held").expect("lock");
    home.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: up"));
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total: 2"));
    home.cmd_with_editor("# Blocked\n\nblocked\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("edit in progress"));
    fs::remove_file(home.archive_root.join("zt.edit.lock")).expect("unlock");

    let mut editing = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn editing session");
    editing
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"go 0/1\ne\n")
        .expect("enter edit");
    home.wait_until(|| home.archive_root.join("zt.edit.lock").exists());
    home.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: up"));
    home.cmd_with_editor("# Blocked\n\nblocked\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("edit in progress"));
    editing
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"\x1bq\n")
        .expect("cancel and quit");
    let editing_output = editing.wait_with_output().expect("editing session");
    assert!(editing_output.status.success());

    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    home.wait_until(|| {
        fs::read_to_string(home.archive_root.join("zt.sessions"))
            .is_ok_and(|pids| pids.lines().any(|pid| pid.trim() == child.id().to_string()))
    });
    home.cmd()
        .arg("down")
        .assert()
        .failure()
        .stderr(predicate::str::contains("session(s) are open"));
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"q\n")
        .expect("quit");
    let output = child.wait_with_output().expect("session output");
    assert!(output.status.success());
    home.cmd().arg("down").assert().success();
}

#[test]
fn dump_is_shell_only_and_requires_service_up() {
    let home = TestHome::new();
    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();

    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("zt dp").not());
    home.cmd()
        .arg("dp")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));

    home.cmd().arg("up").assert().success();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("  zt dp\n"))
        // S13-AC05: a dump with Lock evidence is not a restore command; the CLI offers no way back.
        .stdout(predicate::str::contains("restore").not());
    home.cmd()
        .write_stdin("help\ndp\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown session command: dp"))
        .stdout(predicate::str::contains("zt dp").not());
}

#[test]
fn empty_dump_creates_timestamped_zip_with_empty_mapping() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 0 cards to "));

    let archive_path = single_dump_archive(&home, ".zip");
    assert!(!home.archive_root.join("dump").join(".tmp").exists());
    let archive_name = archive_path.file_stem().unwrap().to_str().unwrap();
    let timestamp = archive_name
        .strip_prefix("zt-archive-")
        .expect("timestamped archive name")
        .parse::<u64>()
        .expect("Unix timestamp");
    assert!(timestamp >= started);
    assert!(timestamp <= started + 5);

    let mut archive =
        zip::ZipArchive::new(File::open(&archive_path).expect("open ZIP")).expect("read ZIP");
    assert_eq!(archive.len(), 3);
    let mut mapping = archive
        .by_name(&format!("{archive_name}/mapping.json"))
        .expect("mapping entry");
    let mut body = String::new();
    std::io::Read::read_to_string(&mut mapping, &mut body).expect("mapping text");
    assert_eq!(body, "{}");
    drop(mapping);
    let mut manifest = archive
        .by_name(&format!("{archive_name}/archive.json"))
        .expect("archive manifest entry");
    let mut body = String::new();
    std::io::Read::read_to_string(&mut manifest, &mut body).expect("manifest text");
    assert_eq!(
        body,
        r#"{"next_topic_id":0,"workspaces":{},"types":{},"term_names":{},"facets":{},"emphasis":{},"levels":{},"forms":{},"hub_levels":{}}"#
    );
    drop(manifest);
    // R315, S13-AC02: a dump of an archive that holds no Lock still carries the version-1 evidence.
    let locks = zip_dump_locks(&mut archive, archive_name);
    assert_eq!(locks["schema_version"], serde_json::json!(1));
    assert_eq!(locks["revision"], serde_json::json!(home.revision()));
    assert_eq!(locks["locks"], serde_json::json!([]));
    assert_eq!(locks["events"], serde_json::json!([]));
    assert_eq!(locks["integrity"]["ok"], serde_json::json!(true));
    assert_eq!(locks["integrity"]["summary"], serde_json::json!("integrity ok"));
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let identity: String = conn
        .query_row("SELECT value FROM metadata WHERE key='archive_id'", [], |row| {
            row.get(0)
        })
        .expect("archive identity");
    assert_eq!(locks["archive"], serde_json::json!(identity));
}

#[test]
fn dump_preserves_card_text_hash_mapping_and_location_order() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    let topic_text =
        "# Topic\n\nfirst line\n\u{7b2c}\u{4e8c}\u{884c}\n\n<!-- zt:reverse-links -->\n";
    let identical_text = "# Same\n\n\u{76f8}\u{540c}\nline two\n\n<!-- zt:reverse-links -->\n";
    home.cmd_with_editor(topic_text)
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor(identical_text)
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor(identical_text)
        .args(["b", "--at", "0/1"])
        .assert()
        .success();

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 3 cards to "));

    let archive_path = single_dump_archive(&home, ".zip");
    let archive_name = archive_path.file_stem().unwrap().to_str().unwrap();
    let cards = [
        ("0/0", topic_text),
        ("0/1", identical_text),
        ("0/1|a", identical_text),
    ];
    let expected_mapping: BTreeMap<String, String> = cards
        .iter()
        .map(|(location, text)| {
            (
                (*location).to_string(),
                expected_dump_filename(location, text),
            )
        })
        .collect();

    let mut archive =
        zip::ZipArchive::new(File::open(&archive_path).expect("open ZIP")).expect("read ZIP");
    let expected_entries: Vec<String> = [
        format!("{archive_name}/mapping.json"),
        format!("{archive_name}/archive.json"),
        format!("{archive_name}/locks.json"),
    ]
    .into_iter()
    .chain(
        cards
            .iter()
            .map(|(location, _)| format!("{archive_name}/{}", expected_mapping[*location])),
    )
    .collect();
    let actual_entries: Vec<String> = (0..archive.len())
        .map(|index| {
            archive
                .by_index(index)
                .expect("ZIP entry")
                .name()
                .to_string()
        })
        .collect();
    assert_eq!(actual_entries, expected_entries);

    let mut mapping_body = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{archive_name}/mapping.json"))
            .expect("mapping entry"),
        &mut mapping_body,
    )
    .expect("mapping text");
    let mapping: BTreeMap<String, String> =
        serde_json::from_str(&mapping_body).expect("mapping JSON object");
    assert_eq!(mapping, expected_mapping);
    let key_positions: Vec<usize> = cards
        .iter()
        .map(|(location, _)| mapping_body.find(&format!("\"{location}\"")).unwrap())
        .collect();
    assert!(key_positions.windows(2).all(|pair| pair[0] < pair[1]));

    for (location, expected_text) in cards {
        let mut actual_text = String::new();
        std::io::Read::read_to_string(
            &mut archive
                .by_name(&format!("{archive_name}/{}", expected_mapping[location]))
                .expect("Card Markdown entry"),
            &mut actual_text,
        )
        .expect("Card Markdown text");
        assert_eq!(actual_text, expected_text);
    }
    assert_ne!(expected_mapping["0/1"], expected_mapping["0/1|a"]);
}

#[test]
fn dump_adds_hashed_literature_markdown_and_exact_citation_key_bibtex() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    let bibtex = "@article{DumpKey,\n  title={Dump Literature}\n}\n\n";
    let card_text = "# Dump Literature\n\nnotes\n\n<!-- zt:reverse-links -->\n";
    let (mut create, _) = home.cmd_with_editor_sequence(&[bibtex, card_text]);
    create.arg("l").assert().success();

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 1 cards to "));

    let archive_path = single_dump_archive(&home, ".zip");
    let archive_name = archive_path.file_stem().unwrap().to_str().unwrap();
    let markdown_name = expected_literature_dump_filename("DumpKey", bibtex, card_text);
    let mut archive =
        zip::ZipArchive::new(File::open(&archive_path).expect("open ZIP")).expect("read ZIP");
    let actual_entries = (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_entries,
        vec![
            format!("{archive_name}/mapping.json"),
            format!("{archive_name}/archive.json"),
            format!("{archive_name}/locks.json"),
            format!("{archive_name}/{markdown_name}"),
            format!("{archive_name}/DumpKey.bib"),
        ]
    );

    let mut mapping = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{archive_name}/mapping.json"))
            .expect("mapping entry"),
        &mut mapping,
    )
    .expect("mapping text");
    assert_eq!(
        serde_json::from_str::<BTreeMap<String, String>>(&mapping).unwrap()["DumpKey"],
        markdown_name
    );

    let mut markdown = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{archive_name}/{markdown_name}"))
            .expect("Literature Markdown"),
        &mut markdown,
    )
    .expect("Markdown text");
    assert!(markdown.starts_with(
        "---\nzt_kind: literature\ncitation_key: DumpKey\nbibtex: |+\n  @article{DumpKey,\n    title={Dump Literature}\n  }\n  \n---\n"
    ));
    assert!(markdown.ends_with(card_text));

    let mut exact_bibtex = Vec::new();
    std::io::Read::read_to_end(
        &mut archive
            .by_name(&format!("{archive_name}/DumpKey.bib"))
            .expect("BibTeX entry"),
        &mut exact_bibtex,
    )
    .expect("BibTeX bytes");
    assert_eq!(exact_bibtex, bibtex.as_bytes());
}

#[test]
fn dump_orders_mixed_markdown_by_address_then_bibtex_by_citation_key() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    let topic_text = "# Topic\n\nregular payload stays exact\n\n<!-- zt:reverse-links -->\n";
    home.cmd_with_editor(topic_text)
        .args(["t", "Topic"])
        .assert()
        .success();
    let literature = [
        (
            "ZebraDump",
            "@misc{ZebraDump, title={Zebra Dump}}\n",
            "# Zebra Dump\n\nzebra\n\n<!-- zt:reverse-links -->\n",
        ),
        (
            "AlphaDump",
            "@book{AlphaDump, title={Alpha Dump}}",
            "# Alpha Dump\n\nalpha\n\n<!-- zt:reverse-links -->\n",
        ),
    ];
    for (_, bibtex, text) in literature {
        let (mut create, _) = home.cmd_with_editor_sequence(&[bibtex, text]);
        create.arg("l").assert().success();
    }

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 3 cards to "));

    let topic_md = expected_dump_filename("0/0", topic_text);
    let alpha_md = expected_literature_dump_filename("AlphaDump", literature[1].1, literature[1].2);
    let zebra_md = expected_literature_dump_filename("ZebraDump", literature[0].1, literature[0].2);
    let archive_path = single_dump_archive(&home, ".zip");
    let archive_name = archive_path.file_stem().unwrap().to_str().unwrap();
    let mut archive =
        zip::ZipArchive::new(File::open(&archive_path).expect("open ZIP")).expect("read ZIP");
    let actual_entries = (0..archive.len())
        .map(|index| archive.by_index(index).unwrap().name().to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        actual_entries,
        vec![
            format!("{archive_name}/mapping.json"),
            format!("{archive_name}/archive.json"),
            format!("{archive_name}/locks.json"),
            format!("{archive_name}/{topic_md}"),
            format!("{archive_name}/{alpha_md}"),
            format!("{archive_name}/{zebra_md}"),
            format!("{archive_name}/AlphaDump.bib"),
            format!("{archive_name}/ZebraDump.bib"),
        ]
    );

    let mut mapping_body = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{archive_name}/mapping.json"))
            .expect("mapping entry"),
        &mut mapping_body,
    )
    .expect("mapping body");
    assert_eq!(
        serde_json::from_str::<BTreeMap<String, String>>(&mapping_body).unwrap(),
        BTreeMap::from([
            ("0/0".into(), topic_md),
            ("AlphaDump".into(), alpha_md),
            ("ZebraDump".into(), zebra_md),
        ])
    );
    let positions =
        ["\"0/0\"", "\"AlphaDump\"", "\"ZebraDump\""].map(|key| mapping_body.find(key).unwrap());
    assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(!mapping_body.contains(".bib"));
}

#[test]
fn repeated_dumps_keep_payload_order_and_mapping_bytes_stable() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd().arg("dp").write_stdin("1\n").assert().success();
    let first_second = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    while std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        == first_second
    {
        thread::sleep(Duration::from_millis(10));
    }
    home.cmd().arg("dp").write_stdin("1\n").assert().success();

    let mut archives: Vec<_> = fs::read_dir(home.archive_root.join("dump"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().ends_with(".zip"))
        .collect();
    archives.sort();
    assert_eq!(archives.len(), 2);
    let snapshots: Vec<_> = archives
        .iter()
        .map(|path| {
            let mut archive = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
            let entries: Vec<_> = (0..archive.len())
                .map(|index| {
                    let name = archive.by_index(index).unwrap().name().to_string();
                    name.split_once('/').unwrap().1.to_string()
                })
                .collect();
            let mapping_name = archive
                .file_names()
                .find(|name| name.ends_with("/mapping.json"))
                .unwrap()
                .to_string();
            let mut mapping = Vec::new();
            std::io::Read::read_to_end(&mut archive.by_name(&mapping_name).unwrap(), &mut mapping)
                .unwrap();
            let locks_name = archive
                .file_names()
                .find(|name| name.ends_with("/locks.json"))
                .expect("every dump names locks.json")
                .to_string();
            let mut locks = Vec::new();
            std::io::Read::read_to_end(&mut archive.by_name(&locks_name).unwrap(), &mut locks)
                .unwrap();
            // S13: a dump changes no archive fact, so two dumps of one state carry the same bytes.
            (entries, mapping, locks)
        })
        .collect();
    assert_eq!(snapshots[0], snapshots[1]);
}

#[test]
fn selecting_tar_gz_creates_the_same_timestamped_payload() {
    if !host_zip_backend_available() || !host_tar_gz_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    home.cmd()
        .arg("dp")
        .write_stdin("2\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("1. zip"))
        .stdout(predicate::str::contains("2. tar.gz"))
        .stdout(predicate::str::contains("dumped 0 cards to "));

    let archive_path = single_dump_archive(&home, ".tar.gz");
    let filename = archive_path.file_name().unwrap().to_str().unwrap();
    let archive_name = filename.strip_suffix(".tar.gz").unwrap();
    let entry = format!("{archive_name}/mapping.json");
    let locks_entry = format!("{archive_name}/locks.json");
    let listing = StdCommand::new(host_tools::host_tar())
        .arg("-tf")
        .arg(&archive_path)
        .output()
        .expect("list tar.gz");
    assert!(listing.status.success());
    assert_eq!(
        String::from_utf8(listing.stdout)
            .unwrap()
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>(),
        [
            entry.clone(),
            format!("{archive_name}/archive.json"),
            locks_entry.clone(),
        ]
    );
    assert_eq!(dump_entry_text(&archive_path, &entry), "{}");
    // S13-AC04/S13-AC06: a tar.gz dump names and holds the same version-1 lock evidence.
    let locks: serde_json::Value =
        serde_json::from_str(&dump_entry_text(&archive_path, &locks_entry)).expect("locks.json");
    assert_eq!(locks["schema_version"], serde_json::json!(1));
    assert_eq!(locks["locks"], serde_json::json!([]));
    assert_eq!(locks["integrity"]["ok"], serde_json::json!(true));
    // The same captured state in another archive format carries the same evidence bytes, so the
    // explicit entry list of every format names one document.
    home.cmd().arg("dp").write_stdin("1\n").assert().success();
    let zip_path = single_dump_archive(&home, ".zip");
    let zip_name = zip_path.file_stem().unwrap().to_str().unwrap();
    let mut zip_archive =
        zip::ZipArchive::new(File::open(&zip_path).expect("open ZIP")).expect("read ZIP");
    let mut zip_locks = String::new();
    std::io::Read::read_to_string(
        &mut zip_archive
            .by_name(&format!("{zip_name}/locks.json"))
            .expect("ZIP locks.json"),
        &mut zip_locks,
    )
    .expect("ZIP locks.json text");
    assert_eq!(zip_locks, dump_entry_text(&archive_path, &locks_entry));
}

#[test]
fn selecting_tar_zst_creates_the_same_timestamped_payload() {
    if !host_zip_backend_available()
        || !host_tar_gz_backend_available()
        || !host_tar_zst_backend_available()
    {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    home.cmd()
        .arg("dp")
        .write_stdin("3\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("3. tar.zst"))
        .stdout(predicate::str::contains("dumped 0 cards to "));

    let archive_path = single_dump_archive(&home, ".tar.zst");
    let filename = archive_path.file_name().unwrap().to_str().unwrap();
    let archive_name = filename.strip_suffix(".tar.zst").unwrap();
    let entry = format!("{archive_name}/mapping.json");
    let locks_entry = format!("{archive_name}/locks.json");
    let listing = StdCommand::new(host_tools::host_tar())
        .arg("-tf")
        .arg(&archive_path)
        .output()
        .expect("list tar.zst");
    assert!(listing.status.success());
    assert_eq!(
        String::from_utf8(listing.stdout)
            .unwrap()
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>(),
        [
            entry.clone(),
            format!("{archive_name}/archive.json"),
            locks_entry.clone(),
        ]
    );
    assert_eq!(dump_entry_text(&archive_path, &entry), "{}");
    // S13-AC04/S13-AC06: a tar.zst dump names and holds the same version-1 lock evidence.
    let locks: serde_json::Value =
        serde_json::from_str(&dump_entry_text(&archive_path, &locks_entry)).expect("locks.json");
    assert_eq!(locks["schema_version"], serde_json::json!(1));
    assert_eq!(locks["locks"], serde_json::json!([]));
    assert_eq!(locks["integrity"]["ok"], serde_json::json!(true));
}

#[test]
fn dump_rejects_compression_flags_and_invalid_prompt_input_before_staging() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    home.cmd()
        .args(["dp", "--compress", "zip"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown command"));
    for choice in ["0\n", "01\n", "zip\n"] {
        home.cmd()
            .arg("dp")
            .write_stdin(choice)
            .assert()
            .failure()
            .stderr(predicate::str::contains("invalid compression option"));
    }
    assert!(!home.archive_root.join("dump").exists());
}

#[cfg(windows)]
#[test]
fn dump_probe_falls_back_to_7zz_and_lists_only_the_usable_format() {
    let home = TestHome::new();
    home.configure_and_up();
    let compressors = build_probe_matrix();

    home.cmd()
        .current_dir(compressors.path())
        .env("PATH", path_with_only_fakes(compressors.path()))
        .env("ZT_FAKE_TAR_ALWAYS_FAIL", "1")
        .arg("dp")
        .write_stdin("0\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("1. 7z"))
        .stdout(predicate::str::contains("zip").not())
        .stdout(predicate::str::contains("tar.gz").not())
        .stdout(predicate::str::contains("tar.zst").not())
        .stderr(predicate::str::contains("invalid compression option"));
    assert!(!home.archive_root.join("dump").exists());
}

#[cfg(windows)]
#[test]
fn dump_reports_all_supported_names_when_every_probe_fails() {
    let home = TestHome::new();
    home.configure_and_up();
    let compressors = build_probe_matrix();

    home.cmd()
        .current_dir(compressors.path())
        .env("PATH", path_with_only_fakes(compressors.path()))
        .env("ZT_FAKE_TAR_ALWAYS_FAIL", "1")
        .env("ZT_FAKE_7Z_ALL_FAIL", "1")
        .arg("dp")
        .assert()
        .failure()
        .stdout(predicate::str::contains("compression options").not())
        .stderr(predicate::str::contains(
            "no supported compression option detected; supported options: zip, tar.gz, tar.zst, 7z",
        ));
    assert!(!home.archive_root.join("dump").exists());
}

#[cfg(windows)]
#[test]
fn selecting_7zz_creates_the_same_timestamped_payload() {
    let home = TestHome::new();
    home.configure_and_up();
    let compressors = build_probe_matrix();

    home.cmd()
        .current_dir(compressors.path())
        .env("PATH", path_with_only_fakes(compressors.path()))
        .env("ZT_FAKE_TAR_ALWAYS_FAIL", "1")
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("1. 7z"))
        .stdout(predicate::str::contains("dumped 0 cards to "));

    let archive_path = single_dump_archive(&home, ".7z");
    let filename = archive_path.file_name().unwrap().to_str().unwrap();
    let archive_name = filename.strip_suffix(".7z").unwrap();
    let entry = format!("{archive_name}/mapping.json");
    let locks_entry = format!("{archive_name}/locks.json");
    let system_tar = host_tools::host_tar();
    let listing = StdCommand::new(&system_tar)
        .arg("-tf")
        .arg(&archive_path)
        .output()
        .expect("list 7z");
    assert!(listing.status.success());
    assert_eq!(
        String::from_utf8(listing.stdout)
            .unwrap()
            .lines()
            .map(str::trim)
            .collect::<Vec<_>>(),
        [
            entry.clone(),
            format!("{archive_name}/archive.json"),
            locks_entry.clone(),
        ]
    );
    assert_eq!(dump_entry_text(&archive_path, &entry), "{}");
    // S13-AC04/S13-AC06: a 7z dump names and holds the same version-1 lock evidence.
    let locks: serde_json::Value =
        serde_json::from_str(&dump_entry_text(&archive_path, &locks_entry)).expect("locks.json");
    assert_eq!(locks["schema_version"], serde_json::json!(1));
    assert_eq!(locks["locks"], serde_json::json!([]));
    assert_eq!(locks["integrity"]["ok"], serde_json::json!(true));
}

#[test]
fn dump_refuses_edit_lock_but_runs_with_an_open_session() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    fs::write(home.archive_root.join("zt.edit.lock"), "held").expect("edit lock");
    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("edit in progress"));
    assert!(!home.archive_root.join("dump").exists());
    fs::remove_file(home.archive_root.join("zt.edit.lock")).expect("unlock");

    let mut session = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    home.wait_until(|| {
        fs::read_to_string(home.archive_root.join("zt.sessions"))
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false)
    });
    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 0 cards to "));
    assert!(single_dump_archive(&home, ".zip").is_file());
    assert!(!home.archive_root.join("zt.edit.lock").exists());

    session
        .stdin
        .as_mut()
        .expect("session stdin")
        .write_all(b"q\n")
        .expect("quit session");
    assert!(session.wait().expect("session exit").success());
}

#[cfg(windows)]
#[test]
fn failed_compression_removes_partial_archive_and_timestamped_staging() {
    let home = TestHome::new();
    home.configure_and_up();
    let fake_tar = build_failing_tar();

    home.cmd()
        .current_dir(fake_tar.path())
        .env("PATH", path_with_only_fakes(fake_tar.path()))
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("1. zip"))
        .stdout(predicate::str::contains("2. tar.gz"))
        .stdout(predicate::str::contains("3. tar.zst"))
        .stderr(predicate::str::contains(
            "compression command for zip failed",
        ));

    let dump_root = home.archive_root.join("dump");
    let files: Vec<_> = fs::read_dir(&dump_root)
        .expect("dump root")
        .map(|entry| entry.expect("dump entry").path())
        .filter(|path| path.is_file())
        .collect();
    assert!(files.is_empty(), "partial archive remains: {files:?}");
    let staging_root = dump_root.join(".tmp");
    assert!(!staging_root.exists(), "staging root remains");
    assert!(!home.archive_root.join("zt.edit.lock").exists());
}

#[cfg(windows)]
#[test]
fn preexisting_dump_archive_is_never_overwritten_or_cleaned_up() {
    let home = TestHome::new();
    home.configure_and_up();
    let fake_tar = build_failing_tar();
    let dump_root = home.archive_root.join("dump");
    fs::create_dir_all(&dump_root).expect("dump root");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let protected: Vec<_> = (now..=now + 10)
        .map(|timestamp| dump_root.join(format!("zt-archive-{timestamp}.zip")))
        .collect();
    for archive in &protected {
        fs::write(archive, b"existing archive").expect("protected archive");
    }

    home.cmd()
        .current_dir(fake_tar.path())
        .env("PATH", path_with_only_fakes(fake_tar.path()))
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("archive already exists"));

    for archive in protected {
        assert_eq!(fs::read(archive).unwrap(), b"existing archive");
    }
    assert!(!dump_root.join(".tmp").exists());
    assert!(!home.archive_root.join("zt.edit.lock").exists());
}



/// S12 (#135) S12-AC01, S12-AC08 / D12: the shell advertises only the one supported delete command,
/// the legacy `zt clear` prints its one-release notice and exits 1 without the service, a prompt or
/// a deletion, and the whole-archive form reads exactly `delete all`.
#[test]
fn delete_all_needs_the_exact_word_and_clear_is_only_a_migration_notice() {
    let home = TestHome::new();
    // `zt clear` needs no configuration and no service: it prints the notice and exits 1.
    home.cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .failure()
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains("zt clear is now `zt del --all`"));
    // The whole-archive delete needs the archive: while the service is down nothing is deleted.
    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("to confirm").not())
        .stderr(predicate::str::contains("service is not up"));

    home.cmd().arg("up").assert().success();
    home.create_topic_and_base();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("  zt del --all\n"))
        .stdout(predicate::str::contains("  zt clear\n").not())
        .stdout(predicate::str::contains("zt clean").not());
    home.cmd()
        .arg("clean")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown command"));
    // Every other word and EOF cancel with no change. Only the line terminator is removed, so a
    // padded or recased word is not the confirmation.
    for input in [
        "",
        "delete\n",
        "DELETE ALL\n",
        "delete all \n",
        " delete all\n",
        "delete  all\n",
        "clear\n",
    ] {
        home.cmd()
            .args(["del", "--all"])
            .write_stdin(input)
            .assert()
            .failure()
            .stdout(predicate::str::contains("type `delete all` to confirm:"))
            .stderr(predicate::str::contains("confirmation did not match"));
    }
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    // The Session offers no delete-all form and no `clear`.
    home.cmd()
        .write_stdin("help\ndel all\nclear\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown session command: clear"))
        .stdout(predicate::str::contains("usage: del [keep-root]"))
        .stdout(predicate::str::contains("--all").not());
}

/// S12 (#135) S12-AC02, S12-AC06, S12-AC07 / D11: `zt del --all` deletes every Card of both
/// Workspaces and resets the Topic allocation to zero, printing the exact success line. Only the
/// Cards and their derived Links change: configuration, saved dumps and artifacts stay, and the
/// next Topic is again `0/0` with its first Regular Card at `0/1`.
#[test]
fn delete_all_empties_both_workspaces_resets_allocation_and_keeps_config_dumps_and_artifacts() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("# Other\n\nother\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Other"])
        .assert()
        .success();
    home.cmd_with_editor("# Agent Topic\n\nagent\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "--ws", "agent", "Agent Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Linked\n\nsee [[0/0]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();
    let config_path = home.config_dir.join("config.toml");
    let config_before = fs::read(&config_path).expect("config bytes");
    let dump_marker = home.archive_root.join("dump").join("existing.dump");
    fs::create_dir_all(dump_marker.parent().unwrap()).expect("dump root");
    fs::write(&dump_marker, b"keep this dump").expect("dump marker");
    let artifact = home.archive_root.join("artifacts").join("figure.png");
    fs::create_dir_all(artifact.parent().unwrap()).expect("artifact root");
    fs::write(&artifact, b"keep this figure").expect("artifact marker");
    // A receipt that survives the reset (R267, R277): old keys are never replayed to recreate Cards.
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS applied_ops(op_key TEXT NOT NULL PRIMARY KEY, request_hash TEXT NOT NULL, result_json TEXT NOT NULL, committed_at TEXT NOT NULL);
         INSERT INTO applied_ops(op_key, request_hash, result_json, committed_at)
         VALUES('run1:op1', 'hash', '{\"at\":\"0/2\"}', '2026-10-21T00:00:00.000Z');",
    )
    .expect("seed a receipt");
    drop(conn);

    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("type `delete all` to confirm:"))
        .stdout(predicate::str::contains("delete verification:\ncards: 5\nnext topic id: 3 -> 0\n"))
        .stdout(predicate::str::contains(
            "deleted: 5 cards; next topic id reset to 0\n",
        ));
    home.cmd().arg("stats").assert().success().stdout(
        predicate::str::is_match(r"^total: 0\ntopics: 0\nregular: 0\nliterature: 0\n$").unwrap(),
    );
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let cards: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap();
    let links: i64 = conn
        .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
        .unwrap();
    let receipt: String = conn
        .query_row("SELECT op_key FROM applied_ops", [], |row| row.get(0))
        .expect("the receipt survives");
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let archive_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'archive_id'",
            [],
            |row| row.get(0),
        )
        .expect("the archive identity survives");
    drop(conn);
    assert_eq!(cards, 0);
    assert_eq!(links, 0, "the derived Link index is emptied too");
    assert_eq!(receipt, "run1:op1");
    assert_eq!(next_topic_id, "0");
    assert_eq!(archive_id.len(), 32);
    assert_eq!(fs::read(&config_path).unwrap(), config_before);
    assert_eq!(fs::read(&dump_marker).unwrap(), b"keep this dump");
    assert_eq!(fs::read(&artifact).unwrap(), b"keep this figure");

    home.cmd_with_editor("# Fresh\n\nfresh\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Fresh"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^0/0\n$").unwrap());
    home.cmd_with_editor("# First\n\nfirst\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^0/1\n$").unwrap());
}

/// S12 (#135) S12-AC07 / R275: the all-archive delete advances the revision once for any real
/// change, including an allocation reset on an archive that holds no Card, and advances nothing
/// when the Card table is already empty and the allocation is already zero.
#[test]
fn delete_all_advances_one_revision_for_a_change_and_none_for_an_empty_zero_archive() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let before = home.revision();
    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success();
    assert_eq!(home.revision(), before + 1);

    // No Card left and the allocation is zero: the confirmed request succeeds and writes nothing.
    let before = home.revision();
    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "deleted: 0 cards; next topic id reset to 0\n",
        ));
    assert_eq!(home.revision(), before);

    // A nonzero allocation alone is still a state change, so it resets and advances once.
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute(
        "UPDATE metadata SET value = '7' WHERE key = 'next_topic_id'",
        [],
    )
    .expect("a nonzero allocation");
    drop(conn);
    let before = home.revision();
    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "deleted: 0 cards; next topic id reset to 0\n",
        ));
    assert_eq!(home.revision(), before + 1);
    assert_eq!(home.next_topic_id(), 0);
    home.cmd_with_editor("# Fresh\n\nfresh\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Fresh"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^0/0\n$").unwrap());
}

/// S12 (#135) S12-AC06 / D11: the all-archive delete keeps today's `clear` handling of saved
/// Spreads, so their metadata remains while tree deletes still follow the deleted positions
/// (R269).
#[test]
fn delete_all_keeps_saved_spread_metadata_while_a_tree_delete_follows_positions() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let spreads = r#"{"current":"s1","spreads":[{"id":"s1","name":"Spread","cards":[{"at":"0/0","x":0,"y":0},{"at":"0/1","x":10,"y":0}]}]}"#;
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute(
        "INSERT INTO metadata(key, value) VALUES('gui_spreads', ?1)",
        [spreads],
    )
    .expect("seed a Spread");
    drop(conn);

    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success();

    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let stored: String = conn
        .query_row("SELECT value FROM metadata WHERE key = 'gui_spreads'", [], |row| {
            row.get(0)
        })
        .expect("spread metadata stays");
    drop(conn);
    assert_eq!(stored, spreads, "the all-archive delete keeps Spread metadata");
}

/// S12 (#135) D09 / S12-AC05: a failure during the all-archive delete rolls the Cards, the derived
/// Links and the allocation reset back together, and releases both guards.
#[test]
fn delete_all_rolls_back_cards_links_and_allocation_when_the_reset_fails() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("# Linked\n\nsee [[0/0]]\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    conn.execute_batch(
        "CREATE TRIGGER fail_topic_reset
         BEFORE UPDATE OF value ON metadata
         WHEN NEW.key = 'next_topic_id'
         BEGIN
             SELECT RAISE(ABORT, 'forced topic reset failure');
         END;",
    )
    .expect("failure trigger");
    let links_before: i64 = conn
        .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
        .expect("links");
    drop(conn);
    assert!(links_before > 0, "the fixture holds authored Links");

    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("forced topic reset failure"));
    for address in ["0/0", "0/1", "0/2"] {
        assert!(home.location_exists(address), "`{address}` stays");
    }
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let links_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM links", [], |row| row.get(0))
        .expect("links");
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);
    assert_eq!(links_after, links_before, "the derived Links come back");
    assert_eq!(next_topic_id, "1", "the allocation comes back");
    assert!(!home.archive_root.join("zt.edit.lock").exists());
    // Both guards are released: a new Session registers at once and quits.
    home.cmd().write_stdin("q\n").assert().success();
}

/// S12 (#135) S12-AC04: the all-archive delete refuses the edit lock before the prompt, so a live
/// write is never interrupted and no confirmation word is asked for nothing.
#[test]
fn delete_all_refuses_the_edit_lock_without_changing_storage() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    fs::write(home.archive_root.join("zt.edit.lock"), "held").expect("edit lock");

    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("type `delete all` to confirm").not())
        .stderr(predicate::str::contains("edit in progress"));
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    assert_eq!(home.next_topic_id(), 1);
}

/// S12 (#135) S12-AC04, S12-AC05 / D06: an open Session refuses the all-archive delete outright,
/// and nothing changes.
#[test]
fn delete_all_refuses_an_open_session_without_changing_storage() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let mut session = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    home.wait_until(|| home.session_count() > 0);

    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("type `delete all` to confirm").not())
        .stderr(predicate::str::contains(
            "cannot delete all while 1 session(s) are open",
        ));
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    assert_eq!(home.next_topic_id(), 1);

    session
        .stdin
        .as_mut()
        .expect("session stdin")
        .write_all(b"q\n")
        .expect("quit session");
    assert!(session.wait().expect("session exit").success());
}

/// S12 (#135) S12-AC05 / D06: a killed all-archive delete leaves no partial change and releases
/// both guards, so a Session registers at once afterwards and the same command can run again.
#[test]
fn a_killed_delete_all_releases_both_guards_without_a_partial_delete() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let revision = home.revision();
    let mut delete = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .args(["del", "--all"])
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn zt del --all");
    let mut delete_stdout = BufReader::new(delete.stdout.take().expect("delete stdout"));
    let mut seen = String::new();
    wait_for_session_output(&mut delete_stdout, &mut seen, "type `delete all` to confirm:");
    delete.kill().expect("kill the waiting delete");
    delete.wait().expect("reap the killed delete");

    for address in ["0/0", "0/1"] {
        assert!(home.location_exists(address), "`{address}` stays");
    }
    assert_eq!(home.revision(), revision, "a killed command changes nothing");

    // Both guards are free: a Session registers and quits at once, then the delete can run.
    home.cmd().write_stdin("q\n").assert().success();
    home.cmd()
        .args(["del", "--all"])
        .write_stdin("delete all\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "deleted: 2 cards; next topic id reset to 0\n",
        ));
}

/// S12 (#135) S12-AC05 / D06: Session registration waits for the whole-archive delete's
/// confirmation and commit, so a starting Session cannot interleave the protected check. Once the
/// delete commits, the waiting Session registers on the emptied archive.
#[test]
fn delete_all_holds_session_registration_through_confirmation_and_commit() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let mut delete = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .args(["del", "--all"])
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn zt del --all");
    let mut delete_stdout = BufReader::new(delete.stdout.take().expect("delete stdout"));
    let mut seen = String::new();
    wait_for_session_output(&mut delete_stdout, &mut seen, "type `delete all` to confirm:");

    // A Session starts while the delete waits for its confirmation. Its registration is blocked:
    // it has not written its pid and has not drawn ROOT.
    let mut session = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    thread::sleep(Duration::from_millis(700));
    assert_eq!(
        home.session_count(),
        0,
        "the starting Session waits to register until the delete ends"
    );

    delete
        .stdin
        .as_mut()
        .expect("delete stdin")
        .write_all(b"delete all\n")
        .expect("confirm");
    wait_for_session_output(
        &mut delete_stdout,
        &mut seen,
        "deleted: 2 cards; next topic id reset to 0",
    );
    assert!(delete.wait().expect("delete exit").success());

    // The waiting Session registers now and finds the emptied archive.
    let mut session_stdout = BufReader::new(session.stdout.take().expect("session stdout"));
    let mut session_seen = String::new();
    wait_for_session_output(&mut session_stdout, &mut session_seen, "ROOT");
    wait_for_session_output(
        &mut session_stdout,
        &mut session_seen,
        "(none yet; create one with t <title>)",
    );
    assert!(
        session_seen.contains("topics:"),
        "the Session registered after the deletion: {session_seen}"
    );
    assert_eq!(home.session_count(), 1, "the Session registered after the delete");
    session
        .stdin
        .as_mut()
        .expect("session stdin")
        .write_all(b"q\n")
        .expect("quit session");
    assert!(session.wait().expect("session exit").success());
}


#[test]
fn session_exits_when_service_disconnects() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");

    home.wait_until(|| {
        fs::read_to_string(home.archive_root.join("zt.sessions"))
            .map(|body| !body.trim().is_empty())
            .unwrap_or(false)
    });
    fs::remove_file(home.archive_root.join("zt.pid")).expect("disconnect service");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(b"status\n")
        .expect("poke session");
    let output = child.wait_with_output().expect("session output");
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("service disconnected"));
}

#[test]
fn killed_session_is_dropped_from_open_session_count() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    home.wait_until(|| {
        fs::read_to_string(home.archive_root.join("zt.sessions"))
            .map(|body| !body.trim().is_empty())
            .unwrap_or(false)
    });
    child.kill().expect("kill session");
    let _ = child.wait();

    home.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("sessions: 0"));
    home.cmd().arg("down").assert().success();
}

#[test]
fn literature_tree_grows_links_compacts_moves_and_renames_inside_its_boundary() {
    let home = TestHome::new();
    home.configure_and_up();
    let bibtex = "@book{TreeLit, title={Tree Work}}";
    for (bib, text) in [
        (bibtex, "# Tree Work\n\nwork\n\n<!-- zt:reverse-links -->\n"),
        (
            "@book{OtherLit, title={Other Work}}",
            "# Other Work\n\nother\n\n<!-- zt:reverse-links -->\n",
        ),
    ] {
        let (mut create, _) = home.cmd_with_editor_sequence(&[bib, text]);
        create.arg("l").assert().success();
    }
    home.cmd_with_editor("# Topic\n\ntopic\n\n<!-- zt:reverse-links -->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("# Idea\n\nidea\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();

    home.cmd_with_editor("# Chapter\n\nchapter notes\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "TreeLit"])
        .assert()
        .success()
        .stdout("TreeLit/1\n");
    home.cmd_with_editor("# Again\n\nagain\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "TreeLit"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "direct successor already exists: TreeLit/1",
        ));
    home.cmd_with_editor("# Next\n\nnext\n\n<!-- zt:reverse-links -->\n")
        .args(["n", "--at", "TreeLit/1"])
        .assert()
        .success()
        .stdout("TreeLit/2\n");
    for expected in ["TreeLit/1|a\n", "TreeLit/1|b\n"] {
        home.cmd_with_editor("# Aside\n\naside\n\n<!-- zt:reverse-links -->\n")
            .args(["b", "--at", "TreeLit/1"])
            .assert()
            .success()
            .stdout(expected);
    }
    assert_eq!(
        home.literature_data("TreeLit").0,
        bibtex,
        "the tree root keeps its exact BibTeX metadata"
    );

    home.cmd_with_editor("# Idea\n\ncites [[TreeLit/1|b]]\n\n<!-- zt:reverse-links -->\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .success();
    assert!(home.card_text("TreeLit/1|b").ends_with("- [[0/1]] Idea\n"));
    home.cmd()
        .write_stdin("go TreeLit\ngo TreeLit/1\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("citation key: TreeLit"))
        .stdout(predicate::str::contains("direct: [[TreeLit/1]]"))
        .stdout(predicate::str::contains("location: TreeLit/1"))
        .stdout(predicate::str::contains("parent: [[TreeLit]]"))
        .stdout(predicate::str::contains("direct: [[TreeLit/2]]"))
        .stdout(predicate::str::contains("side: [[TreeLit/1|a]]"));

    home.cmd()
        .args(["del", "--at", "TreeLit/1|a"])
        .write_stdin("delete\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("TreeLit/1|b -> TreeLit/1|a"));
    assert!(home.card_text("0/1").contains("cites [[TreeLit/1|a]]"));

    for (at, to, error) in [
        (
            "TreeLit/2",
            "0/2",
            "Literature tree Cards cannot leave Literature tree `TreeLit`",
        ),
        (
            "TreeLit/2",
            "OtherLit/1",
            "Literature tree Cards cannot leave Literature tree `TreeLit`",
        ),
        (
            "0/1",
            "TreeLit/1|b",
            "Topic tree Cards cannot move into Literature tree `TreeLit`",
        ),
    ] {
        home.cmd()
            .args(["mv", "--at", at, to])
            .write_stdin("move\n")
            .assert()
            .failure()
            .stdout(predicate::str::contains("move verification").not())
            .stderr(predicate::str::contains(error));
    }
    assert!(home.location_exists("TreeLit/2"));
    assert!(home.location_exists("0/1"));
    assert!(!home.location_exists("OtherLit/1"));

    home.cmd()
        .args(["mv", "--at", "TreeLit/2", "TreeLit/1|b"])
        .write_stdin("move\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("TreeLit/2 -> TreeLit/1|b"));

    home.cmd_with_editor("@book{RenamedLit, title={Tree Work}}")
        .args(["e", "--at", "TreeLit", "--part", "metadata"])
        .write_stdin("move\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("citation key rename:"))
        .stdout(predicate::str::contains("TreeLit -> RenamedLit"))
        .stdout(predicate::str::contains("TreeLit/1 -> RenamedLit/1"))
        .stdout(predicate::str::contains("TreeLit/1|a -> RenamedLit/1|a"))
        .stdout(predicate::str::contains("TreeLit/1|b -> RenamedLit/1|b"))
        .stdout(predicate::str::contains("renamed cards: 4"))
        .stdout(predicate::str::contains("link macros rewritten: 1"));
    for old in ["TreeLit/1", "TreeLit/1|a", "TreeLit/1|b"] {
        assert!(!home.location_exists(old), "{old} was not renamed");
    }
    for new in ["RenamedLit/1", "RenamedLit/1|a", "RenamedLit/1|b"] {
        assert!(home.location_exists(new), "{new} is missing");
    }
    assert!(home.card_text("0/1").contains("cites [[RenamedLit/1|a]]"));
    home.cmd().arg("lsbk").assert().success().stdout("");
    home.cmd()
        .arg("stats")
        .assert()
        .success()
        .stdout("total: 7\ntopics: 1\nregular: 4\nliterature: 2\n");
}

#[test]
fn line_session_grows_a_literature_tree_and_navigates_it_with_up_ls_and_help() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@book{LineTree, title={Line Tree}}",
        "# Line Tree\n\nroot body\n\n<!-- zt:reverse-links -->\n",
    ]);
    create.arg("l").assert().success();

    let output = home
        .cmd()
        .write_stdin(concat!(
            "go LineTree\n",
            "help\n",
            "n\n",
            "# First Note\n\nfirst body\n\n<!-- zt:reverse-links -->\n\x13",
            "help\n",
            "ls\n",
            "up\n",
            "up\n",
            "up\n",
            "q\n",
        ))
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(output).expect("session output is UTF-8");

    assert!(stdout.contains(
        "commands: go [<target>] | root | ws [<workspace>] | up | back | fwd | ls | t <title> | l | n | e | del [keep-root] | stats | status | lsbk | help | q\n"
    ));
    assert!(
        stdout.contains(
            "location: LineTree/1\n# First Note\n\nfirst body\n\n<!-- zt:reverse-links -->\nparent: [[LineTree]]\n"
        )
    );
    assert!(stdout.contains(
        "commands: go [<target>] | root | ws [<workspace>] | up | back | fwd | ls | t <title> | l | n | b | e | del [keep-root] | ins | mv [alone] <new-location> | mv [alone] before <location> | ty <idea|event|finding|model|hub|term|cited> [<facet>] | lv <level|none> | mf <form|none> | stats | status | lsbk | help | q\n"
    ));
    assert!(
        stdout.contains("  mv [alone] <new-location>                               move the current card and its successors (alone: only the card; its chain closes up)")
    );
    assert!(stdout.contains("  ins                                                     insert a new card in the current card's place; it and its successors move down"));
    assert!(
        stdout.contains(
            "  ty <idea|event|finding|model|hub|term|cited> [<facet>]  make the current card an Idea, Event, Finding, Model, Hub, Term, or Cited Card; a facet (any short label) makes it a Model part"
        )
    );
    assert!(stdout.contains(
        "Literature tree LineTree: 2 cards\nLineTree Line Tree\nLineTree/1 First Note\n"
    ));
    let after_ls = &stdout[stdout.find("Literature tree LineTree").unwrap()..];
    let citation_view = after_ls
        .find("citation key: LineTree")
        .expect("up to the tree root");
    let root_view = after_ls.find("ROOT\n").expect("up from the tree root");
    assert!(citation_view < root_view);
    assert_eq!(
        home.card_text("LineTree/1"),
        "# First Note\n\nfirst body\n\n<!-- zt:reverse-links -->\n"
    );
}

#[test]
fn dump_maps_literature_tree_cards_by_location() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    let bibtex = "@book{DumpTree, title={Dump Tree}}";
    let root_text = "# Dump Tree\n\nroot\n\n<!-- zt:reverse-links -->\n";
    let node_text = "# Node\n\nnode\n\n<!-- zt:reverse-links -->\n";
    let (mut create, _) = home.cmd_with_editor_sequence(&[bibtex, root_text]);
    create.arg("l").assert().success();
    home.cmd_with_editor(node_text)
        .args(["n", "--at", "DumpTree"])
        .assert()
        .success();

    home.cmd()
        .arg("dp")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("dumped 2 cards to "));

    let archive_path = single_dump_archive(&home, ".zip");
    let archive_name = archive_path.file_stem().unwrap().to_str().unwrap();
    let mut archive =
        zip::ZipArchive::new(File::open(&archive_path).expect("open ZIP")).expect("read ZIP");
    let mut mapping = String::new();
    std::io::Read::read_to_string(
        &mut archive
            .by_name(&format!("{archive_name}/mapping.json"))
            .expect("mapping entry"),
        &mut mapping,
    )
    .expect("mapping text");
    let mapping = serde_json::from_str::<BTreeMap<String, String>>(&mapping).unwrap();
    assert_eq!(
        mapping.keys().map(String::as_str).collect::<Vec<_>>(),
        ["DumpTree", "DumpTree/1"]
    );
    assert_eq!(
        mapping["DumpTree"],
        expected_literature_dump_filename("DumpTree", bibtex, root_text)
    );
    assert_eq!(
        mapping["DumpTree/1"],
        expected_dump_filename("DumpTree/1", node_text)
    );
}

#[test]
fn up_converts_pre_markdown_cards_after_backing_up_the_archive() {
    let home = TestHome::new();
    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    fs::create_dir_all(&home.archive_root).expect("archive root");
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("old db");
    conn.execute_batch(
        "CREATE TABLE cards (
            row_id INTEGER PRIMARY KEY,
            location TEXT UNIQUE,
            citation_key TEXT UNIQUE,
            is_topic INTEGER NOT NULL CHECK(is_topic IN (0, 1)),
            is_lit INTEGER NOT NULL CHECK(is_lit IN (0, 1)),
            bibtex TEXT,
            text TEXT NOT NULL
         );
         CREATE TABLE metadata(key TEXT PRIMARY KEY, value TEXT NOT NULL);
         INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text) VALUES
            ('0/0', NULL, 1, 0, NULL, 'Topic\n<--->\ndescription\n<--->\nThis note has been referred by note [[0/1]] Idea'),
            ('0/1', NULL, 0, 0, NULL, 'Idea\n<--->\nsee [[0/0]]\nand [[OldLit]]\n<--->\n'),
            (NULL, 'OldLit', 0, 1, '@book{OldLit, title={Old Work}}', 'Old Work\n<--->\nnotes\n<--->\n');
         INSERT INTO metadata VALUES('next_topic_id', '1');",
    )
    .expect("pre-Markdown fixture");
    drop(conn);

    home.cmd().arg("up").assert().success();

    // Conversion is a writer: the derived Link index is built from the
    // converted authored text and matches the full parser at once.
    let checked = home
        .cmd()
        .args(["status", "--check-links"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let report: serde_json::Value = serde_json::from_slice(&checked).expect("Link report");
    assert_eq!(report["ok"], true, "{report}");
    assert_eq!(
        report["links"], 2,
        "one occurrence to the Topic and one to the Literature Card: {report}"
    );

    assert_eq!(
        home.card_text("0/0"),
        "# Topic\n\ndescription\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[0/1]] Idea\n"
    );
    assert_eq!(
        home.card_text("0/1"),
        "# Idea\n\nsee [[0/0]]\nand [[OldLit]]\n\n<!-- zt:reverse-links -->\n"
    );
    assert_eq!(
        home.literature_data("OldLit").1,
        "# Old Work\n\nnotes\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[0/1]] Idea\n"
    );
    let backups = fs::read_dir(&home.archive_root)
        .expect("archive listing")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("zt.sqlite3.pre-markdown-") && name.ends_with(".bak"))
        .collect::<Vec<_>>();
    assert_eq!(backups.len(), 1, "one backup of the pre-Markdown archive");
    let backup = Connection::open(home.archive_root.join(&backups[0])).expect("backup db");
    let old_text: String = backup
        .query_row("SELECT text FROM cards WHERE location = '0/1'", [], |row| {
            row.get(0)
        })
        .expect("backup keeps the original text");
    assert_eq!(
        old_text,
        "Idea\n<--->\nsee [[0/0]]\nand [[OldLit]]\n<--->\n"
    );
    drop(backup);

    let backups_before_restart = fs::read_dir(&home.archive_root)
        .expect("archive listing")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".bak"))
        .count();
    home.cmd().arg("down").assert().success();
    home.wait_until(|| !home.archive_root.join("zt.pid").exists());
    home.cmd().arg("up").assert().success();
    let backups_after_restart = fs::read_dir(&home.archive_root)
        .expect("archive listing")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".bak"))
        .count();
    assert_eq!(
        backups_after_restart, backups_before_restart,
        "completed migrations, including the Link index, do not back up again on restart"
    );
}

fn http_request(
    port: u16,
    method: &str,
    path: &str,
    headers: &[(&str, &str)],
    body: Option<&str>,
) -> (u16, String) {
    use std::io::Read;
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect to gui");
    let mut request = format!("{method} {path} HTTP/1.1\r\nConnection: close\r\n");
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("Host"))
    {
        request.push_str(&format!("Host: 127.0.0.1:{port}\r\n"));
    }
    for (name, value) in headers {
        request.push_str(&format!("{name}: {value}\r\n"));
    }
    let body = body.unwrap_or_default();
    request.push_str(&format!(
        "Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    ));
    stream.write_all(request.as_bytes()).expect("send request");
    let mut response = Vec::new();
    stream.read_to_end(&mut response).expect("read response");
    let response = String::from_utf8(response).expect("UTF-8 response");
    let status = response[9..12].parse().expect("status code");
    let body_start = response.find("\r\n\r\n").expect("header end") + 4;
    (status, response[body_start..].to_string())
}

/// The port of the running service's GUI, as `zt gui` reports it.
fn gui_port(home: &TestHome) -> u16 {
    let output = home
        .cmd()
        .arg("gui")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let output = String::from_utf8(output).expect("UTF-8 output");
    let mut lines = output.lines();
    let port: u16 = lines
        .next()
        .and_then(|line| line.strip_prefix("zt gui: http://127.0.0.1:"))
        .and_then(|rest| rest.strip_suffix('/'))
        .expect("gui prints its URL")
        .parse()
        .expect("port");
    assert_eq!(lines.next(), Some(format!("port: {port}").as_str()));
    port
}

fn gui_token(port: u16) -> String {
    let (status, page) = http_request(port, "GET", "/", &[], None);
    assert_eq!(status, 200);
    page.split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("page embeds the token")
        .to_string()
}

/// End a process at once, as power loss would, with no chance to clean up.
fn kill_process(pid: &str) {
    let status = if cfg!(windows) {
        StdCommand::new("taskkill")
            .args(["/F", "/PID", pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
    } else {
        StdCommand::new("kill").args(["-9", pid]).status()
    };
    assert!(status.expect("kill").success());
}

fn gui_listening(port: u16) -> bool {
    std::net::TcpStream::connect(("127.0.0.1", port)).is_ok()
}

#[test]
fn gui_runs_with_the_service_and_zt_gui_only_reports_where() {
    let home = TestHome::new();
    home.cmd()
        .arg("gui")
        .assert()
        .failure()
        .stderr(predicate::str::contains("archive_root is not configured"));
    home.cmd()
        .args([
            "config",
            "set",
            "archive_root",
            home.archive_root.to_str().unwrap(),
        ])
        .assert()
        .success();
    home.cmd()
        .arg("gui")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));

    let up = home
        .cmd()
        .arg("up")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let up = String::from_utf8(up).expect("UTF-8 output");
    assert!(up.contains("service started pid:"), "{up}");
    let port = gui_port(&home);
    assert!(
        up.contains(&format!("gui: http://127.0.0.1:{port}/")),
        "{up}"
    );
    assert!(gui_listening(port));
    assert_eq!(
        fs::read_to_string(home.archive_root.join("zt.gui")).expect("gui record"),
        format!(
            "http://127.0.0.1:{port}/
"
        )
    );
    home.cmd()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "gui: http://127.0.0.1:{port}/"
        )))
        .stdout(predicate::str::contains("sessions: 0"));

    // `zt gui` starts nothing: asking again names the same server and token.
    let token = gui_token(port);
    assert_eq!(gui_port(&home), port);
    assert_eq!(gui_token(port), token);
    home.cmd()
        .args(["up"])
        .assert()
        .success()
        .stdout(predicate::str::contains("service already running"))
        .stdout(predicate::str::contains(format!(
            "gui: http://127.0.0.1:{port}/"
        )));
    for args in [
        &["gui", "--port", "4717"][..],
        &["gui", "--no-open"],
        &["gui", "x"],
    ] {
        home.cmd()
            .args(args)
            .assert()
            .failure()
            .stderr(predicate::str::contains("usage: zt gui [--open]"));
    }
    let auth = [("X-ZT-Token", token.as_str())];
    assert_eq!(http_request(port, "GET", "/api/ping", &auth, None).0, 200);
    assert_eq!(http_request(port, "GET", "/api/ping", &[], None).0, 403);
    // The window can no longer stop the server.
    assert_eq!(
        http_request(port, "POST", "/api/quit", &auth, Some("{}")).0,
        400
    );
    assert!(gui_listening(port));

    // An open GUI is no Session: `zt down` stops the service and its GUI together.
    home.cmd()
        .arg("down")
        .assert()
        .success()
        .stdout(predicate::str::contains("service stopped"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while gui_listening(port) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!gui_listening(port), "the GUI stops with the service");
    assert!(!home.archive_root.join("zt.gui").exists());
    home.cmd()
        .arg("gui")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));
    let log = fs::read_to_string(home.archive_root.join("zt.log")).expect("log");
    assert!(
        log.contains(&format!("gui started on http://127.0.0.1:{port}/")),
        "{log}"
    );
    assert!(log.contains("gui stopped"), "{log}");

    // A new service brings a new GUI with a new token.
    home.cmd().arg("up").assert().success();
    let port = gui_port(&home);
    assert!(gui_listening(port));
    assert_ne!(gui_token(port), token);
    assert_eq!(http_request(port, "GET", "/api/ping", &auth, None).0, 403);
    home.cmd().arg("down").assert().success();
}

#[test]
fn a_dead_daemon_leaves_no_gui_behind_and_the_restart_brings_a_new_one() {
    let home = TestHome::new();
    home.configure_and_up();
    let port = gui_port(&home);
    let token = gui_token(port);
    let pid = fs::read_to_string(home.archive_root.join("zt.pid")).expect("pid");
    kill_process(pid.trim());
    let deadline = Instant::now() + Duration::from_secs(10);
    while gui_listening(port) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(50));
    }
    assert!(!gui_listening(port));

    // `zt gui` finds the dead daemon and starts the service again, with a new GUI.
    let port = gui_port(&home);
    assert!(gui_listening(port));
    assert_ne!(gui_token(port), token);
    let restarted = fs::read_to_string(home.archive_root.join("zt.pid")).expect("pid");
    assert_ne!(restarted.trim(), pid.trim());
    home.cmd().arg("down").assert().success();
}

#[test]
fn gui_serves_a_token_guarded_api_backed_by_the_same_card_rules() {
    let home = TestHome::new();
    home.configure_and_up();
    let port = gui_port(&home);

    let (status, page) = http_request(port, "GET", "/", &[], None);
    assert_eq!(status, 200);
    let token = page
        .split("name=\"zt-token\" content=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("page embeds the token")
        .to_string();
    assert_eq!(token.len(), 32);
    let auth = [("X-ZT-Token", token.as_str())];
    let api = |method: &str, path: &str, body: Option<&str>| {
        let (status, body) = http_request(port, method, path, &auth, body);
        let value: serde_json::Value = serde_json::from_str(&body).expect("JSON response");
        (status, value)
    };

    assert_eq!(http_request(port, "GET", "/api/overview", &[], None).0, 403);
    assert_eq!(
        http_request(
            port,
            "GET",
            "/api/overview",
            &[("Host", "evil.example:80"), auth[0]],
            None
        )
        .0,
        421
    );
    let (status, created) = api(
        "POST",
        "/api/create",
        Some(r##"{"kind":"topic","text":"# Topic\n\nA **bold** start.\n"}"##),
    );
    assert_eq!((status, created["address"].as_str()), (200, Some("0/0")));
    let (_, created) = api(
        "POST",
        "/api/create",
        Some(
            r##"{"kind":"direct","at":"0/0","text":"# Idea\n\nsee [[0/0]] and `[[not a link]]`\n\n<script>alert(1)</script>\n"}"##,
        ),
    );
    assert_eq!(created["address"], "0/1");
    let (status, rejected) = api(
        "POST",
        "/api/create",
        Some(r##"{"kind":"side","at":"0/1","text":"no heading"}"##),
    );
    assert_eq!(status, 400);
    assert_eq!(
        rejected["error"],
        "card text must start with a `# Title` heading"
    );
    // The GUI shares the storage guard with the ordinary and identified writes:
    // a newly supplied body may not repeat its own Card title, and a refused
    // create leaves no allocated address behind.
    let (status, duplicate) = api(
        "POST",
        "/api/create",
        Some(r##"{"kind":"side","at":"0/1","text":"# Branch\n\n# Branch\n\nsecond branch.\n"}"##),
    );
    assert_eq!(status, 400);
    assert!(
        duplicate["error"]
            .as_str()
            .unwrap()
            .contains("duplicate leading title"),
        "{duplicate}"
    );

    assert_eq!(
        home.card_text("0/0"),
        "# Topic\n\nA **bold** start.\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[0/1]] Idea\n"
    );
    let (_, card) = api("GET", "/api/card?at=0%2F1", None);
    let html = card["html"].as_str().expect("rendered html");
    assert!(html.contains("class=\"zt-link\" href=\"#/card/0/0\" data-target=\"0/0\""));
    assert!(html.contains("<code>[[not a link]]</code>"));
    assert!(!html.contains("<script>"));
    assert_eq!(card["parent"]["address"], "0/0");
    let (_, topic) = api("GET", "/api/card?at=0%2F0", None);
    assert!(
        topic["html"]
            .as_str()
            .unwrap()
            .contains("<strong>bold</strong>")
    );
    assert_eq!(topic["inbound"][0]["address"], "0/1");

    // The GUI is a writer too: its creates, moves and deletes keep the derived
    // Link index in step and its Spreads follow the addresses in the same
    // transaction.
    let check_links = |expected: u64| {
        let out = home
            .cmd()
            .args(["status", "--check-links"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let report: serde_json::Value = serde_json::from_slice(&out).expect("Link report");
        assert_eq!(report["ok"], true, "{report}");
        assert_eq!(report["links"], expected, "{report}");
    };
    check_links(1);
    let (_, branch) = api(
        "POST",
        "/api/create",
        Some(r##"{"kind":"side","at":"0/1","text":"# Branch\n\nsecond branch.\n"}"##),
    );
    assert_eq!(branch["address"], "0/1|a");
    let (_, later) = api(
        "POST",
        "/api/create",
        Some(r##"{"kind":"side","at":"0/1","text":"# Later\n\n依赖 [[0/1]]。\n"}"##),
    );
    assert_eq!(later["address"], "0/1|b");
    check_links(2);
    let (status, spread) = api(
        "POST",
        "/api/spreads",
        Some(
            r##"{"current":"a","spreads":[{"id":"a","name":"Work","cards":[
                {"at":"0/1","x":0,"y":0},{"at":"0/1|a","x":120,"y":0},
                {"at":"0/1|b","x":240,"y":0}]}]}"##,
        ),
    );
    assert_eq!(status, 200);
    assert_eq!(spread["spreads"][0]["cards"].as_array().unwrap().len(), 3);
    // Deleting the unreferenced side successor compacts the later one one label
    // up, and its Spread place follows in the same transaction.
    let (_, deleted_branch) = api(
        "POST",
        "/api/delete",
        Some(r##"{"at":"0/1|a","confirm":"delete"}"##),
    );
    assert_eq!(deleted_branch["deleted"], 1);
    assert_eq!(deleted_branch["moved"]["0/1|b"], "0/1|a");
    check_links(2);
    let followed = api("GET", "/api/spreads", None).1;
    assert_eq!(
        followed["spreads"][0]["cards"]
            .as_array()
            .unwrap()
            .iter()
            .map(|card| card["at"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["0/1", "0/1|a"],
        "the Spread follows the compacted Card and drops the deleted one"
    );
    let (_, deleted_later) = api(
        "POST",
        "/api/delete",
        Some(r##"{"at":"0/1|a","confirm":"delete"}"##),
    );
    assert_eq!(deleted_later["deleted"], 1);
    check_links(1);
    let followed = api("GET", "/api/spreads", None).1;
    assert_eq!(
        followed["spreads"][0]["cards"].as_array().unwrap().len(),
        1,
        "a deleted Card leaves every Spread"
    );

    let (_, stale) = api(
        "POST",
        "/api/edit",
        Some(r##"{"at":"0/1","text":"# Idea\n\nchanged\n","base":"not the stored text"}"##),
    );
    assert!(
        stale["error"]
            .as_str()
            .unwrap()
            .contains("changed since it was opened")
    );
    let before_duplicate_edit = home.card_text("0/1");
    let (status, duplicate) = api(
        "POST",
        "/api/edit",
        Some(r##"{"at":"0/1","text":"# Idea\n\n# Idea\n\nchanged\n"}"##),
    );
    assert_eq!(status, 400);
    assert!(
        duplicate["error"]
            .as_str()
            .unwrap()
            .contains("duplicate leading title"),
        "{duplicate}"
    );
    assert_eq!(
        home.card_text("0/1"),
        before_duplicate_edit,
        "a refused GUI edit changes no Card"
    );

    let (_, plan) = api("POST", "/api/delete", Some(r#"{"at":"0/1"}"#));
    assert_eq!(plan["confirm"], "delete");
    assert!(home.location_exists("0/1"));
    let (_, deleted) = api(
        "POST",
        "/api/delete",
        Some(r#"{"at":"0/1","confirm":"delete"}"#),
    );
    assert_eq!(
        (deleted["deleted"].as_u64(), deleted["address"].as_str()),
        (Some(1), Some("0/0"))
    );
    assert!(!home.location_exists("0/1"));
    check_links(0);
    let followed = api("GET", "/api/spreads", None).1;
    assert_eq!(
        followed["spreads"][0]["cards"].as_array().unwrap().len(),
        0,
        "the Spread keeps its place but no Card"
    );

    let (_, overview) = api("GET", "/api/overview", None);
    assert_eq!(overview["stats"]["total"], 1);
    assert_eq!(overview["trees"][0]["excerpt"], "A bold start.");

    home.cmd().arg("down").assert().success();
}

/// S18 (#99): Reply is the Owner's answer to an agent Card, written as one human Card through the
/// GUI only. Every assertion goes through the real HTTP API on a scratch archive: the captured
/// authored fingerprint, the four relation words, the destination Topic (remembered, default,
/// missing), the placement computed at save time, the generated Reverse link on the agent Card, and
/// the refusals that keep the reader's draft.
#[test]
fn gui_reply_writes_one_human_card_and_refuses_what_is_not_a_reply() {
    let home = TestHome::new();
    home.configure_and_up();
    let port = gui_port(&home);
    let token = gui_token(port);
    let auth = [("X-ZT-Token", token.as_str())];
    let api = |method: &str, path: &str, body: Option<&str>| {
        let (status, text) = http_request(port, method, path, &auth, body);
        let value: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
        (status, value)
    };
    let post = |path: &str, body: serde_json::Value| api("POST", path, Some(&body.to_string()));
    let get = |path: &str| api("GET", path, None);
    let card_of = |at: &str| get(&format!("/api/card?at={}", at.replace('/', "%2F"))).1;

    let (_, topic) = post("/api/create", serde_json::json!({
        "kind": "topic", "text": "# 剩余控制权\n\n代理工作区的一个主题。\n", "workspace": "agent",
    }));
    let agent_topic = topic["address"].as_str().expect("agent topic").to_string();
    let (_, made) = post("/api/create", serde_json::json!({
        "kind": "direct", "at": agent_topic, "text": "# 剩余控制权决定谈判地位\n\n剩余控制权决定谈判地位。\n",
    }));
    let agent = made["address"].as_str().expect("agent card").to_string();
    let card = card_of(&agent);
    let fingerprint = card["authored_fingerprint"].as_str().expect("authored fingerprint").to_string();
    assert_eq!(fingerprint.len(), 64);
    let authored_before = card["text"].as_str().expect("text").to_string();

    // The page's draft rules are served as their own DOM-free module, so the title stays first and
    // the relation selector changes only the supplied prefix (S18, #99; tested under Node).
    let (draft_status, draft_js) = http_request(port, "GET", "/draft.js", &auth, None);
    assert_eq!(draft_status, 200);
    assert!(
        draft_js.contains("replySwapPrefix") && draft_js.contains("replyDestination"),
        "the served draft module carries the Reply rules: {draft_js}"
    );

    // A bare supplied prefix is not a reply, and neither is an unkinded or absent Link.
    let (status, bare) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fingerprint, "text": format!("# 回应剩余控制权\n\n印证 [[{agent}]]：\n"),
    }));
    assert_eq!(status, 200);
    assert_eq!(bare["saved"], false);
    assert_eq!(bare["reason"], "reply_bare");
    let (_, unkinded) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fingerprint, "text": format!("# 关于剩余控制权\n\n参见 [[{agent}]] 与相关讨论。\n"),
    }));
    assert_eq!((unkinded["saved"].clone(), unkinded["reason"].clone()), (serde_json::json!(false), serde_json::json!("link_unkinded")));
    let (_, unlinked) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fingerprint, "text": "# 关于剩余控制权\n\n印证 [[9/9]]：这是一条回应。\n",
    }));
    assert_eq!(unlinked["reason"], "target_link_missing");

    // No refusal and no cancelled desk wrote anything: the human archive still holds no Card.
    let human_cards = || {
        get("/api/graph").1["nodes"].as_array().expect("nodes").iter()
            .filter(|node| node["workspace"] == "human").count()
    };
    assert_eq!(human_cards(), 0, "the refusals created no human Card");

    // A Save that passes the reply checks but fails Card storage writes nothing: the default Topic
    // is created inside the reply's own transaction, so a failed reply leaves no empty Topic (D246).
    let (failed_status, failed) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fingerprint,
        "text": format!("# 融资约束\n\n融资约束\n\n印证 [[{agent}]]：融资约束放大了这一效应。\n"),
    }));
    assert_eq!(failed_status, 400, "{failed}");
    assert!(
        failed["error"].as_str().expect("error").contains("duplicate leading title"),
        "the failed Save answers its own reason: {failed}"
    );
    assert_eq!(human_cards(), 0, "a failed Save left no empty Topic");

    // A Card that changed since the page read it is a renewed-Save refusal, with fresh context.
    let (_, edited) = post("/api/edit", serde_json::json!({
        "at": agent, "text": "# 剩余控制权决定谈判地位\n\n剩余控制权决定谈判地位，并影响融资。\n", "base": authored_before,
    }));
    assert_eq!(edited["address"], agent);
    let (_, stale) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fingerprint, "text": format!("# 融资约束\n\n印证 [[{agent}]]：融资约束放大了这一效应。\n"),
    }));
    assert_eq!(stale["saved"], false);
    assert_eq!(stale["reason"], "target_changed");
    let fresh = stale["target"]["authored_fingerprint"].as_str().expect("fresh fingerprint").to_string();
    assert_ne!(fresh, fingerprint, "the edited Card has a new authored fingerprint");
    assert_eq!(stale["target"]["title"], "剩余控制权决定谈判地位");

    // A valid reply: an empty human archive creates the default Topic and its first Direct successor.
    // The request also carries `workspace: agent`, like a cabinet standing on the agent Workspace:
    // the route writes only the human Workspace whatever the page shows (D240).
    let (_, saved) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fresh, "workspace": "agent",
        "text": format!("# 融资约束放大剩余控制权的效应\n\n印证 [[{agent}]]：外部融资越贵，剩余控制权越值钱。\n"),
    }));
    assert_eq!(saved["saved"], true, "{saved}");
    let reply = saved["address"].as_str().expect("reply address").to_string();
    assert_eq!(saved["kind"], "illustrates");
    assert_eq!(saved["target"]["workspace"], "agent");
    assert_eq!(saved["destination"]["title"], "回应");
    assert_eq!(saved["destination"]["created_topic"], true);
    assert_eq!(saved["destination"]["edge"], "direct");
    let reply_topic = saved["destination"]["topic"].as_str().expect("topic").to_string();
    let human = card_of(&reply);
    assert_eq!(human["workspace"], "human");
    assert_eq!(human["parent"]["address"], reply_topic);

    // The agent Card kept its authored text and gained the generated Reverse link.
    let after = card_of(&agent);
    assert_eq!(after["authored_fingerprint"], fresh);
    assert!(after["inbound"].as_array().expect("inbound").iter().any(|link| link["address"] == reply.as_str()));
    assert_eq!(
        after["body"].as_str().unwrap(),
        "剩余控制权决定谈判地位，并影响融资。",
        "the reply's text never enters the agent Card"
    );

    // The remembered destination is followed, and the next reply is a Side successor of its last Card.
    let (_, again) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fresh, "topic": reply_topic,
        "text": format!("# 契约不完全时结论会变\n\n放宽 [[{agent}]]：完全契约的假设，才能得到这个结论。\n"),
    }));
    assert_eq!(again["saved"], true, "{again}");
    assert_eq!(again["kind"], "relaxes");
    assert_eq!(again["destination"]["created_topic"], false);
    assert_eq!(again["destination"]["topic"], reply_topic);
    assert_eq!(again["destination"]["edge"], "side");
    assert_eq!(again["destination"]["parent"], reply);

    // A destination that no longer exists falls back to the default Topic, never a stale occupant.
    let (_, fallen) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fresh, "topic": "999/0",
        "text": format!("# 这个结论只在一个方向上成立\n\n反驳 [[{agent}]]：反例出现在控制权不可转让时。\n"),
    }));
    assert_eq!(fallen["saved"], true, "{fallen}");
    assert_eq!(fallen["kind"], "objects");
    assert_eq!(fallen["destination"]["topic"], reply_topic);
    assert_eq!(fallen["destination"]["created_topic"], false);

    // The fourth offered kind word: 对应 (corresponds), filed after the last reply as a Side
    // successor, so all four words of the reply row reach their existing Relation kinds.
    let (_, corresponded) = post("/api/reply", serde_json::json!({
        "at": agent, "base": fresh, "topic": reply_topic,
        "text": format!("# 合伙制的控制安排同理\n\n对应 [[{agent}]]：同一机制解释了合伙制里的控制权分配。\n"),
    }));
    assert_eq!(corresponded["saved"], true, "{corresponded}");
    assert_eq!(corresponded["kind"], "corresponds");
    assert_eq!(corresponded["destination"]["topic"], reply_topic);
    assert_eq!(corresponded["destination"]["edge"], "side");
    assert_eq!(corresponded["destination"]["parent"], fallen["address"]);

    // S18-AC01: the desk's usual target is a Side successor, whose address carries a pipe, so the
    // skeleton is `印证 [[0/1|a]]：`. The route must read that whole address as the Link's target;
    // if the Link scanner split it at the pipe, every such reply would answer `target_link_missing`.
    let (_, branchy) = post("/api/create", serde_json::json!({
        "kind": "side", "at": agent, "text": "# 剩余控制权的旁支\n\n剩余控制权的旁支。\n",
    }));
    let piped = branchy["address"].as_str().expect("side successor").to_string();
    assert!(piped.contains('|'), "a Side successor's address carries a pipe: {piped}");
    let piped_card = card_of(&piped);
    let piped_fingerprint = piped_card["authored_fingerprint"]
        .as_str()
        .expect("piped fingerprint")
        .to_string();
    let piped_body = piped_card["body"].as_str().expect("piped body").to_string();
    let (_, piped_saved) = post("/api/reply", serde_json::json!({
        "at": piped, "base": piped_fingerprint, "topic": reply_topic,
        "text": format!("# 旁支也要看融资约束\n\n对应 [[{piped}]]：旁支的结论同样受融资约束影响。\n"),
    }));
    assert_eq!(piped_saved["saved"], true, "{piped_saved}");
    assert_eq!(piped_saved["kind"], "corresponds");
    assert_eq!(piped_saved["target"]["address"], piped.as_str());
    assert_eq!(piped_saved["destination"]["topic"], reply_topic);
    assert_eq!(piped_saved["destination"]["edge"], "side");
    assert_eq!(piped_saved["destination"]["parent"], corresponded["address"]);
    let piped_after = card_of(&piped);
    assert_eq!(piped_after["body"].as_str().unwrap(), piped_body, "the piped agent Card keeps its authored text");
    assert!(
        piped_after["inbound"].as_array().expect("inbound").iter()
            .any(|link| link["address"] == piped_saved["address"]),
        "the generated Reverse link reaches the piped agent Card"
    );

    // A human Card is not a target, and the route is Owner-only.
    let (_, human_topic) = post("/api/create", serde_json::json!({
        "kind": "topic", "text": "# 人类的主题\n\n人类工作区的一个主题。\n", "workspace": "human",
    }));
    let human_at = human_topic["address"].as_str().expect("human topic").to_string();
    let (_, wrong_ws) = post("/api/reply", serde_json::json!({
        "at": human_at, "base": "0".repeat(64), "text": format!("# 回应\n\n印证 [[{human_at}]]：这不成立。\n"),
    }));
    assert_eq!(wrong_ws["saved"], false);
    assert_eq!(wrong_ws["reason"], "target_not_agent");
    assert_eq!(http_request(port, "POST", "/api/reply", &[], Some("{}")).0, 403);
    let (status, _) = post("/api/reply", serde_json::json!({
        "at": "7/7", "base": "0".repeat(64), "text": "# 回应\n\n印证 [[7/7]]：没有这张卡。\n",
    }));
    assert_eq!(status, 400, "a missing Card is an input error, not a saved reply");

    // Every saved reply exists, the link index agrees, and only the owner's writes happened.
    let (_, graph) = get("/api/graph");
    let addresses: Vec<&str> = graph["nodes"].as_array().expect("nodes").iter()
        .filter_map(|node| node["address"].as_str()).collect();
    for address in [&reply, again["address"].as_str().unwrap(), fallen["address"].as_str().unwrap(),
                    corresponded["address"].as_str().unwrap(), piped_saved["address"].as_str().unwrap()] {
        assert!(addresses.contains(&address), "{address} exists in the graph");
    }
    let output = home.cmd().args(["status", "--check-links"]).assert().success()
        .get_output().stdout.clone();
    let report: serde_json::Value = serde_json::from_slice(&output).expect("Link report");
    assert_eq!(report["ok"], true, "{report}");
    home.cmd().arg("down").assert().success();
}

