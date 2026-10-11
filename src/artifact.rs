//! Artifacts: the images a Card shows, kept by content in the archive's
//! `artifacts/` folder.
//!
//! A Card shows an artifact with Markdown image syntax and an `artifact:`
//! address, the SHA-256 of the image's bytes:
//! `![Figure 1: GDP around a democratization](artifact:86460e5d…)`. The bytes
//! are stored once, as `artifacts/<sha256>.<ext>`, with a small
//! `<sha256>.json` beside them saying where they came from. One figure used by
//! many Cards is one file; it never changes, and it still shows when the
//! Scholar Archive moves or drops its copy. Artifacts come in from the Scholar
//! Archive (`scholar_archive_artifact`), which keeps the documents' figures.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The address scheme of an artifact in a Card body.
pub(crate) const SCHEME: &str = "artifact:";
const FOLDER: &str = "artifacts";
/// Image formats an artifact may have, by file extension and media type.
const KINDS: [(&str, &str); 6] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
    ("bmp", "image/bmp"),
];

pub(crate) fn folder(root: &Path) -> PathBuf {
    root.join(FOLDER)
}

pub(crate) fn is_digest(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The image format of these bytes: (extension, media type).
pub(crate) fn sniff(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    let kind = |extension: &str| KINDS.iter().copied().find(|(ext, _)| *ext == extension);
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return kind("png");
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return kind("jpg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return kind("gif");
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return kind("webp");
    }
    if bytes.starts_with(b"BM") {
        return kind("bmp");
    }
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(1024)]).to_ascii_lowercase();
    if head.contains("<svg") {
        return kind("svg");
    }
    None
}

/// The stored file of an artifact, if this archive has it.
pub(crate) fn path(root: &Path, digest: &str) -> Option<PathBuf> {
    if !is_digest(digest) {
        return None;
    }
    let folder = folder(root);
    KINDS
        .iter()
        .map(|(extension, _)| folder.join(format!("{digest}.{extension}")))
        .find(|path| path.is_file())
}

/// The stored bytes and media type of an artifact.
pub(crate) fn read(root: &Path, digest: &str) -> Option<(Vec<u8>, &'static str)> {
    let bytes = fs::read(path(root, digest)?).ok()?;
    let (_, media_type) = sniff(&bytes)?;
    (sha256_hex(&bytes) == digest).then_some((bytes, media_type))
}

/// Keep an image in the archive and return its address digest. Storing the
/// same bytes again changes nothing but refreshes where they came from.
pub(crate) fn store(root: &Path, bytes: &[u8], source: &Value) -> Result<String> {
    let Some((extension, _)) = sniff(bytes) else {
        bail!("an artifact must be a PNG, JPEG, GIF, WebP, SVG, or BMP image");
    };
    let digest = sha256_hex(bytes);
    let folder = folder(root);
    fs::create_dir_all(&folder)
        .with_context(|| format!("failed to create {}", folder.display()))?;
    let target = folder.join(format!("{digest}.{extension}"));
    if !target.is_file() {
        let temporary = folder.join(format!(".{digest}.tmp"));
        fs::write(&temporary, bytes)
            .with_context(|| format!("failed to write {}", temporary.display()))?;
        fs::rename(&temporary, &target)
            .with_context(|| format!("failed to write {}", target.display()))?;
    }
    let record = json!({ "sha256": digest, "bytes": bytes.len(), "file": target.file_name().and_then(|name| name.to_str()), "source": source });
    fs::write(
        folder.join(format!("{digest}.json")),
        serde_json::to_string_pretty(&record)?,
    )
    .with_context(|| format!("failed to describe {}", target.display()))?;
    Ok(digest)
}

/// Where a stored artifact came from, as recorded when it was kept.
pub(crate) fn source(root: &Path, digest: &str) -> Option<Value> {
    if !is_digest(digest) {
        return None;
    }
    let raw = fs::read_to_string(folder(root).join(format!("{digest}.json"))).ok()?;
    serde_json::from_str::<Value>(&raw)
        .ok()
        .map(|record| record["source"].clone())
}

/// Every artifact a Card body names, in order of first appearance.
pub(crate) fn references(body: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for (index, _) in body.match_indices(SCHEME) {
        let digest = body[index + SCHEME.len()..]
            .chars()
            .take(64)
            .collect::<String>();
        if is_digest(&digest) && seen.insert(digest.clone()) {
            found.push(digest);
        }
    }
    found
}

/// The artifacts a Card body names that this archive does not hold.
pub(crate) fn missing(root: &Path, body: &str) -> Vec<String> {
    references(body)
        .into_iter()
        .filter(|digest| path(root, digest).is_none())
        .collect()
}

/// Refuse an agent's Card text that shows an artifact the archive does not hold.
pub(crate) fn require_present(root: &Path, text: &str) -> Result<()> {
    let missing = missing(root, text);
    if let Some(first) = missing.first() {
        bail!(
            "the Card shows artifact:{first}, which this archive does not hold; take it from the Scholar Archive with scholar_archive_artifact first and paste the `markdown` it returns"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfixture";

    #[test]
    fn an_image_is_kept_once_by_its_content_and_read_back_verified() {
        let root = tempfile::tempdir().unwrap();
        let digest = store(root.path(), PNG, &json!({ "file_id": 7 })).unwrap();
        assert_eq!(digest, sha256_hex(PNG));
        assert_eq!(
            store(root.path(), PNG, &json!({ "file_id": 8 })).unwrap(),
            digest
        );
        assert_eq!(
            read(root.path(), &digest).unwrap(),
            (PNG.to_vec(), "image/png")
        );
        assert_eq!(source(root.path(), &digest).unwrap()["file_id"], 8);
        assert!(
            path(root.path(), &digest)
                .unwrap()
                .ends_with(format!("{digest}.png"))
        );
        assert!(store(root.path(), b"not an image", &Value::Null).is_err());
        assert!(read(root.path(), &"0".repeat(64)).is_none());
        assert!(path(root.path(), "../secret").is_none());
    }

    #[test]
    fn a_body_names_its_artifacts_and_only_held_ones_pass() {
        let root = tempfile::tempdir().unwrap();
        let held = store(root.path(), PNG, &Value::Null).unwrap();
        let absent = "a".repeat(64);
        let body = format!(
            "![one](artifact:{held}) and ![two](artifact:{absent}) and again ![one](artifact:{held}); artifact:short"
        );
        assert_eq!(references(&body), vec![held.clone(), absent.clone()]);
        assert_eq!(missing(root.path(), &body), vec![absent]);
        assert!(require_present(root.path(), &body).is_err());
        assert!(require_present(root.path(), &format!("![one](artifact:{held})")).is_ok());
    }

    #[test]
    fn formats_are_recognized_by_their_bytes() {
        assert_eq!(sniff(b"\xff\xd8\xff\xe0").unwrap().1, "image/jpeg");
        assert_eq!(sniff(b"GIF89a").unwrap().0, "gif");
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8").unwrap().0, "webp");
        assert_eq!(sniff(b"<?xml version=\"1.0\"?><svg/>").unwrap().0, "svg");
        assert!(sniff(b"%PDF-1.4").is_none());
    }
}
