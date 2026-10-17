//! What the model scripts got from PowerShell and .NET, kept so the tools
//! behave as the scripts did: truthiness, `[string]` and `[int]`, `-eq` on
//! text (case and ignorable characters do not count), `@(...)` and pipeline
//! enumeration, hashtable keys that ignore case, .NET string lengths (UTF-16
//! code units), `Join-Path` and `Split-Path`, `ConvertTo-Json`, and the lines
//! of `Get-Content`.

use anyhow::{Context, Result, bail};
use regex::Regex;
use serde_json::Value;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::LazyLock;

/// What ICU collation (and so a culture string comparison) skips: control and
/// format characters, and variation selectors (as zt-check's `check.rs`).
static IGNORABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\x{0}-\x{8}\x{E}-\x{1F}\x{7F}-\x{84}\x{86}-\x{9F}\p{Cf}\x{FE00}-\x{FE0F}]").unwrap());
static MARK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\p{M}").unwrap());

// --- values -------------------------------------------------------------------------------------

/// `@($v)` and the pipeline: an array gives its elements, anything else
/// (null too) gives itself.
pub fn pipe(v: &Value) -> Vec<&Value> {
    match v {
        Value::Array(items) => items.iter().collect(),
        other => vec![other],
    }
}

/// `@($v | Where-Object { $null -ne $_ })`.
pub fn list(v: &Value) -> Vec<&Value> {
    pipe(v).into_iter().filter(|x| !x.is_null()).collect()
}

/// The `foreach` statement: null gives nothing.
pub fn each(v: &Value) -> Vec<&Value> {
    match v {
        Value::Null => Vec::new(),
        other => pipe(other),
    }
}

/// `@($v)[i]`: null when out of range.
pub fn at(items: &[&Value], i: usize) -> Value {
    items.get(i).map(|v| (*v).clone()).unwrap_or(Value::Null)
}

/// Whether PowerShell reads a value as true.
pub fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(flag) => *flag,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(text) => !text.is_empty(),
        Value::Array(items) => match items.as_slice() {
            [] => false,
            [one] => truthy(one),
            _ => true,
        },
        Value::Object(_) => true,
    }
}

/// `[string]$v`, and `"$v"` in a string.
pub fn s(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        Value::Bool(true) => "True".to_string(),
        Value::Bool(false) => "False".to_string(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else {
                let f = n.as_f64().unwrap_or_default();
                if f.fract() == 0.0 && f.abs() < 1e15 { format!("{}", f as i64) } else { format!("{f}") }
            }
        }
        Value::Array(items) => items.iter().map(s).collect::<Vec<_>>().join(" "),
        Value::Object(_) => "System.Collections.Hashtable".to_string(),
    }
}

/// `[int]$v`: null is 0, a fraction rounds half to even, text that is no
/// number fails as the cast does.
pub fn int(v: &Value) -> Result<i64> {
    Ok(match v {
        Value::Null => 0,
        Value::Bool(flag) => i64::from(*flag),
        Value::Number(n) => n.as_i64().unwrap_or_else(|| n.as_f64().unwrap_or_default().round_ties_even() as i64),
        Value::String(text) => {
            let t = text.trim();
            if t.is_empty() {
                0
            } else if let Ok(i) = t.parse::<i64>() {
                i
            } else if let Ok(f) = t.parse::<f64>() {
                f.round_ties_even() as i64
            } else {
                bail!("Cannot convert value \"{text}\" to type \"System.Int32\". Error: \"The input string '{text}' was not in a correct format.\"")
            }
        }
        other => bail!("Cannot convert value \"{}\" to type \"System.Int32\".", s(other)),
    })
}

/// `$v -eq $false` (a string compares as text with `False`, a number with 0).
pub fn is_false(v: &Value) -> bool {
    match v {
        Value::Bool(flag) => !flag,
        Value::Number(n) => n.as_f64() == Some(0.0),
        Value::String(text) => eq(text, "False"),
        _ => false,
    }
}

/// `$v -gt $n` with a number on the right: a number compares as a number,
/// text as text (culture order).
pub fn gt(v: &Value, n: i64) -> bool {
    match v {
        Value::Number(x) => x.as_f64().unwrap_or_default() > n as f64,
        Value::Bool(flag) => i64::from(*flag) > n,
        Value::String(text) => culture_cmp(text, &n.to_string()) == Ordering::Greater,
        _ => false,
    }
}

/// The object members as `$hashtable.Keys` lists them.
pub fn keys(v: &Value) -> Vec<String> {
    v.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default()
}

// --- text ---------------------------------------------------------------------------------------

/// The text without the characters a culture comparison ignores.
pub fn culture(text: &str) -> Cow<'_, str> {
    IGNORABLE.replace_all(text, "")
}

/// `-eq` on strings: case and ignorable characters do not count.
pub fn eq(a: &str, b: &str) -> bool {
    a == b || culture(a).to_lowercase() == culture(b).to_lowercase()
}

/// `-in` / `-contains` on strings.
pub fn is_in(value: &str, list: &[impl AsRef<str>]) -> bool {
    list.iter().any(|item| eq(value, item.as_ref()))
}

/// `[array]::IndexOf`: ordinal, -1 when absent.
pub fn index_of(list: &[String], value: &str) -> i64 {
    list.iter().position(|x| x == value).map_or(-1, |i| i as i64)
}

/// A hashtable key: `@{}` ignores case.
pub fn key(text: &str) -> String {
    text.to_lowercase()
}

/// `String.StartsWith(string)`: a culture comparison.
pub fn starts_with(text: &str, prefix: &str) -> bool {
    let text = culture(text);
    text.starts_with(prefix) && !MARK.is_match(&text[prefix.len()..])
}

/// The primary collation weight of a character (ICU root order for ASCII).
fn weight(ch: char) -> u32 {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    if ch.is_whitespace() {
        1
    } else if let Some(at) = PUNCTUATION.find(ch) {
        10 + at as u32
    } else if ch.is_ascii_digit() {
        100 + (ch as u32 - '0' as u32)
    } else if ch.is_ascii_lowercase() {
        200 + (ch as u32 - 'a' as u32)
    } else {
        1000 + ch as u32
    }
}

/// `Sort-Object` order on strings: the culture's collation, ignoring case.
pub fn culture_cmp(a: &str, b: &str) -> Ordering {
    let weights = |text: &str| culture(text).chars().flat_map(char::to_lowercase).map(weight).collect::<Vec<_>>();
    weights(a).cmp(&weights(b))
}

/// `-like`: `*`, `?`, and `[...]`, case ignored.
pub fn like(text: &str, pattern: &str) -> bool {
    let mut re = String::from("(?is)^");
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            '`' => {
                if let Some(next) = chars.next() {
                    re.push_str(&regex::escape(&next.to_string()));
                }
            }
            '[' => {
                let mut class = String::new();
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == ']' {
                        closed = true;
                        break;
                    }
                    class.push(d);
                }
                if closed && !class.is_empty() {
                    re.push('[');
                    for d in class.chars() {
                        if d == '\\' || d == '[' || d == '^' {
                            re.push('\\');
                        }
                        re.push(d);
                    }
                    re.push(']');
                } else {
                    return false;
                }
            }
            other => re.push_str(&regex::escape(&other.to_string())),
        }
    }
    re.push('$');
    Regex::new(&re).is_ok_and(|r| r.is_match(text))
}

/// A .NET string length: UTF-16 code units.
pub fn len16(text: &str) -> usize {
    text.chars().map(char::len_utf16).sum()
}

/// `$text.Substring(0, n)`. A cut inside a surrogate pair leaves half a
/// character, which a UTF-8 writer turns into U+FFFD.
pub fn cut16(text: &str, n: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.len_utf16();
        if used + w > n {
            if used < n {
                out.push('\u{FFFD}');
            }
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

/// `'{0:d2}' -f $n`.
pub fn d2(n: i64) -> String {
    format!("{n:02}")
}

// --- paths --------------------------------------------------------------------------------------

/// `Join-Path $a $b`: separators become `\`, one between the two parts.
pub fn join(a: &str, b: &str) -> String {
    let a = a.replace('/', "\\");
    let b = b.replace('/', "\\");
    format!("{}\\{}", a.trim_end_matches('\\'), b.trim_start_matches('\\'))
}

/// `Split-Path -Parent $p`: '' for a root or a bare name.
pub fn parent(p: &str) -> String {
    let p = p.replace('/', "\\");
    let trimmed = p.trim_end_matches('\\');
    let Some(at) = trimmed.rfind('\\') else { return String::new() };
    let head = &trimmed[..at];
    if head.is_empty() {
        "\\".to_string()
    } else if head.ends_with(':') {
        format!("{head}\\")
    } else {
        head.to_string()
    }
}

/// `[IO.Path]::IsPathRooted`.
pub fn rooted(p: &str) -> bool {
    p.starts_with('\\') || p.starts_with('/') || {
        let mut c = p.chars();
        c.next().is_some_and(|d| d.is_ascii_alphabetic()) && c.next() == Some(':')
    }
}

/// `[IO.Path]::ChangeExtension($p, $ext)`.
pub fn change_extension(p: &str, ext: &str) -> String {
    let name_at = p.rfind(['\\', '/']).map_or(0, |i| i + 1);
    let stem = match p[name_at..].rfind('.') {
        Some(dot) => &p[..name_at + dot],
        None => p,
    };
    format!("{stem}{ext}")
}

/// A path as `Get-ChildItem` names it in `FullName`.
pub fn full_name(p: &str) -> String {
    std::path::absolute(p).map(|a| a.display().to_string()).unwrap_or_else(|_| p.to_string()).replace('/', "\\")
}

/// The file names in `dir` that match `prefix*suffix` (case ignored), in the
/// order NTFS lists them.
pub fn child_files(dir: &str, prefix: &str, suffix: &str) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut names: Vec<String> = read
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| {
            let l = n.to_lowercase();
            l.len() >= prefix.len() + suffix.len() && l.starts_with(&prefix.to_lowercase()) && l.ends_with(&suffix.to_lowercase())
        })
        .collect();
    names.sort_by_key(|n| n.to_uppercase());
    names
}

/// `Test-Path -LiteralPath`.
pub fn exists(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).exists()
}

/// `Get-Content -Encoding utf8`: lines without their ends (`\r\n`, `\n`, `\r`).
pub fn read_lines(path: &str) -> Result<Vec<String>> {
    let bytes = std::fs::read(path).with_context(|| format!("failed to read {path}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    Ok(split_lines(text))
}

pub fn split_lines(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    let mut open = false;
    while let Some(c) = chars.next() {
        open = true;
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                lines.push(std::mem::take(&mut current));
                open = false;
            }
            '\n' => {
                lines.push(std::mem::take(&mut current));
                open = false;
            }
            other => current.push(other),
        }
    }
    if open {
        lines.push(current);
    }
    lines
}

// --- JSON ---------------------------------------------------------------------------------------

/// The characters Newtonsoft escapes that serde_json writes as they are.
fn newtonsoft(text: String) -> String {
    if !text.contains(['\u{85}', '\u{2028}', '\u{2029}']) {
        return text;
    }
    text.replace('\u{85}', "\\u0085").replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029")
}

/// `ConvertTo-Json -Compress`.
pub fn json_compact(v: &Value) -> String {
    newtonsoft(serde_json::to_string(v).unwrap_or_default())
}

/// `ConvertTo-Json`: two-space indent, CRLF line ends on Windows.
pub fn json_pretty(v: &Value) -> String {
    let text = newtonsoft(serde_json::to_string_pretty(v).unwrap_or_default());
    if cfg!(windows) { text.replace('\n', "\r\n") } else { text }
}

// --- hashtables ---------------------------------------------------------------------------------

/// An `@{}` or `[ordered]@{}` hashtable: keys ignore case, insertion order kept.
#[derive(Clone, Debug)]
pub struct CiMap<V> {
    items: Vec<(String, V)>,
}

impl<V> Default for CiMap<V> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<V> CiMap<V> {
    pub fn new() -> Self {
        Self::default()
    }

    fn find(&self, k: &str) -> Option<usize> {
        let wanted = key(k);
        self.items.iter().position(|(x, _)| x == k || key(x) == wanted)
    }

    pub fn get(&self, k: &str) -> Option<&V> {
        self.find(k).map(|i| &self.items[i].1)
    }

    pub fn contains(&self, k: &str) -> bool {
        self.find(k).is_some()
    }

    /// Sets a key; an existing key keeps its place and its first spelling.
    pub fn insert(&mut self, k: &str, v: V) {
        match self.find(k) {
            Some(i) => self.items[i].1 = v,
            None => self.items.push((k.to_string(), v)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &V)> {
        self.items.iter().map(|(k, v)| (k, v))
    }

    pub fn keys(&self) -> Vec<String> {
        self.items.iter().map(|(k, _)| k.clone()).collect()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn values_read_as_powershell_reads_them() {
        assert!(!truthy(&json!(null)) && !truthy(&json!("")) && !truthy(&json!([])) && !truthy(&json!([0])));
        assert!(truthy(&json!("0")) && truthy(&json!("false")) && truthy(&json!({})) && truthy(&json!([0, 0])));
        assert_eq!(s(&json!(["a", 1, null, true])), "a 1  True");
        assert_eq!(s(&json!(2.0)), "2");
        assert_eq!(int(&json!("2.5")).unwrap(), 2);
        assert_eq!(int(&json!(3.5)).unwrap(), 4);
        assert!(int(&json!("x")).is_err());
        assert_eq!(pipe(&json!(null)).len(), 1);
        assert_eq!(list(&json!(null)).len(), 0);
        assert_eq!(list(&json!(["a", null])).len(), 1);
        assert!(is_false(&json!(false)) && is_false(&json!("FALSE")) && !is_false(&json!(null)));
    }

    #[test]
    fn text_compares_and_cuts_as_dotnet() {
        assert!(eq("Formal", "formal") && eq("a\u{200B}b", "ab") && !eq("a", "b"));
        assert_eq!(len16("a😀b"), 4);
        assert_eq!(cut16("a😀b", 2), "a\u{FFFD}");
        assert_eq!(cut16("a😀b", 3), "a😀");
        assert!(like("Key/1", "key/*") && !like("Other/1", "Key/*"));
        let mut v = vec!["12", "3", "_x", "-y", "b", "B", "a", "A", "ab", "a-c"];
        v.sort_by(|a, b| culture_cmp(a, b));
        assert_eq!(v, ["_x", "-y", "12", "3", "a", "A", "a-c", "ab", "b", "B"]);
    }

    #[test]
    fn paths_join_and_split_as_powershell() {
        assert_eq!(join("C:\\a\\", "b"), "C:\\a\\b");
        assert_eq!(join("C:/a/", "b"), "C:\\a\\b");
        assert_eq!(join("rel", "b\\c"), "rel\\b\\c");
        assert_eq!(parent("C:\\a\\b"), "C:\\a");
        assert_eq!(parent("C:\\a"), "C:\\");
        assert_eq!(parent("C:\\"), "");
        assert_eq!(parent("work"), "");
        assert_eq!(parent("C:/x/y"), "C:\\x");
        assert_eq!(change_extension("D:\\w\\m\\job-f01.md", ".allow.txt"), "D:\\w\\m\\job-f01.allow.txt");
        assert!(rooted("C:\\x") && rooted("\\x") && !rooted("sections/s01.md"));
        assert_eq!(split_lines("a\r\nb\nc\rd\n"), ["a", "b", "c", "d"]);
        assert_eq!(split_lines("a\n\n"), ["a", ""]);
    }

    #[test]
    fn json_is_written_as_convertto_json() {
        let v = json!({ "a": [], "b": {}, "c": [1], "s": "x\u{2028}y", "d": 4.0 });
        assert_eq!(json_compact(&v), r#"{"a":[],"b":{},"c":[1],"s":"x\u2028y","d":4.0}"#);
        let pretty = json_pretty(&json!({ "c": [1] }));
        let nl = if cfg!(windows) { "\r\n" } else { "\n" };
        assert_eq!(pretty, format!("{{{nl}  \"c\": [{nl}    1{nl}  ]{nl}}}"));
    }

    #[test]
    fn hashtable_keys_ignore_case() {
        let mut m = CiMap::new();
        m.insert("Rests", 1);
        m.insert("rests", 2);
        assert_eq!(m.len(), 1);
        assert_eq!(m.get("RESTS"), Some(&2));
        assert_eq!(m.keys(), ["Rests"]);
    }
}
