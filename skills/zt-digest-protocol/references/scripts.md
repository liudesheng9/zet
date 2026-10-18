# Tools, prompt files, and tests of the digest base

All paths are under `E:\zet`. "Writes" says whether a tool changes Cards in the slip-box.

Every base stage runs a `zt-digest <tool>` (`tools/zt-digest`, installed beside `zt.exe` by `scripts/install-zt-digest.ps1`). A tool keeps the parameters and output of the PowerShell script it replaced (in brackets); that `.ps1` in `scripts/book-digest/` only forwards to its tool, so an old command still works. The workflow modules build each command with `zdTool` (`ZD_TOOLS` in `zt-digest-lib.js`), and every job file's `SELF-CHECK:` line names the tool. The variants' layers use more tools (`model`, `model-context`, `list-models`, `event`, `finding`, `math`); the variants' blueprints and docs describe them.

## Compilers (the basic block)

| Tool (script) | Stage | Steps | Use it to | Writes |
| --- | --- | --- | --- | --- |
| `zt-digest card` (`card-compiler.ps1`) | digest blocks, link-check, cards-book, Organize notes | `plan`, `precheck`, `apply`, `check`, `report` | Fix the Card language and layer faults of a book's Idea, Event, and Finding Cards and anchors; review a sample against the source; link orphan Cards; carry out Organize notes. DeepSeek writes JSON. Options: `-Book`, `-Staging`, `-Groups`, `-Sample`, `-Rules`, `-MaxCards`, `-NoOrphans`, `-Notes`, `-Compact`, `-MaxJobs`. | yes |
| `zt-digest terms` (`book-terms.ps1`) | terms | `plan`, `apply` | The book term table (术语表) that rule R6 checks against. | the 术语表 Card |
| `zt-digest skeleton` (`book-skeleton.ps1`) | organize | `plan`, `apply` (`-Apply` writes; without it, a dry check) | The Literature Card index, the structure Card, and the Topic bodies. | yes, with `-Apply` |
| `zt-digest hub` (`hub-compiler.ps1`) | hubs, hubs-first | `survey`, `plan`, `precheck`, `apply`, `check`, `report` | The line, topic, and field Hub Cards and the register of the whole agent Workspace. State in `<hubWork>\state.json` (default `D:\zt-digest\hubs`). | yes |
| `zt-digest bridge` (`bridge-compiler.ps1`) | bridge | `plan`, `precheck`, `report`, `apply` | Freeze candidate endpoints, check BR1–BR9 and the Card rules, publish an immutable report, and apply only explicit report-bound ids. Options: `-Staging`, `-CitationKey`, `-Work`, `-Profile`, `-MaxSources`, `-Candidates`, `-Books`, `-Job`, `-ReportId`, `-Accept`, `-AcceptEarlier`, `-Apply`. Without `-Apply` it changes no Card or apply state. | only accepted agent carriers, with `-Apply` |

## Preparation and passes

| Tool (script) | Stage | Use it to | Writes |
| --- | --- | --- | --- |
| `zt-digest prepare` (`prepare-book-digest.ps1`) | prepare | Chinese-only text, section groups (`groups\gNN.md` with their write command and `SELF-CHECK` line), the shared DeepSeek prefixes from `-Instructions`, and with `-Apply` the agent Literature tree with one anchor per group. | anchors |
| `zt-digest snapshot-topics` (`snapshot-topics.ps1`) | digest | A compact list of every Topic tree for one digest run, so it places and links Cards without searching. | no |
| `zt-digest prepare-link` (`prepare-link-pass.ps1`) | link-prepare | A snapshot of every Card; per group, the Cards it owns, the candidate targets, the allow list, and the `SELF-CHECK` line; the groups still pending. | no |
| `zt-digest organize-survey` (`organize-survey.ps1`) | organize | Everything Opus must decide: duplicate and small Topics, merge proposals, model faults, broken Links, missing hubs. Read-only. | no |
| `zt-digest organize-apply` (`organize-apply.ps1`) | organize | Carry out Opus's `decisions.json`: Topic merges and Card moves, logged. | yes |
| `zt-digest figures` (`attach-figures.ps1`) | figures | Show the book's figures (`artifact:`) in the Cards that name them; link the book's PDF and Markdown from the Literature Card. | yes |

## The helper, checks, and audit

| Tool | Use it to | Writes |
| --- | --- | --- |
| `zt-digest call` (the old path `skills/zt/scripts/zt-mcp.ps1` forwards to it) | The zt helper: a JSON array of calls on stdin or in a file, with `-Allow`, `-Log`, `-SrcLog`, `-TopicGate`, `-Moves`, and `-From`/`-Addresses` to resume. DeepSeek writes through it; the tools run it in-process. | as called |
| `zt-check` (`tools/zt-check`, `scripts/install-zt-check.ps1`) | The one checker for every agent: `--cards` (a writer's own Cards, classified as the audit does), `--addresses`, `--model`, `--json` (drafts, offline). Rules R2 to R19 (R18/R19 are the exact repeated-paragraph and repeated-title corruption rules) and I, E, F, L. | no |
| `zt-digest audit` (`digest-audit.ps1`) | Read-only audit of one book or the whole Workspace, over both Workspaces: faults per rule and category, Topic trees, orphans, Cards without a source line or kinded Link, figures, missing hubs, broken Links, and distinct Bridge pairs in and out per book. `island` is true when a book has no Bridge; an incomplete tree read gives `complete: false` and `island: null`. Writes `audit.json`, `audit.md`, `audit-faults.txt`. | no |
| `zt-calls.ps1`, `book-of.ps1`, `relatedness.ps1` | PowerShell libraries kept for PowerShell callers and their tests (`Invoke-Zt`, `Invoke-ZtCheck`; book attribution; TF-IDF relatedness). The tools use their Rust ports. | as called |

## DeepSeek instruction files (`scripts/book-digest/dsh-*.zh.md`)

Each opens the prompt of every job of its task, so DeepSeek caches it. Each has a 自查 (self-check) step.

| File | Task | Output |
| --- | --- | --- |
| `dsh-digest-base.zh.md` | Digest run: the Idea Cards of one group and its anchor. A variant adds supplements through `profile.instructions`; `dsh-instructions.zh.md` is only the default of `zt-digest prepare` without `-Instructions`. | Cards through the helper |
| `dsh-link-instructions.zh.md` | Link run: reasoned Links from a group's own Cards | Links through the helper |
| `dsh-card-instructions.zh.md` | Card cycle fix and review | JSON |
| `dsh-orphan-instructions.zh.md` | Links to orphan Cards | JSON |
| `dsh-terms-instructions.zh.md` | Book term table | JSON |
| `dsh-skeleton-instructions.zh.md` | Literature index, structure Card, Topic bodies | JSON |
| `dsh-hub-instructions.zh.md` | Field and topic hubs | JSON |
| `dsh-bridge-instructions.zh.md` | Bridge judge: read frozen candidate bodies; judge mechanism, scope, and direction; never edit Cards | the assigned `br-NNN.json` only |

## Workflow files

| File | Use it to |
| --- | --- |
| `scripts/book-digest/workflow/zt-digest-base.blueprint.json` | The base: stages, slots, args, roles, modes, checkpoint. |
| `scripts/book-digest/workflow/modules/zt-digest-lib.js` | The shared library: settings from the args, `zdTool`, the digest and link managers, `drive`, the card cycle (`runCards`). |
| `scripts/book-digest/workflow/modules/` (`zt-dsh-source`, `zt-digest-prepare`, `zt-book-terms`, `zt-cards-book`, `zt-digest-blocks`, `zt-hubs`, `zt-link-prepare`, `zt-link-pass`, `zt-link-check`, `zt-dsh-cleanup`, `zt-organize`, `zt-figures`, `zt-bridge`, `zt-audit`, `zt-digest-report`) | The base stage modules, one per stage. |
| `.claude/workflows/<name>.js` | An installed, frozen variant build. Never edit it: change a blueprint or module and reinstall with `assemble.mjs install`. |

## Tests (`scripts/book-digest/tests/`)

| Test | Covers |
| --- | --- |
| `run-all.ps1` | Runs every test file and harness below in parallel (`-Throttle 4`). |
| `framework-harness.mjs` | The variants built in memory with canned agent answers: base stage order, commands, manager counts and the 8-manager cap, deletes not archives, one shared base, frozen structure, installed = built, ported scripts run as tools. |
| `bridge-workflow-harness.mjs` | The Bridge stage of the workflows: discovery and explicit apply as separate, bounded paths. |
| `card-compiler`, `hub-compiler`, `digest-audit`, `book-terms`, `organize`, `attach-figures`, `prepare` `.tests.ps1` | The base tools on scratch archives (`ZT_CONFIG_DIR`, its own `zt up`), including the precheck steps. |
| `bridge-compiler`, `bridge-planning`, `bridge-validation`, `bridge-math`, `bridge-attribution-retry`, `bridge-human`, `book-of` `.tests.ps1` | The Bridge tool on scratch archives, and book attribution. |
| `zt-check.tests.ps1`, `card-layer.tests.ps1` + fixtures; `cargo test` in `tools/zt-check` and `tools/zt-digest` | zt-check's rules, and the tools' unit tests. |
| `zt-calls.tests.ps1`, `zt-helper-gate.tests.ps1`, `requires-version.tests.ps1` | The helper (`-SrcLog`, `-TopicGate`, parallel helpers), UTF-8 under a GBK console, and the PowerShell 7.5 declarations. |
