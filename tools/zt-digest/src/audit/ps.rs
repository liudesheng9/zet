//! PowerShell and .NET semantics that the ported scripts relied on: JSON as
//! `ConvertTo-Json` writes it (Newtonsoft.Json), `Sort-Object` (an unstable
//! introsort with a culture-aware, case-insensitive comparer), `[Random]::new(seed)`,
//! string comparisons that ignore case, `[string]$value`, `[int]` rounding,
//! and the local time of `Get-Date`.

use serde_json::Value;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::LazyLock;

use regex::Regex;

/// The line end of `AppendLine`, `WriteAllLines`, and `ConvertTo-Json` (Environment.NewLine).
pub const NL: &str = if cfg!(windows) { "\r\n" } else { "\n" };

// --- JSON, as ConvertTo-Json writes it ----------------------------------------------------------

fn escape_into(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{85}' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// A double as .NET's round-trip format writes it, with Newtonsoft's `.0` for a whole number.
pub fn dotnet_double(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0".into() } else { "0.0".into() };
    }
    let exponent = f.abs().log10().floor() as i32;
    if !(-5..15).contains(&exponent) {
        // 1.2345678901234568E+17, 1E-07
        let sci = format!("{f:e}");
        let (mantissa, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
        let exp: i32 = exp.parse().unwrap_or(0);
        let sign = if exp < 0 { '-' } else { '+' };
        return format!("{mantissa}E{sign}{:02}", exp.abs());
    }
    let text = format!("{f}");
    if text.contains('.') { text } else { format!("{text}.0") }
}

fn number(n: &serde_json::Number) -> String {
    if n.is_f64() { dotnet_double(n.as_f64().unwrap_or(0.0)) } else { n.to_string() }
}

fn write_value(out: &mut String, value: &Value, pretty: bool, indent: usize) {
    let pad = |out: &mut String, n: usize| {
        for _ in 0..n {
            out.push_str("  ");
        }
    };
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number(n)),
        Value::String(s) => escape_into(out, s),
        Value::Array(list) => {
            if list.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in list.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push_str(NL);
                    pad(out, indent + 1);
                }
                write_value(out, item, pretty, indent + 1);
            }
            if pretty {
                out.push_str(NL);
                pad(out, indent);
            }
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if pretty {
                    out.push_str(NL);
                    pad(out, indent + 1);
                }
                escape_into(out, key);
                out.push(':');
                if pretty {
                    out.push(' ');
                }
                write_value(out, item, pretty, indent + 1);
            }
            if pretty {
                out.push_str(NL);
                pad(out, indent);
            }
            out.push('}');
        }
    }
}

/// `ConvertTo-Json` (indented, two spaces, Environment.NewLine).
pub fn json_pretty(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, true, 0);
    out
}

/// `ConvertTo-Json -Compress`.
pub fn json_compact(value: &Value) -> String {
    let mut out = String::new();
    write_value(&mut out, value, false, 0);
    out
}

/// JSON text as `ConvertFrom-Json` reads it: a BOM and surrounding white space are ignored;
/// an empty text is `$null`.
pub fn parse_json(text: &str) -> Result<Value, serde_json::Error> {
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(text)
}

// --- values as PowerShell reads them ------------------------------------------------------------

/// `[string]$value`: `$null` is empty, a number as .NET writes it, an array joined by spaces.
pub fn ps_str(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Number(n) => {
            if n.is_f64() {
                let f = n.as_f64().unwrap_or(0.0);
                let s = dotnet_double(f);
                s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
            } else {
                n.to_string()
            }
        }
        Value::String(s) => s.clone(),
        Value::Array(list) => list.iter().map(ps_str).collect::<Vec<_>>().join(" "),
        Value::Object(_) => "System.Collections.Hashtable".into(),
    }
}

/// `[string]$object.name`.
pub fn s(value: &Value, name: &str) -> String {
    ps_str(value.get(name).unwrap_or(&Value::Null))
}

/// A value in a boolean context.
pub fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(list) => match list.len() {
            0 => false,
            1 => truthy(&list[0]),
            _ => true,
        },
        Value::Object(_) => true,
    }
}

/// `@($value)`: an array stays as it is, `$null` is one `$null`, anything else one item.
pub fn ps_array(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(list) => list.clone(),
        other => vec![other.clone()],
    }
}

/// `@($value).Count`.
pub fn ps_count(value: &Value) -> usize {
    match value {
        Value::Array(list) => list.len(),
        _ => 1,
    }
}

/// The items a pipeline sees: an array's items, nothing for `$null`, else the value.
pub fn ps_items(value: &Value) -> Vec<Value> {
    match value {
        Value::Array(list) => list.clone(),
        Value::Null => Vec::new(),
        other => vec![other.clone()],
    }
}

/// `[int]` of a double: round half to even.
pub fn ps_int(f: f64) -> i64 {
    f.round_ties_even() as i64
}

/// The keys of an object (`.PSObject.Properties` names), in order.
pub fn keys(value: &Value) -> Vec<String> {
    value.as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default()
}

// --- string comparisons -------------------------------------------------------------------------

// What a culture comparison skips: control and format characters, variation selectors.
static IGNORABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\x{0}-\x{8}\x{E}-\x{1F}\x{7F}-\x{84}\x{86}-\x{9F}\p{Cf}\x{FE00}-\x{FE0F}]").unwrap());

fn culture(text: &str) -> Cow<'_, str> {
    IGNORABLE.replace_all(text, "")
}

/// `-eq` on strings: case does not count.
pub fn ps_eq(a: &str, b: &str) -> bool {
    a == b || culture(a).to_lowercase() == culture(b).to_lowercase()
}

/// `-in` / `-contains` on strings.
pub fn ps_in<S: AsRef<str>>(value: &str, list: &[S]) -> bool {
    list.iter().any(|item| ps_eq(value, item.as_ref()))
}

/// A key of `@{}`: hashtables ignore case.
pub fn ci(text: &str) -> String {
    text.to_lowercase()
}

/// `ToLowerInvariant`: one character for one character.
pub fn lower_invariant(text: &str) -> String {
    text.chars()
        .map(|c| {
            let mut lower = c.to_lowercase();
            match (lower.next(), lower.next()) {
                (Some(l), None) => l,
                _ => c,
            }
        })
        .collect()
}

/// The primary collation weight of a character in this machine's culture (zh-CN): white
/// space, punctuation and symbols in ICU root order, digits, Han, then Latin letters.
fn weight(ch: char) -> u32 {
    const PUNCTUATION: &str = "_-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$";
    if ch.is_whitespace() {
        1
    } else if let Some(at) = PUNCTUATION.find(ch) {
        10 + at as u32
    } else if ch.is_ascii_digit() {
        100 + (ch as u32 - '0' as u32)
    } else if ('\u{2E80}'..='\u{9FFF}').contains(&ch) || ('\u{F900}'..='\u{FAFF}').contains(&ch) || (ch as u32) >= 0x20000 {
        1000 + ch as u32
    } else if ch.is_ascii_lowercase() {
        0x100000 + (ch as u32 - 'a' as u32)
    } else {
        0x200000 + ch as u32
    }
}

/// `Sort-Object` / `Group-Object` order of two strings: the culture's collation, ignoring case.
pub fn culture_cmp(a: &str, b: &str) -> Ordering {
    let weights = |text: &str| culture(text).chars().flat_map(char::to_lowercase).map(weight).collect::<Vec<_>>();
    weights(a).cmp(&weights(b))
}

/// `-match '^R'`-style prefix test that ignores case.
pub fn starts_ci(text: &str, prefix: &str) -> bool {
    text.to_lowercase().starts_with(&prefix.to_lowercase())
}

/// `String.StartsWith(string)` (a culture comparison: ignorable characters do not count).
pub fn culture_starts_with(text: &str, prefix: &str) -> bool {
    culture(text).starts_with(prefix)
}

// --- Sort-Object -------------------------------------------------------------------------------

fn swap_if_greater<T>(keys: &mut [T], cmp: &impl Fn(&T, &T) -> Ordering, i: usize, j: usize) {
    if cmp(&keys[i], &keys[j]) == Ordering::Greater {
        keys.swap(i, j);
    }
}

fn insertion_sort<T: Clone>(keys: &mut [T], cmp: &impl Fn(&T, &T) -> Ordering) {
    for i in 0..keys.len().saturating_sub(1) {
        let t = keys[i + 1].clone();
        let mut j = i as isize;
        while j >= 0 && cmp(&t, &keys[j as usize]) == Ordering::Less {
            keys[j as usize + 1] = keys[j as usize].clone();
            j -= 1;
        }
        keys[(j + 1) as usize] = t;
    }
}

fn down_heap<T: Clone>(keys: &mut [T], mut i: usize, n: usize, cmp: &impl Fn(&T, &T) -> Ordering) {
    let d = keys[i - 1].clone();
    while i <= n >> 1 {
        let mut child = 2 * i;
        if child < n && cmp(&keys[child - 1], &keys[child]) == Ordering::Less {
            child += 1;
        }
        if cmp(&d, &keys[child - 1]) != Ordering::Less {
            break;
        }
        keys[i - 1] = keys[child - 1].clone();
        i = child;
    }
    keys[i - 1] = d;
}

fn heap_sort<T: Clone>(keys: &mut [T], cmp: &impl Fn(&T, &T) -> Ordering) {
    let n = keys.len();
    for i in (1..=n >> 1).rev() {
        down_heap(keys, i, n, cmp);
    }
    for i in (2..=n).rev() {
        keys.swap(0, i - 1);
        down_heap(keys, 1, i - 1, cmp);
    }
}

fn pick_pivot_and_partition<T: Clone>(keys: &mut [T], cmp: &impl Fn(&T, &T) -> Ordering) -> usize {
    let hi = keys.len() - 1;
    let middle = hi >> 1;
    swap_if_greater(keys, cmp, 0, middle);
    swap_if_greater(keys, cmp, 0, hi);
    swap_if_greater(keys, cmp, middle, hi);
    let pivot = keys[middle].clone();
    keys.swap(middle, hi - 1);
    let (mut left, mut right) = (0usize, hi - 1);
    while left < right {
        loop {
            left += 1;
            if cmp(&keys[left], &pivot) != Ordering::Less {
                break;
            }
        }
        loop {
            right -= 1;
            if cmp(&pivot, &keys[right]) != Ordering::Less {
                break;
            }
        }
        if left >= right {
            break;
        }
        keys.swap(left, right);
    }
    if left != hi - 1 {
        keys.swap(left, hi - 1);
    }
    left
}

fn intro_sort<T: Clone>(keys: &mut [T], mut depth_limit: u32, cmp: &impl Fn(&T, &T) -> Ordering) {
    let mut size = keys.len();
    while size > 1 {
        if size <= 16 {
            match size {
                2 => swap_if_greater(keys, cmp, 0, 1),
                3 => {
                    swap_if_greater(keys, cmp, 0, 1);
                    swap_if_greater(keys, cmp, 0, 2);
                    swap_if_greater(keys, cmp, 1, 2);
                }
                _ => insertion_sort(&mut keys[..size], cmp),
            }
            return;
        }
        if depth_limit == 0 {
            heap_sort(&mut keys[..size], cmp);
            return;
        }
        depth_limit -= 1;
        let p = pick_pivot_and_partition(&mut keys[..size], cmp);
        intro_sort(&mut keys[p + 1..size], depth_limit, cmp);
        size = p;
    }
}

/// `Sort-Object`: .NET's `List<T>.Sort` (an introsort, not stable above 16 items).
pub fn ps_sort<T: Clone>(items: &mut [T], cmp: impl Fn(&T, &T) -> Ordering) {
    if items.len() > 1 {
        let depth = 2 * ((items.len() as u32).ilog2() + 1);
        intro_sort(items, depth, &cmp);
    }
}

/// `Sort-Object -Unique` on strings: sorted, then equal neighbours (case ignored) dropped.
pub fn ps_sort_unique(items: &[String]) -> Vec<String> {
    let mut sorted = items.to_vec();
    ps_sort(&mut sorted, |a, b| culture_cmp(a, b));
    let mut out: Vec<String> = Vec::new();
    for item in sorted {
        if out.last().is_none_or(|last| culture_cmp(last, &item) != Ordering::Equal) {
            out.push(item);
        }
    }
    out
}

/// `Select-Object -Unique` on strings: the first of each, case counts.
pub fn select_unique(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

/// `Group-Object` on string keys: one group per key (case ignored), named by the first,
/// in sorted order, each group's items in input order.
pub fn group_by<T: Clone>(items: &[T], key: impl Fn(&T) -> String) -> Vec<(String, Vec<T>)> {
    let mut groups: Vec<(String, Vec<T>)> = Vec::new();
    for item in items {
        let k = key(item);
        match groups.iter_mut().find(|(name, _)| culture_cmp(name, &k) == Ordering::Equal) {
            Some((_, list)) => list.push(item.clone()),
            None => groups.push((k, vec![item.clone()])),
        }
    }
    groups.sort_by(|a, b| culture_cmp(&a.0, &b.0));
    groups
}

// --- [Random]::new(seed) -----------------------------------------------------------------------

/// .NET's seeded `System.Random` (Knuth's subtractive generator, the Net5Compat implementation).
pub struct DotNetRandom {
    seed_array: [i32; 56],
    inext: usize,
    inextp: usize,
}

impl DotNetRandom {
    pub fn new(seed: i32) -> Self {
        const MBIG: i32 = i32::MAX;
        const MSEED: i32 = 161_803_398;
        let mut seed_array = [0i32; 56];
        let subtraction = if seed == i32::MIN { i32::MAX } else { seed.abs() };
        let mut mj = MSEED - subtraction;
        seed_array[55] = mj;
        let mut mk = 1i32;
        let mut ii = 0usize;
        for _ in 1..55 {
            ii += 21;
            if ii >= 55 {
                ii -= 55;
            }
            seed_array[ii] = mk;
            mk = mj - mk;
            if mk < 0 {
                mk += MBIG;
            }
            mj = seed_array[ii];
        }
        for _ in 1..5 {
            for i in 1..56 {
                let mut n = i + 30;
                if n >= 55 {
                    n -= 55;
                }
                seed_array[i] = seed_array[i].wrapping_sub(seed_array[1 + n]);
                if seed_array[i] < 0 {
                    seed_array[i] += MBIG;
                }
            }
        }
        Self { seed_array, inext: 0, inextp: 21 }
    }

    /// `Next()`: an int in [0, int.MaxValue).
    pub fn next(&mut self) -> i32 {
        let mut a = self.inext + 1;
        if a >= 56 {
            a = 1;
        }
        let mut b = self.inextp + 1;
        if b >= 56 {
            b = 1;
        }
        let mut value = self.seed_array[a].wrapping_sub(self.seed_array[b]);
        if value == i32::MAX {
            value -= 1;
        }
        if value < 0 {
            value += i32::MAX;
        }
        self.seed_array[a] = value;
        self.inext = a;
        self.inextp = b;
        value
    }
}

// --- paths and time ----------------------------------------------------------------------------

/// `Join-Path a b`.
pub fn join_path(a: &str, b: &str) -> String {
    if a.is_empty() {
        return b.to_string();
    }
    let a_sep = a.ends_with('\\') || a.ends_with('/');
    let b_sep = b.starts_with('\\') || b.starts_with('/');
    let sep = if cfg!(windows) { "\\" } else { "/" };
    match (a_sep, b_sep) {
        (true, true) => format!("{a}{}", &b[1..]),
        (true, false) | (false, true) => format!("{a}{b}"),
        (false, false) => format!("{a}{sep}{b}"),
    }
}

/// `Split-Path -Leaf`.
pub fn leaf(path: &str) -> String {
    path.trim_end_matches(['\\', '/']).rsplit(['\\', '/']).next().unwrap_or_default().to_string()
}

/// `(Get-Date).ToString('yyyy-MM-dd HH:mm')` in local time.
pub fn local_minute() -> String {
    #[cfg(windows)]
    {
        #[repr(C)]
        #[derive(Default)]
        struct SystemTime {
            year: u16,
            month: u16,
            day_of_week: u16,
            day: u16,
            hour: u16,
            minute: u16,
            second: u16,
            milliseconds: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(time: *mut SystemTime);
        }
        let mut t = SystemTime::default();
        // SAFETY: GetLocalTime fills the struct it is given.
        unsafe { GetLocalTime(&mut t) };
        format!("{:04}-{:02}-{:02} {:02}:{:02}", t.year, t.month, t.day, t.hour, t.minute)
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
        let days = secs.div_euclid(86_400);
        let rem = secs.rem_euclid(86_400);
        // Civil date from days since 1970-01-01 (UTC).
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
        format!("{year:04}-{month:02}-{day:02} {:02}:{:02}", rem / 3600, rem % 3600 / 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn random_matches_dotnet() {
        let mut r = DotNetRandom::new(1);
        let got: Vec<i32> = (0..5).map(|_| r.next()).collect();
        assert_eq!(got, vec![534011718, 237820880, 1002897798, 1657007234, 1412011072]);
        let mut r = DotNetRandom::new(42);
        let got: Vec<i32> = (0..3).map(|_| r.next()).collect();
        assert_eq!(got, vec![1434747710, 302596119, 269548474]);
        assert_eq!(DotNetRandom::new(0).next(), 1559595546);
    }

    #[test]
    fn json_as_convertto_json_writes_it() {
        let v = json!({ "a": "x\ny<>&'é\u{2028}\u{7f}\u{1f}", "b": 1.0, "c": 0.1, "e": true, "f": 5, "x": [], "y": {}, "z": null, "w": [1] });
        assert_eq!(
            json_compact(&v),
            "{\"a\":\"x\\ny<>&'é\\u2028\u{7f}\\u001f\",\"b\":1.0,\"c\":0.1,\"e\":true,\"f\":5,\"x\":[],\"y\":{},\"z\":null,\"w\":[1]}"
        );
        let pretty = json_pretty(&json!({ "x": [], "w": [1], "o": { "k": null } }));
        assert_eq!(pretty, ["{", "  \"x\": [],", "  \"w\": [", "    1", "  ],", "  \"o\": {", "    \"k\": null", "  }", "}"].join(NL));
        assert_eq!(dotnet_double(1e20), "1E+20");
        assert_eq!(dotnet_double(1e-7), "1E-07");
        assert_eq!(dotnet_double(123456789012345678.0), "1.2345678901234568E+17");
        assert_eq!(dotnet_double(0.30000000000000004), "0.30000000000000004");
    }

    #[test]
    fn culture_order_as_sort_object() {
        let mut keys: Vec<String> =
            ["Key/10", "Key/2", "key/1", "0/1|a", "0/1", "0/10", "0/1|b|a", "0/2", "甲", "a-b", "a_b", "ab"].iter().map(|s| s.to_string()).collect();
        ps_sort(&mut keys, |a, b| culture_cmp(a, b));
        assert_eq!(keys.join(" "), "0/1 0/1|a 0/1|b|a 0/10 0/2 甲 a_b a-b ab key/1 Key/10 Key/2");
        let groups = group_by(&["R2", "R10", "I1", "E1", "L2", "N1", "R2"].map(String::from), |s| s.clone());
        assert_eq!(groups.iter().map(|g| g.0.as_str()).collect::<Vec<_>>(), vec!["E1", "I1", "L2", "N1", "R10", "R2"]);
        assert_eq!(ps_sort_unique(&["b", "A", "a", "B"].map(String::from)), vec!["A", "b"]);
    }

    #[test]
    fn sort_is_the_dotnet_introsort() {
        // 200 items keyed i % 3, sorted by key as Sort-Object sorts them (pwsh 7.6): the order of
        // equal keys is .NET's introsort order, not the input order.
        let order = |items: &[(i32, i32)]| items.iter().map(|x| x.1.to_string()).collect::<Vec<_>>().join(",");
        let mut items: Vec<(i32, i32)> = (1..=200).map(|i| (i % 3, i)).collect();
        ps_sort(&mut items, |a, b| a.0.cmp(&b.0));
        assert_eq!(order(&items), "150,66,135,63,138,60,141,57,132,144,147,51,102,48,153,45,156,54,42,69,72,96,105,93,108,90,111,87,129,114,117,81,120,78,123,75,126,84,159,99,39,183,18,12,180,21,189,177,24,9,192,174,186,171,27,30,162,198,36,3,165,6,15,33,168,195,115,106,109,118,112,103,187,193,121,196,190,130,184,157,163,154,166,151,148,169,145,124,172,139,175,136,178,133,160,181,127,142,1,100,70,73,19,22,67,64,25,61,28,76,58,31,52,199,49,34,46,37,43,55,16,40,79,97,4,94,91,7,10,85,88,13,82,38,2,14,164,197,35,17,182,5,167,194,32,188,170,29,8,185,173,191,26,20,161,176,23,11,179,101,50,158,125,77,122,80,119,83,116,74,86,89,110,92,107,95,104,98,113,41,128,131,44,155,47,152,149,53,146,71,56,59,140,62,137,65,134,68,143,200");
        let mut items: Vec<(i32, i32)> = (1..=200).map(|i| (i % 3, i)).collect();
        ps_sort(&mut items, |a, b| b.0.cmp(&a.0));
        assert_eq!(order(&items), "200,134,65,137,62,140,59,143,68,56,53,149,98,152,47,155,44,146,158,131,128,104,95,107,92,110,89,113,71,86,83,119,80,122,77,125,74,116,41,50,161,23,179,20,182,17,185,14,188,11,191,8,194,5,197,2,176,26,101,164,173,38,167,32,29,35,170,190,115,154,112,142,166,109,163,196,106,157,103,193,118,121,187,139,175,172,136,178,133,151,145,181,169,127,148,184,124,160,130,100,1,73,19,22,67,64,25,61,28,76,58,31,52,199,49,34,46,37,43,55,16,70,79,97,4,94,91,7,10,85,88,13,82,40,24,162,198,186,36,3,165,183,18,195,33,6,168,21,189,30,171,192,15,27,9,174,180,39,177,12,150,42,75,123,78,120,81,117,84,126,114,111,90,108,93,105,96,102,87,159,72,69,156,45,153,48,51,147,54,129,144,141,60,138,63,135,66,132,57,99");
        let mut items: Vec<(String, i32)> = (1..=40).map(|i| (format!("a/{}", i % 4), i)).collect();
        ps_sort(&mut items, |a, b| culture_cmp(&a.0, &b.0));
        assert_eq!(items.iter().map(|x| x.1.to_string()).collect::<Vec<_>>().join(","), "20,36,32,28,24,16,12,40,4,8,9,37,33,29,25,21,5,1,17,13,38,10,2,34,30,18,14,22,6,26,27,15,23,31,3,11,35,39,19,7");
    }

    #[test]
    fn paths() {
        assert_eq!(leaf("D:\\a\\b\\fix-01.md"), "fix-01.md");
        assert_eq!(join_path("D:\\a\\", "b"), "D:\\a\\b");
        assert_eq!(join_path("D:\\a", "b"), if cfg!(windows) { "D:\\a\\b" } else { "D:\\a/b" });
    }
}
