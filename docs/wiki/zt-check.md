---
block: zt-check
title: zt-check
paths:
  - "tools/zt-check/**"
  - "scripts/install-zt-check.ps1"
synced: fe5d77864def
updated: 2026-10-16
---

# zt-check

## Purpose

`zt-check` is the Card quality check of the book digest, built as one fast Rust binary. It reads Cards (from a JSON file of drafts, or from the live archive through `zt mcp`) and prints one line per fault against the Card language rules (R2 to R17) and the Card layer rules (I, E, F, L). Every agent that writes digest Cards calls it: Claude sessions, workflow agents, Haiku managers, DeepSeek Harness runs, and the digest scripts through `Invoke-ZtCheck`. It is the only source of truth for Card quality (`tools/zt-check/src/main.rs`, `tools/zt-check/src/check.rs`). It exists as a crate of its own so that it builds and installs without building zt or stopping the ZT service (`tools/zt-check/Cargo.toml`, `scripts/install-zt-check.ps1`).

**Owns:** the rule definitions and their fault text, the order of fault lines, the default rule set of `--cards`, the size limits and their defaults, the Card categories of `--cards` mode, the `--json` entry format, the `zt mcp` client in `tools/zt-check/src/mcp.rs`, and the build and install of `zt-check.exe`.

**Does not own:** the MCP tools it calls (`get_card`, `get_model`, `get_tree`, `list_trees`; see [MCP server](mcp-server.md)), the ZT service that `zt mcp` needs (see [Service, CLI and storage](service-and-cli.md)), Relation kinds and Link parsing inside zt (see [Cards, Workspaces and Links](cards-and-links.md)), the build report and Elaboration level (see [Models and math](models-and-math.md)), the PowerShell wrapper `Invoke-ZtCheck` and the compilers and audit that build its input (see [Digest scripts](digest-scripts.md)), the SELF-CHECK lines in the agent prompts (see [Digest prompts](digest-prompts.md)), the approval rules in the workflows (see [Digest workflows](digest-workflows.md)), and the `zt-digest-protocol` skill that documents it for agents (see [Agent skills](agent-skills.md)).

## At a glance

| | |
|---|---|
| Entry points | `zt-check` command line (`main` in `tools/zt-check/src/main.rs` → `check::run_cli`); `scripts/install-zt-check.ps1` (build and install) |
| Key files | `tools/zt-check/Cargo.toml` — a crate with its own `[workspace]`, so it never builds zt; three dependencies (`anyhow`, `regex`, `serde_json`) |
| | `tools/zt-check/Cargo.lock` — pinned versions (regex 1.13.1, anyhow 1.0.104, serde_json) |
| | `tools/zt-check/src/main.rs` — collects the arguments, calls `check::run_cli`, prints `zt-check: <error>` and exits 1 on an error |
| | `tools/zt-check/src/check.rs` — the rules, the PowerShell text semantics, the four input modes, the option parser, the output, and the unit tests |
| | `tools/zt-check/src/mcp.rs` — `Zt`, a minimal JSON-RPC client that starts `zt mcp` and makes `tools/call` requests |
| | `scripts/install-zt-check.ps1` — `cargo build --release` into `D:\zt-check-target`, then copies `zt-check.exe` beside `zt.exe` |
| Depends on | [MCP server](mcp-server.md) (`zt mcp`, read tools only); [Service, CLI and storage](service-and-cli.md) (the service must be up for `zt mcp` tool calls); the answer shapes of [Models and math](models-and-math.md) (`get_model` `head`, `parts`, `level`, `build.setting`, `build.order`) and [Cards, Workspaces and Links](cards-and-links.md) (`get_tree` `links`, `kinds`); Rust crates `regex`, `serde_json`, `anyhow` |
| Used by | [Digest scripts](digest-scripts.md) (`Invoke-ZtCheck` in `scripts/book-digest/zt-calls.ps1`, used by `card-compiler.ps1`, `model-compiler.ps1`, `event-compiler.ps1`, `finding-compiler.ps1`, `hub-compiler.ps1`, `digest-audit.ps1`); [Digest prompts](digest-prompts.md) (DeepSeek SELF-CHECK commands); [Digest workflows](digest-workflows.md) (managers approve `zt-check` as read-only); [Agent skills](agent-skills.md) (`skills/zt-digest-protocol/SKILL.md`, `skills/zettelkasten/SKILL.md`) |
| Tests | `mod tests` in `tools/zt-check/src/check.rs`; `scripts/book-digest/tests/zt-check.tests.ps1` with `card-style-fixture.json`; `scripts/book-digest/tests/card-layer.tests.ps1` with `card-layer-fixture.json` |

## Concepts

- `Card language` — the plain-text rules for Card titles and bodies (ASD-STE100 adapted for Chinese). zt-check checks rules 2 to 7, 9, and 12 to 17 of the list in [docs/self-sufficient-models-proposal.md](../self-sufficient-models-proposal.md) and [docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md); rules 1, 8, 10, and 11 are not mechanical and are not checked (`Check::language`, `Check::sentence` in `tools/zt-check/src/check.rs`).
- `Card layer rules` — the checks on Cards outside models: I (Idea Cards), E (Event Cards), F (Finding Cards), L (Links). They run only on entries that carry a `category` (`Check::layer`, `finding_rules`).
- `Fault line` — `<address> <rule> <text>`: the text on one line, white space collapsed, clipped to 90 UTF-16 units with `…` (`Faults::add`). A run ends with `faults: N`.
- `Advisory` — a fault line whose text starts with `(advisory)` (I1, I6, L3, E5). In `--cards` mode it is printed but not counted (`filter_rules`); in the other modes every line counts.
- `Entry` — one Card as zt-check reads it: a JSON object with `address`, `title`, `body`, `facet`, and optional layer fields (see "Data and storage"). Property names ignore case (`prop`).
- `Category` — what a Card is for the layer rules: `idea`, `anchor`, `event`, `finding` get layer rules; `root`, `hub`, `part`, `model`, `terms`, `structure` get the R rules only. `--cards` mode computes it as `digest-audit.ps1` (`Get-Category`) does (`check_cards`).
- `Model part` / `Model Card` — read from `get_model`; a Card with a `facet` is a part. A part whose facet starts with `推导` (or `derivation`, any case) is a `Derivation part`, which gets the step and explanation rules R14 to R17 (`Check::language`).
- `Elaboration level` — `concept` or `formal`. A concept model skips R13 to R17; a model with no level gets the formal rules (`Check::language`).
- `Term table` — the table under the line holding `术语表` in the setting Card's body (a model's setting, or the Book term table): `| term | words not to use |`. R6 reads it (`read_terms`, `Check::with_terms`).
- `Shared part` — a part whose `build.order` item (or `--json` entry) has `line: shared`. It follows its home model's term table, so R6 skips it; `--skip-shared` drops it from a `--model` run (`check_model`, `Check::with_shared`).
- `Limits` — the size numbers of R13, R14, R17, I2, L2 (`Limits` struct; defaults in "Configuration").
- `UTF-16 form` — text in which each character outside the Basic Multilingual Plane is two private-use stand-ins (U+E000.., U+E400..), so lengths, regex classes, and clips match .NET (`utf16_form`, `from_utf16_form`).

## Architecture

The binary has three parts: the command (`main.rs`), the checker (`check.rs`), and the zt client (`mcp.rs`). `check.rs` is itself layered: input readers build a list of entries, `Check::run` applies the language rules per Card and then the layer rules across Cards, and `run_cli` filters and prints.

```mermaid
flowchart TD
    main["main.rs: main()"] --> cli["check::run_cli"]
    cli --> parse["Options::parse"]
    cli -->|--json| cj["check_json (file, offline)"]
    cli -->|--addresses| ca["check_addresses"]
    cli -->|--model| cm["check_model"]
    cli -->|--cards / --log| cc["check_cards"]
    ca & cm & cc --> zt["mcp::Zt (spawns zt mcp)"]
    zt -->|get_card, get_model, get_tree, list_trees| server[("zt mcp → ZT service archive")]
    cj & ca & cm & cc --> run["Check::run"]
    run --> lang["Check::language + Check::sentence (R2–R17)"]
    run --> layer["Check::layer + finding_rules (I, E, F, L)"]
    lang & layer --> faults["Faults::add (fault lines)"]
    cc --> filter["filter_rules (--rules / DEFAULT_RULES)"]
    faults --> out["write_lines + faults: N"]
    filter --> out
```

| Component | File | Role |
|---|---|---|
| `main` | `tools/zt-check/src/main.rs` | Arguments in; on `Err`, `zt-check: {error:#}` on stderr and exit code 1 |
| `Options`, `run_cli`, `write_lines` | `tools/zt-check/src/check.rs` | Parse flags, pick one mode, run, filter, print UTF-8 with CRLF on Windows |
| `check_json`, `check_addresses`, `check_model`, `check_cards` | `tools/zt-check/src/check.rs` | Build the entries, the term table, the level, and the shared list for one mode |
| `Check` (`new`, `with_terms`, `with_shared`, `run`, `language`, `sentence`, `layer`) | `tools/zt-check/src/check.rs` | The rules |
| Text helpers (`plain`, `measure`, `prose_lines`, `split_sentences`, `passive`, `read_terms`, `leads_to`, `book_place`, `bigrams`, `similarity`) | `tools/zt-check/src/check.rs` | What text the rules see and how it is counted |
| PowerShell semantics (`ps_eq`, `ps_in`, `culture`, `culture_starts_with`, `culture_cmp`, `ps_string`, `truthy`, `strings`, `ps_int`, `key`) | `tools/zt-check/src/check.rs` | Reproduce the deleted PowerShell checkers' comparisons and conversions |
| `Zt` (`start`, `call`, `Drop`) | `tools/zt-check/src/mcp.rs` | The `zt mcp` client |

## Interfaces

### Command line

```text
zt-check --json <file> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
zt-check --model <address> [--skip-shared] [--setting <address>] [--level concept|formal] [--require-explain] [limits]
zt-check --addresses <a,b,...> [--setting <address>] [--level concept|formal] [--require-explain] [limits]
zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,R3,...] [limits]
limits: --max-sentences 12 --max-chars 400 --max-steps 6 --max-explain-sentences 16
        --max-explain-chars 600 --min-idea-chars 40 --max-idea-chars 300 --min-reason 8
```

(`USAGE` in `tools/zt-check/src/check.rs`; the doc comment in `main.rs` lists only three of the modes.)

| Mode | Input | Reads zt | Term table (R6) | Level | Layer rules | `faults: N` counts |
|---|---|---|---|---|---|---|
| `--json <file>` | A JSON array of entries, or one entry (BOM allowed) | no | `--setting` names an entry in the file; its body gives the table | `--level`, else none (formal rules) | yes, for entries with `category` | every line |
| `--addresses <list>` | Addresses (split on commas and white space) | `get_card` each | `--setting`, else the first Card's `model.build.setting` (from the list, else `get_card`) | `--level`, else the first Card's `model.level` | no | every line |
| `--model <address>` | Any Card of a model | `get_model`, `get_card` of the head (and of the setting when not a member) | `--setting`, else `build.setting` | `--level`, else the model's `level` | no | every line |
| `--cards <list>` / `--log <file>` | Addresses from the list plus one per line of the log file | `get_card` each, `list_trees` (agent), `get_tree` per tree | each model's own setting | each model's own level | yes, by computed category | hard (non-advisory) lines of the selected rules |

Flags:

| Flag | Applies to | Effect |
|---|---|---|
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
| R6 | No word the term table rejects for a term (the term's own text is removed first) | every sentence of a non-shared Card, when a term table is known | no | ``uses `<word>` for `<term>`: <sentence>`` |
| R7 | At most two `的` in one noun phrase (three `的` with at most 7 non-punctuation characters between) | every sentence | yes | `<sentence>` |
| R9 | No hedge or praise word (18 listed in `HEDGES`, such as `值得注意的是`, `精妙`) | every sentence | yes | `` `<word>`: <sentence>`` |
| R12 | Title at most 40 characters (25 words), and no `；`/`;` (two claims) | every title | yes | `(N characters > 40) <title>` or `two claims: <title>` |
| R13 | A Model part holds at most `max-sentences` prose sentences, else at most `max-chars` Chinese prose characters. Steps and explanation do not count | Cards with a facet, not in a concept model | yes | `(N sentences > 12) split the part into two or more parts` |
| R14 | A Derivation part holds at most `max-steps` numbered steps | Derivation parts, not in a concept model | yes | `(N steps > 6) split the derivation into a chain of Derivation parts` |
| R15 | The sections are exactly `思路, 起点, 步骤, 结果, 解读`, in that order | Derivation parts that have 思路 or 解读 (all of them with `--require-explain`), not in a concept model | yes | `sections <found>; want 思路, 起点, 步骤, 结果, 解读` |
| R16 | 思路 at most 5 sentences; 解读 at most 6 (`待写` does not count) | as R15 | yes | `(思路 has N sentences > 5)` |
| R17 | One `说明` line per numbered step, not before the first step, of at most 2 sentences; 思路 + 说明 + 解读 together at most `max-explain-sentences` sentences, else at most `max-explain-chars` characters | as R15 (the position errors are reported in every Derivation part) | yes | `a second 说明 line under step N: …`, `(N explanation sentences > 16) shorten the explanation` |
| I1 | Advisory: a title of at most 6 characters with no claim verb (`CLAIM_VERB`) | `idea`, `anchor` | yes | `(advisory) the title states no claim: <title>` |
| I2 | Prose size: over `max-idea-chars` is a split candidate, under `min-idea-chars` a merge candidate | `idea` | yes | `(N characters > 300) split candidate` |
| I3 | A Link to the book's Literature Card or one of its anchors (`<key>/<n>`) | `idea`, `event` with `book` and `links` | yes | `no source line: no Link to [[key]] or one of its anchors` |
| I4 | A kinded Link to a Card other than the parent | `idea` with `kinds` | yes | `no kinded Link to a Card other than the parent` |
| I5 | A Card other than the parent links here | `idea`, `event`, `finding` with `inbound` | no | `no Card other than the parent links here` |
| I6 | Advisory: no title in the same tree with bigram (Dice) similarity of 0.8 or more | `idea` with `tree` | no | `(advisory) a title very similar to [[a]]` |
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

### `zt mcp` client (`tools/zt-check/src/mcp.rs`)

zt-check has no MCP server mode of its own. It is an MCP client: `Zt::start()` runs `zt mcp` (the program in `ZT_CHECK_ZT`, default `zt` on PATH) with piped stdin and stdout and stderr discarded, sends `initialize` (`protocolVersion` `2025-06-18`), waits for the reply with id 0, and sends `notifications/initialized`. `Zt::call(tool, arguments)` sends one `tools/call` per line with a rising id, skips reply lines that are not JSON or have another id, joins the `text` parts of `result.content`, and parses them as JSON. A JSON-RPC `error` or `result.isError: true` becomes `zt: <message>` (the `error` field of a JSON message when there is one). Dropping `Zt` closes stdin, which ends the server, and waits for it. It mirrors the protocol of `skills/zt/scripts/zt-mcp.ps1`.

| Tool called | Arguments | Used by | Fields read |
|---|---|---|---|
| `get_card` | `{ at }` | all zt modes | `address`, `title`, `body`, `facet`, `type`, `tree`, `parent.address`, `model.head.address`, `model.level`, `model.build.setting` |
| `get_model` | `{ at }` | `--model` | `head.address`, `parts[]`, `level`, `build.setting`, `build.order[].line`, `build.order[].address` |
| `list_trees` | `{ workspace: "agent" }` | `--cards` | `trees[].kind`, `trees[].tree` |
| `get_tree` | `{ tree }` | `--cards` | `cards[].address`, `cards[].kind`, `cards[].links[]`, `cards[].kinds` (object keys) |

### Install script (`scripts/install-zt-check.ps1`)

```powershell
pwsh -NoProfile -File scripts\install-zt-check.ps1 [-TargetDir D:\zt-check-target] [-BinDir <folder>]
```

It sets `CARGO_TARGET_DIR` to `-TargetDir`, runs `cargo build --release` in `tools/zt-check`, deletes the old `<BinDir>\zt-check.exe` or, when a running check holds it open, renames it to `zt-check.exe.old-<yyyyMMdd-HHmmss>`, copies the new one, and prints `installed <target> (on PATH: <found>)`. `-BinDir` defaults to the folder of the `zt` on PATH (normally `~\.cargo\bin`); the script fails when `zt` is not on PATH. It never stops the ZT service.

## Behaviour

### A `--json` run (drafts, offline)

1. `run_cli` reads the file; `check_json` strips a BOM and parses an array (or wraps one object).
2. Entries with `line: shared` form the shared list. When `--setting` is given, the entry with that address gives the term table (`read_terms`); a setting that is not in the file gives no table (no `get_card`).
3. `Check::run` runs `language` on each entry in input order, keeping each Card's prose character count, then `layer` on the entries that carry a truthy `category`.
4. `run_cli` appends `faults: <number of lines>` and `write_lines` prints.

### A `--model` run

1. `Zt::start` launches `zt mcp`. `check_model` calls `get_model { at }`; `head.address` must exist.
2. The entries are the head (`get_card`) and every `parts[]` item, reduced to `address`, `title`, `body`, `facet` (`card_entry`).
3. The setting and level come from the flags, else from `build.setting` and `level`. Shared parts are the `build.order` items with `line: shared`; `--skip-shared` removes them.
4. `setting_body` takes the setting's body from the entries, else from `get_card`. `Check::run` runs; no entry has a category, so only R rules apply.

### A `--addresses` run

1. `get_card` for each address. The first Card's `model` gives the default setting and level for all of them.
2. As `--model` from step 4, with no shared list (R6 applies to every Card).

### A `--cards` run (self-check, as the audit sees it)

1. The addresses of `--cards` and of the `--log` file (one per line, BOM stripped) are de-duplicated (case counts, ignorable characters do not). An empty list prints `no Cards to check`.
2. `check_cards` calls `get_card` for each, `list_trees { workspace: agent }` to know the Literature trees, and `get_tree` once per tree to read each Card's `links` and `kinds`.
3. Each Card gets a category: `root` (the tree entry's kind is `topic` or `literature`), `hub`, `event`, `finding`, `part` (type `model` with a facet), `model`, then in an agent Literature tree `anchor` (`<key>/<n>`), `terms` (title starts with `术语表`), `structure` (body starts with `本卡是《…》的结构卡`), else `idea`.
4. `part` and `model` Cards with a model head are grouped by head; each group runs as one `Check` with its model's setting, level, and `require_explain = true`.
5. The rest run together; `idea`, `anchor`, `event`, `finding` entries get `category`, `parent`, `links`, `kinds`, and `book` (`--book`, else the tree when it is a Literature tree).
6. `filter_rules` keeps the lines whose rule is in `--rules` (default `DEFAULT_RULES`) and counts the non-advisory ones; `faults: N` is that count.

### How text is read

1. `prose_lines` drops fenced code (including chart fences), `$$` blocks, single-line `$$…$$`, headings, and lines that start with `|`, `>`, `<`, or `![`; it strips a list marker and marks numbered lines as steps.
2. `split_sentences` hides formulas and Links, splits after `。！？!?` and after a `.` followed by white space, then restores them.
3. `plain` replaces formulas with `F`, code with `C`, removes Links and bold marks. `measure` counts characters (after removing white space, quotes, brackets, and every `F`/`C`, any case) when the sentence holds a CJK character, else words.
4. In a Derivation part, a line `思路`/`起点`/`步骤`/`结果`/`解读` (bold allowed) opens a section; the position line `第 N 张，共 M 张` in 思路 is skipped; `说明：` lines under 步骤 are explanation; a `来源` line after 解读 is not.

### Rules the code keeps

- Fault order: per Card in input order: R12, then per sentence R2, R3, R4, R5, R6, R7, R9 (R17 position faults inline), then R15, R16, R17, then R13, R14. Layer faults follow all language faults: per Card I1, I2, I3, I4, I5, E1, E2, E4, F1–F4, L1, L2, L3; then I6 per tree (trees in culture order), then E5. In `--cards` mode the model groups come before the other Cards. Callers and tests compare lines in this order.
- R13 and R14 apply only when the level is not `concept`; R15 to R17 likewise. R13 applies only to Cards with a facet, so a Model Card is never size-checked.
- One `Zt` process per run; calls are sequential. zt-check never writes: it calls only read tools.
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
    "book": "Hart1995", "tree": "5", "inbound": ["5/2"], "inbound_kinded": 1 }
]
```

| Field | Meaning | Used by |
|---|---|---|
| `address`, `title`, `body` | The Card | all rules |
| `facet` | Makes the entry a Model part; `推导…` makes it a Derivation part | R13–R17 |
| `line` | `shared` puts the entry in the shared list | R6 |
| `category` | `idea`, `anchor`, `event`, `finding`; anything else (or none) skips the layer rules | I, E, F, L |
| `parent` | The parent's address | I4, I5 |
| `links` | Link targets out | I3, L1 |
| `kinds` | The targets of kinded Links | I4, L1 |
| `book` | The Citation key | I3, F2 |
| `tree` | The tree id | I6 |
| `inbound` | Addresses that link here | I5 |
| `inbound_kinded` | How many Idea Cards link here with a kind word | E4, F4 |

The term table in a setting body:

```markdown
术语表

| 术语 | 不用 |
|---|---|
| 清算 | 卖资产、变卖资产 |
| 可承诺的还款 | 还款额 |
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

The fixed limits R2 (40/30 characters, 25/20 words), R12 (40/25), R16 (5 and 6), R17 (2 per 说明), I1 (6 characters), I6 (0.8), and E5 (0.5) are constants in `tools/zt-check/src/check.rs`.

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

1. Add the check in `Check::sentence` (per sentence), `Check::language` (per Card), `Check::layer` or `finding_rules` (layer, by category), and report it with `faults.add(address, "<Rule>", text)`. Prefix an advisory text with `(advisory)`.
2. Keep the rule id matching `FAULT_LINE` (`[RIELF]\d+`); a new letter needs that pattern changed, or `--cards` drops its lines.
3. Add it to `DEFAULT_RULES` if `--cards` should report it by default.
4. Add a unit test in `mod tests` with the exact lines, and update the PowerShell fixture tests if the `--json` output of their fixtures changes.
5. Run `cargo test`, then `pwsh -NoProfile -File scripts\install-zt-check.ps1`, then the PowerShell tests. Tell callers that filter rule letters (see pitfalls).

### Adding an input field

Read it with `prop(card, "<name>")` and the PowerShell conversions (`text`, `strings`, `ps_int`, `truthy`). For `--cards`, also set it in `check_cards` (the item built for layer Cards), or the rule never fires there.

## Testing

| Test | Command | Covers |
|---|---|---|
| Unit tests, `mod tests` in `tools/zt-check/src/check.rs` (10 tests) | `cd tools/zt-check; $env:CARGO_TARGET_DIR='D:\zt-check-target'; cargo test` | One fault per rule on `card-style-fixture.json` (included with `include_str!`), card size, R15–R17, the position line, the I/E/L rules, the F rules, UTF-16 lengths and clipping, sentence splitting, culture order, the rule filter |
| `scripts/book-digest/tests/zt-check.tests.ps1` | `pwsh -NoProfile -File scripts\book-digest\tests\zt-check.tests.ps1` | The same fixture, card size, and R15–R17 through `Invoke-ZtCheck` |
| `scripts/book-digest/tests/card-layer.tests.ps1` | `pwsh -NoProfile -File scripts\book-digest\tests\card-layer.tests.ps1` | I, E, L rules on `card-layer-fixture.json`, plus formulas and table pipes |
| Compiler tests on scratch archives | `pwsh -NoProfile -File scripts\book-digest\tests\finding-compiler.tests.ps1 [-Zt <zt.exe>] [-ZtCheck <zt-check.exe>]` (also `event-compiler`, `model-compiler`, `hub-compiler` tests) | `--addresses` and `--json` against a live scratch `zt mcp` |

The PowerShell tests run the `zt-check` found on PATH (`$script:ZtCheck` in `zt-calls.ps1`), not the source: install first. `finding-compiler.tests.ps1` copies the debug builds from `D:\zt-target` and `D:\zt-check-target` into a scratch `bin` at the front of PATH and sets `ZT_CONFIG_DIR`, so zt-check's `zt mcp` reaches the scratch archive.

Gaps: no test of `mcp.rs`; no unit test of `--model`, `--addresses`, or `--cards` (they need a live `zt mcp`; only the compiler tests exercise `--addresses`); no test of `Options::parse` errors; I5, I6, E4, F4 are tested only through `--json` fields.

## Decisions and pitfalls

### Decisions

- **One checker, the only source of truth.** zt-check replaced `card-style-check.ps1` and `self-check.ps1` (deleted 2026-10-15) with the same faults, in the same order, in the same words, so every agent and script gets one fault format (`tools/zt-check/src/check.rs` header; [docs/digest-card-layers-proposal.md](../digest-card-layers-proposal.md) D4 and its note; `skills/zt-digest-protocol/SKILL.md`).
- **PowerShell semantics in Rust.** To keep the old output exactly, lengths count UTF-16 units, matches ignore case, string comparisons drop culture-ignorable characters, hashtable keys ignore case, and sorts use an approximation of ICU collation (`weight`, `culture_cmp`).
- **A crate of its own, read through `zt mcp`.** It needs none of zt's code and sees the same answers as every other zt client; building or installing it never touches the running service (`tools/zt-check/src/mcp.rs`, `tools/zt-check/Cargo.toml`). Build into `D:` because E: is nearly full (`scripts/install-zt-check.ps1`).
- **R6 is not a default rule.** A book term table may name different concepts as synonyms, so a fix by it changes meaning; the audit reports R6, and the card cycle fixes it only on request ([docs/digest-card-layers-proposal.md](../digest-card-layers-proposal.md) D13).
- **Concept models keep their parts whole.** R13 to R17 apply only to formal models and models with no level; a concept model has no derivation to split ([docs/self-sufficient-models-proposal.md](../self-sufficient-models-proposal.md) "Card language", [docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md) section 4, [docs/model-elaboration-levels-design.md](../model-elaboration-levels-design.md)).
- **Older Derivation parts are silent.** A part with neither 思路 nor 解读 raises no R15 unless `--require-explain`; writers set it, the build report and the audit do not ([docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md)). `--cards` always sets it for model groups.
- **Finding Cards:** I3 does not apply (F2 is stricter), I1, I2, I4, I6 do not apply, F4 is not default like E4 ([docs/finding-cards-design.md](../finding-cards-design.md)).
- Related design docs: [docs/self-sufficient-models-proposal.md](../self-sufficient-models-proposal.md) (the Card language table), [docs/model-compiler-design.md](../model-compiler-design.md) (writers check their own Cards; the report step runs `--model --skip-shared`), [docs/zt-digest-framework.md](../zt-digest-framework.md) (drafts need `category`).

### Pitfalls

- **`faults: N` means different things.** In `--cards` it counts hard lines of the selected rules; in `--json`, `--model`, `--addresses` it counts every line, advisory included. A `--json` run on layer entries can report `faults: N > 0` with only advisory lines.
- **`--cards` never fires I5, I6, E4, F4.** `check_cards` sets no `inbound`, `inbound_kinded`, or `tree`, so those rules need `--json` entries that carry them (as `digest-audit.ps1` writes). They are also outside `DEFAULT_RULES`.
- **`--cards` knows books only in the agent Workspace** (`list_trees { workspace: "agent" }`); a Card in a human Literature tree is categorised as if it were in a Topic tree.
- **`--cards` checks shared parts under the model in the group** (no shared list); harmless by default because R6 is not a default rule.
- **`--addresses` takes the setting and level from the first Card's model** for all Cards; mixing models gives the wrong term table and level.
- **`PLACEHOLDERS` is `(?i)F|C`:** it removes every `F`, `f`, `C`, `c`, not only the formula and code placeholders, before counting Chinese sentence size, I1 titles, L2 reasons, and bigrams. This mirrors the PowerShell `-replace`; changing it changes counts.
- **English prose counts zero characters.** `measure` gives words for a sentence without CJK, and `language` adds only character sizes, so R13's character limit and I2 see English Cards as empty (I2 then reports a merge candidate).
- **No timeout on `zt mcp`.** `Zt::receive` blocks until a line with the id arrives; `zt mcp` stderr is discarded, so a stuck or failing server shows only as a hang or `zt mcp closed`. The ZT service must be up for tool calls.
- **`--help` exits 1** (it is reported as an error) and `--log` with a missing file is ignored silently.
- **Rule-letter filters in callers.** `card-compiler.ps1` (`Invoke-StyleCheck`) and `digest-audit.ps1` (`Invoke-Check`) keep only lines matching `[RIEL]\d+`, so F lines are dropped there; `model-compiler.ps1` explain-check keeps only `R\d+`. A new rule letter must be added to those patterns too.
- **Install before testing.** `Invoke-ZtCheck` resolves `zt-check` from PATH once, when `zt-calls.ps1` is dot-sourced; a changed PATH after that has no effect.
- **A locked `zt-check.exe`** is renamed aside, not deleted; old `zt-check.exe.old-*` files collect in the bin folder.
- **Stale descriptions:** the `description` in `tools/zt-check/Cargo.toml` and the comment in `scripts/book-digest/zt-calls.ps1` name the layer rules I, E, L without F, and the doc comment in `tools/zt-check/src/main.rs` omits `--addresses`.
- **R16 does not report an empty 思路 or 解读**, although [docs/derivation-explanations-proposal.md](../derivation-explanations-proposal.md) lists "an empty section" as an R16 fault; only too many sentences are reported.

## Change history

- `2026-10-16` `753bd7b` — Finding Cards: the F1 to F4 rules (`finding_rules`), category `finding` in `--cards` (from the Card type), F1–F3 added to `DEFAULT_RULES`, I5 also on Finding Cards.
- `2026-10-15` `02499c0` — zt-check created: the Rust crate replaces `card-style-check.ps1` and `self-check.ps1` with the same rules R2–R17, I, E, L; modes `--json`, `--model`, `--addresses`, `--cards`; the `zt mcp` client; `scripts/install-zt-check.ps1`; DeepSeek writers run it on their own Cards.
- `2026-10-16` — Wiki doc created.
