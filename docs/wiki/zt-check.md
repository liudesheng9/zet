---
block: zt-check
title: zt-check
paths:
  - "tools/zt-check/**"
  - "scripts/install-zt-check.ps1"
synced: cf2cbf393ef3
updated: 2026-10-24
---

# zt-check

## Purpose

`zt-check` is the Card quality check of the book digest, built as one fast Rust binary. It reads Cards (from a JSON file of drafts, or from the live archive through `zt mcp`) and prints one line per fault against the Card language and structural rules (R2 to R19) and the Card layer rules (I, E, F, L). Every agent that writes digest Cards calls it: Claude sessions, workflow agents, Haiku managers, DeepSeek Harness runs, and the digest scripts through `Invoke-ZtCheck`. It is the only source of truth for Card quality (`tools/zt-check/src/main.rs`, `tools/zt-check/src/check.rs`). It exists as a crate of its own so that it builds and installs without building zt or stopping the ZT service (`tools/zt-check/Cargo.toml`, `scripts/install-zt-check.ps1`).

Since #143 the same crate is also a process-free library. `core` (the rules), `text` (the text semantics and the term-table parse), and `policy` (the rule catalog) build without the `cli` feature and hold no file reader, no `zt mcp` client, no process launcher, and no platform binding. The zt service links that core as a path dependency with `default-features = false` (`Cargo.toml`), so one definition of Card legality serves the standalone tool, zt-digest, and the service's in-process check. A change to a shared rule is therefore a service change too: rebuild and redeploy `zt` as well as `zt-check`/`zt-digest`.

Since #144 every fault is a structured `core::Diagnostic`: the address, the rule, a rule-specific fault code, an unclipped semantic key, the full message, the severity, the legacy flag, and the scope. The familiar line is formatted from it at the boundary. `policy::ratchet` compares the report of a Card's old authored version with the report of its proposed version under the same rules and policy, consumes old occurrences in multiplicity, and turns a retained fault into legacy advice; only an unmatched blocking fault still blocks. A draft bundle therefore carries `before` beside each proposed entry (design 5.4 and 6.3 of the Term Cards and shared Card check implementation design, 2026-10-24). `Diagnostic::to_json` returns the eight fields under their published names, with `severity` as `block`/`advise` and `scope` as `local`/`graph`, so a service, batch or future MCP reply carries the same value as the CLI line.

**Owns:** the rule definitions and their fault text, the order of fault lines, the default rule set of `--cards`, the size limits and their defaults, the Card categories of `--cards` mode, the `--json` entry and `before` bundle format, the pure rule core (`tools/zt-check/src/core.rs`), the pure text operations (`tools/zt-check/src/text.rs`), the rule catalog and the legacy ratchet (`tools/zt-check/src/policy.rs`), the `zt mcp` client in `tools/zt-check/src/mcp.rs`, the process-wide Literature write scope that client enforces (`tools/zt-check/src/mcp/scope.rs`), the library entry points (`tools/zt-check/src/lib.rs`: `core::Check`, `text`, `policy` always; `check::run_lines`, `graph`, `mcp::Zt` with the `cli` feature), and the build and install of `zt-check.exe`.

**Does not own:** the MCP tools it calls (`get_card`, `get_model`, `get_tree`, `list_trees`; see [MCP server](mcp-server.md)), the ZT service that `zt mcp` needs (see [Service, CLI and storage](service-and-cli.md)), Relation kinds and Link parsing inside zt (see [Cards, Workspaces and Links](cards-and-links.md)), the build report and Elaboration level (see [Models and math](models-and-math.md)), the PowerShell wrapper `Invoke-ZtCheck` and the compilers and audit that build its input (see [Digest scripts](digest-scripts.md)), the SELF-CHECK lines in the agent prompts (see [Digest prompts](digest-prompts.md)), the approval rules in the workflows (see [Digest workflows](digest-workflows.md)), and the `zt-digest-protocol` skill that documents it for agents (see [Agent skills](agent-skills.md)).

## At a glance

| | |
|---|---|
| Entry points | `zt-check` command line (`main` in `tools/zt-check/src/main.rs` → `check::run_cli`); the library (`zt_check::check::run_lines(args, Some(&mut zt))`, `zt_check::mcp::Zt`), which [zt-digest](zt-digest.md) runs in-process; the process-free core (`zt_check::core::Check`, `zt_check::text`, `zt_check::policy`), which the zt service links and calls in-process; `scripts/install-zt-check.ps1` (build and install) |
| Key files | `tools/zt-check/Cargo.toml` — a crate with its own `[workspace]`, so it never builds zt; features `default = ["cli"]` and `cli = ["dep:windows-sys", "dep:libc"]`; the binary has `required-features = ["cli"]`; `anyhow`, `regex`, `serde_json` are the pure core's only dependencies |
| | `tools/zt-check/Cargo.lock` — pinned versions (regex 1.13.1, anyhow 1.0.104, serde_json) |
| | `tools/zt-check/src/lib.rs` — exports `VERSION`, `core`, `text`, `policy` always; `check`, `graph`, `mcp` only with `cli` |
| | `tools/zt-check/src/main.rs` — collects the arguments, calls `zt_check::check::run_cli`, prints `zt-check: <error>` and exits 1 on an error |
| | `tools/zt-check/src/core.rs` — the rule core: `Check` (the language, layer, and title rules), `Diagnostic`/`Severity`/`Scope` (the structured fault contract), `Faults` (the diagnostics a run raises), `Limits`, the corruption scan, the derivation-facet predicate, and the orphan predicate; its unit tests |
| | `tools/zt-check/src/text.rs` — the PowerShell text semantics (UTF-16 form and lengths, culture comparisons, JSON conversions), the prose projection, sentence splitting, the term-table parse, the title context, and the lexical Link and envelope operations; its unit tests |
| | `tools/zt-check/src/policy.rs` — the rule catalog (`DEFAULT_RULES`, `ADVISORY_RULES`, `GRAPH_RULES`, `severity`, `scope`), `CheckReport` (diagnostics and blockers), `Applicability` (the per-rule signature a metadata change compares), and `ratchet` (the legacy comparison); its unit tests |
| | `tools/zt-check/src/check.rs` — the CLI input adapter: the four input modes, the option parser, the output, and the compatibility re-exports (`check::run_lines`, `check::term_table`, ...) |
| | `tools/zt-check/src/graph.rs` — the whole-archive classification and orphan report (`cli`); re-exports the core's `is_orphan`, `SUBSTANTIVE`, `EXCLUDED` |
| | `tools/zt-check/src/mcp.rs` — `Zt`, a minimal JSON-RPC client that starts `zt mcp` and makes `tools/call` requests |
| | `tools/zt-check/src/mcp/owned.rs` — owned process-tree containment, bounded exit observation and termination |
| | `tools/zt-check/src/mcp/scope.rs` — the process-wide Literature write scope: `set_literature_only`, `literature_only`, `is_topic_address`, `topic_links`, `literature_refusal` |
| | `tools/zt-check/tests/check_parity.rs` — the executable, the library adapter, and the process-free core print the same ordered lines for both recorded fixtures, and the executable and the library print the same ratcheted `before` bundle; the envelope error and the version are the same too |
| | `tools/zt-check/tests/fixtures/` — the recorded ordered lines of both fixtures, the `before` bundle (`legacy-bundle.json` with `legacy-expected.txt`), and the `--json` envelope fixtures (single object, BOM, bundle context, case-folded names, shared line, not JSON) |
| | `scripts/install-zt-check.ps1` — `cargo build --release` into `D:\zt-check-target`, then copies `zt-check.exe` beside `zt.exe` |
| Depends on | [MCP server](mcp-server.md) (`zt mcp`, read tools only; `cli` feature); [Service, CLI and storage](service-and-cli.md) (the service must be up for `zt mcp` tool calls); the answer shapes of [Models and math](models-and-math.md) (`get_model` `head`, `parts`, `level`, `build.setting`, `build.order`) and [Cards, Workspaces and Links](cards-and-links.md) (`get_tree` `links`, `kinds`); Rust crates `regex`, `serde_json`, `anyhow` |
| Used by | [zt-digest](zt-digest.md) (a path dependency with the default `cli` feature: `Check::run` calls `run_lines` on the step's own `zt mcp`; `main` sets the Literature scope for every digest step through `mcp::scope::set_literature_only`); the zt service (a path dependency with `default-features = false`: `src/main.rs` links the crate and `tests/check_core.rs` checks a Card through `core::Check` in-process); [Digest scripts](digest-scripts.md) (`Invoke-ZtCheck` in `scripts/book-digest/zt-calls.ps1`, for PowerShell callers and the tests; every digest script now forwards to a `zt-digest` tool that runs the check in-process); [Digest prompts](digest-prompts.md) (DeepSeek SELF-CHECK commands); [Digest workflows](digest-workflows.md) (managers approve `zt-check` as read-only); [Agent skills](agent-skills.md) (`skills/zt-digest-protocol/SKILL.md`, `skills/zettelkasten/SKILL.md`) |
| Tests | `mod tests` in `tools/zt-check/src/core.rs` (the rules and the structured diagnostics), `src/text.rs` (the text semantics and term table), `src/policy.rs` (the rule filter, `CheckReport`, `Applicability`, and the ratchet), and `src/mcp/scope.rs`; `tests/check_parity.rs` (the CLI, the library, and the core on both recorded fixtures, plus the `before` bundle); `scripts/book-digest/tests/zt-check.tests.ps1` with `card-style-fixture.json`; `scripts/book-digest/tests/card-layer.tests.ps1` with `card-layer-fixture.json`; the root crate's `tests/check_core.rs` (the service link) |

## Concepts

- `Card language` — the plain-text rules for Card titles and bodies (ASD-STE100 adapted for Chinese). zt-check checks rules 2 to 7, 9, 12 to 17, and the standalone pilot rules 20 and 21 of the list in [docs/self-sufficient-models-proposal.md](../self-sufficient-models-proposal.md) and [docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md); rules 1, 8, 10, and 11 are not mechanical and are not checked (`Check::language`, `Check::sentence`, `local_reference`, `unnamed_referent` in `tools/zt-check/src/check.rs`).
- `Card layer rules` — the checks on Cards outside models: I (Idea Cards), E (Event Cards), F (Finding Cards), L (Links). They run only on entries that carry a `category` (`Check::layer`, `finding_rules`).
- `Diagnostic` — one fault under the structured contract (`core::Diagnostic`, design 5.4): `address`, `rule`, `fault`, `key`, `message`, `severity`, `legacy`, `scope`. `fault` is a rule-specific code (`sentence_length`, `missing_labels`, ...); `key` is the rule-specific semantic identity, built from the full offending fragment or missing requirement, never from a line number or the clipped display; `message` is the full text. `Diagnostic::line` formats the familiar line at the boundary; `Diagnostic::to_json` returns those eight fields as the design's JSON document (the policy names `block`/`advise` and `local`/`graph`).
- `Fault line` — `<address> <rule> [(advisory) ]<message>`: the text on one line, white space collapsed, clipped to 90 UTF-16 units with `…` (`Diagnostic::line`). A run ends with `faults: N`.
- `Severity` / `Scope` — `block` or `advise`, and `local` (the Card and its bounded context) or `graph` (whole-archive evidence: I5, I6, E4, E5, F4, L3). `policy::severity` and `policy::scope` hold the default catalog; I7's unknown-source subcase is marked by the core (`Faults::advise`). A `graph` fault is audit evidence, not a per-write decision (design 6.2). S03 adds archive overrides on this catalog.
- `Advisory` — a diagnostic whose severity is `advise`: its line carries `(advisory)`. The default advisory rules are `R20`, `R21`, `I1`, `I6`, `L3`, `E5`, plus I7's unknown-source subcase (`policy::ADVISORY_RULES`). In `--cards` mode it is printed but not counted (`filter_rules`); in `--json`, `--model`, `--addresses` every line counts; in a `before` bundle the count is the number of blockers.
- `Legacy fault` — a diagnostic the old authored version of the same Card already carried, matched by `(rule, fault, key)` in multiplicity under the same rules and policy (`policy::ratchet`); the CLI pairs each `before` entry with its proposed Card, so a Card that moves to another address still keeps an unchanged fault. It becomes `legacy: true` advice and stops blocking. The same identity appearing more often than before is a new occurrence and blocks.
- `Applicability` — the facts a metadata change can alter, grouped into a per-rule signature (`policy::Applicability`): category, level, facet/derivation, shared term table, book, and the authored term name. A rule is compared only when its signature is unchanged, so a Card retyped from Idea to Event must meet the Event format, a concept model promoted to formal must meet R13–R17, and a term-table or book change is a new requirement. A rule the catalog does not name yet keeps the conservative signature: category, level, part/facet, derivation, shared list, book and term name must all be unchanged, so a rule added later is never inherited across a transition until its own signature is named.
- `CheckReport` — the assembled result of one check (`policy::CheckReport`): every diagnostic in the core's traversal order and `blockers`, the number that still block. `lines()` formats the report. It holds no saved baseline: both versions are computed on every run (design 6.4).
- `Standalone rule` — R20 and R21, the pilot's text heuristics for a Card that must stand alone (D125–D134). They are advice in the pilot: they name the matched fragment, the text and the reason, and a semantic review still decides.
- `Source evidence` — the Citation keys a Card's claim comes from, as I7 reads them: an explicit `sources` set, else the `来源` line's Literature Links, else the Literature tree the Card is filed in. Empty is unknown, never a pass (D069–D073).
- `Entry` — one Card as zt-check reads it: a JSON object with `address`, `title`, `body`, `facet`, and optional layer fields (see "Data and storage"). Property names ignore case (`prop`).
- `Category` — what a Card is for the layer rules: `idea`, `anchor`, `event`, `finding` get layer rules; `root`, `hub`, `part`, `model`, `terms`, `structure` get the R rules only. `--cards` mode computes it as `digest-audit.ps1` (`Get-Category`) does (`check_cards`).
- `Model part` / `Model Card` — read from `get_model`; a Card with a `facet` is a part. A part whose facet starts with `推导` (or `derivation`, any case) is a `Derivation part`, which gets the step and explanation rules R14 to R17 (`Check::language`).
- `Elaboration level` — `concept` or `formal`. A concept model skips R13 to R17; a model with no level gets the formal rules (`Check::language`).
- `Term table` — the table under the line holding `术语表` in the setting Card's body (a model's setting, or the Book term table): `| term | words not to use | 意思 | 首次定义 |`. R6 reads the first two columns (`read_terms`, `Check::with_terms`); every left-column term, rows without a rejected word too, is kept longest first in `Check::kept` and masked before R6 matches; `term_table` is the public parser the digest uses for all four (`zt_check::check::term_table`, `TermRow`; #97, D063). Both share one row parse, so R6 and the digest never disagree about a row: `不用` splits on `、,，/`, allows the `—`/`无` of "none", and a later row naming the same term (case ignored) replaces the earlier one. R6 still skips a row with no rejected word, exactly as before, and the four-column reader reports such a row with an empty `avoid`.
- `Shared part` — a part whose `build.order` item (or `--json` entry) has `line: shared`. It follows its home model's term table, so R6 skips it; `--skip-shared` drops it from a `--model` run (`check_model`, `Check::with_shared`).
- `Limits` — the size numbers of R13, R14, R17, I2, L2 (`Limits` struct; defaults in "Configuration").
- `Literature scope` — a process-wide write scope of the `zt mcp` client (`mcp::scope`), off by default (the open scope). A process that turns it on (every [zt-digest](zt-digest.md) digest step) can write only Literature trees: `Zt::request` refuses `create_topic`, a write that names a Topic-tree Card, a Hub Card, and authored text that links a Topic-tree Card, before the call reaches the archive (`docs/literature-only-digest-design.md`). Reads always pass.
- `Topic address` — an address, Citation key or Link target whose tree part (the text before the first `/`) is all ASCII digits: a Topic tree's id. `is_topic_address` decides it from the text alone, with no archive read; `Smith2024/1` and `2002grossman/1` are Literature addresses.
- `UTF-16 form` — text in which each character outside the Basic Multilingual Plane is two private-use stand-ins (U+E000.., U+E400..), so lengths, regex classes, and clips match .NET (`utf16_form`, `from_utf16_form`).

## Architecture

The crate has a library and a binary (`main.rs`). The library is layered by feature. `core`, `text`, and `policy` are always available and are the process-free part: `text` owns the PowerShell text semantics, the prose projection, the term-table parse, the title context, and the lexical Link and envelope operations; `core` owns `Check` (the language rules per Card, then the layer rules across Cards), `Diagnostic`/`Severity`/`Scope` and `Faults` (the structured faults and their line format), the corruption scan, and the orphan predicate; `policy` owns the rule catalog (`severity`, `scope`, `DEFAULT_RULES`), `CheckReport`, `Applicability`, the legacy `ratchet`, and the filter that counts a `--cards` report. `check`, `graph`, and `mcp` need the `cli` feature: `check` reads the four inputs, materializes the term table, level, shared list, and Workspace title context, calls the core, applies the ratchet when a bundle carries `before`, filters, and prints; `graph` classifies a whole archive for the audit; `mcp` is the `zt mcp` client and its process containment. `run_lines(args, zt)` takes an open `Zt` or, given `None`, starts one only when the mode reads the archive (`--json` and an empty `--cards` start none).

```mermaid
flowchart TD
    main["main.rs: main()"] --> cli["check::run_cli (cli)"]
    cli --> parse["Options::parse"]
    cli -->|--json| cj["check_json (file, offline)"]
    cli -->|--addresses| ca["check_addresses"]
    cli -->|--model| cm["check_model"]
    cli -->|--cards / --log| cc["check_cards"]
    ca & cm & cc --> zt["mcp::Zt (spawns zt mcp)"]
    zt -->|get_cards, get_model| server[("zt mcp → ZT service archive")]
    cj & ca & cm & cc --> run["core::Check::report (always available)"]
    run --> lang["Check::language + Check::sentence (R2–R22)"]
    run --> layer["Check::layer + finding_rules (I, E, F, L)"]
    lang & layer --> faults["Faults::add → Diagnostic"]
    lang & layer --> txt["text: prose, term table, titles, Links"]
    faults --> lines["Diagnostic::line (clip, advisory prefix)"]
    cj -->|before bundle| ratchet["policy::ratchet (old vs proposed)"]
    ratchet --> report["policy::CheckReport (diagnostics, blockers)"]
    report --> cj
    cc --> filter["policy::filter_rules (--rules / DEFAULT_RULES)"]
    lines --> out["write_lines + faults: N"]
    filter --> out
    svc["zt service (root crate)"] -->|"path dep, default-features = false"| run
```

| Component | File | Role |
|---|---|---|
| `main` | `tools/zt-check/src/main.rs` | Arguments in; on `Err`, `zt-check: {error:#}` on stderr and exit code 1 |
| `Options`, `run_cli`, `write_lines` | `tools/zt-check/src/check.rs` | Parse flags, pick one mode, run, filter, print UTF-8 with CRLF on Windows |
| `check_json`, `check_addresses`, `check_model`, `check_cards` | `tools/zt-check/src/check.rs` | Build the entries, the term table, the level, the shared list, and the title context for one mode |
| `Check` (`new`, `with_terms`, `with_shared`, `with_context`, `run`, `language`, `sentence`, `layer`) | `tools/zt-check/src/core.rs` | The rules; `run` returns the ordered fault lines |
| `Diagnostic`, `Severity`, `Scope`, `Check::report`, `Faults`, `Limits`, `corruption`, `source_evidence`, `is_orphan` | `tools/zt-check/src/core.rs` | The structured fault contract and its line format, the limit defaults, the R18/R19 scan, I7's source evidence, and the orphan predicate I5 and the audit share |
| Text helpers (a `?` or `!` inside 《…》 ends no sentence in `split_sentences`; the Bridge label `跨书关联：` (`BRIDGE_LABEL`) is stripped before a line's sentences are measured; `plain`, `measure`, `prose_lines`, `split_sentences`, `passive`, `read_terms`, `leads_to`, `book_place`, `bigrams`, `similarity`) | `tools/zt-check/src/text.rs` | What text the rules see and how it is counted |
| PowerShell semantics (`ps_eq`, `ps_in`, `culture`, `culture_starts_with`, `culture_cmp`, `ps_string`, `truthy`, `strings`, `ps_int`, `key`, `utf16_form`) | `tools/zt-check/src/text.rs` | Reproduce the deleted PowerShell checkers' comparisons and conversions |
| `DEFAULT_RULES`, `ADVISORY_RULES`, `GRAPH_RULES`, `severity`, `scope`, `CheckReport`, `Applicability`, `ratchet`, `filter_rules` | `tools/zt-check/src/policy.rs` | The rule catalog (severity and scope per rule), the assembled report, the applicability signature, the legacy ratchet, and the report filter |
| `Zt` (`start`, `call`, `request`, `Drop`) | `tools/zt-check/src/mcp.rs` | The `zt mcp` client (`cli`) |
| `scope` (`set_literature_only`, `literature_only`, `is_topic_address`, `topic_links`, `literature_refusal`) | `tools/zt-check/src/mcp/scope.rs` | The Literature write scope `Zt::request` checks before each tool call (`cli`) |

### Feature boundary and the service link (S01 / #143)

`tools/zt-check/Cargo.toml` declares `default = ["cli"]` and `cli = ["dep:windows-sys", "dep:libc"]`; the binary has `required-features = ["cli"]`. Building with `--no-default-features --lib` therefore compiles only `core`, `text`, and `policy`, and `cargo tree --no-default-features -e normal` names no `windows-sys`, no `libc`, and no archive client. `check.rs` keeps the existing public surface as re-exports (`check::run_lines`, `check::run_cli`, `check::term_table`, `check::TermRow`, `check::source_evidence`, `check::title_key`, `check::title_bigrams`, `check::title_similarity`, `check::TITLE_SIMILARITY`, `check::title_fits`, `check::sentence_fits`), so zt-digest and the digest reports keep their calls while the implementation lives in `core` and `text`.

The root crate (`Cargo.toml`) adds `zt-check = { path = "tools/zt-check", default-features = false }`. There is no dependency cycle: zt-check depends only on `anyhow`, `regex`, and `serde_json`, and zt-digest's `zt-check` path dependency keeps the default `cli` feature. The root's `tests/check_core.rs` proves the link with an in-process check through `zt_check::core::Check`, with no service, no `zt mcp`, and no gate process. Because the rules now ship in the service binary too, a shared-rule change is a deployment of the service (see [Service, CLI and storage](service-and-cli.md)): rebuild and reinstall `zt`, stop and start the daemon, and rebuild `zt-check`/`zt-digest`. A running old service would keep checking with the old rules.

### Structured diagnostics and the legacy ratchet (S02)

`core::Check::report(cards) -> Vec<Diagnostic>` is the rule evaluation; `Check::run` stays the compatibility wrapper that formats the same list (`Diagnostic::line`), so zt-digest's `run_lines` and every fixture keep their bytes. A `Diagnostic` is `address`, `rule`, `fault`, `key`, `message`, `severity`, `legacy`, `scope`; the published JSON form (`Diagnostic::to_json`) is:

```json
{ "address": "K/14", "rule": "E2", "fault": "missing_labels", "key": "labels:参与者,来源",
  "message": "missing labels: 参与者、来源", "severity": "block", "legacy": false, "scope": "local" }
```

`policy::CheckReport { diagnostics, blockers }` assembles one check; `CheckReport::of` counts `severity == block`, and `CheckReport::lines()` formats the familiar text. `policy::ratchet(before, proposed, comparable)` consumes the paired old report's `(rule, fault, key)` multiset one occurrence at a time in the proposed report's order, marks a consumed diagnostic `legacy: true` with `Severity::Advise`, and returns the proposed report with its remaining blockers. It never reads a clipped line, a line number, or a set of identities, so moving an unchanged faulty paragraph keeps its fault, a second copy of a known faulty sentence is a new occurrence, and a different fragment that clips to the same prefix stays distinct.

`policy::Applicability::of(entry)` reads `category`, `level`, `facet`, `line`, and `book`; `comparable(proposed, rule)` compares the rule's own signature. The text rules (R2–R5, R7, R9, R12, R18–R22) mean the same for every type; R6 follows the shared term table; R13–R17 follow the facet, derivation, and formal level; I1/I2/I4/I7 follow the Idea role, I3/F2 also the book; the E/F rules follow the Event/Finding role; L follows the layer role; an unnamed rule falls back to the whole metadata, so a term rule added later is not grandfathered across a term transition until its signature is named.

A `--json` entry may carry `before`: the old authored version as an entry object (its own `title`, `body`, `category`, `facet`, `level`, `line`, `book`), or `null`/absent for a new Card. When at least one entry has a non-null `before`, `check_json` checks the old entries and the proposed entries with one `Check` (same limits, term table, title context), ratchets the proposed report, and returns `faults: N` as the number of remaining blockers; a bundle with no `before` keeps the old output and count. A `before` that is not an object is refused. The old entry's own metadata is what its report is checked with, so a concept→formal or Idea→Event change is evaluated under the old metadata and compared only under the signatures that did not change. There is no persistent baseline table: both sides are computed on every run (design 6.4), and a rule-version change applies to both sides at once.

## Interfaces

`mcp::Zt::archive_view()` makes one `get_cards` call with no addresses and validates the complete empty read, revision, persisted 32-hex archive id and nonempty resolved path. It returns that verified view for preparation/ledger binding. Missing identity or incomplete protocol data raises an explicit error before a caller begins writing; ordinary bulk readers retain their existing API.

`mcp::Zt::server_version()` returns the `version` of the `zt` build from the `initialize` answer the client already received (empty when the server named none). The digest audit records it as the software version that measured the archive (#94 S01), so no report parses `zt --version` or starts a second process.

`check::VERSION` is `lib.rs`'s `zt_check::VERSION` (the crate version), and `zt-check --version` prints it as `zt-check <version>` without starting a client (#102 S25). A review records the checker that produced its lines from the binary itself, so a copied or replaced `zt-check.exe` states which one it is; the version argument is answered before any option parsing or archive read.

### Shared title and source evidence (#94 S01)

Three functions are public so a digest report uses the checker's rules instead of a second copy of them:

| Item | Behaviour |
|---|---|
| `check::source_evidence(card)` | D068–D072's direct set: an explicit `sources` array, else the Citation keys the Card's `来源` lines name, else the Literature tree it is filed in. Empty is unknown, which is not a pass. I7 and the digest audit's `audit-sources.json` call it. |
| `check::title_key(title)` | The title's identity for an exact pair: its letters and digits, lower cased, so case, spacing and punctuation never split two spellings of one title. |
| `check::title_bigrams(title)`, `check::title_similarity(a, b)`, `check::TITLE_SIMILARITY` | I6's own bigram set, the Dice coefficient between two titles, and the 0.8 threshold I6 reports at. A report builds one inverted index over these bigrams with case-folded keys, so neither a near pair nor an exact copy needs a pairwise scan. |


### Shared graph classification and orphan evidence

`tools/zt-check/src/graph.rs` is the pure shared graph contract. `category(card, literature, anchor_counts)` replaces separate audit/compiler/standalone classification code; `category_with_parts(card, literature, anchor_counts, declared_register_parts)` is the same classification with the register's own declaration, whose Cards are `register-part`: a navigation Card that carries term rows, never a substantive Idea (see below). A Card whose body holds a `## 同述入口` section (or whose catalogue row says `restatement`) is `restatement-entry`, the derived navigation role of an accepted restatement entry (§4.5, D086): it is navigation, so it takes no claim-title, Idea-size or duplicate-title rule, and it is excluded from the orphan population. `category` is that function with no declared part, for a scope that read no register. It retains typed Finding/Event/Model/part/Hub distinctions and Literature anchors, term tables and structure inference; notation tables are explicit `notation`. Callers supply the complete anchor counts needed by structure inference. `check::check_cards` obtains those counts from one fresh, complete `export_titles` at the same revision as its selected body reads; it does not infer a whole-book denominator from a requested subset.

`graph::is_orphan(address, category, inbound)` defines one orphan: an eligible substantive Regular Card without a valid authored inbound Link from another Card. Eligible categories are `idea,event,finding,model,part`; excluded purposes are `root,anchor,structure,terms,notation,hub,register-part,restatement-entry`. A self-Link does not help. Filing, a navigation parent, outbound citations and generated Reverse text are not authored inbound evidence. An actual authored Link from the parent does count. Inbound sources span both Workspaces; repair targets stay in the agent Workspace.

`graph::orphan_report(cards, revision, changed_cards)` requires explicit classification and complete inbound/Idea-kinded facts. It returns sorted target/eligible/excluded/orphan addresses, count, source Workspaces, revision and a separate `inbound_from_idea_with_kind` map. Missing facts, unknown categories, duplicate targets and human repair targets are errors. `changed_cards` describes authored target changes; callers label the baseline with `changes_since`. No stronger layer statistic is used to define orphanhood. I5 uses the same predicate; E4/F4 continue to report the stronger Idea-kinded evidence separately.


`mcp::Zt::cards(addresses, view)` returns `CardRead { cards, archive_revision }` through that command's existing MCP process. It chunks at 200 addresses, halves a chunk after a structured byte/address limit error, and fails when a single Card still cannot fit. It verifies complete/missing/count/address order and a common revision across chunks. Missing input or a changing read view fails explicitly; it cannot become an empty successful dataset.

`--addresses` and `--cards` use lean batches through one MCP process. `--addresses` asks for a model context only when its first Card is a Model Card/part. `--cards` uses the returned Link/kind/parent facts and reads each required primary model context once when its filed members establish ownership; a shared part's home is resolved independently. The valid numeric Topic ids and letter-led Literature keys identify agent Literature membership without tree scans. The existing categories, language and layer rules are unchanged.

`--model` uses the head and parts in its complete model context. Required external settings use lean batches. Each live mode compares the archive revision before and after collecting inputs and refuses a changing view. Public temporary-archive tests retain language, term-table and formal explanation faults while blocking old per-Card/tree reads. See `docs/digest-reliability-delivery-status.md` for the current completion boundary.

### Command line

```text
zt-check --version
zt-check --json <file> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
zt-check --model <address> [--skip-shared] [--setting <address>] [--level concept|formal] [--require-explain] [limits]
zt-check --addresses <a,b,...> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,R3,...] [limits]
limits: --max-sentences 12 --max-chars 400 --max-steps 6 --max-explain-sentences 16
        --max-explain-chars 600 --min-idea-chars 40 --max-idea-chars 300 --min-reason 8
```

(`USAGE` in `tools/zt-check/src/check.rs`; the doc comment in `main.rs` lists the modes.)

| Mode | Input | Reads zt | Term table (R6) | Level | Layer rules | `faults: N` counts |
|---|---|---|---|---|---|---|
| `--json <file>` | A JSON array of entries, or one entry (BOM allowed) | no | `--setting` names an entry in the file; its body gives the table | `--level`, else none (formal rules) | yes, for entries with `category` | every line |
| `--addresses <list>` | Addresses (split on commas and white space) | Bounded `get_cards` text batches; `get_model` for the first Card only when it is a Model Card/part; final revision probe | `--setting`, else the first model's `build.setting`; a required external setting is another lean batch | `--level`, else the first model's `level` | no | every line |
| `--model <address>` | Any Card of a model | `get_model` head/parts; revision probes; lean external setting when required | `--setting`, else `build.setting` | `--level`, else the model's `level` | no | every line |
| `--cards <list>` / `--log <file>` | Addresses from the list plus one per line of the log file | Bounded lean batches; required primary model contexts shared across known filed members; one complete `export_titles` that is also I6's Workspace context; final revision probe | each model's own setting | each model's own level | yes, by computed category | hard (non-advisory) lines of the selected rules |

Flags:

| Flag | Applies to | Effect |
|---|---|---|
| `--version`, `-v` | alone | Prints `zt-check <crate version>` and exits 0 before any archive read; with any other argument the run is a check, never a version answer |
| `--setting <address>` | `--json`, `--model`, `--addresses` | The setting Card whose `术语表` gives R6's term table |
| `--level concept\|formal` | `--json`, `--model`, `--addresses` | Overrides the level; any other value is an error; lower-cased |
| `--require-explain` | `--json`, `--model`, `--addresses` | R15 to R17 also on Derivation parts with neither 思路 nor 解读 (older parts) |
| `--skip-shared` | `--model` only | Drops shared parts from the run |
| `--log <file>` | `--cards` | Adds one address per line; a missing file is ignored silently |
| `--book <key>` | `--cards` only | The Citation key for I3 and F2 on every layer Card; default: the Card's tree when it is an agent Literature tree |
| `--rules <list>` | `--cards` only | The rules to print and count; default `DEFAULT_RULES` |
| `--max-*`, `--min-*` | all | The limits; each takes a whole number |
| `-h`, `--help` | — | Prints the usage as an error (exit code 1) |

Errors (exit code 1, `zt-check: <message>` on stderr, no `faults:` line): no mode or two modes (`name one input: --json, --model, --addresses, or --cards/--log`); `--cards` with `--setting`, `--level`, `--require-explain`, or `--skip-shared`; `--book` or `--rules` outside `--cards`; `--skip-shared` outside `--model`; a flag without its value; a limit that is not a whole number; an unknown option; a file that cannot be read or is not JSON; `zt mcp` that does not start or closes; a tool error (`zt: <message>`, for example `the ZT service is not available; run zt up`); `get_model` of a Card in no model (`` `<at>` belongs to no model``).

### Output

```text
K/1|a|a|1 R3 E 在时点 1 付不出去；E 能部分付出。
K/1|a|a|4 R13 (13 sentences > 12) split the part into two or more parts
5/1 I1 (advisory) the title states no claim: 税
faults: 3
```

UTF-8 whatever the console code page, CRLF line ends on Windows and LF elsewhere (`write_lines`). With `--cards` and no addresses, the output is `no Cards to check` and `faults: 0`. A run that prints no `faults:` line did not run.

### Rules

The default column is the default rule set of `--cards` (`DEFAULT_RULES`). The other modes print every rule they check.

| Rule | Checks | Applies to | Default | Fault text |
|---|---|---|---|---|
| R2 | Sentence length: Chinese at most 40 characters (30 in a numbered Derivation step); other text at most 25 words (20 in a step). Formulas, Links, code, bold marks, quotes and brackets do not count | every prose sentence | yes | `(N characters > 40) <sentence>` |
| R3 | No `；` or `;` | every sentence | yes | `<sentence>` |
| R4 | No `被` passive, except `被告`, `被保险`, `被动`, `被子`, and a party (`被收购方`: one or two CJK characters then `方`) | every sentence | yes | `<sentence>` |
| R5 | No light verb + CJK (`进行`, `加以`, `予以`, `作出`, `做出`) | every sentence | yes | `<sentence>` |
| R6 | No word the term table rejects for a term (every term the table keeps, its left column, is masked first, so a rejected word inside another kept term is that term) | every sentence of a non-shared Card, when a term table is known | no | ``uses `<word>` for `<term>`: <sentence>`` |
| R7 | At most two `的` in one noun phrase (three `的` with at most 7 non-punctuation characters between) | every sentence | yes | `<sentence>` |
| R9 | No hedge or praise word (18 listed in `HEDGES`, such as `值得注意的是`, `精妙`) | every sentence | yes | `` `<word>`: <sentence>`` |
| R12 | Title at most 40 characters (25 words), and no `；`/`;` (two claims) | every title | yes | `(N characters > 40) <title>` or `two claims: <title>` |
| R13 | A Model part holds at most `max-sentences` prose sentences, else at most `max-chars` Chinese prose characters. Steps and explanation do not count | Cards with a facet, not in a concept model | yes | `(N sentences > 12) split the part into two or more parts` |
| R14 | A Derivation part holds at most `max-steps` numbered steps | Derivation parts, not in a concept model | yes | `(N steps > 6) split the derivation into a chain of Derivation parts` |
| R15 | The sections are exactly `思路, 起点, 步骤, 结果, 解读`, in that order | Derivation parts that have 思路 or 解读 (all of them with `--require-explain`), not in a concept model | yes | `sections <found>; want 思路, 起点, 步骤, 结果, 解读` |
| R16 | 思路 at most 5 sentences; 解读 at most 6 (`待写` does not count) | as R15 | yes | `(思路 has N sentences > 5)` |
| R17 | One `说明` line per numbered step, not before the first step, of at most 2 sentences; 思路 + 说明 + 解读 together at most `max-explain-sentences` sentences, else at most `max-explain-chars` characters | as R15 (the position errors are reported in every Derivation part) | yes | `a second 说明 line under step N: …`, `(N explanation sentences > 16) shorten the explanation` |
| R18 | Exact repeated substantive prose paragraph, after only line-ending/outer-whitespace normalization | every authored body outside excluded regions | yes | `repeated prose paragraph at lines A-B and C-D` |
| R19 | A repeated Card title heading in authored body text | every authored body outside excluded regions | yes | `repeated Card title at body line N` |
| R20 | Advisory: a local label that stands in for content — `式 (12)`, `命题 3.G.3`, `第 6 章`, `模型 1`, `本节`/`本章`, `§11.D`. The text left after the labels must carry at least `LOCAL_REFERENCE_CONTENT` (5) characters of content, so `式 (12) 的加成项来自需求弹性` passes and `式 (12) 有该项` does not. `第一福利定理` is a name, code/quotes/formulas are skipped, and a `来源` line may carry the locator (D126, D132–D134) | every title and authored prose sentence | yes (advice) | ``(advisory) local reference `<label>` names no content: <text>`` |
| R21 | Advisory: a deictic phrase (`那里`, `那儿`, `那张卡`, `这张卡`, `该卡`, `该卡片`) with no `[[Link]]` in the same sentence. A Link in the sentence before does not name it (D127, D134) | every title and authored prose sentence | yes (advice) | ``(advisory) unnamed referent `<word>` with no named Link in its sentence: <text>`` |
| R22 | An inline formula left open: an odd number of unescaped `$` in the title, or in the body once `$$` display blocks, fenced code and inline code are set aside (`open_math`). A writer that builds a Card in a PowerShell double-quoted string loses `$A_I` and leaves `<A_O$` | every title and authored body | yes | `the title leaves an inline formula open (an odd number of $): <title>` / `the body leaves an inline formula open …` |
| I1 | Advisory: a title of at most 6 characters with no claim verb (`CLAIM_VERB`) | `idea`, `anchor` | yes | `(advisory) the title states no claim: <title>` |
| I2 | Prose size: over `max-idea-chars` is a split candidate, under `min-idea-chars` a merge candidate | `idea` | yes | `(N characters > 300) split candidate` |
| I3 | A Link to the book's Literature Card or one of its anchors (`<key>/<n>`) | `idea`, `event` with `book` and `links` | yes | `no source line: no Link to [[key]] or one of its anchors` |
| I4 | A kinded Link to a Card other than the parent | `idea` with `kinds` | yes | `no kinded Link to a Card other than the parent` |
| I5 | Another Card has a valid authored inbound Link here; a self-Link does not count | substantive categories with `inbound` | no | `no other Card has an authored Link here` |
| I6 | Advisory: no other title in the same Workspace with bigram (Dice) similarity of 0.8 or more, or an exact normalized equality. Candidates come from an inverted bigram index over the Workspace title context (its keys fold case, so an exact copy that differs only in case is a candidate too: no per-Card scan of the catalogue); only pairs touching the checked Cards are reported, once per unordered pair. A context supplied as incomplete (`"complete": false`, or no `complete` key) reports one line per checked Idea Card naming the known rows, so a tree-only read never claims a Workspace-wide check. A context supplied for an entry that names no `workspace` reports the scope as unknown and compares nothing, so no pair crosses a Workspace boundary | `idea` (a checked Idea Card; the context is read-only) | no | `(advisory) a title very similar to [[a]]`; `(advisory) Workspace title context incomplete: 1 known title compared, others unknown`; `(advisory) Workspace scope unknown: the entry names no Workspace, so nothing is compared` |
| I7 | A Topic Idea Card's `rests-on` Link enters its own source's Literature tree. `illustrates`, `corresponds`, and `contrasts` are no dependency, another book is none, and a Card filed in that tree rests on it by design. Missing source evidence is reported as unknown, never as a pass | `idea` with `relations` | yes | `` `rests-on` [[a]] enters its own source `key` `` or `(advisory) source evidence unknown, so [[a]] may be an own-source premise` |
| E1 | A date in the title (a year 1000–2099, or a number + `年`/`世纪`) | `event` | yes | `no date in the title: <title>` |
| E2 | The labels `时间`, `地点`, `参与者`, `经过`, `来源` at line starts (list marker and bold allowed) | `event` | yes | `missing labels: 参与者、经过、来源` |
| E4 | An Idea Card links it with a kind word (`inbound_kinded` > 0) | `event` with `inbound_kinded` | no | `no Idea Card links it with a kind word` |
| E5 | Advisory: no other Event Card with the same year and bigram similarity of 0.5 or more | `event` | no | `(advisory) the same event as [[a]]?` |
| F1 | The labels `测量`, `数据`, `方法`, `结果`, `局限`, `来源` at line starts | `finding` | yes | `missing labels: 方法` |
| F2 | The `来源` line links the book or one of its anchors when the book is known, else is not empty | `finding` with a `来源` line | yes | `the source line links neither [[key]] nor one of its anchors` |
| F3 | One `结果` label, with at most one list item under it (to the next label or blank line) | `finding` | yes | `(2 结果 labels) one result per Card: split it` |
| F4 | As E4, for a Finding Card | `finding` with `inbound_kinded` | no | `no Idea Card links it with a kind word` |
| L1 | Every Link has a Relation kind: not in `kinds`, not a book place (a Citation key or `<key>/<n>`), and not after `引出` in its sentence → `refers` | layer Cards with `links` | yes | `(N Links read as refers) [[a]] [[b]]` (at most 5 shown) |
| L2 | A sentence with a Link (not a `来源`/`出处`/`Source` line) has at least `min-reason` characters of reason, or an inline formula | layer Cards | yes | `(N characters < 8) a Link with no reason: <sentence>` |
| L3 | Advisory: one target linked in two or more paragraphs | layer Cards | no | `(advisory) [[a]] is linked in N paragraphs` |

Rules R1, R8, R10, R11, and E3 exist in the design docs but zt-check does not check them.

### Fault codes and semantic keys (`core::Faults::add` call sites)

Every diagnostic names a rule-specific `fault` and a `key` built from the full offending fragment or missing requirement. The key never contains a source line number or the clipped display, so it is safe for the legacy comparison.

| Rule | `fault` | `key` |
|---|---|---|
| R2 | `sentence_length` | `<measured> <unit> > <limit>: <sentence>` |
| R3 / R4 / R5 / R7 | `semicolon` / `passive` / `light_verb` / `noun_phrase` | the full sentence |
| R6 | `forbidden_word` | `<term>\|<word>\|<sentence>` |
| R9 | `hedge` | `<word>\|<sentence>` |
| R12 | `title_size` / `two_claims` | `<measured> <unit> > <limit>: <title>` / the title |
| R13 | `part_size` | `sentences:<n>/<limit>` or `characters:<n>/<limit>` |
| R14 | `steps` | `steps:<n>/<limit>` |
| R15 | `sections` | the found section order (`思路,步骤`) |
| R16 | `thought_sentences` / `reading_sentences` | `<n> sentences > 5` / `> 6` |
| R17 | `note_before_step` / `second_note` / `note_sentences` / `explanation_size` | the 说明 line; the 说明 line; `<step>:<n> sentences > 2`; `sentences:<n>/<limit>` or `characters:<n>/<limit>` |
| R18 | `repeated_prose` | the repeated paragraph text |
| R19 | `repeated_title` | `heading:<occurrence>` (1-based, so a moved heading keeps its fault) |
| R20 | `local_reference` | `<label>\|<full text>` |
| R21 | `unnamed_referent` | `<word>\|<full text>` |
| R22 | `open_formula` | `title:<full title>` or `body:<full body>` |
| I1 | `no_claim_title` | the title |
| I2 | `idea_size` | `<n> characters > <max>` or `<n> characters < <min>` |
| I3 | `no_source` | the book key |
| I4 / I5 | `no_kinded_link` / `no_inbound` | empty |
| I6 | `similar_title` / `context_incomplete` / `workspace_scope_unknown` | `[[other address]]` / `known:<n>` / empty |
| I7 | `own_source` / `own_source_unknown` | the offending target |
| E1 | `no_date` | the title |
| E2 / F1 | `missing_labels` | `labels:<missing labels, comma-joined in rule order>` |
| E4 / F4 | `no_kinded_inbound` | empty |
| E5 | `same_event` | `[[other address]]\|<year>` |
| F2 | `source_wrong_book` / `source_missing` | the book key / empty |
| F3 | `result_labels` / `result_items` | `<n> labels` / `<n> items` |
| L1 | `unkinded_links` | the `refers` targets, comma-joined |
| L2 | `no_reason` | `<measured> characters < <limit>: <sentence>` |
| L3 | `repeated_paragraph_link` | the target address |

`policy::severity` and `policy::scope` (`ADVISORY_RULES`, `GRAPH_RULES`) give each rule its default severity and scope; the core asks them in `Faults::add`, and only I7's unknown-source subcase overrides the severity itself (`Faults::advise`). A new rule must add its catalog entry and its `Applicability` signature (see "Adding a rule").

### `zt mcp` client (`tools/zt-check/src/mcp.rs`)

zt-check has no MCP server mode of its own. It is an MCP client: `Zt::start()` runs `zt mcp` (the program in `ZT_CHECK_ZT`, default `zt` on PATH) with piped stdin and stdout and stderr discarded, sends `initialize` (`protocolVersion` `2025-06-18`), waits for the reply with id 0, and sends `notifications/initialized`. `Zt::call(tool, arguments)` sends one `tools/call` per line with a rising id, skips reply lines that are not JSON or have another id, joins the `text` parts of `result.content`, and parses them as JSON. A JSON-RPC `error` or `result.isError: true` becomes `zt: <message>` (the `error` field of a JSON message when there is one). `Zt::request(tool, arguments)` returns the whole reply instead, an error reply included; `reply_text` and `reply_error` read it (the zt-digest helper uses them). `call` goes through `request`, and `request` is the only path that sends `tools/call`, so the Literature scope (below) covers every tool call of the client. It mirrors the protocol of `skills/zt/scripts/zt-mcp.ps1`.

A caller that needs long edit patience declares it once for the shared client (`zt_check::mcp::child_edit_patience(180)`), and every child it starts then inherits `ZT_EDIT_PATIENCE`; the value is child-process policy, never a per-command argument (R269). One small I/O worker owns the pipes. Each exchange, including initialize and a blocked stdin write, has a 240-second deadline in the calling thread. Wrong-id/non-JSON noise does not renew that deadline. Input is strict UTF-8 and a reply line cannot exceed 32 MiB; the server's own encoded-tool-result limit remains smaller. This bounds memory and waiting without adding another process/client or changing JSON-RPC tool behavior. A timeout poisons that client, terminates its owned tree and observes exit before returning `TransportFault`. The stable fields are `code` (`command_timeout` or `transport_closed`), operation, child PID, `cleanup_confirmed` and observed cause. A timed-out write may have committed: the domain adapter must recover the accepted operation's receipts, not invent a new identity or assume rollback.

The library also holds the service resolution a digest stage uses before its archive work (R320): `observe_service(program)` runs one bounded `<program> status` and parses `state:`, `pid:`, `archive_root:`, `revision:`, the daemon's `version=` and `last_exit:`, `observed_service()` returns the last successful observation, and `ensure_service(program, wait)` polls it at 500 ms intervals for at most `SERVICE_WAIT` (120 s). `ensure_service` never launches a daemon: `zt status` is the one path that clears a stale record and starts one service, so concurrent stages share a single restart. A service that stays down is one `infrastructure_fault` error that names the wait and leaves the run's units and files as they are. A program that does not answer like `zt status` within the 45-second observation bound (an isolated seam standing in for the MCP server) reports `None`, so a stand-in never becomes a false service state. The crate also exports `VERSION`, so a run report can name the checker that checked its Cards. zt-check's own `Zt::start` is unchanged; [zt-digest](zt-digest.md) calls the resolution for every archive command.

`Zt::shutdown()` first closes the command channel, so the idle worker drops stdin. It allows two seconds for ordinary EOF exit, then force-terminates the owned tree and waits at most two seconds, followed by at most two seconds for the I/O worker. `Drop` uses that path and reports unconfirmed cleanup; it has no unconditional blocking child wait. A timeout skips the graceful phase. Failure to confirm termination remains explicit evidence, not permission for a replacement writer.

On Windows, `owned::Owned` creates the child suspended, assigns it to one Job Object with kill-on-close and resumes its initial thread. Descendants remain in that job; terminating the compiler closes its job handle as well. Termination uses job/process handles rather than executable-name searches. The separately living ZT daemon and explicit Scholar Archive `start` use the narrow service-launch paths in [Service, CLI and storage](service-and-cli.md) and [Scholar Archive bridge](scholar-bridge.md); they leave the client job and inherit no capture handles. Pair/revoke helpers and archive-writing MCP processes stay contained. Unix uses a fresh process group and group termination; Windows parent-death behavior is the host-qualified path, and Unix parent-death equivalence is not claimed.

`mcp-lifecycle.tests.ps1` drives both public binaries and a real archive. It checks silent initialization/receive, unrelated reply noise, a blocked large stdin write, refused EOF shutdown, descendant cleanup, unrelated-process survival, compiler death, before/after actual batch commit timeouts with exact receipt recovery, daemon restart survival before any status repair, and independent Scholar Archive startup without retained capture pipes. Only a canonical temporary `mcp-lifecycle-test-*/config` fixture can shorten deadlines through `ZT_MCP_TEST_TIMEOUT_MS`/`ZT_MCP_TEST_SHUTDOWN_MS` (100–10000 ms); production ignores those variables outside that scope. No provider or actual digest workflow runs.

| Tool called | Arguments | Used by | Fields read |
|---|---|---|---|
| `get_cards` | `{ addresses, view: "text" }` | every live mode (`--model`, `--addresses`, `--cards`; never `--json`) | `cards[].address`, `title`, `body`, `facet`, `type`, `tree`, `links[]`, `kinds`, `archive_revision`, `complete`, `missing` |
| `get_model` | `{ at }` | `--model`, `--addresses` (the first Card when it is a model), `--cards` (each model it checks) | `head`, `parts[]`, `level`, `build.setting`, `build.order[].line`, `build.order[].address` |
| `export_titles` | `{}` | `--cards` | `titles[].address`, `titles[].tree`, `count`, `complete`, `archive_revision` |

### The Literature write scope (`tools/zt-check/src/mcp/scope.rs`)

The scope is one process-wide `AtomicBool`, off at start. The `zt-check` binary never turns it on; [zt-digest](zt-digest.md) turns it on once in `main`, before its first archive call, for every digest step (`literature_scope` in `tools/zt-digest/src/main.rs`; the Topic, hub and register tools and a `call` or `organize-apply` with `-Open` keep it off).

| Function | Signature | Effect |
|---|---|---|
| `set_literature_only` | `(on: bool)` | Puts the process in the Literature scope (`true`) or back in the open scope (`false`). |
| `literature_only` | `() -> bool` | Whether the process is in the Literature scope. |
| `is_topic_address` | `(address: &str) -> bool` | The tree part (before the first `/`, trimmed) is nonempty and all ASCII digits: `408/1\|a`, `12/0` and `7` are Topic addresses; `Smith2024`, `2002grossman/1` and an empty string are not. |
| `topic_links` | `(text: &str) -> Vec<String>` | The Topic-tree targets of the `[[…]]` Links in authored text. Text after `<!-- zt:reverse-links -->` (the generated Reverse links section a Card read ends with) is left out. |
| `literature_refusal` | `(tool: &str, arguments: &Value) -> Option<String>` | Why the scope refuses this call, or `None` when it may go to the archive. |

`literature_refusal` decides per tool:

| Tool | Refused when |
|---|---|
| `create_topic` | always |
| `create_successor`, `edit_card`, `insert_card`, `move_card`, `delete_card`, `edit_literature_metadata` | `at`, `before`, `to` or `into` is a Topic address; or `type` is `hub` (any case); or `title`, `body` or `text` links a Topic-tree Card |
| `create_literature` | `title`, `body` or `text` links a Topic-tree Card |
| `apply_batch` | any item of `items` (or `preview_items`) is refused by the rules above; the reason names the item: `batch item <i> (<tool>): …` |
| any other tool (every read) | never |

Each reason starts `the digest writes only the book's Literature tree` and names what was refused (a Topic tree, a Topic-tree Card, a Hub Card, or the linked Topic-tree Cards). When the scope is on, `Zt::request` checks `literature_refusal` first and fails with `literature_only: <tool>: <reason>` without sending the call, so the archive sees nothing and no id is spent. A Link to another book's Literature tree, a Literature Card's own edits, and every read pass.

### The box pin (`tools/zt-check/src/mcp/boxpin.rs`)

A digest run on a copy of the box (a box with its own zt service, named by its `ZT_CONFIG_DIR`) names that box once: the first line of `zt-box.txt` in its staging folder is the box's `ZT_CONFIG_DIR`. `boxpin::apply(argv)` runs first in the `main` of both `zt-check` and [zt-digest](zt-digest.md). It looks for the pin above every absolute path argument (`D:…`, `D:/…`, `\server…`; a file path's folders, then each parent) and above the working folder. When it finds one and `ZT_CONFIG_DIR` is unset, it sets `ZT_CONFIG_DIR` to the pinned folder (and `ZT_GUI_PORT=0` when that is unset), so the process's `zt mcp` and every child serve that box. A relay's command carries `-Out` or another path in the staging folder, and a DeepSeek job runs in the staging folder, so every command of the run reaches the box without an environment line, and a frozen workflow script needs no change. A pin that names no folder, two pins that name different boxes, and a `ZT_CONFIG_DIR` that names another box are errors (exit 1, before any archive call). Without a pin nothing changes: the live box.

| Function | Signature | Effect |
|---|---|---|
| `find` | `(argv: &[String], cwd: Option<&Path>) -> Result<Option<Pin>>` | the one pin above the absolute path arguments and `cwd`; refuses two different boxes (paths compared without case, separator or trailing slash) and an empty pin |
| `apply` | `(argv: &[String]) -> Result<Option<Pin>>` | `find` with the process's working folder, then sets the environment as above |

Making a box: `$env:ZT_CONFIG_DIR='D:zt-pilot-boxconfig'; $env:ZT_GUI_PORT='<port or 0>'; zt config set archive_root D:zt-pilot-boxarchive; zt up` (copy `scholar-bridge.json` from the live config folder into the box config when the run's Figures stage needs the Scholar Archive), then write `zt-box.txt` into the staging folder before the run starts.

### Install script (`scripts/install-zt-check.ps1`)

```powershell
pwsh -NoProfile -File scripts\install-zt-check.ps1 [-TargetDir D:\zt-check-target] [-BinDir <folder>]
```

It sets `CARGO_TARGET_DIR` to `-TargetDir`, runs `cargo build --release` in `tools/zt-check`, deletes the old `<BinDir>\zt-check.exe` or, when a running check holds it open, renames it to `zt-check.exe.old-<yyyyMMdd-HHmmss>`, copies the new one, and prints `installed <target> (on PATH: <found>)`. `-BinDir` defaults to the folder of the `zt` on PATH (normally `~\.cargo\bin`); the script fails when `zt` is not on PATH. It never stops the ZT service.

## Behaviour

### Exact corruption checks

`check.rs::corruption` runs for every checked body before the sentence rules. R18 compares substantive prose paragraphs after CRLF/lone-CR normalization and outer paragraph trimming only. Interior whitespace, indented paragraph continuation, Markdown and inline formula content remain part of exact equality. Each repeated occurrence reports the first and current body-line ranges; three copies yield two paired faults. Source-only lines, known unwritten placeholders and text without prose letters are not substantive paragraphs. A source label followed by substantive prose cannot hide a repeated paragraph.

R19 reports every authored level-one heading that repeats the Card title, with its body line. Valid closing ATX hash decoration is recognized. Both checks exclude code/chart fences, display formulas (`$$`, `\[...\]` and supported equation/align/gather environments), indented code, Markdown block quotations including lazy continuation, nested HTML blockquotes, whole explicitly quoted paragraphs and generated Reverse content. Heading/section labels and tables do not become repeated prose. These rules report; they do not remove duplicates, compare paraphrases or rewrite uncertain Chinese characters such as 向向/价价. Source review decides those cases.


### A `--json` run (drafts, offline)

1. `run_cli` reads the file; `check_json` strips a BOM and parses an array (or wraps one object).
2. Entries with `line: shared` form the shared list. When `--setting` is given, the entry with that address gives the term table (`read_terms`); a setting that is not in the file gives no table (no `get_card`).
3. Without `before`, `Check::report` runs `language` on each entry in input order, keeping each Card's prose character count, then `layer` on the entries that carry a truthy `category`; `run_cli` appends `faults: <number of lines>` (every line, advisory included) and `write_lines` prints.
4. With a non-null `before` on at least one entry, `before_entries` pairs each old entry with its proposed Card (the old entry keeps its own metadata; a missing `address` is filled from the proposed entry, and an old address that differs from the proposed one is a move, whose diagnostics are paired to the proposed address first). The old entries and all proposed entries are checked by the same `Check`; `policy::ratchet` marks the inherited occurrences `legacy`, and `faults: N` is the number of remaining blockers. Because both reports use the proposed bundle's limits, a rule-version change (a tighter limit, a new fault code) applies to both sides: an unchanged old Card stays advice, while a new Card must meet the current requirement. `run_cli` prints the proposed report's lines, so a retained fault shows `(advisory)` and an unmatched one keeps its blocking text.

### A `--model` run

1. `Zt::start` launches `zt mcp`. `check_model` calls `get_model { at }`; `head.address` must exist. It first probes the revision with one empty `get_cards` text read.
2. The entries are the `get_model` head and every `parts[]` item, reduced to `address`, `title`, `body`, `facet` (`card_entry`).
3. The setting and level come from the flags, else from `build.setting` and `level`. Shared parts are the `build.order` items with `line: shared`; `--skip-shared` removes them.
4. `setting_body` takes the setting's body from the entries, else from one required lean `get_cards` read for that external setting. `Check::run` runs; no entry has a category, so only R rules apply.

### A `--addresses` run

1. One bounded text `get_cards` read for every address. When the first Card is a Model Card, one `get_model` read gives its default setting and level; otherwise the flags (else empty) apply.
2. As `--model` from step 4, with no shared list (R6 applies to every Card).

### A `--cards` run (self-check, as the audit sees it)

1. The addresses of `--cards` and of the `--log` file (one per line, BOM stripped) are de-duplicated (case counts, ignorable characters do not). An empty list prints `no Cards to check`.
2. `check_cards` makes one bounded text `get_cards` read of those Cards, one complete `export_titles` catalogue for classification, and one `get_model` read per distinct model it must check. Each lean row carries its indexed `links` and `kinds`, so no per-Card or per-tree read remains.
3. Each Card gets a category: `root` (the tree entry's kind is `topic` or `literature`), `hub`, `event`, `finding`, `part` (type `model` with a facet), `model`, then `register-part` when both Workspaces' `get_register` reports declare its address under `词条分册` (`declared_register_parts`, one `entry_limit: 1` read per Workspace), then `restatement-entry` when its authored body holds a `## 同述入口` section (or its `export_titles` row says `restatement`), then in an agent Literature tree `anchor` (`<key>/<n>`), `terms` (title starts with `术语表`), `structure` (body starts with `本卡是《…》的结构卡`), else `idea`. A `register-part` is navigation, like `hub` or `root`: it takes no claim-title or Idea-size rule and no topic-hub requirement of its own, and its term rows are checked by the register's own entry projection (`get_register` faults) rather than as prose. A `restatement-entry` is navigation with its own checks: it keeps the source-line rule (I3) and the repeat rule (L3), while the `## 同述入口` section's own Link is navigation and takes no relation-kind (L1) or Link-reason (L2) rule — a Link outside that section is checked as usual; it takes no I1, I2, I4 or I7 rule, and neither it nor its title is a duplicate-title candidate (I6). The audit derives the same categories from the same declarations and derivations.
4. `part` and `model` Cards with a model head are grouped by head; each group runs as one `Check` with its model's setting, level, and `require_explain = true`.
5. The rest run together; `idea`, `anchor`, `event`, `finding` entries get `category`, `parent`, `links`, `kinds`, and `book` (`--book`, else the tree when it is a Literature tree).
6. `filter_rules` keeps the lines whose rule is in `--rules` (default `DEFAULT_RULES`) and counts the non-advisory ones; `faults: N` is that count.

### How text is read

1. `prose_lines` drops fenced code (including chart fences), `$$` blocks, single-line `$$…$$`, headings, and lines that start with `|`, `>`, `<`, or `![`; it strips a list marker and marks numbered lines as steps.
2. `split_sentences` hides formulas and Links, splits after `。！？!?` and after a `.` followed by white space, then restores them.
3. `plain` replaces formulas with `F`, code with `C`, removes Links and bold marks. `measure` counts characters (after removing white space, quotes, brackets, and every `F`/`C`, any case) when the sentence holds a CJK character, else words.
4. In a Derivation part, a line `思路`/`起点`/`步骤`/`结果`/`解读` (bold allowed) opens a section; the position line `第 N 张，共 M 张` in 思路 is skipped; `说明：` lines under 步骤 are explanation; a `来源` line after 解读 is not.

### Rules the code keeps

- Fault order: per Card in input order: structural R18/R19 in body scan order, R12, then per sentence R2, R3, R4, R5, R6, R7, R9 (R17 position faults inline), then R15, R16, R17, then R13, R14. Layer faults follow all language faults: per Card I1, I2, I3, I4, I5, E1, E2, E4, F1–F4, L1, L2, L3; then I6 in checked-Card order (candidates in catalogue order), then E5. In `--cards` mode the model groups come before the other Cards. Callers and tests compare lines in this order.
- R13 and R14 apply only when the level is not `concept`; R15 to R17 likewise. R13 applies only to Cards with a facet, so a Model Card is never size-checked.
- One `Zt` process per run; calls are sequential. zt-check never writes: it calls only read tools.
- The Literature scope refuses before sending. A refused call never reaches `zt mcp`, takes no request id, and leaves the client usable for the next call; the scope never refuses a read, so a checker run inside a Literature-scope process (a zt-digest SELF-CHECK) reads Topic trees as before.
- Every mode except `--cards` counts advisory lines in `faults: N`.

## Data and storage

zt-check keeps no state. It exchanges two formats.

The `--json` entry (fields read with case-insensitive names; all but `address` optional):

```json
[
  { "address": "S/1|a|a", "facet": "设定", "title": "设定", "body": "…| 术语 | 不用 |\n…", "line": "head" },
  { "address": "S/1|a|a|1", "facet": "推导：参与约束", "title": "推导：参与约束", "body": "**思路**\n…", "line": "shared" },
  { "address": "5/1", "title": "税", "body": "依赖 [[5/3]] 的结论，税负落在买方。",
    "category": "idea", "parent": "", "links": ["5/3"], "kinds": ["5/3"],
    "relations": { "5/3": "rests-on" }, "book": "Hart1995", "tree": "5", "workspace": "agent",
    "inbound": ["5/2"], "inbound_kinded": 1 }
]
```

I6's Workspace context is a separate read-only catalogue, either in the bundle around the
entries or from the mode's own bulk reader:

```json
{ "cards": [ … ], "context": { "complete": true,
  "titles": [ { "address": "5/1", "title": "税", "workspace": "agent" } ] } }
```

An edit bundle adds `before` to each edited entry (the fixture `tools/zt-check/tests/fixtures/legacy-bundle.json` is one):

```json
{ "cards": [
  { "address": "K/10|c", "category": "event", "title": "通用汽车收购费雪车身公司",
    "body": "通用汽车收购了费雪车身公司。",
    "before": { "category": "idea", "title": "通用汽车收购费雪车身公司",
                "body": "通用汽车收购了费雪车身公司。" } }
] }
```

| Field | Meaning | Used by |
|---|---|---|
| `address`, `title`, `body` | The Card | all rules |
| `facet` | Makes the entry a Model part; `推导…` makes it a Derivation part | R13–R17 |
| `level` | `concept` or `formal` for this entry, overriding the run's level; a `before` entry carries its old level so the old version is checked as it was | R13–R17 and `Applicability` |
| `line` | `shared` puts the entry in the shared list | R6 |
| `category` | `idea`, `anchor`, `event`, `finding`, `restatement-entry`; anything else (or none) skips the layer rules (`register-part` is navigation: it takes none of the substantive claim or size rules; `restatement-entry` keeps only the source and Link checks) | I, E, F, L |
| `parent` | The parent's address | I4, I5 |
| `links` | Link targets out | I3, L1 |
| `kinds` | The targets of kinded Links | I4, L1 |
| `relations` | The kind word of every kinded Link (`{ "<target>": "rests-on" }`, as `get_card` returns it). Without it I7 sees no dependency | I7 |
| `sources` | The Card's explicit source evidence: the Citation keys of its Literature trees. Absent, I7 derives them from the `来源` line, else from the Card's own Literature tree | I7 |
| `book` | The Citation key | I3, F2 |
| `tree` | The tree id | I7 |
| `workspace` | `agent` or `human`; I6 compares only inside one Workspace | I6 |
| `inbound` | Addresses that link here | I5 |
| `inbound_kinded` | How many Idea Cards link here with a kind word | E4, F4 |
| `before` | The old authored version of this Card, as another entry object; absent or `null` means a new Card with no legacy allowance | the ratchet (`check_json`) |

The term table in a setting body (R6 needs the first two columns; the digest's concept review reads all four):

```markdown
术语表

| 术语 | 不用 | 意思 | 首次定义 |
|---|---|---|---|
| 清算 | 卖资产、变卖资产 | 把资产换成现金 | §3.1 |
| 可承诺的还款 | 还款额 | 契约能保证的偿还金额 | §4.2 |
```

The first column is the term; the second lists the words not to use, split on `、`, `,`, `，`, `/`; `无`, `-`, `—` mean none. The table ends at the first non-table line after a row.

The wire format to `zt mcp` is one JSON-RPC 2.0 message per line (see "Interfaces").

## Configuration

| Setting | Where | Default | Effect |
|---|---|---|---|
| `--max-sentences` | flag | 12 | R13 sentence limit |
| `--max-chars` | flag | 400 | R13 character limit |
| `--max-steps` | flag | 6 | R14 step limit |
| `--max-explain-sentences` | flag | 16 | R17 explanation sentence budget |
| `--max-explain-chars` | flag | 600 | R17 explanation character budget |
| `--min-idea-chars` | flag | 40 | I2 merge candidate below |
| `--max-idea-chars` | flag | 300 | I2 split candidate above |
| `--min-reason` | flag | 8 | L2 reason length |
| `--rules` | flag (`--cards`) | `R2,R3,R4,R5,R7,R9,R12,R13,R14,R15,R16,R17,I1,I2,I3,I4,L1,L2,E1,E2,F1,F2,F3` | Which rules print and count |
| `ZT_CHECK_ZT` | environment | `zt` | The program started as `<program> mcp` |
| `-TargetDir` | `install-zt-check.ps1` | `D:\zt-check-target` | Cargo target folder (E: has little free space) |
| `-BinDir` | `install-zt-check.ps1` | folder of `zt` on PATH | Where `zt-check.exe` goes |
| `[profile.release]` | `tools/zt-check/Cargo.toml` | `opt-level = 3`, `codegen-units = 16` | Release build |

The fixed limits R2 (40/30 characters, 25/20 words), R12 (40/25), R16 (5 and 6), R17 (2 per 说明), I1 (6 characters), I6 (0.8), and E5 (0.5) are constants in `tools/zt-check/src/core.rs` and `tools/zt-check/src/text.rs`.

## Usage

The digest scripts call it through `Invoke-ZtCheck` (`scripts/book-digest/zt-calls.ps1`), which maps PowerShell parameters to flags, always passes all eight limits, merges stderr into the output, and throws when no `faults: N` line appears:

```powershell
. scripts\book-digest\zt-calls.ps1
Invoke-ZtCheck -Json $file -Setting 'K/1|a|a'                         # drafts (card-compiler, hub-compiler, model-compiler explain-check)
Invoke-ZtCheck -Model $model -SkipShared -MaxSentences 12 -MaxChars 400 # model-compiler report step
Invoke-ZtCheck -Addresses "$a,$b" -RequireExplain                      # model-compiler tests
```

DeepSeek writers run it themselves, from the SELF-CHECK line of their job file (`scripts/book-digest/prepare-book-digest.ps1`, `scripts/book-digest/prepare-link-pass.ps1`, `scripts/book-digest/dsh-fill-instructions.zh.md`):

```powershell
zt-check --cards "Smith2024/3" --log "D:\out\owned-3.txt" --book Smith2024
zt-check --log "D:\out\allow-2.txt" --cards "" --book Smith2024 --rules L1,L2,R2,R3,R4,R5,R7,R9
zt-check --addresses "3/1|a|a|1,3/1|a|a|2" --max-sentences 12 --max-chars 400 --max-steps 6 --require-explain
```

The compilers check drafts by writing `--json` entries with a `category` so the layer rules apply (`Get-DraftFaults` in `scripts/book-digest/event-compiler.ps1`, `New-CheckEntry` in `scripts/book-digest/card-compiler.ps1`); the hub compiler sends plain entries (no category) and keeps only R2, R3, R4, R5, R7, R9 (`scripts/book-digest/hub-compiler.ps1`).

### Adding a rule

1. Add the check in `Check::sentence` (per sentence), `Check::language` (per Card), `Check::layer` or `finding_rules` (layer, by category) in `tools/zt-check/src/core.rs`, and report it with `faults.add(address, "<Rule>", "<fault code>", "<semantic key>", text)`. The message carries no `(advisory)` prefix: the line adds it from the severity. An advisory subcase of an otherwise blocking rule calls `faults.advise(...)`. A text helper belongs in `tools/zt-check/src/text.rs`.
2. Choose the `fault` code and the `key` deliberately (see "Fault codes and semantic keys"): the key must identify the requirement, not the printed line, so an unchanged fault keeps its identity while an added occurrence does not. Add the rule's signature to `policy::Applicability::signature` when its applicability depends on type, role, level, term table, or book; an unnamed rule falls back to metadata equality.
3. Give the rule its default `severity` and `scope` in `tools/zt-check/src/policy.rs` (`ADVISORY_RULES`, `GRAPH_RULES`, `severity`, `scope`). A `graph` rule is audit evidence and is not decided per write (design 6.2).
4. Keep the rule id matching `FAULT_LINE` (`[RIELF]\d+`) in `tools/zt-check/src/policy.rs`; a new letter needs that pattern changed, or `--cards` drops its lines.
5. Add it to `DEFAULT_RULES` in `tools/zt-check/src/policy.rs` if `--cards` should report it by default.
6. Add a unit test in the module's `mod tests` with the exact lines and the diagnostic's `fault`/`key`, then check that `tools/zt-check/tests/fixtures/card-style-expected.txt` (and `card-layer-expected.txt` if it changes) matches the new order, and update the PowerShell fixture tests if the `--json` output of their fixtures changes. A rule that can block an edit needs a `--json` `before` test for the retained and the newly introduced case.
7. Run `cargo test` and `cargo test --no-default-features --lib`, then `cargo build --no-default-features --lib` (the core must stay process-free), then `pwsh -NoProfile -File scripts\install-zt-check.ps1`, then the PowerShell tests. Rebuild and redeploy `zt` too: a shared rule change changes the service's in-process check. Tell callers that filter rule letters (see pitfalls).

### Adding an input field

Read it with `prop(card, "<name>")` and the PowerShell conversions (`text`, `strings`, `ps_int`, `truthy`). For `--cards`, also set it in `check_cards` (the item built for layer Cards), or the rule never fires there.

## Testing

| Test | Command | Covers |
|---|---|---|
| Unit tests, `mod tests` in `tools/zt-check/src/core.rs` (22 tests; they run with `--no-default-features --lib` too) | `cd tools/zt-check; cargo test` and `cargo test --no-default-features --lib` | The recorded ordered lines of `card-style-fixture.json` through `core::Check` (one fault per rule, with the shared part's term table skipped), R22's open formula, card size, R15–R17, the position line, the I/E/L rules, the F rules, the standalone rules R20/R21 (content-bearing label, name, source locator, code/quote/formula exclusion, a book-local model number in a title, a Literature root's bibliographic body, same-sentence and preceding-sentence referents), I7 (own source, other book, `illustrates`/`corresponds`/`contrasts`, own Literature tree, unknown evidence, `来源` line, several named sources), I6 (Workspace context, one line per pair, an incomplete context marked unknown, an unscoped entry marked unknown, and an exact copy that differs only in case or in a trailing mark found without a scan of the context), UTF-16 lengths and clipping, the Bridge label, the structured diagnostic (`address`/`rule`/`fault`/`key`/`message`/`severity`/`legacy`/`scope` and the unchanged line), its published JSON projection matching design 5.4's E2 example field for field and its legacy form as `advise`, two long faults that clip to the same prefix keeping distinct full-fragment keys, and the measured value/limit and missing-label keys of a size or label fault |
| Unit tests, `mod tests` in `tools/zt-check/src/text.rs` (5 tests) | the same `cargo test` | The four-column term table against R6's own view, a book title's end marks inside one sentence, sentence splitting that keeps formulas whole, culture order and starts-with, and title identity and similarity |
| Unit tests, `mod tests` in `tools/zt-check/src/policy.rs` (8 tests) | the same `cargo test` | The rule filter; a retained fault becoming legacy advice and a new Card getting no allowance; occurrence multiplicity (a second copy is new, a removed copy is not); semantic keys over printed lines (a moved paragraph, a same-prefix fragment); `Applicability` signatures for category, role, concept/formal, term-table, book and term-name changes, and an unnamed rule's conservative signature; `CheckReport` blockers and lines; a changed fault code not being inherited |
| Unit tests, `mod tests` in `tools/zt-check/src/mcp/scope.rs` (4 tests) | the same `cargo test` | Topic addresses are numbered trees; authored Topic Links are found outside the Reverse links section; the scope refuses `create_topic`, a Topic-tree `at`/`to`/`into`/`before`, a Hub Card, a Topic Link in `body` or `text`, and the offending `apply_batch` item by index; Literature writes, Literature Links, `create_literature` without Topic Links, reads and a clean batch pass |
| Integration test, `tools/zt-check/tests/check_parity.rs` (6 tests) | the same `cargo test` | The executable, `check::run_lines`, and the process-free `core::Check` print the same ordered lines for `card-style-fixture.json` and `card-layer-fixture.json`, all equal to the recorded `tests/fixtures/*-expected.txt`; the executable and the library print the same ratcheted `before` bundle (`legacy-bundle.json`, whose moved Card keeps its fault across the address change); the same for `legacy-rule-version-bundle.json` under the tightened `--max-sentences 8` (the old part stays advice, the new part blocks); the binary states `zt_check::VERSION`; the not-JSON envelope error is the same in the binary and the library |
| CLI adapter tests, `mod tests` in `tools/zt-check/src/check.rs` (24 tests) | the same `cargo test` | The recorded fixture lines through `check_json` and `run_lines`; a single object, a BOM'd array, and case-folded property names; a bundle's title context; the setting lookup and a shared part; the not-JSON and unreadable-file messages; two inputs refused; an empty Card list; the `before` bundle (retained fault advice, a second copy blocking, a new Card with no allowance and an explicit `before: null`, Idea→Event and concept→formal changes, a regular↔part role change and a facet change, a moved paragraph, a changed long fragment sharing the clipped prefix, a same-sentence duplicate on two Cards not consuming another Card's occurrence, a larger size fault, a wider missing-label set, a non-object `before` refused, a Card's own address change keeping its unchanged fault as advice, and no saved baseline across repeated runs) |
| Core-only build | `cd tools/zt-check; cargo build --no-default-features --lib; cargo tree --no-default-features -e normal` | The pure core builds alone and its dependency tree names no `windows-sys`, no `libc`, and no archive client |
| Root service link | `cargo test --test check_core` (root) | The zt binary links `zt-check` with `default-features = false` and checks a Card in-process through `zt_check::core::Check`; `the_service_can_ratchet_an_edit_against_the_old_version_in_process` runs the same `policy::ratchet` the S04 write gate will call, keeps a retained R3 as advice, and blocks the newly applicable E1/E2 |
| `scripts/book-digest/tests/zt-helper-gate.tests.ps1` | `pwsh -NoProfile -File scripts\book-digest\tests\zt-helper-gate.tests.ps1` | The scope end to end through `zt-digest call` on a scratch archive: without `-Open` the helper refuses `create_topic`, a Card in a Topic tree and a Literature Card that links a Topic root, and still writes a Literature tree |
| `scripts/book-digest/tests/zt-check.tests.ps1` | `pwsh -NoProfile -File scripts\book-digest\tests\zt-check.tests.ps1` | The same fixture, card size, and R15–R17 through `Invoke-ZtCheck` |
| `scripts/book-digest/tests/card-layer.tests.ps1` | `pwsh -NoProfile -File scripts\book-digest\tests\card-layer.tests.ps1` | I, E, L rules on `card-layer-fixture.json`, plus formulas and table pipes |
| Compiler tests on scratch archives | `pwsh -NoProfile -File scripts\book-digest\tests\finding-compiler.tests.ps1 [-Zt <zt.exe>] [-ZtCheck <zt-check.exe>]` (also `event-compiler`, `model-compiler`, `hub-compiler` tests) | `--addresses` and `--json` against a live scratch `zt mcp` |

The PowerShell tests run the `zt-check` found on PATH (`$script:ZtCheck` in `zt-calls.ps1`), not the source: install first. `finding-compiler.tests.ps1` copies the debug builds from `D:\zt-target` and `D:\zt-check-target` into a scratch `bin` at the front of PATH and sets `ZT_CONFIG_DIR`, so zt-check's `zt mcp` reaches the scratch archive.

Gaps: no unit test of `mcp.rs` itself (the scope's refusal inside `Zt::request` is covered only end to end by `zt-helper-gate.tests.ps1`); no unit test of `--model`, `--addresses`, or `--cards` (they need a live `zt mcp`; the compiler tests exercise `--addresses` and `scripts/book-digest/tests/workspace-titles.tests.ps1` exercises `--cards` with its Workspace context); no test of `Options::parse` errors beyond the two-input refusal; I5, I6, E4, F4 are tested only through `--json` fields apart from that I6 test, and `--addresses` sets no layer category, so it checks no layer rule at all (including I6 and I7).

## Decisions and pitfalls

### Decisions

- **Stable fault identity, printed lines as the interface.** A fault's identity for the legacy comparison is its unclipped semantic key, never the clipped line or a source line number (design 3.1 and 5.4). The familiar line stays the human interface, so the digest's readers and fixtures see no change.
- **One checker, the only source of truth.** zt-check replaced `card-style-check.ps1` and `self-check.ps1` (deleted 2026-10-15) with the same faults, in the same order, in the same words, so every agent and script gets one fault format (`tools/zt-check/src/check.rs` header; [docs/digest-card-layers-proposal.md](../digest-card-layers-proposal.md) D4 and its note; `skills/zt-digest-protocol/SKILL.md`).
- **PowerShell semantics in Rust.** To keep the old output exactly, lengths count UTF-16 units, matches ignore case, string comparisons drop culture-ignorable characters, hashtable keys ignore case, and sorts use an approximation of ICU collation (`weight`, `culture_cmp`).
- **A crate of its own, read through `zt mcp`, with one process-free core.** It needs none of zt's code and sees the same answers as every other zt client; building or installing it never touches the running service (`tools/zt-check/src/mcp.rs`, `tools/zt-check/Cargo.toml`). The rules themselves live in `core`/`text`/`policy`, which the zt service links with `default-features = false` and calls in-process (#143), so one implementation serves the tool, zt-digest, and the service; a rule change therefore needs both binaries redeployed. Build into `D:` because E: is nearly full (`scripts/install-zt-check.ps1`).
- **The Literature scope lives in the shared client.** A digest builds only the book's Literature tree (owner decision, `docs/literature-only-digest-design.md`). Every zt-digest step and the helper reach the archive through `mcp::Zt`, so one check in `Zt::request` covers them all, before the archive, with no second process and no archive read: a Topic tree is recognised by its all-digit id.
- **R6 is not a default rule.** A book term table may name different concepts as synonyms, so a fix by it changes meaning; the audit reports R6, and the card cycle fixes it only on request ([docs/digest-card-layers-proposal.md](../digest-card-layers-proposal.md) D13).
- **Concept models keep their parts whole.** R13 to R17 apply only to formal models and models with no level; a concept model has no derivation to split ([docs/self-sufficient-models-proposal.md](../self-sufficient-models-proposal.md) "Card language", [docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md) section 4, [docs/model-elaboration-levels-design.md](../model-elaboration-levels-design.md)).
- **Older Derivation parts are silent.** A part with neither 思路 nor 解读 raises no R15 unless `--require-explain`; writers set it, the build report and the audit do not ([docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md)). `--cards` always sets it for model groups.
- **Finding Cards:** I3 does not apply (F2 is stricter), I1, I2, I4, I6 do not apply, F4 is not default like E4 ([docs/finding-cards-design.md](../finding-cards-design.md)).
- Related design docs: [docs/self-sufficient-models-proposal.md](../self-sufficient-models-proposal.md) (the Card language table), [docs/model-compiler-design.md](../model-compiler-design.md) (writers check their own Cards; the report step runs `--model --skip-shared`), [docs/zt-digest-framework.md](../zt-digest-framework.md) (drafts need `category`).

### Pitfalls

- **I6 reads a Workspace title context, and only advice comes of it.** `--cards` builds it from one complete `export_titles` read (the same read that classifies the Cards) and refuses a partial or changing catalogue; `--json` accepts it as the bundle's `context` object. Rows are comparisons only: I6 reports on the checked Card, never on a context Card, and never edits, merges, or deletes. A pair is reported once, on the later checked Card. A supplied context that is not complete marks I6 unknown with one `(advisory)` line per checked Idea Card before its pair lines, naming how many known titles were compared: the reader could not supply the whole Workspace, so the run says so instead of claiming a Workspace-wide check. A supplied context with an entry that names no `workspace` is unknown the other way round: the pair cannot be kept inside one Workspace, so the Card is not compared at all. Without a context the comparison is limited to the entries the run holds, which is why a digest's scoped review passes the catalogue through its own reader.
- **The candidate index folds case, the Dice function does not.** `bigrams` keeps case, so two spellings of one title that differ only in case have no common bigram; the index keys are lower-cased instead, which makes the exact copy a candidate while the decision stays the existing function (an exact normalized equality or Dice 0.8). A title of one character has no bigram at all and is compared with the other short titles.
- **`anchor-title` is a hub fault, not similarity.** `src/hub.rs` normalizes whitespace and case only and refuses an exact match with an anchor's title; a differently worded navigation title is no fault, and an existing hub is reported read-only until its correction is authorized. A topic hub has two anchors: the Card it hangs under (`<n>/1` for one the compiler creates) and its tree's Topic Card `<n>/0`; a line hub has the Card where the line starts. `zt-digest hub -Step precheck` refuses a new draft that copies either.
- **`faults: N` means different things.** In `--cards` it counts hard lines of the selected rules; in `--json` without `before`, `--model`, `--addresses` it counts every line, advisory included; in a `--json` bundle with `before` it counts the blockers the ratchet leaves, so a retained fault is not counted. A `--json` run on layer entries can report `faults: N > 0` with only advisory lines. The coordinated switch of every mode belongs to S06 (#148).
- **The ratchet is only as faithful as the old entry.** A `before` entry that omits its old `category`, `level`, `facet`, `line`, or `book` declares different metadata, so its rules are not comparable and cannot be inherited; a `before` without `title`/`body` finds no old fault. A bundle that carries only a subset of the Cards also evaluates pairwise rules (I6, E5) on that subset, on both sides. The bundle's `--setting` term table is one table for both sides, so editing the term-table Card itself is not yet modelled as two tables; the Term slices add that. The service gate will read the old version from the archive, not trust a bundle.
- **A duplicate-title envelope refusal is not a diagnostic.** The repeated leading title and the multiple Reverse markers are storage envelope invariants in the service (`validate_card_text`, `parse_card_text`), so the ratchet cannot inherit them; `src/main.rs` keeps the `authored_changed` allowance and the test `the_leading_title_allowance_follows_the_authored_change` pins it.
- **`--cards` never fires I5, E4, F4.** `check_cards` sets no `inbound` or `inbound_kinded`, so those rules need `--json` entries that carry them (as `digest-audit.ps1` writes). They are also outside `DEFAULT_RULES`.
- **I7 needs the relationship evidence.** `get_card` returns `kinds` as a `{target: kind}` object; `check_cards` passes it as `relations` and `digest-audit` writes it into every entry. An entry without `relations` reports nothing for I7 — it is not a silent pass, because the published-Card path always carries it, and the audit and card cycle both use that path.
- **R20 and R21 are advice.** They start in the pilot as `(advisory)` lines inside `DEFAULT_RULES` and `DEFAULT_CARD_RULES`: visible in every check, counted as no fault by `--cards`, and split out by the audit (`style.advisory`, `style.advice_by_rule`). Promoting an unambiguous case to a blocking fault is a later frozen change with its own fixtures.
- **`--cards` knows books only in the agent Workspace** (`list_trees { workspace: "agent" }`); a Card in a human Literature tree is categorised as if it were in a Topic tree.
- **`--cards` checks shared parts under the model in the group** (no shared list); harmless by default because R6 is not a default rule.
- **`--addresses` takes the setting and level from the first Card's model** for all Cards; mixing models gives the wrong term table and level.
- **`PLACEHOLDERS` is `(?i)F|C`:** it removes every `F`, `f`, `C`, `c`, not only the formula and code placeholders, before counting Chinese sentence size, I1 titles, L2 reasons, and bigrams. This mirrors the PowerShell `-replace`; changing it changes counts.
- **English prose counts zero characters.** `measure` gives words for a sentence without CJK, and `language` adds only character sizes, so R13's character limit and I2 see English Cards as empty (I2 then reports a merge candidate).
- **One 240-second exchange deadline.** `Zt::receive` waits for a line with the id inside the configured deadline (240 s in production, shorter only in an isolated lifecycle fixture) and reports `command_timeout` with the owned child and cleanup fact; `zt mcp` stderr is discarded, so ordinary server failures show only as the deadline or `zt mcp closed`. The ZT service must be up for tool calls.
- **The scope reads only some fields.** `literature_refusal` reads the address fields `at`, `before`, `to`, `into`, the type `type`, and Links in `title`, `body` and `text`. A Link in any other argument (a `bibtex` field, a hub `entries` list) is not checked, and a tool it does not name passes. A Citation key made only of digits would read as a Topic tree.
- **A refusal is an error, not a reply.** `Zt::request` returns `Err` (`literature_only: …`), not an error reply, so `reply_error` never sees it; a caller that loops over calls stops at the first refused one unless it handles the `Err` itself (the zt-digest helper does not: its run ends with exit 1).
- **`--help` exits 1** (it is reported as an error) and `--log` with a missing file is ignored silently.
- **Rule-letter filters in callers.** `card-compiler.ps1` (`Invoke-StyleCheck`) and `digest-audit.ps1` (`Invoke-Check`) keep only lines matching `[RIEL]\d+`, so F lines are dropped there; `model-compiler.ps1` explain-check keeps only `R\d+`. A new rule letter must be added to those patterns too.
- **Install before testing.** `Invoke-ZtCheck` resolves `zt-check` from PATH once, when `zt-calls.ps1` is dot-sourced; a changed PATH after that has no effect.
- **A locked `zt-check.exe`** is renamed aside, not deleted; old `zt-check.exe.old-*` files collect in the bin folder.
- **Stale descriptions:** the `description` in `tools/zt-check/Cargo.toml` and the comment in `scripts/book-digest/zt-calls.ps1` name the layer rules I, E, L without F, and the doc comment in `tools/zt-check/src/main.rs` omits `--addresses`.
- **R16 does not report an empty 思路 or 解读**, although [docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md) lists "an empty section" as an R16 fault; only too many sentences are reported.

## Change history
- `2026-10-24` — #144 S02 verification: the published contract is serialized by `Diagnostic::to_json` and pinned field for field to design 5.4's E2 example, including the legacy `advise` form; an unnamed rule's conservative `Applicability` now also covers the shared list, book and term name; role-change (regular↔part, facet change), moved-paragraph, clipped-prefix, cross-Card multiset and explicit `before: null` cases are pinned; `legacy-bundle.json` moves one Card's address; `legacy-rule-version-bundle.json` runs the tightened `--max-sentences 8` over both sides; `tests/check_core.rs` runs the ratchet in-process as the S04 adapter shape. Tests: zt-check 68 lib + 6 parity, 35 core-only. zt-check 0.3.1, zt 0.26.5.
- `2026-10-24` — #144 S02: structured diagnostics and the legacy ratchet. `core::Diagnostic` (address, rule, `fault`, unclipped `key`, full `message`, `severity`, `legacy`, `scope`) is the new fault contract; `Check::report` returns diagnostics, and `Check::run`/`Diagnostic::line` format the same bytes as before (advisory lines get their `(advisory)` prefix from the severity). Every rule site names a rule-specific fault code and a semantic key built from the full fragment, measured value and limit, or missing-label set. `policy::severity`/`scope` and the `ADVISORY_RULES`/`GRAPH_RULES` catalog give defaults (I5/I6/E4/E5/F4/L3 are `graph`). `policy::Applicability` compares a per-rule signature (category, level, facet/derivation, shared term table, book) so a retype or concept→formal change cannot inherit newly applicable rules. `policy::ratchet` consumes the paired old report's `(rule, fault, key)` multiset and turns a retained fault into legacy advice; `policy::CheckReport` carries the diagnostics and the remaining blockers. `--json` entries may carry `before` (the old authored version with its old metadata), and an entry may name its own `level`; `check_json` then checks both sides with one `Check`, ratchets, and counts only blockers, while a bundle without `before` keeps the old lines and count. No baseline is stored. Verification: `cargo test` 62 lib + 5 parity tests, `cargo test --no-default-features --lib` 33 tests, both recorded fixture outputs byte-identical, the new `legacy-bundle.json` fixture identical through the executable and the library, and the service-side envelope allowance test `the_leading_title_allowance_follows_the_authored_change`.
- `2026-10-24` — #143 S01: run the checker as a process-free library. `check.rs` is split, without a rule change, into `core.rs` (the rules, `Faults`, `Limits`, the corruption scan, I7's source evidence, and the orphan predicate), `text.rs` (the PowerShell text semantics, the prose projection, the term-table parse, the title context, and the lexical Link and envelope operations), `policy.rs` (`DEFAULT_RULES`, `filter_rules`), and the CLI adapter `check.rs`. `Cargo.toml` gains `default = ["cli"]`, `cli = ["dep:windows-sys", "dep:libc"]`, and `required-features = ["cli"]` on the binary; `lib.rs` exports `core`/`text`/`policy` always and `check`/`graph`/`mcp` with `cli`; `check` re-exports the old public names; `graph` re-exports the core's orphan predicate. The root crate links `zt-check` with `default-features = false` (no cycle; `tests/check_core.rs` checks a Card in-process), and zt-digest keeps the default `cli`. Verification: `cargo test` 43 lib + 4 parity tests, `cargo test --no-default-features --lib` 24 tests, `cargo build --no-default-features --lib` and `cargo tree --no-default-features -e normal` naming no process or archive dependency, both PowerShell fixture suites, and both recorded `tests/fixtures/*-expected.txt` lists byte-identical to the pre-extraction binary.
- `2026-10-23` — The box pin: new `tools/zt-check/src/mcp/boxpin.rs` (`find`, `apply`, `PIN = "zt-box.txt"`); `main` applies it before the check. A run on a copy of the box names the box in its staging folder and every zt-check and zt-digest process of the run serves that box. Tests: four `boxpin` unit tests (a pin found from a nested file argument, from the working folder, none, two boxes and an empty pin refused); `cargo test` 32 passed.
- `2026-10-23` — R6 masks every term the table keeps before it matches a rejected word, each as one mark so no word forms across the gap (`Check::kept`, test `a_rejected_word_inside_another_kept_term_is_that_term`). On ebrill2001modern 66 of 100 R6 faults read `税率差` (rejected for `幅度`) inside `税率差异化`, a term of its own; the old check removed only the row's own term.
- `2026-10-22` — The Literature write scope (`docs/literature-only-digest-design.md`): new `tools/zt-check/src/mcp/scope.rs` (`set_literature_only`, `literature_only`, `is_topic_address`, `topic_links`, `literature_refusal`, with four unit tests), and `Zt::request` refuses a refused call with `literature_only: <tool>: <reason>` before it reaches the archive. In the scope, `create_topic`, a write that names a Topic-tree Card, a Hub Card, authored text that links a Topic-tree Card, and an `apply_batch` holding any of them are refused; reads pass. zt-digest turns it on for every digest step; the `zt-check` binary keeps the open scope.
- `2026-10-20` — The list lines of a Model Card's `**结构**` section (`structure_lines`: each a facet, a Link and the part's title, written by the model compiler) are navigation that no prose rule reads; each title is checked as its part's own title. On grossman2002integration trials 10 and 11 a long Derivation facet made a 结构 line an R2 fault in a done Model Card (`7|a`) that the bounded correction may not reopen. Test `the_structure_list_of_a_model_card_is_not_prose`.
- `2026-10-20` — R2 and R12 count a book title in 《…》 as one name (`BOOK_TITLE` in `measure`), and the Literature Card's document line (`原书文档：[原书 PDF](scholar:78) · …`, `DOCUMENT_LINE`) is a locator that no prose rule reads. On grossman2002integration trial 10 the Literature Card's index line and document line and the structure Card's marker sentence `本卡是《Integration versus Outsourcing in Industry Equilibrium》的结构卡：…` were three residual R2 faults the bounded correction may not reopen (a root and a structure Card). The term table therefore names even a very long title in full. Test `a_book_title_and_the_document_line_do_not_make_r2`.
- `2026-10-20` — R22 (hard, in `DEFAULT_RULES` and the card cycle's `DEFAULT_CARD_RULES`): a title or body that leaves an inline formula open (`open_math`: an odd number of unescaped `$` once display blocks and code are set aside). On grossman2002integration trial 8 the group 4 writer built its draft through a PowerShell double-quoted string, so `$A_I` expanded to nothing and the anchor title `普遍纵向一体化均衡稳定当且仅当 <A_O$` passed the writer's SELF-CHECK and group-finish; only Organize's notes cycle repaired it. The digest base prompt asks for paired `$` and no `$` text in double-quoted strings; the card instructions say how to repair R22. Test `an_open_inline_formula_is_r22`.
- `2026-10-20` — `check::title_fits` and `check::sentence_fits`: public length checks with the R12 and R2 limits and the same `measure`, so a tool that composes a Card (the zt-digest term table) can choose a title or sentence that the checker will accept.
- `2026-10-19` — #107 S08: `graph::is_restatement_entry` and the `restatement-entry` category make an accepted restatement entry navigation (§4.5, D086). `category_with_parts` classifies a Card whose authored body holds a `## 同述入口` section, or whose `export_titles` row says `restatement`, as `restatement-entry`: it joins `EXCLUDED` (so it is no orphan), is added to the language rules' `navigation` list, and enters `layer` for the source-line rule (I3, broadened to `idea`, `event` and `restatement-entry`) and the Link rules, with the section's own navigation Link left out of L1 and L2 (`entry_section` names its Links and list lines). I1, I2, I4 and I7 stay with a substantive Idea, and I6 neither checks an entry nor offers one as a duplicate-title candidate: `TitleRow` carries `restatement` and `TitleContext::from_titles` reads it, so a title-only context excludes an entry's title. The unit tests cover the derived category (section, flag, and prose that only names the word), the kept source and Link checks, the excluded I1/I2/I4 rules, and an entry title that is no I6 candidate.
- `2026-10-19` — #104 S11: `graph::category_with_parts` adds the `register-part` navigation category, derived from the register's own `词条分册` declaration (`check::declared_register_parts` reads both Workspaces' `get_register` once, at `entry_limit: 1`). It joins `EXCLUDED`, so an orphan report accepts it as navigation rather than an unknown category, and it joins the language rule's `navigation` list, so a part's rows take no claim-title, size, or source-line rule of their own. `graph::category` keeps its signature and passes no declared part, so a scope that read no register classifies exactly as before. `audit::book_of::book_index_with_parts` lets the audit use the same category from the register it already reads; `book_index` stays for the scopes that read none.
- `2026-10-19` — #102 (S25): `zt-check --version` prints `zt-check <crate version>` (from `zt_check::VERSION`) and exits 0 before any option parsing or archive read, so a calibration review records the checker that produced its lines rather than a path or a file date. Pinned by `the_checker_states_its_version`. No rule changes.
- `2026-10-19` — S03 re-verification: the candidate index folds case, so an exact copy that differs only in case is found by the index instead of a per-Card scan of the whole context (the scan is gone; the same pairs are reported, and a 5,000-entry `--json` run over the live 10,895 titles fell from 43 s to 9 s with byte-identical output). A supplied context with an entry that names no `workspace` marks I6 unknown and compares nothing, so no pair crosses a Workspace boundary. `anchor-title` gained the topic hub's second anchor (`<n>/0`), matching the compiler precheck. Pinned by `an_exact_title_copy_is_found_without_scanning_the_context`, `an_entry_without_a_workspace_marks_i6_unknown`, and `a_topic_hub_that_copies_its_topic_title_is_named`.
- `2026-10-19` — A supplied Workspace title context that is not complete marks I6 unknown instead of silently comparing the known rows (`complete` was parsed but unread). `TitleContext` gains `supplied`; `similar_titles` reports one `(advisory)` line per checked Idea Card before its pair lines, naming the known titles compared, and still reports the pairs it can decide. A bundle with no context keeps the documented run-scoped comparison. Pinned by `an_incomplete_title_context_marks_i6_unknown` and by the layer-advice assertions of `scripts/book-digest/tests/standalone-and-source.tests.ps1`.
- `2026-10-19` — #101 (S10): `zt-digest register -Step plan` reads every agent book term table through the same public `check::term_table` (#97), so the register's proposed rows and R6 never disagree about a row either. No rule changes.
- `2026-10-19` — #97 (design D063): expose the term-table parse as a small public library function for the digest. `check::term_table(body) -> Vec<TermRow>` returns all four columns (`术语`, `不用`, `意思`, `首次定义`), while `read_terms` keeps R6's own view of the same parse: the first two columns, a row with no rejected word skipped before it can replace an earlier row, and the ordered-hashtable replacement of a term that differs only in case. R6's faults are unchanged, which the unit test `the_term_table_exposes_all_four_columns_without_changing_r6` pins.

- `2026-10-19` — #93 S03: I6 compares a checked Idea title with every title of its own Workspace through a read-only title context (an inverted bigram index, the same Dice function, one line per unordered pair, only pairs touching the checked Cards). `--cards` builds the context from its complete `export_titles` read; `--json` accepts a `{"cards": …, "context": …}` bundle. The entry contract gains `workspace`.
- `2026-10-19` — The standalone pilot rules of #93 S02: R20 (an unexplained local reference, with its content constant), R21 (a deictic with no named Link in its sentence), and I7 (a Topic Idea Card resting on its own source's Literature tree, with unknown source evidence reported as unknown). `relations` (the kind word of every kinded Link), `sources`, and `tree` join the entry contract; `check_cards` supplies `tree` and `relations` from the bulk readers, `digest-audit` writes `relations`, and both default rule lists carry R20, R21, and I7. The audit splits advice from blocking style faults (`style.advisory`, `style.advice_by_rule`).
- `2026-10-17` — Bound MCP writes/reads and shutdown using one in-process I/O worker and owned process containment. Add typed cleanup facts, native Windows kill-on-close ownership and public hangs/parent-death/real receipt-recovery/service-lifetime tests.

- `2026-10-17` — All live modes use required lean Card batches or complete model contexts through one client, preserve Link/category/term/explanation rules, reuse filed-model ownership within the command, and refuse revision changes. Public tests cover the removed per-Card/tree reads and the existing fault outputs.

- `2026-10-17` — Add the shared bounded bulk client used by digest link preparation, with public tool-boundary tests for chunk reduction and explicit missing-input failures. Migrate `--addresses` and external setting reads; the Card input-mode migration remains in progress in #67.

- `2026-10-16` — Every digest script forwards to a `zt-digest` tool (stage 2), so the card, model, hub, Bridge, event, and finding checks all run in-process through `check::Check`; `Invoke-ZtCheck` stays for PowerShell callers.
- `2026-10-16` — zt-check becomes a library too (`src/lib.rs`): `check::run_lines` returns the lines on a given `Zt`, `Zt::request` returns a raw reply, and `reply_text`/`reply_error` read it, for zt-digest.
- `2026-10-16` `753bd7b` — Finding Cards: the F1 to F4 rules (`finding_rules`), category `finding` in `--cards` (from the Card type), F1–F3 added to `DEFAULT_RULES`, I5 also on Finding Cards.
- `2026-10-15` `02499c0` — zt-check created: the Rust crate replaces `card-style-check.ps1` and `self-check.ps1` with the same rules R2–R17, I, E, L; modes `--json`, `--model`, `--addresses`, `--cards`; the `zt mcp` client; `scripts/install-zt-check.ps1`; DeepSeek writers run it on their own Cards.
- `2026-10-16` — Wiki doc created.
