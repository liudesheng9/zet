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
#[allow(dead_code)]
pub fn print_last(value: &Value) -> Result<()> {
    print_line(&compact(value))
}

/// PowerShell's positional binding: a value that follows no parameter name goes to the next
/// parameter of `params` (in declaration order) that was not named. Returns the arguments with
/// each such value named, for `Args::parse`; a value left over is an error, as in PowerShell.
pub fn bind_positional(argv: &[String], params: &[&str]) -> Result<Vec<String>> {
    let is_name = |a: &str| a.len() > 1 && a.starts_with('-') && a[1..].chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    let mut named: Vec<String> = Vec::new();
    let mut positional: Vec<String> = Vec::new();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if is_name(arg) {
            let (name, inline) = match arg[1..].split_once(':') {
                Some((name, _)) => (name.to_ascii_lowercase(), true),
                None => (arg[1..].to_ascii_lowercase(), false),
            };
            out.push(arg.clone());
            if params.iter().any(|p| p.eq_ignore_ascii_case(&name)) && !inline && i + 1 < argv.len() {
                out.push(argv[i + 1].clone());
                i += 1;
            }
            named.push(name);
        } else {
            positional.push(arg.clone());
        }
        i += 1;
    }
    let mut free = params.iter().filter(|p| !named.contains(&p.to_ascii_lowercase()));
    for value in positional {
        match free.next() {
            Some(p) => {
                out.push(format!("-{p}"));
                out.push(value);
            }
            None => anyhow::bail!("a positional parameter cannot be found that accepts argument '{value}'"),
        }
    }
    Ok(out)
}
