//! Files and output, as the PowerShell scripts wrote them: UTF-8 without a
//! BOM, CRLF line ends on the console on Windows, and compact JSON.

use anyhow::{Context, Result};
use serde_json::Value;
use std::io::Write;
use std::path::Path;

/// A JSON file (a BOM is ignored).
pub fn read_json(path: impl AsRef<Path>) -> Result<Value> {
    let path = path.as_ref();
    let raw = read_text(path)?;
    serde_json::from_str(&raw).with_context(|| format!("{} is not JSON", path.display()))
}

/// A UTF-8 text file without its BOM.
pub fn read_text(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let raw = std::fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(raw.strip_prefix('\u{FEFF}').map(str::to_string).unwrap_or(raw))
}

/// Writes UTF-8 without a BOM, making the parent folder first.
pub fn write_text(path: impl AsRef<Path>, text: &str) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("failed to make {}", parent.display()))?;
    }
    std::fs::write(path, text).with_context(|| format!("failed to write {}", path.display()))
}

/// Appends UTF-8 text (the file is made when missing).
pub fn append_text(path: impl AsRef<Path>, text: &str) -> Result<()> {
    let path = path.as_ref();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    file.write_all(text.as_bytes())?;
    Ok(())
}

/// Compact JSON, as `ConvertTo-Json -Compress` writes it.
pub fn compact(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

/// Prints lines in UTF-8 whatever the console code page, with the line ends
/// PowerShell writes (CRLF on Windows).
pub fn print_lines(lines: &[String]) -> Result<()> {
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    let mut text = String::new();
    for line in lines {
        text.push_str(line);
        text.push_str(newline);
    }
    let mut stdout = std::io::stdout().lock();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()?;
    Ok(())
}

pub fn print_line(line: &str) -> Result<()> {
    print_lines(&[line.to_string()])
}

/// The last line of a compiler step: the step's result as compact JSON.
pub fn print_last(value: &Value) -> Result<()> {
    print_line(&compact(value))
}
