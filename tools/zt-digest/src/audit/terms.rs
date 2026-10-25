//! The `terms` section of `zt-digest audit` (S26, #169; design 14.4, proposal 10.3).
//!
//! It publishes, per book, the Term list's structure and coverage, each head's math/concept
//! category and auto-link choice, the T faults, Term uses by the using Card's type, the Cards
//! and heads with no Term use, the avoid words still written plainly, the ten most used terms,
//! the run's Term generation from its ledger, and a quality status. The status is separate from
//! the audit's own `complete`: an audit that ran fully can still report `degraded` coverage, and
//! a refused Term write is `refused`, never a silent success (R0441/R0442).
//!
//! Every fact is derived in one pass from the audit's own archive read (`get_terms` plus the
//! bodies it already holds). The section writes nothing and reads no clock.

use crate::audit::Fault;
use crate::ps::{self, eq, field, is_in, key as ci, ps_count, ps_items, truthy};
use rusqlite::OptionalExtension;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

/// The template's category head line (design 11.1; `zt_check::term::HEAD_LABELS` names it too).
const CATEGORY_LABEL: &str = "类别";
/// The Term rules whose fault lines the terms section reports apart (design 10.2).
const T_RULES: [&str; 10] = ["T1", "T2", "T3", "T4", "T5", "T6", "T7", "T8", "T9", "T10"];

/// The facts one book's terms section is built from.
pub(crate) struct Facts<'a> {
    /// The book's one Term list address, when it has one (a `term` Card without a name).
    pub(crate) list: Option<String>,
    /// The archive's computed Term structure for the book, or why it could not be read.
    pub(crate) terms: Result<Value, String>,
    /// Every Card of the book, in the audit's own graph read.
    pub(crate) book: &'a [&'a Value],
    /// `ci(address) → (title, body)` for every Card the audit read.
    pub(crate) bodies: &'a HashMap<String, (String, String)>,
    /// The audit's own category of one address (`book_of::BookIndex`).
    pub(crate) category: &'a dyn Fn(&str) -> String,
    /// Every Card language and Card layer fault line of the audit.
    pub(crate) faults: &'a [Fault],
    /// The run's Term generation read from the staging ledger, or an unavailable reason.
    pub(crate) generation: Value,
}

/// Build one book's `terms` section.
pub(crate) fn build(f: &Facts) -> Value {
    let list = f.list.clone();
    let Some(list) = list else {
        return json!({
            "status": "skipped",
            "reason": "the book has no Term list (a `term` Card without a term name)",
            "list": Value::Null,
            "generation": f.generation,
        });
    };
    let title = f
        .bodies
        .get(&ci(&list))
        .map(|(title, _)| title.clone())
        .unwrap_or_default();
    let terms = match &f.terms {
        Ok(terms) => terms,
        Err(error) => {
            return json!({
                "status": "degraded",
                "reason": format!("the Term structure could not be read: {error}"),
                "list": list,
                "title": title,
                "rows": Value::Null, "published": Value::Null, "missing": Value::Null,
                "heads": Value::Null, "parts": Value::Null, "unfiled": Value::Null,
                "math": Value::Null, "concept": Value::Null, "unclassified": Value::Null,
                "faults": {"total": 0, "by_rule": {}, "by_fault": {}, "list": []},
                "uses": {"total": 0, "cards": 0, "by_type": {}, "by_target": {}},
                "no_use": {"heads": [], "cards": []},
                "avoid": {"count": 0, "list": []},
                "most_used": [],
                "heads_detail": [], "parts_detail": [],
                "generation": f.generation,
            });
        }
    };
    let coverage = &terms["coverage"];
    let rows = ps_items(&terms["rows"]);
    let heads = ps_items(&terms["heads"]);
    let parts = ps_items(&terms["parts"]);
    let unfiled = ps_items(&terms["unfiled"]);
    let uses = ps_items(&terms["uses"]);

    // --- math/concept and per-head facts ------------------------------------------------------
    let mut math = 0usize;
    let mut concept = 0usize;
    let mut unclassified = 0usize;
    let mut head_rows: Vec<Value> = Vec::new();
    for head in &heads {
        let address = field(head, "address");
        let body = f.bodies.get(&ci(&address)).map(|(_, body)| body.clone()).unwrap_or_default();
        let category = head_category(&body);
        match category.as_str() {
            "math" => math += 1,
            "concept" => concept += 1,
            _ => unclassified += 1,
        }
        head_rows.push(json!({
            "address": address,
            "term": field(head, "term"),
            "title": field(head, "title"),
            "category": if category.is_empty() { Value::Null } else { json!(category) },
            "auto_link": !crate::term_link::auto_link_off(&body),
            "parts": ps_count(&head["parts"]),
            "uses": head["uses"].as_i64().unwrap_or(0),
        }));
    }
    let mut part_rows: Vec<Value> = parts
        .iter()
        .map(|part| {
            json!({
                "address": field(part, "address"),
                "term": field(part, "term"),
                "title": field(part, "title"),
                "head": field(part, "head"),
            })
        })
        .collect();
    part_rows.sort_by(|a, b| ps::culture_cmp(&field(a, "address"), &field(b, "address")));

    // --- uses by the using Card's type, and the no-use observations ---------------------------
    let graph: HashMap<String, &Value> = f.book.iter().map(|card| (ci(&field(card, "address")), *card)).collect();
    let mut by_type: BTreeMap<String, i64> = BTreeMap::new();
    let mut use_cards: BTreeSet<String> = BTreeSet::new();
    let mut by_term: BTreeMap<String, i64> = BTreeMap::new();
    for use_row in &uses {
        let count = use_row["count"].as_i64().unwrap_or(0);
        let source = field(use_row, "source");
        use_cards.insert(ci(&source));
        let kind = graph
            .get(&ci(&source))
            .map(|card| field(card, "type"))
            .filter(|kind| !kind.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        *by_type.entry(kind).or_default() += count;
        *by_term.entry(field(use_row, "target")).or_default() += count;
    }
    let mut most_used: Vec<Value> = head_rows
        .iter()
        .filter(|head| head["uses"].as_i64().unwrap_or(0) > 0)
        .map(|head| {
            json!({
                "term": field(head, "term"),
                "address": field(head, "address"),
                "uses": head["uses"],
            })
        })
        .collect();
    most_used.sort_by(|a, b| {
        b["uses"]
            .as_i64()
            .unwrap_or(0)
            .cmp(&a["uses"].as_i64().unwrap_or(0))
            .then_with(|| ps::culture_cmp(&field(a, "term"), &field(b, "term")))
    });
    most_used.truncate(10);
    let no_use_heads: Vec<String> = head_rows
        .iter()
        .filter(|head| head["uses"].as_i64().unwrap_or(0) == 0)
        .map(|head| field(head, "address"))
        .collect();
    // Substantive Cards without any Term use. The list, roots, anchors, hubs, structure and
    // notation Cards are outside the reading, and a Card with no Term use is an observation, not
    // a defect (design 13.7).
    let mut no_use_cards: Vec<String> = Vec::new();
    for card in f.book {
        let address = field(card, "address");
        if !is_in(
            &(f.category)(&address),
            &["idea", "event", "finding", "model", "part", "term"],
        ) {
            continue;
        }
        if !use_cards.contains(&ci(&address)) {
            no_use_cards.push(address);
        }
    }
    no_use_cards.sort_by(|a, b| ps::culture_cmp(a, b));

    // --- the avoid words still written plainly -----------------------------------------------
    let avoid = avoid_observations(&list, &rows, f.book, f.bodies);

    // --- the faults: the T rules, plus the Term structure's own coverage faults ---------------
    let mut faults_by_rule: BTreeMap<String, i64> = BTreeMap::new();
    let mut faults_by_fault: BTreeMap<String, i64> = BTreeMap::new();
    let mut fault_rows: Vec<Value> = Vec::new();
    let book_addresses: HashSet<String> = f.book.iter().map(|card| ci(&field(card, "address"))).collect();
    for fault in f.faults {
        if !book_addresses.contains(&ci(&fault.address)) || !T_RULES.contains(&fault.rule.as_str()) {
            continue;
        }
        *faults_by_rule.entry(fault.rule.clone()).or_default() += 1;
        fault_rows.push(json!({
            "rule": fault.rule,
            "fault": Value::Null,
            "address": fault.address,
            "text": fault.line,
        }));
    }
    let mut structure_faults: Vec<Value> = ps_items(&terms["faults"])
        .iter()
        .map(|fault| {
            let rule = field(fault, "rule");
            let code = field(fault, "fault");
            if !rule.is_empty() {
                *faults_by_rule.entry(rule.clone()).or_default() += 1;
            }
            if !code.is_empty() {
                *faults_by_fault.entry(code.clone()).or_default() += 1;
            }
            let mut row = json!({
                "rule": if rule.is_empty() { Value::Null } else { json!(rule) },
                "fault": if code.is_empty() { Value::Null } else { json!(code) },
                "address": fault_address(&fault),
                "text": fault_text(&fault),
            });
            if let Some(collected) = row.as_object_mut() {
                for (key, value) in fault.as_object().into_iter().flatten() {
                    collected.entry(key.clone()).or_insert_with(|| value.clone());
                }
            }
            row
        })
        .collect();
    let mut fault_rows_collected = std::mem::take(&mut fault_rows);
    fault_rows_collected.append(&mut structure_faults);
    fault_rows_collected.sort_by(|a, b| {
        ps::culture_cmp(&string(&a["address"]), &string(&b["address"]))
            .then_with(|| ps::culture_cmp(&string(&a["rule"]), &string(&b["rule"])))
    });

    // --- the shared mapping fingerprints (the finalization round's invalidation input) --------
    let owned = book_rows(f.book, f.bodies);
    let by_address: BTreeMap<String, &Value> = owned.iter().map(|(key, value)| (key.clone(), value)).collect();
    let fingerprints = match crate::term_link::vocabulary(terms, &by_address) {
        Ok(facts) => json!({
            // The list's authored body hash is the plan's own `fingerprints.list`: the Term list
            // body is where a row change lives.
            "list": f
                .bodies
                .get(&ci(&list))
                .map(|(_, body)| crate::canon::sha256_hex(body.as_bytes()))
                .unwrap_or_default(),
            "mapping": facts.mapping_hash,
            "autolink": facts.autolink_hash,
            "vocabulary": facts.vocabulary_hash,
        }),
        Err(error) => json!({"error": format!("{error}")}),
    };

    // --- the quality status, never the audit's own completion ---------------------------------
    let status = book_status(coverage, &fault_rows_collected, &f.generation);
    let reason = status_reason(&status, coverage, &fault_rows_collected, &f.generation);
    json!({
        "status": status,
        "reason": reason,
        "list": list,
        "title": title,
        "rows": rows.len(),
        "published": coverage["published"].as_i64().unwrap_or(0),
        "missing": coverage["missing"].as_i64().unwrap_or(0),
        "heads": heads.len(),
        "parts": parts.len(),
        "unfiled": unfiled.len(),
        "math": math,
        "concept": concept,
        "unclassified": unclassified,
        "duplicates": ps_count(&terms["row_duplicates"]),
        "faults": {
            "total": fault_rows_collected.len(),
            "by_rule": faults_by_rule.into_iter().map(|(key, value)| (key, json!(value))).collect::<Map<_, _>>(),
            "by_fault": faults_by_fault.into_iter().map(|(key, value)| (key, json!(value))).collect::<Map<_, _>>(),
            "list": fault_rows_collected,
        },
        "uses": {
            "total": uses.iter().map(|row| row["count"].as_i64().unwrap_or(0)).sum::<i64>(),
            "cards": use_cards.len(),
            "by_type": by_type.into_iter().map(|(key, value)| (key, json!(value))).collect::<Map<_, _>>(),
            "by_target": by_term.into_iter().map(|(key, value)| (key, json!(value))).collect::<Map<_, _>>(),
        },
        "no_use": {"heads": no_use_heads, "cards": no_use_cards},
        "avoid": avoid,
        "most_used": most_used,
        "heads_detail": head_rows,
        "parts_detail": part_rows,
        "fingerprints": fingerprints,
        "generation": f.generation,
    })
}

/// The audit's aggregate `workspace.terms` block from the per-book sections.
pub(crate) fn aggregate(books: &[Value]) -> Value {
    let mut with_list = 0usize;
    let mut rows = 0i64;
    let mut published = 0i64;
    let mut missing = 0i64;
    let mut heads = 0i64;
    let mut parts = 0i64;
    let mut unfiled = 0i64;
    let mut math = 0i64;
    let mut concept = 0i64;
    let mut uses = 0i64;
    let mut avoid = 0i64;
    let mut faults = 0i64;
    let mut no_use_heads = 0i64;
    let mut no_use_cards = 0i64;
    let mut statuses: BTreeSet<String> = BTreeSet::new();
    for book in books {
        let terms = &book["terms"];
        let status = field(terms, "status");
        if status.is_empty() || eq(&status, "skipped") {
            continue;
        }
        with_list += 1;
        statuses.insert(status);
        rows += terms["rows"].as_i64().unwrap_or(0);
        published += terms["published"].as_i64().unwrap_or(0);
        missing += terms["missing"].as_i64().unwrap_or(0);
        heads += terms["heads"].as_i64().unwrap_or(0);
        parts += terms["parts"].as_i64().unwrap_or(0);
        unfiled += terms["unfiled"].as_i64().unwrap_or(0);
        math += terms["math"].as_i64().unwrap_or(0);
        concept += terms["concept"].as_i64().unwrap_or(0);
        uses += terms["uses"]["total"].as_i64().unwrap_or(0);
        avoid += terms["avoid"]["count"].as_i64().unwrap_or(0);
        faults += terms["faults"]["total"].as_i64().unwrap_or(0);
        no_use_heads += ps_count(&terms["no_use"]["heads"]) as i64;
        no_use_cards += ps_count(&terms["no_use"]["cards"]) as i64;
    }
    // The worst status names the whole Workspace's Term quality: a refusal, then a degradation;
    // books without a Term list are skipped, not failures.
    let status = if statuses.contains("refused") {
        "refused"
    } else if statuses.contains("degraded") {
        "degraded"
    } else if with_list == 0 {
        "skipped"
    } else {
        "complete"
    };
    json!({
        "status": status,
        "books": books.len(),
        "books_with_list": with_list,
        "rows": rows, "published": published, "missing": missing,
        "heads": heads, "parts": parts, "unfiled": unfiled,
        "math": math, "concept": concept,
        "uses": uses, "avoid": avoid, "faults": faults,
        "no_use_heads": no_use_heads, "no_use_cards": no_use_cards,
    })
}

// --- helpers -------------------------------------------------------------------------------------

/// One head's authored category: `数学概念` is `math`, any other authored category is `concept`
/// (the compiler's own two categories), and no line is empty.
fn head_category(body: &str) -> String {
    let category = zt_check::term::parse_template(body)
        .line(CATEGORY_LABEL)
        .map(|line| line.value.trim().to_string())
        .unwrap_or_default();
    if category == zt_check::term::CATEGORY_MATH {
        "math".to_string()
    } else if category == zt_check::term::CATEGORY_CONCEPT {
        "concept".to_string()
    } else {
        String::new()
    }
}

/// The book's rows as minimal Card values the shared vocabulary derivation can read: its address
/// and authored body (the only fields it needs beside the Term structure's own facts).
fn book_rows(
    book: &[&Value],
    bodies: &HashMap<String, (String, String)>,
) -> BTreeMap<String, Value> {
    let mut rows = BTreeMap::new();
    for card in book {
        let address = field(card, "address");
        let body = bodies.get(&ci(&address)).map(|(_, body)| body.clone()).unwrap_or_default();
        rows.insert(address.clone(), json!({"address": address, "body": body, "type": field(card, "type")}));
    }
    rows
}

/// The avoid words still written in the book's prose, with the Card and line. A Term list's own
/// table, headings, table rows, fenced code, inline code, math, source lines, Term metadata lines
/// and the generated Reverse text are not prose. The scan is literal: it reports an observation,
/// never a defect, and it never writes a word (design 13.7, N4).
fn avoid_observations(
    list: &str,
    rows: &[Value],
    book: &[&Value],
    bodies: &HashMap<String, (String, String)>,
) -> Value {
    let words: Vec<(String, String)> = rows
        .iter()
        .flat_map(|row| {
            let term = field(row, "term");
            row["avoid"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(|word| (term.clone(), word.to_string()))
                .collect::<Vec<_>>()
        })
        .filter(|(_, word)| !word.trim().is_empty())
        .collect();
    let mut total = 0i64;
    let mut occurrences: Vec<Value> = Vec::new();
    if words.is_empty() {
        return json!({"count": 0, "list": []});
    }
    let mut ordered: Vec<&Value> = book.iter().copied().collect();
    ordered.sort_by(|a, b| ps::culture_cmp(&field(a, "address"), &field(b, "address")));
    for card in ordered {
        let address = field(card, "address");
        if eq(&address, list) {
            continue;
        }
        let Some((_, body)) = bodies.get(&ci(&address)) else { continue };
        for (line, text) in prose_lines(body) {
            for (term, word) in &words {
                let mut from = 0usize;
                while let Some(at) = text[from..].find(word.as_str()) {
                    total += 1;
                    occurrences.push(json!({
                        "address": address,
                        "line": line,
                        "term": term,
                        "word": word,
                        "text": text.trim(),
                    }));
                    from += at + word.len();
                }
            }
        }
    }
    json!({"count": total, "list": occurrences})
}

/// The prose lines of one authored body: no fenced code, inline code or math, and none of the
/// structural lines a Term-link pass protects. Line numbers are the body's own 1-based lines.
fn prose_lines(body: &str) -> Vec<(usize, String)> {
    let mut masked = body.as_bytes().to_vec();
    for range in zt_check::text::code_ranges(body)
        .into_iter()
        .chain(zt_check::text::math_ranges(body))
    {
        for byte in &mut masked[range] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    let masked = String::from_utf8_lossy(&masked).into_owned();
    let mut out: Vec<(usize, String)> = Vec::new();
    let mut fenced = false;
    let mut reverse = false;
    for (index, raw) in body.lines().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if raw.contains("<!-- zt:reverse-links -->") {
            reverse = true;
        }
        if reverse {
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with('#')
            || trimmed.starts_with('|')
            || zt_check::term::head_line(trimmed).is_some()
            || trimmed.starts_with(&format!("{}：", zt_check::term::SOURCE_LABEL))
            || trimmed.starts_with("出处：")
        {
            continue;
        }
        let masked_line = masked.lines().nth(index).unwrap_or_default();
        if !masked_line.trim().is_empty() {
            out.push((index + 1, masked_line.to_string()));
        }
    }
    out
}

/// The address a Term-structure fault names, across the shapes `get_terms` publishes.
fn fault_address(fault: &Value) -> String {
    for key in ["address", "source"] {
        let at = field(fault, key);
        if !at.is_empty() {
            return at;
        }
    }
    String::new()
}

/// One readable line of a Term-structure fault.
fn fault_text(fault: &Value) -> String {
    let code = field(fault, "fault");
    let term = field(fault, "term");
    let mut text = code;
    if !term.is_empty() {
        text.push(' ');
        text.push_str(&term);
    }
    text.trim().to_string()
}

fn string(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_string()
}

/// The book's Term quality from its coverage, faults and the run's own generation. It is not the
/// audit's completion: a refused Term write is `refused`, incomplete coverage is `degraded`.
fn book_status(coverage: &Value, faults: &[Value], generation: &Value) -> &'static str {
    if refused_by_generation(generation) {
        return "refused";
    }
    let unfiled = coverage["unfiled"].as_i64().unwrap_or(0);
    let published = coverage["published"].as_i64().unwrap_or(0);
    let rows = coverage["rows"].as_i64().unwrap_or(0);
    if rows == 0 || published < rows || unfiled > 0 || !faults.is_empty() {
        return "degraded";
    }
    if degraded_by_generation(generation) {
        return "degraded";
    }
    "complete"
}

fn status_reason(status: &str, coverage: &Value, faults: &[Value], generation: &Value) -> String {
    match status {
        "refused" => format!("a Term write was refused: {}", generation_reason(generation)),
        "degraded" => {
            let mut reasons: Vec<String> = Vec::new();
            let rows = coverage["rows"].as_i64().unwrap_or(0);
            let published = coverage["published"].as_i64().unwrap_or(0);
            let unfiled = coverage["unfiled"].as_i64().unwrap_or(0);
            if rows == 0 {
                reasons.push("the Term list has no row".to_string());
            }
            if published < rows {
                reasons.push(format!("{}/{} rows publish a Term Card", published, rows));
            }
            if unfiled > 0 {
                reasons.push(format!("{unfiled} unfiled Term Card(s)"));
            }
            if !faults.is_empty() {
                reasons.push(format!("{} T fault(s)", faults.len()));
            }
            let generation = generation_reason(generation);
            if !generation.is_empty() {
                reasons.push(generation);
            }
            reasons.join("; ")
        }
        _ => String::new(),
    }
}

fn refused_by_generation(generation: &Value) -> bool {
    generation_units(generation)
        .into_iter()
        .any(|unit| unit["state"] == json!("failed") && refusal_code(&unit))
}

fn degraded_by_generation(generation: &Value) -> bool {
    generation_units(generation)
        .into_iter()
        .any(|unit| unit["state"] != json!("done") && !refusal_code(&unit))
}

fn refusal_code(unit: &Value) -> bool {
    let code = unit["fault"]["code"].as_str().unwrap_or_default();
    code == "card_check" || code == "card_locked"
}

fn generation_reason(generation: &Value) -> String {
    if generation["available"] != json!(true) {
        return String::new();
    }
    let mut reasons: Vec<String> = Vec::new();
    for unit in generation_units(generation) {
        if unit["state"] == json!("done") {
            continue;
        }
        let id = field(&unit, "unit");
        let state = field(&unit, "state");
        let code = unit["fault"]["code"].as_str().unwrap_or_default();
        reasons.push(if code.is_empty() {
            format!("{id} is {state}")
        } else {
            format!("{id} is {state} ({code})")
        });
    }
    reasons.join(", ")
}

fn generation_units(generation: &Value) -> Vec<Value> {
    let mut units: Vec<Value> = Vec::new();
    for key in ["term_cards", "term_link"] {
        if truthy(&generation[key]) {
            units.push(generation[key].clone());
        }
    }
    units.extend(ps_items(&generation["final_rounds"]));
    units
}

/// Read the run's Term generation from the staging folder's ledger, read-only. A folder that is
/// no prepared run has no ledger; that is a stated fact, never a zero.
pub(crate) fn generation(out: &Path) -> Value {
    let ledger = out.join("ledger.sqlite3");
    if !ledger.is_file() {
        return json!({"available": false, "reason": "the audit folder holds no prepared run ledger"});
    }
    let conn = match crate::ledger::connect(&ledger, false) {
        Ok(conn) => conn,
        Err(error) => {
            return json!({"available": false, "reason": format!("the run ledger could not be read: {error}")});
        }
    };
    let latest = |layer: &str, logical: &str| -> Value {
        let row = conn
            .query_row(
                "SELECT id,state,revision,fault_json,result_json FROM units WHERE layer=?1 AND logical_key=?2 ORDER BY revision DESC LIMIT 1",
                rusqlite::params![layer, logical],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional();
        match row {
            Ok(Some((id, state, revision, fault, result))) => {
                let mut value = json!({"unit": id, "state": state, "revision": revision});
                if let Some(text) = fault
                    && let Ok(parsed) = serde_json::from_str::<Value>(&text)
                {
                    value["fault"] = parsed;
                }
                if let Some(text) = result
                    && let Ok(parsed) = serde_json::from_str::<Value>(&text)
                {
                    value["complete"] = parsed["complete"].clone();
                    value["counts"] = parsed["counts"].clone();
                }
                value
            }
            _ => Value::Null,
        }
    };
    let finals = conn
        .prepare("SELECT id FROM units WHERE layer='termlink' AND logical_key='final' ORDER BY revision")
        .and_then(|mut statement| {
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        })
        .unwrap_or_default();
    let mut final_rounds: Vec<Value> = Vec::new();
    for unit in finals {
        let row = conn
            .query_row(
                "SELECT id,state,revision,fault_json FROM units WHERE id=?1",
                [&unit],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional();
        if let Ok(Some((id, state, revision, fault))) = row {
            let mut value = json!({"unit": id, "state": state, "revision": revision});
            if let Some(text) = fault
                && let Ok(parsed) = serde_json::from_str::<Value>(&text)
            {
                value["fault"] = parsed;
            }
            final_rounds.push(value);
        }
    }
    json!({
        "available": true,
        "term_cards": latest("termcards", "book"),
        "term_link": latest("termlink", "book"),
        "final_rounds": final_rounds,
    })
}

/// The statuses as `(status, reason)` for one book without a Term structure read (a skipped book
/// still states why it is skipped).
pub(crate) fn skipped() -> Value {
    json!({"status": "skipped", "reason": "the book has no Term list"})
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fault(address: &str, rule: &str, line: &str) -> Fault {
        Fault {
            address: address.to_string(),
            rule: rule.to_string(),
            line: line.to_string(),
        }
    }

    fn book(addresses: &[&str]) -> Vec<Value> {
        addresses
            .iter()
            .map(|address| json!({"address": address, "type": "idea"}))
            .collect()
    }

    #[test]
    fn an_authored_category_is_math_or_concept_and_none_without_the_line() {
        assert_eq!(
            head_category(&format!("术语：均衡\n{CATEGORY_LABEL}：数学概念\n")),
            "math"
        );
        assert_eq!(
            head_category(&format!("术语：均衡\n{CATEGORY_LABEL}：概念\n")),
            "concept"
        );
        assert_eq!(head_category("术语：均衡\n"), "");
        assert_eq!(head_category(&format!("术语：均衡\n{CATEGORY_LABEL}：别的\n")), "");
    }

    #[test]
    fn prose_lines_skip_code_math_structure_metadata_and_generated_text() {
        let body = "第一行有均衡。\n\n```\n代码均衡\n```\n\n`行内均衡` 与 $均衡$。\n\n| 表 | 均衡 |\n|---|---|\n| a | b |\n\n## 标题均衡\n\n术语：均衡\n\n来源：[[K]] 均衡\n\n<!-- zt:reverse-links -->\n\n## Term uses\n\n- 均衡\n";
        let lines: Vec<String> = prose_lines(body).into_iter().map(|(_, text)| text).collect();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("第一行有均衡"), "{lines:?}");
        assert!(
            !lines[1].contains("均衡"),
            "the code and math spans are masked before the scan: {lines:?}"
        );
    }

    #[test]
    fn avoid_observations_name_the_card_and_line_and_skip_the_term_list() {
        let mut bodies: HashMap<String, (String, String)> = HashMap::new();
        bodies.insert(
            "k/2|a".to_string(),
            ("观点".to_string(), "第一行。\n平衡在这里。".to_string()),
        );
        bodies.insert(
            "k/1|t".to_string(),
            ("术语表".to_string(), "| 术语 | 不用 |\n| 均衡 | 平衡 |".to_string()),
        );
        let list = json!({"term": "均衡", "avoid": ["平衡"]});
        let cards = book(&["k/2|a", "k/1|t"]);
        let refs: Vec<&Value> = cards.iter().collect();
        let found = avoid_observations("k/1|t", &[list], &refs, &bodies);
        assert_eq!(found["count"], json!(1), "{found}");
        assert_eq!(found["list"][0]["address"], json!("k/2|a"));
        assert_eq!(found["list"][0]["line"], json!(2));
    }

    #[test]
    fn the_quality_status_is_separate_from_the_audits_completion() {
        let complete = json!({"rows": 2, "published": 2, "missing": 0, "unfiled": 0});
        let partial = json!({"rows": 2, "published": 1, "missing": 1, "unfiled": 0});
        let none = json!({"rows": 0, "published": 0, "missing": 0, "unfiled": 0});
        let no_generation = json!({"available": false});
        assert_eq!(book_status(&complete, &[], &no_generation), "complete");
        assert_eq!(book_status(&partial, &[], &no_generation), "degraded");
        assert_eq!(book_status(&none, &[], &no_generation), "degraded");
        assert_eq!(book_status(&complete, &[json!({"rule": "T3"})], &no_generation), "degraded");
        let refused = json!({
            "available": true,
            "term_link": {"unit": "termlink/book@1", "state": "failed", "fault": {"code": "card_check"}},
            "final_rounds": [],
        });
        assert_eq!(book_status(&complete, &[], &refused), "refused");
        let waiting = json!({
            "available": true,
            "term_cards": {"unit": "termcards/book@1", "state": "failed", "fault": {"code": "incomplete"}},
            "final_rounds": [],
        });
        assert_eq!(book_status(&complete, &[], &waiting), "degraded");
    }

    #[test]
    fn the_workspace_aggregate_keeps_skipped_books_out_of_the_quality() {
        let complete = json!({"status": "complete", "rows": 2, "published": 2, "heads": 2, "parts": 1,
            "uses": {"total": 3}, "avoid": {"count": 1}, "faults": {"total": 0},
            "no_use": {"heads": [], "cards": ["k/2"]}});
        let skipped = json!({"status": "skipped"});
        let books = vec![json!({"terms": complete}), json!({"terms": skipped})];
        let all = aggregate(&books);
        assert_eq!(all["status"], json!("complete"));
        assert_eq!(all["books_with_list"], json!(1));
        assert_eq!(all["rows"], json!(2));
        assert_eq!(all["uses"], json!(3));
        assert_eq!(all["no_use_cards"], json!(1));
        let degraded = vec![json!({"terms": {"status": "degraded"}}), json!({"terms": skipped})];
        assert_eq!(aggregate(&degraded)["status"], json!("degraded"));
        assert_eq!(aggregate(&[json!({"terms": skipped})])["status"], json!("skipped"));
    }
}
