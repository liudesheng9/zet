use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::process::{Command as StdCommand, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::TempDir;

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
        cmd.env("ZT_CONFIG_DIR", &self.config_dir);
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

    fn create_topic_and_base(&self) {
        self.cmd_with_editor("Topic\n<--->\ndescription\n<--->\n")
            .args(["t", "Topic"])
            .assert()
            .success();
        self.cmd_with_editor("Base\n<--->\nbase\n<--->\n")
            .args(["n", "--at", "0/0"])
            .assert()
            .success();
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

#[cfg(windows)]
fn host_zip_backend_available() -> bool {
    StdCommand::new("tar.exe")
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
    StdCommand::new("tar.exe")
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
    StdCommand::new("tar.exe")
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
    StdCommand::new("tar")
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
    StdCommand::new("tar")
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

#[cfg(windows)]
fn path_with_prepend(directory: &std::path::Path) -> std::ffi::OsString {
    let mut paths = vec![directory.to_path_buf()];
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path));
    }
    std::env::join_paths(paths).expect("test PATH")
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
         VALUES(NULL, 'ExactCase', 0, 1, '@book{ExactCase,title={X}}', 'X\n<--->\n\n<--->\n')",
        [],
    )
    .expect("valid Literature row");
    assert!(
        conn.execute(
            "INSERT INTO cards(location, citation_key, is_topic, is_lit, bibtex, text)
             VALUES(NULL, 'exactcase', 0, 1, '@book{exactcase,title={Y}}', 'Y\n<--->\n\n<--->\n')",
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
         INSERT INTO cards VALUES('0/0', 1, 'Legacy\n<--->\nexact body\n<--->\n');
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
        ("0/0".into(), 1, "Legacy\n<--->\nexact body\n<--->\n".into())
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

    home.cmd_with_editor("Topic One\n<--->\ndescription\n<--->\n")
        .args(["t", "Topic One"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/0"));

    home.cmd_with_editor("First\n<--->\nbody\n<--->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/1"));

    home.cmd_with_editor("No implicit pointer\n<--->\nbody\n<--->\n")
        .arg("n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("expected --at <location>"));

    home.cmd_with_editor("Second\n<--->\nnext\n<--->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/2"));

    home.cmd_with_editor("Branch A\n<--->\nside\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/1|a"));

    home.cmd_with_editor("Branch B\n<--->\nside\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/1|b"));

    home.cmd_with_editor("Invalid\n<--->\nside\n<--->\n")
        .args(["b", "--at", "0/0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not valid on a topic"));

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
        "Client cannot replace this title\n<--->\nmy literature notes\n<--->\nclient reverse",
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
        "A Study\n<--->\n\n<--->\n"
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
        "Repaired Metadata\n<--->\nnotes\n<--->\n",
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
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        exact_bib,
        "changed by client\nsecond generated-title line\n<--->\nnotes\n<--->\n",
    ]);
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

    home.cmd_with_canceling_editor("Canceled Topic\n<--->\nnot saved\n<--->\n")
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
    home.cmd_with_editor("Topic\n<--->\ndescription\n<--->\n")
        .env("ZT_EDITOR_PATH_OUT", &path_out)
        .args(["t", "Topic"])
        .assert()
        .success();
    let temp_path = fs::read_to_string(&path_out).expect("editor path");
    let temp_path = temp_path.trim();
    assert!(temp_path.ends_with(".zt.md"));
    assert!(!std::path::Path::new(&temp_path).exists());

    home.cmd_with_canceling_editor("Canceled\n<--->\nnot saved\n<--->\n")
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

    home.cmd_with_editor("Ignored\n<--->\nignored\n<--->\n")
        .args(["t", ""])
        .assert()
        .failure()
        .stderr(predicate::str::contains("topic title"));

    home.cmd_with_editor("Malformed\n<--->\nmissing reverse delimiter\n")
        .args(["t", "Malformed"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exactly two"));

    home.cmd_with_editor("Topic\n<--->\nvalid\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success()
        .stdout(predicate::str::contains("0/0"));

    home.cmd_with_editor("Topic\n<--->\ninvalid [[0/0]]\n<--->\n")
        .args(["e", "--at", "0/0"])
        .assert()
        .success();
    assert!(
        home.card_text("0/0")
            .contains("This note has been referred by note [[0/0]] Topic")
    );

    home.cmd_with_editor("First\n<--->\nbody\n<--->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();

    home.cmd_with_editor("First\n<--->\ninvalid [[0/01]]\n<--->\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid link macro target"));
}

#[test]
fn reverse_links_and_broken_links_are_generated_from_card_text() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd_with_editor("Topic\n<--->\ndescription\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("Base\n<--->\nbase\n<--->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor("Target\n<--->\ntarget\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("Source\n<--->\nsee [[0/1|a]] and [[0/1|a]] plus [[0/0]]\n<--->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();

    let target_text = home.card_text("0/1|a");
    assert!(target_text.contains("This note has been referred by note [[0/2]] Source"));
    assert_eq!(
        target_text
            .matches("This note has been referred by note [[0/2]] Source")
            .count(),
        1
    );
    assert!(
        home.card_text("0/0")
            .contains("This note has been referred by note [[0/2]] Source")
    );

    home.cmd_with_editor("Source\n<--->\nnew broken [[0/99]]\n<--->\n")
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

    home.cmd_with_editor("Source\n<--->\nsee [[0/1|a]] but keep editing\n<--->\ndamaged reverse\n")
        .args(["e", "--at", "0/2"])
        .assert()
        .success();
}

#[test]
fn every_card_kind_can_link_to_literature_and_literature_can_link_to_locations() {
    let home = TestHome::new();
    home.configure_and_up();

    let (mut create_lit_a, _) = home.cmd_with_editor_sequence(&[
        "@article{LitA, title={Literature A}}",
        "Literature A\n<--->\nfirst work\n<--->\n",
    ]);
    create_lit_a.arg("l").assert().success();

    home.cmd_with_editor("Topic\n<--->\nplain topic\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("Regular\n<--->\nplain regular\n<--->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor("Topic\n<--->\ntopic cites [[LitA]] and [[0/1]]\n<--->\n")
        .args(["e", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor("Regular\n<--->\nregular cites [[LitA]]\n<--->\n")
        .args(["e", "--at", "0/1"])
        .assert()
        .success();

    let (mut create_lit_b, _) = home.cmd_with_editor_sequence(&[
        "@book{LitB, title={Literature B}}",
        "Literature B\n<--->\nliterature cites [[LitA]], [[0/0]], and [[0/1]]\n<--->\n",
    ]);
    create_lit_b.arg("l").assert().success();

    home.cmd()
        .write_stdin("go LitA\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "This note has been referred by note [[0/0]] Topic",
        ))
        .stdout(predicate::str::contains(
            "This note has been referred by note [[0/1]] Regular",
        ))
        .stdout(predicate::str::contains(
            "This note has been referred by note [[LitB]] Literature B",
        ));
    home.cmd()
        .write_stdin("go 0/0\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "This note has been referred by note [[LitB]] Literature B",
        ));
}

#[test]
fn session_root_topic_and_literature_lists_keep_their_locked_membership_and_order() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    for (key, title) in [("ZebraKey", "Zebra Work"), ("AlphaKey", "Alpha Work")] {
        let bib = format!("@book{{{key}, title={{{title}}}}}");
        let text = format!("{title}\n<--->\nnotes\n<--->\n");
        let (mut create, _) = home.cmd_with_editor_sequence(&[&bib, &text]);
        create.arg("l").assert().success();
    }

    let automatic_root = home
        .cmd()
        .write_stdin("q\n")
        .output()
        .expect("automatic ROOT output");
    let automatic_root = String::from_utf8(automatic_root.stdout).unwrap();
    assert!(automatic_root.starts_with("ROOT\n0/0 Topic\n"));
    assert!(!automatic_root.contains("0/1 Base"));
    assert!(!automatic_root.contains("AlphaKey"));
    assert!(!automatic_root.contains("ZebraKey"));

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
    assert!(topic_ls.contains("0/0 Topic\n0/1 Base\n"));
    assert!(!topic_ls.contains("AlphaKey Alpha Work"));
    assert!(!topic_ls.contains("ZebraKey Zebra Work"));

    let literature_ls = home
        .cmd()
        .write_stdin("go ZebraKey\nls\nq\n")
        .output()
        .expect("Literature ls output");
    let literature_ls = String::from_utf8(literature_ls.stdout).unwrap();
    assert!(literature_ls.contains("AlphaKey Alpha Work\nZebraKey Zebra Work\n"));
    assert_eq!(literature_ls.matches("0/0 Topic").count(), 1);
    assert!(!literature_ls.contains("0/1 Base"));
}

#[test]
fn shell_metadata_part_updates_raw_bibtex_and_generated_title_without_editing_body() {
    let home = TestHome::new();
    home.configure_and_up();
    home.cmd_with_editor("Topic\n<--->\ntarget\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@article{EditKey, title={Original Title}}",
        "Original Title\n<--->\nbody stays here [[0/0]]\n<--->\n",
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
        .stdout(predicate::str::contains("title: Updated Title"))
        .stdout(predicate::str::contains(
            "@article{EditKey, title={Updated {Title}}}",
        ))
        .stdout(predicate::str::contains("body stays here"));
    home.cmd()
        .write_stdin("go 0/0\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "This note has been referred by note [[EditKey]] Updated Title",
        ));
}

#[test]
fn shell_literature_edit_selector_errors_fail_before_editor_launch() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@book{SelectorKey, title={Selector Work}}",
        "Selector Work\n<--->\nbody\n<--->\n",
    ]);
    create.arg("l").assert().success();
    home.cmd_with_editor("Topic\n<--->\nbody\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("  zt l\n"))
        .stdout(predicate::str::contains("  zt del --at <target>\n"))
        .stdout(predicate::str::contains("  zt n --at <location>\n"));

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
        let text = format!("{title}\n<--->\nbody {key}\n<--->\n");
        let (mut create, _) = home.cmd_with_editor_sequence(&[&bib, &text]);
        create.arg("l").assert().success();
    }
    home.cmd_with_editor("Topic\n<--->\nsource [[KeepKey]]\n<--->\n")
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
        "Generated Title\n<--->\nold body\n<--->\n",
    ]);
    create.arg("l").assert().success();
    home.cmd_with_editor("Topic\n<--->\nsource [[TextEdit]]\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();

    home.cmd_with_editor(
        "\nClient added another title line\n<--->\nnew body\n<--->\nclient reverse must disappear",
    )
    .args(["e", "--at", "TextEdit", "--part", "text"])
    .assert()
    .success();

    home.cmd()
        .write_stdin("go TextEdit\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("title: Generated Title"))
        .stdout(predicate::str::contains("new body"))
        .stdout(predicate::str::contains(
            "This note has been referred by note [[0/0]] Topic",
        ))
        .stdout(predicate::str::contains("client reverse must disappear").not());
}

#[test]
fn citation_key_rename_requires_move_and_rewrites_links_from_every_card_kind() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create_old, _) = home.cmd_with_editor_sequence(&[
        "@book{OldKey, title={Old Work}}",
        "Old Work\n<--->\nwork\n<--->\n",
    ]);
    create_old.arg("l").assert().success();
    home.cmd_with_editor("Topic [[OldKey]]\n<--->\ntopic [[OldKey]]\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("Regular\n<--->\nregular [[OldKey]]\n<--->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success();
    let (mut create_source, _) = home.cmd_with_editor_sequence(&[
        "@misc{LitSource, title={Literature Source}, note={metadata [[OldKey]] stays literal}}",
        "Literature Source\n<--->\nliterature [[OldKey]]\n<--->\n",
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
        .stdout(predicate::str::contains("title: Renamed Work"));
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
            .starts_with("Topic [[OldKey]]\n<--->\ntopic [[NewKey]]\n"),
        "Citation-key rename must rewrite only actual body Links"
    );
}

#[test]
fn literature_delete_leaves_broken_inbound_links_and_refuses_location_topology() {
    let home = TestHome::new();
    home.configure_and_up();
    let (mut create, _) = home.cmd_with_editor_sequence(&[
        "@article{DeleteLit, title={Delete Literature}}",
        "Delete Literature\n<--->\nwork\n<--->\n",
    ]);
    create.arg("l").assert().success();
    home.cmd_with_editor("Topic\n<--->\nsource [[DeleteLit]]\n<--->\n")
        .args(["t", "Topic"])
        .assert()
        .success();
    home.cmd_with_editor("Topic\n<--->\nwrong case [[deletelit]]\n<--->\n")
        .args(["e", "--at", "0/0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "link target `deletelit` does not exist",
        ));

    for command in ["n", "b"] {
        home.cmd_with_editor("Never\n<--->\ncreated\n<--->\n")
            .args([command, "--at", "DeleteLit"])
            .assert()
            .failure()
            .stderr(predicate::str::contains("Literature Card"));
    }
    home.cmd()
        .args(["mv", "--at", "DeleteLit", "0/1"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Literature Card"));

    home.cmd()
        .args(["del", "--at", "DeleteLit"])
        .write_stdin("delete\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("delete DeleteLit"))
        .stdout(predicate::str::contains("deleted: 1"))
        .stdout(predicate::str::contains("compacted:").not());

    home.cmd()
        .arg("lsbk")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "0/0 Topic -> DeleteLit: source [[DeleteLit]]",
        ));
    home.cmd_with_editor("Topic\n<--->\nsource [[DeleteLit]] after deletion\n<--->\nstale reverse")
        .args(["e", "--at", "0/0"])
        .assert()
        .success();
    home.cmd_with_editor("New Source\n<--->\nnew [[DeleteLit]]\n<--->\n")
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

    home.cmd_with_editor("Side A\n<--->\na\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("Side B\n<--->\nb\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("Side C\n<--->\nc\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("Side C Child\n<--->\nchild\n<--->\n")
        .args(["n", "--at", "0/1|c"])
        .assert()
        .success();
    home.cmd_with_editor("Source\n<--->\nlinks [[0/1|c]] and [[0/1|c|1]]\n<--->\n")
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
fn move_subtree_validates_target_and_rewrites_links() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    home.cmd_with_editor("Movable\n<--->\nmove me\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("Movable Child\n<--->\nchild\n<--->\n")
        .args(["n", "--at", "0/1|a"])
        .assert()
        .success();
    home.cmd_with_editor("Source\n<--->\npoints [[0/1|a]] and [[0/1|a|1]]\n<--->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .success();

    home.cmd()
        .args(["mv", "--at", "0/0", "0/2|a"])
        .write_stdin("move\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("topic cards cannot be moved"));

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
    assert!(
        home.card_text("0/2|a")
            .contains("This note has been referred by note [[0/2]] Source")
    );
}

#[test]
fn session_navigation_and_read_only_commands_use_the_session_pointer() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("Second\n<--->\nsecond\n<--->\n")
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
            "edit mode\nBase\n<--->\nbase\n<--->",
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
                "Session Topic\n<--->\ndescription\n<--->\n",
                "\x13",
                "n\n",
                "First\n<--->\nbody\n<--->\n",
                "\x13",
                "b\n",
                "Side\n<--->\nside body\n<--->\n",
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
                "Canceled\n<--->\nnot saved\n<--->\n",
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
                "Broken\n<--->\nmissing delimiter\n",
                "\x13",
                "Fixed\n<--->\nupdated body\n<--->\ndamaged reverse\n",
                "\x13",
                "q\n",
            )
            .as_bytes(),
        )
        .assert()
        .success()
        .stdout(predicate::str::contains("exactly two"));
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
    input.push_str("client title\nextra title line\n<--->\nline body\n<--->\nclient reverse");
    input.push('\x13');
    input.push_str("e\n9\n1\n");
    input.push_str("@book{LineLit, title={Updated Line Literature}}");
    input.push('\x13');
    input.push_str("e\n2\n");
    input.push_str("\nreplacement title line\n<--->\nupdated line body\n<--->\nstale reverse");
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
        .stdout(predicate::str::contains("title: Updated Line Literature"))
        .stdout(predicate::str::contains("updated line body"))
        .stdout(predicate::str::contains("stale reverse").not());

    assert_eq!(
        home.literature_data("LineLit"),
        (
            "@book{LineLit, title={Updated Line Literature}}".into(),
            "Updated Line Literature\n<--->\nupdated line body\n<--->\n".into(),
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
    home.cmd_with_editor("Movable\n<--->\nmove\n<--->\n")
        .args(["b", "--at", "0/1"])
        .assert()
        .success();
    home.cmd_with_editor("Source\n<--->\nsource [[0/1|a]]\n<--->\n")
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

    home.cmd_with_editor("Next Topic\n<--->\nnext\n<--->\n")
        .args(["t", "Next Topic"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1/0"));
    assert!(home.location_exists("1/0"));
}

#[test]
fn side_successor_labels_roll_over_from_z_to_aa() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut input = String::from("go 0/1\n");
    for index in 1..=27 {
        input.push_str("b\n");
        input.push_str(&format!("Side {index}\n<--->\nbody {index}\n<--->\n"));
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
        .stdout(predicate::str::contains("zt n --at <location>").not());
    home.cmd()
        .arg("stats")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));
    home.cmd_with_editor("Nope\n<--->\nnope\n<--->\n")
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
        .stdout(predicate::str::contains("zt n --at <location>"))
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
    home.cmd_with_editor("Blocked\n<--->\nblocked\n<--->\n")
        .args(["n", "--at", "0/1"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("edit in progress"));
    fs::remove_file(home.archive_root.join("zt.edit.lock")).expect("unlock");

    let mut editing = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
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
    home.cmd_with_editor("Blocked\n<--->\nblocked\n<--->\n")
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
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
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
        .stdout(predicate::str::contains("  zt dp\n"));
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
    assert_eq!(archive.len(), 1);
    let mut mapping = archive
        .by_name(&format!("{archive_name}/mapping.json"))
        .expect("mapping entry");
    let mut body = String::new();
    std::io::Read::read_to_string(&mut mapping, &mut body).expect("mapping text");
    assert_eq!(body, "{}");
}

#[test]
fn dump_preserves_card_text_hash_mapping_and_location_order() {
    if !host_zip_backend_available() {
        return;
    }

    let home = TestHome::new();
    home.configure_and_up();
    let topic_text = "Topic\n<--->\nfirst line\n\u{7b2c}\u{4e8c}\u{884c}\n<--->\n";
    let identical_text = "Same\n<--->\n\u{76f8}\u{540c}\nline two\n<--->\n";
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
    let expected_entries: Vec<String> = std::iter::once(format!("{archive_name}/mapping.json"))
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
    let card_text = "Dump Literature\n<--->\nnotes\n<--->\n";
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
    let topic_text = "Topic\n<--->\nregular payload stays exact\n<--->\n";
    home.cmd_with_editor(topic_text)
        .args(["t", "Topic"])
        .assert()
        .success();
    let literature = [
        (
            "ZebraDump",
            "@misc{ZebraDump, title={Zebra Dump}}\n",
            "Zebra Dump\n<--->\nzebra\n<--->\n",
        ),
        (
            "AlphaDump",
            "@book{AlphaDump, title={Alpha Dump}}",
            "Alpha Dump\n<--->\nalpha\n<--->\n",
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
            (entries, mapping)
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
    let listing = StdCommand::new("tar")
        .arg("-tf")
        .arg(&archive_path)
        .output()
        .expect("list tar.gz");
    assert!(listing.status.success());
    assert_eq!(String::from_utf8(listing.stdout).unwrap().trim(), entry);
    let mapping = StdCommand::new("tar")
        .args(["-xOf"])
        .arg(&archive_path)
        .arg(&entry)
        .output()
        .expect("read tar.gz mapping");
    assert!(mapping.status.success());
    assert_eq!(mapping.stdout, b"{}");
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
    let listing = StdCommand::new("tar")
        .arg("-tf")
        .arg(&archive_path)
        .output()
        .expect("list tar.zst");
    assert!(listing.status.success());
    assert_eq!(String::from_utf8(listing.stdout).unwrap().trim(), entry);
    let mapping = StdCommand::new("tar")
        .args(["-xOf"])
        .arg(&archive_path)
        .arg(&entry)
        .output()
        .expect("read tar.zst mapping");
    assert!(mapping.status.success());
    assert_eq!(mapping.stdout, b"{}");
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
        .env("PATH", path_with_prepend(compressors.path()))
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
        .env("PATH", path_with_prepend(compressors.path()))
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
        .env("PATH", path_with_prepend(compressors.path()))
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
    let system_tar = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap())
        .join("System32")
        .join("tar.exe");
    let listing = StdCommand::new(&system_tar)
        .arg("-tf")
        .arg(&archive_path)
        .output()
        .expect("list 7z");
    assert!(listing.status.success());
    assert_eq!(String::from_utf8(listing.stdout).unwrap().trim(), entry);
    let mapping = StdCommand::new(system_tar)
        .args(["-xOf"])
        .arg(&archive_path)
        .arg(&entry)
        .output()
        .expect("read 7z mapping");
    assert!(mapping.status.success());
    assert_eq!(mapping.stdout, b"{}");
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
        .env("PATH", path_with_prepend(fake_tar.path()))
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
        .env("PATH", path_with_prepend(fake_tar.path()))
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

#[test]
fn clear_is_shell_only_service_gated_and_exact_confirmation_cancels_safely() {
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
        .stdout(predicate::str::contains("zt clear").not());
    home.cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));

    home.cmd().arg("up").assert().success();
    home.create_topic_and_base();
    home.cmd()
        .arg("help")
        .assert()
        .success()
        .stdout(predicate::str::contains("  zt clear\n"))
        .stdout(predicate::str::contains("zt clean").not());
    home.cmd()
        .arg("clean")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown command"));
    for confirmation in ["CLEAR\n", " clear\n", "clear \n"] {
        home.cmd()
            .arg("clear")
            .write_stdin(confirmation)
            .assert()
            .success()
            .stdout(predicate::str::contains("clear canceled\n"))
            .stdout(predicate::str::contains("compression options").not());
    }
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    home.cmd()
        .write_stdin("help\nclear\nq\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("unknown session command: clear"))
        .stdout(predicate::str::contains("zt clear").not());
}

#[test]
fn confirmed_clear_resets_storage_and_topic_allocation_without_touching_archives() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    home.cmd_with_editor("Other\n<--->\nother\n<--->\n")
        .args(["t", "Other"])
        .assert()
        .success()
        .stdout(predicate::str::contains("1/0"));
    let (mut create_literature, _) = home.cmd_with_editor_sequence(&[
        "@book{ClearLit, title={Clear Literature}}",
        "Clear Literature\n<--->\nclear me too\n<--->\n",
    ]);
    create_literature.arg("l").assert().success();
    let config_path = home.config_dir.join("config.toml");
    let config_before = fs::read(&config_path).expect("config bytes");
    let dump_marker = home.archive_root.join("dump").join("existing.dump");
    fs::create_dir_all(dump_marker.parent().unwrap()).expect("dump root");
    fs::write(&dump_marker, b"keep this dump").expect("dump marker");

    home.cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "cleared 4 cards; next topic id reset to 0\n",
        ));
    home.cmd().arg("stats").assert().success().stdout(
        predicate::str::is_match(r"^total: 0\ntopics: 0\nregular: 0\nliterature: 0\n$").unwrap(),
    );
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let cards: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap();
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    drop(conn);
    assert_eq!(cards, 0);
    assert_eq!(next_topic_id, "0");
    assert_eq!(fs::read(&config_path).unwrap(), config_before);
    assert_eq!(fs::read(&dump_marker).unwrap(), b"keep this dump");

    home.cmd_with_editor("Fresh\n<--->\nfresh\n<--->\n")
        .args(["t", "Fresh"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^0/0\n$").unwrap());
    home.cmd_with_editor("First\n<--->\nfirst\n<--->\n")
        .args(["n", "--at", "0/0"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^0/1\n$").unwrap());
}

#[test]
fn clear_refuses_edit_lock_without_changing_storage() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    fs::write(home.archive_root.join("zt.edit.lock"), "held").expect("edit lock");

    home.cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("type clear to confirm").not())
        .stderr(predicate::str::contains("edit in progress"));
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(next_topic_id, "1");
}

#[test]
fn clear_refuses_open_sessions_without_changing_storage() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let mut session = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
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
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .failure()
        .stdout(predicate::str::contains("type clear to confirm").not())
        .stderr(predicate::str::contains(
            "cannot clear while 1 session(s) are open",
        ));
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(next_topic_id, "1");
    drop(conn);

    session
        .stdin
        .as_mut()
        .expect("session stdin")
        .write_all(b"q\n")
        .expect("quit session");
    assert!(session.wait().expect("session exit").success());
}

#[test]
fn clear_rolls_back_card_deletion_when_topic_reset_fails() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
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
    drop(conn);

    home.cmd()
        .arg("clear")
        .write_stdin("clear\n")
        .assert()
        .failure()
        .stderr(predicate::str::contains("forced topic reset failure"));
    assert!(home.location_exists("0/0"));
    assert!(home.location_exists("0/1"));
    let conn = Connection::open(home.archive_root.join("zt.sqlite3")).expect("open db");
    let card_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards", [], |row| row.get(0))
        .unwrap();
    let next_topic_id: String = conn
        .query_row(
            "SELECT value FROM metadata WHERE key = 'next_topic_id'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(card_count, 2);
    assert_eq!(next_topic_id, "1");
    assert!(!home.archive_root.join("zt.edit.lock").exists());
}

#[test]
fn session_exits_when_service_disconnects() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &home.config_dir)
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
