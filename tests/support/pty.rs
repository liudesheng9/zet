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
}

pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    CtrlC,
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
            Key::Delete => b"\x1b[3~",
            Key::CtrlC => b"\x03",
            Key::CtrlS => b"\x13",
            Key::Esc => b"\x1b",
        }
    }
}

impl PtySession {
    pub fn spawn(program: &Path, config_dir: &Path) -> Self {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");

        let mut cmd = CommandBuilder::new(program);
        cmd.env("ZT_CONFIG_DIR", config_dir);
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

    pub fn send_left_click(&mut self, row: u16, col: u16) {
        let sequence = format!("\x1b[<0;{};{}M", col + 1, row + 1);
        self.send_text(&sequence);
    }

    pub fn wait_for_text(&mut self, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut answered_cursor_position_query = false;
        while Instant::now() < deadline {
            let output = self.output();
            if output.contains(expected) || strip_ansi(&output).contains(expected) {
                return;
            }
            if !answered_cursor_position_query && output.contains("\x1b[6n") {
                self.send_text("\x1b[1;1R");
                answered_cursor_position_query = true;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "timed out waiting for `{expected}` in PTY output:\n{}",
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
