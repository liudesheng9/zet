//! What the ports of book-terms.ps1, book-skeleton.ps1, organize-survey.ps1, and
//! organize-apply.ps1 share: PSCustomObject reads, the manifest, the instruction files,
//! the audit in-process, `Invoke-Zt` reads, and writes through the helper.

use crate::audit;
use crate::calls;
use crate::ps;
use crate::util;
use crate::zt::Zt;
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static NULL: Value = Value::Null;
static OK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d+ ok ").unwrap());

/// `$object.name` on what `ConvertFrom-Json` (without -AsHashtable) made: a property name
/// ignores case. Anything that is not an object has no properties.
pub fn prop<'a>(v: &'a Value, name: &str) -> &'a Value {
    match v.as_object() {
        Some(map) => map.get(name).or_else(|| map.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, x)| x)).unwrap_or(&NULL),
        None => &NULL,
    }
}

/// `[string]$object.name` (a PSCustomObject).
pub fn pstr(v: &Value, name: &str) -> String {
    ps::s(prop(v, name))
}

/// A StringBuilder whose `AppendLine` ends lines with Environment.NewLine.
#[derive(Default)]
pub struct Sb(pub String);

impl Sb {
    pub fn new() -> Self {
        Self(String::new())
    }

    pub fn line(&mut self, text: impl AsRef<str>) -> &mut Self {
        self.0.push_str(text.as_ref());
        self.0.push_str(ps::NL);
        self
    }

    pub fn blank(&mut self) -> &mut Self {
        self.0.push_str(ps::NL);
        self
    }
}

/// `Get-Content -Raw -Encoding utf8 | ConvertFrom-Json` (with or without -AsHashtable: the
/// value is the same; the caller reads properties with `prop` or exact keys).
pub fn read_json(path: &str) -> Result<Value> {
    if !ps::exists(path) {
        bail!("Cannot find path '{path}' because it does not exist.");
    }
    Ok(ps::parse_json(&util::read_text(path)?)?)
}

/// `<Staging>\manifest.json`.
pub fn manifest(staging: &str) -> Result<Value> {
    read_json(&ps::join(staging, "manifest.json"))
}

/// A mandatory string parameter: given and not empty, as PowerShell's binding demands.
pub fn mandatory(a: &crate::args::Args, name: &str) -> Result<String> {
    let value = a.required(name)?;
    if value.is_empty() {
        bail!("Cannot bind argument to parameter '{name}' because it is an empty string.");
    }
    Ok(value)
}

/// The folder of the scripts and their instruction files ($PSScriptRoot): ZT_DIGEST_SCRIPTS
/// (the forwarders set it), else scripts\book-digest of the repository zt-digest was built from
/// (or of ZT_DIGEST_REPO).
pub fn scripts_dir() -> String {
    match std::env::var("ZT_DIGEST_SCRIPTS") {
        Ok(dir) if !dir.trim().is_empty() => dir.trim_end_matches(['\\', '/']).to_string(),
        _ => crate::model::Scripts::find().dir,
    }
}

/// `pwsh digest-audit.ps1 -CitationKey <key> -Out <out>` in-process, with the script's defaults.
pub fn run_audit(zt: &mut Zt, key: &str, out: &str) -> Result<()> {
    let params = audit::Params {
        orphan_targets: String::new(),
        citation_key: key.to_string(),
        out: out.to_string(),
        max_sentences: 12,
        max_chars: 400,
        max_steps: 6,
        sample: 15,
        skip_models: false,
    };
    if let Err(error) = audit::run(zt, &params, &mut |_| {}) {
        bail!("digest-audit failed: {error:#}");
    }
    Ok(())
}

/// `Invoke-Zt` with read calls (zt-calls.ps1's direct path): the answers that are JSON
/// objects; a failed call fails with `zt-mcp failed: <i> error <message>`.
pub fn reads(zt: &mut Zt, calls: &[(&str, Value)]) -> Result<Vec<Value>> {
    crate::model::read_calls(zt, calls)
}

/// `@(Invoke-Zt @(<one read>))[0]`.
pub fn read_first(zt: &mut Zt, tool: &str, arguments: Value) -> Result<Value> {
    Ok(reads(zt, &[(tool, arguments)])?.into_iter().next().unwrap_or(Value::Null))
}

/// The helper run on `calls` (`$calls | ConvertTo-Json -AsArray | pwsh zt-mcp.ps1 - ...`):
/// its output lines, and whether it exited 0. A dead `zt mcp` is an error.
pub fn helper(zt: &mut Zt, calls: &[Value], options: &mut calls::Options) -> Result<(Vec<String>, bool)> {
    let (done, failure) = calls::run(zt, calls, options)?;
    let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
    let ok = failure.is_none();
    if let Some(failure) = failure {
        lines.extend(failure.lines());
    }
    Ok((lines, ok))
}

/// A write batch as book-terms.ps1 and book-skeleton.ps1 ran it: on a failure, `zt: <every
/// line joined by ' | '>`. Returns the lines.
pub fn write_all(zt: &mut Zt, calls: &[Value]) -> Result<Vec<String>> {
    let (lines, ok) = match helper(zt, calls, &mut calls::Options::new()) {
        Ok(result) => result,
        Err(error) => bail!("zt: {error:#}"),
    };
    if !ok {
        bail!("zt: {}", lines.join(" | "));
    }
    Ok(lines)
}

/// `(@($lines | Where-Object { $_ -match '^\d+ ok ' })[0] -split ' ok ', 2)[1].Trim()`.
pub fn first_ok(lines: &[String]) -> String {
    lines
        .iter()
        .find(|l| OK_LINE.is_match(l))
        .and_then(|l| l.split_once(" ok "))
        .map(|(_, a)| a.trim().to_string())
        .unwrap_or_default()
}

/// One `zt mcp`, started by the first call that needs it (a step that reads nothing starts none).
pub struct Lazy {
    zt: Option<Zt>,
}

impl Lazy {
    pub fn new() -> Self {
        Self { zt: None }
    }

    pub fn get(&mut self) -> Result<&mut Zt> {
        if self.zt.is_none() {
            self.zt = Some(crate::zt::start()?);
        }
        Ok(self.zt.as_mut().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn properties_ignore_case_and_ok_lines_give_addresses() {
        let v = json!({ "Citation_Key": "K", "n": 2 });
        assert_eq!(pstr(&v, "citation_key"), "K");
        assert_eq!(pstr(&v, "N"), "2");
        assert!(prop(&json!("text"), "x").is_null());
        let lines = vec!["0 {\"a\":1}".to_string(), "1 ok K/1|a ".to_string(), "2 ok K/2".to_string()];
        assert_eq!(first_ok(&lines), "K/1|a");
        assert_eq!(first_ok(&[]), "");
    }
}
