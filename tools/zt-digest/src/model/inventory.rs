//! Staged source inventory (`inventory.json`) for Level 2 formal economic models
//! (docs/formal-models-proposal.md §5.5 Layer 3 & §8.3).
//!
//! Deterministic script extraction reads every display math block (`$$...$$`, single-line or
//! multi-line) with its printed `\tag{n}` / `(n)`, every explicit statement/assumption/definition/
//! proof heading (English and Chinese), exact 1-based line spans, section paths (preventing
//! restarted-numbering collisions across sections/appendices), and SHA-256 content fingerprints.
//!
//! Validation enforces all five Layer 3 gates:
//! - `source_stale`: source file SHA-256 differs from pinned `inventory.json`
//! - `anchor_invalid`: plan `src` span out of bounds or unknown `inventory_ref`
//! - `printed_mismatch`: copied display equation / `\tag{n}` disagrees with pinned source span
//! - `ownership_conflict`: two parts/models claim primary ownership of the same obligation
//! - `obligation_unsettled`: an accepted obligation has neither an owning part nor a disposition

use crate::canon;
use crate::ps::{self, list, pipe, s, truthy};
use regex::Regex;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*$").unwrap());

static TAG_CMD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\\tag\*?\s*\{((?:[^{}]|\{[^{}]*\})+)\}").unwrap());

static TRAILING_EQ_NUM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:\\quad|\\qquad|\s)\(([A-Za-z]?\d+(?:[.\-]\d+)*[A-Za-z′']*)\)\s*$").unwrap());

static LEADING_EXT_TAG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*\(([A-Za-z]?\d+(?:[.\-]\d+)*[A-Za-z′']*)\)").unwrap());

// Explicit statement/assumption/definition/proof openers (English & Chinese).
static ITEM_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^\s*(?:\*\*|__)?(?:(Proposition|Theorem|Lemma|Corollary|Definition|Assumption|Proof\s+of\s+(?:Proposition|Theorem|Lemma|Corollary)|Proof)|(命题|定理|引理|推论|定义|假设|证明))\s*(\([A-Za-z0-9.\-′']+\)|（[A-Za-z0-9.\-′']+）|[A-Za-z]?\d+(?:[.\-]\d+)*[A-Za-z′']*)?(?:\*\*|__)?\s*(?:[.:：\-—\(（]|$)",
    )
    .unwrap()
});

static SRC_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?):(\d+)(?:-(\d+))?$").unwrap());

fn normalize_ws(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sha256_str(text: &str) -> String {
    canon::sha256_hex(text.as_bytes())
}

fn file_stem(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    base.strip_suffix(".md").unwrap_or(base).to_string()
}

fn canonical_item_kind(raw: &str) -> &'static str {
    let low = raw.trim().to_lowercase();
    if low.starts_with("proof") || raw.starts_with("证明") {
        "proof"
    } else if low.starts_with("proposition") || raw.starts_with("命题") {
        "proposition"
    } else if low.starts_with("theorem") || raw.starts_with("定理") {
        "theorem"
    } else if low.starts_with("lemma") || raw.starts_with("引理") {
        "lemma"
    } else if low.starts_with("corollary") || raw.starts_with("推论") {
        "corollary"
    } else if low.starts_with("definition") || raw.starts_with("定义") {
        "definition"
    } else {
        "assumption"
    }
}

fn extract_tag(tex: &str) -> Option<String> {
    if let Some(c) = TAG_CMD.captures(tex) {
        let tag = c[1].trim();
        if !tag.is_empty() {
            return Some(tag.to_string());
        }
    }
    if let Some(c) = TRAILING_EQ_NUM.captures(tex.trim()) {
        let tag = c[1].trim();
        if !tag.is_empty() {
            return Some(tag.to_string());
        }
    }
    None
}

/// Extract deterministic equations and named items from one staged Markdown file.
pub(crate) fn extract_file(text: &str, file: &str) -> Value {
    let lines: Vec<&str> = text.lines().collect();
    let file_sha = sha256_str(text);
    let stem = file_stem(file);

    let mut heading_stack: Vec<(usize, String)> = Vec::new();
    let mut equations: Vec<Value> = Vec::new();
    let mut items: Vec<Value> = Vec::new();

    let mut eq_idx = 0usize;
    let mut item_idx = 0usize;
    let mut i = 0usize;
    let mut in_fence = false;
    let mut fence_char = '\0';

    let section_path = |stack: &[(usize, String)]| -> String {
        stack
            .iter()
            .map(|(_, h)| h.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    };

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        let line_no = i + 1;

        // Track fenced code blocks (``` or ~~~) so Markdown examples inside code fences are ignored.
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let ch = trimmed.chars().next().unwrap_or('`');
            if !in_fence {
                in_fence = true;
                fence_char = ch;
            } else if ch == fence_char {
                in_fence = false;
            }
            i += 1;
            continue;
        }
        if in_fence {
            i += 1;
            continue;
        }

        let mut heading_text: Option<String> = None;
        if let Some(c) = HEADING.captures(trimmed) {
            let depth = c[1].len();
            let title = c[2].trim().to_string();
            while heading_stack.last().is_some_and(|(d, _)| *d >= depth) {
                heading_stack.pop();
            }
            heading_stack.push((depth, title.clone()));
            heading_text = Some(title);
        }

        // Display math block (`$$...$$`), supporting multiple single-line displays, trailing `(n)`
        // equation numbers after closing `$$`, and same-line displays after a multiline close (Astra H4).
        if line.contains("$$") {
            let mut cur_line = i;
            let mut col = 0usize;
            let mut moved_line = false;
            loop {
                let slice = &lines[cur_line][col..];
                let Some(open_rel) = slice.find("$$") else {
                    break;
                };
                let after_open_col = col + open_rel + 2;
                let after_open = &lines[cur_line][after_open_col..];
                if let Some(close_rel) = after_open.find("$$") {
                    let inner = after_open[..close_rel].trim().to_string();
                    let mut next_col = after_open_col + close_rel + 2;
                    let mut tag = extract_tag(&inner);
                    if tag.is_none()
                        && let Some(c) = LEADING_EXT_TAG.captures(&lines[cur_line][next_col..])
                    {
                        let t = c[1].trim();
                        if !t.is_empty() {
                            tag = Some(t.to_string());
                        }
                        next_col += c.get(0).map(|m| m.end()).unwrap_or(0);
                    }
                    if !inner.is_empty() {
                        eq_idx += 1;
                        let norm = normalize_ws(&inner);
                        equations.push(json!({
                            "id": format!("eq:{stem}:{eq_idx}"),
                            "file": file,
                            "section_path": section_path(&heading_stack),
                            "start_line": cur_line + 1,
                            "end_line": cur_line + 1,
                            "tag": tag,
                            "tex": inner,
                            "sha256": sha256_str(&norm),
                        }));
                    }
                    col = next_col;
                } else {
                    let start_line = cur_line + 1;
                    let mut body_parts = vec![after_open.to_string()];
                    let mut end_idx = cur_line + 1;
                    let mut close_after_col = 0usize;
                    let mut found_close = false;
                    while end_idx < lines.len() {
                        if let Some(close) = lines[end_idx].find("$$") {
                            body_parts.push(lines[end_idx][..close].to_string());
                            close_after_col = close + 2;
                            found_close = true;
                            break;
                        }
                        body_parts.push(lines[end_idx].to_string());
                        end_idx += 1;
                    }
                    let end_line = (end_idx + 1).min(lines.len());
                    let inner = body_parts.join("\n").trim().to_string();
                    let mut tag = extract_tag(&inner);
                    if found_close && tag.is_none()
                        && let Some(c) = LEADING_EXT_TAG.captures(&lines[end_idx][close_after_col..])
                    {
                        let t = c[1].trim();
                        if !t.is_empty() {
                            tag = Some(t.to_string());
                        }
                        close_after_col += c.get(0).map(|m| m.end()).unwrap_or(0);
                    }
                    if !inner.is_empty() {
                        eq_idx += 1;
                        let norm = normalize_ws(&inner);
                        equations.push(json!({
                            "id": format!("eq:{stem}:{eq_idx}"),
                            "file": file,
                            "section_path": section_path(&heading_stack),
                            "start_line": start_line,
                            "end_line": end_line,
                            "tag": tag,
                            "tex": inner,
                            "sha256": sha256_str(&norm),
                        }));
                    }
                    moved_line = true;
                    if found_close {
                        cur_line = end_idx;
                        col = close_after_col;
                        // Continue scanning the remainder of the closing line for another `$$`.
                        continue;
                    } else {
                        i = lines.len();
                        break;
                    }
                }
            }
            if moved_line {
                if i < lines.len() {
                    i = cur_line + 1;
                }
                continue;
            }
        }

        // Explicit named statement / assumption / definition / proof opener (including `## Theorem 1` headings).
        let candidate_line = heading_text.as_deref().unwrap_or(trimmed);
        if let Some(c) = ITEM_HEADER.captures(candidate_line) {
            let head_word = c
                .get(1)
                .or_else(|| c.get(2))
                .map(|m| m.as_str())
                .unwrap_or("");
            let num = c.get(3).map(|m| m.as_str().trim()).unwrap_or("");
            let kind = canonical_item_kind(head_word);
            let is_proof = kind == "proof";
            // Require a label on non-proof statements (prevents matching prose sentences like "definition of ...").
            let has_explicit_marker = !num.is_empty()
                || is_proof
                || candidate_line.starts_with("**")
                || candidate_line.starts_with("__");
            if has_explicit_marker {
                item_idx += 1;
                let label = if num.is_empty() {
                    head_word.trim().to_string()
                } else {
                    format!("{} {}", head_word.trim(), num)
                };
                // Extend span until next blank-line + new header/item, bounded to 40 lines.
                let mut end_idx = i;
                while end_idx + 1 < lines.len() && end_idx - i < 40 {
                    let next = lines[end_idx + 1].trim();
                    if next.is_empty() {
                        break;
                    }
                    if HEADING.is_match(next) || ITEM_HEADER.is_match(next) {
                        break;
                    }
                    end_idx += 1;
                }
                let excerpt_raw = lines[i..=end_idx].join("\n");
                let excerpt: String = excerpt_raw.chars().take(240).collect();
                let slug: String = label
                    .chars()
                    .filter(|ch| ch.is_ascii_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();
                // Always include `stem` so items in different files never collide (Astra #17).
                let id = if slug.is_empty() {
                    format!("item:{kind}:{stem}:{item_idx}")
                } else {
                    format!("item:{kind}:{stem}:{slug}:{item_idx}")
                };
                items.push(json!({
                    "id": id,
                    "kind": kind,
                    "label": label,
                    "file": file,
                    "section_path": section_path(&heading_stack),
                    "start_line": line_no,
                    "end_line": end_idx + 1,
                    "excerpt": excerpt,
                    "sha256": sha256_str(&normalize_ws(&excerpt_raw)),
                }));
            }
        }

        i += 1;
    }

    json!({
        "file": file,
        "sha256": file_sha,
        "lines": lines.len(),
        "equations": equations,
        "items": items,
    })
}

/// Build a complete pinned `inventory.json` across one or more `(file_rel_path, content)` pairs.
pub(crate) fn build_inventory(
    model: &str,
    work_id: Option<&Value>,
    file_id: Option<&Value>,
    source_sha256: Option<&str>,
    sections: &[(String, String)],
) -> Value {
    let mut files = Vec::new();
    let mut equations = Vec::new();
    let mut items = Vec::new();
    for (file, content) in sections {
        let one = extract_file(content, file);
        files.push(json!({
            "file": one["file"],
            "sha256": one["sha256"],
            "lines": one["lines"],
        }));
        equations.extend(list(&one["equations"]).into_iter().cloned());
        items.extend(list(&one["items"]).into_iter().cloned());
    }
    json!({
        "schema_version": 1,
        "model": model,
        "work_id": work_id.cloned().unwrap_or(Value::Null),
        "file_id": file_id.cloned().unwrap_or(Value::Null),
        "file_sha256": source_sha256.map(|h| json!(h)).unwrap_or(Value::Null),
        "files": files,
        "equations": equations,
        "items": items,
        "obligations": [],
    })
}

fn norm_file_path(path: &str) -> String {
    path.replace('\\', "/").trim().to_lowercase()
}

/// Merge reader-curated obligations from an optional `obligations.json` file (either an array of
/// obligation objects or `{"obligations": [...]}`) into `inventory["obligations"]` without duplicating IDs.
pub(crate) fn merge_obligations_file(inventory: &mut Value, path: &str) -> std::result::Result<(), String> {
    if !ps::exists(path) {
        return Ok(());
    }
    let parsed = crate::util::read_json(path)
        .map_err(|e| format!("inventory_invalid: `{path}` could not be parsed as JSON: {e:#}"))?;
    let extra: Vec<Value> = if let Some(arr) = parsed.as_array() {
        arr.clone()
    } else if let Some(arr) = parsed["obligations"].as_array() {
        arr.clone()
    } else {
        return Err(format!("inventory_invalid: `{path}` must be a JSON array or object with `obligations`"));
    };
    let mut existing: Vec<Value> = list(&inventory["obligations"]).into_iter().cloned().collect();
    let mut seen: BTreeSet<String> = existing.iter().map(|o| s(&o["id"]).to_lowercase()).filter(|x| !x.is_empty()).collect();
    for ob in extra {
        let id = s(&ob["id"]);
        if !id.is_empty() && seen.insert(id.to_lowercase()) {
            existing.push(ob);
        }
    }
    inventory["obligations"] = Value::Array(existing);
    Ok(())
}

/// Cross-model `ownership_conflict` check across sibling plans in a batch (Astra H3).
/// Returns `(model, fault_line)` pairs whenever two distinct models claim primary ownership of the
/// same `inventory_ref` / `obligations` / `inventory_refs` ID.
pub(crate) fn cross_plan_ownership_faults(plans: &[(String, &Value)]) -> Vec<(String, String)> {
    let mut owners: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut faults = Vec::new();
    for (model, plan) in plans {
        for part in list(&plan["parts"]) {
            if truthy(&part["take"]) {
                continue;
            }
            let pid = s(&part["id"]);
            let mut refs = Vec::new();
            let r = s(&part["inventory_ref"]);
            if !r.is_empty() {
                refs.push(r);
            }
            for ob in pipe(&part["obligations"]).into_iter().chain(pipe(&part["inventory_refs"])) {
                let oid = s(ob);
                if !oid.is_empty() {
                    refs.push(oid);
                }
            }
            for r in refs {
                match owners.get(&r) {
                    Some((prev_model, prev_part)) if !ps::eq(prev_model, model) => {
                        faults.push((
                            model.clone(),
                            format!(
                                "ownership_conflict: `{r}` is claimed by both model `{prev_model}` (part `{prev_part}`) and model `{model}` (part `{pid}`)"
                            ),
                        ));
                    }
                    None => {
                        owners.insert(r, (model.clone(), pid.clone()));
                    }
                    _ => {}
                }
            }
        }
    }
    faults
}

/// Extract normalized file paths from a part's `src` spans (e.g. `["sections/s03.md:1-10"]` -> `{"sections/s03.md"}`).
pub(crate) fn src_files_of_spans(spans: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for span in spans {
        let f = SRC_SPAN
            .captures(span)
            .map(|c| norm_file_path(&c[1]))
            .unwrap_or_else(|| norm_file_path(span));
        if !f.is_empty() && !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// Validate a Level 2 formal `plan.json` against its pinned `inventory.json` (proposal §5.5 Layer 3).
/// Returns human-readable fault lines prefixed with the exact fault code (`source_stale`,
/// `anchor_invalid`, `printed_mismatch`, `ownership_conflict`, `obligation_unsettled`).
pub(crate) fn validate_plan(
    plan: &Value,
    inventory: &Value,
    staging: Option<&str>,
) -> Vec<String> {
    let mut faults = Vec::new();

    // Index files by normalized path.
    let mut file_lines: BTreeMap<String, (usize, String, String)> = BTreeMap::new();
    for f in list(&inventory["files"]) {
        let path = s(&f["file"]);
        let lines = f["lines"].as_u64().unwrap_or(0) as usize;
        let sha = s(&f["sha256"]);
        if !path.is_empty() {
            file_lines.insert(norm_file_path(&path), (lines, sha, path));
        }
    }
    if file_lines.is_empty() {
        faults.push("inventory_invalid: inventory.json lists no pinned source files; re-extract inventory.json".into());
    }

    // 1. source_stale: verify pinned files on disk exist and match SHA-256 (and manifest source_sha256)
    // when staging is supplied.
    if let Some(staging_dir) = staging.filter(|d| !d.is_empty()) {
        let pinned_source_sha = s(&inventory["file_sha256"]);
        if !pinned_source_sha.is_empty() {
            let manifest_path = ps::join(staging_dir, "manifest.json");
            if let Ok(manifest) = crate::util::read_json(&manifest_path) {
                let actual_src_sha = s(&manifest["source_sha256"]);
                if !actual_src_sha.is_empty() && !actual_src_sha.eq_ignore_ascii_case(&pinned_source_sha) {
                    faults.push(format!(
                        "source_stale: manifest.json source_sha256 changed (pinned {pinned_source_sha}, manifest {actual_src_sha}); re-extract inventory.json"
                    ));
                }
            }
        }
        for (lines, expected_sha, raw_path) in file_lines.values() {
            let _ = lines;
            let full = ps::join(staging_dir, raw_path);
            match std::fs::read_to_string(&full) {
                Ok(content) => {
                    let actual = sha256_str(&content);
                    if !expected_sha.is_empty() && !actual.eq_ignore_ascii_case(expected_sha) {
                        faults.push(format!(
                            "source_stale: `{raw_path}` SHA-256 changed (pinned {expected_sha}, disk {actual}); re-extract inventory.json"
                        ));
                    }
                }
                Err(_) => {
                    faults.push(format!(
                        "source_stale: pinned file `{raw_path}` is missing or unreadable on disk; re-extract inventory.json"
                    ));
                }
            }
        }
    }

    // Index inventory items & equations by id.
    let mut known_refs: BTreeMap<String, &Value> = BTreeMap::new();
    for eq in list(&inventory["equations"]) {
        let id = s(&eq["id"]);
        if !id.is_empty() {
            known_refs.insert(id, eq);
        }
    }
    for item in list(&inventory["items"]) {
        let id = s(&item["id"]);
        if !id.is_empty() {
            known_refs.insert(id, item);
        }
    }
    for ob in list(&inventory["obligations"]) {
        let id = s(&ob["id"]);
        if !id.is_empty() {
            known_refs.insert(id, ob);
        }
    }

    // 2. anchor_invalid, 3. printed_mismatch, 4. ownership_conflict
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    let mut claimed_obligations: BTreeSet<String> = BTreeSet::new();

    for part in list(&plan["parts"]) {
        let id = s(&part["id"]);
        // Validate `src` spans against pinned file bounds when files are indexed.
        if !file_lines.is_empty() {
            for span_val in pipe(&part["src"]) {
                let span = s(span_val);
                let Some(c) = SRC_SPAN.captures(&span) else {
                    if !span.is_empty() {
                        faults.push(format!(
                            "anchor_invalid: {id}: malformed src span `{span}` (expected file:from-to)"
                        ));
                    }
                    continue;
                };
                let fkey = norm_file_path(&c[1]);
                let from = c[2].parse::<usize>().unwrap_or(0);
                let to = c.get(3).and_then(|m| m.as_str().parse::<usize>().ok()).unwrap_or(from);
                match file_lines.get(&fkey) {
                    None => {
                        faults.push(format!(
                            "anchor_invalid: {id}: src file `{}` is not in pinned inventory.json",
                            &c[1]
                        ));
                    }
                    Some((max_lines, _, _)) => {
                        if from == 0 || to < from || to > *max_lines {
                            faults.push(format!(
                                "anchor_invalid: {id}: src span `{span}` is outside `{}` (1..={max_lines})",
                                &c[1]
                            ));
                        }
                    }
                }
            }
        }

        // Validate `inventory_ref` and ownership uniqueness.
        let inv_ref = s(&part["inventory_ref"]);
        if !inv_ref.is_empty() {
            if !known_refs.contains_key(&inv_ref) {
                faults.push(format!(
                    "anchor_invalid: {id}: inventory_ref `{inv_ref}` does not exist in inventory.json"
                ));
            } else {
                if let Some(prev_owner) = owners.insert(inv_ref.clone(), id.clone()) {
                    faults.push(format!(
                        "ownership_conflict: inventory_ref `{inv_ref}` is claimed by both `{prev_owner}` and `{id}`"
                    ));
                }
                claimed_obligations.insert(inv_ref.clone());
                // Printed label consistency check when both source_label and inventory item label exist.
                let src_label = s(&part["source_label"]);
                if !src_label.is_empty()
                    && let Some(item) = known_refs.get(&inv_ref)
                {
                    let inv_label = s(&item["label"]);
                    if !inv_label.is_empty() && !labels_compatible(&src_label, &inv_label) {
                        faults.push(format!(
                            "printed_mismatch: {id}: source_label `{src_label}` disagrees with pinned inventory label `{inv_label}` ({inv_ref})"
                        ));
                    }
                }
            }
        }

        // Additional claimed obligations listed in `part["obligations"]` or `part["inventory_refs"]`.
        for ob in pipe(&part["obligations"]).into_iter().chain(pipe(&part["inventory_refs"])) {
            let ob_id = s(ob);
            if ob_id.is_empty() {
                continue;
            }
            if !known_refs.contains_key(&ob_id) {
                faults.push(format!(
                    "anchor_invalid: {id}: obligation `{ob_id}` does not exist in inventory.json"
                ));
            } else {
                if let Some(prev_owner) = owners.insert(ob_id.clone(), id.clone()) {
                    faults.push(format!(
                        "ownership_conflict: obligation `{ob_id}` is claimed by both `{prev_owner}` and `{id}`"
                    ));
                }
                claimed_obligations.insert(ob_id);
            }
        }

        // Printed `\tag{n}` + TeX fidelity check on `to` against pinned equations when `inventory` has equations.
        // Scope to the part's pinned `src` file(s) when supplied to prevent restarted `(1)` collisions across sections (Astra H4).
        let to_tex = s(&part["to"]);
        if let Some(tag) = extract_tag(&to_tex) {
            let eqs = list(&inventory["equations"]);
            if !eqs.is_empty() {
                let part_spans: Vec<String> = pipe(&part["src"]).into_iter().map(s).filter(|x| !x.is_empty()).collect();
                let part_files = src_files_of_spans(&part_spans);
                let scoped: Vec<&Value> = if !part_files.is_empty() {
                    eqs.iter()
                        .copied()
                        .filter(|e| part_files.contains(&norm_file_path(&s(&e["file"]))))
                        .collect()
                } else {
                    eqs.clone()
                };
                let pool = if !scoped.is_empty() { scoped } else { eqs };
                let matching: Vec<&Value> = pool
                    .into_iter()
                    .filter(|e| s(&e["tag"]).eq_ignore_ascii_case(&tag))
                    .collect();
                if matching.is_empty() {
                    faults.push(format!(
                        "printed_mismatch: {id}: `to` cites equation tag `({tag})` which does not appear in pinned inventory.json"
                    ));
                } else {
                    let cand_norm = normalize_tex_for_compare(&to_tex);
                    if !cand_norm.is_empty()
                        && !matching.iter().any(|e| normalize_tex_for_compare(&s(&e["tex"])) == cand_norm)
                    {
                        faults.push(format!(
                            "printed_mismatch: {id}: `to` equation with tag `({tag})` does not match the pinned TeX in inventory.json"
                        ));
                    }
                }
            }
        }
    }

    // Explicit dispositions in `plan["dispositions"]`.
    let mut disposed: BTreeSet<String> = BTreeSet::new();
    for d in list(&plan["dispositions"]) {
        let r = s(&d["ref"]);
        if !r.is_empty() && truthy(&d["reason"]) {
            disposed.insert(r);
        }
    }

    // 5. obligation_unsettled: every accepted obligation in inventory.json must be owned or disposed.
    for ob in list(&inventory["obligations"]) {
        if ob["accepted"] == json!(false) {
            continue;
        }
        let ob_id = s(&ob["id"]);
        if ob_id.is_empty() {
            continue;
        }
        if !claimed_obligations.contains(&ob_id) && !disposed.contains(&ob_id) {
            faults.push(format!(
                "obligation_unsettled: accepted obligation `{ob_id}` ({}) has neither an owning part nor an explicit disposition in plan.json",
                s(&ob["label"])
            ));
        }
    }

    faults
}

/// Normalize a display TeX formula for printed-equation fidelity comparison (strips `$$`, `\tag{...}`,
/// `\label{...}`, trailing `(n)`, spacing commands like `\quad`/`\,`, and whitespace, while keeping
/// every mathematical symbol, sign, operator, and subscript intact).
fn normalize_tex_for_compare(tex: &str) -> String {
    let mut s = tex.trim().trim_matches('$').to_string();
    s = TAG_CMD.replace_all(&s, "").into_owned();
    s = TRAILING_EQ_NUM.replace(&s, "").into_owned();
    for cmd in ["\\qquad", "\\quad", "\\displaystyle", "\\,", "\\;", "\\:", "\\!", "\\ "] {
        s = s.replace(cmd, "");
    }
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Check copied display equations with `\tag{n}` in a written Card body against pinned `inventory.json`
/// (both tag existence AND normalized TeX equality, catching sign flips / dropped terms; Astra #5).
/// When `src_files` is non-empty, scopes lookup to those pinned source files to prevent restarted `(1)`
/// collisions across chapters/appendices (Astra H4).
pub(crate) fn card_printed_mismatches_scoped(body: &str, inventory: &Value, src_files: &[String]) -> Vec<String> {
    let eqs = list(&inventory["equations"]);
    if eqs.is_empty() {
        return Vec::new();
    }
    let norm_scope: Vec<String> = src_files.iter().map(|f| norm_file_path(f)).filter(|f| !f.is_empty()).collect();
    let scoped: Vec<&Value> = if !norm_scope.is_empty() {
        eqs.iter()
            .copied()
            .filter(|e| norm_scope.contains(&norm_file_path(&s(&e["file"]))))
            .collect()
    } else {
        eqs.clone()
    };
    let pool = if !scoped.is_empty() { scoped } else { eqs };
    let mut out = Vec::new();
    for formula in zt_check::formal::display_formulas(body) {
        if let Some(tag) = extract_tag(&formula) {
            let matching: Vec<&Value> = pool
                .iter()
                .copied()
                .filter(|e| s(&e["tag"]).eq_ignore_ascii_case(&tag))
                .collect();
            if matching.is_empty() {
                out.push(format!(
                    "printed_mismatch: display equation cites `\\tag{{{tag}}}` which is not in pinned inventory.json"
                ));
            } else {
                let cand_norm = normalize_tex_for_compare(&formula);
                if !cand_norm.is_empty()
                    && !matching
                        .iter()
                        .any(|e| normalize_tex_for_compare(&s(&e["tex"])) == cand_norm)
                {
                    out.push(format!(
                        "printed_mismatch: display equation tagged `({tag})` does not match the pinned TeX in inventory.json"
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn card_printed_mismatches(body: &str, inventory: &Value) -> Vec<String> {
    card_printed_mismatches_scoped(body, inventory, &[])
}

fn label_kind(raw: &str) -> Option<&'static str> {
    let lower = raw.to_lowercase();
    if lower.contains("theorem") || raw.contains("定理") {
        Some("theorem")
    } else if lower.contains("proposition") || raw.contains("命题") {
        Some("proposition")
    } else if lower.contains("lemma") || raw.contains("引理") {
        Some("lemma")
    } else if lower.contains("corollary") || raw.contains("推论") {
        Some("corollary")
    } else if lower.contains("assumption") || raw.contains("假设") || raw.contains("假定") {
        Some("assumption")
    } else if lower.contains("definition") || raw.contains("定义") {
        Some("definition")
    } else if lower.contains("proof") || raw.contains("证明") {
        Some("proof")
    } else {
        None
    }
}

fn labels_compatible(a: &str, b: &str) -> bool {
    let norm = |t: &str| -> String {
        t.chars()
            .filter(|c| !c.is_whitespace())
            .collect::<String>()
            .to_lowercase()
    };
    let na = norm(a);
    let nb = norm(b);
    if na == nb {
        return true;
    }
    // Statement kinds must agree when both are stated (e.g. Lemma 1 != Theorem 1, Astra #5).
    if let (Some(ka), Some(kb)) = (label_kind(a), label_kind(b))
        && ka != kb
    {
        return false;
    }
    // Allow Chinese-English label equivalence when the kind matches and the trailing identifier matches
    // (e.g. "命题 1" vs "Proposition 1", "假设 (B)" vs "Assumption (B)").
    let ident = |t: &str| -> String {
        let mut s = t.to_string();
        for w in [
            "proposition", "theorem", "lemma", "corollary", "assumption", "definition", "proof",
            "命题", "定理", "引理", "推论", "假设", "假定", "定义", "证明",
        ] {
            s = s.replace(w, "");
        }
        s.chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
            .collect()
    };
    let da = ident(&na);
    let db = ident(&nb);
    !da.is_empty() && da == db
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_equations_tags_and_named_items_with_section_paths() {
        let src = "\
# Section 4: Optimal Taxation

Assumption (B): $V(x,y) = -y u_2/u_1$ is increasing in $y$ and bounded on rectangles.

Theorem 1: An allocation is incentive compatible iff $z_n$ is non-decreasing and $0 \\le z_n < n$.

$$
\\frac{du}{dn} = -\\frac{y u_2}{n} \\tag{28}
$$

## Appendix B

Proof of Proposition 1. Substituting (39) yields:
$$
\\hat{\\pi}_{N,t} = \\frac{\\nu}{1-\\nu} \\hat{x}_t \\tag{B.1}
$$
";
        let inv = build_inventory("K/1", Some(&json!(29916)), Some(&json!(100)), Some("abc"), &[("sections/s04.md".into(), src.into())]);
        let eqs = list(&inv["equations"]);
        assert_eq!(eqs.len(), 2, "{eqs:?}");
        assert_eq!(s(&eqs[0]["tag"]), "28");
        assert_eq!(s(&eqs[1]["tag"]), "B.1");
        assert!(s(&eqs[1]["section_path"]).contains("Appendix B"));

        let items = list(&inv["items"]);
        assert!(items.iter().any(|i| s(&i["kind"]) == "assumption" && s(&i["label"]).contains("(B)")), "{items:?}");
        assert!(items.iter().any(|i| s(&i["kind"]) == "theorem" && s(&i["label"]).contains("1")), "{items:?}");
        assert!(items.iter().any(|i| s(&i["kind"]) == "proof" && s(&i["label"]).contains("Proposition 1")), "{items:?}");
    }

    #[test]
    fn catches_all_layer3_inventory_faults() {
        let src = "# Section 3\n\nTheorem 1: Linear identification.\n\n$$ y = f(x) \\tag{3.1} $$\n";
        let mut inv = build_inventory("K/1", None, None, None, &[("sections/s03.md".into(), src.into())]);
        inv["obligations"] = json!([
            {"id": "claim:thm1", "label": "Theorem 1", "accepted": true},
            {"id": "claim:sec34", "label": "§3.4 Unnumbered Technology Identification", "accepted": true}
        ]);
        let plan = json!({
            "level": "formal",
            "parts": [
                {"id": "R1", "inventory_ref": "claim:thm1", "source_label": "Theorem 2", "src": ["sections/s03.md:1-99"]},
                {"id": "R2", "inventory_ref": "claim:thm1", "to": "$$y=g(x) \\tag{99}$$", "src": ["sections/missing.md:1-2"]}
            ]
        });
        let faults = validate_plan(&plan, &inv, None).join("\n");
        assert!(faults.contains("anchor_invalid: R1: src span"), "{faults}");
        assert!(faults.contains("anchor_invalid: R2: src file `sections/missing.md`"), "{faults}");
        assert!(faults.contains("ownership_conflict: inventory_ref `claim:thm1`"), "{faults}");
        assert!(faults.contains("printed_mismatch: R1: source_label `Theorem 2`"), "{faults}");
        assert!(faults.contains("printed_mismatch: R2: `to` cites equation tag `(99)`"), "{faults}");
        assert!(faults.contains("obligation_unsettled: accepted obligation `claim:sec34`"), "{faults}");
    }

    #[test]
    fn astra_5_and_17_tex_fidelity_kind_match_fences_headings_and_unique_ids() {
        let s1 = "\
## Theorem 1
```markdown
$$ fake = 1 \\tag{999} $$
```
$$ y = x \\tag{1} $$ and $$ z = 2x \\tag{2} $$
";
        let s2 = "Theorem 1. Another chapter's theorem.\n$$ w = 3x \\tag{3} $$\n";
        let inv = build_inventory(
            "K/1",
            None,
            None,
            None,
            &[
                ("sections/s01.md".into(), s1.into()),
                ("sections/s02.md".into(), s2.into()),
            ],
        );
        let eqs = list(&inv["equations"]);
        assert_eq!(eqs.len(), 3, "code fence ignored, both inline displays extracted: {eqs:?}");
        assert!(!eqs.iter().any(|e| s(&e["tag"]) == "999"));
        let items = list(&inv["items"]);
        assert_eq!(items.len(), 2, "heading Theorem 1 + second file Theorem 1: {items:?}");
        assert_ne!(s(&items[0]["id"]), s(&items[1]["id"]), "IDs include file stem and never collide");

        // Astra #5: sign flip keeping \tag{1} is caught; matching TeX passes; Lemma 1 != Theorem 1.
        assert!(!card_printed_mismatches("$$y = -x \\tag{1}$$", &inv).is_empty());
        assert!(card_printed_mismatches("$$y = x \\tag{1}$$", &inv).is_empty());
        assert!(!labels_compatible("Lemma 1", "Theorem 1"));
        assert!(labels_compatible("命题 1", "Proposition 1"));
    }

    #[test]
    fn astra_h2_h3_h4_external_tag_same_line_close_file_scoping_empty_files_and_cross_model_conflict() {
        // H4: external `(1)` after closing `$$` + same-line second display after multiline close + restarted `(1)` across files.
        let s1 = "# Sec 1\n$$ y = a x $$ (1)\n$$\nz = b x\n$$ (2) $$ w = c x $$ (3)\n";
        let s2 = "# Appendix\n$$ y = -a x \\tag{1} $$\n";
        let inv = build_inventory(
            "K/1",
            None,
            None,
            None,
            &[
                ("sections/s01.md".into(), s1.into()),
                ("sections/s02.md".into(), s2.into()),
            ],
        );
        let eqs = list(&inv["equations"]);
        assert_eq!(eqs.len(), 4, "all 4 displays extracted including same-line after multiline close: {eqs:?}");
        assert_eq!(s(&eqs[0]["tag"]), "1");
        assert_eq!(s(&eqs[1]["tag"]), "2");
        assert_eq!(s(&eqs[2]["tag"]), "3");
        assert_eq!(s(&eqs[3]["tag"]), "1");

        // Scoped check: card citing s01's tag (1) with s02's sign MUST fail when scoped to s01!
        let mm = card_printed_mismatches_scoped("$$ y = -a x \\tag{1} $$", &inv, &["sections/s01.md".into()]);
        assert!(!mm.is_empty(), "scoped check must catch cross-file restarted (1) mismatch");
        assert!(card_printed_mismatches_scoped("$$ y = a x \\tag{1} $$", &inv, &["sections/s01.md".into()]).is_empty());
        // Spaced `\tag {1}` (MinerU format) must also be parsed as tag 1.
        let mm_spaced = card_printed_mismatches_scoped("$$ y = -a x \\tag {1} $$", &inv, &["sections/s01.md".into()]);
        assert!(!mm_spaced.is_empty(), "spaced \\tag {{1}} must not bypass fidelity check");

        // H2: empty files array must produce inventory_invalid rather than silently skipping spans.
        let empty_inv = json!({"schema_version": 1, "files": [], "equations": [], "items": [], "obligations": []});
        let p = json!({"parts": [{"id": "S0", "src": ["sections/s01.md:1-5"]}]});
        let ef = validate_plan(&p, &empty_inv, None).join("\n");
        assert!(ef.contains("inventory_invalid"), "{ef}");

        // H3: cross-model ownership conflict across sibling plans.
        let p1 = json!({"model": "K/1|a", "parts": [{"id": "R1", "inventory_ref": "eq:s01:1"}]});
        let p2 = json!({"model": "K/2|a", "parts": [{"id": "R2", "inventory_ref": "eq:s01:1"}]});
        let conflicts = cross_plan_ownership_faults(&[("K/1|a".into(), &p1), ("K/2|a".into(), &p2)]);
        assert_eq!(conflicts.len(), 1, "{conflicts:?}");
        assert!(conflicts[0].1.contains("ownership_conflict"));
    }
}
