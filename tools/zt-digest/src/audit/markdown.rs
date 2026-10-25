//! audit.md, as digest-audit.ps1 wrote it (AppendLine: Environment.NewLine).

use super::Params;
use crate::ps::{NL, ps_count, ps_items, s, truthy};
use serde_json::{Value, json};

/// `$cut`: the first -Sample items in backticks, then `, … (N)`; or `none`.
fn cut(list: &[String], sample: i64) -> String {
    let quote = |x: &String| format!("`{x}`");
    if list.len() as i64 > sample {
        // PowerShell's range 0..(Sample - 1) counts down when Sample is below 1.
        let indices: Vec<i64> = if sample >= 1 { (0..sample).collect() } else { (sample - 1..=0).rev().collect() };
        let picked: Vec<String> = indices
            .iter()
            .map(|&i| {
                let at = if i < 0 { list.len() as i64 + i } else { i };
                if at >= 0 && (at as usize) < list.len() { list[at as usize].clone() } else { String::new() }
            })
            .collect();
        format!("{}, … ({})", picked.iter().map(quote).collect::<Vec<_>>().join(", "), list.len())
    } else if !list.is_empty() {
        list.iter().map(quote).collect::<Vec<_>>().join(", ")
    } else {
        "none".into()
    }
}

/// `$tally`: `name count` pairs, or `none`.
fn tally(t: &Value) -> String {
    let pairs: Vec<String> = t.as_object().map(|m| m.iter().map(|(k, v)| format!("{k} {}", s(v))).collect()).unwrap_or_default();
    if pairs.is_empty() { "none".into() } else { pairs.join(", ") }
}

fn strings(v: &Value) -> Vec<String> {
    ps_items(v).iter().map(s).collect()
}

fn n(v: &Value) -> String {
    s(v)
}

/// The advisory rules of one rule group, read from the checker's shared advisory catalog (S27,
/// #166, AC06): the report never keeps its own list of what advice is. `prefixes` are the rule
/// families of the group (`R` for the Card language, `IEFL` for the Card layers).
fn advisory_rules(prefixes: &[char]) -> String {
    zt_check::policy::ADVISORY_RULES
        .iter()
        .filter(|rule| rule.chars().next().is_some_and(|c| prefixes.contains(&c)))
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn render(report: &Value, p: &Params) -> String {
    let mut md = String::new();
    let mut line = |text: &str| {
        md.push_str(text);
        md.push_str(NL);
    };
    let w = &report["workspace"];
    line(&format!("# Digest audit: {}", n(&report["scope"])));
    line("");
    line(&format!(
        "Generated {} by `scripts/book-digest/digest-audit.ps1` (read-only), in {} s. Card size limits: {} sentences, {} characters, {} steps.",
        n(&report["generated"]),
        n(&report["seconds"]),
        p.max_sentences,
        p.max_chars,
        p.max_steps
    ));
    line("");
    line("## Agent Workspace");
    line("");
    line("| Measure | Value |");
    line("| --- | --- |");
    line(&format!("| Trees | {} ({} Topic trees, {} books) |", n(&w["trees"]), n(&w["topic_trees"]), n(&w["books"])));
    line(&format!("| Cards | {}: {} |", n(&w["cards"]), tally(&w["cards_by_category"])));
    let q = &w["topic_tree_sizes"];
    line(&format!(
        "| Topic tree size | median {}, 25th percentile {}, 75th percentile {}, min {}, max {} |",
        n(&q["median"]),
        n(&q["p25"]),
        n(&q["p75"]),
        n(&q["min"]),
        n(&q["max"])
    ));
    line(&format!(
        "| Hub Cards | {}. Missing hubs {}. Hubs with faults {}. |",
        tally(&w["hubs"]["counts"]),
        n(&w["hubs"]["missing"]),
        n(&w["hubs"]["with_faults"])
    ));
    line(&format!("| Broken Links | {} |", n(&w["broken_links"])));
    // S26 (#169): the Workspace's Term coverage is the books' own sections.
    let wt = &w["terms"];
    line(&format!(
        "| Terms | **{}**: {} book(s) with a Term list; rows {} ({} published), heads {} ({} math, {} concept), parts {}, uses {}, avoid words left plain {}, T faults {} |",
        n(&wt["status"]),
        n(&wt["books_with_list"]),
        n(&wt["rows"]),
        n(&wt["published"]),
        n(&wt["heads"]),
        n(&wt["math"]),
        n(&wt["concept"]),
        n(&wt["parts"]),
        n(&wt["uses"]),
        n(&wt["avoid"]),
        n(&wt["faults"])
    ));
    if report["gate"].is_object() {
        line(&format!(
            "| Check contract | rule version {}; stage {}; policy {}; zt-check {} |",
            n(&report["gate"]["rule_version"]),
            n(&report["gate"]["stage"]),
            n(&report["gate"]["policy_hash"]),
            n(&report["gate"]["checker_version"])
        ));
    }
    let books = ps_items(&report["books"]);
    let keys = |f: &dyn Fn(&Value) -> bool| -> Vec<String> { books.iter().filter(|b| f(&b["bridges"]["island"])).map(|b| format!("`{}`", n(&b["key"]))).collect() };
    let islands = keys(&|v| *v == Value::Bool(true));
    let unknown = keys(&|v| v.is_null());
    line(&format!(
        "| Islands (books with no semantic Bridge in or out) | {}{} |",
        if islands.is_empty() { "none".to_string() } else { islands.join(", ") },
        if unknown.is_empty() { String::new() } else { format!(". Not known (incomplete read or no Card outside the book): {}", unknown.join(", ")) }
    ));
    let u = &w["unattributed"];
    line(&format!("| Topic-tree Cards that link no book | {}: {} |", n(&u["count"]), tally(&u["by_category"])));
    if truthy(&u["style"]) {
        let us = &u["style"];
        let by_cat: Vec<String> =
            us["by_category"].as_object().map(|m| m.iter().map(|(k, v)| format!("{k}: {}", tally(v))).collect()).unwrap_or_default();
        line(&format!("| Their Card language faults | {} in {} Cards. {} |", n(&us["faults"]), n(&us["cards_with_faults"]), by_cat.join(". ")));
        let orphans = strings(&u["orphans"]);
        line(&format!("| Their orphans | {}: {} |", orphans.len(), cut(&orphans, p.sample)));
    }
    line("");
    line("## Books");
    line("");
    line("| Book | Cards | Topic trees (most Cards) | Card language faults (Cards) | Card layer faults (Cards), advisory | Orphans | Idea and Event Cards with no source | with no kinded Link | Models not self-sufficient | Figures named, not shown (known) |");
    line("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let int = |v: &Value| v.as_i64().unwrap_or(0);
    for b in &books {
        line(&format!(
            "| `{}` | {} | {} ({}) | {} ({}) | {} ({}), {} | {} | {} of {} | {} of {} | {} of {} | {} ({}) |",
            n(&b["key"]),
            n(&b["cards"]),
            n(&b["topics"]["trees"]),
            n(&b["topics"]["majority"]),
            n(&b["style"]["faults"]),
            n(&b["style"]["cards_with_faults"]),
            n(&b["layer"]["faults"]),
            n(&b["layer"]["cards_with_faults"]),
            n(&b["layer"]["advisory"]),
            n(&b["orphans"]["count"]),
            n(&b["no_source"]["count"]),
            n(&b["no_source"]["of"]),
            n(&b["unkinded"]["count"]),
            n(&b["unkinded"]["of"]),
            n(&b["models"]["not_self_sufficient"]),
            n(&b["models"]["count"]),
            int(&b["figures"]["missing_known"]) + int(&b["figures"]["missing_unknown"]),
            n(&b["figures"]["missing_known"])
        ));
    }
    line("");
    for b in &books {
        line(&format!("### `{}`: {}", n(&b["key"]), n(&b["title"])));
        line("");
        let unread = ps_items(&b["unread"]).len();
        line(&format!(
            "- **Cards**: {} ({}). {} are in Topic trees.{}",
            n(&b["cards"]),
            tally(&b["cards_by_category"]),
            n(&b["cards_in_topic_trees"]),
            if unread > 0 { format!(" {unread} Cards could not be read.") } else { String::new() }
        ));
        line(&format!(
            "- **Topic trees**: {}. The book holds most Cards of {}. Size median {}, max {}.",
            n(&b["topics"]["trees"]),
            n(&b["topics"]["majority"]),
            n(&b["topics"]["sizes"]["median"]),
            n(&b["topics"]["sizes"]["max"])
        ));
        line(&format!(
            "- **Card language**: {} faults in {} of {} Cards. By rule: {}. Standalone advice ({}): {} by rule: {}.",
            n(&b["style"]["faults"]),
            n(&b["style"]["cards_with_faults"]),
            n(&b["style"]["cards_checked"]),
            tally(&b["style"]["by_rule"]),
            advisory_rules(&['R']),
            n(&b["style"]["advisory"]),
            tally(&b["style"]["advice_by_rule"])
        ));
        let term = if truthy(&b["layer"]["term_table"]) { format!("`{}`", n(&b["layer"]["term_table"])) } else { "none".into() };
        line(&format!(
            "- **Card layer rules** (I, E, F, L): {} faults in {} Cards, {} advisory ({}; I7 unknown-source subcase). By rule: {}. Term table: {term}.",
            n(&b["layer"]["faults"]),
            n(&b["layer"]["cards_with_faults"]),
            n(&b["layer"]["advisory"]),
            advisory_rules(&['I', 'E', 'F', 'L']),
            tally(&b["layer"]["by_rule"])
        ));
        if let Some(m) = b["style"]["by_category"].as_object() {
            for (cat, t) in m {
                line(&format!("  - {cat}: {}", tally(t)));
            }
        }
        // S26 (#169): the Term coverage and quality, apart from the audit's own completion.
        let t = &b["terms"];
        if t["status"] == "skipped" {
            line(&format!("- **Terms**: skipped ({}).", n(&t["reason"])));
        } else if truthy(&t["list"]) {
            let most: Vec<String> = ps_items(&t["most_used"])
                .iter()
                .map(|row| format!("{} ({})", n(&row["term"]), n(&row["uses"])))
                .collect();
            line(&format!(
                "- **Terms** (`{}`, **{}**): rows {} ({} published), heads {} ({} math, {} concept), parts {}, unfiled {}. Uses {} in {} Cards ({}). No use: {} head(s), {} Card(s). Avoid words left plain: {}. T faults: {}. Most used: {}.",
                n(&t["list"]),
                n(&t["status"]),
                n(&t["rows"]),
                n(&t["published"]),
                n(&t["heads"]),
                n(&t["math"]),
                n(&t["concept"]),
                n(&t["parts"]),
                n(&t["unfiled"]),
                n(&t["uses"]["total"]),
                n(&t["uses"]["cards"]),
                tally(&t["uses"]["by_type"]),
                ps_count(&t["no_use"]["heads"]),
                ps_count(&t["no_use"]["cards"]),
                n(&t["avoid"]["count"]),
                n(&t["faults"]["total"]),
                if most.is_empty() { "none".to_string() } else { most.join(", ") }
            ));
            if !t["reason"].as_str().unwrap_or_default().is_empty() {
                line(&format!("  - Why not complete: {}.", n(&t["reason"])));
            }
            if !ps_items(&t["no_use"]["heads"]).is_empty() {
                line(&format!(
                    "  - Heads with no Term use: {}",
                    cut(&strings(&t["no_use"]["heads"]), p.sample)
                ));
            }
            if !ps_items(&t["avoid"]["list"]).is_empty() {
                let rows: Vec<String> = ps_items(&t["avoid"]["list"])
                    .iter()
                    .take(p.sample.max(1) as usize)
                    .map(|row| format!("`{}` line {}: {}", n(&row["address"]), n(&row["line"]), n(&row["word"])))
                    .collect();
                line(&format!("  - Avoid words still written plainly: {}", rows.join("; ")));
            }
        }
        let g = &t["generation"];
        if g["available"] == json!(true) {
            let unit = |value: &Value| {
                if value.is_null() {
                    "none".to_string()
                } else {
                    format!("{} {}", n(&value["unit"]), n(&value["state"]))
                }
            };
            let finals: Vec<String> = ps_items(&g["final_rounds"]).iter().map(unit).collect();
            line(&format!(
                "- **Term generation**: Term-card book unit {}; whole-book Term-link unit {}{}.",
                unit(&g["term_cards"]),
                unit(&g["term_link"]),
                if finals.is_empty() { String::new() } else { format!("; finalization rounds: {}", finals.join(", ")) }
            ));
        } else {
            line(&format!("- **Term generation**: not known ({}).", n(&g["reason"])));
        }
        line(&format!(
            "- **Orphans** (no inbound Link): {} ({}): {}",
            n(&b["orphans"]["count"]),
            tally(&b["orphans"]["by_category"]),
            cut(&strings(&b["orphans"]["list"]), p.sample)
        ));
        line(&format!(
            "- **No source line** (Idea and Event Cards with no Link to the Literature Card or an anchor): {} of {}: {}",
            n(&b["no_source"]["count"]),
            n(&b["no_source"]["of"]),
            cut(&strings(&b["no_source"]["list"]), p.sample)
        ));
        line(&format!(
            "- **No kinded Link** to a Card other than the parent: {} of {}. Links read as \"refers\": {}. {}",
            n(&b["unkinded"]["count"]),
            n(&b["unkinded"]["of"]),
            n(&b["unkinded"]["refers_links"]),
            cut(&strings(&b["unkinded"]["list"]), p.sample)
        ));
        let not_self: Vec<String> =
            ps_items(&b["models"]["list"]).iter().filter(|r| !truthy(&r["self_sufficient"])).map(|r| s(&r["address"])).collect();
        line(&format!(
            "- **Models**: {} ({}). Not self-sufficient: {}. With get_model issues: {}. Derivation parts with 思路 and 解读: {} of {}. {}",
            n(&b["models"]["count"]),
            tally(&b["models"]["levels"]),
            n(&b["models"]["not_self_sufficient"]),
            n(&b["models"]["with_issues"]),
            n(&b["models"]["explained"]),
            n(&b["models"]["derivations"]),
            cut(&not_self, p.sample)
        ));
        let known: Vec<String> = ps_items(&b["figures"]["list"])
            .iter()
            .filter(|f| truthy(&f["known"]))
            .map(|f| format!("{} 图 {}", s(&f["address"]), s(&f["figure"])))
            .collect();
        line(&format!(
            "- **Figures**: {} figures shown. Named but not shown: {} known, {} unknown. {}",
            n(&b["figures"]["shown_labels"]),
            n(&b["figures"]["missing_known"]),
            n(&b["figures"]["missing_unknown"]),
            cut(&known, p.sample)
        ));
        let bb = &b["bridges"];
        let side_text = |side: &Value| {
            let mut parts: Vec<String> =
                side["books"].as_object().map(|m| m.iter().map(|(k, v)| format!("`{k}` {}", s(v))).collect()).unwrap_or_default();
            if truthy(&side["human"]) {
                parts.push(format!("human {}", s(&side["human"])));
            }
            if truthy(&side["unattributed"]) {
                parts.push(format!("unattributed {}", s(&side["unattributed"])));
            }
            if parts.is_empty() { "none".to_string() } else { parts.join(", ") }
        };
        let island_text = if bb["island"] == Value::Bool(true) {
            " **Island.**"
        } else if bb["island"].is_null() {
            if !truthy(&bb["complete"]) { " Island: not known (a tree could not be read)." } else { " Island: not known (no Card outside the book)." }
        } else {
            ""
        };
        line(&format!(
            "- **Semantic Bridges** (kinded Links between substantive Cards of different books; inferred attribution): out {}. In {}.{island_text}",
            side_text(&bb["out"]),
            side_text(&bb["in"])
        ));
        line(&format!("- **Broken Links** from the book's Cards: {}", n(&b["broken_links"])));
        line("");
    }
    if truthy(&report["baseline"]) {
        baseline(&mut line, &report["baseline"], p);
    }
    md
}

/// `## Baseline measures (#94 S01)`: the evidence, the source sets, the P1–P15 measures, the
/// fixed manual task sheet, and the comparison. Every unknown measure names what is missing;
/// nothing here is a zero that stands in for missing evidence.
fn baseline(line: &mut impl FnMut(&str), b: &Value, p: &Params) {
    let e = &b["evidence"];
    line("## Baseline measures (#94 S01)");
    line("");
    line(&format!(
        "- **Evidence**: archive `{}` at `{}`, revision {}. Software: zt-digest {}, zt-check {}, zt {}.",
        n(&e["archive"]["id"]),
        n(&e["archive"]["path"]),
        n(&e["archive"]["revision"]),
        n(&e["software"]["zt_digest"]),
        n(&e["software"]["zt_check"]),
        n(&e["software"]["zt"])
    ));
    let thresholds = e["thresholds"]
        .as_object()
        .map(|map| map.iter().map(|(name, value)| format!("{name} {value}")).collect::<Vec<_>>().join(", "))
        .unwrap_or_default();
    line(&format!("- **Thresholds**: {thresholds}. Rules: {}.", n(&e["software"]["rules"])));
    line(&format!(
        "- **Scope**: `{}` ({}), {} Cards in {} Topic trees; books: {}.",
        n(&e["scope"]["name"]),
        n(&e["scope"]["kind"]),
        n(&e["scope"]["cards"]),
        n(&e["scope"]["trees"]),
        cut(&strings(&e["scope"]["books"]), p.sample)
    ));
    line(&format!(
        "- **Sample**: seed {}, drew {} of {} Idea Cards: {}.",
        n(&e["sample"]["seed"]),
        n(&e["sample"]["drawn"]),
        n(&e["sample"]["population"]),
        cut(&strings(&e["sample"]["addresses"]), p.sample)
    ));
    let inputs: Vec<String> = ["baseline", "concept_report", "decisions", "run_report", "task_sheet"]
        .iter()
        .map(|name| {
            let input = &e["inputs"][*name];
            if truthy(&input["read"]) { format!("{name} `{}`", n(&input["path"])) } else { format!("{name} absent") }
        })
        .collect();
    line(&format!("- **Inputs**: {}.", inputs.join(", ")));
    let missing = ps_items(&e["missing"]);
    if missing.is_empty() {
        line("- **Missing evidence**: none. Every measure below has the evidence it names.");
    } else {
        line(&format!("- **Missing evidence**: {}. No measure below is reported as a zero in its place.", missing.len()));
        for entry in &missing {
            line(&format!("  - `{}`: {}. Measures without it: {}.", n(&entry["kind"]), n(&entry["reason"]), cut(&strings(&entry["measures"]), p.sample)));
        }
    }
    line("");
    line("| Measure | Known | Value | Denominator | Per 100 | Note |");
    line("| --- | --- | --- | --- | --- | --- |");
    for name in ["P1", "P2", "P3_P5", "P4", "P7", "P8", "P9", "P11", "P12_P13", "P14", "P15"] {
        let m = &b["measures"][name];
        if m.is_null() {
            continue;
        }
        let known = if truthy(&m["known"]) {
            "yes".to_string()
        } else {
            format!("**no** — {}", n(&m["reason"]))
        };
        line(&format!(
            "| {} | {known} | {} | {} | {} | {} |",
            name,
            n(&m["values"]["value"]),
            n(&m["values"]["denominator"]),
            n(&m["rate"]["per_100"]),
            n(&m["measure"])
        ));
    }
    line("");
    let s = &b["sources"];
    line("### Source evidence");
    line("");
    line(&format!(
        "- **Sets**: {} of {} Cards have explicit evidence ({} from their Literature tree, {} from a `来源` line; {} derived from a model head). {} are unknown and {} name several books.",
        n(&s["cards"]["known"]),
        n(&s["cards"]["total"]),
        n(&s["direct"]["literature_tree"]),
        n(&s["direct"]["source_line"]),
        n(&s["cards"]["derived"]),
        n(&s["cards"]["unknown"]),
        n(&s["cards"]["multiple"])
    ));
    line(&format!(
        "- **Differs from the existing inferred attribution**: {} Cards. The complete evidence set is `audit-sources.json`.",
        n(&s["differs"])
    ));
    for row in ps_items(&s["differences"]) {
        line(&format!(
            "  - `{}` 推断 `{}` → 证据 {}（{}）",
            n(&row["address"]),
            if truthy(&row["inferred"]) { n(&row["inferred"]) } else { "none".into() },
            cut(&strings(&row["sources"]), p.sample),
            n(&row["title"])
        ));
    }
    line(&format!(
        "- **Human Workspace**: {} Cards, {} authored Links into agent Cards.",
        n(&s["human"]["cards"]),
        n(&s["human"]["to_agent_links"])
    ));
    line("");
    let p7 = &b["measures"]["P7"];
    line("### Cross-book hub lines");
    line("");
    line(&format!(
        "- **Cross-book lines**: {} of {} hub member lines ({} per 100). A member line crosses books only on explicit evidence, so an unsupported Link cannot improve a ratio.",
        n(&p7["cross_book_lines"]["value"]),
        n(&p7["cross_book_lines"]["denominator"]),
        n(&p7["cross_book_lines"]["per_100"])
    ));
    line(&format!(
        "- **Hubs**: {} with lines; {} stay in one book and {} are `mirror` advice. Unsupported lines (an endpoint without explicit evidence): {}.",
        n(&p7["hubs"]["total"]),
        n(&p7["hubs"]["one_book"]),
        n(&p7["hubs"]["mirror"]),
        n(&p7["unsupported"]["value"])
    ));
    let p8 = &b["measures"]["P8"];
    line("");
    line("### Register and reach (P8)");
    line("");
    if truthy(&p8["known"]) {
        line(&format!(
            "- **Accepted terms**: {} entries, {} rows, {} unresolved rows, {} incomplete entries.",
            n(&p8["terms"]["accepted"]),
            n(&p8["terms"]["rows"]),
            n(&p8["terms"]["unresolved_rows"]),
            n(&p8["terms"]["incomplete_entries"])
        ));
        line(&format!(
            "- **Direct destinations**: {} substantive Cards. **Reach within {} semantic Links** (outbound): {} at one Link, {} at two. A register part is no hop.",
            n(&p8["direct"]["value"]),
            n(&p8["reach"]["hops"]),
            n(&p8["reach"]["at_1"]),
            n(&p8["reach"]["at_2"])
        ));
    } else {
        line(&format!("- Not known: {}.", n(&p8["reason"])));
    }
    line(&format!(
        "- **Hub unreached** (a different definition, reported apart): {} Cards in {} hubs.",
        n(&p8["hub_unreached"]["unreached"]),
        n(&p8["hub_unreached"]["hubs_with_unreached"])
    ));
    let p4 = &b["measures"]["P4"];
    line("");
    line("### Appended tails and cost (P4)");
    line("");
    line(&format!(
        "- **`关联：` tails**: {} in {} of {} Cards ({} per 100). **Duplicate owned paragraphs**: {}.",
        n(&p4["tails"]["value"]),
        n(&p4["tails"]["cards"]),
        n(&p4["tails"]["denominator"]),
        n(&p4["tails"]["per_100"]),
        n(&p4["duplicate_owned_paragraphs"]["value"])
    ));
    if truthy(&p4["run"]["known"]) {
        line(&format!(
            "- **Link jobs**: {}. **Exact per-run usage**: {} (status `{}`, {} known runs, {} missing).",
            tally(&p4["run"]["link"]),
            n(&p4["run"]["usage"]["totals"]),
            n(&p4["run"]["usage"]["status"]),
            n(&p4["run"]["usage"]["known_runs"]),
            n(&p4["run"]["usage"]["missing_runs"])
        ));
    } else {
        line(&format!("- **Link jobs and usage**: not known: {}.", n(&p4["run"]["reason"])));
    }
    let p9 = &b["measures"]["P9"];
    line("");
    line("### Standalone rates (P9)");
    line("");
    line(&format!(
        "- **Local reference (R20)**: {} Cards of {} ({} per 100). **Unnamed referent (R21)**: {} Cards ({} per 100).",
        n(&p9["local_reference"]["cards"]),
        n(&p9["local_reference"]["denominator"]),
        n(&p9["local_reference"]["per_100"]),
        n(&p9["unnamed_referent"]["cards"]),
        n(&p9["unnamed_referent"]["per_100"])
    ));
    if truthy(&p9["manual"]["known"]) {
        line(&format!(
            "- **Manual annotation**: {} sampled Cards; {} local-reference and {} unnamed-referent false positives.",
            n(&p9["manual"]["annotated"]),
            n(&p9["manual"]["local_reference_false_positive"]),
            n(&p9["manual"]["unnamed_referent_false_positive"])
        ));
    } else {
        line(&format!("- **Manual annotation**: not known: {}.", n(&p9["manual"]["reason"])));
    }
    let p11 = &b["measures"]["P11"];
    line("");
    line("### Titles (P11)");
    line("");
    line(&format!(
        "- **Idea population**: {} Cards; {} exact and {} near title pairs. **Navigation population**: {} Cards; {} exact and {} near pairs. Threshold {}.",
        n(&p11["idea"]["cards"]),
        n(&p11["idea"]["exact_pairs"]),
        n(&p11["idea"]["near_pairs"]),
        n(&p11["navigation"]["cards"]),
        n(&p11["navigation"]["exact_pairs"]),
        n(&p11["navigation"]["near_pairs"]),
        n(&p11["threshold"])
    ));
    line(&format!("- **Anchor-title faults**: {}.", n(&p11["anchor_title_faults"])));
    let sheet = &b["task_sheet"];
    line("");
    line("### Fixed manual task sheet (P12/P13, P15)");
    line("");
    if truthy(&sheet["present"]) {
        let p12 = &b["measures"]["P12_P13"];
        line(&format!(
            "- **Sheet**: `{}`, {} tasks over {}. Valid: {}.",
            n(&sheet["path"]),
            ps_items(&sheet["tasks"]).len(),
            format!("{}..{}", n(&sheet["interval"]["from"]), n(&sheet["interval"]["to"])),
            if truthy(&sheet["valid"]) { "yes" } else { "no" }
        ));
        for fault in ps_items(&sheet["faults"]) {
            line(&format!("  - Fault `{}` on `{}`: {}", n(&fault["fault"]), n(&fault["task"]), n(&fault["detail"])));
        }
        if truthy(&p12["known"]) {
            line(&format!(
                "- **Time to the first useful Card**: median {} s over {} tasks. **Kept Spread**: median {} s over {} tasks. **Entry rank**: median {}.",
                n(&p12["first_useful_card"]["median_seconds"]),
                n(&p12["first_useful_card"]["tasks"]),
                n(&p12["kept_spread"]["median_seconds"]),
                n(&p12["kept_spread"]["tasks"]),
                n(&p12["entry_rank"]["median"])
            ));
            line(&format!(
                "- **Removed Cards**: {} recorded, {} verified absent. **Human replies**: {}.",
                n(&p12["removed_cards"]["recorded"]),
                n(&p12["removed_cards"]["verified_absent"]),
                n(&p12["human_replies"])
            ));
        } else {
            line(&format!("- Not known: {}.", n(&p12["reason"])));
        }
        let p15 = &b["measures"]["P15"];
        line(&format!(
            "- **Distant Cards**: {} tasks, {} offered, {} opened, {} laid.",
            n(&p15["tasks"]),
            n(&p15["offered"]),
            n(&p15["opened"]),
            n(&p15["laid"])
        ));
    } else {
        line(&format!("- Not known: {}.", n(&sheet["reason"])));
    }
    let p14 = &b["measures"]["P14"];
    line("");
    line("### The human half (P14)");
    line("");
    line(&format!(
        "- **Observed**: {} human Cards; {} authored and {} semantic Links from a human Card into an agent Card. An observation, not a quota.",
        n(&p14["human_cards"]),
        n(&p14["to_agent_links"]["authored"]),
        n(&p14["to_agent_links"]["semantic"])
    ));
    if truthy(&p14["interval"]) {
        line(&format!(
            "- **Interval**: since revision {}, human Cards {:+}, human-to-agent Links {:+}. A net change between two reads, not a count of events.",
            n(&p14["interval"]["baseline_revision"]),
            n(&p14["interval"]["human_cards_net_change"]).parse::<i64>().unwrap_or(0),
            n(&p14["interval"]["to_agent_links_net_change"]).parse::<i64>().unwrap_or(0)
        ));
    } else {
        line("- **Interval**: none. A baseline report that recorded the human counts is required.");
    }
    let c = &b["comparison"];
    line("");
    line("### Comparison with an earlier report (S01-AC06)");
    line("");
    if truthy(&c["known"]) {
        line(&format!(
            "- **Baseline**: `{}`, generated {}, scope `{}`, revision {}, {} addresses.",
            n(&c["baseline"]["path"]),
            n(&c["baseline"]["generated"]),
            n(&c["baseline"]["scope"]),
            n(&c["baseline"]["archive_revision"]),
            n(&c["baseline"]["addresses"])
        ));
        if truthy(&c["matched"]) {
            line("- **Scopes match**: the deltas below compare one population with itself.");
        } else {
            line(&format!("- **Scope change, no delta**: {}.", n(&c["scope_change"])));
        }
        if truthy(&c["new_cards"]) {
            line(&format!("- **New Cards**: {} (inside this scope, absent from the baseline's address list).", n(&c["new_cards"]["count"])));
        }
        line("");
        line("| Value | Baseline | Current | Delta | Status |");
        line("| --- | --- | --- | --- | --- |");
        if let Some(rows) = c["measures"].as_object() {
            for (name, row) in rows {
                line(&format!(
                    "| {name} | {} | {} | {} | {} |",
                    n(&row["baseline"]),
                    n(&row["current"]),
                    n(&row["delta"]),
                    n(&row["status"])
                ));
            }
        }
    } else {
        line(&format!("- Not known: {}.", n(&c["reason"])));
    }
    line("");
    let proposal = &b["proposal"];
    line(&format!(
        "### The accepted proposal's baseline, in its original scope ({})",
        n(&proposal["source"])
    ));
    line("");
    line(&format!(
        "- Measured {} on {} Cards of the whole agent Workspace ({} Topic Cards, {} Topics, {} Literature Cards). {}.",
        n(&proposal["measured"]),
        n(&proposal["scope"]["cards"]),
        n(&proposal["scope"]["topic_cards"]),
        n(&proposal["scope"]["topics"]),
        n(&proposal["scope"]["literature_cards"]),
        n(&b["comparison"]["proposal"]["scope_change"])
    ));
    if let Some(values) = proposal["values"].as_object() {
        for (name, row) in values {
            line(&format!("  - **{name}**: {} — {}.", n(&row["measure"]), n(&row["value"])));
        }
    }
    line("");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advisory_rule_lists_come_from_the_shared_catalog() {
        // S27 (#166, AC06): the rendered advice names the checker's own advisory rules of that rule
        // group, so a rule that becomes advisory or blocking changes the report with the catalog.
        assert_eq!(advisory_rules(&['R']), "R20, R21");
        assert_eq!(advisory_rules(&['I', 'E', 'F', 'L']), "I1, I6, L3, E5");
        // No advisory layer rule beyond I1/I6/L3/E5, and the Term rule T4 never leaks into the Card
        // layer's list: it is not a layer rule, whatever the catalog holds.
        assert_eq!(advisory_rules(&['T']), "T4");
        assert_eq!(advisory_rules(&['Z']), "");
    }

    #[test]
    fn cut_lists() {
        let l: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(cut(&l, 15), "`a`, `b`, `c`");
        assert_eq!(cut(&l, 2), "`a`, `b`, … (3)");
        assert_eq!(cut(&[], 2), "none");
        // -Sample 0: PowerShell's 0..-1 is the first and the last item.
        assert_eq!(cut(&l, 0), "`a`, `c`, … (3)");
    }
}
