mod support;

use assert_cmd::cargo::cargo_bin;
use std::fs;
use std::path::Path;
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

    fn citation_exists(&self, citation_key: &str) -> bool {
        let conn =
            rusqlite::Connection::open(self.archive_root.join("zt.sqlite3")).expect("open db");
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM cards WHERE citation_key = ?1)",
            [citation_key],
            |row| row.get::<_, i64>(0),
        )
        .expect("citation exists")
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

    fn create_literature(&self, citation_key: &str, title: &str, body: &str) {
        let script = self.config_dir.join("fake-literature-editor.ps1");
        fs::create_dir_all(&self.config_dir).expect("config dir");
        fs::write(
            &script,
            concat!(
                "if ($args[0].EndsWith('.bib')) {\n",
                "  [IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_BIB)\n",
                "} else {\n",
                "  [IO.File]::WriteAllText($args[0], $env:ZT_EDITOR_CARD)\n",
                "}\n",
            ),
        )
        .expect("literature editor script");
        let status = self
            .zt_command()
            .env(
                "EDITOR",
                format!(
                    "powershell -NoProfile -ExecutionPolicy Bypass -File {}",
                    script.display()
                ),
            )
            .env(
                "ZT_EDITOR_BIB",
                format!("@book{{{citation_key}, title={{{title}}}}}"),
            )
            .env("ZT_EDITOR_CARD", format!("{title}\n<--->\n{body}\n<--->\n"))
            .arg("l")
            .status()
            .expect("create Literature Card");
        assert!(status.success(), "create Literature Card failed: {status}");
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

#[cfg(windows)]
struct ClipboardTextRestore(String);

#[cfg(windows)]
impl Drop for ClipboardTextRestore {
    fn drop(&mut self) {
        if let Ok(mut clipboard) = arboard::Clipboard::new() {
            let _ = clipboard.set_text(&self.0);
        }
    }
}

#[cfg(windows)]
#[test]
fn terminal_clear_accepts_confirmation_after_session_returns_to_shell() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let zt = cargo_bin("zt");
    let zt_command = format!("& '{}'", zt.display().to_string().replace('\'', "''"));
    let mut shell = PtySession::spawn_with_args(
        Path::new("powershell.exe"),
        &home.config_dir,
        &["-NoLogo", "-NoProfile", "-NoExit"],
    );
    shell.wait_for_text("PS ");

    shell.send_text(&zt_command);
    shell.send_enter();
    shell.wait_for_text("ROOT");
    shell.send_text("q");
    shell.send_enter();
    shell.wait_for_text_count("PS ", 2);

    shell.send_text(&format!("{zt_command} clear"));
    shell.send_enter();
    shell.wait_for_text("type clear to confirm: ");
    shell.send_text("clear");
    shell.send_enter();
    shell.wait_for_text("cleared 2 cards; next topic id reset to 0");

    let output = shell.plain_output();
    assert!(
        output.contains("type clear to confirm: clear"),
        "confirmation input was not visible:\n{}",
        output.escape_debug()
    );

    shell.send_text("exit");
    shell.send_enter();
    let status = shell.wait_for_exit();
    assert!(status.success(), "PowerShell exited with {status}");
}

#[test]
fn terminal_session_starts_at_root_and_quits_cleanly() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.wait_for_text(">");
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_drag_highlights_visible_root_text() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");

    session.send_left_drag(0, 0, 0, 3);
    session.wait_for_raw_text("\x1b[7m");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_copies_rendered_selection_through_fake_clipboard() {
    let home = TestHome::new();
    home.configure_and_up();
    let clipboard_output = home.config_dir.join("clipboard.txt");

    let mut session = PtySession::spawn_with_test_clipboard(
        &cargo_bin("zt"),
        &home.config_dir,
        "seed",
        Some(&clipboard_output),
    );
    session.wait_for_text("ROOT");
    session.send_left_drag(0, 0, 0, 3);
    session.wait_for_raw_text("\x1b[7m");
    session.send_key(Key::CtrlShiftC);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !clipboard_output.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(
        clipboard_output.exists(),
        "fake clipboard output was not written:\n{}",
        session.output().escape_debug(),
    );
    assert_eq!(fs::read_to_string(&clipboard_output).unwrap(), "ROOT");

    fs::remove_file(&clipboard_output).unwrap();
    let root_count = session.plain_output().matches("ROOT").count();
    session.resize(25, 81);
    session.wait_for_text_count("ROOT", root_count + 1);
    session.send_key(Key::CtrlShiftC);
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert!(
        !clipboard_output.exists(),
        "resize must clear screen selection"
    );

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_pastes_fake_clipboard_into_command_bar_without_executing() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn_with_test_clipboard(
        &cargo_bin("zt"),
        &home.config_dir,
        "st\tat\r\ns",
        None,
    );
    session.wait_for_text("ROOT");
    session.send_key(Key::CtrlShiftV);
    session.wait_for_text("> st at s");
    assert!(!session.plain_output().contains("total:"));

    for _ in 0..7 {
        session.send_key(Key::Backspace);
    }
    session.send_text("stats");
    session.send_enter();
    session.wait_for_text("total: 0 | topics: 0 | regular: 0 | literature: 0");
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[cfg(windows)]
#[test]
#[ignore = "manual acceptance: temporarily uses and restores the real Windows text clipboard"]
fn manual_real_windows_clipboard_copy_and_paste_smoke() {
    let mut os_clipboard = arboard::Clipboard::new().expect("open Windows clipboard");
    let original = os_clipboard
        .get_text()
        .expect("manual smoke requires restorable text clipboard contents");
    let _restore = ClipboardTextRestore(original);
    os_clipboard.set_text("stats").expect("seed clipboard");
    drop(os_clipboard);

    let home = TestHome::new();
    home.configure_and_up();
    let mut session =
        PtySession::spawn_with_real_clipboard_shortcuts(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_key(Key::CtrlShiftV);
    session.wait_for_text("> stats");
    assert!(!session.plain_output().contains("total:"));
    session.send_enter();
    session.wait_for_text("total: 0 | topics: 0 | regular: 0 | literature: 0");

    session.send_left_drag(0, 0, 0, 3);
    session.wait_for_raw_text("\x1b[7m");
    session.send_key(Key::CtrlShiftC);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let copied = loop {
        let copied = arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.get_text())
            .expect("read copied Windows clipboard text");
        if copied == "ROOT" || std::time::Instant::now() >= deadline {
            break copied;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    assert_eq!(copied, "ROOT");
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text("中文\r\n\tline  \r\n\r\nend "))
        .expect("seed multiline editor clipboard");

    session.send_text("t Paste");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_key(Key::CtrlShiftV);
    session.wait_for_text("中文");
    session.send_key(Key::CtrlS);
    session.wait_for_text("title: Paste");
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
    assert_eq!(
        home.card_text("0/0"),
        "Paste\n<--->\n中文\n\tline  \n\nend \n<--->\n"
    );
}

#[test]
fn terminal_editor_copies_logical_selection_and_plain_ctrl_c_still_cancels() {
    let home = TestHome::new();
    home.configure_and_up();
    let clipboard_output = home.config_dir.join("editor-clipboard.txt");

    let mut session = PtySession::spawn_with_test_clipboard(
        &cargo_bin("zt"),
        &home.config_dir,
        "seed",
        Some(&clipboard_output),
    );
    session.wait_for_text("ROOT");
    session.send_text("t Copy");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_left_drag(2, 0, 2, 3);
    session.wait_for_raw_text("\x1b[7m");
    session.send_key(Key::CtrlShiftC);

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !clipboard_output.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(fs::read_to_string(&clipboard_output).unwrap(), "Copy");

    session.send_key(Key::CtrlC);
    session.wait_for_text_count("ROOT", 2);
    assert!(!home.location_exists("0/0"));
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_editor_replaces_selection_from_fake_clipboard_and_saves() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn_with_test_clipboard(
        &cargo_bin("zt"),
        &home.config_dir,
        "中文\r\n\tline  \r\n\r\nend ",
        None,
    );
    session.wait_for_text("ROOT");
    session.send_text("t Paste");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_left_click(2, 2);
    session.send_text("X");
    session.wait_for_text("PaXste");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("replace");
    session.send_left_drag(4, 0, 4, 6);
    session.wait_for_raw_text("\x1b[7m");
    session.send_key(Key::CtrlShiftV);
    session.wait_for_text("中文");
    session.send_key(Key::CtrlS);
    session.wait_for_text("location: 0/0");
    session.wait_for_text("title: PaXste");

    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.wait_for_text("line");
    session.send_key(Key::CtrlC);
    session.wait_for_text("title: PaXste");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
    assert_eq!(
        home.card_text("0/0"),
        "PaXste\n<--->\n中文\n\tline  \n\nend \n<--->\n"
    );
}

#[test]
fn terminal_editor_drag_scrolls_both_axes_and_selection_survives_resize() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn_with_test_clipboard_at_size(
        &cargo_bin("zt"),
        &home.config_dir,
        "one\ntwo\nthree\nfour\nfive\nsix\nabcdefghijklmnopqrstuvwxyz0123456789",
        7,
        20,
    );
    session.wait_for_text("ROOT");
    session.send_text("l");
    session.send_enter();
    session.wait_for_text("metadata edit");
    session.send_key(Key::CtrlShiftV);
    for _ in 0..6 {
        session.send_key(Key::Up);
    }
    session.send_key(Key::Home);
    session.wait_for_text("one");

    session.send_left_press(2, 0);
    for column in [0, 1, 0] {
        let reverse_count = session.output().matches("\x1b[7m").count();
        session.send_left_drag_to(5, column);
        session.wait_for_raw_text_count("\x1b[7m", reverse_count + 1);
    }
    session.wait_for_text("abcdefghijklmnopqrst");
    for column in [19, 18].into_iter().cycle().take(16) {
        let reverse_count = session.output().matches("\x1b[7m").count();
        session.send_left_drag_to(5, column);
        session.wait_for_raw_text_count("\x1b[7m", reverse_count + 1);
    }
    session.send_left_release(5, 19);
    session.wait_for_text("qrstuvwxyz0123456789");
    let reverse_count = session.output().matches("\x1b[7m").count();

    session.resize(8, 14);
    session.wait_for_raw_text_count("\x1b[7m", reverse_count + 1);
    session.send_key(Key::CtrlC);
    session.wait_for_text_count("ROOT", 2);
    session.send_text("q");
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
    session.send_text("t 中文主题");
    session.send_enter();
    session.wait_for_text("edit mode");

    session.send_left_drag(2, 0, 2, 5);
    session.wait_for_raw_text("\x1b[7m");

    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("中文正文");
    session.send_key(Key::CtrlS);

    session.wait_for_text("location: 0/0");
    session.wait_for_text("中文正文");
    session.send_left_drag(1, 7, 1, 12);
    session.wait_for_raw_text("\x1b[7m");
    session.send_text("root");
    session.send_enter();
    session.wait_for_text("ROOT");
    session.send_text("go 0/0");
    session.send_enter();
    session.wait_for_text("location: 0/0");
    session.wait_for_text("中文正文");
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/0"), "中文主题\n<--->\n中文正文\n<--->\n");
}

#[test]
fn terminal_session_grows_and_navigates_a_literature_tree_with_panels_and_scrolling() {
    let home = TestHome::new();
    home.configure_and_up();
    let body = (1..=40)
        .map(|line| format!("body line {line:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    home.create_literature("TreeKey", "Tree Work", &body);

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.wait_for_text("(none yet; create one with t <title>)");
    session.wait_for_text("TreeKey Tree Work");
    // ROOT, topics:, empty hint, literature:, then the clickable Literature entry.
    session.send_left_click(4, 2);
    session.wait_for_text("citation key: TreeKey");
    session.wait_for_text("lines 1-22 of 44");
    assert!(!session.plain_output().contains("body line 40"));
    session.send_key(Key::PageDown);
    session.wait_for_text("lines 22-43 of 44");
    session.send_key(Key::PageDown);
    session.wait_for_text("lines 23-44 of 44");
    session.wait_for_text("body line 40");
    session.send_key(Key::PageUp);
    session.wait_for_text("lines 2-23 of 44");

    session.send_text("b");
    session.send_enter();
    session.wait_for_text("side successors cannot start from a Literature Card");
    session.send_text("n");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_text("Chapter");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("chapter body");
    session.send_key(Key::CtrlS);
    session.wait_for_text("location: TreeKey/1");
    session.wait_for_text("parent: [[TreeKey]]");

    session.send_text("ls");
    session.send_enter();
    session.wait_for_text("Literature tree TreeKey: 2 cards");
    session.wait_for_text("TreeKey/1 Chapter");
    let views = session
        .plain_output()
        .matches("location: TreeKey/1")
        .count();
    session.send_key(Key::Esc);
    session.wait_for_text_count("location: TreeKey/1", views + 1);

    session.send_text("ls");
    session.send_enter();
    session.wait_for_text_count("Literature tree TreeKey: 2 cards", 2);
    let roots = session
        .plain_output()
        .matches("citation key: TreeKey")
        .count();
    // Panel rows: heading, then TreeKey, then TreeKey/1.
    session.send_left_click(1, 2);
    session.wait_for_text_count("citation key: TreeKey", roots + 1);

    session.send_text("go TreeKey/1");
    session.send_enter();
    session.wait_for_text_count("location: TreeKey/1", views + 2);
    session.send_text("help");
    session.send_enter();
    session.wait_for_text("commands: go [<target>] | root | up | ls | t <title> | l | n | b | e");
    session.wait_for_text("go to the parent card; a tree root goes to ROOT");
    session.send_text("up");
    session.send_enter();
    session.wait_for_text_count("citation key: TreeKey", roots + 2);
    let root_views = session.plain_output().matches("topics:").count();
    session.send_text("up");
    session.send_enter();
    session.wait_for_text_count("topics:", root_views + 1);

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
    assert_eq!(
        home.card_text("TreeKey/1"),
        "Chapter\n<--->\nchapter body\n<--->\n"
    );
}

#[test]
fn terminal_session_creates_navigates_and_lists_literature_card() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("l");
    session.send_enter();
    session.wait_for_text("metadata edit");
    session.send_text("@book{Session2025, title={Session Literature}}");
    session.send_key(Key::CtrlS);

    session.wait_for_text("Session Literature");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("session literature notes");
    session.send_key(Key::CtrlS);

    session.wait_for_text("citation key: Session2025");
    session.wait_for_text("title: Session Literature");
    session.wait_for_text("metadata:");
    session.wait_for_text("@book{Session2025, title={Session Literature}}");
    session.wait_for_text("session literature notes");

    session.send_text("root");
    session.send_enter();
    session.send_text("ls");
    session.send_enter();
    session.wait_for_text("Session2025 Session Literature");

    session.send_text("stats");
    session.send_enter();
    session.wait_for_text("total: 1 | topics: 0 | regular: 0 | literature: 1");

    session.send_text("go Session2025");
    session.send_enter();
    session.wait_for_text("citation key: Session2025");
    session.send_text("ls");
    session.send_enter();
    session.wait_for_text("Literature tree Session2025: 1 cards");
    session.send_text("help");
    session.send_enter();
    session.wait_for_text("go [<target>] | root | up");
    session.wait_for_text(" l ");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_literature_creation_retries_metadata_and_cancels_atomically() {
    let home = TestHome::new();
    home.configure_and_up();
    let invalid = "@comment{invalid}";

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("l");
    session.send_enter();
    session.wait_for_text("metadata edit");
    let blocked = home
        .zt_command_with_editor("Blocked\n<--->\nblocked\n<--->\n")
        .args(["t", "Blocked During Metadata"])
        .output()
        .expect("blocked write during metadata stage");
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("edit in progress"));
    session.send_key(Key::Esc);
    session.wait_for_text_count("ROOT", 2);
    assert!(!home.citation_exists("NeverStored"));

    session.send_text("l");
    session.send_enter();
    session.wait_for_text_count("metadata edit", 2);
    session.send_text(invalid);
    session.send_key(Key::CtrlS);
    session.wait_for_text("exactly one ordinary bibliographic entry");
    session.wait_for_text_count(invalid, 2);
    for _ in invalid.chars() {
        session.send_key(Key::Backspace);
    }
    session.send_text("@book{NeverStored, title={Canceled Body Stage}}");
    session.send_key(Key::CtrlS);
    session.wait_for_text("Canceled Body Stage");
    let blocked = home
        .zt_command_with_editor("Blocked\n<--->\nblocked\n<--->\n")
        .args(["t", "Blocked During Body"])
        .output()
        .expect("blocked write during body stage");
    assert!(!blocked.status.success());
    assert!(String::from_utf8_lossy(&blocked.stderr).contains("edit in progress"));
    session.send_key(Key::Esc);
    session.wait_for_text_count("ROOT", 3);
    assert!(!home.citation_exists("NeverStored"));

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_clicks_citation_links_refuses_topology_and_deletes_to_root() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_literature("ClickLit", "Clickable Literature", "literature body");
    let status = home
        .zt_command_with_editor("Topic\n<--->\nopen [[ClickLit]]\n<--->\n")
        .args(["t", "Topic"])
        .status()
        .expect("create citation source");
    assert!(status.success(), "create citation source failed: {status}");

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("go 0/0");
    session.send_enter();
    session.wait_for_text("open [[ClickLit]]");
    session.send_left_click(2, 8);
    session.wait_for_text("citation key: ClickLit");

    session.send_text("b");
    session.send_enter();
    session.wait_for_text("side successors cannot start from a Literature Card");
    session.send_text("mv 0/1");
    session.send_enter();
    session.wait_for_text("Literature Card cannot be moved");

    session.send_text("del");
    session.send_enter();
    session.wait_for_text("delete ClickLit");
    session.wait_for_text("type `ClickLit` to confirm:");
    let roots_before_delete = session.plain_output().matches("ROOT").count();
    session.send_text("ClickLit");
    session.send_enter();
    session.wait_for_text_count("ROOT", roots_before_delete + 1);
    assert!(!home.citation_exists("ClickLit"));

    session.send_text("go ClickLit");
    session.send_enter();
    session.wait_for_text("target `ClickLit` does not exist");
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_selects_literature_edit_part_and_confirms_rename() {
    let home = TestHome::new();
    home.configure_and_up();
    let original_bib = "@book{SessionEdit, title={Original Session Title}}";
    let clipboard_output = home.config_dir.join("modal-clipboard.txt");

    let mut session = PtySession::spawn_with_test_clipboard(
        &cargo_bin("zt"),
        &home.config_dir,
        "seed",
        Some(&clipboard_output),
    );
    session.wait_for_text("ROOT");
    session.send_text("l");
    session.send_enter();
    session.wait_for_text("metadata edit");
    session.send_text(original_bib);
    session.send_key(Key::CtrlS);
    session.wait_for_text("Original Session Title");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_text("original body");
    session.send_key(Key::CtrlS);
    session.wait_for_text("citation key: SessionEdit");

    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit literature card:");
    session.wait_for_text("1. metadata");
    session.wait_for_text("2. main text");
    session.send_left_drag(0, 0, 1, 12);
    session.wait_for_raw_text("\x1b[7m");
    session.send_key(Key::CtrlShiftC);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !clipboard_output.exists() && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert_eq!(
        fs::read_to_string(&clipboard_output).unwrap(),
        "edit literature card:\n  1. metadata"
    );
    session.send_text("3");
    session.send_enter();
    session.wait_for_text("invalid edit option");
    session.send_text("1");
    session.send_enter();
    session.wait_for_text("metadata edit");
    session.send_key(Key::End);
    for _ in original_bib.chars() {
        session.send_key(Key::Backspace);
    }
    session.send_text("@book{SessionRenamed, title={Renamed Session Title}}");
    session.send_key(Key::CtrlS);
    session.wait_for_text("SessionEdit -> SessionRenamed");
    session.wait_for_text("type `move` to confirm:");
    session.send_text("move");
    session.send_enter();
    session.wait_for_text("citation key: SessionRenamed");
    session.wait_for_text("title: Renamed Session Title");
    session.wait_for_text("original body");

    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit literature card:");
    session.send_text("2");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_key(Key::Down);
    session.send_key(Key::Down);
    session.send_key(Key::End);
    session.send_text(" edited");
    session.send_key(Key::CtrlS);
    session.wait_for_text("citation key: SessionRenamed");
    session.wait_for_text("original body edited");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_topic_title_does_not_parse_shell_quotes() {
    let home = TestHome::new();
    home.configure_and_up();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("t \"Quoted Topic\"");
    session.send_enter();
    session.wait_for_text("edit mode");
    session.send_key(Key::CtrlS);
    session.wait_for_text("location: 0/0");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/0"), "\"Quoted Topic\"\n<--->\n\n<--->\n");
}

#[test]
fn terminal_session_navigates_and_activates_rendered_links() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.wait_for_text("0/0 Topic");

    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.wait_for_text("[[0/0]]");

    session.send_text("go 9/9");
    session.send_enter();
    session.wait_for_text("target `9/9` does not exist");
    session.wait_for_text("location: 0/1");

    session.send_left_drag(2, 5, 2, 8);
    session.wait_for_raw_text("\x1b[7m");
    assert!(!session.plain_output().contains("location: 0/0"));

    session.send_left_click(2, 5);
    session.wait_for_text("location: 0/0");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_bare_go_returns_to_root_and_clears_status() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");

    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("stats");
    session.send_enter();
    session.wait_for_text("total: 2 | topics: 1 | regular: 1");

    let typed_go_before = session.plain_output().matches("> go").count();
    session.send_text("go");
    session.wait_for_text_count("> go", typed_go_before + 1);
    let roots_before_go = session.plain_output().matches("ROOT").count();
    let output_before_go = session.output().len();
    session.send_enter();
    session.wait_for_text_count("ROOT", roots_before_go + 1);

    let output = session.output();
    let go_redraw = &output[output_before_go..];
    assert!(!go_redraw.contains("unknown session command: go"));
    assert!(!go_redraw.contains("total: 2 | topics: 1 | regular: 1"));

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_bare_go_at_root_is_idempotent() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.wait_for_text("0/0 Topic");

    let typed_go_before = session.plain_output().matches("> go").count();
    session.send_text("go");
    session.wait_for_text_count("> go", typed_go_before + 1);
    let roots_before_enter = session.plain_output().matches("ROOT").count();
    let topics_before_enter = session.plain_output().matches("0/0 Topic").count();
    session.send_enter();
    session.wait_for_text_count("ROOT", roots_before_enter + 1);
    session.wait_for_text_count("0/0 Topic", topics_before_enter + 1);

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");
}

#[test]
fn terminal_session_rejects_extra_go_arguments_without_moving_pointer() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("go 0/0 extra");
    session.wait_for_text("> go 0/0 extra");
    let output_before_enter = session.output().len();
    session.send_enter();
    session.wait_for_text("usage: go [<target>]");

    let output = session.output();
    let error_redraw = &output[output_before_enter..];
    assert!(error_redraw.contains("location: 0/1"));
    assert!(!error_redraw.contains("unknown session command: go"));

    session.send_text("q");
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

    session.send_text("stats");
    session.send_enter();
    session.wait_for_text("total: 2 | topics: 1 | regular: 1");

    session.send_text("status");
    session.send_enter();
    session.wait_for_text("state: up | sessions: 1");

    session.send_text("ls");
    session.send_enter();
    session.wait_for_text_count("0/0 Topic", 2);
    assert!(!session.plain_output().contains("0/1 Base"));

    session.send_text("lsbk");
    session.send_enter();
    session.wait_for_text("no broken links");

    session.send_text("help");
    session.send_enter();
    session.wait_for_text("go [<target>] | root");

    session.send_text("foo bar");
    session.send_enter();
    session.wait_for_text("unknown session command: foo");

    session.send_text("zt e");
    session.send_enter();
    session.wait_for_text("unknown session command: zt");

    session.send_text("  go 0/1  ");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("root");
    session.send_enter();
    session.wait_for_text("ROOT");

    session.send_text("q");
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
    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("n");
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

    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("b");
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

    session.send_text("q");
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

    session.send_text("status");
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
    session.send_text("go 0/1|a");
    session.send_enter();
    session.wait_for_text("location: 0/1|a");

    session.send_text("mv 0/2|a");
    session.send_enter();
    session.wait_for_text("move verification:");
    session.send_text("move");
    session.send_enter();
    session.wait_for_text("location: 0/2|a");

    session.send_text("del");
    session.send_enter();
    session.wait_for_text("delete verification:");
    session.send_text("delete");
    session.send_enter();
    session.wait_for_text("location: 0/2");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert!(!home.location_exists("0/1|a"));
    assert!(!home.location_exists("0/2|a"));
    assert!(home.card_text("0/2").contains("[[0/2|a]]"));
}

#[test]
fn terminal_delete_confirmation_does_not_duplicate_prompt_or_append_to_command_bar() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_move_fixture();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("go 0/1|a");
    session.send_enter();
    session.wait_for_text("location: 0/1|a");

    session.send_text("del");
    session.send_enter();
    session.wait_for_text("type `delete` to confirm:");
    session.send_text("delete");
    session.send_enter();
    session.wait_for_text("location: 0/1");

    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();
    assert!(status.success(), "session exited with {status}");

    let output = session.plain_output();
    assert!(
        !output.contains("> deldelete verification:"),
        "delete verification was appended to the command bar:\n{}",
        output.escape_debug()
    );
    assert_eq!(
        output.matches("> del").count(),
        1,
        "delete command was redrawn more than once:\n{}",
        output.escape_debug()
    );
    assert_eq!(
        output.matches("type `delete` to confirm: delete").count(),
        1,
        "final delete confirmation prompt was duplicated:\n{}",
        output.escape_debug()
    );
}

#[test]
fn terminal_session_edit_lock_blocks_other_writes() {
    let home = TestHome::new();
    home.configure_and_up();
    home.create_topic_and_base();

    let mut session = PtySession::spawn(&cargo_bin("zt"), &home.config_dir);
    session.wait_for_text("ROOT");
    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("e");
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
    session.send_text("q");
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
    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit mode");

    session.send_key(Key::CtrlC);
    session.wait_for_text("location: 0/1");
    session.send_text("q");
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
    session.send_text("go 0/1");
    session.send_enter();
    session.wait_for_text("location: 0/1");
    session.send_text("e");
    session.send_enter();
    session.wait_for_text("edit mode");

    session.send_key(Key::Down);
    session.send_key(Key::Delete);
    session.send_key(Key::CtrlS);
    session.wait_for_text("exactly two");

    session.send_key(Key::Esc);
    session.wait_for_text("location: 0/1");
    session.send_text("q");
    session.send_enter();
    let status = session.wait_for_exit();

    assert!(status.success(), "session exited with {status}");
    assert_eq!(home.card_text("0/1"), original);
}
