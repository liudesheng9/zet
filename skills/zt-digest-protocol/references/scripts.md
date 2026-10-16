# Scripts and instruction files of the digest workflows

All paths are under `E:\zet`. Scripts are in `scripts/book-digest/` unless the path says otherwise. "Writes" says whether the script changes Cards in the slip-box.

## Compilers (the basic block)

| Script | Stage | Steps | Use it to | Writes |
| --- | --- | --- | --- | --- |
| `model-compiler.ps1` | Models | `brief`, `validate`, `apply`, `derive`, `review`, `split`, `report`; `explain`, `explain-check`, `explain-apply`, `explain-report` | Build each Model Card's parts: context packs and briefs, validate Sonnet's `plan.json`, the skeleton and setting tables, fill and derive jobs, review verdicts and splits, the report. The explain steps add 思路, 说明, and 解读 to finished Derivation parts. | yes |
| `card-compiler.ps1` | Cards, Link check, Organize notes | `plan`, `precheck`, `apply`, `check`, `report` | Fix the Card language and layer faults of a book's Idea Cards, Event Cards, and anchors; review a sample against the source; link orphan Cards; carry out Organize notes. DeepSeek writes JSON. | yes |
| `hub-compiler.ps1` | Hubs | `survey`, `plan`, `precheck`, `apply`, `check`, `report` | Build the line, topic, and field Hub Cards and the register of the whole agent Workspace. State in `D:\zt-digest\hubs\state.json`. | yes |
| `book-terms.ps1` | Prepare | `plan`, `apply` | The book term table (术语表) that rule R6 checks against. | the 术语表 Card |
| `book-skeleton.ps1` | Organize | `plan`, `apply` (`-Apply` writes; without it, a dry check) | The Literature Card index, the structure Card, and the Topic bodies. | yes, with `-Apply` |
| `math-compiler.ps1` | math variant | see its header | Numbered statements, mathematical units, Theory Cards (`doc/math-cards-design.md`). | yes |

## Preparation and passes

| Script | Stage | Use it to | Writes |
| --- | --- | --- | --- |
| `prepare-book-digest.ps1` | Prepare | Chinese-only text, section groups, the group files (with their `SELF-CHECK` line), the shared DeepSeek prefixes, and with `-Apply` the agent Literature tree with one anchor per group. | anchors |
| `snapshot-topics.ps1` | Digest | A compact list of every Topic tree for one digest run, so it places and links Cards without searching. | no |
| `relatedness.ps1` | Digest, Link | Dot-sourced: TF-IDF over Chinese character bigrams; tells each run which other sections and Cards are related. | no |
| `prepare-link-pass.ps1` | Link | A snapshot of every Card; per group, the Cards it owns, the candidate targets, the allow list, and the `SELF-CHECK` line. | no |
| `list-models.ps1` | Models | The Model Cards a digest wrote (`-Groups`, `-All -CitationKey`), with level, parts, and faults. | no |
| `model-context.ps1` | Models, Explain | The context pack of one model: its Cards, Links, source group files and headings, the anchor, the other models. | no |
| `organize-survey.ps1` | Organize | Everything Opus must decide: duplicate and small Topics, merge proposals, model faults, broken Links, missing hubs. Read-only. | no |
| `organize-apply.ps1` | Organize | Carry out Opus's `decisions.json`: Topic merges and Card moves, logged. | yes |
| `attach-figures.ps1` | Figures | Show the book's figures (`artifact:`) in the Cards that name them; link the book's PDF and Markdown from the Literature Card. | yes |
| `figures.ps1` | Prepare, Figures | Dot-sourced: the figures of a Scholar Archive book. | no |

## Checks and audit

| Tool | Use it to | Writes |
| --- | --- | --- |
| `zt-check` (`tools/zt-check`, install with `scripts/install-zt-check.ps1`) | The one checker and the only source of truth, for every agent: `--cards` (a writer's own Cards, classified as the audit does), `--addresses`, `--model`, `--json` (drafts, offline). Rules R2 to R17, I, E, L. Starts `zt mcp` once to read Cards; `--json` needs no archive. | no |
| `Invoke-ZtCheck` (in `zt-calls.ps1`) | How the scripts call zt-check: the old parameter names (`-Json`, `-Model`, `-Addresses`, `-Cards`, `-Log`, `-Book`, `-Rules`, `-Setting`, `-Level`, `-RequireExplain`, `-SkipShared`, `-Max*`). Throws when zt-check does not run. | no |
| `scripts/install-zt-check.ps1` | Build the crate (target `D:\zt-check-target`) and install `zt-check.exe` beside `zt.exe`. A running copy is renamed aside. | no |
| `digest-audit.ps1` | Read-only audit of one book or the whole Workspace: faults per rule and category, Topic trees, orphans, Cards without a source line or kinded Link, models not self-sufficient, explained Derivation parts, figures, missing hubs, broken Links. Writes `audit.json`, `audit.md`, `audit-faults.txt`. | no |
| `zt-calls.ps1` | Dot-sourced: `Invoke-Zt` calls zt MCP tools through the helper with UTF-8 decoding. | as called |

## DeepSeek instruction files (`dsh-*.zh.md`)

Each opens the prompt of every job of its task, so DeepSeek caches it. Each has a 自查 (self-check) step.

| File | Task | Output |
| --- | --- | --- |
| `dsh-instructions.zh.md` | Digest run: Idea, Event, and Model Cards of one group, and its anchor | Cards through the helper |
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
| `dsh-math-*.zh.md` | The math variant's stages | see `doc/zt-math-digest-workflow.md` |
| `model-architect.md`, `model-architect-math.md` | Sonnet's planning instructions | `plan.json` |

## Workflows and generators

| File | Use it to |
| --- | --- |
| `.claude/workflows/zt-book-digest.js` | The base workflow. Run an edited copy by `scriptPath`. |
| `.claude/workflows/zt-math-digest.js` | The math variant. Generated: do not edit by hand. |
| `scripts/book-digest/make-math-workflow.mjs` | Regenerate the math variant after every change to the base. |

## Tests (`scripts/book-digest/tests/`)

| Test | Covers |
| --- | --- |
| `workflow-harness.mjs`, `math-workflow-harness.mjs` | Each workflow with canned agent answers: stage order, commands, manager counts and the 8-manager cap, deletes not archives, drift between base and variant. |
| `model-compiler.tests.ps1`, `card-compiler.tests.ps1`, `hub-compiler.tests.ps1`, `math-compiler.tests.ps1` | Each compiler on a scratch archive (`ZT_CONFIG_DIR`, its own `zt up`), including the precheck and explain-check steps. |
| `zt-check.tests.ps1`, `card-layer.tests.ps1` + fixtures, and `cargo test` in `tools/zt-check` | zt-check's rules, on fixtures. |
| `book-terms`, `organize`, `attach-figures`, `zt-calls`, `zt-helper-gate` `.tests.ps1` | Their scripts. |
