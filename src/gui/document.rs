//! Source documents in the GUI. A Document link (`[label](scholar:<file_id>)`) opens a PDF or
//! Markdown document the Scholar Archive manages, beside the slip-box: a PDF goes to the
//! browser's own viewer, and a Markdown document is set as a reading page whose figures come
//! through the bridge and whose TeX is typeset as the reader scrolls. Each figure on the page
//! carries an id (`fig-<file stem>`), so a Card's figure can open its place in the book.
//!
//! The bridge may take seconds for a large document, so each request runs on a thread of its
//! own and never holds up the slip-box.

use super::{escape_html, header, normalize_math_delimiters, respond_text};
use crate::scholar::{Bridge, Document};
use std::collections::BTreeMap;
use std::fs;
use std::panic::{self, AssertUnwindSafe};
use std::thread;
use tiny_http::{Request, Response};

pub(super) const DOC_JS: &str = include_str!("doc.js");

/// The reading page may run only this server's scripts, so raw HTML in a book cannot act.
const READING_POLICY: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";

/// Answer `/scholar/file/<id>` or `/scholar/figure/<file_id>?path=…` on a thread of its own.
pub(super) fn serve(request: Request, path: String, query: BTreeMap<String, String>) {
    thread::spawn(move || {
        let _ = panic::catch_unwind(AssertUnwindSafe(|| answer(request, &path, &query)));
    });
}

fn answer(request: Request, path: &str, query: &BTreeMap<String, String>) {
    let id = |prefix: &str| {
        path.strip_prefix(prefix)
            .and_then(|id| id.parse::<u64>().ok())
            .filter(|id| *id > 0)
    };
    if let Some(file_id) = id("/scholar/file/") {
        match Bridge::connect().and_then(|mut bridge| bridge.document(file_id)) {
            Ok(document) => respond_document(request, &document),
            Err(error) => respond_text(
                request,
                502,
                "text/plain; charset=utf-8",
                format!("Scholar Archive document {file_id}: {error:#}"),
            ),
        }
    } else if let (Some(file_id), Some(figure)) = (id("/scholar/figure/"), query.get("path")) {
        match Bridge::connect().and_then(|mut bridge| bridge.artifact(file_id, figure)) {
            Ok((bytes, _)) => match crate::artifact::sniff(&bytes) {
                Some((_, media_type)) => {
                    let response = Response::from_data(bytes)
                        .with_header(header("Content-Type", media_type))
                        .with_header(header("Cache-Control", "private, max-age=86400"))
                        .with_header(header("X-Content-Type-Options", "nosniff"))
                        .with_header(header("Cross-Origin-Resource-Policy", "same-origin"))
                        .with_header(header(
                            "Content-Security-Policy",
                            "sandbox; default-src 'none'; style-src 'unsafe-inline'",
                        ));
                    request.respond(response).ok();
                }
                None => respond_text(request, 415, "text/plain", "not an image".to_string()),
            },
            Err(error) => respond_text(
                request,
                502,
                "text/plain; charset=utf-8",
                format!("{error:#}"),
            ),
        }
    } else {
        respond_text(request, 404, "text/plain", "not found".to_string());
    }
}

fn respond_document(request: Request, document: &Document) {
    let lower = document.name.to_ascii_lowercase();
    let markdown = document.kind == "markdown" || lower.ends_with(".md");
    if markdown {
        let page = match fs::read(&document.path) {
            Ok(bytes) => reading_page(&String::from_utf8_lossy(&bytes), document),
            Err(error) => {
                respond_text(request, 500, "text/plain", error.to_string());
                return;
            }
        };
        let response = Response::from_string(page)
            .with_header(header("Content-Type", "text/html; charset=utf-8"))
            .with_header(header("Cache-Control", "no-store"))
            .with_header(header("Content-Security-Policy", READING_POLICY))
            .with_header(header("X-Content-Type-Options", "nosniff"))
            .with_header(header("Cross-Origin-Resource-Policy", "same-origin"))
            .with_header(header("Referrer-Policy", "no-referrer"));
        request.respond(response).ok();
        return;
    }
    let pdf = document.kind == "pdf" || lower.ends_with(".pdf");
    let file = match fs::File::open(&document.path) {
        Ok(file) => file,
        Err(error) => {
            respond_text(request, 500, "text/plain", error.to_string());
            return;
        }
    };
    let (media_type, disposition) = if pdf {
        ("application/pdf", "inline")
    } else {
        ("application/octet-stream", "attachment")
    };
    let response = Response::from_file(file)
        .with_header(header("Content-Type", media_type))
        .with_header(header(
            "Content-Disposition",
            &format!(
                "{disposition}; filename*=UTF-8''{}",
                percent_encode(&document.name)
            ),
        ))
        .with_header(header("Cache-Control", "private, max-age=60"))
        .with_header(header("X-Content-Type-Options", "nosniff"))
        .with_header(header("Cross-Origin-Resource-Policy", "same-origin"));
    request.respond(response).ok();
}

/// A Markdown document as a page to read: one sheet of paper, the book's own figures in place.
fn reading_page(text: &str, document: &Document) -> String {
    let body = render_document(text, document);
    let title = escape_html(&document.name);
    let path = escape_html(&document.relative_path);
    format!(
        r#"<!doctype html>
<html lang="zh">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<link rel="stylesheet" href="/vendor/katex/katex.min.css">
<style>{READING_CSS}</style>
</head>
<body>
<header class="doc-head"><p class="doc-from">Scholar Archive · document {file_id}</p><h1>{title}</h1><p class="doc-path">{path}</p></header>
<main class="doc-sheet">
{body}
</main>
<script src="/vendor/katex/katex.min.js"></script>
<script src="/doc.js"></script>
</body>
</html>
"#,
        file_id = document.file_id,
    )
}

/// The document's Markdown as HTML. Relative images become this server's `/scholar/figure/`
/// addresses, each with the id `fig-<file stem>`; TeX becomes math spans for KaTeX.
fn render_document(text: &str, document: &Document) -> String {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_MATH;
    let folder = document
        .relative_path
        .rsplit_once('/')
        .map(|(folder, _)| folder)
        .unwrap_or("");
    let source = normalize_math_delimiters(text);
    let mut events = Vec::new();
    // An image being read: its resolved path and the alt text so far.
    let mut image: Option<(String, String)> = None;
    for event in Parser::new_ext(&source, options) {
        if let Some((_, alt)) = image.as_mut() {
            match event {
                Event::Text(text) | Event::Code(text) | Event::InlineMath(text) => {
                    alt.push_str(&text)
                }
                Event::End(TagEnd::Image) => {
                    let (path, alt) = image.take().unwrap_or_default();
                    events.push(Event::InlineHtml(
                        format!(
                            "<img id=\"{id}\" src=\"/scholar/figure/{file_id}?path={query}\" alt=\"{alt}\" loading=\"lazy\">",
                            id = figure_anchor(&path),
                            file_id = document.file_id,
                            query = percent_encode(&path),
                            alt = escape_html(&alt),
                        )
                        .into(),
                    ));
                }
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::Image { ref dest_url, .. }) => match resolve(folder, dest_url) {
                Some(path) => image = Some((path, String::new())),
                None => events.push(event),
            },
            other => events.push(other),
        }
    }
    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

/// The id a figure carries on a reading page: `fig-` and its file name without the extension.
pub(crate) fn figure_anchor(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    let stem = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect::<String>();
    format!("fig-{stem}")
}

/// A relative image address of a document in `folder`, as a path below the archive's `files/`.
/// Absolute addresses, other schemes, and paths that climb out of the archive are left alone.
fn resolve(folder: &str, dest: &str) -> Option<String> {
    let dest = percent_decode(dest.split(['?', '#']).next().unwrap_or(""));
    if dest.is_empty() || dest.contains(':') || dest.starts_with('/') || dest.starts_with('\\') {
        return None;
    }
    let mut parts: Vec<&str> = folder.split('/').filter(|part| !part.is_empty()).collect();
    for part in dest.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("/")).filter(|path| !path.is_empty())
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Some(byte) = std::str::from_utf8(&bytes[index + 1..index + 3])
                .ok()
                .and_then(|hex| u8::from_str_radix(hex, 16).ok())
            {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"-_.~/".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// The reading page: a sheet on the desk, set like the Cards, light or dark with the system.
const READING_CSS: &str = r#"
:root { --desk: #f2eee6; --paper: #fffefa; --ink: #1c1915; --ink-soft: #5a5348; --ink-faint: #9b9281;
  --edge: rgba(90, 65, 30, .16); --rule: #e3dacb; --link: #2c5aa8; --mark: #f6e5d9; --accent: #b4572a;
  --serif: "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, "Noto Serif SC", "Source Han Serif SC", "SimSun", serif;
  --sans: "Segoe UI Variable Text", system-ui, -apple-system, "Segoe UI", "Microsoft YaHei UI", sans-serif; color-scheme: light; }
@media (prefers-color-scheme: dark) { :root { --desk: #121317; --paper: #1f2228; --ink: #e9e5dc; --ink-soft: #b6afa2;
  --ink-faint: #7f796e; --edge: rgba(0, 0, 0, .5); --rule: #34383f; --link: #8fb0e8; --mark: #3a2d24; --accent: #e08a5a; color-scheme: dark; } }
* { box-sizing: border-box; }
body { margin: 0; background: var(--desk); color: var(--ink); font: 17px/1.75 var(--serif); }
.doc-head { max-width: 50rem; margin: 0 auto; padding: 28px 16px 12px; }
.doc-from { margin: 0; font: 600 11px var(--sans); letter-spacing: .08em; text-transform: uppercase; color: var(--ink-faint); }
.doc-head h1 { margin: 4px 0 2px; font: 600 22px/1.3 var(--serif); }
.doc-path { margin: 0; font: 12px var(--sans); color: var(--ink-faint); overflow-wrap: anywhere; }
.doc-sheet { max-width: 50rem; margin: 0 auto 64px; padding: 36px clamp(16px, 6vw, 64px); background: var(--paper);
  border-radius: 3px; box-shadow: 0 0 0 .5px var(--edge), 0 1px 2px rgba(60, 45, 20, .08), 0 18px 36px -18px rgba(60, 45, 20, .3); overflow-wrap: anywhere; }
.doc-sheet h1, .doc-sheet h2, .doc-sheet h3 { line-height: 1.35; margin: 1.6em 0 .5em; }
.doc-sheet h1 { font-size: 1.5em; } .doc-sheet h2 { font-size: 1.25em; } .doc-sheet h3 { font-size: 1.08em; }
.doc-sheet a { color: var(--link); }
.doc-sheet img { display: block; max-width: 100%; margin: 1em auto; padding: 6px; background: #fff; border-radius: 2px;
  box-shadow: 0 0 0 .5px var(--edge), 0 1px 2px rgba(60, 45, 20, .12), 0 6px 14px -8px rgba(60, 45, 20, .28); scroll-margin: 30vh; }
.doc-sheet img:target { outline: 3px solid var(--accent); outline-offset: 4px; }
.doc-sheet table { border-collapse: collapse; margin: 1em 0; font-size: .9em; display: block; overflow-x: auto; }
.doc-sheet td, .doc-sheet th { border: 1px solid var(--rule); padding: 4px 8px; }
.doc-sheet blockquote { margin: 1em 0; padding-left: 1em; border-left: 3px solid var(--rule); color: var(--ink-soft); }
.doc-sheet pre { overflow-x: auto; font-size: .85em; }
.math-display { display: block; overflow-x: auto; overflow-y: hidden; margin: .6em 0; }
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn book() -> Document {
        Document {
            file_id: 83,
            kind: "markdown".into(),
            name: "MWG.md".into(),
            relative_path: "Books/MWG/MWG.md".into(),
            path: PathBuf::new(),
        }
    }

    #[test]
    fn relative_images_resolve_below_the_archive_and_nothing_climbs_out() {
        assert_eq!(
            resolve("Books/MWG", "images/a.jpg").as_deref(),
            Some("Books/MWG/images/a.jpg")
        );
        assert_eq!(
            resolve("MissingMarket/Aligned", "../Assets/G%20H/images/b.jpg").as_deref(),
            Some("MissingMarket/Assets/G H/images/b.jpg")
        );
        assert_eq!(resolve("Books", "../../x.png"), None);
        assert_eq!(resolve("Books", "https://example.org/x.png"), None);
        assert_eq!(resolve("Books", "/etc/x.png"), None);
    }

    #[test]
    fn a_document_reads_with_its_figures_and_math_and_no_live_script() {
        let html = render_document(
            "# 第一章\n\n![](images/0b30f66.jpg)\nFigure 2.C.1\n\n价格 $p$ 与 $$w$$。\n\n<script>alert(1)</script>",
            &book(),
        );
        assert!(
            html.contains("<img id=\"fig-0b30f66\" src=\"/scholar/figure/83?path=Books/MWG/images/0b30f66.jpg\""),
            "{html}"
        );
        assert!(html.contains("class=\"math math-inline\""), "{html}");
        // Raw HTML passes through, but the page's policy runs only this server's scripts.
        let scripts = READING_POLICY
            .split(';')
            .map(str::trim)
            .find(|rule| rule.starts_with("script-src"));
        assert_eq!(scripts, Some("script-src 'self'"));
        let page = reading_page("x", &book());
        assert!(page.contains("<script src=\"/doc.js\"></script>"));
        assert_eq!(figure_anchor("Books/MWG/images/0b30f66.jpg"), "fig-0b30f66");
    }
}
