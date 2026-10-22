//! zt-digest: the book digest's tools as one binary (tools/zt-digest).
//!
//!   zt-digest call  <file|-> [-From N -Addresses a,b] [-Log f] [-Allow f] [-Moves f] [-SrcLog f] [-TopicGate f]
//!   zt-digest call  -Tool <tool> [-Json '{...}']
//!   zt-digest audit -CitationKey <key> -Out <folder> ...      (digest-audit.ps1)
//!   zt-digest card  -Step plan|apply|check|report|precheck -Work <folder> ...   (card-compiler.ps1)
//!   zt-digest model -Step <step> ...                          (model-compiler.ps1)
//!   zt-digest model-context / list-models ...                 (model-context.ps1, list-models.ps1)
//!   zt-digest finalize -Step plan|scope|refresh|final -Out <out> ...   (the finite finalization, #86)
//!   zt-digest concepts -Step prepare|judge|report|decision -Staging <folder> ...   (the concept review, #97)
//!   zt-digest register -Step plan|apply|check -Work <folder> [-Review <report>] ...   (the concept register, #101)
//!   zt-digest join  -Step plan|apply -Work <folder> [-File <selection>] ...   (existing Topic pairs and one approved join, #109)
//!   zt-digest pilot -Step plan|preflight|qualify -Staging <out> -Work <folder> ...   (the one-book pilot gate, #117)
//!   zt-digest publish -Step plan|apply|check -Work <folder> [-Register <p>] [-Hub <p>] [-File <s>] ...   (the existing-register and crossing-hub publication gate, #119)
//!   zt-digest resolve -Step plan|apply|check -Work <folder> [-File <s>] ...   (a separately selected historical resolution batch, #122)
//!
//! Each tool takes the parameters of the PowerShell script it replaces, in the
//! same `-Name value` form (names ignore case; a switch is `-Name` alone), and
//! prints what that script printed: a compiler step ends with one compact JSON
//! line. One process holds one `zt mcp` for all its reads and writes, and runs
//! zt-check in-process, so a step starts no pwsh, no helper, and no checker.

mod args;
mod audit;
mod bridge;
mod calls;
mod canon;
mod card;
mod check;
mod concepts;
mod earlier;
mod event;
mod faults;
mod figures;
mod finalize;
mod finding;
mod gate;
mod hub;
mod ledger;
mod link_gap;
mod lockcheck;
mod math;
mod model;
mod organize;
mod pilot;
mod prepare;
mod publish;
mod ps;
mod register;
mod report;
mod resolve;
mod restate;
mod skeleton;
mod snapshot;
mod terms;
mod topic_home;
mod grow;
mod topic_join;
mod util;
mod zt;

use anyhow::{Context, Result};

/// `zt-digest service-wait [-Wait <0..120>] [-Out <staging>]`: R320's stage health check.
///
/// It asks `zt status` at bounded intervals and waits at most `-Wait` seconds for the configured
/// restart/autostart path. It never launches a daemon of its own: `zt status` is the one path
/// that clears a stale record and starts one service, so concurrent stages share one restart.
/// After a valid restart it reconnects one client, which reports the archive revision the stage
/// continues on; the stage's own receipt replay is what reconciles writes the disappearance
/// interrupted. A service that stays down is one infrastructure fault (exit 4) that leaves the
/// run's units and files exactly as they are.
fn service_wait_cli(argv: &[String]) -> Result<i32> {
    let a = args::Args::parse(argv, &["Wait", "Out"], &[])?;
    let wait: u64 = if a.has("Wait") {
        a.required("Wait")?
            .parse()
            .ok()
            .filter(|seconds| *seconds <= 120)
            .context("-Wait must be a whole number of seconds from 0 through 120")?
    } else {
        120
    };
    let program = std::env::var("ZT_CHECK_ZT").unwrap_or_else(|_| "zt".to_string());
    let started = std::time::Instant::now();
    match zt_check::mcp::ensure_service(&program, std::time::Duration::from_secs(wait)) {        Ok(Some(facts)) => {
            // One client, opened after the restart: this is the reconnection the stage continues on.
            let reconnected = zt::start().is_ok();
            let archive = zt_check::mcp::observed_service().unwrap_or(facts);
            let value = serde_json::json!({
                "schema_version": 1,
                "ok": true,
                "state": "up",
                "waited_ms": started.elapsed().as_millis() as u64,
                "pid": archive.pid.clone(),
                "archive_root": archive.archive_root.as_ref().map(|root| root.display().to_string()),
                "archive_revision": archive.revision,
                "last_exit_abnormal": archive.last_exit_abnormal,
                "reconnected": reconnected,
                "stage": zt::staging().map(|out| out.display().to_string()),
            });
            util::print_line(&util::compact(&value))?;
            Ok(0)
        }
        Ok(None) => {
            // The program is not a `zt` that answers `status` (an isolated stand-in): nothing was
            // observed, so nothing is asserted about the service.
            let value = serde_json::json!({
                "schema_version": 1, "ok": true, "state": "unobserved",
                "waited_ms": started.elapsed().as_millis() as u64, "reconnected": false,
            });
            util::print_line(&util::compact(&value))?;
            Ok(0)
        }
        Err(error) => {
            let value = serde_json::json!({
                "schema_version": 1,
                "ok": false,
                "code": "infrastructure_fault",
                "waited_ms": started.elapsed().as_millis() as u64,
                "error": format!("{error:#}"),
                "stage": zt::staging().map(|out| out.display().to_string()),
            });
            util::print_line(&util::compact(&value))?;
            Ok(4)
        }
    }
}

fn main() {
    let mut argv = std::env::args().skip(1);
    let tool = argv.next().unwrap_or_default();
    let rest: Vec<String> = argv.collect();
    // R269: a write-capable step gives its child `zt mcp` long edit patience; a read
    // or a precheck keeps the ordinary short bound. Set once, before the one client
    // of this process starts.
    zt_check::mcp::child_edit_patience(if util::writes_cards(&tool, &rest) {
        util::WRITE_EDIT_PATIENCE_SECONDS
    } else {
        0
    });
    // R318: the invocation's staging folder ties its archive active-run marker to one run.
    zt::note_invocation(&rest);
    // docs/literature-only-digest-design.md: a digest step writes only the book's Literature tree.
    // The Topic-tree, hub and register tools (idea-grow's) keep the open scope.
    zt_check::mcp::scope::set_literature_only(literature_scope(&tool, &rest));
    let result = match tool.to_ascii_lowercase().as_str() {
        "call" => calls::cli(&rest),
        "service-wait" => service_wait_cli(&rest),
        "audit" => audit::cli(&rest),
        "card" => card::cli(&rest),
        "model" => model::cli(&rest),
        "model-context" => model::context_cli(&rest),
        "list-models" => model::list_cli(&rest),
        "hub" => hub::cli(&rest),
        "register" => register::cli(&rest),
        "bridge" => bridge::cli(&rest),
        "math" => math::cli(&rest),
        "event" => event::cli(&rest),
        "finding" => finding::cli(&rest),
        "terms" => terms::cli(&rest),
        "skeleton" => skeleton::cli(&rest),
        "organize-survey" => organize::survey_cli(&rest),
        "organize-apply" => organize::apply_cli(&rest),
        "figures" => figures::cli(&rest),
        "prepare" => prepare::cli(&rest),
        "source-faults" => faults::cli(&rest),
        "ledger" => ledger::cli(&rest),
        "finalize" => finalize::cli(&rest),
        "gate" => gate::cli(&rest),
        "concepts" => concepts::cli(&rest),
        "join" => topic_join::cli(&rest),
        "topic-home" => topic_home::cli(&rest),
        "grow" => grow::cli(&rest),
        "restate" => restate::cli(&rest),
        "pilot" => pilot::cli(&rest),
        "publish" => publish::cli(&rest),
        "resolve" => resolve::cli(&rest),
        "group-start" => ledger::group_start_cli(&rest),
        "group-finish" => ledger::group_finish_cli(&rest),
        "link-start" => ledger::link_start_cli(&rest),
        "link-finish" => ledger::link_finish_cli(&rest),
        "cards-finish" => ledger::cards_finish_cli(&rest),
        "book-cards-finish" => ledger::book_cards_finish_cli(&rest),
        "events-finish" => ledger::events_finish_cli(&rest),
        "findings-finish" => ledger::findings_finish_cli(&rest),
        "model-finish" => ledger::model_finish_cli(&rest),
        "explain-finish" => ledger::explain_finish_cli(&rest),
        "cleanup-finish" => ledger::cleanup_finish_cli(&rest),
        "model-deps" => ledger::model_deps_cli(&rest),
        "prepare-link" => prepare::link_cli(&rest),
        "snapshot-topics" => snapshot::cli(&rest),
        "version" | "--version" | "-v" => {
            println!("zt-digest {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        _ => {
            eprintln!("usage: zt-digest <tool> [-Name value ...]\ntools: call, grow, service-wait, audit, card, model, model-context, list-models, hub, register, join, topic-home, bridge, math, event, finding, gate,
       terms, skeleton, organize-survey, organize-apply, figures, prepare, prepare-link, source-faults, snapshot-topics, ledger, finalize, concepts, pilot, publish, resolve, group-start, group-finish, link-finish, cards-finish, book-cards-finish, events-finish, findings-finish, model-finish, explain-finish, cleanup-finish, model-deps");
            Ok(2)
        }
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            // R338: a step whose own write the archive refused with its typed Lock refusal answers
            // that refusal as its last stdout line and exits 2, so the Workflow stops the affected
            // write stage without reading the refusal as a provider fault or matching English text.
            if let Some(code) = crate::lockcheck::stopped_exit(&tool, &error) {
                std::process::exit(code);
            }
            eprintln!("zt-digest {tool}: {error:#}");
            std::process::exit(1);
        }
    }
}

/// The tools that compose Topic trees, hubs and the register (zt-idea-grow's, and the hand tools
/// beside them). Every other tool is a digest step and runs in the Literature scope, Organize's
/// survey and apply included: a digest's Organize merges no Topic and moves no Topic Card.
const OPEN_TOOLS: &[&str] = &[
    "grow", "hub", "register", "gate", "join", "topic-home", "publish", "resolve", "pilot",
    "restate", "snapshot-topics", "service-wait", "list-models", "version", "--version", "-v",
];

/// Whether this invocation runs in the Literature scope: every digest step, and `call` and
/// `organize-apply` unless given `-Open` (a hand-run helper call, or a Topic merge run by hand or by
/// a later idea-grow round, that may write a Topic tree).
fn literature_scope(tool: &str, rest: &[String]) -> bool {
    let tool = tool.to_ascii_lowercase();
    if tool == "call" || tool == "organize-apply" {
        return !rest.iter().any(|a| a.eq_ignore_ascii_case("-Open"));
    }
    !OPEN_TOOLS.contains(&tool.as_str())
}

#[cfg(test)]
mod scope_tests {
    use super::literature_scope;

    #[test]
    fn digest_steps_run_in_the_literature_scope() {
        for tool in ["group-finish", "link-finish", "card", "event", "finding", "bridge", "skeleton", "finalize", "prepare", "model", "math", "organize-apply", "organize-survey", "Card"] {
            assert!(literature_scope(tool, &[]), "{tool}");
        }
        assert!(literature_scope("call", &["-".into(), "-Allow".into(), "x".into()]));
    }

    #[test]
    fn idea_grow_and_hub_tools_keep_the_open_scope() {
        for tool in ["grow", "hub", "register", "restate", "gate", "topic-home", "join", "publish"] {
            assert!(!literature_scope(tool, &[]), "{tool}");
        }
        assert!(!literature_scope("call", &["-".into(), "-open".into()]));
        assert!(literature_scope("organize-apply", &["-Work".into(), "w".into()]));
        assert!(!literature_scope("organize-apply", &["-Work".into(), "w".into(), "-Open".into()]));
    }
}
