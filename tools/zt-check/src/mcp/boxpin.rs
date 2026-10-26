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

/// The one pin above the absolute path arguments and the working folder, or None. Two pins that
/// name different boxes are an error: the invocation would not know which box it writes.
pub fn find(argv: &[String], cwd: Option<&Path>) -> Result<Option<Pin>> {
    let mut found: Option<Pin> = None;
    let paths = argv.iter().filter(|a| absolute(a)).map(|a| PathBuf::from(a.trim())).chain(cwd.map(Path::to_path_buf));
    for path in paths {
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

fn same(a: &str, b: &str) -> bool {
    // One separator per step: a JSON-escaped command can name `D:\\box\\config` (live run 2026-10-26, grow5).
    let norm = |s: &str| {
        let t = s.trim().trim_end_matches(['\\', '/']).replace('/', "\\").to_ascii_lowercase();
        let mut out = String::with_capacity(t.len());
        for c in t.chars() {
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
}
