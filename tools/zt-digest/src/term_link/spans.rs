//! What a deterministic Term-link pass may not touch (design 13.4, S22 / #160).
//!
//! The pass edits authored bytes, so every construct that is not plain prose is a protected span:
//! fenced and inline code, chart fences, dollar and LaTeX math, Link macros, Markdown links and
//! images (inline, reference and reference-definition forms), raw HTML, heading lines, table rows,
//! source lines, Term head metadata, generated Reverse text, a Term Card's own term, and the
//! uncertain tail of an incomplete code or math delimiter.
//!
//! Nothing here reserializes Markdown: every range is a UTF-8 byte range into the body as the
//! archive returned it. Emphasis delimiters and backslash escapes split the eligible text as well,
//! so a match never crosses markup: `生**产**函数` links neither `生产函数` nor a shorter word that
//! would span the delimiter.

use std::ops::Range;

/// Why one byte range is outside the pass. [`Protection::id`] is the skip reason the plan reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Protection {
    /// Fenced or inline code; charts are their own reason because they are executable content.
    Code,
    Chart,
    /// Inline or display math, in the dollar or LaTeX forms the reader renders as a formula.
    Math,
    /// A Link macro. Its label is handled as a macro, so no plain-word occurrence is reported here.
    Link,
    /// A Markdown link, image, autolink or reference definition.
    MarkdownLink,
    /// A raw HTML block, comment or inline tag.
    Html,
    /// An ATX or setext heading line.
    Heading,
    /// A table row (header, delimiter or data).
    Table,
    /// A `来源`/`出处` line.
    Source,
    /// A Term head metadata line (`术语：`, `类别：`, `自动链接：` …).
    Metadata,
    /// Generated Reverse text, from its marker to the end.
    Generated,
    /// The current Term Card's own exact term.
    OwnTerm,
    /// The uncertain tail after an unclosed inline code opener.
    IncompleteCode,
    /// The uncertain tail after an unclosed math opener.
    IncompleteMath,
}

impl Protection {
    /// The plan's skip reason.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Protection::Code => "code",
            Protection::Chart => "chart",
            Protection::Math => "math",
            Protection::Link => "link",
            Protection::MarkdownLink => "markdown_link",
            Protection::Html => "html",
            Protection::Heading => "heading",
            Protection::Table => "table",
            Protection::Source => "source",
            Protection::Metadata => "metadata",
            Protection::Generated => "generated",
            Protection::OwnTerm => "own_term",
            Protection::IncompleteCode => "incomplete_code",
            Protection::IncompleteMath => "incomplete_math",
        }
    }

    /// Whether a plain link word inside this span is a skip the report counts. A Link span holds a
    /// macro, not plain prose, so its label is never a skipped ordinary occurrence.
    pub(crate) fn reportable(self) -> bool {
        self != Protection::Link
    }
}

/// One protected byte range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Span {
    pub(crate) range: Range<usize>,
    pub(crate) protection: Protection,
}

/// The Card-specific facts that add protection: a Term head or part protects its own exact term,
/// and a Term Card protects its template's metadata lines.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Profile<'a> {
    pub(crate) own_term: Option<&'a str>,
    pub(crate) term_card: bool,
}

/// One body's protected spans and eligible runs, both in byte order.
#[derive(Debug, Default)]
pub(crate) struct Layout {
    /// Protected spans, ordered and non-overlapping.
    pub(crate) spans: Vec<Span>,
    /// The text a match may cover: the gaps between spans, split further at emphasis delimiters
    /// and backslash escapes. Ordered and non-overlapping.
    pub(crate) runs: Vec<Range<usize>>,
}

impl Layout {
    /// The protection covering a byte, if one does. The plan walks spans with its own cursors; the
    /// fixtures ask this directly.
    #[cfg(test)]
    pub(crate) fn protection_at(&self, index: usize) -> Option<Protection> {
        self.spans
            .iter()
            .find(|span| span.range.start <= index && index < span.range.end)
            .map(|span| span.protection)
    }
}

/// The protected spans and eligible runs of one authored body.
pub(crate) fn layout(body: &str, profile: &Profile<'_>) -> Layout {
    let mut set = SpanSet::new(body.len());
    let code = zt_check::text::code_ranges(body);
    let fenced = zt_check::text::fenced_code_ranges(body);

    // 1. Code and charts. A fence whose info string names a chart is executable content.
    for range in &fenced {
        let protection = if is_chart_fence(body, range.start) {
            Protection::Chart
        } else {
            Protection::Code
        };
        set.add(range.clone(), protection);
    }
    for range in &code {
        if !fenced
            .iter()
            .any(|fence| fence.start <= range.start && range.end <= fence.end)
        {
            set.add(range.clone(), Protection::Code);
        }
    }

    // 2. Math, in the shared dollar forms and in the LaTeX delimiters the reader renders.
    for range in zt_check::text::math_ranges(body) {
        set.add(range, Protection::Math);
    }
    for range in tex_math_ranges(body, &code) {
        set.add(range, Protection::Math);
    }

    // 3. The uncertain tail of an incomplete delimiter. An unclosed span is no permission to
    // relink its contents (R0385).
    for span in incomplete_tails(body, &code) {
        set.add(span.0, span.1);
    }

    // 4. Link macros, ordinary and Term. A macro's bytes and label are authored syntax.
    for occurrence in zt_check::text::link_occurrences(body) {
        set.add(occurrence.macro_range, Protection::Link);
    }

    // 5. Markdown links, images, reference definitions and autolinks.
    for range in markdown_spans(body, &set) {
        set.add(range, Protection::MarkdownLink);
    }

    // 6. Raw HTML: comments, tags, and blocks that open with a block-level element.
    for (range, protection) in html_spans(body, &set) {
        set.add(range, protection);
    }

    // 7. Generated Reverse text: never part of the authored body, and never relinked.
    let lines = line_ranges(body);
    for range in &lines {
        if body[range.clone()].trim() == REVERSE_MARKER {
            set.add(range.start..body.len(), Protection::Generated);
            break;
        }
    }

    // 8-11. Line structure: headings, tables, source lines and Term metadata.
    for (index, range) in lines.iter().enumerate() {
        let text = &body[range.clone()];
        let trimmed = text.trim_start();
        if is_heading_line(body, &lines, index, text) {
            if is_setext_underline(text) && index > 0 {
                set.add(lines[index - 1].clone(), Protection::Heading);
            }
            set.add(range.clone(), Protection::Heading);
        } else if is_table_line(trimmed) {
            set.add(range.clone(), Protection::Table);
        } else if is_source_line(trimmed) {
            set.add(range.clone(), Protection::Source);
        } else if profile.term_card && zt_check::term::head_line(trimmed).is_some() {
            set.add(range.clone(), Protection::Metadata);
        }
    }

    // 12. A Term Card's own exact term: no self-reference and no redundant continuation use.
    if let Some(term) = profile.own_term.filter(|term| !term.is_empty()) {
        for range in occurrences(body, term, &set) {
            set.add(range, Protection::OwnTerm);
        }
    }

    set.finish(body)
}

/// The Reverse marker a derived section starts with. The archive returns the authored body without
/// it, so this is defensive: a body that still holds the marker protects everything after it.
pub(crate) const REVERSE_MARKER: &str = "<!-- zt:reverse-links -->";

/// Whether a fence opens a chart (` ```chart `, or any info string whose first word is `chart`).
fn is_chart_fence(body: &str, start: usize) -> bool {
    let first = body[start..].lines().next().unwrap_or_default();
    let info = first
        .trim_start_matches(' ')
        .trim_start_matches(['`', '~']);
    info.split_whitespace()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case("chart"))
}

/// Byte ranges of `\(...\)` and `\[...\]` math outside code. `\\(` is an escaped backslash and a
/// parenthesis, not TeX, exactly as the reader's delimiter rewrite decides.
fn tex_math_ranges(body: &str, code: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < body.len() {
        if code.iter().any(|range| range.contains(&index)) {
            index += 1;
            continue;
        }
        let rest = &body[index..];
        let opener = if rest.starts_with("\\[") {
            ("\\[", "\\]")
        } else if rest.starts_with("\\(") {
            ("\\(", "\\)")
        } else {
            index += char_len(body, index);
            continue;
        };
        if escaped(body, index) {
            index += 2;
            continue;
        }
        let inner = index + 2;
        match body[inner..].find(opener.1) {
            Some(at) => {
                let end = inner + at + 2;
                ranges.push(index..end);
                index = end;
            }
            None => index += 2,
        }
    }
    ranges
}

/// The uncertain tails after an unclosed inline code, `$`, `$$`, `\(` or `\[` opener. A display
/// opener protects the rest of the body; an inline opener protects the rest of its line; an
/// unclosed backtick protects the rest of its prose section, the way the shared code scanner reads
/// a code span that continues over line breaks.
fn incomplete_tails(body: &str, code: &[Range<usize>]) -> Vec<(Range<usize>, Protection)> {
    let mut tails = Vec::new();
    let macros: Vec<Range<usize>> = zt_check::text::link_occurrences(body)
        .into_iter()
        .map(|occurrence| occurrence.macro_range)
        .collect();
    let math: Vec<Range<usize>> = zt_check::text::math_ranges(body)
        .into_iter()
        .chain(tex_math_ranges(body, code))
        .collect();
    let fenced = zt_check::text::fenced_code_ranges(body);
    let opaque = |index: usize| {
        code.iter().any(|range| range.contains(&index))
            || math.iter().any(|range| range.contains(&index))
            || macros.iter().any(|range| range.contains(&index))
    };
    // An unclosed inline span ends with its paragraph, never at the end of the book.
    let paragraph_end = |index: usize| -> usize {
        let mut end = index;
        let mut first = true;
        for line in body[index..].split_inclusive('\n') {
            if !first && line.trim().is_empty() {
                break;
            }
            first = false;
            end += line.len();
        }
        fenced
            .iter()
            .find(|fence| fence.start > index)
            .map_or(end, |fence| end.min(fence.start))
    };
    let line_end = |index: usize| -> usize {
        body[index..]
            .find('\n')
            .map_or(body.len(), |at| index + at)
    };

    let mut index = 0;
    while index < body.len() {
        if opaque(index) {
            index += 1;
            continue;
        }
        let rest = &body[index..];
        if rest.starts_with('`') && !is_fence_backtick(body, index) {
            let run = rest.bytes().take_while(|byte| *byte == b'`').count();
            let closed = find_run(body, index + run, b'`', run, &opaque).is_some();
            if closed {
                // A closed inline span is covered by the shared code ranges.
                index += run;
                continue;
            }
            let end = paragraph_end(index);
            tails.push((index..end, Protection::IncompleteCode));
            index = end.max(index + 1);
            continue;
        }
        if let Some((_open, protection, display)) = math_opener(rest)
            && !escaped(body, index)
        {
            // Every closed span is opaque, so a visible opener is exactly an unclosed one.
            let end = if display { body.len() } else { line_end(index) };
            if end > index {
                tails.push((index..end, protection));
            }
            index = end.max(index + 1);
            continue;
        }
        index += char_len(body, index);
    }
    tails
}

/// The opener shape of a math delimiter at the start of `rest`, with its protection and whether it
/// is display math.
fn math_opener(rest: &str) -> Option<(&'static str, Protection, bool)> {
    if rest.starts_with("$$") {
        Some(("$$", Protection::IncompleteMath, true))
    } else if rest.starts_with("\\[") {
        Some(("\\[", Protection::IncompleteMath, true))
    } else if rest.starts_with("\\(") {
        Some(("\\(", Protection::IncompleteMath, false))
    } else if rest.starts_with('$') {
        Some(("$", Protection::IncompleteMath, false))
    } else {
        None
    }
}

/// Whether the backtick run at `index` is part of a fence line (three or more backticks at the
/// start of a line), which the shared code scan already owns.
fn is_fence_backtick(body: &str, index: usize) -> bool {
    let line_start = body[..index].rfind('\n').map_or(0, |at| at + 1);
    let prefix = &body[line_start..index];
    prefix.trim_start_matches(' ').is_empty() && run_len(body, index, b'`') >= 3
}

fn find_run(
    body: &str,
    from: usize,
    byte: u8,
    length: usize,
    opaque: &dyn Fn(usize) -> bool,
) -> Option<usize> {
    let mut index = from;
    while index < body.len() {
        if opaque(index) {
            index += 1;
            continue;
        }
        if body.as_bytes()[index] == byte {
            let run = run_len(body, index, byte);
            if run == length {
                return Some(index + run);
            }
            index += run;
            continue;
        }
        index += char_len(body, index);
    }
    None
}

fn run_len(body: &str, index: usize, byte: u8) -> usize {
    body.as_bytes()[index..]
        .iter()
        .take_while(|value| **value == byte)
        .count()
}

/// Inline links, images, collapsed/full reference links, reference definitions and autolinks.
fn markdown_spans(body: &str, set: &SpanSet) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    // Reference definitions: a line that starts with `[id]:`.
    for range in line_ranges(body) {
        let text = &body[range.clone()];
        let trimmed = text.trim_start();
        if let Some(rest) = trimmed.strip_prefix('[')
            && let Some((_, after)) = rest.split_once("]:")
            && !after.trim().is_empty()
            && set.is_clear(&range)
        {
            spans.push(range);
        }
    }
    let mut index = 0;
    while index < body.len() {
        if set.claimed(index) {
            index += 1;
            continue;
        }
        if body[index..].starts_with("[[") {
            index += 2;
            continue;
        }
        if body[index..].starts_with("![")
            || (body.as_bytes()[index] == b'[' && (index == 0 || body.as_bytes()[index - 1] != b'!'))
        {
            let start = index;
            let bracket = if body[index..].starts_with("![") { index + 1 } else { index };
            if let Some(close) = matching(body, bracket + 1, b'[', b']') {
                let after = close + 1;
                if body[after..].starts_with('(') {
                    if let Some(end) = matching_parens(body, after) {
                        let range = start..end;
                        if set.is_clear(&range) {
                            spans.push(range);
                        }
                        index = end;
                        continue;
                    }
                } else if body[after..].starts_with('[') {
                    if let Some(end) = matching(body, after + 1, b'[', b']') {
                        let range = start..end + 1;
                        if set.is_clear(&range) {
                            spans.push(range);
                        }
                        index = end + 1;
                        continue;
                    }
                }
            }
        }
        if body.as_bytes()[index] == b'<'
            && let Some(end) = autolink_end(body, index)
        {
            let range = index..end;
            if set.is_clear(&range) {
                spans.push(range);
            }
            index = end;
            continue;
        }
        index += char_len(body, index);
    }
    spans
}

/// The byte after the `]` that closes the bracket opened just before `from`, ignoring `[[` macros
/// by leaving them to the Link scan.
fn matching(body: &str, from: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 1usize;
    let mut index = from;
    while index < body.len() {
        let byte = body.as_bytes()[index];
        if byte == open {
            depth += 1;
        } else if byte == close {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        } else if byte == b'\n' {
            return None;
        }
        index += char_len(body, index);
    }
    None
}

/// The byte after the `)` that closes an inline link destination opened at `from`.
fn matching_parens(body: &str, from: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut index = from + 1;
    while index < body.len() {
        match body.as_bytes()[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index + 1);
                }
            }
            b'\n' => return None,
            b'\\' => {
                index += 1;
                if index >= body.len() {
                    return None;
                }
            }
            _ => {}
        }
        index += char_len(body, index);
    }
    None
}

/// An autolink (`<https://…>`, `<mailto:…>` or `<user@host>`), ending after its `>`.
fn autolink_end(body: &str, start: usize) -> Option<usize> {
    let rest = &body[start..];
    let end = rest.find('>')?;
    if end < 3 {
        return None;
    }
    let inner = &rest[1..end];
    if inner.contains(['<', ' ', '\t', '\n']) {
        return None;
    }
    let scheme = inner.starts_with("mailto:") || inner.contains('@') || {
        let Some((scheme, _)) = inner.split_once(':') else {
            return None;
        };
        !scheme.is_empty()
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'.' | b'-'))
            && scheme.as_bytes()[0].is_ascii_alphabetic()
    };
    scheme.then_some(start + end + 1)
}

/// Raw HTML: comments, tags, and a block that opens with a block-level element.
fn html_spans(body: &str, set: &SpanSet) -> Vec<(Range<usize>, Protection)> {
    let mut spans = Vec::new();
    // A block that starts with a block-level tag runs to its closing tag, else to the next blank
    // line. Charts in this archive are exactly such blocks.
    for range in line_ranges(body) {
        let text = &body[range.clone()];
        let trimmed = text.trim_start();
        let Some(tag) = block_tag(trimmed) else {
            continue;
        };
        if !set.is_clear(&range) {
            continue;
        }
        let close = format!("</{tag}");
        let rest = &body[range.start..];
        let end = match rest.to_ascii_lowercase().find(&close) {
            Some(at) => {
                let after = range.start + at;
                body[after..].find('>').map_or(body.len(), |gt| after + gt + 1)
            }
            None => {
                // An HTML block type 6 ends at the next blank line.
                let mut end = range.start;
                for line in body[range.start..].split_inclusive('\n') {
                    if line.trim().is_empty() {
                        break;
                    }
                    end += line.len();
                }
                if end == range.start { body.len() } else { end }
            }
        };
        spans.push((range.start..end, Protection::Html));
    }
    // Inline comments and tags.
    let mut index = 0;
    while index < body.len() {
        if set.claimed(index) {
            index += 1;
            continue;
        }
        if body[index..].starts_with("<!--") {
            let end = body[index..]
                .find("-->")
                .map_or(body.len(), |at| index + at + 3);
            spans.push((index..end, Protection::Html));
            index = end;
            continue;
        }
        if body.as_bytes()[index] == b'<'
            && let Some(end) = tag_end(body, index)
        {
            spans.push((index..end, Protection::Html));
            index = end;
            continue;
        }
        index += char_len(body, index);
    }
    spans
}

/// The tag an HTML block line opens with, when it starts a block-level element.
fn block_tag(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix('<')?.strip_prefix('/').unwrap_or_else(|| &trimmed[1..]);
    let name: String = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric())
        .collect();
    let tag = name.to_ascii_lowercase();
    const BLOCKS: [&str; 26] = [
        "address", "article", "aside", "blockquote", "canvas", "div", "details", "dialog",
        "figure", "figcaption", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header",
        "iframe", "ol", "pre", "script", "section", "style", "table",
    ];
    (BLOCKS.contains(&tag.as_str()) || matches!(tag.as_str(), "ul" | "p" | "svg" | "video"))
        .then_some(tag)
}

/// The byte after a valid inline HTML tag at `start`, if the `<` opens one.
fn tag_end(body: &str, start: usize) -> Option<usize> {
    let mut inner = &body[start + 1..];
    if let Some(stripped) = inner.strip_prefix('/') {
        inner = stripped;
    }
    let first = inner.chars().next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let end = inner.find('>')?;
    if inner[..end].contains(['<', '\n']) {
        return None;
    }
    Some(body.len() - inner.len() + end + 1)
}

/// An ATX heading line.
fn is_heading_line(body: &str, lines: &[Range<usize>], index: usize, text: &str) -> bool {
    let trimmed = text.trim_start();
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if (1..=6).contains(&hashes) {
        let after = &trimmed[hashes..];
        if after.is_empty() || after.starts_with(' ') || after.starts_with('\t') {
            return true;
        }
    }
    // A setext underline protects itself and the nonblank line above it.
    if index > 0 && is_setext_underline(text) {
        let above = &body[lines[index - 1].clone()];
        if !above.trim().is_empty() && !above.trim_start().starts_with('|') {
            return true;
        }
    }
    false
}

/// A line of only `=` or only `-`: the underline of a setext heading.
fn is_setext_underline(text: &str) -> bool {
    let line = text.trim();
    !line.is_empty() && (line.bytes().all(|byte| byte == b'=') || line.bytes().all(|byte| byte == b'-'))
}

/// A source locator line: `来源：…`, `出处：…` or `Source: …` (the checker reads the third too).
fn is_source_line(trimmed: &str) -> bool {
    if ["来源", "出处"].iter().any(|label| {
        trimmed
            .strip_prefix(label)
            .is_some_and(|rest| rest.trim_start().starts_with(['：', ':']))
    }) {
        return true;
    }
    trimmed
        .to_ascii_lowercase()
        .strip_prefix("source")
        .is_some_and(|rest| rest.trim_start().starts_with(':'))
}

/// A table row, with or without the edge pipes GFM allows (`| a | b |` or `a | b`), and a
/// delimiter row (`--- | :---:`). Cells are labels, never prose. A `|` inside a `[[…]]` Link is part
/// of its address (`[[免税@K/1|a|al]]`), never a cell separator.
fn is_table_line(trimmed: &str) -> bool {
    let unlinked = without_link_macros(trimmed);
    let trimmed = unlinked.as_str();
    if trimmed.starts_with('|') || trimmed.matches('|').count() >= 2 || trimmed.contains(" | ") {
        return true;
    }
    let cells: Vec<&str> = trimmed.split('|').map(str::trim).collect();
    cells.len() >= 2 && cells.iter().all(|cell| is_table_rule(cell))
}

/// The line with every closed `[[…]]` Link replaced by one placeholder, so its address pipes vanish.
fn without_link_macros(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find("[[") {
        let Some(close) = rest[open + 2..].find("]]") else { break };
        out.push_str(&rest[..open]);
        out.push('L');
        rest = &rest[open + 2 + close + 2..];
    }
    out.push_str(rest);
    out
}

/// One table delimiter cell: three or more hyphens with an optional alignment colon.
fn is_table_rule(cell: &str) -> bool {
    let core = cell.trim_matches(':');
    core.len() >= 3 && core.bytes().all(|byte| byte == b'-')
}

/// Byte ranges of the body's lines, without their line terminator, in order.
fn line_ranges(body: &str) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    for line in body.split_inclusive('\n') {
        let end = start + line.trim_end_matches(['\n', '\r']).len();
        ranges.push(start..end);
        start += line.len();
    }
    if ranges.is_empty() {
        ranges.push(0..0);
    }
    ranges
}

/// Every occurrence of `term` in the unclaimed text, with the ASCII boundaries a Latin edge needs.
fn occurrences(body: &str, term: &str, set: &SpanSet) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut from = 0;
    while let Some(at) = body[from..].find(term) {
        let start = from + at;
        let end = start + term.len();
        if set.is_clear(&(start..end)) && boundaries_ok(body, start, end) {
            ranges.push(start..end);
        }
        from = start + char_len(body, start);
    }
    ranges
}

/// Whether the match at `start..end` has the ASCII word boundaries a Latin edge requires: a letter,
/// digit or underscore at either edge must not continue into another such byte.
pub(crate) fn boundaries_ok(body: &str, start: usize, end: usize) -> bool {
    let word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let first = body.as_bytes()[start];
    if word(first)
        && start > 0
        && word(body.as_bytes()[start - 1])
    {
        return false;
    }
    let last = body.as_bytes()[end - 1];
    if word(last)
        && end < body.len()
        && word(body.as_bytes()[end])
    {
        return false;
    }
    true
}

/// The byte length of the character at `index`.
fn char_len(body: &str, index: usize) -> usize {
    body[index..].chars().next().map_or(1, char::len_utf8)
}

/// Whether the byte at `index` is escaped by a directly preceding backslash.
fn escaped(body: &str, index: usize) -> bool {
    index > 0 && body.as_bytes()[index - 1] == b'\\'
}

/// A set of protected spans that each addition can only shrink: an earlier protection keeps its
/// bytes, and a later one claims only what is still free.
struct SpanSet {
    spans: Vec<Span>,
    len: usize,
}

impl SpanSet {
    fn new(len: usize) -> Self {
        Self {
            spans: Vec::new(),
            len,
        }
    }

    fn add(&mut self, range: Range<usize>, protection: Protection) {
        if range.start >= range.end || range.end > self.len {
            return;
        }
        let mut parts = vec![range];
        for span in &self.spans {
            parts = parts
                .into_iter()
                .flat_map(|part| subtract(part, &span.range))
                .collect();
        }
        for part in parts {
            if part.start < part.end {
                self.spans.push(Span {
                    range: part,
                    protection,
                });
            }
        }
        self.spans.sort_by_key(|span| span.range.start);
    }

    fn claimed(&self, index: usize) -> bool {
        self.spans
            .iter()
            .any(|span| span.range.start <= index && index < span.range.end)
    }

    fn is_clear(&self, range: &Range<usize>) -> bool {
        !self
            .spans
            .iter()
            .any(|span| span.range.start < range.end && range.start < span.range.end)
    }

    /// The final layout: the spans, and the runs between them after emphasis and escapes split the
    /// free text.
    fn finish(mut self, body: &str) -> Layout {
        self.spans.sort_by_key(|span| span.range.start);
        let mut runs = Vec::new();
        let mut cursor = 0;
        for span in &self.spans {
            if cursor < span.range.start {
                runs.push(cursor..span.range.start);
            }
            cursor = cursor.max(span.range.end);
        }
        if cursor < body.len() {
            runs.push(cursor..body.len());
        }
        // Emphasis delimiters and escapes are boundaries: no match may cross one.
        let mut boundaries: Vec<Range<usize>> = Vec::new();
        for index in 0..body.len() {
            let byte = body.as_bytes()[index];
            if !body.is_char_boundary(index) {
                continue;
            }
            if matches!(byte, b'*' | b'_' | b'~') {
                boundaries.push(index..index + 1);
            } else if byte == b'\\' {
                boundaries.push(index..index + 1);
                let next = index + 1;
                if next < body.len()
                    && body.is_char_boundary(next)
                    && body.as_bytes()[next].is_ascii_punctuation()
                {
                    boundaries.push(next..next + 1);
                }
            }
        }
        for boundary in boundaries {
            runs = runs
                .into_iter()
                .flat_map(|run| subtract(run, &boundary))
                .filter(|run| run.start < run.end)
                .collect();
        }
        Layout {
            spans: self.spans,
            runs,
        }
    }
}

/// `range` without `cut`.
fn subtract(range: Range<usize>, cut: &Range<usize>) -> Vec<Range<usize>> {
    if cut.end <= range.start || range.end <= cut.start {
        return vec![range];
    }
    let mut parts = Vec::new();
    if range.start < cut.start {
        parts.push(range.start..cut.start);
    }
    if cut.end < range.end {
        parts.push(cut.end..range.end);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scanner under test, under a name the tests' own `Layout` bindings cannot shadow.
    fn lay(body: &str, profile: &Profile<'_>) -> Layout {
        layout(body, profile)
    }

    /// The protection of the span holding `needle`, or a panic that names the fixture.
    fn at(body: &str, layout: &Layout, needle: &str) -> Option<Protection> {
        let start = body.find(needle).unwrap_or_else(|| panic!("no {needle:?} in {body:?}"));
        layout.protection_at(start)
    }

    fn assert_protection(body: &str, layout: &Layout, needle: &str, protection: Protection) {
        assert_eq!(at(body, layout, needle), Some(protection), "{needle:?}");
    }

    /// Design 13.4: code, charts, math, Link macros and every Markdown link form are protected.
    #[test]
    fn code_charts_math_links_and_markdown_links_are_protected() {
        let body = "# 标题\n\n正文 `code 均衡` 与\n\n```rust\n均衡\n```\n\n```chart\n<svg>均衡</svg>\n```\n\n公式 $a 均衡$ 与 $$b 均衡$$ 和 \\(c 均衡\\) 及 \\[d 均衡\\]。\n\n见 [[均衡@K/1|a]] 与 [均衡](https://x/均衡) 和 ![图](img.png) 与 [均衡][ref]。\n\n[ref]: https://x\n";
        let layout = lay(body, &Profile::default());
        assert_protection(body, &layout, "# 标题", Protection::Heading);
        assert_protection(body, &layout, "code 均衡", Protection::Code);
        assert_protection(body, &layout, "```rust", Protection::Code);
        assert_protection(body, &layout, "```chart", Protection::Chart);
        assert_protection(body, &layout, "$a 均衡$", Protection::Math);
        assert_protection(body, &layout, "$$b 均衡$$", Protection::Math);
        assert_protection(body, &layout, "c 均衡", Protection::Math);
        assert_protection(body, &layout, "d 均衡", Protection::Math);
        assert_protection(body, &layout, "[[均衡@K/1|a]]", Protection::Link);
        assert_protection(body, &layout, "[均衡](https", Protection::MarkdownLink);
        assert_protection(body, &layout, "![图](img", Protection::MarkdownLink);
        assert_protection(body, &layout, "[均衡][ref]", Protection::MarkdownLink);
        assert_protection(body, &layout, "[ref]: https", Protection::MarkdownLink);
    }

    /// R0380-R0384: heading lines, table rows, source lines, Term metadata and generated text are
    /// exact; Term metadata applies only on a Term Card.
    #[test]
    fn headings_tables_sources_metadata_and_generated_text_are_protected() {
        let body = "## 小节\n\n标题\n===\n\n| 术语 | 含义 |\n| --- | --- |\n| 均衡 | 稳定 |\n\n来源：[[K]] §1\n\n类别：概念\n自动链接：否\n自动链接理由：太一般。\n\n<!-- zt:reverse-links -->\n## Reverse links\n\n- [[K/3]] 均衡\n";
        let term = lay(
            body,
            &Profile {
                own_term: None,
                term_card: true,
            },
        );
        assert_protection(body, &term, "## 小节", Protection::Heading);
        assert_protection(body, &term, "标题", Protection::Heading);
        assert_protection(body, &term, "===", Protection::Heading);
        assert_protection(body, &term, "| 术语", Protection::Table);
        assert_protection(body, &term, "| --- |", Protection::Table);
        // GFM allows rows without the edge pipes; their cells are labels too.
        let loose = "均衡 | 稳定状态\n--- | :---:\n来源：[[K]]\nSource: 均衡\n";
        let loose_layout = lay(loose, &Profile::default());
        assert_protection(loose, &loose_layout, "均衡 | 稳定状态", Protection::Table);
        assert_protection(loose, &loose_layout, "--- | :---:", Protection::Table);
        assert_protection(loose, &loose_layout, "Source: 均衡", Protection::Source);
        assert_protection(body, &term, "来源：", Protection::Source);
        assert_protection(body, &term, "类别：", Protection::Metadata);
        assert_protection(body, &term, "自动链接：否", Protection::Metadata);
        assert_protection(body, &term, "自动链接理由：", Protection::Metadata);
        assert_protection(body, &term, "## Reverse links", Protection::Generated);
        // The same template lines are prose on a Card that is not a Term Card.
        let prose = lay(body, &Profile::default());
        assert_ne!(at(body, &prose, "类别："), Some(Protection::Metadata));
        assert_ne!(at(body, &prose, "自动链接：否"), Some(Protection::Metadata));
    }

    /// R0383: a Term Card never links its own term, and a Latin own term needs the same ASCII
    /// word boundaries as a link word.
    #[test]
    fn own_term_occurrences_are_protected_without_latin_substrings() {
        let body = "均衡是稳定的。均衡不等于最优。";
        let layout = lay(
            body,
            &Profile {
                own_term: Some("均衡"),
                term_card: true,
            },
        );
        let first = body.find("均衡").unwrap();
        assert_eq!(layout.protection_at(first), Some(Protection::OwnTerm));
        let second = body.rfind("均衡").unwrap();
        assert_eq!(layout.protection_at(second), Some(Protection::OwnTerm));

        let body = "GDP 与 GDPR 与 GDP_2024 与 gdp 与 GDP。";
        let latin = lay(
            body,
            &Profile {
                own_term: Some("GDP"),
                term_card: true,
            },
        );
        assert_eq!(latin.protection_at(0), Some(Protection::OwnTerm));
        assert_eq!(at(body, &latin, "GDPR"), None, "a Latin substring is no use");
        assert_eq!(at(body, &latin, "GDP_2024"), None);
        assert_eq!(at(body, &latin, "gdp"), None, "case decides");
        let last = body.rfind("GDP").unwrap();
        assert_eq!(latin.protection_at(last), Some(Protection::OwnTerm));
    }

    /// R0386/R0402: a match never crosses emphasis, an escape or an HTML tag, and the protected
    /// spans are what splits the eligible text.
    #[test]
    fn emphasis_escapes_and_html_tags_split_the_eligible_runs() {
        let body = "生产**函数**，净生产函数，\\均衡，<span>均衡</span>均衡";
        let layout = lay(body, &Profile::default());
        let runs: Vec<&str> = layout
            .runs
            .iter()
            .map(|run| &body[run.clone()])
            .collect();
        assert!(runs.contains(&"生产"), "{runs:?}");
        assert!(runs.contains(&"函数"), "{runs:?}");
        assert!(
            !runs.iter().any(|run| run.contains("生产**函数**")),
            "no run crosses the emphasis"
        );
        assert!(runs.iter().all(|run| !run.contains(['*', '\\', '<', '>'])), "{runs:?}");
        assert_eq!(
            runs.iter().filter(|run| **run == "均衡").count(),
            2,
            "the escaped byte and the tags are boundaries: {runs:?}"
        );
    }

    /// R0385: an unclosed delimiter protects its uncertain tail instead of relinking it.
    #[test]
    fn unclosed_code_and_math_protect_their_uncertain_tail() {
        let body = "前 $x 没有闭合\n后 均衡\n\n前 `code 没有闭合\n后 均衡\n\n前 $$x\n均衡\n";
        let layout = lay(body, &Profile::default());
        assert_protection(body, &layout, "$x 没有闭合", Protection::IncompleteMath);
        assert_protection(body, &layout, "`code 没有闭合", Protection::IncompleteCode);
        assert_protection(body, &layout, "$$x", Protection::IncompleteMath);
        for needle in ["后 均衡\n\n前", "后 均衡\n\n前"] {
            let start = body.find(needle).unwrap();
            assert_eq!(
                layout.protection_at(start),
                None,
                "the line after an unclosed inline opener is prose again"
            );
        }
        // A closed dollar span, an escaped dollar, and a LaTeX form are not incomplete tails.
        let closed = "价格 $p$ 与 \\$5 与 \\(x\\) 与 \\[y\\] 后面 均衡";
        let closed_layout = lay(closed, &Profile::default());
        assert_eq!(
            closed_layout.protection_at(closed.rfind("均衡").unwrap()),
            None,
            "{:?}",
            closed_layout.spans
        );
    }

    /// The layout is an exact partition of the body: ordered spans and runs never overlap, and no
    /// run covers a protected byte.
    #[test]
    fn spans_and_runs_never_overlap_and_runs_hold_no_protected_byte() {
        let body = "# 标题\n\n正文 `code` 与 $x$ 与 [[均衡@K/1]] 与 [标签](x) 与 <b>粗</b>。\n\n| 表 |\n| --- |\n\n来源：[[K]]\n";
        let layout = lay(
            body,
            &Profile {
                own_term: Some("正文"),
                term_card: true,
            },
        );
        for pair in layout.spans.windows(2) {
            assert!(pair[0].range.end <= pair[1].range.start, "{:?}", layout.spans);
        }
        for span in &layout.spans {
            assert!(!span.range.is_empty());
            assert!(span.range.end <= body.len());
            assert!(body.is_char_boundary(span.range.start));
            assert!(body.is_char_boundary(span.range.end));
        }
        for pair in layout.runs.windows(2) {
            assert!(pair[0].end <= pair[1].start, "{:?}", layout.runs);
        }
        for run in &layout.runs {
            assert!(!run.is_empty());
            assert!(body.is_char_boundary(run.start));
            assert!(body.is_char_boundary(run.end));
            for span in &layout.spans {
                assert!(
                    span.range.end <= run.start || run.end <= span.range.start,
                    "run {run:?} covers {span:?}"
                );
            }
        }
    }

    #[test]
    fn latin_boundaries_treat_letters_digits_and_underscore_as_word_bytes() {
        assert!(boundaries_ok("GDP 增长", 0, 3));
        assert!(!boundaries_ok("GDPR", 0, 3));
        assert!(!boundaries_ok("GDP_2024", 0, 3));
        assert!(boundaries_ok("增长与 GDP", "增长与 ".len(), "增长与 ".len() + 3));
        assert!(boundaries_ok("均衡", 0, "均衡".len()), "a Han edge needs no boundary");
    }
}
