//! zt-check: the book digest's Card quality check as one binary.
//!
//!   zt-check --version
//!   zt-check --json <file> [--setting <address>] [--level concept|formal]
//!   zt-check --model <address> [--skip-shared]
//!   zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,R3,...]
//!
//! Output: one line per fault `<address> <rule> <text>`, then `faults: N`.
//! It is the one Card quality check and the only source of truth: Claude
//! sessions, workflow agents, DeepSeek runs, and the digest tools (zt-digest,
//! which runs it in-process) all call it.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = zt_check::check::run_cli(&args) {
        eprintln!("zt-check: {error:#}");
        std::process::exit(1);
    }
}
