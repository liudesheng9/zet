# Scripts and instruction files of the digest workflows

All paths are under `E:\zet`. Scripts are in `scripts/book-digest/` unless the path says otherwise. "Writes" says whether the script changes Cards in the slip-box.

Every digest script runs as a `zt-digest <tool>` (`tools/zt-digest`, installed beside `zt.exe` by `scripts/install-zt-digest.ps1`) with the same parameters and output. The tables name the tool and, in brackets, the `.ps1` it replaced; that `.ps1` only forwards to its tool, so an old command still works. The workflows run the tools directly (`zdTool` in `zt-digest-lib.js`), and every job file's `SELF-CHECK:` line names the tool. `zt-digest call` is the zt helper.

## Compilers (the basic block)

| Tool (script) | Stage | Steps | Use it to | Writes |
| --- | --- | --- | --- | --- |
| `zt-digest model` (`model-compiler.ps1`) | Models | `brief`, `validate`, `apply`, `derive`, `review`, `split`, `report`; `explain`, `explain-check`, `explain-apply`, `explain-report` | Build each Model Card's parts: context packs and briefs, validate Sonnet's `plan.json`, the skeleton and setting tables, fill and derive jobs, review verdicts and splits, the report. The explain steps add 思路, 说明, and 解读 to finished Derivation parts. | yes |
| `zt-digest card` (`card-compiler.ps1`) | Cards, Link check, Organize notes | `plan`, `precheck`, `apply`, `check`, `report` | Fix the Card language and layer faults of a book's Idea Cards, Event Cards, Finding Cards, and anchors; review a sample against the source; link orphan Cards; carry out Organize notes. DeepSeek writes JSON. | yes |
| `zt-digest hub` (`hub-compiler.ps1`) | Hubs | `survey`, `plan`, `precheck`, `apply`, `check`, `report` | Build the line, topic, and field Hub Cards and the register of the whole agent Workspace. State in `D:\zt-digest\hubs\state.json`. | yes |
| `zt-digest terms` (`book-terms.ps1`) | Prepare | `plan`, `apply` | The book term table (术语表) that rule R6 checks against. | the 术语表 Card |
| `zt-digest skeleton` (`book-skeleton.ps1`) | Organize | `plan`, `apply` (`-Apply` writes; without it, a dry check) | The Literature Card index, the structure Card, and the Topic bodies. | yes, with `-Apply` |
| `zt-digest bridge` (`bridge-compiler.ps1`) | Bridge | `plan`, `precheck`, `report`, `apply` | Freeze candidate endpoints, check BR1–BR9 and Card rules, publish an immutable report, and apply only explicit report-bound ids. Flags: Step/Staging/CitationKey/Work/Profile/MaxSources/Candidates/Books/Job/ReportId/Accept/AcceptEarlier/Apply. Preview without -Apply changes no Card or apply state. The workflow runs `report` only when every judge job answered. Apply checks the changed carrier for L1 and L2 as an Idea draft. The summary line holds `outcome_counts`, and `outcomes` only when there are at most 30. | only accepted agent carriers with `-Apply` |
| `zt-digest math` (`math-compiler.ps1`) | math variant | `index`, `docs`, `skeleton`, `notation`, `notation-apply`, `boundary`, `boundary-apply`, `boundary-settle`, `proof`, `proof-apply`, `theory`, `theory-apply`, `check` | Numbered statements, mathematical units, Theory Cards (`docs/math-cards-design.md`). | yes |

## Preparation and passes

| Tool (script) | Stage | Use it to | Writes |
| --- | --- | --- | --- |
| `zt-digest prepare` (`prepare-book-digest.ps1`) | Prepare | Chinese-only text, section groups, the group files (with their `SELF-CHECK` line), the shared DeepSeek prefixes, and with `-Apply` the agent Literature tree with one anchor per group. | anchors |
| `zt-digest snapshot-topics` (`snapshot-topics.ps1`) | Digest | A compact list of every Topic tree for one digest run, so it places and links Cards without searching. | no |
| `book-of.ps1` | none | Kept for old callers and `tests/book-of.tests.ps1`; the tools use its Rust port (`src/audit/book_of.rs`). Pure ordinal classification and weighted inferred attribution, including Finding Cards. A tie goes as in the old audit (sorted, case-insensitive key order). No archive calls. | no |
| `relatedness.ps1` | none | Kept for old callers; the tools use its Rust port (`src/card/relatedness.rs`). Dot-sourced: TF-IDF over Chinese character bigrams; tells each run which other sections and Cards are related. | no |
| `zt-digest prepare-link` (`prepare-link-pass.ps1`) | Link | A snapshot of every Card; per group, the Cards it owns, the candidate targets, the allow list, and the `SELF-CHECK` line. | no |
| `zt-digest list-models` (`list-models.ps1`) | Models | The Model Cards a digest wrote (`-Groups`, `-All -CitationKey`), with level, parts, and faults. | no |
| `zt-digest model-context` (`model-context.ps1`) | Models, Explain | The context pack of one model: its Cards, Links, source group files and headings, the anchor, the other models. | no |
| `zt-digest organize-survey` (`organize-survey.ps1`) | Organize | Everything Opus must decide: duplicate and small Topics, merge proposals, model faults, broken Links, missing hubs. Read-only. | no |
| `zt-digest organize-apply` (`organize-apply.ps1`) | Organize | Carry out Opus's `decisions.json`: Topic merges and Card moves, logged. | yes |
| `zt-digest figures` (`attach-figures.ps1`, with the functions of the removed `figures.ps1`) | Figures | Show the book's figures (`artifact:`) in the Cards that name them; link the book's PDF and Markdown from the Literature Card. | yes |

## Checks and audit

| Tool | Use it to | Writes |
| --- | --- | --- |
| `zt-check` (`tools/zt-check`, install with `scripts/install-zt-check.ps1`) | The one checker and the only source of truth, for every agent: `--cards` (a writer's own Cards, classified as the audit does), `--addresses`, `--model`, `--json` (drafts, offline). Rules R2 to R17, I, E, L. Starts `zt mcp` once to read Cards; `--json` needs no archive. | no |
| `Invoke-ZtCheck` (in `zt-calls.ps1`) | How a PowerShell caller calls zt-check (the tools run zt-check in-process): the old parameter names (`-Json`, `-Model`, `-Addresses`, `-Cards`, `-Log`, `-Book`, `-Rules`, `-Setting`, `-Level`, `-RequireExplain`, `-SkipShared`, `-Max*`). Throws when zt-check does not run. | no |
| `scripts/install-zt-check.ps1` | Build the crate (target `D:\zt-check-target`) and install `zt-check.exe` beside `zt.exe`. A running copy is renamed aside. | no |
| `zt-digest audit` (`digest-audit.ps1`) | Read-only audit of one book or the whole Workspace. It reads the trees of both Workspaces. It reports faults per rule and category, Topic trees, orphans, Idea, Event, and Finding Cards without a source line or kinded Link, models not self-sufficient, explained Derivation parts, figures, missing hubs, broken Links, and actual distinct semantic Bridge pairs in/out by book/human/unattributed. `island` is true when a book has no Bridge in or out. Incomplete tree reads give complete=false and island=null. Writes `audit.json`, `audit.md` (with an Islands row), `audit-faults.txt`. | no |
| `zt-calls.ps1` | Dot-sourced by PowerShell callers and the tests (the tools do this in-process): `Invoke-Zt` calls zt MCP tools through the helper with UTF-8 decoding. | as called |

## DeepSeek instruction files (`dsh-*.zh.md`)

Each opens the prompt of every job of its task, so DeepSeek caches it. Each has a 自查 (self-check) step.

| File | Task | Output |
| --- | --- | --- |
| `dsh-digest-base.zh.md`, `dsh-model-cards.zh.md` | Digest run: Idea and Model Cards of one group, and its anchor (the variants pass both with `-Instructions`; `dsh-instructions.zh.md` is only the default of `zt-digest prepare` without `-Instructions`) | Cards through the helper |
| `dsh-bridge-instructions.zh.md` | Read frozen candidate bodies; assess actual mechanism, scope and direction; SELF-CHECK; never edit Cards | assigned `br-NNN.json` only |
| `dsh-link-instructions.zh.md` | Link run: reasoned Links from a group's own Cards | Links through the helper |
| `dsh-brief-instructions.zh.md` | Model brief: equations, skipped steps, the book's own intuition (直觉) | `brief.md` |
| `dsh-fill-instructions.zh.md` | Model fill and derive: Model Card, setting, parts, charts, Derivation parts with 思路, 说明, 解读 | Cards through the helper |
| `dsh-review-instructions.zh.md` | Derivation review: errors, jumps, density, explanation | `review-NN.json` |
| `dsh-explain-instructions.zh.md` | Explanations for finished Derivation parts | JSON |
| `dsh-card-instructions.zh.md` | Card cycle fix and review | JSON |
| `dsh-orphan-instructions.zh.md` | Links to orphan Cards | JSON |
| `dsh-terms-instructions.zh.md` | Book term table | JSON |
| `dsh-skeleton-instructions.zh.md` | Literature index, structure Card, Topic bodies | JSON |
| `dsh-hub-instructions.zh.md` | Field and topic hubs | JSON |
| `dsh-math-*.zh.md` | The math variant's stages | see `docs/zt-math-digest-workflow.md` |
| `model-architect.md`, `model-architect-math.md` | Sonnet's planning instructions | `plan.json` |

## Workflows and blueprints

| File | Use it to |
| --- | --- |
| `scripts/book-digest/workflow/zt-digest-base.blueprint.json` | The base: shared stages, slots, args, roles, modes. |
| `scripts/book-digest/workflow/zt-econ-digest.blueprint.json`, `zt-math-digest.blueprint.json` | The two variants: profile and slot fills. |
| `scripts/book-digest/workflow/modules/*.js` | The stage modules and libraries. |
| `.claude/workflows/zt-econ-digest.js`, `.claude/workflows/zt-math-digest.js` | The installed, frozen builds. Never edit them: change a blueprint or module and reinstall with `assemble.mjs install`. |

## Tests (`scripts/book-digest/tests/`)

| Test | Covers |
| --- | --- |
| `framework-harness.mjs` | Both variants built in memory with canned agent answers: stage order, commands, manager counts and the 8-manager cap, deletes not archives, one shared base, frozen structure, installed = built. |
| `model-compiler.tests.ps1`, `card-compiler.tests.ps1`, `hub-compiler.tests.ps1`, `math-compiler.tests.ps1`, `event-compiler.tests.ps1`, `finding-compiler.tests.ps1` | Each compiler on a scratch archive (`ZT_CONFIG_DIR`, its own `zt up`), including the precheck and explain-check steps. |
| `zt-check.tests.ps1`, `card-layer.tests.ps1` + fixtures, and `cargo test` in `tools/zt-check` | zt-check's rules, on fixtures. |
| `book-terms`, `organize`, `attach-figures`, `prepare`, `zt-calls`, `zt-helper-gate` `.tests.ps1` | Their tools, through the forwarders. |
| `book-of`, `bridge-compiler`, `bridge-planning`, `bridge-validation`, `bridge-math`, `bridge-attribution-retry`, `bridge-human` `.tests.ps1`; `bridge-workflow-harness.mjs` | The attribution helper, the Bridge compiler on scratch archives, and the Bridge stage of the workflows. `bridge-human.tests.ps1` seeds its scratch database with `node:sqlite` and needs no Python (`-Python` is ignored). |
