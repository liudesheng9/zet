---
block: agent-skills
title: Agent skills and integrations
paths:
  - "skills/**"
  - "scripts/install-agent-integrations.ps1"
synced: f1a113d96258
updated: 2026-11-09
---

# Agent skills and integrations

## Purpose

This block holds what AI agents get to know about ZT, and the means to put it in front of them. It has three agent skills: `zt` teaches the slip-box's Workspaces, the `zt` MCP server's tools, and the Access rules. `zettelkasten` teaches how to compose, place, and link a Card the way Luhmann worked. `zt-digest-protocol` teaches the digest base, the shared part of every book-digest workflow. `zt-model-fold` teaches how to fold a model the digest split into several Model Cards into one (input: a citation key). `zt-model-settings` teaches how to gather every setting, assumption and symbol of one model into its setting part (input: the Model Card's address). `zt-model-import` teaches how one model uses other models' building blocks without copying them (input: the Model Card's address): it finds what the model takes from other models and writes the `**引入**` import lines (`单卡`, `整块`). `zt-model-restructure` teaches how to rebuild the inner structure of one model, given its Model Card's address, as an intuition spine (chapters, results, proofs) with one notation card and one card format per role, shaped for the GUI's model network (each chapter hangs under the chapter it builds on most; no forward or unanchored thread). `zt-literature-organize` teaches how to reorganize one Literature tree so it reads in the order and division of its source, for any discipline and any form of material (input: a citation key). It also holds the shell helper `zt-mcp.ps1`, which lets an agent with no MCP tools (DeepSeek Harness) call the same server, and the installer `scripts/install-agent-integrations.ps1`. The installer copies the skills into Claude Code, Codex, and DeepSeek Harness and registers `zt mcp` as an MCP server. The block exists because the server only enforces the rules (agent Access, Card rules, confirm plans). The conventions that make Cards good (one idea, claim titles, kinded Links, model structure) live in skill text, and every agent harness must load that text from one source.

**Owns:** the skill texts and their frontmatter (`skills/*/SKILL.md`, `skills/*/references/*.md`, `skills/*/agents/openai.yaml`); the helper path `skills/zt/scripts/zt-mcp.ps1`, a forwarder to `zt-digest call` and, with `open-call` first, to `zt-digest open-call` (the helper itself, with batching, `@@N` references, `-Allow`, `-Beneath`, `-Log`, `-SrcLog`, `-Moves` and `append_card`, is owned by [zt-digest](zt-digest.md); the Write scope it declares is the [MCP server](mcp-server.md)'s); the global installation and registration done by `scripts/install-agent-integrations.ps1`.

**Does not own:** the MCP server, its tool schemas and its `INSTRUCTIONS` text ([MCP server](mcp-server.md), `src/mcp.rs`); the Card operations and confirm plans behind the tools ([Card operations](card-operations.md)); the Card checker that enforces the Card language ([zt-check](zt-check.md)); the digest scripts that call the helper, including `zt-calls.ps1` ([Digest scripts](digest-scripts.md)); the DeepSeek instruction files ([Digest prompts](digest-prompts.md)); the Workflow scripts and the framework that builds them ([Digest workflows](digest-workflows.md)); the service that `zt mcp` needs ([Service, CLI and storage](service-and-cli.md)).

## At a glance

| | |
|---|---|
| Entry points | A harness loads a skill when its `description` matches the task (Claude Code from `~/.claude/skills/<name>/SKILL.md`, Codex from `~/.codex/skills/<name>`, DeepSeek Harness from `~/.dsh/skills/<name>` or the `skills` field of `deepseek_run`). `pwsh -File scripts/install-agent-integrations.ps1 [-ZtPath <zt.exe>]` installs everything. `zt-digest call <calls.json or ->` calls the server from a shell; `pwsh -NoProfile -File <skill dir>/scripts/zt-mcp.ps1` with the same arguments forwards to it. |
| Key files | `skills/zt/SKILL.md` — the slip-box and the MCP tools: what to call, the Access rules, the helper, how to develop zt. |
| | `skills/zt/agents/openai.yaml` — display name and short description of the `zt` skill for Codex. |
| | `skills/zt/scripts/zt-mcp.ps1` — the helper path: forwards its arguments and stdin to `zt-digest call`, which runs a batch of tool calls on one `zt mcp` and prints one line per call. |
| | `skills/zettelkasten/SKILL.md` — how to compose, place, and link a Card: filing steps, Card language, Event, Finding, and Model Cards, model structure, charts, figures, addresses, placement, Links, hubs, maintenance. |
| | `skills/zettelkasten/agents/openai.yaml` — display name and short description of the `zettelkasten` skill for Codex. |
| | `skills/zettelkasten/references/luhmann-and-practice.md` — the research behind each zettelkasten rule, with sources, and the mapping of Luhmann's box to ZET. |
| | `skills/zt-digest-protocol/SKILL.md` — the protocol of the digest base only: roles, the compiler cycle, the base stages, slots, modes, and args, the profile contract a variant fills, the rules every run keeps, quality checks, operating rules, the Bridge contract. |
| | `skills/zt-digest-protocol/references/scripts.md` — a table of the base's tools, instruction files, workflow files, and tests. |
| | `skills/zt-model-fold/SKILL.md` — the fold skill (input: a citation key): decide how many models the book holds by the models' nature (the same question and primitives continued, or a model of its own), never by the book's size; fold each model of two or more Model Cards into one around zt's retype rule; verify and report the decision table; `scripts/` (`zt.mjs` helper, `model-survey.mjs` with its test (candidate groups from rests-on Links and shared parts), `mask-links.mjs` with its test, `check-model.mjs`) run through `zt-digest call`. |
| | `skills/zt-model-settings/SKILL.md` — the settings skill (input: the Model Card's address): every setting, assumption and symbol of a model in the setting part as a dependency tree (each setting under the setting it builds on, its premises in a **起点** section, the base blocks before one branch per model variant, each symbol defined at first use), nothing else there, no setting in a chapter; `scripts/` (`zt.mjs` helper, `check-model.mjs`, `check-settings.mjs` with its test) run through `zt-digest call`; `check-settings.mjs` refuses a setting outside the setting part and a setting part that is not a dependency tree (`start`, `misfiled`, `order`), a setting card or notation card out of place (`place`), a symbol a setting uses without a notation row (`uncovered`), a notation row no card uses or with the wrong first card (`unused`, `first`), a setting card not in the one format (`format`), a restated setting (`repeat`) and an assumption named without its Link (`cite`); it advises on balance, and with `--symbols` lists the symbols each setting uses first. |
| | `skills/zt-model-import/SKILL.md` — the import skill (input: the Model Card's address): what an import is (`单卡`, `整块` with its block and closure, a whole model), what it is not (a False share, a legacy （共用） line, a copy), the plan → apply → repeat cycle, copies rewritten as variants by hand, imported symbols; `scripts/` (`zt.mjs` helper, `imports.mjs` with `plan`, `apply`, `check` and its test) run through `zt-digest call`. `imports.mjs check` fails on an import fault, a premise in another model or a legacy line; `plan` proposes `premise`, `upgrade`, `legacy` and `false share` lines and names whole-model Links, non-part premises and possible copies for the reader; `apply` writes them (refusing a locked tree) and reads the model again. `scripts/explain.mjs` (`plan`, `apply`, `finish`, tested by `explain.test.mjs`) is the skill's last step, the explanation refresh: it runs `zt-digest model -Step explain` / `explain-apply` on the one model in a temp work folder (no workflow, staging folder or ledger), the caller runs the one DeepSeek job, and `finish` relinks Terms in the changed parts only (`term-link -Cards` over a minimal staging folder that names the book), checks that a second plan changes nothing, and plans the explanations again. |
| | `skills/zt-literature-organize/SKILL.md`, `references/source-forms.md` — the Literature organize skill (input: a citation key): judgement-led, for any source form: the source's own division as the skeleton, the top line, nesting, whole units, reference lists at the tail, crossers, summary leads (each division opens on a Card that summarizes all of it, and an old lead is rewritten or demoted into the subdivision it belongs to), and the moves with their traps (`through`, `before`, the 2,000-rename limit and parking); `scripts/` (`zt.mjs` helper, `outline.mjs` with its test, `batch.mjs` with its test, `leads.mjs` with its test, `check.mjs` with its test) run through `zt-digest call`. The scripts show and move the tree; `check.mjs` looks at structure only (broken Links, lists at the tail, Card count, whole units unchanged) and never at the form of a Card or a source line. |
| | `skills/zt-model-restructure/SKILL.md`, `references/card-formats.md` — the restructure skill (input: the Model Card's address; `check-model.mjs` refuses a part's address): the working-memory principle, the intuition spine and its build rules, settings as primitives, forward faults fixed by moving cards, the card formats, the parts section (`**分部**`), what the model board draws; `scripts/` (`zt.mjs` helper, `tree-mover.mjs`, `check-drafts.mjs`, `check-model.mjs`, `check-network.mjs` and `check-premises.mjs` with their tests) run through `zt-digest call`; `check-network.mjs` loads the GUI's own layout code from the running service's `/layout.js` (or `ZT_LAYOUT`). |
| | `skills/zt-term-sync/SKILL.md` — the Term sync skill (input: none, or one citation key): Opus runs `zt-digest term-sync -Step status` and `-Step start`, runs the DeepSeek jobs through `dsmp-manager` agents, and reads their decisions itself before the write; each Term Card gets a Link to the Term Cards that name the same concept in the other books of its Literature type, and no Term or Card text changes otherwise. Design: `docs/term-sync-design.md`. |
| | `scripts/install-agent-integrations.ps1` — copies and links the skills, registers the server in Claude Code and Codex, checks the handshake. |
| Depends on | [MCP server](mcp-server.md) (`zt mcp`, the tools the skills name and the helper calls); [Service, CLI and storage](service-and-cli.md) (`zt` on PATH, `zt up`, `zt version`); [zt-check](zt-check.md) (the checker the zettelkasten and digest-protocol skills send agents to); [zt-digest](zt-digest.md) (`zt-digest call`, which the helper path forwards to); external: PowerShell 7.5 (the installer calls `ConvertFrom-Json -DateKind`; `zt-mcp.ps1` declares 7.5 too), the `codex` CLI (optional), Claude Code's `~/.claude.json` and `~/.claude/settings.json`, DeepSeek Harness's `~/.dsh`. |
| Used by | [Digest scripts](digest-scripts.md) (`zt-calls.ps1`, `model-compiler.ps1`, `organize-apply.ps1`, `card-compiler.ps1`, `math-compiler.ps1`, and the job files of `prepare-book-digest.ps1`, `prepare-link-pass.ps1`, `event-compiler.ps1`, `finding-compiler.ps1` call the helper); [Digest prompts](digest-prompts.md) (`dsh-digest-base.zh.md`, `dsh-link-instructions.zh.md`, `dsh-fill-instructions.zh.md`, `dsh-event-instructions.zh.md` show `zt-digest call` commands); [Digest workflows](digest-workflows.md) (`ZD_CALL` in `zt-digest-lib.js`: Organize's zt calls and the manager approvals, which name `zt-digest call` and pwsh with `zt-mcp.ps1`; and the `zettelkasten` skill sent to digest and link runs); every Claude Code, Codex, and DeepSeek Harness conversation that touches the slip-box. |
| Tests | `scripts/book-digest/tests/zt-helper-gate.tests.ps1` (helper `-SrcLog`, the server's Topic title rule and Literature scope through the forwarder, `open-call`), `scripts/book-digest/tests/zt-calls.tests.ps1` (helper output under a GBK console), `scripts/book-digest/tests/card-compiler.tests.ps1` (uses `append_card`). The skills and the installer have no tests. |

## Concepts

- `Skill` — a folder with a `SKILL.md` whose YAML frontmatter has `name` and `description`; the harness reads the description to decide when to load the body. Here: `skills/zt`, `skills/zettelkasten`, `skills/zt-digest-protocol`.
- `Workspace` — the `human` or `agent` half of the archive (`CONTEXT.md`). The `zt` skill tells the agent that `human` is read-only and `agent` is its own.
- `Access` — which Workspaces a client may change (`CONTEXT.md`). The MCP server and therefore the helper act with agent Access (`const ACCESS: Access = Access::Agent` in `src/mcp.rs`). The `zt` shell commands act with human Access, so `skills/zt/SKILL.md` forbids writing with them.
- `MCP server` — `zt mcp`, JSON-RPC 2.0 over stdio (`CONTEXT.md`, `src/mcp.rs`). Registered under the name `zt`; Claude Code exposes its tools as `mcp__zt__*`.
- `Card language` — the plain-text rules for titles and bodies (ASD-STE100 adapted for Chinese; `CONTEXT.md`). `skills/zettelkasten/SKILL.md` states them as a 12-row table; `zt-check` checks most of them.
- `Card type` — `idea` (default), `event`, `finding`, `model`, `hub`. The `zt` skill names the tool arguments; the `zettelkasten` skill gives the body templates (Event Card, Finding Card with six labelled lines, Model Card).
- `Model part`, `Model facet`, `Emphasis`, `Relation kind`, `Elaboration level`, `Math form`, `Derivation part`, `Derivation chain`, `Building block`, `Self-sufficient model` — the model vocabulary of `CONTEXT.md`. `skills/zettelkasten/SKILL.md` (*Model structure*) teaches how to design them; `skills/zt/SKILL.md` names the arguments and the `get_model` report fields.
- `Hub Card`, `Hub level`, `Register`, `Structure Card` — entry points (`CONTEXT.md`); the zettelkasten skill (*Entry points, hubs, and structure Cards*) says when to write one by hand.
- `Insert` — putting a new Card in another Card's place (`CONTEXT.md`); `insert_card` in the `zt` skill, and the sibling form `create_successor` with `kind: side` and `before`, which files a new Card in a Side child's place and shifts that sibling and every later one one label on (S33, #182).
- `Literature scope` — the Write scope `zt-digest call` declares (`docs/literature-only-digest-design.md`, `docs/agent-write-scope-proposal.md`): its `zt mcp` starts with `--scope literature`, so the server refuses `create_topic`, any write to a Topic-tree Card, a Hub Card, and authored text that links a Topic-tree Card, with `scope_refused`, as for every ZT digest. Hand work on Topic trees runs `open-call` (`zt-mcp.ps1 open-call …`); there is no `-Open` switch. `zt-idea-grow` writes its Topic trees in-process.
- `Topic title rule` — the server refuses an agent Topic whose title another agent Topic has (`topic_exists`, naming that Topic), so parallel hand runs never make the same Topic twice (`CONTEXT.md`). It replaced the helper's `-TopicGate`.
- `Call batch` — the helper's input: one JSON object `{"tool","arguments"}` or an array of them; `@@N` in a string argument is the address returned by call `N` (0-based) (`skills/zt/scripts/zt-mcp.ps1`).
- `Allow list` — the file passed with `-Allow`: one address per line; the run may change only those Cards and the Cards it creates (`skills/zt/scripts/zt-mcp.ps1`).
- `Compiler cycle` — survey, plan, DeepSeek writes, SELF-CHECK, apply, check, one fix round, report, cleanup: the basic block of every digest stage (`skills/zt-digest-protocol/SKILL.md`, section 2).
- `SELF-CHECK line` — the exact read-only check command in each job file; the DeepSeek writer runs it until `faults: 0` (`skills/zt-digest-protocol/SKILL.md`, section 5).

## Architecture

The block has four parts. The three skills are text only. The helper and the installer are PowerShell. The installer is the only bridge from the repository to the harnesses. Nothing reads `skills/` in place except the digest scripts' `zt-calls.ps1` and the tests, which call the repository copy of the helper.

```mermaid
flowchart LR
  subgraph repo[Repository]
    S1[skills/zt]
    S2[skills/zettelkasten]
    S3[skills/zt-digest-protocol]
    H[skills/zt/scripts/zt-mcp.ps1]
    I[scripts/install-agent-integrations.ps1]
    ZC[scripts/book-digest/zt-calls.ps1]
  end
  subgraph home[User home]
    CS[~/.claude/skills/&lt;name&gt; copy]
    XS[~/.codex/skills/&lt;name&gt; copy]
    DS[~/.dsh/skills/&lt;name&gt; junction]
    CJ[~/.claude.json mcpServers.zt]
    ST[~/.claude/settings.json allow mcp__zt]
    CT[~/.codex/config.toml via codex mcp add]
  end
  I -->|Copy-Item| CS
  I -->|Copy-Item| XS
  I -->|junction to Codex copy| DS
  DS --> XS
  I --> CJ
  I --> ST
  I --> CT
  I -->|handshake: initialize, tools/list| MCP[zt mcp - src/mcp.rs]
  CC[Claude Code] --> CS
  CC --> CJ --> MCP
  CX[Codex] --> XS
  CX --> CT --> MCP
  DSH[DeepSeek Harness run] --> DS
  ZD[zt-digest call]
  DSH -->|zt-digest call, as the prompts say| ZD
  DSH -->|pwsh installed helper, as older job files say| XH[~/.codex/skills/zt/scripts/zt-mcp.ps1]
  XH -->|forwards| ZD
  ZD -->|spawns, stdio| MCP
  ZC -->|pwsh repo helper| H -->|forwards| ZD
  WF[.claude/workflows/*.js] -->|ZD_CALL| ZD
  JF[job files of the PowerShell scripts] --> XH
```

### Skills

- `skills/zt/SKILL.md` is the tool manual. It restates, in agent-facing prose, what `src/mcp.rs` exposes: the tool names, their arguments, the report fields of `get_model`, `get_hub`, `list_hubs`, the confirm flow, the Scholar Archive tools, and the fallback through the helper.
- `skills/zettelkasten/SKILL.md` is the writing manual. It owns the conventions the server does not check: the body templates, placement rules, Link wording, model design, chart palette, entry points, maintenance. It sends the reader to `references/luhmann-and-practice.md` for the reasons, and to the `zt` skill for the mechanics.
- `skills/zt-digest-protocol/SKILL.md` is the operator's manual for the digest base (`zt-digest-base.blueprint.json`), not for a variant's layers. It names the roles, the compiler cycle, the base stages, the checks, and the operating rules; `references/scripts.md` lists the base's tools with the script each replaced (`zt-digest call`, `card`, `terms`, `skeleton`, `hub`, `bridge`, `prepare`, `snapshot-topics`, `prepare-link`, `organize-survey`, `organize-apply`, `figures`, `audit`; installed beside `zt.exe` by `scripts/install-zt-digest.ps1`; the workflows run them through `zdTool`) and names the variants' tools (`model`, `model-context`, `list-models`, `event`, `finding`, `math`) only to point to their docs. Since #93 S02 and S03 its quality-check section states the current rule set (R2 to R21 and the layer rules I, E, F, L, including I7), that R20, R21, I7's unknown-source line, and I6 are advice split from blocking faults, that the pilot's outcome target is under 30% of new Topic Idea Cards with an own-source `rests-on` dependency, and that a similar title never merges or deletes a Card. Since issue #184 it also states the Literature-note row (`applicability_version` 3, zt-check 0.15.0): the Idea-card sentence, claim-title, duplicate-title and own-source rules are off or advice for a note, R12 advises at 60 characters, I2 advises only below 80, and I3, R22, the Event/Finding formats and the Term rules keep their level.
- `skills/zt/agents/openai.yaml` and `skills/zettelkasten/agents/openai.yaml` hold an `interface` map with `display_name` and `short_description`, the names Codex shows for the skill. `skills/zt-digest-protocol` has no such file.

### The helper (`skills/zt/scripts/zt-mcp.ps1`)

The script forwards. It sets the console input and output to UTF-8, takes `zt-digest` from `ZT_DIGEST` or PATH (and fails with `zt-digest is not installed` when neither has it), pipes stdin through when its arguments contain `-`, runs `zt-digest call` with the same arguments (or `zt-digest open-call` when the first argument is `open-call`), and exits with that exit code. `zt-digest call` (`tools/zt-digest/src/calls.rs`) is one process per run: it starts `zt mcp` with its Write scope (`--scope literature`, and `--allow`/`--beneath` from `-Allow`/`-Beneath`), sends `initialize` (protocol `2025-06-18`) and `notifications/initialized`, then sends one `tools/call` per call, in order, and waits for the reply with the matching `id`. Between calls it resolves `@@N`, strips `src`, and writes the logs; the server refuses a call outside the scope with a typed `scope_refused` line. At the end it closes the server's stdin and waits for it. The script's help text names `open-call` in the synopsis and has a Scope paragraph: the Write scope is the server's, `call` is the Literature scope, `open-call` is for hand work on Topic trees. The script keeps the line `zt-calls: direct reads`. Its documented output line also states the typed-refusal line a Lock produces (`card_locked <compact JSON>` just before `resume:`), that the Lock's tree, Lock id, reason and hint survive as JSON, and that such an answer is a destination refusal: it is never retried as a provider fault, and the tree is unlocked by its owner in the GUI (#137).

### The installer (`scripts/install-agent-integrations.ps1`)

Four sections in this order: skills, Claude Code, Codex, check. Each config file is backed up (`Backup`), rewritten through a temp file that is parsed again before it replaces the original (`Write-JsonAtomically`), and members are added or replaced with `Set-Member`.

## Interfaces

### Skill frontmatter (when each skill triggers)

| Skill | `name` | Triggers on (from `description`) | Body sections |
|---|---|---|---|
| `skills/zt/SKILL.md` | `zt` | The user mentions zt, ZET, the slip-box, zettelkasten, Cards, Topic or Literature trees; asks to record, file, look up, or connect notes, ideas, or papers; or works on the zt project itself. | Use the MCP server; Read the Scholar Archive; Rules; When the server is unavailable; Developing zt itself |
| `skills/zettelkasten/SKILL.md` | `zettelkasten` | Writing, filing, splitting, placing, or linking a Card; taking notes on a paper or conversation into the slip-box; building an outline from Cards; reviewing a tree's structure. | Filing a thought: the steps; Composing one Card (with the Card language table); Event, Finding, and Model Cards (Model structure, HTML charts, The source's own figures); ZET addresses; Placement; Links; Entry points, hubs, and structure Cards; Collector's fallacy; Maintenance |
| `skills/zt-digest-protocol/SKILL.md` | `zt-digest-protocol` | Running or changing the digest base, debugging a base stage, building a new variant on it. Not a variant's own layers. | 1 Roles; 2 The basic block: the compiler cycle; 3 The base stages (with modes and args); 4 What a variant fills; 5 Rules every run keeps; 6 Quality checks; 7 Operating rules; 8 Cross-book Bridge contract |

The frontmatter holds only `name` and `description`. The descriptions of `zt` and `zettelkasten` are double-quoted YAML strings; the `zt-digest-protocol` description is a plain scalar, so it must not contain `: ` or a leading special character.

### What each skill teaches

**`zt`** (`skills/zt/SKILL.md`):

1. Read before writing: `list_workspaces`, `list_trees` (filter `workspace`, `zone`), `get_tree` or `get_card`, `search_cards` before creating, `list_links` with `cross_workspace_only`. `search_cards mode: "entry"` ranks ways in first, and a row whose `folded` is above zero stands for several Cards that state one confirmed statement: its `representative` is the canonical Card and its `members` name every Card behind the row, so an agent reads those instead of treating the row's title as a new claim. `ask_box` answers a whole question with an unsaved Spread proposal (matched terms, a held entry Card, up to nine explained Cards, threads, counts and bounds): its low-confidence `fallback` must be said as such, `sparse` is not to be filled, `truncated` means refine the question, and it writes nothing. A Spread the human kept can be filed from the GUI's Table as one agent Hub Card draft (S17, #118): the skill says the desk opens with the question, the Spread's Links and their reasons, the placement controls choose the hub level and its Card, and the precheck must pass before its Save, so the agent reads or finishes that draft over MCP like any other agent Card.
2. Writing tools and their arguments: `create_topic`, `create_literature` (one BibTeX entry with a `title`), `create_successor` (`at`, `title`, `body`, `kind` `direct` or `side`), `edit_card`, `edit_literature_metadata`, `insert_card` (`before`), `move_card` (`to` or `before`, `alone`), `delete_card` (`into`).
3. Card types (`type`), Model parts (`facet` up to 32 characters, `emphasis` `core` or `aside`), Relation kinds by the word before the Link, the elaboration `level` (`concept`/`formal`, also `1`/`2`), the Math `form`, and the `get_model` report: `models`, `relations`, `loose`, `neighbours`, `issues`, `build` (`setting`, `order` with `line` `main`/`side`/`off`/`shared`, `chains`, `closure`, `forward`, `unanchored`, `tolerated`, `beyond_level`, `self_sufficient`), and `math` for units and Theory Cards. Since S28 (#170) it also states the Term Card type: no `term` name makes the Term list (its four-column body), a name makes a Term Card (immediate Side successor of the list) or a Term part (same-named Direct successor), `get_terms` reads the computed rows/heads/parts/uses/coverage/faults, a named Term Card keeps the authored template the T rules check, `自动链接：否` plus its reason opts out, and a direct Term use `[[label@address]]` must name a Term head and reads as Relation kind `term` (no Model membership or reasoning credit, graph views hide it by default). It adds the projection rule (`term_links: "label"` on `get_card`/`get_cards`/`get_tree`; raw for edit, receipt hash and fingerprint) and the gate rules: accepted warnings are kept, a `card_check` refusal is content for the finite fix rounds, and a relation word or synonym is never changed only to silence a check.
4. Bodies: `![caption](artifact:<sha256>)` figures, TeX, ```` ```chart ```` fences with `--zt-*` palette variables and `data-zt` clicks; never write a Reverse links section.
5. The confirm flow: `delete_card`, `move_card`, `insert_card`, and Citation key renames return a plan; call again with the `confirm` text (`delete`, the tree root's address, `move`, `insert`).
6. The Scholar Archive tools: `scholar_archive_status`, `scholar_archive_read`, `scholar_archive_sql`, `scholar_archive_file` (its `link` is a Document link `[name](scholar:<file_id>)`), `scholar_archive_export`, `scholar_archive_artifact`.
7. Rules: never change the human Workspace by any route (not by shell commands, the GUI API, or SQLite); write only through the MCP server; cite Cards by address.
8. Fallbacks: if the service is down, `zt status` then `zt up`; if the harness has no `zt` tools, use the zt helper `zt-digest call` (one binary beside `zt`; `scripts/zt-mcp.ps1` forwards to it with the same arguments): `zt-digest call calls.json` for a UTF-8 JSON file of calls, `@'...'@ | zt-digest call -` for calls piped in, or `zt-digest call -Tool get_card -Json '{"at":"3/1"}'` for one call. `@@N`, the output lines, and the stop at the first error are as before. `zt-digest call` runs in the Literature scope, as the ZT digests do (they write only Literature trees): the server refuses `create_topic`, writes to Topic-tree and Hub Cards, and Links to Topic-tree Cards with `scope_refused`; hand work on a Topic tree runs `zt-digest open-call` (`scripts/zt-mcp.ps1 open-call …`), and `-Allow` limits a run to its own Cards, enforced by the server (#227). The `zt` shell commands act with human Access, so never write with them.
9. Developing zt: `cargo test` from PowerShell (dump tests fail under Git Bash because of PATH), the deploy sequence (`zt down`, back up `zt.sqlite3` on a schema change, `cargo install --path . --locked`, `zt up`, `zt version`, `zt status`), the release set (install the checker and digest tools, install both frozen variants, reinstall the skills, then `scripts/verify-release.ps1`, which checks the binaries, frozen scripts, installed skills and the running service against `docs/term-cards-links-and-check-gate-release-manifest.json`), and re-running the installer after a skill or registration change.

**`zettelkasten`** (`skills/zettelkasten/SKILL.md`):

1. The seven filing steps: converse first (search both Workspaces), decide whether a Card is needed, compose, place, link (at least one Link besides the parent), update entry points, report.
2. One Card: one idea, a claim title (a Topic title names the subject), own words, standalone, about 50 to 250 words, the source line `来源：[[Key]] 第10页`, the tree's language, no generated parts.
3. The Card language table (rules 1 to 12: one statement per sentence; at most 40 characters per description sentence and 30 per derivation step; no `；`; active voice; a verb for an action; one term per meaning; at most two `的`; keep modality; no hedges or praise; lists for three or more items; a sentence for each display formula; a one-claim title of at most 40 characters). It says `zt-check` checks rules 2–7, 9, and 12.
4. Event Cards (title with date; what, when, where, who, source), Finding Cards (six labelled lines 测量, 数据, 方法, 结果, 局限, 来源; one result per Card; F1–F4 checked by `zt-check`), Model Cards (title with name and claim; level `concept` or `formal`; every discussing Card links it), and Term Cards (S28, #170: the book's Term list, Term Card and Term part roles; the exact name and authored template; a definition the composer's own argument needs is an Idea Card, not a Term Card; a Term use `[[label@address]]` is a vocabulary reference of Relation kind `term`, never a reasoning Link, written only for the literal word, never for a synonym or with a relation word added; preserve an existing macro byte for byte and respect `自动链接：否`).
5. Model structure: membership by filing or by the Model Card's Link, facets, kinded relations, shape, loose parts, emphasis, mathematical units and Theory Cards, self-sufficiency, build order (setting `…|a` with symbol and term tables, main line of Direct successors, examples as Side successors, Derivation parts and chains, building blocks), the Model Card's 问题, 概要, 结构 list, 关联, 来源.
6. HTML charts (palette variables, `viewBox` width up to about 440, `data-zt`), and the source's own figures (`list_work_artifacts`, `scholar_archive_artifact`, Document links with `#page=`).
7. ZET addresses (shape, roots, Direct and Side successors, Topic numbers never reused) and where ZET differs from Luhmann's box.
8. Placement (four rules in order), Literature Zone roles, Links with a reason (a table of relationships), entry points (Topic body, findable titles, Hub Card levels, Structure Card), the Collector's fallacy, and maintenance (split, merge, move, fix broken Links). A missing step goes in with `insert_card before` an occupied step, and a Link names a whole Card. Only agent Cards are split; the human Workspace stays read-only. The Hub Card rule names the GUI's filing of a kept Spread as one way such a draft begins (S17, #118). `references/luhmann-and-practice.md` compares this with Luhmann's own box (57/12a filed between 57/12 and 57/13).

**`zt-digest-protocol`** (`skills/zt-digest-protocol/SKILL.md`), the base only:

1. Roles: the `zt-digest` tools do every structural step and every zt write of a JSON answer; DeepSeek (V4.1 Flash, effort high) writes text; Haiku (role `relay`, the `zt-relay` agent type, for a one-command relay; role `manager` otherwise) runs commands and manages DeepSeek runs; Opus (role `organizer`) writes only `decisions.json`. The base declares `architect` (Sonnet) for the variants' model layers; no base stage uses it.
2. The compiler cycle, `drive()`, and the base's compilers (`zt-digest card`, `terms`, `skeleton`, `bridge`) with the Term stages as their own modules (`zt-term-cards`, `zt-term-link` with the shared `zt-term-link-lib`).
3. The base stage graph of mode `full` with its four empty slots (`after-prepare`, `after-digest`, `after-link`, `after-audit`): the book term table and the Term-card branch before the group writers, the per-group Card checks and the whole-book card cycle, the gap-only Link pass, Organize, Figures, the report-first Bridge, the deterministic Term-link pass after the Bridge, and the finite finalization with its read-only audit last; a table of every base stage with its module, steps, writes, and failure policy; the base modes (`full`, `resume`, `link`, `cards`, `figures`, `audit`, `bridge`, `term-cards`, `term-link`), the checkpoint `until: "digest"`, and the base's args including the Term settings (`termCards`, `termLink`, `termCardsPerJob`, `termCardsMathPerJob`, `termLinkOccurrences`).
4. What a variant fills: the profile fields the base stages read (`instructions`, `noEventCandidates`, `digestPrefix`, `linkPrefix`, `bridgeProfile`), the slot fills, its own modes and extra reports, and the steps to build and install one. It names no variant's layers.
5. Rules every run keeps: the DeepSeek Subagent Managed Protocol, the manager approval list (`zt-digest call`, `zt-check`, the job tool's `-Step precheck`, reads under `out`, the job's own JSON file), skills sent in the `skills` field, `state.json` per compiler, compact JSON last lines, the zt helper `zt-digest call` and its flags, source fidelity marks `（补：书中略去）`.
6. `zt-check` as the only checker, its command forms (including `--draft`, `--workspace` and `--report`), its output (`faults: N` counting blocking diagnostics, advice tagged `(advisory)`), its 0/1/2 exit contract, the Term rules T1–T10, the gate's advisory/enforce behavior with `card_check` as content and `card_locked` as a planning stop, and the SELF-CHECK command of each base task. The Link run's SELF-CHECK is `zt-digest link-finish … -CheckOnly`: the Link stage works only the receipt-owned semantic gap set (one `link-start` frozen scope per group), so its check covers the binding, the gap coverage and `L1,L2` instead of the old `zt-check --log` list. The Term SELF-CHECKs are the writer's `term-cards -Step check` and the reviewer's `-Step review-plan`; the Term-link pass proves preservation and idempotence.
7. Operating rules (back up with `VACUUM INTO`, run installed workflows by `scriptPath`, do not deploy zt during a run, build on D:, install `zt-digest` from `E:\zet`, the test commands, script traps) and the cross-book Bridge contract.

### Helper command line (`zt-digest call`, or `zt-mcp.ps1`)

`zt-digest call` takes the parameters below; `zt-mcp.ps1` passes its arguments and stdin through unchanged, so both forms work:

```powershell
zt-digest call <File|-> [-From N] [-Addresses a0,a1,...] [-Log <file>] [-Allow <file>] [-Beneath <file>] [-Moves <file>] [-SrcLog <file>]
zt-digest call -Tool <name> [-Json '<arguments object>']
zt-digest open-call ...   (the same forms, in the open scope)
pwsh -NoProfile -File skills/zt/scripts/zt-mcp.ps1 [open-call] <File|-> [-From N] [-Addresses a0,a1,...] [-Log <file>] [-Allow <file>] [-Beneath <file>] [-Moves <file>] [-SrcLog <file>]
pwsh -NoProfile -File skills/zt/scripts/zt-mcp.ps1 -Tool <name> [-Json '<arguments object>']
```

| Parameter | Set | Default | Effect |
|---|---|---|---|
| `File` (position 0) | File | required | UTF-8 JSON file with one call or an array of calls; `-` reads the same JSON from stdin. |
| `-From` | File | `0` | Skip calls before `N`; used to resume after a failure. |
| `-Addresses` | File | `''` | Comma-separated addresses of the skipped calls, so `@@N` still resolves after `-From`. |
| `-Log` | File | `''` | Append the address of every Card a `create*` call makes, one per line. |
| `-Allow` | File | `''` | Allow-list file. Refuses `create_topic`, `move_card`, `insert_card`, `delete_card`, `create_literature`, `edit_literature_metadata`. `edit_card`, `append_card`, and `create_successor` must target a listed address or a Card created earlier in the run; a `create_successor` with `before` is refused. Each `apply_batch` item is checked as its own call (zt-digest 0.37.12). Read tools and batch previews pass. |
| `-Moves` | File | `''` | Append `{"i":N,"moved":{"old":"new",...}}` for each call whose result has a non-empty `moved` map. |
| `-SrcLog` | File | `''` | Append `<address><TAB><src>` for each created Card whose call carried `src`. |
| `open-call` | command | — | Not a switch: `zt-digest open-call` (or `zt-mcp.ps1 open-call`) is the same helper in the open scope, for hand work on Topic trees. `call -Open` fails and names it. |
| `-Tool` | Inline | required | One tool name. |
| `-Json` | Inline | `'{}'` | That tool's arguments as a JSON object. |

The inline set has no `-Log`, `-Allow`, or other options.

Call objects:

```json
[{"tool": "create_successor", "arguments": {"at": "Key", "title": "...", "body": "...", "src": "sections/s05.md:120-160"}},
 {"tool": "create_successor", "arguments": {"at": "@@0", "kind": "side", "title": "...", "body": "... [[@@0]] ..."}},
 {"tool": "append_card", "arguments": {"at": "3/2", "text": "对照 [[5/1]]：……"}}]
```

- `tool`: any tool `zt mcp` lists, or the helper-only `append_card` (`at`, `text`): the helper reads the Card with `get_card`, then calls `edit_card` with `body` = the old body (trailing space trimmed), a blank line, and the trimmed `text`.
- `arguments.src`: a create call's source lines; removed before the call reaches zt. The server's input schemas set `additionalProperties: false` (`src/mcp.rs`, `fn tool`), so a stray key would be refused.

Output, one line per call, on stdout:

| Line | When |
|---|---|
| `N ok <address>` | The result has an `address` and the tool name starts with `create`, `edit`, `append`, `move`, `insert`, or `delete`. |
| `N <compact json>` | Any other result (reads, and plans that still need `confirm`). |
| `N error <message>` | A JSON-RPC error, an `isError` result (its `error` field), an unresolved `@@N`, or an allow-list or Topic-gate refusal. Newlines in the message become spaces. |
| `resume: -From N -Addresses a0,a1,...` | After an error line; the exit code is 1. |

A call outside the Write scope prints `N error scope_refused: <the server's sentence>`, the line `scope_refused {...}` (with `tool`, `at`, `scope`, `reason`) and the resume line, and exits 1; the calls before it have already run.

### Installer

```powershell
pwsh -File scripts/install-agent-integrations.ps1 [-ZtPath <path to zt.exe>]
```

- `-ZtPath`: defaults to `(Get-Command zt).Source`. The script throws `zt is not installed; run cargo install --path . --locked ...` when it is missing, then resolves it to a full path and prints `zt binary: <path> (<zt version>)`.
- It needs PowerShell 7.5 (`#Requires -Version 7.5`).
- Errors: `<link> is a real directory, not a link; move it aside and re-run.` (a non-link `~/.dsh/skills/<name>`); `codex mcp add failed`; `zt mcp did not answer the handshake`.
- Warnings, not errors: `~/.dsh does not exist; skipped the DeepSeek Harness skills.`, `codex is not on PATH; skipped the Codex registration.`

## Behaviour

### Installing the integrations (`scripts/install-agent-integrations.ps1`)

1. Resolve `$repo` (the parent of `scripts/`), `$skillsSource = <repo>/skills`, and `$ZtPath`.
2. **Skills.** For each directory under `skills/` (today `zt`, `zettelkasten`, and `zt-digest-protocol`):
   1. For `~/.claude/skills` and `~/.codex/skills`: create the folder, delete `<folder>/<name>` recursively if it exists, and `Copy-Item -Recurse` the repository folder there.
   2. If `~/.dsh` exists: create `~/.dsh/skills`; if `~/.dsh/skills/<name>` exists and is not a link, throw; otherwise delete the old link and create a directory junction `~/.dsh/skills/<name>` → `~/.codex/skills/<name>`.
3. **Claude Code, user scope.** Back up `~/.claude.json` to `~/.claude.json.zt-backup` (if it exists), read it, and set `mcpServers.zt` to `{"type":"stdio","command":"<ZtPath>","args":["mcp"],"env":{}}`. If `projects` has an entry whose key equals the repository path with forward slashes (case-insensitive), add `zt` to its `enabledMcpjsonServers` and remove it from `disabledMcpjsonServers`. That approves the repository's own `.mcp.json`, which names the same server with `"command": "zt"`. Write the file atomically.
4. **Claude Code permissions.** Back up `~/.claude/settings.json`, add `mcp__zt` to `permissions.allow` (once), and write it atomically. `mcp__zt` allows every tool of the server without a prompt; the server itself limits writes to the agent Workspace.
5. **Codex.** If `codex` is on PATH: back up `~/.codex/config.toml`, run `codex mcp remove zt` (output discarded), then `codex mcp add zt -- <ZtPath> mcp`; throw on a non-zero exit.
6. **Handshake check.** Pipe three lines into `<ZtPath> mcp`: `initialize` (id 1, protocol `2025-06-18`), `notifications/initialized`, and `tools/list` (id 2). Parse every reply line; throw unless the reply with id 2 has tool names. Print `zt mcp answers with <count> tools: <names>`.
7. Print `Done. Start a new Claude Code or Codex conversation to use the server.`

DeepSeek Harness gets the skills only (through the junctions); it gets no MCP registration, so DeepSeek runs reach zt through the helper.

### Running a call batch (`zt-digest call`, through `zt-mcp.ps1`)

1. Before anything else, `zt-digest` puts the process in the Literature scope (`literature_scope` in `tools/zt-digest/src/main.rs`; `open-call` keeps the open scope, and `call -Open` fails naming `open-call`). Parse the input: the file, stdin (`-`), or `-Tool`/`-Json`, as JSON (a BOM is ignored). Start `zt mcp` with the scope and the `-Allow`/`-Beneath` lists.
2. Find `zt` on PATH and start `zt mcp` with UTF-8 stdin and stdout.
3. Send `initialize` (id 0), wait for its reply, send `notifications/initialized`.
4. Fill the resolved-address list for the skipped calls from `-Addresses`.
5. For each call from `-From` on:
   1. `Resolve-Refs` replaces every `@@N` in every string, recursively through objects and arrays. A reference to a call with no address fails the call.
   2. With `-Allow`: refuse the structural tools; refuse `edit_card`, `append_card`, or `create_successor` whose `at` is not allowed, and a `create_successor` with `before`; check each item of an `apply_batch` the same way (`allow_refusal` in `tools/zt-digest/src/calls.rs`).
   3. `append_card`: `get_card`, then rewrite the call as `edit_card` with the appended body.
   4. Remove `src` from the arguments and keep it for `-SrcLog`.
   5. Topic titles: the server refuses an agent Topic whose title another agent Topic has, with ``topic exists: `<root>` <title>. File the Card under that Topic instead of creating one.`` and the typed `topic_exists` line.
   6. Send `tools/call` with the next id and wait for that reply; a write outside the Write scope comes back as the server's `scope_refused`.
   7. On an error, print the error and resume lines and exit 1.
   8. Record the result's `address` (or null). For a `create*` call with an address: add it to the allow list, append it to `-Log`, and append `address<TAB>src` to `-SrcLog`. Append a non-empty `moved` map to `-Moves`.
   9. Print the `ok` line or the compact JSON line.
6. Close the server's stdin and wait for it to exit.

### Rules the code keeps

- **One server per run.** Each helper run starts its own `zt mcp` child, so calls within a run share nothing but the archive. The server checks that the service is up on each call (`archive_root` in `src/mcp.rs`), so the service must be running (`zt up`).
- **Stop at the first error.** Calls after a failed call never run. The resume line lets a caller fix call `N` in the same input and continue; earlier addresses still resolve.
- **A plan is not an error.** `delete_card`, `move_card`, `insert_card`, and `edit_literature_metadata` without `confirm` return `{"applied": false, "confirm": ..., "plan": [...]}` (`plan_or_done` in `src/mcp.rs`); the helper prints it as `N <json>` and goes on. A batch that must change something carries the `confirm` text.
- **Created Cards join the allow list**, so a run under `-Allow` may grow and edit the Cards it made. An allow file that lists no Card makes a run read-only, because every create and edit then needs a listed `at`; `model-compiler.ps1` gives its brief jobs `read-only.allow.txt` for this.
- **Literature scope by default.** `call` can write only Literature trees, the same scope as every digest step, and the server enforces it; hand work on a Topic tree runs `open-call`, which no digest job's approval allows.
- **The server serializes Topic creation.** The Topic title rule runs under the edit lock, so two helpers that create the same Topic at once make it once.
- **Addresses move.** After `insert_card`, `move_card`, or a `delete_card` that shifts siblings, an address read before may name another Card. The helper does not rewrite later calls; callers use `-Moves` or read again.
- **The installer replaces only what it owns**: the `<name>` folders it copies, the junctions it made, `mcpServers.zt`, the `zt` entries in the two `*McpjsonServers` lists, `mcp__zt` in `permissions.allow`, and Codex's `zt` server. It refuses to delete a real directory under `~/.dsh/skills`.
- **Open conversations keep their old server.** A new MCP tool or a changed skill reaches only conversations started after the installer runs (`skills/zt/SKILL.md`, the installer's help text).

## Data and storage

The skills keep no data. The helper and the installer read and write these files.

### Installed copies and links

| Path | Kind | Written by |
|---|---|---|
| `~/.claude/skills/<name>/` | full copy of `skills/<name>/` | installer, replaced on each run |
| `~/.codex/skills/<name>/` | full copy of `skills/<name>/` | installer, replaced on each run |
| `~/.dsh/skills/<name>` | directory junction to `~/.codex/skills/<name>` | installer, if `~/.dsh` exists |

### Config entries

`~/.claude.json` after the installer:

```json
{
  "mcpServers": {
    "zt": { "type": "stdio", "command": "C:\\Users\\<you>\\.cargo\\bin\\zt.exe", "args": ["mcp"], "env": {} }
  },
  "projects": {
    "E:/zet": { "enabledMcpjsonServers": ["zt"], "disabledMcpjsonServers": [] }
  }
}
```

`~/.claude/settings.json`: `"permissions": { "allow": [..., "mcp__zt"] }`. `~/.codex/config.toml`: whatever `codex mcp add zt -- <ZtPath> mcp` writes. The repository's `.mcp.json` declares the same server for the project scope: `{"mcpServers":{"zt":{"type":"stdio","command":"zt","args":["mcp"]}}}`.

Backups: `<file>.zt-backup` next to `~/.claude.json`, `~/.claude/settings.json`, and `~/.codex/config.toml`. Temp files: `<file>.zt-tmp` (moved over the file after a parse check). The JSON is rewritten by `ConvertTo-Json -Depth 100`, so the installer reformats `~/.claude.json` and `~/.claude/settings.json`.

### Helper files

| File | Format | Example line |
|---|---|---|
| input (`File` or stdin) | UTF-8 JSON, one call object or an array | `[{"tool":"get_card","arguments":{"at":"3/1"}}]` |
| `-Allow` | one address per line, trimmed, blank lines ignored | `Smith2024/1\|a` |
| `-Log` | one created address per line | `3/2\|a` |
| `-SrcLog` | `address<TAB>src` per created Card | `3/2\|a<TAB>sections/s02.md:10-20` |
| `-Moves` | one compact JSON object per renumbering call | `{"i":2,"moved":{"3/2":"3/3","3/2\|a":"3/3\|a"}}` |

All appends use UTF-8 without a BOM.

### MCP wire format the helper uses

Requests are one compact JSON-RPC 2.0 line each:

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"create_successor","arguments":{"at":"3/1","title":"...","body":"..."}}}
```

A reply's `result.content[0].text` holds pretty JSON; `result.structuredContent` holds the same object, from which the helper reads `address` and `moved`; `result.isError` marks a tool failure whose text is `{"error": "..."}` (`tool_result` in `src/mcp.rs`).

## Configuration

| Setting | Where | Default | Effect |
|---|---|---|---|
| `-ZtPath` | installer parameter | `zt` on PATH | The binary registered in Claude Code and Codex and used for the handshake. Re-run the installer after moving the binary. |
| `~/.dsh` exists | file system | — | Enables the DeepSeek Harness junctions. |
| `codex` on PATH | environment | — | Enables the Codex registration. |
| `zt` on PATH | environment | — | The helper starts `zt mcp` from it; the tests prepend a scratch `bin` to PATH to test another `zt.exe`. |
| `ZT_CONFIG_DIR` | environment, read by `zt` | the user's config | Points the `zt mcp` child (and `zt up`) at another archive; the helper tests use a scratch one. |
| `ZT_GUI_PORT` | environment, read by `zt` | — | The helper-gate test sets `0`, so its scratch service does not take the live GUI port. |
| `open-call` | helper command | — (`call` is the Literature scope) | Lets the run write Topic trees, Hub Cards and Topic Links. |
| Skill trigger text | `description` in each `SKILL.md` | — | Decides when a harness loads the skill. |
| Codex display names | `skills/*/agents/openai.yaml` | — | `display_name` and `short_description` shown by Codex. |

## Usage

### Install or update the integrations

```powershell
cargo install --path . --locked     # zt must be installed first
pwsh -File scripts/install-agent-integrations.ps1
```

Re-run it after any change under `skills/`, after a change to the server's tools, or after moving `zt.exe`. Then start a new conversation. To verify a rollout, compare the skill hashes of the repository, Claude Code, Codex, and DeepSeek Harness copies, and check the guidance in a new conversation. A conversation that existed before can keep its old instructions.

### Call the server from a shell (DeepSeek Harness, or any harness without `zt` tools)

```powershell
# one read inline (the skill's form; pwsh -NoProfile -File ~/.codex/skills/zt/scripts/zt-mcp.ps1 with the same arguments forwards to it)
zt-digest call -Tool get_card -Json '{"at":"3/1"}'

# a chain of Literature writes from stdin, with logs (the digest prompts' form; the Literature scope)
@'
[{"tool":"create_successor","arguments":{"at":"Smith2024/7","kind":"side","title":"所有者决定空白处的用法","body":"正文。","src":"sections/s02.md:10-20"}},
 {"tool":"create_successor","arguments":{"at":"@@0","kind":"direct","title":"……","body":"依赖 [[@@0]]：……"}}]
'@ | zt-digest call - -Log owned.txt -SrcLog src.txt

# hand work on a Topic tree: open-call (the server refuses a Topic title another agent Topic has)
@'
[{"tool":"create_topic","arguments":{"title":"剩余控制权","body":"本主题追问：谁决定契约没写的事。"}},
 {"tool":"create_successor","arguments":{"at":"@@0","title":"所有者决定空白处的用法","body":"正文。"}}]
'@ | zt-digest open-call -
```

Output of the second run:

```text
0 ok 7/0
1 ok 7/1
```

After `1 error <reason>` and `resume: -From 1 -Addresses 7/0`, fix call 1 in the same input and run again with `-From 1 -Addresses 7/0`. With `call` instead of `open-call` the second run stops at call 0 with the server's `scope_refused` and writes nothing.

### Call it from a digest script

Digest scripts dot-source `scripts/book-digest/zt-calls.ps1` and call `Invoke-Zt`, which pipes the calls to the repository copy of the helper and returns each read result as an object (it ignores `ok` lines and throws on `error` lines). See [Digest scripts](digest-scripts.md).

```powershell
. (Join-Path $PSScriptRoot 'zt-calls.ps1')
$card = Invoke-Zt @(@{ tool = 'get_card'; arguments = @{ at = '3/1' } })
```

### Add a skill

1. Create `skills/<name>/SKILL.md` with frontmatter `name` and a `description` that says what it teaches and when to use it; quote the description if it contains `: `.
2. Optionally add `skills/<name>/agents/openai.yaml` with `interface.display_name` and `interface.short_description`, and `references/*.md` for long material the body links.
3. Run the installer; it installs every directory under `skills/` with no list to edit.

### Change a tool, then the skills

When `src/mcp.rs` gains or changes a tool or argument, change in the same commit: the tool's description and schema and `INSTRUCTIONS` in `src/mcp.rs`, the tool list in `skills/zt/SKILL.md`, the writing rules in `skills/zettelkasten/SKILL.md` if the convention changes, `CONTEXT.md`, and the READMEs. Commits `753bd7b` (Finding Cards), `82eac86` (Hub Cards), and `7a878da` (insert, build report) change all of these together. Then rebuild and deploy zt, and re-run the installer.

## Testing

No test runs the installer. `scripts/book-digest/tests/agent-lock-guidance.tests.ps1` (S17, #140) reads the two skill texts and a real `zt mcp` handshake's `instructions`, so the Lock policy they must state is checked instead of assumed. The helper is tested through scratch archives (each test sets `ZT_CONFIG_DIR`, runs its own `zt up`, and stops it); `zt` must be installed and on PATH. Run from PowerShell:

```powershell
pwsh -NoProfile -File scripts\book-digest\tests\zt-helper-gate.tests.ps1 [-Zt <zt.exe>]
pwsh -NoProfile -File scripts\book-digest\tests\zt-calls.tests.ps1
pwsh -NoProfile -File scripts\book-digest\tests\card-compiler.tests.ps1
pwsh -NoProfile -File scripts\book-digest\tests\agent-lock-guidance.tests.ps1 [-Zt <zt.exe>]
cargo test mcp::tests
node --test skills/zt-model-import/scripts/imports.test.mjs skills/zt-model-import/scripts/explain.test.mjs skills/zt-model-restructure/scripts/tree-mover.test.mjs skills/zt-model-fold/scripts/mask-links.test.mjs skills/zt-model-fold/scripts/model-survey.test.mjs skills/zt-model-restructure/scripts/check-network.test.mjs skills/zt-model-restructure/scripts/check-premises.test.mjs skills/zt-model-settings/scripts/check-settings.test.mjs skills/zt-literature-organize/scripts/batch.test.mjs skills/zt-literature-organize/scripts/leads.test.mjs skills/zt-literature-organize/scripts/check.test.mjs skills/zt-literature-organize/scripts/outline.test.mjs
```

| Test | Covers |
|---|---|
| `scripts/book-digest/tests/agent-lock-guidance.tests.ps1` | The owner's Lock policy as the agents receive it: the `initialize` answer of a real `zt mcp` and both skill texts say a locked Card is readable and linkable, new thought belongs in another unlocked tree, only the owner changes Locks, and `card_locked` is never retried; none names a Lock route, a token or a second write path. |
| `scripts/book-digest/tests/zt-helper-gate.tests.ps1` | `src` never reaches zt and goes to `-SrcLog`; the server's Topic title rule refuses the same title (spaces and punctuation changed) with the Topic's address, the typed `topic_exists` line and a resume line; a different title passes; `-TopicGate` is an unknown parameter; `call` meets the server's `scope_refused` for `create_topic`, a Card in a Topic tree and a Link to a Topic root and still writes a Literature tree; `call -Open` names `open-call`; two helpers at once create one Topic. `-Zt` tests another binary. |
| `scripts/book-digest/tests/requires-version.tests.ps1` | Every script under `scripts/` and `skills/` that uses `-DateKind` or dot-sources `zt-calls.ps1` declares `#Requires -Version 7.5`. |
| `scripts/book-digest/tests/zt-calls.tests.ps1` | A batch of `create_topic`, `create_literature`, and `create_successor` with `@@` chaining through the helper (`open-call`); `list-models.ps1` reads the helper's UTF-8 output under a GBK (936) console. |
| `scripts/book-digest/tests/card-compiler.tests.ps1` | The card compiler, whose orphan Links use `append_card`. |
| `skills/zt-model-restructure/scripts/check-network.test.mjs` | The network check with the repo's `src/gui/layout.js` (`ZT_LAYOUT`): each block's parent, a forward Link with its one move per Card, an unanchored chapter, a missing and a short 直觉 lead, chapters with no result as advice only, the tree lines, a clean spine passing, and a shared premise block (`kind: shared`) before the setting that a chapter rests on without a forward fault. `mask-links.test.mjs` covers masking Links to Model parts. `check-premises.test.mjs`: a card naming an equation a later card derives is a hit even when linked; pointer sections, the card's own proof, the notation card and prose that only uses an equation are not. |
| `cargo test mcp::tests` (`src/mcp.rs`) | The server side the helper relies on: the handshake and protocol fallback, error codes, unique tool names with object schemas, no new trees in the human Workspace. |

Gaps: `-Allow` has unit tests only (`allow_refusal` in `tools/zt-digest/src/calls.rs`); no test covers `-Moves`, `-From`/`-Addresses` resume, or `-Log`; nothing checks that the skill texts name only tools and arguments that `tool_definitions()` has; the installer is checked only by its own handshake step when run.

## Decisions and pitfalls

### Decisions

- **The server enforces, the skills teach.** The MCP server limits writes to the agent Workspace and keeps every Card rule (`src/mcp.rs` module comment; `docs/workspaces-and-mcp-design.md`). Body conventions are not validated by ZT: the skills teach them and `zt-check` checks them (`docs/event-and-model-cards-design.md`, `docs/finding-cards-design.md`). The hub rules live in `skills/zettelkasten/SKILL.md` (`docs/hub-cards-design.md`).
- **One source, copied out.** The repository is the only source of the skills; the installer copies them rather than linking, so a harness never sees a half-edited working tree. DeepSeek Harness links to the Codex copy, so one copy serves both.
- **A shell helper instead of a second server.** DeepSeek Harness has no `zt` tools, so the helper (`zt-digest call`, also reached through `zt-mcp.ps1`) speaks MCP to the same `zt mcp` with the same agent Access. The `zt` shell commands are never a fallback because they act with human Access (`skills/zt/SKILL.md`).
- **Ownership by allow list, enforced by the server.** Parallel digest runs get `-Allow` so a DeepSeek run can change only its own Cards; the server holds the list for the connection (`--allow`), `-Log` records what each run made, and the Topic title rule stops two runs making one Topic.
- **The helper is Literature-only unless asked.** A digest builds only the book's Literature tree, and most helper runs are digest jobs, so `call` is the Literature scope; a hand run that writes a Topic tree runs `open-call` (`docs/literature-only-digest-design.md`, `docs/agent-write-scope-proposal.md`). `zt-idea-grow` writes its Topic trees through its own open tools (`zt-digest grow`, `hub`, `register`).
- **Skills are sent, not pasted.** Digest managers pass `zettelkasten` and `asd-ste100-zh` in the `skills` field of `deepseek_run` and every `deepseek_prompt`; no prompt file carries skill text. Where an instruction file's limits differ from a skill, the instruction file wins, because `zt-check` checks those numbers (`docs/zt-digest-workflow.md`, *Skills for DeepSeek*).

### Keeping the skill text consistent

- `src/mcp.rs` holds three descriptions of the same behaviour: `INSTRUCTIONS` (sent on `initialize`), each tool's `description` and schema in `tool_definitions()`, and the skill `skills/zt/SKILL.md`. Change all three in one commit, together with `CONTEXT.md`, as the feature commits do. Nothing tests that they agree.
- `skills/zettelkasten/SKILL.md` states rules that `zt-check` checks (the Card language table, F1–F4). A new or changed check rule changes the skill text in the same commit; [zt-check](zt-check.md) owns the rule code.
- `skills/zt-digest-protocol/SKILL.md` and `references/scripts.md` restate the digest scripts, their steps, and the SELF-CHECK commands. A new script, step, or flag in `scripts/book-digest/` changes these tables. The skill text has not followed the Literature-only base 5.0.0 yet: it still shows the `concepts` and `hubs` stages, mode `hubs`, the hub and `registerPlan` args, the `concepts`/`hub`/`register`/`restate` compilers, and a report with `concepts`, `review` and `register` facts, none of which the base runs now (see [Digest workflows](digest-workflows.md)).
- `skills/zt/scripts/zt-mcp.ps1` exists twice at run time: the repository copy (used by `zt-calls.ps1` and the tests) and the installed copy at `~/.codex/skills/zt/scripts/zt-mcp.ps1` (`$installedHelper` in `prepare-book-digest.ps1`, `prepare-link-pass.ps1`, `event-compiler.ps1`, `finding-compiler.ps1`). Both copies only forward to `zt-digest call`, so the helper's behaviour changes when `scripts/install-zt-digest.ps1` installs a new `zt-digest.exe`, not when this installer runs. The skill texts, the DeepSeek prompts, and the workflows name `zt-digest call` directly; the job files of the PowerShell scripts still name the installed `zt-mcp.ps1`.
- After any of these changes, re-run the installer and start a new conversation.

### Pitfalls

- **The installer deletes the installed folders.** Each run removes `~/.claude/skills/<name>` and `~/.codex/skills/<name>` before copying, so local edits there are lost. It never removes a skill that was deleted from `skills/`.
- **One backup slot.** `Backup` overwrites `<file>.zt-backup` on every run, so a second run replaces the original with the first run's output.
- **The project approval needs an existing entry.** The `.mcp.json` approval is written only when `~/.claude.json` already has a `projects` key for this repository (the repository path with `/`); on a fresh machine open the project in Claude Code once and re-run.
- **All three skills are installed.** The loop takes every directory under `skills/`, but the installer's help text and `README.md` name only `zt` and `zettelkasten`, and `skills/zt/SKILL.md` asks for a re-run only after changes to `skills/zt/` and `skills/zettelkasten/`.
- **PowerShell version.** The installer calls `ConvertFrom-Json -DateKind String`, a parameter that PowerShell 7.5 added, so it declares `#Requires -Version 7.5`; `zt-mcp.ps1` keeps the same declaration although it now only forwards; an older pwsh refuses them at the start. `scripts/book-digest/tests/requires-version.tests.ps1` checks every script that needs 7.5.
- **`-Allow` refuses more than its help text says.** The code also refuses `create_literature` and `edit_literature_metadata`; the comment lists only `create_topic`, `move_card`, `insert_card`, and `delete_card`.
- **Hand Topic work runs `open-call`.** With `call`, a run that creates a Topic, writes a Topic-tree or Hub Card, or links a Topic-tree Card stops at that call with `scope_refused`, a resume line and exit 1; the calls before it are already written. Re-run the remaining calls with `open-call` (and `-From`/`-Addresses` for the earlier addresses).
- **`@@N` counts calls, not writes.** A read call takes an index too; `@@N` of a call whose result has no `address` throws `@@N has no address yet`.
- **Inline mode has no options.** `-Tool`/`-Json` is its own parameter set; use a file or stdin for `-Allow`, `-Log`, and the rest.
- **Write the batch as UTF-8.** The helper reads files as UTF-8 and sets the console to UTF-8; a caller under a GBK console must decode the output as UTF-8 (`zt-calls.ps1` sets `[Console]::OutputEncoding` for this; `zt-calls.tests.ps1` guards it).

## Change history
- `2026-11-10` — the setting intuition. `zt-model-settings`: a setting card's format is **起点**, **设定**/**假设**, an optional **直觉** (是什么, 为什么需要, 怎样进入模型), an optional **说明**; `check-settings.mjs` `format` accepts the section only in that place, and the SKILL.md says a pass keeps it with the content it explains. Test: `check-settings.test.mjs` (22). The global skill `zt-model-illustrate` (outside this repository) writes the section too: `scripts/intuition.mjs`, `apply.mjs` places a module's `intuitions`, `verify.mjs` reports a setting card without the three lines; brandimarti2026reorganizing/13|a|a has it on its 16 setting cards and its 3 定义 parts on the live box. The digest writes the same section since zt-digest 0.52.0 ([zt-digest](zt-digest.md)).
- `2026-11-10` — the post-digest passes on becker2025division and hansen2021demand after their appendix extensions (proposal §14.16); the post-digest sequence now ends with `zt-literature-organize` (fold → settings → restructure → import → evidence dialogue → literature organize; `zt-model-import`, `zt-evidence-dialogue` and `zt-literature-organize` SKILL.md name it). `check-settings.mjs` `tokens`: an environment name and an array column spec (`egin{array}{l l}`, `end{array}`) are no symbols; they gave uncovered $a$, $l$ on becker2025division/3|a|a|g|b|a. `zt-literature-organize` `outline.mjs`: the walk is the exported `outline(full, key, maxDepth)`, and a whole unit on the top line (a Term list that is not last) passes the walk on to its Direct successor; Becker's Term list at `/8` had hidden groups 10–27. Tests: `check-settings.test.mjs` (21), `outline.test.mjs` (1).
- `2026-11-10` — the organize pass on brandimarti2026reorganizing (proposal §14.14). `zt-literature-organize` `check.mjs`: whole units that share one type and title (the same specification in two empirical models) are compared as a set of sizes (`unitChanges`, exported; the CLI runs only as the main module), so a move that refiles them is no fault. Test: `check.test.mjs` (2).
- `2026-11-10` — the appendix rework of brandimarti2026reorganizing's models (proposal §14.13). `check-settings.mjs`: a definition written with `:=` (`记 $\sigma_i:=…$`) names the symbol before `:=`, not `\sigma_i:` (`symbolOf` splits at `:=` too); the colon gave false `notation` faults on 15|a|a|3|f|a. `imports.mjs plan`: only this model's own parts decide whether an imported result is the prediction under test (`tested` reads rests-on Links from `own` parts); 7|e imports corollary A.5(i) and A.5(ii), and A.5(ii) rests on A.5(i) in its home model, which had proposed a `整块` upgrade of A.5(i). Tests: `check-settings.test.mjs` (20), `imports.test.mjs` (8).
- `2026-11-10` — the model passes on acemoglu2001colonial and dell2010persistent (proposal §14.10). `check-settings.mjs`: a bare notation symbol is not met inside a longer row symbol of the same card (`usesSymbol(body, sym, longer)`: $\beta$ inside the row $\beta_R$ gave a false `first` fault on acemoglu2001colonial/5|a), in the `first` check and the `--symbols` reading aid; `auditEmpirical` skips a card another model lends (build line `shared`): acemoglu2001colonial/5|b imports 5|a's identification block whole, and its 假设 1 was called an assumption outside 5|b's design. `dialogue.mjs survey`: a Term that at least half of a model's estimates link (three estimates or more; `frameTerms`) is the model's reporting frame (标准误, 设定, 点估计) and forms no pair, on either side; dell2010persistent's surveys fell from 1,105 pairs, nearly all on 标准误 alone, to 129. Tests: `check-settings.test.mjs` (19), `dialogue.test.mjs` (13).
- `2026-11-09` — the model passes on the six live empirical models (proposal §14.8). `check-settings.mjs` (`auditEmpirical`) adds `start` (a sample, measure or descriptive card of the data tree with no **起点**), `order` (a data-tree card that rests on a later one: hansen2021demand/5|c|e|a|d rested on its later sibling |a|g) and makes `assumption` require a Side successor of the design card (ebrill2001modern/5|a: 假设 2 stood as the Direct successor of 假设 1). `imports.mjs plan` proposes no `整块` upgrade for a `单卡` import that only 假说 parts of an empirical model rest on: a test imports the prediction, not its proof (§7, §9.4). `dialogue.mjs`: `survey` removes the message files of an earlier survey (`writeMessages`), and `check` reads no links when no apply ran (`readApplied`; a model with no pair failed on a missing applied.json). Tests: `check-settings.test.mjs` (17), `imports.test.mjs` (7), `dialogue.test.mjs` (12).
- `2026-11-09` — live P6 checks. `check-settings.mjs` (`auditEmpirical`): a Greek letter written in prose (σ) uses every 变量表 row built on it (`$\sigma$`, `$\hat \theta$`); becker2025division/6|g's rows were called unused. `check-network.mjs`: an empirical model's 数据 block is its primitive and is no unanchored chapter (ebrill2001modern/4|d|a). Tests: the Greek-letter case in `check-settings.test.mjs`, the data-block case in `check-network.test.mjs`.
- `2026-11-09` — pilot fixes on the copied box (ebrill2001modern/5|a, 4|d|a, hansen2021demand/5|c|e). `zt-evidence-dialogue`: the estimand Terms are the head's title, **问题** and **估计对象** (a Term is linked only where a Card first names it, so **估计对象** alone held none and no survey ever proposed a pair); an estimate pairs on the Terms of what it measures and falls back to the estimand Terms; the other side comes from `get_tree` of every Literature tree (search_cards stops at 200 rows, and 5|a and 5|c|e were outside it) with each other estimate's head Terms (the two ebrill models paired one way only); a judged record per box (`~/.zt-evidence-dialogue/judged-<config>.jsonl`) keeps every answered pair from being judged again (the mirror survey of 4|d|a fell from 27 pairs to 0); a same-estimand `not-comparable` pair is written with 对照 (`linksFromAnswers` dropped it); `zt.mjs cards` reads 40 batches a process. Measured: 54 pairs, 5 messages, 0.125 CNY. Tests: `dialogue.test.mjs` (11). `zt-model-restructure`: `check-network.mjs` gives an empirical model no "no result card" advice (its results are 估计 cards, which the board does not count yet) and sets `process.exitCode` instead of `process.exit`, which aborted with a libuv assertion (exit 127) on Node 25 on every model. Test: the empirical case in `check-network.test.mjs`.
- `2026-11-08` — empirical models: `zt-model-fold` (a theory and its test are two models; `model-survey.mjs` flags `levels mixed`), `zt-model-settings` (the data tree and the specification ladder; `check-settings.mjs` runs `auditEmpirical` for level 3), `zt-model-restructure` (the evidence spine), `zt-model-import` (hypotheses, data, theories) and the new `zt-evidence-dialogue` (`scripts/dialogue.mjs survey | apply | check`, a DeepSeek judge in messages of twelve pairs, Links in agent Cards only). Tests: `dialogue.test.mjs`, the new cases in `model-survey.test.mjs` and `check-settings.test.mjs`.
- `2026-11-08` — `zt-literature-organize`: a `leads.mjs` demote moves each Side child up to the new summary with its own Direct chain (`through`). Before, it moved only the child's head Card and left the chain under the old lead; organizing `kennedy2001armies` and `morony2005iraq` met this, and their 7 stranded chains were moved back by hand. The batch is built by the tested function `demoteItems`; the header says the leads file takes full addresses.
- `2026-11-06` — the folders `skills/zt-term-align` and `skills/zt-term-review` are deleted from the repository; `zt-term-sync` is the only skill that links Terms across books.
- `2026-11-06` — New skill `zt-term-sync` (owner's request): Opus conducts it, a DeepSeek job does the matching, and `zt-digest term-sync` does the rest. It adds a Link between equivalent Terms of different books and changes no Term. It replaces the skills `zt-term-align` and `zt-term-review` and the Workflow `zt-term-align` 1.0.0, none of which ran live. Design: `docs/term-sync-design.md`.
- `2026-11-05` — `zt-digest-protocol` names the official DeepSeek source (`deepseek-official/deepseek-flash`) as the frozen source of the digests, in place of OpenCode Go (`opencode-go/deepseek-v4.1-flash`); the `source` stage still runs no probe.
- `2026-11-05` — the `zt` skill names `set_literature_type`.
- `2026-11-05` — the five Card-writing skills (`zt-literature-organize`, `zt-model-fold`, `zt-model-settings`, `zt-model-restructure`, `zt-model-import`) end on `term-relink.ps1`, because a rewritten Card loses its Term links.
- `2026-11-04` — `zt-digest-protocol` names OpenCode Go as the frozen DeepSeek source of the digests.
- `2026-11-03` — `zt-digest-protocol` names the three installed variants, and its stage graph matches the blueprint: the `cited` stage before the digest, the slot `after-models`, and `term-cards` and `cited-cards` beside the Link stages after `cards-book`.
- `2026-11-03` — `zt-literature-organize` adds summary leads (owner's request): every division with subdivisions opens on a summary of the whole division. `leads.mjs` applies a leads file, one batch per entry, deepest first: `rewrite` keeps the lead with a new text; `demote` creates the summary as a Side Card, moves it into the lead's place (an insert would shift the Term list while the new Card's Term uses are checked), and files the old lead under it. It turns Term names into Term uses and, once every Card has settled, points the summaries' Links and the root's section headings at the final summaries. Applied to all nine agent Literature trees on the live box.
- `2026-11-03` — New skill `zt-literature-organize` (owner's request): reorganizes one Literature tree by its source, for any discipline and form of material, with few format checks. Tried on `atkinson2015lectures` on the live box (7,440 Cards), which met the batch limit of 2,000 renamed Cards and gave the skill its parking method. Installed for Claude Code and Codex, linked for DeepSeek Harness.
- `2026-11-03` — `zt-model-settings` made strict (owner's request): fixed placement (setting card `<head>|a`, notation card its `|a`); one setting card format (**起点**, **设定**/**假设**, **说明**; titles `设定：`/`假设 N：`; no local tables); the notation card covers every setting's symbols, lists only used symbols, and names each symbol's first card; assumptions stated once and cited by Link; balance advice. `check-settings.mjs` reads formulas as TeX tokens and adds `place`, `uncovered`, `unused`, `first`, `format`, `repeat`, `cite` and `balance` (14 tests). Applied to `acemoglu2007equilibrium/2|a` on the live box (model-merge-proposal.md §11.3). Installed for Claude Code and Codex.
- `2026-11-01` — Model Card limits doubled (zt-check rule version 6): `zettelkasten` states the doubled Model Card limits (a first Model Card up to about 1200 words, three to twelve lines per step, twelve steps per Card, a part of about 24 sentences or 800 characters, 80/60-character sentences and 80-character titles); `zt-digest-protocol` gives the full doubled list in section 6; `zt-model-restructure/references/card-formats.md` allows an 80-character chapter title.
- `2026-10-31` — #237 (S9 of PRD #228): the model skills' shared `scripts/zt.mjs` (all four copies) gains `tidy(items, label)`: one reorganization as one `apply_batch` through `zt-digest call`, previewed, then committed under its plan hash; it returns the plan (`moved` in start addresses). `zt-model-fold` sends a head's moves, retype and reorder as one `tidy` batch (atomic, so the retype-and-move trap is gone) and reorders Side cards with `reorder` instead of parking; `zt-model-restructure` sends one pass as one batch with `move_span`, `indent`/`outdent` and `reorder`. Verified on a box copy: the installed fold helper reversed 7 Side branches of `hansen2021demand/1` (154 Cards) in one batch.
- `2026-10-31` — #232, #233 (PRD #228): `skills/zt/SKILL.md` teaches the tree tools (`move_span`, `delete_span`, `indent`, `outdent`, `reorder`, `relink`) and one reorganization as one `apply_batch`: start-state addresses, destinations by Card (`after`, `side_of`, `before`), `plan_hash` confirmation, the 64-item and 2,000-Card limits.
- `2026-10-31` — #227: `skills/zt/SKILL.md` states `scope_refused` (with its reasons) as a planning stop beside `card_locked`, `topic_exists`, and `open-call` for hand Topic work; the model skills' helper facts and `zt-digest-protocol` (SKILL.md, references/scripts.md) state the server's allow rule and Literature scope. Not yet installed into the harnesses; that is the release step.
- `2026-10-31` — #226: the helper declares its Write scope to the server. `zt-mcp.ps1` forwards `open-call …` to `zt-digest open-call`; `call` has no `-Open`; `-TopicGate`/`-TopicSimilarity` are gone (the server's Topic title rule); refusals are the server's typed `scope_refused`/`topic_exists` lines; `append_card` sends a base. `zt-helper-gate.tests.ps1` rewritten for the server's rules.
- `2026-10-31` — The model skills run fold → settings → restructure → import, and `zt-model-import` ends with the explanation refresh (step 8, `scripts/explain.mjs`), so a book needs no run of the digest in mode `explain` after its model passes (owner's request: that run is slow). The `zt` and `zt-model-settings` skills state the new order. Tried end to end on a box copy (`hart1995firms/3|b`, one Derivation part with 思路 and 解读 removed: one DeepSeek job, applied, 0 refused, Term links on 1 Card, second plan 0, nothing left). Also synced from the installed copies: `model-survey.mjs` labels a false share and never joins models on one; `tree-mover.mjs` resumes at the failed move and has `tree-mover.test.mjs`.
- `2026-10-31` — The `zt` skill names the import marks of `build.order` (`import`, `legacy`, `premise`), the `import_target`/`import_cycle` faults, `false_share`, `imports`, `lent` and `blocks`, the retype rule's import exception, and the order of the model skills (fold → import → settings → restructure); the `zettelkasten` skill lists the Model Card's **引入** section before 结构. The release manifest names `zt-model-import`.
- `2026-10-31` — New skill `zt-model-import` (owner's request after #205): `scripts/imports.mjs` `plan` / `apply` / `check` over `get_model` (pure `propose`, `rewrite`, `audit`, tested by `imports.test.mjs`, 6 cases). Checked on the live box read-only (78 models: no import line needed after the #213 migration; 4 possible copies named) and on a box copy end to end (`hart1995firms/15|c` given premises in `14|a|a|5` and a cross-book `acemoglu2010labor` block: check fails on 3 outside premises, plan proposes `单卡` + one `整块` that absorbs, apply writes them, a second plan upgrades the `单卡` (tolerated 3 → 0), then nothing to import). The `zt` and `zettelkasten` skills now teach sharing by `**引入**` (not the 结构 list) and point to the new skill; `zt-model-settings` names it as the step before. Installed for Claude Code and Codex, linked for DeepSeek Harness.
- `2026-10-31` — #211 (I6 of #205): the model skills read **引入** imports as intended structure. `zt-model-fold` `model-survey.mjs` counts an import apart (`imports`, listed to read, never a join) from a legacy （共用） share (`legacy share`, still a join); the fold keeps a folded Model Card's **引入** lines; an import never blocks a retype. `zt-model-restructure`: no "fold first" for an imported block; `check-premises.mjs` leaves legacy shares out of the equation scan (`scan(order, cards, relations, foreign)`). `zt-model-settings` `check-settings.mjs`: an imported notation card counts, `沿用 [[…]] 的全部符号` replaces an own notation card, borrowed Cards are not audited (`notation`, `stated`). Tests: the three scripts' `node --test` files. Installed copies refreshed.
- `2026-10-29` — `check-network.mjs` names the `shared` block (parts shared from other models, before the setting) 共用; a Link that rests on a shared part is no forward fault. Test: the shared-premise case in `check-network.test.mjs`.
- `2026-10-29` — `zt-model-fold` no longer assumes one model per book: it decides how many models a book holds by their nature (the same question and primitives continued, a stage, an extension on the same setting, an estimation or the verbal version are one model; an own question or own primitives, a borrowed result, a comparison are a model of its own), never by the book's size, records a decision table, and folds each model separately. `model-survey.mjs` prints each head's question and setting and proposes candidate groups (connected by rests-on Links and shared parts; kinship and comparison Links are listed to read, never joined), with a test.
- `2026-10-29` — `check-settings.mjs` skips a setting shared from another model (`line: shared`): its home model's audit checks where it is filed (test added). `tree-mover.mjs` saves the next step after each move, so a resume no longer repeats the last move (a resume after a failed move had parked a card into a slot under itself). Found on the Modern VAT formal models (model-merge-proposal.md §12.3).
- `2026-10-29` — the skills' `zt.mjs` returns `zt-digest call` output when the batch exits 1 (one call failed), so `writes()` reports the error lines instead of throwing; `check-settings.mjs` drops TeX spaces when it reads the notation symbols (test added). First settings pass on Acemoglu 2010 (model-merge-proposal.md §10).
- `2026-10-29` — `zt-model-settings`: the setting part is a dependency tree a reader builds from scratch, not a list. Each setting is filed under the setting it builds on and names its premises in a **起点** section; the base model's blocks come first, then one branch per model variant; each symbol is defined where the reader first meets it. `check-settings.mjs` adds the faults `start`, `misfiled`, `order` and the `--symbols` reading aid (test added). The setting card format in `card-formats.md` gains **起点**. Becker rebuilt (model-merge-proposal.md §9.8).
- `2026-10-29` — a third model skill, `zt-model-settings`, takes the settings out of `zt-model-restructure`: every setting, assumption and symbol in the setting part, none in a chapter (stage settings, equilibrium conditions, a chapter's technology definitions included), every derivation symbol in the notation card; `check-settings.mjs` (new, with a test) fails on any exception. The order is fold, settings, restructure; the restructure checks the setting part before and after. Becker: six settings moved out of four chapters (model-merge-proposal.md §9.7).
- `2026-10-29` — the model skill is split in two: `zt-model-fold` (Steps 1–2 of the old skill: survey, fold, verify; input a citation key; `model-survey.mjs` and `mask-links.mjs` moved there) and `zt-model-restructure` (input the Model Card's address; Step 1 reads the model, `check-model.mjs` refuses a part's address; the spine, the cards, the checks). Both carry their own `zt.mjs`; the release manifest lists both.
- `2026-10-28` — `zt-model-restructure`: settings are primitives and stay in the setting part; a setting with a forward Link has a reversed relation (the derivation uses the setting), which is turned the right way, never solved by moving the setting into a chapter; only a derivation filed too early moves. `check-network.mjs` gives that fix for a setting (test updated). Owner correction; Becker repaired accordingly (model-merge-proposal.md §9.6).
- `2026-10-28` — `zt-model-restructure` gains `scripts/check-premises.mjs` (Step 5): the dependencies the Links hide, a card naming an equation a later card derives; each hit is read, a use is fixed by moving. First run on Becker found two (the β̃ redundancy by (2) stated in the setting, a restored start formula (4) in the derivation of (3)), both fixed.
- `2026-10-28` — `zt-model-restructure`: a forward fault is a placement fault, fixed by moving cards (the dependent after its premise, or the premise first, with every card that rests on the moved one), never by deleting a Link or dropping its premise cue; a pointer (a chapter's list of its results, the notation-card line) is written as a pointer from the first draft, decided by meaning, not by the check (owner correction).
- `2026-10-28` — `zt-model-restructure` follows the model network (zt 0.44): the network's rules in Step 3 (a chapter hangs under the block it rests on most; a card rests only on its own block or earlier ones, so a setting that rests on a chapter's proof is that chapter's 本章设定; the 直觉 opening is the block's lead), the network check `scripts/check-network.mjs` in Step 5 (tree, `forward` with its move, `unanchored`, `parts`, `lead`, `advice`; test `check-network.test.mjs`), and the chapter-card rules for the lead and `依赖` in `references/card-formats.md`.
- `2026-10-28` — `zt-model-restructure` added (94dac75) and given the parts section (`**分部**`, #204); the release manifest lists it with the other installable skills.

- `2026-10-25` — S28 (#170): the release guidance. `skills/zt/SKILL.md` gains the `term` Card type, `get_terms`, the Term use `[[label@address]]` and its `term` Relation kind, the `term_links: "label"` projection with raw-as-edit-base, the gate's accepted-warnings/`card_check`/`card_locked` rules, and the release-set deploy sequence; `skills/zettelkasten/SKILL.md` gains "Term Cards and Term uses" (role by name and Location, the exact name and template, an Idea Card for a definition the argument needs, no synonym or relation word, respect `自动链接：否`); `skills/zt-digest-protocol/SKILL.md` and its `references/scripts.md` gain the Term-card branch, the deterministic Term-link pass, both backfill modes, the Term args, T1–T10, `--draft`/exit codes, the gate behavior and the Term SELF-CHECKs, and replace the removed base stages. `scripts/install-agent-integrations.ps1` names the three skills. `scripts/book-digest/tests/release-set.tests.ps1` asserts this guidance; `scripts/install-agent-integrations.ps1` was re-run so the installed copies carry it.
- `2026-10-23` — `zt-digest-protocol` operating rules: watch a run with `scripts/book-digest/watch-run.mjs` (only actionable lines reach the session; repairs go to their own session), and finish claimed units with one `zt-digest finish-batch` command (base 5.2.0; `docs/digest-claude-token-proposal.md` P1, P9, P10).
- `2026-10-22` — Literature-only digests (`docs/literature-only-digest-design.md`): the helper runs in the Literature scope by default. `skills/zt/SKILL.md` (When the server is unavailable) says the helper refuses `create_topic`, writes to Topic-tree and Hub Cards and Links to Topic-tree Cards, as the ZT digests do, and that hand Topic work adds `-Open`; `skills/zt/scripts/zt-mcp.ps1` names `[-Open]` in its synopsis and gains a Scope paragraph (`zt-idea-grow` needs no `-Open`). `zt-helper-gate.tests.ps1` proves the three refusals and the Literature write; it and `zt-calls.tests.ps1` pass `-Open` for their Topic batches. `skills/zt-digest-protocol/SKILL.md` is not yet updated.

- `2026-10-22` — #140 S17 (Tree Lock): both skills carry the owner's Lock rule. `skills/zt/SKILL.md` (Rules) and `skills/zettelkasten/SKILL.md` (Maintenance) say a Card whose read carries `lock` belongs to a locked tree, its Cards stay readable, selectable and linkable, the new thought goes into another unlocked tree, only the owner changes Locks, a Card lock never widens Access, and a `card_locked` refusal is a planning error that is never retried, never answered with a different Access and never worked around through the shell, the GUI API, the archive file or another client's path. No route name or token appears. The installer was run after the change: `~/.claude/skills/{zt,zettelkasten,zt-digest-protocol}`, `~/.codex/skills/*` and the `~/.dsh/skills/*` junctions carry the new text, and the freshly installed `zt mcp` answers the handshake with 29 tools. Open conversations keep the instructions they started with, so a new one is needed for the new metadata.

- `2026-10-22` — #137 (Tree Lock S14): the helper's `skills/zt/scripts/zt-mcp.ps1` documentation names the typed-refusal line (`card_locked <compact JSON>` before `resume:`), the Lock facts it keeps, and that a `card_locked` answer is a destination refusal rather than a provider fault. The skill texts themselves stay with S17 (#140).
- `2026-10-19` — #118 S17 (filing a kept Spread as a hub draft): `skills/zt/SKILL.md` tells an agent that a Spread the human kept from a question can be filed from the GUI's Table as one agent Hub Card draft — the desk opens with the question, the Spread's Links and their reasons, the placement controls choose the hub level and the Card it is filed under, and the precheck must pass before its Save — so the draft is read or finished over MCP like any other agent Card. `skills/zettelkasten/SKILL.md`'s Hub Card rule names the same filing as one way such a draft begins.

- `2026-10-19` - #111 S14 (restatement folding): `skills/zt/SKILL.md` tells an agent that a `search_cards mode: entry` row with `folded` above zero stands for several Cards stating one confirmed statement (an accepted `## 同述入口` entry, or two Idea Cards with an exact duplicate statement), to read `representative` and `members` instead of comparing the row's title with a statement it does not hold, and to count `total` (Cards) apart from `row_total` (rows).
- `2026-10-19` - #93 S13 (entry-first search): the block's code now carries the shared entry ranking used by MCP search and the GUI, with plain search and empty-query listings unchanged.
- `2026-10-17` — Teach `get_cards`, `get_tree` bodies, `export_titles`, explicit read limits, missing-address checks and revision consistency in the ZT skill.

- `2026-10-16` — `skills/zt-digest-protocol` is rebuilt as the protocol of the digest base only: the base stages, slots, modes, and args, the profile contract, the base tools and their SELF-CHECK lines. The variants' layers (models, events, findings, math) and the legacy notes are gone; `references/scripts.md` lists only the base's tools, prompts, and tests.
- `2026-10-16` — `skills/zt-digest-protocol` names the `zt-digest` tools for every stage and every SELF-CHECK command (hub, Bridge, event, finding, skeleton, card, model), with the script each replaced; `figures.ps1` is gone from `references/scripts.md`, and `book-of.ps1` and `relatedness.ps1` are marked as kept for old callers.
- `2026-10-16` — The skills name the helper `zt-digest call`. `skills/zt/SKILL.md` tells an agent with no `zt` tools to run `zt-digest call calls.json`, `@'...'@ | zt-digest call -`, or `zt-digest call -Tool … -Json …` (`scripts/zt-mcp.ps1` forwards). `skills/zt-digest-protocol/SKILL.md` gives the Scripts role to the `zt-digest` tools and the PowerShell scripts not ported yet, and names `zt-digest call` (or zt-mcp.ps1) in the approvals and as the zt helper; `references/scripts.md` lists the ported tools (`call`, `audit`, `card`, `model`, `model-context`, `list-models`).
- `2026-10-16` — `zt-mcp.ps1` forwards to `zt-digest call` ([zt-digest](zt-digest.md)): the helper runs in Rust with the same parameters and output lines; `ZT_DIGEST` names another build.
- `2026-10-16` — `zt-mcp.ps1` carries the line `zt-calls: direct reads`: `scripts/book-digest/zt-calls.ps1` sends batches of read calls straight to one long-lived `zt mcp` and must keep this helper's read output and stop-at-first-error rule. A replacement helper without the line (a test seam) gets every call.

- `2026-10-19` — #115: the `zt-digest-protocol` skill shows the integrated stage order (terms → concepts → digest … cards-book → review → link-prepare … organize → figures → bridge → hubs → audit → finalize) and names the two new compilers (`concepts`, `restate`) plus the register plan its `hubs` stage runs; `references/scripts.md` adds the `zt-digest concepts`, `zt-digest restate` and `zt-digest register` rows with their steps, `-Review`/`-Readings`/`-Merges` inputs, what each writes, and the report-first rule (`args.registerPlan` authorizes a register publication).

- `2026-10-19` — #110: the `zt-digest-protocol` skill names the gap-only Link stage: `prepare-link` computes the receipt-owned semantic gap set, `link-start` freezes it and writes the bound job, a gap-free group starts no provider run, and the Link SELF-CHECK is `zt-digest link-finish -CheckOnly` (binding, gap coverage, `L1,L2`).

- `2026-10-16` — Review fixes: `zt-digest-protocol` shows Bridge between Figures and Hubs in the stage diagram, adds Bridge counts and islands to the Audit row, and states that a Bridge report is published only when every judge job answered and that the cleanup always runs. `zettelkasten/references/luhmann-and-practice.md` cites repository paths instead of github.com URLs and adds Luhmann's 57/12a and `insert_card before`.
- `2026-10-16` — Implement report-first Bridge selection, guarded apply/recovery, math citation direction, semantic audits and workflow adapters; installed and live gates are recorded separately.
- `2026-10-16` — C1: insertion, Placement, and Maintenance agree with `insert_card before`; whole-Card Links and human read-only splitting are explicit. Rollout verifies installed hashes and freshly loaded client guidance.

- `2026-10-16` — `zt-mcp.ps1` declares `#Requires -Version 7.5` (it uses `ConvertFrom-Json -DateKind`).
- `2026-10-16` `753bd7b` — Finding Cards: `finding` type in the `zt` skill, the Finding Card template and F1–F4 in the `zettelkasten` skill, F rules in the digest protocol.
- `2026-10-16` `63d77d1` — Paths in the skills changed from `doc/` to `docs/` (one in `zt-digest-protocol/SKILL.md` was missed).
- `2026-10-15` `02499c0` — `zt-digest-protocol` skill and its script reference added; the `zettelkasten` skill names `zt-check` (not `card-style-check.ps1`) as the checker of the Card language.
- `2026-10-15` `298d4db` — Math forms, units, and Theory Cards in the `zt` and `zettelkasten` skills.
- `2026-10-15` `caf2cdc` — Helper gains `-SrcLog`, `src` stripping, and the Topic gate (`-TopicGate`, `-TopicSimilarity`).
- `2026-10-15` `6b36918` — Document links (`scholar:<file_id>`) and the book's own figures in the skills.
- `2026-10-14` `82eac86` — Hub Cards and hub levels in both skills.
- `2026-10-14` `67b80a9` — Helper gains `-Moves` (the `moved` map of each renumbering call) for the model compiler.
- `2026-10-13` `1bec685`, `7d636e4`, `b61f2c4` — Derivation chains, elaboration levels, and `delete_card` `into` in the skills; one UTF-8 helper call.
- `2026-10-13` `4f0414d`, `2026-10-11` `daea37b` — Deploy notes in the `zt` skill: GUI inside the service, `zt-boot` and autostart.
- `2026-10-12` `7a878da`, `d92f367` — Self-sufficient models, `insert_card`, `move_card` `before`/`alone`, build report, the Card language table (`7a878da`); open models whose parts join by Link and Links that state their kind; `-Allow` refuses `insert_card`, and `insert` results print as `ok` lines.
- `2026-10-11` `ea9a8f0`, `2b05ba6`, `eec0940` — Figures by `artifact:`, author-designed model structure, Model parts, TeX, and HTML charts in the skills.
- `2026-10-10` `fcc5226` — `zettelkasten` skill with its research reference; the shell helper `zt-mcp.ps1` (batches, `@@N`, resume with `-From`/`-Addresses`, `-Log`, `-Allow`, `append_card`); the installer installs every skill folder and adds DeepSeek Harness junctions.
- `2026-10-10` `4e990a0` — Scholar Archive tools in the `zt` skill.
- `2026-10-10` `f3eec44` — `zt` skill, its `agents/openai.yaml`, and the Claude Code and Codex installer.
- `2026-10-16` — Wiki doc created.

