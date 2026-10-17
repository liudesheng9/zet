//! The pure parts of `zt-digest hub`: hub bodies as apply writes them (`Format-Item`,
//! `Format-Body`), the words of a reason (`Get-Words`), the survey's quantiles and hash, and the
//! PowerShell renderings of values the script's messages used (`[string]`, `ConvertTo-Json -Depth`).

use crate::ps::{self, NL};
use anyhow::Result;
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

/// The kind words a neighbour line may start with (`$Kinds`).
pub const KINDS: &[&str] = &["依赖", "基于", "例示", "案例", "对照", "区别", "放宽", "推广", "对应", "类比", "反驳", "批评"];

pub static LINE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^line:(.+)$").unwrap());
pub static TREE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^tree:(.+)$").unwrap());
pub static FIELD_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^field:(.+)$").unwrap());
static LINE_OR_FIELD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^(line|field):").unwrap());
static TREE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^tree:").unwrap());
static ROOT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/0$").unwrap());
static FORMULA_OR_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\$[\s\S]*?\$\$|\$[^$\n]+\$|\[\[[^\]]*\]\]").unwrap());
static NOT_WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}]").unwrap());

/// `$Kinds.ContainsKey($kind)` (a hashtable: case ignored).
pub fn is_kind(kind: &str) -> bool {
    KINDS.iter().any(|k| ps::key(k) == ps::key(kind))
}

/// `[string]$v` for a value `ConvertFrom-Json -AsHashtable` made: an object is an
/// OrderedHashtable, an array its items joined by spaces.
pub fn st(v: &Value) -> String {
    match v {
        Value::Object(_) => "System.Management.Automation.OrderedHashtable".to_string(),
        Value::Array(items) => items.iter().map(st).collect::<Vec<_>>().join(" "),
        other => ps::s(other),
    }
}

/// `Get-TreeId`: the part of an address before the first `/`.
pub fn tree_id(address: &str) -> String {
    address.split('/').next().unwrap_or_default().to_string()
}

/// `$value | ConvertTo-Json -Compress -Depth <depth>`: a container deeper than `depth` is
/// written as its type name; an array on the pipeline is its items (none: no text; one: the item).
pub fn piped_json(value: &Value, depth: usize) -> String {
    fn write(v: &Value, level: usize, depth: usize) -> Value {
        match v {
            Value::Object(_) if level > depth => json!("System.Management.Automation.OrderedHashtable"),
            Value::Array(_) if level > depth => json!("System.Object[]"),
            Value::Object(map) => Value::Object(map.iter().map(|(k, x)| (k.clone(), write(x, level + 1, depth))).collect()),
            Value::Array(items) => Value::Array(items.iter().map(|x| write(x, level + 1, depth)).collect()),
            other => other.clone(),
        }
    }
    match value {
        Value::Array(items) if items.is_empty() => String::new(),
        Value::Array(items) if items.len() == 1 => ps::json_compact(&write(&items[0], 0, depth)),
        other => ps::json_compact(&write(other, 0, depth)),
    }
}

/// `Get-Words`: the letters and digits of a reason, formulas and Links removed. .NET matches
/// UTF-16 units, so a character outside the BMP (two surrogates) is no letter.
pub fn words(text: &str) -> String {
    let stripped = FORMULA_OR_LINK.replace_all(text, "");
    NOT_WORD.replace_all(&stripped, "").chars().filter(|c| (*c as u32) <= 0xFFFF).collect()
}

/// `Format-Item`: one member line, or None when the ref did not resolve.
pub fn format_item(r: &str, reason: &str, address: Option<&str>, kind: &str) -> Option<String> {
    let address = address.filter(|a| !a.is_empty())?;
    // A small Topic has no topic hub, so `tree:<n>` may resolve to its root, which is not a hub.
    let is_hub = LINE_OR_FIELD.is_match(r) || (TREE_PREFIX.is_match(r) && !ROOT.is_match(address));
    let words = if !kind.is_empty() && is_kind(kind) { format!("{kind} ") } else { String::new() };
    Some(format!("- {words}[[{address}]]{}：{reason}", if is_hub { "（枢纽）" } else { "" }))
}

/// A resolver of member refs: the address, or None when apply would drop the line.
pub type Resolve<'a> = dyn FnMut(&str) -> Result<Option<String>> + 'a;

/// `Format-Body`: 范围, one `##` section per group with lines, then 邻近; refs that do not resolve
/// go to `dropped`. The StringBuilder's lines end in CRLF; the end is trimmed.
pub fn format_body(scope: &str, groups: &Value, neighbours: &Value, resolve: &mut Resolve, dropped: &mut Vec<String>) -> Result<String> {
    let mut out = String::new();
    if !scope.is_empty() {
        out.push_str(&format!("范围：{scope}{NL}{NL}"));
    }
    for g in ps::pipe(groups) {
        let mut lines = Vec::new();
        for item in ps::pipe(&g["items"]) {
            let r = st(&item["ref"]);
            let address = resolve(&r)?;
            match format_item(&r, &st(&item["reason"]), address.as_deref(), "") {
                Some(line) => lines.push(line),
                None => dropped.push(r),
            }
        }
        if !lines.is_empty() {
            out.push_str(&format!("## {}{NL}", st(&g["name"])));
            for l in &lines {
                out.push_str(l);
                out.push_str(NL);
            }
            out.push_str(NL);
        }
    }
    let mut near = Vec::new();
    for item in ps::pipe(neighbours) {
        let r = st(&item["ref"]);
        let address = resolve(&r)?;
        match format_item(&r, &st(&item["reason"]), address.as_deref(), &st(&item["kind"])) {
            Some(line) => near.push(line),
            None => dropped.push(r),
        }
    }
    if !near.is_empty() {
        out.push_str(&format!("## 邻近{NL}"));
        for l in &near {
            out.push_str(l);
            out.push_str(NL);
        }
    }
    Ok(out.trim_end().to_string())
}

/// `Get-Quantiles`: n, min, p25, median, p75, p90, max (nearest rank, rounding half up).
pub fn quantiles(values: &[i64]) -> Value {
    let mut sorted = values.to_vec();
    sorted.sort();
    if sorted.is_empty() {
        return json!({ "n": 0 });
    }
    let last = sorted.len() - 1;
    let at = |q: f64| sorted[last.min((q * last as f64 + 0.5).floor() as usize)];
    json!({ "n": sorted.len(), "min": sorted[0], "p25": at(0.25), "median": at(0.5), "p75": at(0.75), "p90": at(0.9), "max": sorted[last] })
}

/// The first 16 hex digits (upper case) of the SHA-256 of a text's UTF-8 bytes: a tree's hash.
pub fn hash16(text: &str) -> String {
    sha256(text.as_bytes()).iter().map(|b| format!("{b:02X}")).collect::<String>()[..16].to_string()
}

/// SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be,
        0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa,
        0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85,
        0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
        0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f,
        0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut h: [u32; 8] = [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
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
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 32];
    for (i, word) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&word.to_be_bytes());
    }
    out
}

/// The `SELF-CHECK:` line of a job file: the precheck of one DeepSeek file, run as this tool.
pub fn self_check(work: &str, file: &str) -> String {
    format!("SELF-CHECK: zt-digest hub -Step precheck -Work \"{work}\" -File \"{file}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_the_standard_vectors() {
        let hex = |d: &[u8]| sha256(d).iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
        // What pwsh 7 printed for [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData(
        // $utf8.GetBytes("1/1`t标题`n1/2`t第二"))).Substring(0, 16).
        assert_eq!(hash16("1/1\t标题\n1/2\t第二"), "E23D58A77D154A84");
    }

    #[test]
    fn quantiles_round_half_up() {
        assert_eq!(quantiles(&[]), json!({ "n": 0 }));
        let q = quantiles(&[9, 1, 5, 3]);
        assert_eq!(q, json!({ "n": 4, "min": 1, "p25": 3, "median": 5, "p75": 5, "p90": 9, "max": 9 }));
        assert_eq!(quantiles(&[7])["p90"], json!(7));
    }

    #[test]
    fn words_drop_formulas_links_and_punctuation() {
        assert_eq!(words("高质量卖者退出，平均质量下降。"), "高质量卖者退出平均质量下降");
        assert_eq!(words("见 [[3/1]] 与 $x+y$ 和 $$a$$ ok1"), "见与和ok1");
        assert_eq!(words("𠀀甲"), "甲");
    }

    #[test]
    fn items_and_bodies_as_apply_writes_them() {
        assert_eq!(format_item("line:3/2", "一支。", Some("3/2|a"), "").unwrap(), "- [[3/2|a]]（枢纽）：一支。");
        assert_eq!(format_item("tree:4", "主题。", Some("4/0"), "").unwrap(), "- [[4/0]]：主题。");
        assert_eq!(format_item("tree:4", "主题。", Some("4/1|a"), "").unwrap(), "- [[4/1|a]]（枢纽）：主题。");
        assert_eq!(format_item("5/1", "对。", Some("5/1"), "对照").unwrap(), "- 对照 [[5/1]]：对。");
        assert_eq!(format_item("5/1", "对。", Some("5/1"), "随便").unwrap(), "- [[5/1]]：对。");
        assert!(format_item("5/9", "x", None, "").is_none());
        let groups = json!([{ "name": "后果", "items": [{ "ref": "3/1", "reason": "甲。" }, { "ref": "3/9", "reason": "乙。" }] }, { "name": "空", "items": [] }]);
        let near = json!([{ "kind": "对照", "ref": "4/1", "reason": "丙。" }]);
        let mut dropped = Vec::new();
        let mut resolve = |r: &str| -> Result<Option<String>> { Ok((r != "3/9").then(|| r.to_string())) };
        let body = format_body("入口。", &groups, &near, &mut resolve, &mut dropped).unwrap();
        assert_eq!(body, ["范围：入口。", "", "## 后果", "- [[3/1]]：甲。", "", "## 邻近", "- 对照 [[4/1]]：丙。"].join(NL));
        assert_eq!(dropped, ["3/9"]);
        // A hub without groups or neighbours: @($null) is one item, so apply drops one empty ref each.
        let mut dropped = Vec::new();
        let body = format_body("", &Value::Null, &Value::Null, &mut |_: &str| Ok(None), &mut dropped).unwrap();
        assert_eq!((body.as_str(), dropped.len()), ("", 2));
    }

    #[test]
    fn values_render_as_powershell_rendered_them() {
        assert_eq!(st(&json!({ "a": 1 })), "System.Management.Automation.OrderedHashtable");
        assert_eq!(st(&json!(["a", 2])), "a 2");
        let g = json!({ "name": "", "items": [{ "ref": "a", "reason": { "x": [1, { "y": 2 }] } }] });
        assert_eq!(piped_json(&g, 2), r#"{"name":"","items":[{"ref":"a","reason":"System.Management.Automation.OrderedHashtable"}]}"#);
        assert_eq!(piped_json(&g, 4), r#"{"name":"","items":[{"ref":"a","reason":{"x":[1,"System.Management.Automation.OrderedHashtable"]}}]}"#);
        assert_eq!(piped_json(&json!(null), 4), "null");
        assert_eq!(piped_json(&json!(["x"]), 4), "\"x\"");
        assert_eq!(piped_json(&json!([]), 4), "");
        assert_eq!(self_check("D:\\w", "D:\\w\\out\\hub-3.json"), r#"SELF-CHECK: zt-digest hub -Step precheck -Work "D:\w" -File "D:\w\out\hub-3.json""#);
    }
}
