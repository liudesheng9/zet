//! The login command `zt autostart on` registers: it starts the ZT service when the user signs
//! in. It is a window-less program, so signing in flashes no console; the work is done by the
//! `zt` binary beside it, run without a console window, which records the outcome in
//! `autostart.log` in the config directory.
#![cfg_attr(windows, windows_subsystem = "windows")]

use std::env;
use std::process::{Command, Stdio};

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(zt) = env::current_exe()
        .ok()
        .map(|exe| exe.with_file_name(format!("zt{}", env::consts::EXE_SUFFIX)))
    else {
        std::process::exit(1);
    };
    let mut command = Command::new(zt);
    command
        .arg("__boot")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let [flag, dir] = args.as_slice()
        && flag == "--config-dir"
    {
        command.env("ZT_CONFIG_DIR", dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let code = command
        .status()
        .ok()
        .and_then(|status| status.code())
        .unwrap_or(1);
    std::process::exit(code);
}
