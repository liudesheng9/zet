//! Starting the ZT service when the user signs in: `zt autostart on` registers `zt-boot`, the
//! window-less login command beside `zt`, under the user's Run key, so Windows starts the
//! service at every sign-in (and so after every boot) without admin rights. The daemon stops
//! itself when the machine goes down (see `shutdown`). Each login start is recorded in
//! `autostart.log` in the config directory, since there is no console to report it.

use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_FILE: &str = "autostart.log";

pub(crate) fn run(args: &[String]) -> Result<()> {
    match args {
        [] => print_status(),
        [arg] if arg == "on" => enable(),
        [arg] if arg == "off" => disable(),
        _ => bail!("usage: zt autostart [on|off]"),
    }
}

/// The `autostart:` line of `zt status`, where autostart is supported.
pub(crate) fn status_line() -> Option<String> {
    registry::supported().then(|| match registry::read() {
        Ok(Some(_)) => "autostart: on".to_string(),
        Ok(None) => "autostart: off".to_string(),
        Err(error) => format!("autostart: unknown ({error:#})"),
    })
}

/// Records how a login start went, for `zt autostart` to show.
pub(crate) fn record_boot(outcome: &str) -> Result<()> {
    let path = log_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    writeln!(file, "{seconds} {}", outcome.replace(['\r', '\n'], " "))?;
    Ok(())
}

fn log_path() -> Result<PathBuf> {
    let config = crate::config_file()?;
    Ok(config.with_file_name(LOG_FILE))
}

fn last_boot() -> Option<String> {
    let log = fs::read_to_string(log_path().ok()?).ok()?;
    log.lines().last().map(str::to_string)
}

/// The login command: `zt-boot` beside this `zt`, told which config directory to use when
/// `ZT_CONFIG_DIR` overrides the default.
fn login_command() -> Result<(PathBuf, String)> {
    let exe = std::env::current_exe().context("cannot locate the zt binary")?;
    let boot = exe.with_file_name(format!("zt-boot{}", std::env::consts::EXE_SUFFIX));
    let mut command = format!("\"{}\"", boot.display());
    if let Some(dir) = std::env::var_os("ZT_CONFIG_DIR") {
        let dir = PathBuf::from(dir);
        let dir = dir.to_string_lossy();
        command.push_str(&format!(
            " --config-dir \"{}\"",
            dir.trim_end_matches(['\\', '/'])
        ));
    }
    Ok((boot, command))
}

fn enable() -> Result<()> {
    if !registry::supported() {
        bail!("zt autostart is only available on Windows");
    }
    let (boot, command) = login_command()?;
    if !boot.is_file() {
        bail!(
            "{} is missing; install zt with `cargo install --path . --locked`",
            boot.display()
        );
    }
    registry::write(&command)?;
    println!("autostart: on");
    println!("the service starts when you sign in and stops when the machine goes down");
    println!("command: {command}");
    Ok(())
}

fn disable() -> Result<()> {
    if !registry::supported() {
        bail!("zt autostart is only available on Windows");
    }
    if registry::remove()? {
        println!("autostart: off");
    } else {
        println!("autostart: already off");
    }
    Ok(())
}

fn print_status() -> Result<()> {
    if !registry::supported() {
        bail!("zt autostart is only available on Windows");
    }
    match registry::read()? {
        Some(command) => {
            println!("autostart: on");
            println!("command: {command}");
            if let Some(program) = program_of(&command)
                && !program.is_file()
            {
                println!(
                    "warning: {} is missing; run `zt autostart on` again",
                    program.display()
                );
            }
        }
        None => println!("autostart: off"),
    }
    if let Some(last) = last_boot() {
        let (seconds, outcome) = last.split_once(' ').unwrap_or(("", &last));
        println!("last login start: {outcome} (at {seconds})");
    }
    Ok(())
}

/// The program a registered command line runs: its first, possibly quoted, word.
fn program_of(command: &str) -> Option<PathBuf> {
    let command = command.trim_start();
    let program = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => command.split_whitespace().next()?,
    };
    Some(PathBuf::from(program))
}

#[cfg(windows)]
mod registry {
    use anyhow::{Result, bail};
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS};
    use windows_sys::Win32::System::Registry::{
        HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const VALUE: &str = "zt";

    pub(super) fn supported() -> bool {
        true
    }

    /// The Run key under HKEY_CURRENT_USER; tests point `ZT_AUTOSTART_KEY` at a scratch key.
    fn key() -> Vec<u16> {
        let key = std::env::var("ZT_AUTOSTART_KEY").unwrap_or_else(|_| RUN_KEY.to_string());
        wide(&key)
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    fn check(status: u32, action: &str) -> Result<()> {
        if status != ERROR_SUCCESS {
            bail!(
                "failed to {action} the login command: {}",
                std::io::Error::from_raw_os_error(status as i32)
            );
        }
        Ok(())
    }

    pub(super) fn read() -> Result<Option<String>> {
        let (key, value) = (key(), wide(VALUE));
        let mut size = 0u32;
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                null_mut(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(status, "read")?;
        let mut buffer = vec![0u16; (size as usize).div_ceil(2)];
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        check(status, "read")?;
        let end = buffer
            .iter()
            .position(|&unit| unit == 0)
            .unwrap_or(buffer.len());
        Ok(Some(String::from_utf16_lossy(&buffer[..end])))
    }

    pub(super) fn write(command: &str) -> Result<()> {
        let (key, value, data) = (key(), wide(VALUE), wide(command));
        let status = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                value.as_ptr(),
                REG_SZ,
                data.as_ptr().cast(),
                (data.len() * 2) as u32,
            )
        };
        check(status, "register")
    }

    /// Removes the login command; false when there was none.
    pub(super) fn remove() -> Result<bool> {
        let (key, value) = (key(), wide(VALUE));
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), value.as_ptr()) };
        if status == ERROR_FILE_NOT_FOUND {
            return Ok(false);
        }
        check(status, "remove")?;
        Ok(true)
    }
}

#[cfg(not(windows))]
mod registry {
    use anyhow::Result;

    pub(super) fn supported() -> bool {
        false
    }

    pub(super) fn read() -> Result<Option<String>> {
        Ok(None)
    }

    pub(super) fn write(_command: &str) -> Result<()> {
        Ok(())
    }

    pub(super) fn remove() -> Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::program_of;
    use std::path::PathBuf;

    #[test]
    fn program_of_reads_the_quoted_or_bare_first_word() {
        assert_eq!(
            program_of(r#""C:\Program Files\zt\zt-boot.exe" --config-dir "C:\cfg""#),
            Some(PathBuf::from(r"C:\Program Files\zt\zt-boot.exe"))
        );
        assert_eq!(
            program_of(r"C:\zt\zt-boot.exe --config-dir C:\cfg"),
            Some(PathBuf::from(r"C:\zt\zt-boot.exe"))
        );
        assert_eq!(program_of("   "), None);
    }
}
