//! The report half of the Bridge compiler: `Publish-Report` (proposals.json), `Load-Published`,
//! `Get-State`, `Render-Report` (report.md), `Report-Summary` (the last line), and
//! `Select-Proposals` (an accepted selection, checked again).

use super::answer::{self, Checked};
use super::canon::{self, ordinal_cmp, refuse, sort_unique_cs};
use super::{Bridge, ceq, field_of, get, message, ne_one};
use crate::ps::{self, ps_array, ps_count, ps_items, s, truthy};
use anyhow::Result;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::sync::LazyLock;

static PROPOSAL_ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^b[0-9]{3}$").unwrap());
const KIND_ORDER: [&str; 7] = ["corresponds", "contrasts", "illustrates", "relaxes", "objects", "rests-on", "specializes"];

/// `Markdown-Cell`.
pub fn cell(value: &Value) -> String {
    s(value).replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('|', "\\|").replace('\r', " ").replace('\n', " ")
}

fn cell_str(text: &str) -> String {
    cell(&json!(text))
}

fn row(cells: &[Value]) -> String {
    format!("| {} |", cells.iter().map(cell).collect::<Vec<_>>().join(" | "))
}

impl Bridge {
    /// `Publish-Report`: checks every job answer of the frozen plan, numbers the proposals, and
    /// writes the immutable proposals.json.
    pub(super) fn publish_report(&mut self, plan: &Value) -> Result<Value> {
        let mut proposals: Vec<Value> = Vec::new();
        let mut refused: Vec<Value> = Vec::new();
        let mut outcomes = Map::new();
        for entry in ps_items(get(plan, "sources")) {
            let address = get(&entry, "address").clone();
            let with = ps_count(get(&entry, "candidates")) > 0;
            outcomes.insert(
                s(&address),
                json!({ "source": address, "outcome": if with { "failed" } else { "no_candidates" },
                        "why": if with { "No valid job output." } else { "No eligible candidate." } }),
            );
        }
        let mut completed = 0;
        for assignment in ps_items(get(plan, "jobs")) {
            let id = get(&assignment, "id").clone();
            let path = ps::join_path(&self.work, &format!("out\\{}.json", s(&id)));
            if !ps::exists(&path) {
                refused.push(json!({ "job": id, "fault": "missing job output" }));
                continue;
            }
            let check = match canon::read_json(&path).and_then(|a| answer::test_answer(&self.work, &a, plan, &assignment)) {
                Ok(check) => check,
                Err(e) => Checked { faults: vec![format!("BR1 {}", message(&e))], proposals: Vec::new(), none: Vec::new() },
            };
            for fault in &check.faults {
                refused.push(json!({ "job": id, "fault": fault }));
            }
            proposals.extend(check.proposals.iter().cloned());
            for entry in &check.none {
                outcomes.insert(field_of(entry, "source"), json!({ "source": get(entry, "source"), "outcome": "none", "why": get(entry, "why") }));
            }
            if !check.proposals.is_empty() || !check.none.is_empty() {
                completed += 1;
            }
            for source in ps_items(get(&assignment, "sources")) {
                let key = s(&source);
                if check.proposals.iter().any(|p| ceq(get(p, "source"), &key)) {
                    outcomes.insert(key, json!({ "source": source, "outcome": "proposed", "why": "" }));
                } else if !ceq(&outcomes.get(&key).map(|o| o["outcome"].clone()).unwrap_or(Value::Null), "none") {
                    outcomes.insert(key, json!({ "source": source, "outcome": "refused", "why": check.faults.join(" | ") }));
                }
            }
        }
        // One sentence per directed pair: a second, different sentence refuses both.
        let mut pairs: Vec<(String, usize, bool)> = Vec::new();
        for (i, bridge) in proposals.iter().enumerate() {
            let key = format!("{}\0{}", field_of(bridge, "carrier"), field_of(bridge, "target"));
            match pairs.iter_mut().find(|(k, _, _)| *k == key) {
                None => pairs.push((key, i, false)),
                Some((_, first, conflict)) => {
                    let prior = &proposals[*first];
                    if field_of(prior, "sentence") != field_of(bridge, "sentence") || field_of(prior, "word") != field_of(bridge, "word") {
                        *conflict = true;
                        refused.push(json!({ "job": "report", "fault": format!("conflicting sentences for {} -> {}", field_of(bridge, "carrier"), field_of(bridge, "target")) }));
                    }
                }
            }
        }
        let mut ordered: Vec<(String, Value)> = pairs
            .iter()
            .filter(|(_, _, conflict)| !conflict)
            .map(|(_, i, _)| {
                let b = &proposals[*i];
                let key = ["source", "carrier", "target", "word", "sentence"].map(|k| field_of(b, k)).join("\0");
                (key, b.clone())
            })
            .collect();
        ordered.sort_by(|a, b| ordinal_cmp(&a.0, &b.0));
        let cards = get(plan, "cards");
        let mut published: Vec<Value> = Vec::new();
        for (n, (_, mut bridge)) in ordered.into_iter().enumerate() {
            let carrier_fp = get(cards.get(field_of(&bridge, "carrier")).unwrap_or(&Value::Null), "fingerprint").clone();
            let target_fp = get(cards.get(field_of(&bridge, "target")).unwrap_or(&Value::Null), "fingerprint").clone();
            let map = bridge.as_object_mut().expect("a proposal is an object");
            map.insert("id".into(), json!(format!("b{:03}", n + 1)));
            map.insert("carrier_fingerprint".into(), carrier_fp);
            map.insert("target_fingerprint".into(), target_fp);
            published.push(bridge);
        }
        for entry in outcomes.values_mut() {
            if ceq(get(entry, "outcome"), "proposed") && !published.iter().any(|b| ceq(get(b, "source"), &field_of(entry, "source"))) {
                entry["outcome"] = json!("refused");
                entry["why"] = json!("Conflicting pair was refused.");
            }
        }
        let section_count = |name: &str| published.iter().filter(|b| ceq(get(b, "section"), name)).count();
        let counts_in = get(plan, "counts");
        let mut counts = Map::new();
        counts.insert("eligible".into(), get(counts_in, "eligible").clone());
        counts.insert("selected".into(), get(counts_in, "selected").clone());
        counts.insert("omitted".into(), get(counts_in, "omitted").clone());
        counts.insert("with_candidates".into(), json!(ps_count(get(plan, "sources")) as i64 - ps_count(get(plan, "no_candidates")) as i64));
        counts.insert("proposals".into(), json!(published.len()));
        counts.insert("new".into(), json!(section_count("new")));
        counts.insert("earlier".into(), json!(section_count("earlier")));
        counts.insert("refused".into(), json!(refused.len()));
        for class in ["proposed", "none", "no_candidates", "refused", "failed"] {
            counts.insert(format!("{class}_sources"), json!(outcomes.values().filter(|o| ceq(get(o, "outcome"), class)).count()));
        }
        let status = if ps_count(get(plan, "jobs")) > 0 && completed == 0 {
            "failed"
        } else if !refused.is_empty() {
            "partial"
        } else {
            "done"
        };
        let mut payload = Map::new();
        payload.insert("schema".into(), json!(1));
        payload.insert("plan_id".into(), get(plan, "plan_id").clone());
        payload.insert("citation_key".into(), json!(self.key));
        payload.insert("book_title".into(), json!(s(get(plan, "book_title"))));
        for (to, from) in [("profile", "profile"), ("options", "options"), ("read_coverage", "read_coverage"), ("empty_reason", "empty_reason"), ("cards", "cards"), ("term_tables", "term_tables"), ("plan_sources", "sources")] {
            payload.insert(to.into(), get(plan, from).clone());
        }
        payload.insert("proposals".into(), Value::Array(published));
        payload.insert("sources".into(), Value::Array(outcomes.into_values().collect()));
        payload.insert("omitted_sources".into(), Value::Array(ps_array(get(plan, "omitted_sources"))));
        payload.insert("refused".into(), Value::Array(refused));
        payload.insert("counts".into(), Value::Object(counts));
        payload.insert("status".into(), json!(status));
        let mut payload = Value::Object(payload);
        let id = format!("r-{}", canon::content_hash(&payload, &["report_id", "created_at"]));
        payload["report_id"] = json!(id);
        payload["created_at"] = json!(canon::utc_now_o());
        canon::save_json(&self.proposal_file, &payload)?;
        Ok(payload)
    }

    /// `Load-Published`.
    pub(super) fn load_published(&self) -> Result<Value> {
        let value = canon::read_json(&self.proposal_file)?;
        let id = format!("r-{}", canon::content_hash(&value, &["report_id", "created_at"]));
        if ne_one(get(&value, "schema")) || !ceq(get(&value, "citation_key"), &self.key) || !ceq(get(&value, "report_id"), &id) {
            return Err(refuse("invalid published report schema, Citation key, or fingerprint"));
        }
        Ok(value)
    }

    /// `Get-State`: the apply journal of the report (a new one when there is no state.json).
    pub(super) fn get_state(&self, published: &Value) -> Result<Value> {
        let file = ps::join_path(&self.work, "state.json");
        if !ps::exists(&file) {
            return Ok(json!({ "schema": 1, "report_id": get(published, "report_id"), "accepted": { "new": [], "earlier": [] },
                              "outcomes": {}, "attempts": [], "status": "done" }));
        }
        let state = canon::read_json(&file)?;
        if ne_one(get(&state, "schema")) || !super::ceqv(get(&state, "report_id"), get(published, "report_id")) {
            return Err(refuse("apply state belongs to a different report"));
        }
        Ok(state)
    }

    /// `Render-Report`: report.md.
    pub(super) fn render_report(&self, published: &Value, state: &Value) -> Result<()> {
        let mut lines: Vec<String> = Vec::new();
        let key = get(published, "citation_key");
        let title = get(published, "book_title");
        let book = if truthy(title) { format!("{}（{}）", s(title), s(key)) } else { s(key) };
        let options = get(published, "options");
        lines.push(format!("# 跨书桥接报告：{book}"));
        lines.push(String::new());
        lines.push(format!("Plan: {}", field_of(published, "plan_id")));
        lines.push(format!("Report: {}", field_of(published, "report_id")));
        lines.push(format!(
            "Generated: {}. Profile: {}. Source limit: {}. Similarity candidates: {}, plus at most two term matches.",
            field_of(published, "created_at"),
            field_of(published, "profile"),
            field_of(options, "max_sources"),
            field_of(options, "candidates")
        ));
        lines.push("Retrieval uses title twice plus excerpt, existing 80-term TF-IDF, and literal preferred terms. This is a bounded search, not an exhaustive semantic search. Attribution is inferred at planning time.".into());
        lines.push(format!("Status: {}. Counts: {}.", field_of(published, "status"), ps::json_compact(get(published, "counts"))));
        lines.push(format!("Read coverage: {}.", ps::json_compact(get(published, "read_coverage"))));
        if truthy(get(published, "empty_reason")) {
            lines.push(field_of(published, "empty_reason"));
        }
        lines.push("Acceptance is for these exact sentences and this report id. New and earlier carriers require separate selections. No acceptance is inferred from this report or a timer.".into());
        lines.push(String::new());
        lines.push("## 按目标书".into());
        lines.push(String::new());
        lines.push(format!("| Target book or bucket | Proposals | {} |", KIND_ORDER.join(" | ")));
        lines.push(format!("| --- | --- |{}", " --- |".repeat(KIND_ORDER.len())));
        let cards = get(published, "cards");
        let card = |at: &str| cards.get(at).cloned().unwrap_or(Value::Null);
        let proposals = ps_items(get(published, "proposals"));
        let mut groups: Vec<(String, i64, Vec<(String, i64)>)> = Vec::new();
        for bridge in &proposals {
            let other = if ceq(get(bridge, "section"), "earlier") { card(&field_of(bridge, "carrier")) } else { card(&field_of(bridge, "target")) };
            let bucket = if ceq(get(&other, "workspace"), "human") {
                "human".to_string()
            } else if truthy(get(&other, "book")) {
                field_of(&other, "book")
            } else {
                "unattributed".to_string()
            };
            let at = match groups.iter().position(|g| g.0 == bucket) {
                Some(at) => at,
                None => {
                    groups.push((bucket, 0, Vec::new()));
                    groups.len() - 1
                }
            };
            let group = &mut groups[at];
            group.1 += 1;
            let kind = field_of(bridge, "kind");
            match group.2.iter_mut().find(|(k, _)| ps::key(k) == ps::key(&kind)) {
                Some((_, n)) => *n += 1,
                None => group.2.push((kind, 1)),
            }
        }
        groups.sort_by(|a, b| ordinal_cmp(&a.0, &b.0));
        for (bucket, total, kinds) in &groups {
            let counts: Vec<String> = KIND_ORDER.iter().map(|k| kinds.iter().find(|(x, _)| ps::key(x) == *k).map_or(0, |(_, n)| *n).to_string()).collect();
            lines.push(format!("| {} | {total} | {} |", cell_str(bucket), counts.join(" | ")));
        }
        let outcomes = get(state, "outcomes");
        for section in ["new", "earlier"] {
            lines.push(String::new());
            lines.push(if section == "new" { "## 写入新书卡片的提议" } else { "## 写入旧书卡片的提议（需要单独同意）" }.into());
            lines.push(String::new());
            lines.push("| id | Source | Carrier | Relation | Target | Exact sentence | Reason | State |".into());
            lines.push("| --- | --- | --- | --- | --- | --- | --- | --- |".into());
            for bridge in proposals.iter().filter(|b| ceq(get(b, "section"), section)) {
                let (carrier, target) = (card(&field_of(bridge, "carrier")), card(&field_of(bridge, "target")));
                let id = field_of(bridge, "id");
                let status = match outcomes.get(&id) {
                    Some(o) => get(o, "status").clone(),
                    None => json!("open"),
                };
                let source = field_of(bridge, "source");
                lines.push(row(&[
                    get(bridge, "id").clone(),
                    json!(format!("{source} {}", field_of(&card(&source), "title"))),
                    json!(format!("{} {}", field_of(bridge, "carrier"), field_of(&carrier, "title"))),
                    get(bridge, "word").clone(),
                    json!(format!("{} {}", field_of(bridge, "target"), field_of(&target, "title"))),
                    get(bridge, "sentence").clone(),
                    get(bridge, "why").clone(),
                    status,
                ]));
            }
        }
        lines.extend(["", "## 没有桥接的源卡", "", "| Source | Outcome | Reason |", "| --- | --- | --- |"].map(String::from));
        for entry in ps_items(get(published, "sources")) {
            if !ceq(get(&entry, "outcome"), "proposed") {
                let source = field_of(&entry, "source");
                let title = cards.get(&source).map(|c| field_of(c, "title")).unwrap_or_default();
                lines.push(row(&[json!(format!("{source} {title}")), get(&entry, "outcome").clone(), get(&entry, "why").clone()]));
            }
        }
        lines.extend(["", "## 因上限而未选的源卡", "", "| Source | Title | Reason |", "| --- | --- | --- |"].map(String::from));
        for entry in ps_items(get(published, "omitted_sources")) {
            if truthy(&entry) {
                lines.push(format!("| {} | {} | Source cap |", cell(get(&entry, "address")), cell(get(&entry, "title"))));
            }
        }
        lines.extend(["", "## 拒绝和失败", "", "| Job | Fault |", "| --- | --- |"].map(String::from));
        for entry in ps_items(get(published, "refused")) {
            lines.push(format!("| {} | {} |", cell(get(&entry, "job")), cell(get(&entry, "fault"))));
        }
        lines.extend(["", "## 应用与未完成的验证", "", "| id | Carrier | State | Detail |", "| --- | --- | --- | --- |"].map(String::from));
        if let Some(map) = outcomes.as_object() {
            for (id, o) in map {
                lines.push(row(&[json!(id), get(o, "carrier").clone(), get(o, "status").clone(), get(o, "detail").clone()]));
            }
        }
        let text = lines.join("\n") + "\n";
        std::fs::write(ps::join_path(&self.work, "report.md"), text)?;
        Ok(())
    }

    /// `Report-Summary`: the last line of report and apply.
    pub(super) fn report_summary(&self, published: &Value, state: &Value, what: &str) -> Value {
        let status = if ceq(get(published, "status"), "done") { get(state, "status").clone() } else { get(published, "status").clone() };
        let state_outcomes = get(state, "outcomes");
        let outcomes: Vec<Value> = ps_items(get(published, "proposals"))
            .iter()
            .map(|bridge| match state_outcomes.get(field_of(bridge, "id")) {
                Some(o) => o.clone(),
                None => json!({ "id": get(bridge, "id"), "carrier": get(bridge, "carrier"), "target": get(bridge, "target"),
                                "status": "open", "detail": "Not accepted or applied." }),
            })
            .collect();
        let mut by_status: Vec<(String, i64)> = Vec::new();
        for o in &outcomes {
            let k = field_of(o, "status");
            match by_status.iter_mut().find(|(x, _)| ps::key(x) == ps::key(&k)) {
                Some((_, n)) => *n += 1,
                None => by_status.push((k, 1)),
            }
        }
        let by_status: Map<String, Value> = by_status.into_iter().map(|(k, n)| (k, json!(n))).collect();
        let changed = sort_unique_cs(
            ps_items(get(state, "attempts"))
                .iter()
                .filter(|a| super::cin(get(a, "status"), &["verified", "written_unverified"]))
                .map(|a| field_of(a, "carrier")),
        );
        let few = outcomes.len() <= 30;
        let mut out = Map::new();
        out.insert("ok".into(), json!(super::cin(&status, &["done", "skipped"])));
        out.insert("step".into(), json!(what));
        out.insert("status".into(), status);
        out.insert("citation_key".into(), json!(self.key));
        out.insert("plan_id".into(), get(published, "plan_id").clone());
        out.insert("report_id".into(), get(published, "report_id").clone());
        out.insert("counts".into(), get(published, "counts").clone());
        out.insert("outcome_counts".into(), Value::Object(by_status));
        out.insert("outcomes".into(), Value::Array(if few { outcomes } else { Vec::new() }));
        out.insert("outcomes_in".into(), json!(if few { "" } else { "report.md and state.json" }));
        out.insert("changed_carriers".into(), json!(changed));
        out.insert("report".into(), json!(ps::join_path(&self.work, "report.md")));
        Value::Object(out)
    }

    /// `Select-Proposals`: the accepted proposals (-Accept for new carriers, -AcceptEarlier for
    /// earlier ones), bound to -ReportId and checked again as one answer.
    pub(super) fn select_proposals(&self, published: &Value) -> Result<Vec<Value>> {
        if (!self.accept.is_empty() || !self.accept_earlier.is_empty() || self.apply) && !ceq(get(published, "report_id"), &self.report_id) {
            return Err(refuse("acceptance must name this exact published report id"));
        }
        let proposals = ps_items(get(published, "proposals"));
        let mut chosen: Vec<Value> = Vec::new();
        let mut ids: HashSet<String> = HashSet::new();
        for section in ["new", "earlier"] {
            let input = if section == "new" { &self.accept } else { &self.accept_earlier };
            if input.is_empty() {
                continue;
            }
            let values: Vec<String> = if input == "all" {
                proposals.iter().filter(|p| ceq(get(p, "section"), section)).map(|p| field_of(p, "id")).collect()
            } else {
                input.split(',').map(|v| v.trim().to_string()).collect()
            };
            for id in values {
                if !PROPOSAL_ID.is_match(&id) || !ids.insert(id.clone()) {
                    return Err(refuse("invalid or repeated accepted proposal id"));
                }
                let found: Vec<&Value> = proposals.iter().filter(|p| ceq(get(p, "id"), &id) && ceq(get(p, "section"), section)).collect();
                if found.len() != 1 {
                    return Err(refuse("unknown id or id in the wrong acceptance section"));
                }
                chosen.push(found[0].clone());
            }
        }
        if chosen.is_empty() && (self.apply || !self.report_id.is_empty()) {
            return Err(refuse("a report id or write switch requires a nonempty accepted selection"));
        }
        if !chosen.is_empty() {
            let sources = sort_unique_cs(chosen.iter().map(|b| field_of(b, "source")));
            let answer_sources: Vec<Value> = sources
                .iter()
                .map(|source| json!({ "source": source, "bridges": chosen.iter().filter(|b| ceq(get(b, "source"), source)).cloned().collect::<Vec<_>>(), "none": "" }))
                .collect();
            let answer = json!({ "schema": 1, "plan_id": get(published, "plan_id"), "job": "apply", "sources": answer_sources });
            let view = json!({ "plan_id": get(published, "plan_id"), "citation_key": get(published, "citation_key"), "profile": get(published, "profile"),
                               "options": get(published, "options"), "cards": get(published, "cards"), "term_tables": get(published, "term_tables"),
                               "sources": get(published, "plan_sources") });
            let checked = answer::test_answer(&self.work, &answer, &view, &json!({ "id": "apply", "sources": sources }))?;
            if !checked.faults.is_empty() {
                return Err(refuse(format!("accepted proposals fail validation: {}", checked.faults.join(" | "))));
            }
        }
        Ok(chosen)
    }
}
