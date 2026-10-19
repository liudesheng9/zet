//! S05 (#98): the bounded earlier-Card context pack published before a digest group starts.
//!
//! The pack answers one question for the group writer: which Cards written before this group are
//! the likeliest earlier thought to link, to place beside, or to refuse to duplicate? It reuses the
//! existing character-bigram TF-IDF (`card::relatedness`) and the existing bounded `get_cards`
//! text read, and it starts no retrieval framework of its own.
//!
//! Two stages keep the read bounded: the title catalogue (already exported for the group snapshot)
//! ranks the whole archive lexically, then only the top [`POOL`] addresses - plus the target Cards
//! of accepted register terms this group touches - are read in one bounded text batch and ranked
//! again on their bodies. The result is frozen evidence in the group's startup job, so a resumed
//! group reads the same pack, and nothing here writes a Card.

use crate::canon;
use crate::card::relatedness::Relatedness;
use crate::ps::s;
use crate::zt::Zt;
use anyhow::{Result, bail};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// The pack offers at most this many earlier Cards (D091, D286).
pub(crate) const CANDIDATES: usize = 20;
/// Fewer is valid and is reported, never padded with unrelated Cards (D095).
pub(crate) const ENOUGH: usize = 10;
/// One excerpt never exceeds this many Unicode characters (D095).
pub(crate) const EXCERPT_CHARS: usize = 800;
/// The pack never exceeds this many excerpt characters in total (D095, D286).
pub(crate) const PACK_CHARS: usize = 16_000;
/// How many title-ranked addresses are read for their bodies (one bounded text batch).
pub(crate) const POOL: usize = 60;
/// The title stage reports its match count up to this many addresses.
pub(crate) const MATCH_CAP: usize = 200;
/// Register targets added to the read pool beyond the title-ranked pool.
pub(crate) const REGISTER_ADD: usize = 40;
/// Weighted terms kept per document, as the existing relatedness callers do.
pub(crate) const KEEP: usize = 150;
/// The query's own source text is cut here before it joins the term index.
pub(crate) const SOURCE_CHARS: usize = 6_000;
/// Term meanings join the query up to this many characters.
pub(crate) const TERM_CHARS: usize = 4_000;
/// Proposed claims join the query up to this many characters each.
pub(crate) const CLAIM_CHARS: usize = 2_000;
/// A pseudo-document id of the group's own query in the first stage is not used: the query is
/// ranked against the corpus by `Relatedness::rank_text`, so it never joins the document count.

/// What the group asks: its title, its selected source, the book's term meanings and the claims it
/// proposes to write when a prepared skeleton names them (D090).
pub(crate) struct Query {
    pub(crate) title: String,
    pub(crate) source: String,
    pub(crate) terms: String,
    pub(crate) claims: Vec<String>,
}

impl Query {
    /// The one query text the term index sees. Each part is cut, so a long source cannot crowd out
    /// the title or the meanings.
    fn text(&self) -> String {
        let mut text = String::new();
        text.push_str(&self.title);
        text.push('\n');
        text.push_str(&clip(&self.source, SOURCE_CHARS).0);
        text.push('\n');
        text.push_str(&clip(&self.terms, TERM_CHARS).0);
        for claim in &self.claims {
            text.push('\n');
            text.push_str(&clip(claim, CLAIM_CHARS).0);
        }
        text
    }

    fn facts(&self, text: &str) -> Value {
        json!({
            "title": self.title,
            "source_chars": chars(&self.source),
            "terms_chars": chars(&self.terms),
            "claims": self.claims,
            "chars": chars(text),
        })
    }
}

/// The accepted register entries this group's text names, and how the register read went.
pub(crate) struct Register {
    pub(crate) facts: Value,
    /// entry Card address -> the accepted terms whose target it is.
    pub(crate) targets: BTreeMap<String, Vec<String>>,
}

/// `get_register` for the agent Workspace: the register is an ordinary read, so a missing,
/// duplicated or unreadable register is reported rather than substituted (D089).
fn read_register(zt: &mut Zt, text: &str) -> Register {
    let answer = match zt.call("get_register", json!({"workspace":"agent"})) {
        Ok(answer) => answer,
        Err(error) => {
            return Register {
                facts: json!({"status":"failed","fault":{"code":"register_read_failed","detail":format!("{error:#}")}}),
                targets: BTreeMap::new(),
            };
        }
    };
    if answer["register_missing"] == json!(true) {
        return Register {
            facts: json!({"status":"absent","fault":"register_missing"}),
            targets: BTreeMap::new(),
        };
    }
    if let Some(many) = answer["duplicate_registers"].as_array()
        && !many.is_empty()
    {
        return Register {
            facts: json!({"status":"ambiguous","fault":"duplicate","registers":many}),
            targets: BTreeMap::new(),
        };
    }
    let mut targets: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut accepted = 0u64;
    let mut matched = 0u64;
    for entry in answer["entries"].as_array().into_iter().flatten() {
        if entry["complete"] != json!(true) {
            continue;
        }
        let term = s(&entry["term"]);
        if term.is_empty() || !text.contains(&term) {
            continue;
        }
        matched += 1;
        for target in entry["targets"].as_array().into_iter().flatten() {
            let address = s(&target["address"]);
            if address.is_empty() || target["exists"] != json!(true) {
                continue;
            }
            let terms = targets.entry(address).or_default();
            if !terms.iter().any(|seen| seen == &term) {
                terms.push(term.clone());
            }
            accepted += 1;
        }
    }
    for terms in targets.values_mut() {
        terms.sort_by(|a, b| crate::ps::culture_cmp(a, b));
    }
    Register {
        facts: json!({
            "status":"read",
            "card": answer["register"],
            "title": answer["title"],
            "archive_revision": answer["archive_revision"],
            "entries": answer["entry_total"],
            "terms_matched": matched,
            "targets": accepted,
        }),
        targets,
    }
}

/// The earlier-context request of one group.
pub(crate) struct Request {
    pub(crate) unit: String,
    pub(crate) group: u64,
    pub(crate) query: Query,
    /// Addresses the selection never offers: the group's own anchor and every group of this book
    /// that has not completed yet (D094).
    pub(crate) exclude: BTreeSet<String>,
}

/// The published pack: its immutable JSON, the writer-facing Markdown, and the retained facts.
pub(crate) struct Pack {
    pub(crate) json: Value,
    pub(crate) markdown: String,
    pub(crate) revision: u64,
    pub(crate) candidates: usize,
}

impl Pack {
    /// The compact facts the group-start answer carries, so an operator sees the pack without
    /// opening it.
    pub(crate) fn facts(&self) -> Value {
        json!({
            "candidates": self.json["count"],
            "matches": self.json["catalog"]["matches"],
            "excerpt_chars": self.json["excerpt_chars"],
            "truncated": self.json["truncated"],
            "register": self.json["register"]["status"],
            "archive_revision": self.revision,
        })
    }
}

/// Build the pack from the group's already-read title catalogue and one bounded body read.
///
/// `trees` is the startup snapshot's own `export_titles` result, so the pack reuses that read
/// instead of scanning titles a second time (D096). Every read must report the snapshot's archive
/// revision: a read set that crosses revisions is retried once and then reported as
/// `read_view_changed`, never mixed (AC04).
pub(crate) fn build(
    zt: &mut Zt,
    trees: &[Value],
    revision: u64,
    identity: &Value,
    request: &Request,
) -> Result<Pack> {
    let catalog = rows(trees);
    let text = request.query.text();
    let register = read_register(zt, &text);
    let ranked = rank_titles(&catalog, &text, &request.exclude, &register.targets);
    let mut attempt = 0;
    let cards = loop {
        attempt += 1;
        let read = zt.cards(&ranked.read, "text")?;
        let after = zt.archive_view()?;
        let coherent = read.archive_revision == revision
            && after["archive_revision"].as_u64() == Some(revision)
            && after["archive_identity"] == *identity;
        if coherent {
            break read.cards;
        }
        if attempt > 1 {
            bail!(
                "read_view_changed: the earlier-Card context could not be read from archive revision {revision}; retry the group start"
            );
        }
    };
    let mut bodies: BTreeMap<String, Value> = BTreeMap::new();
    for card in cards {
        bodies.insert(s(&card["address"]), card);
    }
    let missing: Vec<&String> = ranked.read.iter().filter(|at| !bodies.contains_key(*at)).collect();
    if !missing.is_empty() {
        bail!("incomplete earlier-Card read: no Card for {}", missing.iter().map(|at| at.as_str()).collect::<Vec<_>>().join(", "));
    }
    let order = rank_bodies(&catalog, &bodies, &text, &ranked.read);
    let pack = assemble(
        request,
        &catalog,
        &bodies,
        &register,
        &ranked,
        &order,
        revision,
        identity,
    );
    Ok(pack)
}

/// The title-catalogue stage: the whole archive ranked by title vocabulary, in one index build.
struct Ranked {
    /// The read set: the title-ranked pool, then the register's own targets.
    read: Vec<String>,
    /// Title-vocabulary matches across the archive (capped at [`MATCH_CAP`]).
    matches: usize,
    capped: bool,
    /// How many title-ranked matches the pool could not read.
    below_pool: usize,
    /// Register targets that the catalogue does not hold, or that the read pool could not take.
    register_unread: Vec<String>,
    /// How many Cards the term index held.
    corpus: usize,
    /// `tfidf` for the weighted ranking, `overlap` when the corpus was too uniform to separate.
    ranking: &'static str,
}

fn rank_titles(
    catalog: &[Value],
    query: &str,
    exclude: &BTreeSet<String>,
    register: &BTreeMap<String, Vec<String>>,
) -> Ranked {
    let documents = catalog_documents(catalog, exclude);
    let model = Relatedness::new(&documents, KEEP);
    let mut found = model.rank_text(query, MATCH_CAP, |id| exclude.contains(id));
    let mut ranking = "tfidf";
    if found.is_empty() && !documents.is_empty() {
        // Every shared term was universal: a term that every document uses carries no weight and is
        // dropped, so a small or homogeneous archive answers by raw term overlap instead.
        found = model.overlap(query, MATCH_CAP, |id| exclude.contains(id));
        ranking = "overlap";
    }
    let capped = found.len() >= MATCH_CAP;
    let matches = found.len();
    let mut read: Vec<String> = found.iter().take(POOL).cloned().collect();
    let mut register_unread = Vec::new();
    let mut added = 0usize;
    for address in register.keys() {
        if read.iter().any(|at| at == address) {
            continue;
        }
        if !catalog.iter().any(|row| s(&row["address"]) == *address) {
            register_unread.push(address.clone());
            continue;
        }
        if added >= REGISTER_ADD {
            register_unread.push(address.clone());
            continue;
        }
        read.push(address.clone());
        added += 1;
    }
    Ranked {
        read,
        matches,
        capped,
        below_pool: matches.saturating_sub(POOL),
        register_unread,
        corpus: documents.len(),
        ranking,
    }
}

/// The body stage: the read set ranked again on title plus bounded body, so an earlier Card whose
/// title says little but whose body says much is not lost.
fn rank_bodies(
    catalog: &[Value],
    bodies: &BTreeMap<String, Value>,
    query: &str,
    read: &[String],
) -> Vec<String> {
    let mut docs: Vec<(String, String)> = Vec::new();
    for address in read {
        let Some(card) = bodies.get(address) else { continue };
        let row = catalog.iter().find(|row| s(&row["address"]) == *address);
        let title = match row {
            Some(row) => document_title(row),
            None => s(&card["title"]),
        };
        docs.push((
            address.clone(),
            format!("{title}\n{}", clip(&s(&card["body"]), EXCERPT_CHARS).0),
        ));
    }
    let model = Relatedness::new(&docs, KEEP);
    let mut order = model.rank_text(query, read.len(), |_| false);
    // A read Card the body stage cannot separate from the rest (its terms are in every document,
    // or it shares none) keeps its title-stage place: it was a match when the pool was selected.
    for address in read {
        if !order.contains(address) {
            order.push(address.clone());
        }
    }
    order
}

/// The catalogue documents of the title stage: one short text per Card that may be offered.
fn catalog_documents(catalog: &[Value], exclude: &BTreeSet<String>) -> Vec<(String, String)> {
    let mut documents = Vec::new();
    for row in catalog {
        let address = s(&row["address"]);
        if address.is_empty()
            || exclude.contains(&address)
            || s(&row["kind"]) == "literature"
            || s(&row["title"]).is_empty()
        {
            continue;
        }
        documents.push((address, document_title(row)));
    }
    documents
}

/// A catalogue row as the term index sees it: its title and what kind of Card it is.
fn document_title(row: &Value) -> String {
    let mut text = s(&row["title"]);
    if s(&row["type"]) == "model" {
        let facet = s(&row["facet"]);
        if facet.is_empty() {
            text.push_str(" 模型");
        } else {
            text.push_str(&format!(" 模型 {facet}"));
        }
    }
    text
}

/// The published pack, from the read bodies and the retained selection facts. Pure: the same
/// catalogue, bodies and register give the same bytes, so a retry selects the same earlier Cards.
#[allow(clippy::too_many_arguments)]
fn assemble(
    request: &Request,
    catalog: &[Value],
    bodies: &BTreeMap<String, Value>,
    register: &Register,
    ranked: &Ranked,
    order: &[String],
    revision: u64,
    identity: &Value,
) -> Pack {
    // Accepted register entries come first: they are the archive's own accepted vocabulary for the
    // concepts this group's text names (D089).
    let mut selected: Vec<String> = Vec::new();
    for address in order {
        if register.targets.contains_key(address) && !selected.contains(address) {
            selected.push(address.clone());
        }
    }
    for address in order {
        if !register.targets.contains_key(address) && !selected.contains(address) {
            selected.push(address.clone());
        }
    }
    for address in ranked.read.iter() {
        if register.targets.contains_key(address) && !selected.contains(address) {
            selected.push(address.clone());
        }
    }
    let mut candidates = Vec::new();
    let mut used = 0usize;
    let mut over_budget = 0usize;
    for address in &selected {
        if candidates.len() >= CANDIDATES {
            break;
        }
        let (Some(card), Some(row)) = (
            bodies.get(address),
            catalog.iter().find(|row| s(&row["address"]) == *address),
        ) else {
            continue;
        };
        let body = s(&card["body"]);
        let (excerpt, cut) = clip(&body, EXCERPT_CHARS);
        let width = chars(&excerpt);
        if used + width > PACK_CHARS {
            over_budget += 1;
            continue;
        }
        used += width;
        candidates.push(candidate(request, card, row, register, excerpt, cut, width, used));
    }
    let below_selection = selected.len().saturating_sub(candidates.len() + over_budget);
    let count = candidates.len();
    let enough = count >= ENOUGH;
    let mut faults = Vec::new();
    if let Some(fault) = register.facts.get("fault") {
        faults.push(json!({
            "code":"register_unavailable",
            "detail":format!("the register could not supply accepted terms: {fault}"),
        }));
    }
    let json = json!({
        "schema_version": 1,
        "unit": request.unit,
        "group": request.group,
        "query": request.query.facts(&request.query.text()),
        "archive": {"identity": identity, "revision": revision},
        "catalog": {
            "rows": catalog.len(),
            "corpus": ranked.corpus,
            "ranking": ranked.ranking,
            "matches": ranked.matches,
            "matches_capped": ranked.capped,
            "pool": ranked.read.len(),
            "excluded": request.exclude.iter().cloned().collect::<Vec<_>>(),
            "below_pool": ranked.below_pool,
        },
        "register": register.facts,
        "cutoffs": {
            "candidates": CANDIDATES,
            "enough": ENOUGH,
            "excerpt_chars": EXCERPT_CHARS,
            "pack_chars": PACK_CHARS,
            "rank_pool": POOL,
        },
        "count": count,
        "excerpt_chars": used,
        "truncated": candidates.iter().any(|candidate| candidate["truncated"] == json!(true)),
        "enough": enough,
        "sufficiency": {
            "enough": enough,
            "detail": if enough {
                format!("{count} earlier Cards carry terms this group's text shares; the pack is full enough to read before writing")
            } else {
                format!("only {count} earlier Cards carry terms this group's text shares; a sparse earlier context is valid and no unrelated Card fills the quota")
            },
        },
        "candidates": candidates,
        "omitted": {
            "below_pool": ranked.below_pool,
            "below_selection": below_selection,
            "over_budget": over_budget,
            "register_unread": ranked.register_unread,
        },
        "assertion_rule": "在断言同一主张、互相矛盾或公式完全一致之前，必须用 locator 读整张卡；摘要不够。",
        "faults": faults,
    });
    let markdown = render(&json);
    Pack {
        json,
        markdown,
        revision,
        candidates: count,
    }
}

/// One candidate row: what it is, why it is here, its bounded excerpt and its full-content locator.
fn candidate(
    request: &Request,
    card: &Value,
    row: &Value,
    register: &Register,
    excerpt: String,
    cut: bool,
    width: usize,
    used: usize,
) -> Value {
    let address = s(&card["address"]);
    let terms = register.targets.get(&address).cloned().unwrap_or_default();
    let workspace = s(&card["workspace"]);
    let writable = workspace == "agent";
    json!({
        "address": address,
        "title": s(&card["title"]),
        "type": s(&card["type"]),
        "facet": card["facet"],
        "form": card["form"],
        "level": card["level"],
        "workspace": workspace,
        "tree": s(&row["tree"]),
        "writable": writable,
        "destination": if writable {"agent-card"} else {"read-only-reference"},
        "source_evidence": source_evidence(card),
        "excerpt": excerpt,
        "excerpt_chars": width,
        "excerpt_offset": 0,
        "truncated": cut,
        "text_chars": chars(&s(&card["text"])),
        "text_sha256": canon::sha256_hex(s(&card["text"]).as_bytes()),
        "locator": {"tool":"get_card","arguments":{"at":s(&card["address"])}},
        "reason": {
            "register_terms": terms,
            "shared_with": format!("group {}", request.group),
            "detail": if terms.is_empty() {
                "its title and body share rare terms with this group's title, source and terms".to_string()
            } else {
                format!("an accepted register term of this group names it: {}", terms.join("、"))
            },
        },
        "pack_chars_after": used,
    })
}

/// A Card's source evidence: the checker's own derivation, with the Literature trees its Links
/// name as the fallback when the Card writes no `来源` line.
fn source_evidence(card: &Value) -> Vec<String> {
    let mut sources = zt_check::check::source_evidence(card);
    if !sources.is_empty() {
        return sources;
    }
    let tree = s(&card["tree"]);
    if !tree.is_empty() && tree.bytes().next().is_some_and(|byte| byte.is_ascii_uppercase()) {
        sources.push(tree);
    }
    sources
}

/// The writer-facing pack: the rule it is read under, then each candidate with its reason, its
/// excerpt and how to read all of it.
fn render(pack: &Value) -> String {
    let mut text = String::new();
    text.push_str("# 本组之前的卡片（早前上下文包）\n\n");
    text.push_str(&format!(
        "- 档案修订 {}；标题目录 {} 张，词面匹配 {} 张{}，取前 {} 张读正文。\n",
        pack["archive"]["revision"],
        pack["catalog"]["rows"],
        pack["catalog"]["matches"],
        if pack["catalog"]["matches_capped"] == json!(true) { "（已截断）" } else { "" },
        pack["catalog"]["pool"],
    ));
    text.push_str(&format!(
        "- 本包 {} 张，摘要共 {} 字（每张上限 {} 字，整包上限 {} 字）。\n",
        pack["count"], pack["excerpt_chars"], pack["cutoffs"]["excerpt_chars"], pack["cutoffs"]["pack_chars"],
    ));
    match s(&pack["register"]["status"]).as_str() {
        "read" => text.push_str(&format!(
            "- 登记：{}，命中本组术语 {} 个，目标卡 {} 张。\n",
            s(&pack["register"]["card"]),
            pack["register"]["terms_matched"],
            pack["register"]["targets"],
        )),
        other => text.push_str(&format!("- 登记：{other}（没有可用的登记条目；只按词面选卡）。\n")),
    }
    if pack["sufficiency"]["enough"] != json!(true) {
        text.push_str(&format!("- 提醒：{}\n", s(&pack["sufficiency"]["detail"])));
    }
    text.push_str("\n## 判读规则\n\n");
    text.push_str("1. 这些卡是读的参考，不是必须链接的清单。只链接真实有关系、并在句中写出关系的卡。\n");
    text.push_str("2. 断言同一主张、互相矛盾或公式完全一致之前，必须用 locator 读整张卡。摘要只够判断主题相邻。\n");
    text.push_str("3. 人类工作区的卡只读：可以链接，不能改写，也不能把你的卡放进人类工作区。\n");
    text.push_str("4. 代理工作区的早前卡可以做 Link 的目标。本组只写自己的卡和锚点，不改写它们的正文。\n");
    text.push_str("5. 早前卡已经说出你这段的主张时，改写你的卡或并进那张卡，不要另建近似标题的新卡。\n\n");
    text.push_str("## 候选\n\n");
    for (index, candidate) in pack["candidates"].as_array().into_iter().flatten().enumerate() {
        text.push_str(&format!(
            "### {}. `{}` {}\n\n",
            index + 1,
            s(&candidate["address"]),
            s(&candidate["title"]),
        ));
        let mut facts = format!(
            "- 类型：{}；工作区：{}（{}）；正文 {} 字",
            {
                let card_type = s(&candidate["type"]);
                let facet = s(&candidate["facet"]);
                if facet.is_empty() { card_type } else { format!("{card_type}·{facet}") }
            },
            s(&candidate["workspace"]),
            if candidate["writable"] == json!(true) { "可写" } else { "只读参考" },
            candidate["text_chars"],
        );
        let sources = candidate["source_evidence"].as_array().into_iter().flatten().map(|source| format!("[[{}]]", s(source))).collect::<Vec<_>>();
        if !sources.is_empty() {
            facts.push_str(&format!("；来源：{}", sources.join(" ")));
        }
        text.push_str(&facts);
        text.push('\n');
        text.push_str(&format!("- 为什么：{}\n", s(&candidate["reason"]["detail"])));
        text.push_str(&format!(
            "- 读全文：`zt-digest call -Tool get_card -Json '{{\"at\":\"{}\"}}'`\n",
            s(&candidate["address"]),
        ));
        if candidate["truncated"] == json!(true) {
            text.push_str(&format!(
                "- 摘要：前 {}/{} 字\n\n",
                candidate["excerpt_chars"], candidate["text_chars"],
            ));
        } else {
            text.push_str("- 摘要（全文）\n\n");
        }
        text.push_str(&s(&candidate["excerpt"]));
        text.push_str("\n\n");
    }
    text
}

/// The catalogue rows of the snapshot's trees, in the snapshot's own order.
pub(crate) fn rows(trees: &[Value]) -> Vec<Value> {
    let mut rows = Vec::new();
    for tree in trees {
        for card in tree["cards"].as_array().into_iter().flatten() {
            rows.push(json!({
                "address": card["address"],
                "tree": card["tree"],
                "workspace": card["workspace"],
                "type": card["type"],
                "facet": card["facet"],
                "title": card["title"],
                "kind": card["kind"],
            }));
        }
    }
    rows
}

/// The first `max` Unicode characters of `text`, and whether anything was cut.
pub(crate) fn clip(text: &str, max: usize) -> (String, bool) {
    let mut out = String::new();
    for (taken, character) in text.chars().enumerate() {
        if taken >= max {
            return (out, true);
        }
        out.push(character);
    }
    (out, false)
}

fn chars(text: &str) -> usize {
    text.chars().count()
}

/// The retained claims a prepared group file offers, when the mathematics skeleton appended them.
/// Nothing else is a proposed claim: a group without them reports none (D090).
pub(crate) fn group_claims(group_file: &str, marker: &str) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(group_file) else {
        return Vec::new();
    };
    let Some((_, tail)) = text.split_once(marker) else {
        return Vec::new();
    };
    let mut claims = Vec::new();
    for line in tail.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            break;
        }
        let claim = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).unwrap_or("");
        let claim = claim.trim();
        if !claim.is_empty() && claim.chars().count() <= CLAIM_CHARS {
            claims.push(claim.to_string());
        }
    }
    claims
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(address: &str, title: &str, workspace: &str, card_type: &str, kind: &str) -> Value {
        json!({"address":address,"tree":address.split('/').next().unwrap(),"workspace":workspace,
               "type":card_type,"facet":null,"title":title,"kind":kind})
    }

    fn card(address: &str, title: &str, body: &str, workspace: &str) -> Value {
        json!({"address":address,"tree":address.split('/').next().unwrap(),"workspace":workspace,
               "type":"idea","facet":null,"form":null,"level":null,"title":title,
               "body":body,"text":format!("# {title}\n\n{body}\n"),"links":[]})
    }

    fn request_for(group: u64, title: &str, source: &str, exclude: BTreeSet<String>) -> Request {
        Request {
            unit: format!("digest/g{group:03}@1"),
            group,
            query: Query {
                title: title.to_string(),
                source: source.to_string(),
                terms: "剩余控制权：对资产剩余收益的控制。".to_string(),
                claims: vec!["剩余控制权决定谈判地位".to_string()],
            },
            exclude,
        }
    }

    fn empty_register() -> Register {
        Register {
            facts: json!({"status":"absent","fault":"register_missing"}),
            targets: BTreeMap::new(),
        }
    }

    fn bodies(list: &[Value]) -> BTreeMap<String, Value> {
        list.iter().map(|card| (s(&card["address"]), card.clone())).collect()
    }

    /// One unrelated earlier Card, so a small corpus has a real document frequency: the existing
    /// relatedness rule drops a term that every document uses.
    fn filler() -> (Value, Value) {
        (
            row("3/1", "债务把控制权交给债权人", "agent", "idea", "regular"),
            card("3/1", "债务把控制权交给债权人", "债务把控制权交给债权人。", "agent"),
        )
    }

    fn build_pack(request: &Request, catalog: &[Value], cards: &[Value], register: &Register) -> Pack {
        let map = bodies(cards);
        let text = request.query.text();
        let ranked = rank_titles(catalog, &text, &request.exclude, &register.targets);
        let order = rank_bodies(catalog, &map, &text, &ranked.read);
        let order: Vec<String> = order.into_iter().filter(|at| map.contains_key(at)).collect();
        assemble(request, catalog, &map, register, &ranked, &order, 7, &json!({"id":"ab","path":"x"}))
    }

    fn addresses(pack: &Pack) -> Vec<String> {
        pack.json["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| s(&candidate["address"]))
            .collect()
    }

    #[test]
    fn offers_up_to_twenty_earlier_cards_and_reports_the_rest() {
        let mut catalog = vec![row("9/0", "总目录", "agent", "idea", "topic")];
        let mut cards = vec![card("9/0", "总目录", "控制权分配的总目录。", "agent")];
        for n in 1..=30 {
            let address = format!("9/{n}");
            catalog.push(row(&address, &format!("剩余控制权的谈判地位 {n}"), "agent", "idea", "regular"));
            cards.push(card(&address, &format!("剩余控制权的谈判地位 {n}"), "剩余控制权决定谈判地位。", "agent"));
        }
        let request = request_for(2, "剩余控制权与谈判", "剩余控制权决定谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &catalog, &cards, &empty_register());
        assert_eq!(pack.json["count"], json!(20));
        assert_eq!(pack.candidates, 20);
        assert!(pack.json["omitted"]["below_selection"].as_u64().unwrap() >= 10);
        assert_eq!(pack.json["sufficiency"]["enough"], json!(true));
        assert_eq!(pack.json["catalog"]["rows"], json!(31));
        assert_eq!(pack.json["catalog"]["corpus"], json!(31));
    }

    #[test]
    fn a_sparse_earlier_context_is_reported_and_never_padded() {
        let catalog = vec![
            row("9/0", "控制权分配的总目录", "agent", "idea", "topic"),
            row("9/1", "剩余控制权决定谈判地位", "agent", "idea", "regular"),
            row("4/1", "通用汽车收购费雪车身", "agent", "idea", "regular"),
            filler().0,
        ];
        let cards = vec![
            card("9/0", "控制权分配的总目录", "控制权分配的总目录。", "agent"),
            card("9/1", "剩余控制权决定谈判地位", "剩余控制权决定谈判地位。", "agent"),
            card("4/1", "通用汽车收购费雪车身", "通用汽车收购费雪车身公司。", "agent"),
            filler().1,
        ];
        let request = request_for(2, "剩余控制权", "剩余控制权决定谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &catalog, &cards, &empty_register());
        let count = pack.json["count"].as_u64().unwrap();
        assert!(count >= 2 && count < ENOUGH as u64, "a few matches: {count}");
        assert!(!addresses(&pack).contains(&"4/1".to_string()), "an unrelated Card is never padding");
        assert_eq!(pack.json["enough"], json!(false));
        assert_eq!(pack.json["sufficiency"]["enough"], json!(false));
        assert!(s(&pack.json["sufficiency"]["detail"]).contains("sparse"));
    }

    #[test]
    fn placeholders_and_literature_roots_are_never_offered() {
        let catalog = vec![
            row("A2020", "Stiglitz, 经济学", "agent", "idea", "literature"),
            row("A2020/1", "导论：本组占位", "agent", "idea", "regular"),
            row("9/1", "剩余控制权决定谈判地位", "agent", "idea", "regular"),
            filler().0,
        ];
        let cards = vec![
            card("A2020", "Stiglitz, 经济学", "文献卡。", "agent"),
            card("A2020/1", "导论：本组占位", "剩余控制权决定谈判地位。", "agent"),
            card("9/1", "剩余控制权决定谈判地位", "剩余控制权决定谈判地位。", "agent"),
            filler().1,
        ];
        let mut exclude = BTreeSet::new();
        exclude.insert("A2020/1".to_string());
        let request = request_for(1, "剩余控制权", "剩余控制权决定谈判地位。", exclude);
        let pack = build_pack(&request, &catalog, &cards, &empty_register());
        assert!(addresses(&pack).contains(&"9/1".to_string()));
        assert!(!addresses(&pack).contains(&"A2020/1".to_string()), "the group's own placeholder is never offered");
        assert!(!addresses(&pack).contains(&"A2020".to_string()), "a Literature root is never earlier thought");
        assert_eq!(pack.json["catalog"]["excluded"], json!(["A2020/1"]));
    }

    #[test]
    fn human_cards_are_read_only_references() {
        let catalog = vec![
            row("9/1", "剩余控制权决定谈判地位", "agent", "idea", "regular"),
            row("5/2", "剩余控制权来自所有权", "human", "idea", "regular"),
            filler().0,
        ];
        let cards = vec![
            card("9/1", "剩余控制权决定谈判地位", "剩余控制权决定谈判地位。", "agent"),
            card("5/2", "剩余控制权来自所有权", "剩余控制权来自所有权，谈判地位由此而来。", "human"),
            filler().1,
        ];
        let request = request_for(2, "剩余控制权", "剩余控制权决定谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &catalog, &cards, &empty_register());
        let human = pack.json["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| s(&candidate["workspace"]) == "human")
            .expect("the human Card is offered as a reference");
        assert_eq!(human["writable"], json!(false));
        assert_eq!(human["destination"], json!("read-only-reference"));
        let agent = pack.json["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| s(&candidate["workspace"]) == "agent")
            .unwrap();
        assert_eq!(agent["destination"], json!("agent-card"));
        assert!(pack.markdown.as_str().contains("人类工作区的卡只读"));
    }

    #[test]
    fn a_long_body_is_cut_to_its_first_eight_hundred_characters_with_its_locator() {
        let long = "剩余控制权".repeat(400);
        let catalog = vec![
            row("9/1", "剩余控制权决定谈判地位", "agent", "idea", "regular"),
            filler().0,
        ];
        let cards = vec![
            card("9/1", "剩余控制权决定谈判地位", &long, "agent"),
            filler().1,
        ];
        let request = request_for(2, "剩余控制权", "剩余控制权决定谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &catalog, &cards, &empty_register());
        assert_eq!(addresses(&pack)[0], "9/1");
        let candidate = &pack.json["candidates"][0];
        assert_eq!(candidate["excerpt_chars"], json!(EXCERPT_CHARS));
        assert_eq!(candidate["truncated"], json!(true));
        assert_eq!(chars(&s(&candidate["excerpt"])), EXCERPT_CHARS);
        assert_eq!(candidate["locator"]["tool"], json!("get_card"));
        assert_eq!(candidate["locator"]["arguments"]["at"], json!("9/1"));
        assert!(pack.markdown.as_str().contains("前 800/"));
        assert!(pack.json["excerpt_chars"].as_u64().unwrap() <= PACK_CHARS as u64);
    }

    #[test]
    fn model_heads_and_parts_are_offered_beside_regular_cards() {
        let catalog = vec![
            row("9/1", "剩余控制权的模型", "agent", "model", "regular"),
            row("9/2", "剩余控制权的设定", "agent", "model", "regular"),
            row("9/3", "剩余控制权的结论", "agent", "idea", "regular"),
        ];
        let mut fix = catalog.clone();
        fix[1]["facet"] = json!("设定");
        let cards = vec![
            card("9/1", "剩余控制权的模型", "剩余控制权的模型说明谈判地位。", "agent"),
            card("9/2", "剩余控制权的设定", "剩余控制权的设定。", "agent"),
            card("9/3", "剩余控制权的结论", "剩余控制权决定谈判地位。", "agent"),
        ];
        let mut fix_cards = cards.clone();
        fix_cards[1]["facet"] = json!("设定");
        fix_cards[1]["type"] = json!("model");
        let request = request_for(2, "剩余控制权的模型", "剩余控制权的模型与设定讨论谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &fix, &fix_cards, &empty_register());
        let addresses: Vec<String> = pack.json["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .map(|candidate| s(&candidate["address"]))
            .collect();
        assert!(addresses.contains(&"9/1".to_string()), "model head: {addresses:?}");
        assert!(addresses.contains(&"9/2".to_string()), "model part: {addresses:?}");
        let part = pack.json["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|candidate| s(&candidate["address"]) == "9/2")
            .unwrap();
        assert_eq!(part["facet"], json!("设定"));
        assert_eq!(part["type"], json!("model"));
        assert!(pack.markdown.as_str().contains("model·设定"));
    }

    #[test]
    fn an_accepted_register_term_puts_its_targets_first() {
        let catalog = vec![
            row("9/1", "谈判地位的核心", "agent", "idea", "regular"),
            row("9/2", "剩余控制权", "agent", "idea", "regular"),
        ];
        let cards = vec![
            card("9/1", "谈判地位的核心", "谈判地位取决于信息。", "agent"),
            card("9/2", "剩余控制权", "剩余控制权是对资产的剩余控制。", "agent"),
        ];
        let mut register = empty_register();
        register.facts = json!({"status":"read","card":"索引/0","entries":1,"terms_matched":1,"targets":1});
        register.targets.insert("9/2".to_string(), vec!["剩余控制权".to_string()]);
        let request = request_for(2, "剩余控制权", "剩余控制权决定谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &catalog, &cards, &register);
        assert_eq!(pack.json["candidates"][0]["address"], json!("9/2"));
        assert_eq!(pack.json["candidates"][0]["reason"]["register_terms"], json!(["剩余控制权"]));
        assert_eq!(pack.json["register"]["status"], json!("read"));
        assert!(pack.markdown.as_str().contains("登记术语") || pack.markdown.as_str().contains("登记："));
    }

    #[test]
    fn a_failed_register_is_reported_as_a_fault_and_selection_continues() {
        let catalog = vec![
            row("9/1", "剩余控制权决定谈判地位", "agent", "idea", "regular"),
            filler().0,
        ];
        let cards = vec![
            card("9/1", "剩余控制权决定谈判地位", "剩余控制权决定谈判地位。", "agent"),
            filler().1,
        ];
        let mut register = empty_register();
        register.facts = json!({"status":"failed","fault":{"code":"register_read_failed","detail":"transport closed"}});
        let request = request_for(2, "剩余控制权", "剩余控制权决定谈判地位。", BTreeSet::new());
        let pack = build_pack(&request, &catalog, &cards, &register);
        assert!(pack.json["count"].as_u64().unwrap() >= 1);
        assert_eq!(pack.json["register"]["status"], json!("failed"));
        assert_eq!(pack.json["faults"][0]["code"], json!("register_unavailable"));
    }

    #[test]
    fn a_homogeneous_corpus_falls_back_to_raw_term_overlap() {
        // Every title uses the same terms, so the weighted ranking drops them all as universal.
        // The pack still offers the matching Cards, and says which ranking answered.
        let catalog = vec![
            row("9/1", "剩余控制权与谈判地位", "agent", "idea", "regular"),
            row("9/2", "剩余控制权与所有权", "agent", "idea", "regular"),
        ];
        let cards = vec![
            card("9/1", "剩余控制权与谈判地位", "剩余控制权决定谈判地位。", "agent"),
            card("9/2", "剩余控制权与所有权", "剩余控制权来自所有权。", "agent"),
        ];
        let request = Request {
            unit: "digest/g002@1".to_string(),
            group: 2,
            query: Query {
                title: "剩余控制权".to_string(),
                source: "剩余控制权。".to_string(),
                terms: String::new(),
                claims: Vec::new(),
            },
            exclude: BTreeSet::new(),
        };
        let pack = build_pack(&request, &catalog, &cards, &empty_register());
        assert_eq!(pack.json["catalog"]["ranking"], json!("overlap"));
        assert_eq!(pack.json["catalog"]["corpus"], json!(2));
        assert!(pack.json["catalog"]["matches"].as_u64().unwrap() >= 2);
        assert_eq!(pack.json["count"], json!(2));
    }

    #[test]
    fn proposed_claims_and_term_meanings_join_the_query() {
        let request = request_for(2, "剩余控制权", "本组原文", BTreeSet::new());
        let text = request.query.text();
        assert!(text.contains("剩余控制权"));
        assert!(text.contains("对资产剩余收益的控制"));
        assert!(text.contains("剩余控制权决定谈判地位"));
        let facts = request.query.facts(&text);
        assert_eq!(facts["claims"], json!(["剩余控制权决定谈判地位"]));
        assert!(facts["chars"].as_u64().unwrap() > 0);
    }

    #[test]
    fn the_group_extension_offers_claims_and_nothing_else_does() {
        let dir = std::env::temp_dir().join(format!("zt-earlier-claims-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("g001.md");
        std::fs::write(&file, "原文\n\n## 本组的数学单元\n\n- 定理：剩余控制权决定谈判地位\n- 定义：剩余控制权\n").unwrap();
        let claims = group_claims(&file.to_string_lossy(), "## 本组的数学单元");
        assert_eq!(claims, vec!["定理：剩余控制权决定谈判地位", "定义：剩余控制权"]);
        std::fs::write(&file, "原文，没有单元列表。\n").unwrap();
        assert!(group_claims(&file.to_string_lossy(), "## 本组的数学单元").is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
