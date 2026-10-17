//! The pure helpers of the Bridge compiler: the canonical JSON and SHA-256 fingerprints
//! (`Canonical-Json`, `Hash-Value`), `Read-Json` (strict JSON, no repeated key), the option
//! bounds (`Integer-Option`), ordinal and case-sensitive culture order, and the UTC time stamp.

use crate::ps;
use anyhow::Result;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::HashSet;

// --- errors ---------------------------------------------------------------------------------------

/// An `[ArgumentException]` of the script: a refusal of the input (exit code 1). Any other
/// error is an operational failure (exit code 2).
#[derive(Debug)]
pub struct Refuse(pub String);

impl std::fmt::Display for Refuse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refuse {}

pub fn refuse(message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(Refuse(message.into()))
}

pub fn is_refusal(error: &anyhow::Error) -> bool {
    error.downcast_ref::<Refuse>().is_some()
}

// --- SHA-256 --------------------------------------------------------------------------------------

const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
    0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
    0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
    0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
    0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// SHA-256 of `data`, as lower-case hex (`[Convert]::ToHexStringLower(SHA256.HashData(...))`).
pub fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bits.to_be_bytes());
    for block in message.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (slot, add) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *slot = slot.wrapping_add(add);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

// --- canonical JSON -------------------------------------------------------------------------------

/// `[StringComparer]::Ordinal`: UTF-16 code unit order.
pub fn ordinal_cmp(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

/// `Canonical-Value`: every object with its keys in ordinal order.
pub fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_by(|a, b| ordinal_cmp(a, b));
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), canonical(&map[key]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

/// `Canonical-Json`: `ConvertTo-Json -Compress` of the canonical value.
pub fn canonical_json(value: &Value) -> String {
    ps::json_compact(&canonical(value))
}

/// `Hash-Value`: SHA-256 of the UTF-8 canonical JSON.
pub fn hash_value(value: &Value) -> String {
    sha256_hex(canonical_json(value).as_bytes())
}

/// `Content-Hash`: the hash of an object without the keys in `except`.
pub fn content_hash(value: &Value, except: &[&str]) -> String {
    let mut copy = Map::new();
    if let Some(map) = value.as_object() {
        for (k, v) in map {
            if !except.contains(&k.as_str()) {
                copy.insert(k.clone(), v.clone());
            }
        }
    }
    hash_value(&Value::Object(copy))
}

// --- Read-Json ------------------------------------------------------------------------------------

/// The first object key that repeats (keys compared after unescaping), in document order.
/// `text` is valid JSON.
fn repeated_key(text: &str) -> Option<String> {
    struct Scan<'a> {
        b: &'a [u8],
        i: usize,
        text: &'a str,
    }
    impl Scan<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n') {
                self.i += 1;
            }
        }
        fn string(&mut self) -> String {
            let start = self.i;
            self.i += 1;
            while self.i < self.b.len() {
                match self.b[self.i] {
                    b'\\' => self.i += 2,
                    b'"' => {
                        self.i += 1;
                        break;
                    }
                    _ => self.i += 1,
                }
            }
            serde_json::from_str::<String>(&self.text[start..self.i]).unwrap_or_default()
        }
        fn value(&mut self) -> Option<String> {
            self.ws();
            match self.b.get(self.i) {
                Some(b'{') => {
                    self.i += 1;
                    let mut keys = HashSet::new();
                    loop {
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b'}') => {
                                self.i += 1;
                                return None;
                            }
                            Some(b',') => self.i += 1,
                            Some(b'"') => {
                                let key = self.string();
                                if !keys.insert(key.clone()) {
                                    return Some(key);
                                }
                                self.ws();
                                self.i += 1; // ':'
                                if let Some(found) = self.value() {
                                    return Some(found);
                                }
                            }
                            _ => return None,
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    loop {
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b']') => {
                                self.i += 1;
                                return None;
                            }
                            Some(b',') => self.i += 1,
                            None => return None,
                            _ => {
                                if let Some(found) = self.value() {
                                    return Some(found);
                                }
                            }
                        }
                    }
                }
                Some(b'"') => {
                    self.string();
                    None
                }
                Some(_) => {
                    while self.i < self.b.len() && !matches!(self.b[self.i], b',' | b']' | b'}' | b' ' | b'\t' | b'\r' | b'\n') {
                        self.i += 1;
                    }
                    None
                }
                None => None,
            }
        }
    }
    Scan { b: text.as_bytes(), i: 0, text }.value()
}

/// `Read-Json`: the file as JSON (a BOM is ignored). A missing file fails as Get-Content did;
/// text that is not strict JSON, or that repeats a key in one object, is a refusal
/// `invalid JSON: …`.
pub fn read_json(path: &str) -> Result<Value> {
    if !std::path::Path::new(path).is_file() {
        anyhow::bail!("Cannot find path '{path}' because it does not exist.");
    }
    let bytes = std::fs::read(path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    if text.is_empty() {
        return Err(refuse("invalid JSON: Value cannot be null. (Parameter 'json')"));
    }
    let value: Value = serde_json::from_str(text).map_err(|e| refuse(format!("invalid JSON: {e}")))?;
    if let Some(key) = repeated_key(text) {
        return Err(refuse(format!("invalid JSON: duplicate JSON key {key}")));
    }
    Ok(value)
}

/// `Save-Json`: `ConvertTo-Json` text (UTF-8, no BOM) through a temporary file and a move.
pub fn save_json(path: &str, value: &Value) -> Result<()> {
    let temp = format!("{path}.{}.tmp", std::process::id());
    std::fs::write(&temp, ps::json_pretty(value)).map_err(|e| anyhow::anyhow!("{e}: {temp}"))?;
    std::fs::rename(&temp, path).map_err(|e| anyhow::anyhow!("{e}: {path}"))?;
    Ok(())
}

// --- options and order ----------------------------------------------------------------------------

/// `Integer-Option`: a whole number in `low..=high` (an invariant-culture float that is whole).
pub fn integer_option(value: &str, name: &str, low: i64, high: i64) -> Result<i64> {
    let number = value.trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{B}' | '\u{C}' | '\r')).parse::<f64>().ok();
    match number {
        Some(n) if n.is_finite() && n.floor() == n && n >= low as f64 && n <= high as f64 => Ok(n as i64),
        _ => Err(refuse(format!("{name} must be an integer in {low}..{high}"))),
    }
}

/// `Sort-Object -CaseSensitive` order of two strings: the culture order, lower case first
/// where two strings differ only in case.
pub fn culture_cmp_cs(a: &str, b: &str) -> Ordering {
    match ps::culture_cmp(a, b) {
        Ordering::Equal => {
            let (x, y) = (ps::culture(a), ps::culture(b));
            for (p, q) in x.chars().zip(y.chars()) {
                if p != q {
                    return match (p.is_lowercase() && q.is_uppercase(), p.is_uppercase() && q.is_lowercase()) {
                        (true, _) => Ordering::Less,
                        (_, true) => Ordering::Greater,
                        _ => p.cmp(&q),
                    };
                }
            }
            x.chars().count().cmp(&y.chars().count())
        }
        other => other,
    }
}

/// `Sort-Object -Unique -CaseSensitive` on strings.
pub fn sort_unique_cs(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut sorted: Vec<String> = items.into_iter().collect();
    ps::ps_sort(&mut sorted, |a, b| culture_cmp_cs(a, b));
    let mut out: Vec<String> = Vec::new();
    for item in sorted {
        if out.last().is_none_or(|last| culture_cmp_cs(last, &item) != Ordering::Equal) {
            out.push(item);
        }
    }
    out
}

/// One character's simple upper case, as `OrdinalIgnoreCase` folds it.
fn fold(c: char) -> char {
    let mut upper = c.to_uppercase();
    match (upper.next(), upper.next()) {
        (Some(u), None) if u.len_utf16() == c.len_utf16() => u,
        _ => c,
    }
}

/// `$text.IndexOf($term, [StringComparison]::OrdinalIgnoreCase) -ge 0`.
pub fn contains_ignore_case(text: &str, term: &str) -> bool {
    let text: String = text.chars().map(fold).collect();
    let term: String = term.chars().map(fold).collect();
    text.contains(&term)
}

/// `[DateTime]::UtcNow.ToString('o')`: `2026-10-16T12:34:56.1234567Z`.
pub fn utc_now_o() -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = now.as_secs() as i64;
    let ticks = now.subsec_nanos() / 100;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
    format!("{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{ticks:07}Z", rem / 3600, rem % 3600 / 60, rem % 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    #[test]
    fn canonical_json_sorts_keys_ordinally() {
        let v = json!({ "b": [{ "z": 1, "a": null }], "a": "x", "B": true, "\u{e000}": 1, "\u{1F600}": 2 });
        // UTF-16 order: a surrogate (D83D) sorts before U+E000.
        assert_eq!(canonical_json(&v), "{\"B\":true,\"a\":\"x\",\"b\":[{\"a\":null,\"z\":1}],\"\u{1F600}\":2,\"\u{e000}\":1}");
        assert_eq!(content_hash(&json!({ "a": 1, "report_id": "r", "created_at": "t" }), &["report_id", "created_at"]), hash_value(&json!({ "a": 1 })));
    }

    #[test]
    fn repeated_keys_are_found_after_unescaping() {
        assert_eq!(repeated_key(r#"{"a":1,"b":{"c":[1,{"d":2,"d":3}]}}"#).as_deref(), Some("d"));
        assert_eq!(repeated_key(r#"{"a":1,"a":2}"#).as_deref(), Some("a"));
        assert_eq!(repeated_key(r#"[{"a":"}\"{"},{"a":1}]"#), None);
    }

    #[test]
    fn integer_options_and_order() {
        assert_eq!(integer_option("60", "MaxSources", 1, 120).unwrap(), 60);
        assert_eq!(integer_option(" 1e2 ", "MaxSources", 1, 120).unwrap(), 100);
        for bad in ["0", "121", "1.5", "", "x", "Infinity", "NaN"] {
            assert_eq!(integer_option(bad, "MaxSources", 1, 120).unwrap_err().to_string(), "MaxSources must be an integer in 1..120");
        }
        assert_eq!(sort_unique_cs(["b", "B", "a", "A", "b"].map(String::from)), vec!["a", "A", "b", "B"]);
        assert!(contains_ignore_case("Notation of X", "NOTATION") && !contains_ignore_case("控制权", "投资"));
        let stamp = utc_now_o();
        assert_eq!(stamp.len(), 28);
        assert!(stamp.ends_with('Z') && stamp.as_bytes()[10] == b'T');
    }
}
