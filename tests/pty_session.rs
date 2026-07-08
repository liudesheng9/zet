mod support;

use assert_cmd::cargo::cargo_bin;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

use support::pty::{Key, PtySession};

struct TestHome {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
        }
    }

    fn zt_command(&self) -> Command {
        let mut command = Command::new(cargo_bin("zt"));
        command.env("ZT_CONFIG_DIR", &self.config_dir);
        command
    }

    fn zt_command_with_editor(&self, text: &str) -> Command {
        let script = self.config_dir.join("fake-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            concat!(
                "[IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_TEXT)\n",
                "if ($env:ZT_EDITOR_EXIT) { exit ([int]$env:ZT_EDITOR_EXIT) }\n",
            ),
        )
        .expect("editor script");
        let mut command = self.zt_command();
        command.env(
            "EDITOR",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                script.display()
            ),
        );
        command.env("ZT_EDITOR_TEXT", text);
        command
    }

    fn configure_and_up(&self) {
        let status = self
            .zt_command()
            .args([
                "config",
                "set",
                "archive_root",
                self.archive_root.to_str().expect("archive root"),
            ])
            .status()
            .expect("config set");
        assert!(status.success(), "config set failed: {status}");

        let status = self.zt_command().arg("up").status().expect("zt up");
        assert!(status.success(), "zt up failed: {status}");
    }

    fn card_text(&self, location: &str) -> String {
        let conn =
            rusqlite::Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT text FROM cards WHERE location = ?1",
            [location],
            |row| row.get(0),
        )
        .expect("card text")
    }

    fn location_exists(&self, location: &str) -> bool {
        let conn =
            rusqlite::Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE location = ?1)",
            [location],
            |row| row.get::<_, i64>(0),
        )
        .expect("location exists")
            == 1
    }

    fn create_topic_and_base(&self) {
        let status = self
            .zt_command_with_editor("Topic\n<--->\ndescription\n<--->\n")
            .args(["t", "Topic"])
            .status()
            .expect("create topic");
        assert!(status.success(), "create topic failed: {status}");

        let status = self
            .zt_command_with_editor("Base\n<--->\nbody [[0/0]]\n<--->\n")
            .args(["n", "--at", "0/0"])
            .status()
            .expect("create base");
        assert!(status.success(), "create base failed: {status}");
    }

    fn create_move_fixture(&self) {
        self.create_topic_and_base();

        let status = self
            .zt_command_with_editor("Movable\n<--->\nmove\n<--->\n")
            .args(["b", "--at", "0/1"])
            .status()
            .expect("create side");
        assert!(status.success(), "create side failed: {status}");

        let status = self
            .zt_command_with_editor("Source\n<--->\nsource [[0/1|a]]\n<--->\n")
            .args(["n", "--at", "0/1"])
            .status()
            .expect("create source");
        assert!(status.success(), "create source failed: {status}");
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.archive_root.join("zt.pid"));
    }
}

#[test]
fn terminal_session_starts_at_root_and_quits_cleanly() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_creates_chinese_topic_with_tui_edit_caret() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt t 中文主题");
    session.send_enter();
    session.wait_for_text("edit mode");

    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("中文正文");
    session.send_key(Key::CtrlS);

    session.wait_for_text("location: 0/0");
    session.wait_for_text("中文正文");
    session.send_text("zt root");
    session.send_enter();
    session.wait_for_text("ROOT");
    session.send_text("zt go 0/0");
    session.send_enter();
    session.wait_for_text("location: 0/0");
    session.wait_for_text("中文正文");
    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/0"), "中文主题\n<--->\n中文正文\n<--->\n");
}

#[test]
fn terminal_session_navigates_and_activates_rendered_links() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.wait_for_text("0/0 Topic");

    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.wait_for_text("[[0/0]]");

    session.send_text("zt go 9/9");
    session.send_enter();
    session.wait_for_text("location `9/9` does not exist");
    session.wait_for_text("location: 0/1");

    session.send_left_click(2, 5);
    session.wait_for_text("location: 0/0");

    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_runs_read_only_commands() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");

    session.send_text("zt stats");
    session.send_enter();
    session.wait_for_text("total: 2 | topics: 1 | regular: 1");

    session.send_text("zt status");
    session.send_enter();
    session.wait_for_text("state: up | sessions: 1");

    session.send_text("zt ls");
    session.send_enter();
    session.wait_for_text("0/0 Topic | 0/1 Base");

    session.send_text("zt lsbk");
    session.send_enter();
    session.wait_for_text("no broken links");

    session.send_text("zt help");
    session.send_enter();
    session.wait_for_text("zt root | zt go <location>");

    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("zt root");
    session.send_enter();
    session.wait_for_text("ROOT");

    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_creates_direct_and_side_cards() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("zt n");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_text("Diret");
    session.send_key(Key::Left);
    session.send_text("c");
    session.send_key(Key::Right);
    session.send_key(Key::Down);
    session.send_key(Key::Up);
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("direct body");
    session.send_key(Key::CtrlS);
    session.wait_for_text("location: 0/2");

    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("zt b");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_text("Side");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("body");
    session.send_key(Key::Home);
    session.send_text("side ");
    session.send_key(Key::End);
    session.send_key(Key::CtrlS);
    session.wait_for_text("location: 0/1|a");

    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/2"), "Direct\n<--->\ndirect body\n<--->\n");
    assert_eq!(home.card_text("0/1|a"), "Side\n<--->\nside body\n<--->\n");
}

#[test]
fn terminal_session_exits_when_service_disconnects() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    fs::remove_file(home.archive_root.join("zt.pid")).expect("disconnect service");

    session.send_text("zt status");
    session.send_enter();
    session.wait_for_text("service disconnected");
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_moves_and_deletes_cards_with_confirmations() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_move_fixture();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt go 0/1|a");
    session.send_enter();
    session.wait_for_text("location: 0/1|a");

    session.send_text("zt mv 0/2|a");
    session.send_enter();
    session.wait_for_text("move verification:");
    session.send_text("move");
    session.send_enter();
    session.wait_for_text("location: 0/2|a");

    session.send_text("zt del");
    session.send_enter();
    session.wait_for_text("delete verification:");
    session.send_text("delete");
    session.send_enter();
    session.wait_for_text("location: 0/2");

    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert!(!home.location_exists("0/1|a"));
    assert!(!home.location_exists("0/2|a"));
    assert!(home.card_text("0/2").contains("[[0/2|a]]"));
}

#[test]
fn terminal_session_edit_lock_blocks_other_writes() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("zt e");
    session.send_enter();
    session.wait_for_text("edit mode");

    let output = home
        .zt_command_with_editor("Blocked\n<--->\nblocked\n<--->\n")
        .args(["n", "--at", "0/1"])
        .output()
        .expect("blocked write");
    assert!(!output.status.success(), "write unexpectedly succeeded");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("edit in progress"),
        "stderr did not contain edit lock message: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    session.send_key(Key::Esc);
    session.wait_for_text("location: 0/1");
    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_ctrl_c_cancels_edit_mode_without_exiting_session() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let original = home.card_text("0/1");

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("zt e");
    session.send_enter();
    session.wait_for_text("edit mode");

    session.send_key(Key::CtrlC);
    session.wait_for_text("location: 0/1");
    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/1"), original);
}

#[test]
fn terminal_session_retries_invalid_edit_save_and_cancels_without_write() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();
    let original = home.card_text("0/1");

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("zt go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("zt e");
    session.send_enter();
    session.wait_for_text("edit mode");

    session.send_key(Key::Down);
    session.send_key(Key::Delete);
    session.send_key(Key::CtrlS);
    session.wait_for_text("exactly two");

    session.send_key(Key::Esc);
    session.wait_for_text("location: 0/1");
    session.send_text("zt q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/1"), original);
}
