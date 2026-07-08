use assert_cmd::Command;
use predicates::prelude::*;
use rusqlite::Connection;
use std::fs;
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
    assert_eq!(columns, vec!["location", "is_topic", "text"]);
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
        .failure()
        .stderr(predicate::str::contains(
            "topic descriptions cannot contain link macros",
        ));

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
        .write_stdin("zt go 0/0\nzt ls\nzt go 0/1\nzt root\nzt go 0/99\nzt stats\nzt q\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("ROOT"))
        .stdout(predicate::str::contains("0/0 Topic"))
        .stdout(predicate::str::contains("location: 0/0"))
        .stdout(predicate::str::contains("direct: [[0/1]]"))
        .stdout(predicate::str::contains("0/1 Base"))
        .stdout(predicate::str::contains("location: 0/1"))
        .stdout(predicate::str::contains("location `0/99` does not exist"))
        .stdout(predicate::str::contains("total: 3"));
}

#[test]
fn session_writes_use_pointer_tui_save_cancel_and_retry_validation() {
    let home = TestHome::new();
    home.configure_and_up();

    home.cmd()
        .write_stdin(
            concat!(
                "zt t Session Topic\n",
                "Session Topic\n<--->\ndescription\n<--->\n",
                "\x13",
                "zt n\n",
                "First\n<--->\nbody\n<--->\n",
                "\x13",
                "zt b\n",
                "Side\n<--->\nside body\n<--->\n",
                "\x13",
                "zt q\n",
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
                "zt go 0/1\n",
                "zt n\n",
                "Canceled\n<--->\nnot saved\n<--->\n",
                "\x1b",
                "zt q\n",
            )
            .as_bytes(),
        )
        .assert()
        .success();
    assert!(!home.location_exists("0/2"));

    home.cmd()
        .write_stdin(
            concat!(
                "zt go 0/1\n",
                "zt e\n",
                "Broken\n<--->\nmissing delimiter\n",
                "\x13",
                "Fixed\n<--->\nupdated body\n<--->\ndamaged reverse\n",
                "\x13",
                "zt q\n",
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
                "zt go 0/1|a\n",
                "zt mv 0/2|a\n",
                "move\n",
                "zt del\n",
                "delete\n",
                "zt q\n",
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

    let mut input = String::from("zt go 0/1\n");
    for index in 1..=27 {
        input.push_str("zt b\n");
        input.push_str(&format!("Side {index}\n<--->\nbody {index}\n<--->\n"));
        input.push('\x13');
        input.push_str("zt go 0/1\n");
    }
    input.push_str("zt q\n");

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
        .write_all(b"zt go 0/1\nzt e\n")
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
        .write_all(b"\x1bzt q\n")
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
        .write_all(b"zt q\n")
        .expect("quit");
    let output = child.wait_with_output().expect("session output");
    assert!(output.status.success());
    home.cmd().arg("down").assert().success();
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
        .write_all(b"zt status\n")
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
