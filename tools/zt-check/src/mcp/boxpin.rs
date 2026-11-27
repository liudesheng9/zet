//! The box pin. A digest run on a copy of the box (a box with its own zt service, named by its
//! `ZT_CONFIG_DIR`) names that box once, in the file `zt-box.txt` in its staging folder. Every
//! zt-digest and zt-check process looks for the pin above each absolute path argument and above its
//! working folder, before its first archive call, and points its `zt mcp` and every child at that
//! box. The relays and the DeepSeek jobs carry the staging folder in their arguments or run in it,
//! so no command of the run reaches the live box through a forgotten environment line, and a frozen
//! workflow script needs no change.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// The pin's file name; its first line is the box's `ZT_CONFIG_DIR`.
pub const PIN: &str = "zt-box.txt";

/// A pin found for this invocation: the pin file and the config folder it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub file: PathBuf,
    pub config: String,
}

/// Whether an argument is an absolute Windows path (`D:\…`, `D:/…`, `\\server\…`).
fn absolute(arg: &str) -> bool {
    let b = arg.as_bytes();
    (b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/'))
        || arg.starts_with("\\\\")
}

/// The pin above one path (the path itself when it is a folder, then each parent), if any.
fn pin_above(path: &Path) -> Result<Option<Pin>> {
    for dir in path.ancestors() {
        let file = dir.join(PIN);
        if file.is_file() {
            let text = std::fs::read_to_string(&file)?;
            let config = text.lines().next().unwrap_or_default().trim().to_string();
            if config.is_empty() {
                bail!("the box pin {} names no ZT_CONFIG_DIR", file.display());
            }
            return Ok(Some(Pin { file, config }));
        }
    }
    Ok(None)
}

/// Whether a flag name carries a filesystem path that can anchor a box pin.
fn is_path_flag(name: &str) -> bool {
    matches!(
        name.trim_start_matches('-').to_ascii_lowercase().as_str(),
        "staging" | "out" | "work" | "allow" | "log" | "boxpin" | "file" | "source"
            | "snapshot" | "report" | "json" | "draft" | "charts-json"
            | "batchesfile" | "unitsfile" | "figuresfile" | "plan" | "audit" | "moves"
            | "outputs" | "register" | "hub" | "review" | "fixwork" | "decisions"
            | "runreport" | "conceptreport" | "baseline" | "thesis" | "copy"
            | "secondplan" | "readings" | "tasksheet" | "instructions" | "runoptions"
            | "changes" | "applied" | "skeleton" | "coverage" | "scope" | "renewunits"
    )
}

/// Whether a flag takes TWO filesystem path values (`--draft-diff <orig> <draft>`).
fn is_two_path_flag(name: &str) -> bool {
    name.trim_start_matches('-').eq_ignore_ascii_case("draft-diff")
}

/// Non-path flags that consume one value argument (citation keys, model addresses, steps, etc.)
/// so their values are never inspected as paths (H2).
fn is_non_path_value_flag(name: &str) -> bool {
    matches!(
        name.trim_start_matches('-').to_ascii_lowercase().as_str(),
        "citationkey" | "model" | "models" | "step" | "occurrences" | "cards" | "round"
            | "token" | "tool" | "book" | "rules" | "setting" | "level" | "tree" | "stage"
            | "format" | "workspace" | "chapters" | "addresses" | "wait"
    )
}

/// Splits `-Name:value` or `-Name=value` at the first `:` or `=` after leading `-`.
fn split_inline_flag(arg: &str) -> Option<(&str, &str)> {
    let rest = arg.strip_prefix('-')?;
    let idx = match (rest.find(':'), rest.find('=')) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) => a,
        (None, Some(b)) => b,
        (None, None) => return None,
    };
    let name = &rest[..idx];
    let val = &rest[idx + 1..];
    if name.is_empty() || val.is_empty() {
        return None;
    }
    Some((name, val))
}

fn candidate_paths(argv: &[String], cwd: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let push_val = |out: &mut Vec<PathBuf>, raw: &str, allow_relative: bool| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        if absolute(trimmed) {
            out.push(PathBuf::from(trimmed));
        } else if allow_relative && let Some(base) = cwd {
            let joined = base.join(trimmed);
            out.push(std::fs::canonicalize(&joined).unwrap_or(joined));
        }
    };
    let mut i = 0;
    while i < argv.len() {
        let arg = &argv[i];
        if absolute(arg) {
            push_val(&mut out, arg, false);
        } else if let Some((name, val)) = split_inline_flag(arg) {
            if is_path_flag(name) {
                push_val(&mut out, val, true);
            } else if !is_non_path_value_flag(name) {
                push_val(&mut out, val, false);
            }
        } else if arg.starts_with('-') && is_two_path_flag(arg) {
            let mut consumed = 0;
            for offset in 1..=2 {
                if let Some(next) = argv.get(i + offset)
                    && !next.starts_with('-')
                {
                    push_val(&mut out, next, true);
                    consumed += 1;
                } else {
                    break;
                }
            }
            i += consumed;
        } else if arg.starts_with('-') && is_path_flag(arg) {
            // Only consume the next token if it is not itself another flag (e.g. `model -Report -Work rel`
            // uses `-Report` as a switch, so it must NOT swallow `-Work`).
            if let Some(next) = argv.get(i + 1)
                && !next.starts_with('-')
            {
                push_val(&mut out, next, true);
                i += 1;
            }
        } else if arg.starts_with('-') && is_non_path_value_flag(arg) {
            // Skip value of non-path flags (e.g. `-CitationKey K`) so a citation key matching a
            // subdirectory name can never select that subdirectory's box pin (H2).
            if let Some(next) = argv.get(i + 1)
                && !next.starts_with('-')
            {
                i += 1;
            }
        }
        i += 1;
    }
    if let Some(base) = cwd {
        out.push(base.to_path_buf());
    }
    out
}

/// The one pin above the path arguments (absolute, inline `-Flag:path`, or resolved relative
/// path-flag values) and the working folder, or None. Two pins that name different boxes are
/// an error: the invocation would not know which box it writes.
pub fn find(argv: &[String], cwd: Option<&Path>) -> Result<Option<Pin>> {
    let mut found: Option<Pin> = None;
    for path in candidate_paths(argv, cwd) {
        if let Some(pin) = pin_above(&path)? {
            match &found {
                Some(first) if !same(&first.config, &pin.config) => bail!(
                    "two box pins name different boxes: {} ({}) and {} ({})",
                    first.file.display(), first.config, pin.file.display(), pin.config
                ),
                Some(_) => {}
                None => found = Some(pin),
            }
        }
    }
    Ok(found)
}

pub fn same(a: &str, b: &str) -> bool {
    // One separator per step: a JSON-escaped command can name `D:\\box\\config` (live run 2026-10-26, grow5),
    // while preserving a leading UNC prefix `\\server\share`.
    let norm = |s: &str| {
        let t = s.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_ascii_lowercase();
        let (prefix, rest) = if let Some(stripped) = t.strip_prefix("\\\\") {
            ("\\\\", stripped)
        } else {
            ("", t.as_str())
        };
        let mut out = String::with_capacity(t.len());
        out.push_str(prefix);
        for c in rest.chars() {
            if !(c == '\\' && out.ends_with('\\')) {
                out.push(c);
            }
        }
        out
    };
    norm(a) == norm(b)
}

/// Points this process at the pinned box: sets `ZT_CONFIG_DIR` (and `ZT_GUI_PORT=0`, so a box
/// service never opens a GUI port) when a pin is found and the variable is unset. A variable that
/// names another box is an error. Call it first in `main`, while the process has one thread.
pub fn apply(argv: &[String]) -> Result<Option<Pin>> {
    let cwd = std::env::current_dir().ok();
    let Some(pin) = find(argv, cwd.as_deref())? else {
        return Ok(None);
    };
    match std::env::var("ZT_CONFIG_DIR") {
        Ok(set) if !set.trim().is_empty() => {
            if !same(&set, &pin.config) {
                bail!("the box pin {} names {}, but ZT_CONFIG_DIR is {}", pin.file.display(), pin.config, set);
            }
        }
        _ => {
            // SAFETY: called first in main, before any thread or child starts.
            unsafe { std::env::set_var("ZT_CONFIG_DIR", &pin.config) };
            if std::env::var_os("ZT_GUI_PORT").is_none() {
                unsafe { std::env::set_var("ZT_GUI_PORT", "0") };
            }
        }
    }
    Ok(Some(pin))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("zt-boxpin-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("cards").join("out")).unwrap();
        dir
    }

    fn s(p: &Path) -> String {
        p.display().to_string()
    }

    #[test]
    fn a_pin_in_the_staging_folder_is_found_from_a_nested_file_argument() {
        let out = scratch("nested");
        std::fs::write(out.join(PIN), "D:\\box\\config\r\n").unwrap();
        let log = out.join("cards").join("out").join("owned.txt");
        let argv = vec!["-".into(), "-Log".into(), s(&log)];
        let pin = find(&argv, None).unwrap().unwrap();
        assert_eq!(pin.config, "D:\\box\\config");
        assert_eq!(pin.file, out.join(PIN));
    }

    #[test]
    fn the_working_folder_finds_the_pin_when_no_argument_is_a_path() {
        let out = scratch("cwd");
        std::fs::write(out.join(PIN), "D:\\box\\config").unwrap();
        let argv = vec!["-Tool".into(), "get_card".into()];
        assert_eq!(find(&argv, Some(&out.join("cards"))).unwrap().unwrap().config, "D:\\box\\config");
    }

    #[test]
    fn a_config_named_with_doubled_separators_is_the_same_box() {
        // Live run 2026-10-26 (grow5): a JSON-escaped command set ZT_CONFIG_DIR to `D:\\zt-pilot-172\\config`.
        assert!(same(r"D:\\zt-pilot-172\\config", r"D:\zt-pilot-172\config"));
        assert!(same("D://box//config/", r"d:\box\config"));
        assert!(!same(r"D:\box\config", r"D:\box2\config"));
        assert!(same(r"\\server\share\\box", r"\\server\share\box"));
        assert!(!same(r"\\server\share", r"\server\share"));
    }

    #[test]
    fn no_pin_means_the_live_box() {
        let out = scratch("none");
        let argv = vec!["-Out".into(), s(&out), "relative\\path".into()];
        assert_eq!(find(&argv, Some(&out)).unwrap(), None);
    }

    #[test]
    fn two_pins_for_different_boxes_are_refused_and_an_empty_pin_too() {
        let a = scratch("two-a");
        let b = scratch("two-b");
        std::fs::write(a.join(PIN), "D:\\box-a\\config").unwrap();
        std::fs::write(b.join(PIN), "D:\\box-b\\config").unwrap();
        let argv = vec!["-Out".into(), s(&a), "-Allow".into(), s(&b.join("x.txt"))];
        assert!(find(&argv, None).unwrap_err().to_string().contains("different boxes"));
        std::fs::write(b.join(PIN), "D:/BOX-A/config/").unwrap();
        assert!(find(&argv, None).unwrap().is_some(), "the same box in another spelling is one pin");
        std::fs::write(b.join(PIN), "  \n").unwrap();
        assert!(find(&argv, None).unwrap_err().to_string().contains("names no ZT_CONFIG_DIR"));
    }

    #[test]
    fn finds_pin_from_inline_flag_and_relative_staging_path() {
        let root = scratch("rel-staging");
        let sub = root.join("stage-sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join(PIN), "D:\\copy-box\\config\n").unwrap();

        // Inline `-Staging:D:\...` and `-Staging=D:\...` must be discovered.
        let argv_colon = vec![format!("-Staging:{}", s(&sub))];
        assert_eq!(find(&argv_colon, None).unwrap().unwrap().config, "D:\\copy-box\\config");
        let argv_eq = vec![format!("-Staging={}", s(&sub))];
        assert_eq!(find(&argv_eq, None).unwrap().unwrap().config, "D:\\copy-box\\config");

        // Relative `-Staging stage-sub` resolved against cwd must be discovered.
        let argv_rel = vec!["-Step".into(), "relink".into(), "-Staging".into(), "stage-sub".into()];
        assert_eq!(find(&argv_rel, Some(&root)).unwrap().unwrap().config, "D:\\copy-box\\config");

        // H2: an ordinary non-path argument like `-CitationKey stage-sub` must NOT select
        // `stage-sub\zt-box.txt` even though a directory of that name exists under cwd.
        let argv_key = vec!["-Step".into(), "relink".into(), "-CitationKey".into(), "stage-sub".into()];
        assert_eq!(find(&argv_key, Some(&root)).unwrap(), None);
        let argv_pos = vec!["relink".into(), "stage-sub".into()];
        assert_eq!(find(&argv_pos, Some(&root)).unwrap(), None);

        // H2: a switch whose name also exists as a path flag on another tool (e.g. `model -Report -Work rel`)
        // must NOT swallow `-Work`, and `-RenewUnits` relative paths must be discovered.
        let argv_switch_before_work = vec!["-Report".into(), "-Work".into(), "stage-sub".into()];
        assert_eq!(
            find(&argv_switch_before_work, Some(&root)).unwrap().unwrap().config,
            "D:\\copy-box\\config"
        );
        let argv_renew = vec!["-RenewUnits".into(), "stage-sub\\units.json".into()];
        assert_eq!(
            find(&argv_renew, Some(&root)).unwrap().unwrap().config,
            "D:\\copy-box\\config"
        );
    }
}
