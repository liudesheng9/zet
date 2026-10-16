//! The service follows the machine: `zt autostart on` registers a login command that brings
//! the service up when the user signs in, and the daemon takes the service down when the
//! machine ends the user's session, or reads as down when it died with the machine.
//!
//! Signing in is simulated by running the registered login command the way Windows does, and
//! the machine going down by sending the daemon's window the messages Windows sends at
//! shutdown and sign-out. The tests register under a scratch key, never the user's Run key.
#![cfg(windows)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tempfile::TempDir;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RegDeleteTreeW};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    ENDSESSION_LOGOFF, FindWindowW, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_ENDSESSION,
    WM_QUERYENDSESSION,
};

static NEXT_KEY: AtomicUsize = AtomicUsize::new(0);

struct Machine {
    _tmp: TempDir,
    config_dir: PathBuf,
    archive_root: PathBuf,
    run_key: String,
}

impl Machine {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let run_key = format!(
            r"Software\zt-tests\{}-{nanos}-{}\Run",
            std::process::id(),
            NEXT_KEY.fetch_add(1, Ordering::SeqCst)
        );
        Self {
            config_dir: tmp.path().join("config"),
            archive_root: tmp.path().join("archive"),
            _tmp: tmp,
            run_key,
        }
    }

    fn zt(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .env("ZT_AUTOSTART_KEY", &self.run_key)
            // Keeps the template a new Card opens with.
            .env("EDITOR", "cmd /C rem");
        cmd
    }

    fn configure(&self) {
        self.zt()
            .args([
                "config",
                "set",
                "archive_root",
                self.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
    }

    fn stdout(&self, args: &[&str]) -> String {
        let output = self.zt().args(args).output().expect("run zt");
        assert!(
            output.status.success(),
            "zt {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("utf-8 stdout")
    }

    fn is_up(&self) -> bool {
        self.stdout(&["status"]).contains("state: up")
    }

    fn pid(&self) -> String {
        fs::read_to_string(self.archive_root.join("zt.pid"))
            .expect("service record")
            .trim()
            .to_string()
    }

    fn log(&self) -> String {
        fs::read_to_string(self.archive_root.join("zt.log")).unwrap_or_default()
    }

    fn boot_log(&self) -> String {
        fs::read_to_string(self.config_dir.join("autostart.log")).unwrap_or_default()
    }

    /// The login command as Windows stores it, read with `reg.exe` rather than through zt.
    fn registered_command(&self) -> Option<String> {
        let output = StdCommand::new("reg")
            .args(["query", &format!(r"HKCU\{}", self.run_key), "/v", "zt"])
            .output()
            .expect("reg query");
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| {
                let line = line.trim();
                let rest = line.strip_prefix("zt")?.trim_start();
                Some(rest.strip_prefix("REG_SZ")?.trim().to_string())
            })
    }

    /// Signs in: runs the registered login command as Windows does, the program named by its
    /// first word given the whole command line.
    fn sign_in(&self) -> std::process::ExitStatus {
        let command = self
            .registered_command()
            .expect("a registered login command");
        let rest = command.strip_prefix('"').expect("quoted program");
        let (program, args) = rest.split_once('"').expect("closing quote");
        StdCommand::new(program)
            .raw_arg(args.trim_start())
            .env_remove("ZT_CONFIG_DIR")
            .env("ZT_GUI_PORT", "0")
            .status()
            .expect("run the login command")
    }

    /// Delivers what Windows sends every top-level window as the session ends.
    fn end_session(&self, pid: &str, ending: bool, flags: u32) {
        let window = find_daemon_window(pid);
        let mut answer = 0usize;
        unsafe {
            SendMessageTimeoutW(
                window,
                WM_QUERYENDSESSION,
                0,
                flags as isize,
                SMTO_ABORTIFHUNG,
                5000,
                &mut answer,
            );
        }
        assert_eq!(answer, 1, "the daemon must let the session end");
        unsafe {
            SendMessageTimeoutW(
                window,
                WM_ENDSESSION,
                ending as usize,
                flags as isize,
                SMTO_ABORTIFHUNG,
                5000,
                &mut answer,
            );
        }
    }

    fn wait_until(&self, what: &str, condition: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if condition() {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        assert!(condition(), "timed out waiting until {what}");
    }
}

impl Drop for Machine {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        let parent = Path::new(&self.run_key).parent().unwrap();
        let key: Vec<u16> = parent
            .to_string_lossy()
            .encode_utf16()
            .chain(Some(0))
            .collect();
        unsafe { RegDeleteTreeW(HKEY_CURRENT_USER, key.as_ptr()) };
        thread::sleep(Duration::from_millis(300));
    }
}

fn find_daemon_window(pid: &str) -> *mut core::ffi::c_void {
    let class: Vec<u16> = "ztDaemon".encode_utf16().chain(Some(0)).collect();
    let title: Vec<u16> = format!("zt daemon {pid}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let window = unsafe { FindWindowW(class.as_ptr(), title.as_ptr()) };
        if !window.is_null() {
            return window;
        }
        assert!(
            Instant::now() < deadline,
            "daemon {pid} has no window for the system to notify"
        );
        thread::sleep(Duration::from_millis(50));
    }
}

fn process_exists(pid: &str) -> bool {
    let output = StdCommand::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .expect("tasklist");
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .any(|word| word == pid)
}

#[test]
fn autostart_registers_a_window_less_login_command_and_removes_it() {
    let machine = Machine::new();
    machine
        .zt()
        .arg("autostart")
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: off"));

    machine
        .zt()
        .args(["autostart", "on"])
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: on"));
    let command = machine.registered_command().expect("registered");
    let boot = assert_cmd::cargo::cargo_bin("zt-boot");
    assert_eq!(
        command,
        format!(
            "\"{}\" --config-dir \"{}\"",
            boot.display(),
            machine.config_dir.display()
        )
    );
    machine
        .zt()
        .arg("autostart")
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: on"))
        .stdout(predicate::str::contains(command.as_str()))
        .stdout(predicate::str::contains("warning").not());
    machine
        .zt()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: on"));

    // Enabling twice keeps one entry.
    machine.zt().args(["autostart", "on"]).assert().success();
    assert_eq!(machine.registered_command(), Some(command));

    machine
        .zt()
        .args(["autostart", "off"])
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: off"));
    assert_eq!(machine.registered_command(), None);
    machine
        .zt()
        .args(["autostart", "off"])
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: already off"));
    machine
        .zt()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("autostart: off"));
    machine
        .zt()
        .args(["autostart", "sometimes"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("usage: zt autostart [on|off]"));
}

#[test]
fn the_login_command_is_a_windows_program_without_a_console() {
    let boot = fs::read(assert_cmd::cargo::cargo_bin("zt-boot")).expect("zt-boot binary");
    let pe = u32::from_le_bytes(boot[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&boot[pe..pe + 4], b"PE\0\0");
    // The optional header follows the 20-byte file header; its Subsystem field is at 68.
    let subsystem = u16::from_le_bytes(boot[pe + 24 + 68..pe + 24 + 70].try_into().unwrap());
    const IMAGE_SUBSYSTEM_WINDOWS_GUI: u16 = 2;
    assert_eq!(subsystem, IMAGE_SUBSYSTEM_WINDOWS_GUI);
}

#[test]
fn signing_in_brings_the_service_up() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().args(["autostart", "on"]).assert().success();
    assert!(!machine.is_up());

    let status = machine.sign_in();
    assert!(status.success(), "login command failed: {status}");
    assert!(machine.is_up());
    let pid = machine.pid();
    assert!(process_exists(&pid));
    assert!(
        machine
            .boot_log()
            .contains(&format!("service started pid: {pid}")),
        "autostart.log: {}",
        machine.boot_log()
    );
    assert!(machine.log().contains("login start: service started pid:"));
    machine
        .zt()
        .arg("autostart")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "last login start: service started pid: {pid}"
        )));
    machine
        .zt()
        .args(["t", "Written after sign-in"])
        .assert()
        .success();

    // Signing in again leaves the running service alone.
    assert!(machine.sign_in().success());
    assert_eq!(machine.pid(), pid);
    assert!(
        machine
            .boot_log()
            .contains(&format!("service already running pid: {pid}"))
    );
}

#[test]
fn a_login_start_that_fails_is_recorded() {
    let machine = Machine::new();
    machine.zt().args(["autostart", "on"]).assert().success();

    let status = machine.sign_in();
    assert!(!status.success());
    assert!(
        machine
            .boot_log()
            .contains("error: archive_root is not configured"),
        "autostart.log: {}",
        machine.boot_log()
    );
    machine
        .zt()
        .arg("autostart")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "last login start: error: archive_root is not configured",
        ));
}

#[test]
fn shutting_down_the_machine_takes_the_service_down() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().arg("up").assert().success();
    let pid = machine.pid();

    // An open Session does not hold the machine up: the system is ending it too.
    let mut session = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
        .env("ZT_CONFIG_DIR", &machine.config_dir)
        .env("ZT_GUI_PORT", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn session");
    machine.wait_until("the Session registers", || {
        machine.stdout(&["status"]).contains("sessions: 1")
    });
    machine
        .zt()
        .arg("down")
        .assert()
        .failure()
        .stderr(predicate::str::contains("session(s) are open"));

    machine.end_session(&pid, true, 0);
    assert!(!machine.archive_root.join("zt.pid").exists());
    assert!(
        machine
            .log()
            .contains("service stopped: the machine is shutting down"),
        "zt.log: {}",
        machine.log()
    );
    machine.wait_until("the daemon exits", || !process_exists(&pid));
    assert!(!machine.is_up());
    machine
        .zt()
        .arg("down")
        .assert()
        .success()
        .stdout(predicate::str::contains("service already stopped"));

    session
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"status\n")
        .unwrap();
    let output = session.wait_with_output().expect("session output");
    assert!(String::from_utf8_lossy(&output.stdout).contains("service disconnected"));
}

#[test]
fn signing_out_takes_the_service_down() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().arg("up").assert().success();
    let pid = machine.pid();

    machine.end_session(&pid, true, ENDSESSION_LOGOFF);
    assert!(!machine.archive_root.join("zt.pid").exists());
    assert!(
        machine
            .log()
            .contains("service stopped: the user is signing out")
    );
    machine.wait_until("the daemon exits", || !process_exists(&pid));
}

#[test]
fn a_cancelled_shutdown_leaves_the_service_up() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().arg("up").assert().success();
    let pid = machine.pid();

    machine.end_session(&pid, false, 0);
    thread::sleep(Duration::from_millis(600));
    assert!(machine.is_up());
    assert_eq!(machine.pid(), pid);
    assert!(!machine.log().contains("service stopped:"));
    machine.zt().arg("down").assert().success();
    machine.wait_until("the daemon exits", || !process_exists(&pid));
}

#[test]
fn restarting_the_machine_brings_the_service_down_and_up_again() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().args(["autostart", "on"]).assert().success();
    assert!(machine.sign_in().success());
    let first = machine.pid();
    machine
        .zt()
        .args(["t", "Before the restart"])
        .assert()
        .success();

    machine.end_session(&first, true, 0);
    machine.wait_until("the daemon exits", || !process_exists(&first));
    assert!(!machine.is_up());

    assert!(machine.sign_in().success());
    let second = machine.pid();
    assert_ne!(first, second);
    assert!(machine.is_up());
    machine
        .zt()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("topics: 1"));
    let log = machine.log();
    let stopped = log
        .find("service stopped: the machine is shutting down")
        .unwrap();
    let restarted = log.rfind("login start: service started").unwrap();
    assert!(stopped < restarted, "zt.log: {log}");
}

/// Ends the daemon at once, as a power cut or a killed process would, with no chance to clean
/// up, and waits until it is gone. Its record stays behind.
fn kill_daemon(machine: &Machine) -> String {
    let pid = machine.pid();
    let killed = StdCommand::new("taskkill")
        .args(["/F", "/PID", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("taskkill");
    assert!(killed.success());
    machine.wait_until("the daemon is gone", || !process_exists(&pid));
    assert!(machine.archive_root.join("zt.pid").exists());
    pid
}

#[test]
fn the_next_command_restarts_a_daemon_that_ended_without_zt_down() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().arg("up").assert().success();
    let pid = kill_daemon(&machine);

    machine
        .zt()
        .arg("stats")
        .assert()
        .success()
        .stdout(predicate::str::contains("total:"));
    let restarted = machine.pid();
    assert_ne!(restarted, pid);
    assert!(process_exists(&restarted));
    let log = machine.log();
    let cleared = log
        .find(&format!(
            "service record of daemon {pid} cleared: the daemon ended without `zt down`"
        ))
        .expect("the stale record is logged");
    let started = log
        .find(&format!(
            "service restarted: daemon {pid} ended without `zt down`"
        ))
        .expect("the restart is logged");
    assert!(cleared < started, "zt.log: {log}");

    // `zt status` is a check of the service too.
    let pid = kill_daemon(&machine);
    machine
        .zt()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: up"));
    assert_ne!(machine.pid(), pid);

    // `zt down` is the way to keep the service down.
    machine.zt().arg("down").assert().success();
    machine
        .zt()
        .arg("stats")
        .assert()
        .failure()
        .stderr(predicate::str::contains("service is not up"));
    machine
        .zt()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("state: down"));
}

#[test]
fn commands_that_find_the_same_dead_daemon_start_one_service() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().arg("up").assert().success();
    let pid = kill_daemon(&machine);

    let commands: Vec<_> = (0..4)
        .map(|_| {
            let mut command = StdCommand::new(env!("CARGO_BIN_EXE_zt"));
            command
                .arg("stats")
                .env("ZT_CONFIG_DIR", &machine.config_dir)
                .env("ZT_GUI_PORT", "0")
                .env("ZT_AUTOSTART_KEY", &machine.run_key)
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            command.spawn().expect("zt stats")
        })
        .collect();
    for command in commands {
        let output = command.wait_with_output().expect("zt stats");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let log = machine.log();
    assert_eq!(
        log.matches(&format!("service restarted: daemon {pid}"))
            .count(),
        1,
        "zt.log: {log}"
    );
    assert_eq!(log.matches("daemon started").count(), 2, "zt.log: {log}");
    assert!(process_exists(&machine.pid()));
}

#[test]
fn a_record_left_by_an_older_daemon_does_not_block_the_next_start() {
    let machine = Machine::new();
    machine.configure();
    machine.zt().arg("up").assert().success();
    machine.zt().arg("down").assert().success();
    machine.wait_until("the daemon stops", || {
        machine.log().contains("daemon stopped")
    });

    // zt 0.7 left only zt.pid behind, naming a process that is gone after the restart.
    fs::remove_file(machine.archive_root.join("zt.daemon.lock")).expect("remove lock");
    let mut gone = StdCommand::new("cmd")
        .args(["/C", "exit"])
        .spawn()
        .expect("spawn");
    let gone_pid = gone.id().to_string();
    gone.wait().unwrap();
    fs::write(machine.archive_root.join("zt.pid"), &gone_pid).expect("old record");

    machine
        .zt()
        .arg("up")
        .assert()
        .success()
        .stdout(predicate::str::contains("service started pid:"));
    assert_ne!(machine.pid(), gone_pid);
    assert!(
        machine
            .log()
            .contains(&format!("service record of daemon {gone_pid} cleared"))
    );
}
