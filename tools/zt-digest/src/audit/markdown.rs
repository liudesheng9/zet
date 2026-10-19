//! audit.md, as digest-audit.ps1 wrote it (AppendLine: Environment.NewLine).

use super::Params;
use crate::ps::{NL, ps_items, s, truthy};
use serde_json::Value;

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
            "- **Card language**: {} faults in {} of {} Cards. By rule: {}. Standalone advice (R20, R21): {} by rule: {}.",
            n(&b["style"]["faults"]),
            n(&b["style"]["cards_with_faults"]),
            n(&b["style"]["cards_checked"]),
            tally(&b["style"]["by_rule"]),
            n(&b["style"]["advisory"]),
            tally(&b["style"]["advice_by_rule"])
        ));
        let term = if truthy(&b["layer"]["term_table"]) { format!("`{}`", n(&b["layer"]["term_table"])) } else { "none".into() };
        line(&format!(
            "- **Card layer rules** (I, E, F, L): {} faults in {} Cards, {} advisory (I1, I6, E5, L3, I7 unknown). By rule: {}. Term table: {term}.",
            n(&b["layer"]["faults"]),
            n(&b["layer"]["cards_with_faults"]),
            n(&b["layer"]["advisory"]),
            tally(&b["layer"]["by_rule"])
        ));
        if let Some(m) = b["style"]["by_category"].as_object() {
            for (cat, t) in m {
                line(&format!("  - {cat}: {}", tally(t)));
            }
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
    md
}

#[cfg(test)]
mod tests {
    use super::*;

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
