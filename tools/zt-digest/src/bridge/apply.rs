//! The apply half of the Bridge compiler: `Current-Context` (the Cards an apply touches, read
//! fast), `Selection-States`, `Verify-Attempt`, and `Apply-Selected` (the journal in state.json,
//! guarded edits in one batch, one read after all writes, verification in write order).

use super::canon::{self, refuse, sort_unique_cs};
use super::{Bridge, body_text, ceq, ceqv, cin, endpoint, field_of, get, message, zt_check_json};
use crate::audit::book_of::{self, BookIndex};
use crate::calls;
use crate::ps::{self, ps_array, ps_items, s, truthy};
use anyhow::{Result, bail};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

static OK_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+) ok ").unwrap());
static BEFORE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^before L[12]\b").unwrap());
static AFTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^after L[12]\b").unwrap());
static BEFORE_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^before ").unwrap());
static AFTER_PREFIX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^after ").unwrap());

/// What `Current-Context` returns.
pub(super) struct Context {
    pub index: BookIndex,
    /// address -> get_tree summary.
    pub summaries: HashMap<String, Value>,
    /// address -> get_card answer.
    pub cards: HashMap<String, Value>,
}

/// The ids of an attempt (`$attempt.ids`).
fn ids_of(attempt: &Value) -> Vec<String> {
    ps_items(get(attempt, "ids")).iter().map(s).collect()
}

/// `Expected-Fingerprint`: the intended fingerprint of the last verified attempt on the Card,
/// else its snapshot's.
fn expected_fingerprint(snapshot: &Value, attempts: &[Value]) -> Value {
    let address = get(snapshot, "address");
    match attempts.iter().filter(|a| ceqv(get(a, "carrier"), address) && ceq(get(a, "status"), "verified")).last() {
        Some(a) => get(a, "intended_fingerprint").clone(),
        None => get(snapshot, "fingerprint").clone(),
    }
}

/// `Expected-Book`.
fn expected_book(snapshot: &Value, attempts: &[Value]) -> Value {
    let address = get(snapshot, "address");
    match attempts
        .iter()
        .filter(|a| ceqv(get(a, "carrier"), address) && ceq(get(a, "status"), "verified") && a.get("intended_book").is_some())
        .last()
    {
        Some(a) => get(a, "intended_book").clone(),
        None => get(snapshot, "book").clone(),
    }
}

/// `Body-Fingerprint`: the fingerprint the snapshot would have with `body`.
fn body_fingerprint(snapshot: &Value, body: &str) -> String {
    let mut values: Vec<Value> = ["workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title"].iter().map(|k| get(snapshot, k).clone()).collect();
    values.push(json!(body_text(&json!(body))));
    format!("sha256:{}", canon::hash_value(&Value::Array(values)))
}

/// `Model-Shape`: the members of a Card's model and its build faults.
fn model_shape(model: &Value) -> Value {
    if !truthy(model) {
        return json!({ "members": [], "faults": [] });
    }
    let members = sort_unique_cs(ps_array(get(model, "parts")).iter().map(|p| field_of(p, "address")));
    let mut faults: Vec<String> = Vec::new();
    for kind in ["closure", "reach_in", "forward", "unanchored", "beyond_level"] {
        for fault in ps_array(&model["build"][kind]) {
            if fault.is_null() {
                continue;
            }
            faults.push(format!("{kind}|{}|{}|{}|{}", field_of(&fault, "from"), field_of(&fault, "to"), field_of(&fault, "unit"), field_of(&fault, "address")));
        }
    }
    json!({ "members": members, "faults": faults })
}

/// `@($value.Keys)`: an object's keys; anything else is one null.
fn keys_of(value: &Value) -> Value {
    match value {
        Value::Object(map) => json!(map.keys().collect::<Vec<_>>()),
        _ => json!([null]),
    }
}

/// `$object.($name)`: a property, its name compared without case.
fn property<'a>(value: &'a Value, name: &str) -> &'a Value {
    match value.as_object() {
        Some(map) => map.get(name).or_else(|| map.iter().find(|(k, _)| k.to_lowercase() == name.to_lowercase()).map(|(_, v)| v)).unwrap_or(&Value::Null),
        None => &Value::Null,
    }
}

impl Bridge {
    /// `Current-Context`: the tree index once per run, the trees that hold the addresses (a Card's
    /// category and inferred book depend only on its own tree, its own Links, and the list of
    /// Literature trees), and the Cards themselves in one batched read.
    pub(super) fn current_context(&mut self, addresses: &[String]) -> Result<Context> {
        if self.tree_index.is_none() {
            let read = self.read_zt(&[json!({ "tool": "list_trees", "arguments": {} })])?;
            self.tree_index = Some(ps_array(&read.first().cloned().unwrap_or(Value::Null)["trees"]));
        }
        let trees = self.tree_index.clone().unwrap_or_default();
        let known: HashSet<String> = trees.iter().map(|t| field_of(t, "tree")).collect();
        let mut wanted: Vec<String> = Vec::new();
        for address in addresses {
            let tree = address.split('/').next().unwrap_or_default().to_string();
            if known.contains(&tree) && !wanted.contains(&tree) {
                wanted.push(tree);
            }
        }
        let calls: Vec<Value> = wanted.iter().map(|t| json!({ "tool": "get_tree", "arguments": { "tree": t } })).collect();
        let details = self.read_zt(&calls)?;
        let summaries: Vec<Value> = details.iter().flat_map(|d| ps_items(get(d, "cards"))).collect();
        let index = book_of::book_index(&trees, &summaries);
        let mut by_address: HashMap<String, Value> = HashMap::new();
        for card in &summaries {
            by_address.insert(field_of(card, "address"), card.clone());
        }
        let present: Vec<String> = addresses.iter().filter(|a| by_address.contains_key(*a)).cloned().collect();
        let mut cards: HashMap<String, Value> = HashMap::new();
        let reads = if present.is_empty() {
            None
        } else {
            let calls: Vec<Value> = present.iter().map(|a| json!({ "tool": "get_card", "arguments": { "at": a } })).collect();
            self.read_zt(&calls).ok()
        };
        match reads {
            Some(reads) if reads.len() == present.len() => {
                for (address, card) in present.iter().zip(reads) {
                    cards.insert(address.clone(), card);
                }
            }
            _ => {
                // One read failed: read each Card alone, so the failure names its Card.
                for address in &present {
                    if let Ok(read) = self.read_zt(&[json!({ "tool": "get_card", "arguments": { "at": address } })]) {
                        cards.insert(address.clone(), read.into_iter().next().unwrap_or(Value::Null));
                    }
                }
            }
        }
        Ok(Context { index, summaries: by_address, cards })
    }

    /// `Selection-States`: per accepted Bridge, ready, stale, already_linked, or verified.
    pub(super) fn selection_states(&self, chosen: &[Value], published: &Value, state: &Value, ctx: &Context) -> Vec<Value> {
        let attempts = ps_items(get(state, "attempts"));
        let outcomes = get(state, "outcomes");
        let snapshots = get(published, "cards");
        let mut results = Vec::new();
        for bridge in chosen {
            let (carrier, target) = (field_of(bridge, "carrier"), field_of(bridge, "target"));
            let mut status = "ready".to_string();
            let mut detail = String::new();
            let mut expected: HashMap<String, Value> = HashMap::new();
            let mut actual: HashMap<String, Value> = HashMap::new();
            for address in [&carrier, &target] {
                let Some(card) = ctx.cards.get(address) else {
                    status = "stale".into();
                    detail = format!("missing endpoint {address}");
                    continue;
                };
                let snapshot = snapshots.get(address).cloned().unwrap_or(Value::Null);
                let now = endpoint(card, &ctx.index);
                for name in ["workspace", "tree", "kind", "type", "facet", "emphasis", "level", "form", "title"] {
                    if !ceqv(get(&now, name), get(&snapshot, name)) {
                        status = "stale".into();
                        detail = format!("changed endpoint {address} {name}");
                    }
                }
                if !ceqv(get(&now, "book"), &expected_book(&snapshot, &attempts)) {
                    status = "stale".into();
                    detail = format!("changed endpoint {address} book");
                }
                expected.insert(address.clone(), snapshot);
                actual.insert(address.clone(), now);
            }
            let outcome = |status: &str, detail: &str| {
                json!({ "id": get(bridge, "id"), "carrier": get(bridge, "carrier"), "target": get(bridge, "target"), "status": status, "detail": detail })
            };
            if status == "stale" {
                results.push(outcome(&status, &detail));
                continue;
            }
            let verified = |id: &str| outcomes.get(id).is_some_and(|o| ceq(get(o, "status"), "verified"));
            let journal = verified(&field_of(bridge, "id"));
            let known_pair = ps_items(get(published, "proposals"))
                .iter()
                .any(|p| verified(&field_of(p, "id")) && field_of(p, "carrier") == carrier && field_of(p, "target") == target);
            let linked = ps_array(get(&actual[&carrier], "links")).iter().any(|l| ceq(l, &target))
                || ps_array(get(&actual[&target], "links")).iter().any(|l| ceq(l, &carrier));
            if linked && !journal && !known_pair {
                status = "already_linked".into();
                detail = "An existing Link is preserved; this is not evidence of an applied explanation.".into();
            } else {
                for address in [&carrier, &target] {
                    if !ceqv(get(&actual[address], "fingerprint"), &expected_fingerprint(&expected[address], &attempts)) {
                        status = "stale".into();
                        detail = format!("changed body or endpoint {address}");
                    }
                }
                if status == "ready" && journal {
                    status = "verified".into();
                    detail = "Previously verified accepted paragraph.".into();
                }
            }
            results.push(outcome(&status, &detail));
        }
        results
    }

    /// `Verify-Attempt`: the persisted carrier, its Relations, the Reverse links, the Card checks,
    /// and the model boundaries. Returns the carrier's inferred book (`intended_book`).
    fn verify(&self, attempt: &Value, published: &Value, attempts: &[Value], ctx: &Context) -> Result<Value> {
        let carrier = field_of(attempt, "carrier");
        let Some(card) = ctx.cards.get(&carrier) else { bail!("written carrier cannot be reread") };
        let current = endpoint(card, &ctx.index);
        if !ceqv(get(&current, "fingerprint"), get(attempt, "intended_fingerprint")) {
            bail!("persisted carrier differs from saved intent");
        }
        let cards = get(published, "cards");
        let before = cards.get(&carrier).cloned().unwrap_or(Value::Null);
        let after_kinds = match ctx.summaries.get(&carrier).map(|c| get(c, "kinds")) {
            Some(Value::Object(map)) => json!(map.keys().collect::<Vec<_>>()),
            _ => json!([null]),
        };
        let drafts = json!([
            { "address": "before", "title": get(&before, "title"), "body": get(&before, "body"), "category": "idea", "parent": "",
              "links": ps_array(get(&before, "links")), "kinds": keys_of(get(&before, "kinds")) },
            { "address": "after", "title": get(&current, "title"), "body": get(&current, "body"), "category": "idea", "parent": "",
              "links": ps_array(get(&current, "links")), "kinds": after_kinds },
        ]);
        let file = ps::join_path(&self.work, &format!("tmp\\changed-card-{}.json", std::process::id()));
        canon::save_json(&file, &drafts)?;
        let checks = zt_check_json(&file, "")?;
        let old: Vec<String> = checks.iter().filter(|l| BEFORE.is_match(l)).map(|l| BEFORE_PREFIX.replace(l, "").into_owned()).collect();
        for fault in checks.iter().filter(|l| AFTER.is_match(l)).map(|l| AFTER_PREFIX.replace(l, "").into_owned()) {
            if !old.contains(&fault) {
                bail!("new changed-Card fault: {fault}");
            }
        }
        let proposals = ps_items(get(published, "proposals"));
        let kinds = ctx.summaries.get(&carrier).map(|c| get(c, "kinds").clone()).unwrap_or(Value::Null);
        for id in ids_of(attempt) {
            let bridge = proposals.iter().find(|p| ceq(get(p, "id"), &id)).cloned().unwrap_or(Value::Null);
            let target_at = field_of(&bridge, "target");
            let Some(target_card) = ctx.cards.get(&target_at) else { bail!("target cannot be reread: {target_at}") };
            let target = endpoint(target_card, &ctx.index);
            let snapshot = cards.get(&target_at).cloned().unwrap_or(Value::Null);
            if !ceqv(get(&target, "fingerprint"), &expected_fingerprint(&snapshot, attempts)) {
                bail!("target changed: {target_at}");
            }
            if !ps_array(get(&current, "links")).iter().any(|l| ceq(l, &target_at)) || !ceqv(property(&kinds, &target_at), get(&bridge, "kind")) {
                bail!("persisted Relation kind or Link differs from accepted sentence");
            }
            if !ps_array(get(target_card, "inbound")).iter().any(|i| ceq(get(i, "address"), &carrier)) {
                bail!("generated Reverse link is missing");
            }
            let view = json!({ "cards": cards, "term_tables": get(published, "term_tables") });
            let mut map = bridge.as_object().cloned().unwrap_or_default();
            if !super::answer::sentence_faults(&self.work, &mut map, &view)?.is_empty() {
                bail!("accepted sentence no longer passes the Card checks");
            }
        }
        let mut addresses = vec![carrier.clone()];
        addresses.extend(ps_array(get(attempt, "targets")).iter().map(s));
        for address in addresses {
            let saved = attempt.get("model_before").and_then(|m| m.get(&address));
            let baseline = match saved {
                Some(shape) => shape.clone(),
                None => model_shape(&cards.get(&address).cloned().unwrap_or(Value::Null)["model"]),
            };
            let actual = model_shape(&ctx.cards.get(&address).cloned().unwrap_or(Value::Null)["model"]);
            if canon::canonical_json(get(&baseline, "members")) != canon::canonical_json(get(&actual, "members")) {
                bail!("model membership changed at {address}");
            }
            let known = ps_array(get(&baseline, "faults"));
            for fault in ps_items(get(&actual, "faults")) {
                if !known.iter().any(|k| ceqv(k, &fault)) {
                    bail!("new model fault at {address}: {}", s(&fault));
                }
            }
        }
        // Accepted Links can change the heuristic vote. Remember only a fully verified
        // result, alongside the exact intended body fingerprint, for later retries.
        Ok(get(&current, "book").clone())
    }

    /// `Verify-Attempt` on attempt `i` of the state, then its status and outcomes.
    fn verify_into(&self, state: &mut Value, i: usize, published: &Value, ctx: &Context, ok_detail: &str, keep_failure: bool) -> Option<String> {
        let attempts = ps_items(get(state, "attempts"));
        match self.verify(&attempts[i], published, &attempts, ctx) {
            Ok(book) => {
                state["attempts"][i]["intended_book"] = book;
                set_attempt(state, i, "verified", ok_detail);
                None
            }
            Err(e) => {
                if !keep_failure {
                    set_attempt(state, i, "written_unverified", &message(&e));
                }
                Some(message(&e))
            }
        }
    }

    fn save_state(&self, state: &Value) -> Result<()> {
        canon::save_json(&ps::join_path(&self.work, "state.json"), state)
    }

    /// `Apply-Selected`: under the report lock, reconcile saved intents, save one intent per
    /// carrier, write all carriers in one guarded batch (a carrier that is also a target first),
    /// read once, and verify in write order.
    pub(super) fn apply_selected(&mut self, chosen: &[Value], published: &Value) -> Result<Value> {
        // FileShare.None is the lock. An abandoned file is safe to reopen after process exit.
        let _lock = open_lock(&ps::join_path(&self.work, "apply.lock")).map_err(|_| refuse("another apply owns this report lock"))?;
        let mut state = self.get_state(published)?;
        if state.get("accepted").is_none() {
            state["accepted"] = json!({ "new": [], "earlier": [] });
        }
        for section in ["new", "earlier"] {
            let mut ids: Vec<String> = ps_items(&state["accepted"][section]).iter().filter(|v| !v.is_null()).map(s).collect();
            ids.extend(chosen.iter().filter(|b| ceq(get(b, "section"), section)).map(|b| field_of(b, "id")));
            state["accepted"][section] = json!(sort_unique_cs(ids));
        }
        let mut addresses: Vec<String> = Vec::new();
        for bridge in chosen {
            for at in [field_of(bridge, "carrier"), field_of(bridge, "target")] {
                if !addresses.contains(&at) {
                    addresses.push(at);
                }
            }
        }
        let mut ctx = self.current_context(&addresses)?;
        let chosen_carriers: Vec<String> = chosen.iter().map(|b| field_of(b, "carrier")).collect();
        let chosen_ids: Vec<String> = chosen.iter().map(|b| field_of(b, "id")).collect();
        // Reconcile saved intents before considering another paragraph on the same carrier.
        let open: Vec<usize> = ps_items(get(&state, "attempts"))
            .iter()
            .enumerate()
            .filter(|(_, a)| !cin(get(a, "status"), &["verified", "abandoned"]))
            .map(|(i, _)| i)
            .collect();
        for i in open {
            let attempt = state["attempts"][i].clone();
            let carrier = field_of(&attempt, "carrier");
            if !chosen_carriers.contains(&carrier) {
                continue;
            }
            let now = ctx.cards.get(&carrier).map(|c| endpoint(c, &ctx.index));
            let now_fp = now.as_ref().map(|n| get(n, "fingerprint").clone());
            let is = |key: &str| now_fp.as_ref().is_some_and(|fp| ceqv(fp, get(&attempt, key)));
            if ids_of(&attempt).iter().any(|id| !chosen_ids.contains(id)) {
                if is("expected_fingerprint") {
                    state["attempts"][i]["status"] = json!("abandoned");
                    set_outcomes(&mut state, i, "open", "A saved intent was never written and the new selection differs; it was dropped.");
                } else if is("intended_fingerprint") {
                    self.verify_into(&mut state, i, published, &ctx, "Recovered persisted accepted paragraph.", false);
                } else {
                    set_attempt(&mut state, i, "conflict", "A saved intent holds ids outside this selection, and the carrier changed.");
                }
                continue;
            }
            if is("intended_fingerprint") {
                self.verify_into(&mut state, i, published, &ctx, "Recovered persisted accepted paragraph.", false);
            } else if now.is_none() || !is("expected_fingerprint") {
                set_attempt(&mut state, i, "conflict", "Current carrier matches neither saved expected nor intended content.");
            } else {
                state["attempts"][i]["status"] = json!("prepared");
            }
        }
        // Prepare every carrier's intent from the one read above. No write happened since it.
        let snapshots = get(published, "cards").clone();
        let mut prepared: Vec<usize> = Vec::new();
        for carrier in sort_unique_cs(chosen_carriers.iter().cloned()) {
            let mut group: Vec<Value> = chosen.iter().filter(|b| field_of(b, "carrier") == carrier).cloned().collect();
            ps::ps_sort(&mut group, |a, b| canon::culture_cmp_cs(&field_of(a, "id"), &field_of(b, "id")));
            let outcomes = self.selection_states(&group, published, &state, &ctx);
            let pending: Vec<usize> = ps_items(get(&state, "attempts"))
                .iter()
                .enumerate()
                .filter(|(_, a)| field_of(a, "carrier") == carrier && !cin(get(a, "status"), &["verified", "abandoned"]))
                .map(|(i, _)| i)
                .collect();
            if pending.iter().any(|&i| cin(&state["attempts"][i]["status"], &["conflict", "written_unverified"])) {
                // C9. Every accepted id gets an outcome: a blocked carrier names the reason.
                for bridge in &group {
                    let id = field_of(bridge, "id");
                    let known = state["outcomes"].get(&id).is_some_and(|o| cin(get(o, "status"), &["verified", "conflict", "written_unverified"]));
                    if !known {
                        state["outcomes"][&id] = json!({ "id": get(bridge, "id"), "carrier": carrier, "target": get(bridge, "target"), "status": "blocked",
                                                         "detail": "The carrier has an earlier conflict or unverified write; resolve it first." });
                    }
                }
                continue;
            }
            for outcome in outcomes {
                let id = field_of(&outcome, "id");
                state["outcomes"][id] = outcome;
            }
            let mut ready: Vec<Value> = group.iter().filter(|b| ceq(&state["outcomes"][field_of(b, "id")]["status"], "ready")).cloned().collect();
            if let Some(&last) = pending.last() {
                let saved = ids_of(&state["attempts"][last]);
                for bridge in ready.iter().filter(|b| !saved.contains(&field_of(b, "id"))) {
                    state["outcomes"][field_of(bridge, "id")] = json!({ "id": get(bridge, "id"), "carrier": carrier, "status": "open",
                                                                         "detail": "Resume the saved intent first; apply this additional id afterward." });
                }
                ready.retain(|b| saved.contains(&field_of(b, "id")));
            }
            if ready.is_empty() {
                continue;
            }
            let current = ctx.cards.get(&carrier).cloned().unwrap_or(Value::Null);
            let mut body = body_text(get(&current, "body"));
            for bridge in &ready {
                body.push_str(&format!("\n\n跨书关联：{}", field_of(bridge, "sentence")));
            }
            let i = match pending.last() {
                Some(&last) => last,
                None => {
                    let targets: Vec<Value> = ready.iter().map(|b| get(b, "target").clone()).collect();
                    let snapshot = snapshots.get(&carrier).cloned().unwrap_or(Value::Null);
                    let mut attempt = Map::new();
                    attempt.insert("carrier".into(), json!(carrier));
                    attempt.insert("ids".into(), json!(ready.iter().map(|b| get(b, "id").clone()).collect::<Vec<_>>()));
                    attempt.insert("targets".into(), json!(targets));
                    attempt.insert("expected_fingerprint".into(), get(&endpoint(&current, &ctx.index), "fingerprint").clone());
                    attempt.insert("intended_body".into(), json!(body));
                    attempt.insert("intended_fingerprint".into(), json!(body_fingerprint(&snapshot, &body)));
                    attempt.insert("status".into(), json!("prepared"));
                    // The model shape of the carrier and targets just before the edit: verification compares with it.
                    let mut before = Map::new();
                    for address in std::iter::once(carrier.clone()).chain(targets.iter().map(s)) {
                        if let Some(card) = ctx.cards.get(&address) {
                            before.insert(address, model_shape(get(card, "model")));
                        }
                    }
                    attempt.insert("model_before".into(), Value::Object(before));
                    if !state["attempts"].is_array() {
                        state["attempts"] = Value::Array(ps_items(&state["attempts"]));
                    }
                    let list = state["attempts"].as_array_mut().expect("attempts is an array");
                    list.push(Value::Object(attempt));
                    list.len() - 1
                }
            };
            set_outcomes(&mut state, i, "prepared", "Saved intent; no write verified yet.");
            prepared.push(i);
        }
        self.save_state(&state)?;
        if !prepared.is_empty() {
            // A carrier that is also a target of another accepted Bridge goes first: an edit of its
            // source regenerates its Reverse links, which would change its raw text before its own edit.
            let carrier_of = |state: &Value, i: usize| field_of(&state["attempts"][i], "carrier");
            let targeted: HashSet<String> = prepared.iter().flat_map(|&i| ps_array(&state["attempts"][i]["targets"]).iter().map(s).collect::<Vec<_>>()).collect();
            let mut queue: Vec<usize> = prepared.iter().copied().filter(|&i| targeted.contains(&carrier_of(&state, i))).collect();
            queue.extend(prepared.iter().copied().filter(|&i| !targeted.contains(&carrier_of(&state, i))));
            let order = queue.clone();
            let mut written: Vec<usize> = Vec::new();
            let mut tries: HashMap<String, i64> = HashMap::new();
            while !queue.is_empty() {
                // Each edit carries the carrier's raw text, read just now, as base: ZT refuses an edit to a
                // Card that changed since. The helper runs the calls in order and stops at the first failure.
                let reads: Vec<Value> = queue.iter().map(|&i| json!({ "tool": "get_card", "arguments": { "at": carrier_of(&state, i) } })).collect();
                let raw = self.read_zt(&reads)?;
                let allow = ps::join_path(&self.work, "tmp\\accepted-carriers.allow.txt");
                let list: Vec<String> = queue.iter().map(|&i| carrier_of(&state, i)).collect();
                std::fs::write(&allow, list.join("\n") + "\n").map_err(|e| anyhow::anyhow!("{e}: {allow}"))?;
                let edits: Vec<Value> = queue
                    .iter()
                    .enumerate()
                    .map(|(k, &i)| {
                        let base = raw.get(k).map(|r| get(r, "text").clone()).unwrap_or(Value::Null);
                        json!({ "tool": "edit_card", "arguments": { "at": carrier_of(&state, i), "body": get(&state["attempts"][i], "intended_body"), "base": base } })
                    })
                    .collect();
                let mut options = calls::Options::new().allow_file(&allow)?;
                let (done, failure) = calls::run(self.zt()?, &edits, &mut options)?;
                let mut lines: Vec<String> = done.into_iter().map(|d| d.line).collect();
                if let Some(failure) = &failure {
                    lines.extend(failure.lines());
                }
                let ok: HashSet<String> = lines.iter().filter_map(|l| OK_LINE.captures(l).map(|c| c[1].to_string())).collect();
                let fail_at = (0..queue.len()).find(|k| !ok.contains(&k.to_string()));
                let end = fail_at.unwrap_or(queue.len());
                written.extend(queue[..end].iter().copied());
                let Some(fail_at) = fail_at else { break };
                let failed = queue[fail_at];
                let mut rest: Vec<usize> = queue[fail_at + 1..].to_vec();
                let carrier = carrier_of(&state, failed);
                let tried = tries.entry(carrier.clone()).or_insert(0);
                *tried += 1;
                let tried = *tried;
                let detail = format!("guarded edit was not confirmed: {}", lines.join(" | "));
                let fp = match self.read_zt(&[json!({ "tool": "get_card", "arguments": { "at": carrier } })]) {
                    Ok(read) => s(get(&endpoint(&read.into_iter().next().unwrap_or(Value::Null), &ctx.index), "fingerprint")),
                    Err(_) => String::new(),
                };
                let attempt = &state["attempts"][failed];
                if !fp.is_empty() && ceq(get(attempt, "intended_fingerprint"), &fp) {
                    written.push(failed);
                } else if !fp.is_empty() && ceq(get(attempt, "expected_fingerprint"), &fp) && tried < 2 {
                    rest.insert(0, failed);
                } else if !fp.is_empty() {
                    set_attempt(&mut state, failed, "conflict", &detail);
                } else {
                    set_attempt(&mut state, failed, "written_unverified", &detail);
                }
                queue = rest;
                self.save_state(&state)?;
            }
            // One read after all writes, then verification in write order. A check that failed because
            // a target carrier was not verified yet runs once more after the others.
            if !written.is_empty() {
                ctx = self.current_context(&addresses)?;
                let done = "Persisted text, Relation, Reverse link, and model boundaries verified.";
                let mut again: Vec<usize> = Vec::new();
                for &i in order.iter().filter(|i| written.contains(i)) {
                    if self.verify_into(&mut state, i, published, &ctx, done, true).is_some() {
                        again.push(i);
                    }
                }
                for i in again {
                    self.verify_into(&mut state, i, published, &ctx, done, false);
                }
            }
            self.save_state(&state)?;
        }
        let unfinished = state["outcomes"]
            .as_object()
            .is_some_and(|m| m.values().any(|o| cin(get(o, "status"), &["stale", "conflict", "prepared", "written_unverified", "open", "blocked"])));
        state["status"] = json!(if unfinished { "partial" } else { "done" });
        self.save_state(&state)?;
        self.render_report(published, &state)?;
        Ok(state)
    }
}

/// `$attempt.status = …; Set-Outcomes …`.
fn set_attempt(state: &mut Value, i: usize, status: &str, detail: &str) {
    state["attempts"][i]["status"] = json!(status);
    set_outcomes(state, i, status, detail);
}

/// `Set-Outcomes`: one outcome per id of the attempt.
fn set_outcomes(state: &mut Value, i: usize, status: &str, detail: &str) {
    let attempt = state["attempts"][i].clone();
    if !state["outcomes"].is_object() {
        state["outcomes"] = json!({});
    }
    for id in ps_items(get(&attempt, "ids")) {
        state["outcomes"][s(&id)] = json!({ "id": id, "carrier": get(&attempt, "carrier"), "status": status, "detail": detail });
    }
}

/// `[IO.File]::Open(path, OpenOrCreate, ReadWrite, FileShare.None)`.
fn open_lock(path: &str) -> std::io::Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0);
    }
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_shapes_and_expected_values() {
        assert_eq!(model_shape(&Value::Null), json!({ "members": [], "faults": [] }));
        let model = json!({ "parts": [{ "address": "b" }, { "address": "a" }, { "address": "b" }],
                            "build": { "closure": [{ "from": "a", "to": "x" }], "forward": null } });
        assert_eq!(model_shape(&model), json!({ "members": ["a", "b"], "faults": ["closure|a|x||"] }));
        let snapshot = json!({ "address": "0/1", "fingerprint": "sha256:a", "book": "New" });
        let attempts = vec![
            json!({ "carrier": "0/1", "status": "verified", "intended_fingerprint": "sha256:b", "intended_book": "Old" }),
            json!({ "carrier": "0/1", "status": "written_unverified", "intended_fingerprint": "sha256:c" }),
        ];
        assert_eq!(expected_fingerprint(&snapshot, &attempts), json!("sha256:b"));
        assert_eq!(expected_book(&snapshot, &attempts), json!("Old"));
        assert_eq!(expected_fingerprint(&snapshot, &attempts[1..]), json!("sha256:a"));
        assert_eq!(property(&json!({ "Old/1|A": "rests-on" }), "old/1|a"), &json!("rests-on"));
        assert_eq!(keys_of(&Value::Null), json!([null]));
    }
}
