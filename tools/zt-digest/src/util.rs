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

/// Complete UTF-8 evidence publication, preserving the old file on a refused replacement.
pub(crate) fn write_atomic(path: impl AsRef<Path>, text: &str) -> Result<()> {
    let path = path.as_ref();
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("create temporary artifact for {}", path.display()))?;
    temporary.write_all(text.as_bytes())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)
        .with_context(|| format!("atomically publish artifact {}", path.display()))?;
    Ok(())
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

/// The edit patience a write-capable step gives its child `zt mcp` (R269).
///
/// A digest apply waits on the archive edit lock while its own checker and receipt
/// replay run, so it needs far more patience than an ordinary agent write. Ordinary
/// reads and prechecks keep the short default: a busy archive fails fast there.
pub const WRITE_EDIT_PATIENCE_SECONDS: u64 = 180;

/// Whether this tool invocation can write Cards and so needs the long edit patience.
///
/// The test is the step or switch that performs the write, never the tool name alone:
/// every compiler tool also has read-only plan, check and report steps. The helper
/// (`call`) always writes; `organize-apply` and `figures` write; `prepare` writes only
/// with `-Apply`; a compiler step writes with `-Apply` or in one of the listed write
/// steps, and `group-finish`/`link-finish` apply their checked draft.
pub fn writes_cards(tool: &str, argv: &[String]) -> bool {
    let step = argv
        .iter()
        .position(|a| a.eq_ignore_ascii_case("-Step"))
        .and_then(|at| argv.get(at + 1))
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let apply = argv.iter().any(|a| {
        a.eq_ignore_ascii_case("-Apply") || a.eq_ignore_ascii_case("-Apply:$true") || a.eq_ignore_ascii_case("-Apply:true")
    });
    if matches!(tool, "call" | "organize-apply" | "figures" | "group-finish" | "link-finish") {
        return true;
    }
    if tool == "skeleton" && step == "apply" && !apply {
        // `skeleton -Step apply` without -Apply is the read-only draft precheck.
        return false;
    }
    apply
        || matches!(
            step.as_str(),
            "apply"
                | "explain-apply"
                | "notation-apply"
                | "boundary-apply"
                | "proof-apply"
                | "theory-apply"
                | "resume"
                | "split"
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_write_step_gets_long_edit_patience_and_a_read_step_keeps_the_short_bound() {
        // The helper and the two apply-only tools always write.
        assert!(writes_cards("call", &argv(&["-", "-Allow", "a.txt"])));
        assert!(writes_cards("organize-apply", &argv(&["-Work", "D:\\w"])));
        assert!(writes_cards("figures", &argv(&["-CitationKey", "K"])));
        assert!(writes_cards("group-finish", &argv(&["-Out", "D:\\o", "-Group", "1"])));
        // A compiler step writes only in an apply step or with -Apply.
        assert!(writes_cards("card", &argv(&["-Step", "apply", "-Work", "D:\\w"])));
        assert!(!writes_cards("card", &argv(&["-Step", "plan", "-Work", "D:\\w"])));
        assert!(!writes_cards("card", &argv(&["-Step", "precheck", "-Job", "j"])));
        assert!(writes_cards("prepare", &argv(&["-Source", "s.md", "-Out", "D:\\o", "-Apply"])));
        assert!(!writes_cards("prepare", &argv(&["-Source", "s.md", "-Out", "D:\\o"])));
        // The skeleton's read-only draft precheck names -Step apply without -Apply.
        assert!(!writes_cards("skeleton", &argv(&["-Step", "apply", "-Work", "D:\\w"])));
        assert!(writes_cards("skeleton", &argv(&["-Step", "apply", "-Work", "D:\\w", "-Apply"])));
        // Every write step of the model and math compilers.
        for step in ["apply", "explain-apply", "notation-apply", "boundary-apply", "proof-apply", "theory-apply", "resume", "split"] {
            assert!(writes_cards("model", &argv(&["-Step", step])), "{step}");
        }
        for step in ["brief", "plan", "report", "explain-check", "explain-report"] {
            assert!(!writes_cards("model", &argv(&["-Step", step])), "{step}");
        }
        assert_eq!(WRITE_EDIT_PATIENCE_SECONDS, 180);
    }
}