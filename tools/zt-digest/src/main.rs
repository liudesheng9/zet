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
mod calls;
mod card;
mod check;
mod model;
mod util;
mod zt;

fn main() {
    let mut argv = std::env::args().skip(1);
    let tool = argv.next().unwrap_or_default();
    let rest: Vec<String> = argv.collect();
    let result = match tool.to_ascii_lowercase().as_str() {
        "call" => calls::cli(&rest),
        "audit" => audit::cli(&rest),
        "card" => card::cli(&rest),
        "model" => model::cli(&rest),
        "model-context" => model::context_cli(&rest),
        "list-models" => model::list_cli(&rest),
        "version" | "--version" | "-v" => {
            println!("zt-digest {}", env!("CARGO_PKG_VERSION"));
            Ok(0)
        }
        _ => {
            eprintln!("usage: zt-digest <tool> [-Name value ...]\ntools: call, audit, card, model, model-context, list-models");
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
