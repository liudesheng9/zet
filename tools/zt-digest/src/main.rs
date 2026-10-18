//! zt-digest: the book digest's tools as one binary (tools/zt-digest).
//!
//!   zt-digest call  <file|-> [-From N -Addresses a,b] [-Log f] [-Allow f] [-Moves f] [-SrcLog f] [-TopicGate f]
//!   zt-digest call  -Tool <tool> [-Json '{...}']
//!   zt-digest audit -CitationKey <key> -Out <folder> ...      (digest-audit.ps1)
//!   zt-digest card  -Step plan|apply|check|report|precheck -Work <folder> ...   (card-compiler.ps1)
//!   zt-digest model -Step <step> ...                          (model-compiler.ps1)
//!   zt-digest model-context / list-models ...                 (model-context.ps1, list-models.ps1)
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
mod event;
mod faults;
mod figures;
mod finding;
mod hub;
mod ledger;
mod math;
mod model;
mod organize;
mod prepare;
mod ps;
mod skeleton;
mod snapshot;
mod terms;
mod util;
mod zt;

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
    let result = match tool.to_ascii_lowercase().as_str() {
        "call" => calls::cli(&rest),
        "audit" => audit::cli(&rest),
        "card" => card::cli(&rest),
        "model" => model::cli(&rest),
        "model-context" => model::context_cli(&rest),
        "list-models" => model::list_cli(&rest),
        "hub" => hub::cli(&rest),
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
        "group-start" => ledger::group_start_cli(&rest),
        "group-finish" => ledger::group_finish_cli(&rest),
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
            eprintln!("usage: zt-digest <tool> [-Name value ...]\ntools: call, audit, card, model, model-context, list-models, hub, bridge, math, event, finding,
       terms, skeleton, organize-survey, organize-apply, figures, prepare, prepare-link, source-faults, snapshot-topics, ledger, group-start, group-finish, link-finish, cards-finish, book-cards-finish, events-finish, findings-finish, model-finish, explain-finish, cleanup-finish, model-deps");
            Ok(2)
        }
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("zt-digest {tool}: {error:#}");
            std::process::exit(1);
        }
    }
}
