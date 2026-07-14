use portable_pty::{CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct PtySession {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<String>>,
    answered_cursor_position_queries: usize,
}

pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Backspace,
    Delete,
    CtrlC,
    CtrlShiftC,
    CtrlShiftV,
    CtrlS,
    Esc,
}

impl Key {
    fn bytes(&self) -> &'static [u8] {
        match self {
            Key::Up => b"\x1b[A",
            Key::Down => b"\x1b[B",
            Key::Left => b"\x1b[D",
            Key::Right => b"\x1b[C",
            Key::Home => b"\x1b[H",
            Key::End => b"\x1b[F",
            Key::Backspace => b"\x7f",
            Key::Delete => b"\x1b[3~",
            Key::CtrlC => b"\x03",
            Key::CtrlShiftC => b"\x1c",
            Key::CtrlShiftV => b"\x1d",
            Key::CtrlS => b"\x13",
            Key::Esc => b"\x1b",
        }
    }
}

impl PtySession {
    pub fn spawn(program: &Path, config_dir: &Path) -> Self {
        Self::spawn_with_args(program, config_dir, &[])
    }

    pub fn spawn_with_args(program: &Path, config_dir: &Path, args: &[&str]) -> Self {
        Self::spawn_configured(program, config_dir, args, None, false, (24, 80))
    }

    pub fn spawn_with_test_clipboard(
        program: &Path,
        config_dir: &Path,
        clipboard_text: &str,
        clipboard_output: Option<&Path>,
    ) -> Self {
        Self::spawn_configured(
            program,
            config_dir,
            &[],
            Some((clipboard_text, clipboard_output)),
            false,
            (24, 80),
        )
    }

    pub fn spawn_with_test_clipboard_at_size(
        program: &Path,
        config_dir: &Path,
        clipboard_text: &str,
        rows: u16,
        cols: u16,
    ) -> Self {
        Self::spawn_configured(
            program,
            config_dir,
            &[],
            Some((clipboard_text, None)),
            false,
            (rows, cols),
        )
    }

    pub fn spawn_with_real_clipboard_shortcuts(program: &Path, config_dir: &Path) -> Self {
        Self::spawn_configured(program, config_dir, &[], None, true, (24, 80))
    }

    fn spawn_configured(
        program: &Path,
        config_dir: &Path,
        args: &[&str],
        test_clipboard: Option<(&str, Option<&Path>)>,
        test_shortcuts: bool,
        size: (u16, u16),
    ) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: size.0,
                cols: size.1,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");

        let mut cmd = CommandBuilder::new(program);
        cmd.args(args);
        cmd.env("ZT_CONFIG_DIR", config_dir);
        if let Some((text, output)) = test_clipboard {
            cmd.env("ZT_TEST_CLIPBOARD_TEXT", text);
            if let Some(path) = output {
                cmd.env("ZT_TEST_CLIPBOARD_OUTPUT", path);
            }
        }
        if test_shortcuts {
            cmd.env("ZT_TEST_SHORTCUTS", "1");
        }
        let child = pair.slave.spawn_command(cmd).expect("spawn pty command");
        drop(pair.slave);

        let mut reader = pair.master.try_clone_reader().expect("clone pty reader");
        let writer = pair.master.take_writer().expect("take pty writer");
        let output = Arc::new(Mutex::new(String::new()));
        let output_reader = Arc::clone(&output);
        thread::spawn(move || {
            let mut buffer = [0; 4096];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&buffer[..n]);
                        output_reader.lock().expect("output lock").push_str(&chunk);
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            child,
            _master: pair.master,
            writer,
            output,
            answered_cursor_position_queries: 0,
        }
    }

    pub fn send_text(&mut self, text: &str) {
        self.writer.write_all(text.as_bytes()).expect("pty write");
        self.writer.flush().expect("pty flush");
    }

    pub fn send_enter(&mut self) {
        self.send_text("\r");
    }

    pub fn send_key(&mut self, key: Key) {
        self.writer.write_all(key.bytes()).expect("pty key write");
        self.writer.flush().expect("pty flush");
    }

    pub fn resize(&self, rows: u16, cols: u16) {
        self._master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("pty resize");
    }

    pub fn send_left_click(&mut self, row: u16, col: u16) {
        self.send_left_press(row, col);
        self.send_left_release(row, col);
    }

    pub fn send_left_press(&mut self, row: u16, col: u16) {
        self.send_text(&format!("\x1b[<0;{};{}M", col + 1, row + 1));
    }

    pub fn send_left_drag_to(&mut self, row: u16, col: u16) {
        self.send_text(&format!("\x1b[<32;{};{}M", col + 1, row + 1));
    }

    pub fn send_left_release(&mut self, row: u16, col: u16) {
        self.send_text(&format!("\x1b[<0;{};{}m", col + 1, row + 1));
    }

    pub fn send_left_drag(&mut self, from_row: u16, from_col: u16, to_row: u16, to_col: u16) {
        self.send_left_press(from_row, from_col);
        self.send_left_drag_to(to_row, to_col);
        self.send_left_release(to_row, to_col);
    }

    pub fn wait_for_raw_text(&mut self, expected: &str) {
        self.wait_for_raw_text_count(expected, 1);
    }

    pub fn wait_for_raw_text_count(&mut self, expected: &str, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.output().matches(expected).count() >= count {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for {count} occurrence(s) of raw terminal text `{expected}` in PTY output:\n{}",
            self.output().escape_debug()
        );
    }

    pub fn wait_for_text(&mut self, expected: &str) {
        self.wait_for_text_count(expected, 1);
    }

    pub fn wait_for_text_count(&mut self, expected: &str, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let output = self.output();
            if output.matches(expected).count() >= count
                || strip_ansi(&output).matches(expected).count() >= count
            {
                return;
            }
            let cursor_position_queries = output.matches("\x1b[6n").count();
            while self.answered_cursor_position_queries < cursor_position_queries {
                self.send_text("\x1b[1;1R");
                self.answered_cursor_position_queries += 1;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for {count} occurrence(s) of `{expected}` in PTY output:\n{}",
            self.output().escape_debug()
        );
    }

    pub fn wait_for_exit(&mut self) -> portable_pty::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(status) = self.child.try_wait().expect("try wait child") {
                return status;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("PTY child did not exit before timeout");
    }

    pub fn output(&self) -> String {
        self.output.lock().expect("output lock").clone()
    }

    pub fn plain_output(&self) -> String {
        strip_ansi(&self.output())
    }
}

fn strip_ansi(input: &str) -> String {
    let mut output = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            match chars.next() {
                Some('[') => {
                    for c in chars.by_ref() {
                        if ('@'..='~').contains(&c) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    while let Some(c) = chars.next() {
                        if c == '\x07' {
                            break;
                        }
                        if c == '\x1b' && chars.peek() == Some(&'\\') {
                            chars.next();
                            break;
                        }
                    }
                }
                Some(_) | None => {}
            }
        } else if ch != '\r' {
            output.push(ch);
        }
    }
    output
}

impl Drop for PtySession {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
        }
    }
}
