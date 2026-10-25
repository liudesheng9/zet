# Tools, prompt files, and tests of the digest base

All paths are under `E:\zet`. "Writes" says whether a tool changes Cards in the slip-box.

Every base stage runs a `zt-digest <tool>` (`tools/zt-digest`, installed beside `zt.exe` by `scripts/install-zt-digest.ps1`). A tool keeps the parameters and output of the PowerShell script it replaced (in brackets); that `.ps1` in `scripts/book-digest/` only forwards to its tool, so an old command still works. The workflow modules build each command with `zdTool` (`ZD_TOOLS` in `zt-digest-lib.js`), and every job file's `SELF-CHECK:` line names the tool. The variants' layers use more tools (`model`, `model-context`, `list-models`, `event`, `finding`, `math`); the variants' blueprints and docs describe them.

## Compilers (the basic block)

| Tool (script) | Stage | Steps | Use it to | Writes |
| --- | --- | --- | --- | --- |
| `zt-digest card` (`card-compiler.ps1`) | digest blocks, link-check, cards-book, Organize notes | `plan`, `precheck`, `apply`, `check`, `report` | Fix the Card language and layer faults of a book's Idea, Event, and Finding Cards and anchors; review a sample against the source; link orphan Cards; carry out Organize notes. DeepSeek writes JSON. Options: `-Book`, `-Staging`, `-Groups`, `-Sample`, `-Rules`, `-MaxCards`, `-NoOrphans`, `-Notes`, `-Compact`, `-MaxJobs`. | yes |
| `zt-digest terms` (`book-terms.ps1`) | terms | `plan`, `apply` | The book term table (术语表) that rule R6 checks against. | the 术语表 Card |
| `zt-digest term-cards` (`zt-term-cards` module) | term-cards | `plan`, `jobs`, `check`, `review-plan`, `fix-plan`, `apply` | The Term Cards of one book: plan the source-bound rows and their writer jobs, read each exact draft's T1–T10 check, record the independent mathematical review and one finite fix, then publish the ready ordered prefix. `apply -Token` completes the book unit only from its publication report; a failed row stays staged. Mode `term-cards` runs the same tool as the backfill of an already digested book (its prepare branch reuses or rebuilds only the staged source slices). | Term Cards |
| `zt-digest term-link` (`zt-term-link` module, shared `zt-term-link-lib`) | term-link, finalize | `plan`, `apply` | The deterministic text-preserving Term-link pass: freeze the plan (leftmost-longest, blockers, homonyms, protected spans), apply it in bounded identified batches, and complete the unit only from the apply report. A second plan has no edit; a changed list, source or body invalidates the plan before any write. Mode `term-link` is the backfill pass; the finite finalization runs the same library for its changed Cards or for the whole book when the correction changed the Term mapping. | Term links |
| `zt-digest skeleton` (`book-skeleton.ps1`) | organize | `plan`, `apply` (`-Apply` writes; without it, a dry check) | The Literature Card index and the structure Card (no Topic body: an answer with `topics` is a problem). | yes, with `-Apply` |
| `zt-digest hub` (`hub-compiler.ps1`) | none since base 5.0.0 (`zt-idea-grow` runs it through `grow -Step hubs-*`) | `survey`, `plan`, `precheck`, `apply`, `check`, `report` | The line, topic, and field Hub Cards and the register of the whole agent Workspace. State in `<hubWork>\state.json` (default `D:\zt-digest\hubs`). | yes |
| `zt-digest register` (`register-compiler.ps1`) | none since base 5.0.0 (`zt-idea-grow` `grow -Step hubs-report`) | `plan`, `apply`, `check` | The concept register's 词条 plan (read-only: every agent book term table, the reviewed concept report through `-Review`, the accepted pinyin readings through `-Readings`), its publication (`apply`, replaying receipts and publishing parts before the root), and the page-by-page `check`. Report-first: the hub stage applies only the exact plan id in `args.registerPlan`. | `apply` only |
| `zt-digest concepts` (`concept-compiler.ps1`) | none since base 5.0.0 (a follow-up may move it to `zt-idea-grow`) | `propose`, `prepare`, `judge`, `report`, `decision` | The frozen concept review of the run's own book term table: `propose` derives the proposals from `<Staging>\terms.md` (one per term row), `prepare` freezes the bounded candidate evidence and its one judge job, `judge` validates the answer, and `report` renders `concept-review.json`/`.md` from the frozen files alone. No Card is written. | no |
| `zt-digest restate` (`restate-compiler.ps1`) | none since base 5.0.0 (`zt-idea-grow` reviews its new Topic Cards with it) | `prepare`, `judge`, `apply`, `report` | The scoped restatement review of the run's own new Cards: without `-Cards`, `prepare` reads the staged `owned-g*.txt` manifests, narrows and bounds the population and reports its omissions; `apply -OperationPrefix` carries each accepted rewrite through identified receipts, and `-Merges` counts a Card-cycle merge verdict as pending until a rewrite covers it. | `apply` only |
| `zt-digest bridge` (`bridge-compiler.ps1`) | bridge | `plan`, `precheck`, `report`, `apply` | Freeze candidate endpoints, check BR1–BR9 and the Card rules, publish an immutable report, and apply only explicit report-bound ids. Options: `-Staging`, `-CitationKey`, `-Work`, `-Profile`, `-MaxSources`, `-Candidates`, `-Books`, `-Job`, `-ReportId`, `-Accept`, `-AcceptEarlier`, `-Apply`. Without `-Apply` it changes no Card or apply state. | only accepted agent carriers, with `-Apply` |

## Preparation and passes

| Tool (script) | Stage | Use it to | Writes |
| --- | --- | --- | --- |
| `zt-digest prepare` (`prepare-book-digest.ps1`) | prepare | Chinese-only text, section groups (`groups\gNN.md` with their write command and `SELF-CHECK` line), the shared DeepSeek prefixes from `-Instructions`, and with `-Apply` the agent Literature tree with one anchor per group. | anchors |
| `zt-digest snapshot-topics` (`snapshot-topics.ps1`) | none (`group-start` writes each group's snapshot of the Literature trees itself) | A compact list of every tree. | no |
| `zt-digest prepare-link` (`prepare-link-pass.ps1`) | link-prepare | A snapshot of every Literature-tree Card; per group, the receipt-owned Cards, the semantic gap set with its reasons and candidates, the gap-only allow list and the frozen `link-gap-gNN.json`; the scheduled, gap-free and blocked groups. | no |
| `zt-digest organize-survey` (`organize-survey.ps1`) | organize | What Opus decides: merge proposals, model faults, broken Links (it also lists Topics and hubs, which a digest leaves alone). Read-only. | no |
| `zt-digest organize-apply` (`organize-apply.ps1`) | organize | Record Opus's `decisions.json` (merges and moves are always empty in a digest; in the Literature scope it can move no Topic Card; `-Open` only for hand Topic merges). | no in a digest |
| `zt-digest figures` (`attach-figures.ps1`) | figures | Show the book's figures (`artifact:`) in the Cards that name them; link the book's PDF and Markdown from the Literature Card. | yes |

## The helper, checks, and audit

| Tool | Use it to | Writes |
| --- | --- | --- |
| `zt-digest call` (the old path `skills/zt/scripts/zt-mcp.ps1` forwards to it) | The zt helper: a JSON array of calls on stdin or in a file, with `-Allow`, `-Log`, `-SrcLog`, `-TopicGate`, `-Moves`, and `-From`/`-Addresses` to resume. DeepSeek writes through it; the tools run it in-process. | as called |
| `zt-check` (`tools/zt-check`, `scripts/install-zt-check.ps1`) | The one checker for every agent: `--cards` (a writer's own Cards, classified as the audit does), `--addresses`, `--model`, `--json` (drafts, offline), `--draft` (an exact proposed Card bundle with its type fields, optional prior state and declared context), `--workspace human|agent`, `--report` (the structured status document). Rules R2 to R21, the Card layer rules I, E, F, L, and the Term rules T1–T10; `faults: N` counts blocking diagnostics and an advisory line carries `(advisory)`. Exit codes 0/1/2: complete clean, complete with blockers, invalid/incomplete/stale. A live check compares the running service's rule/diagnostic/applicability versions and Term capabilities with its own; a mismatch is `stale`, never a pass. | no |
| `zt-digest audit` (`digest-audit.ps1`) | Read-only audit of one book or the whole Workspace, over both Workspaces: faults per rule and category, Topic trees, orphans, Cards without a source line or kinded Link, figures, missing hubs, broken Links, and distinct Bridge pairs in and out per book. Its `terms` section reports coverage, heads/parts, T faults, uses, no-use observations, avoid words and top terms, with the run's Term generation and the running service's check contract, so `audit ran` never means `all terms passed`. `island` is true when a book has no Bridge; an incomplete tree read gives `complete: false` and `island: null`. Writes `audit.json`, `audit.md`, `audit-faults.txt`. | no |
| `zt-digest finalize` (`zt-finalize` module) | The finite finalization of a full run: `plan` (the audit into one bounded correction cycle), `scope` (the changed-Card/touched-tree manifest), the round's Term-link pass through `zt-term-link-lib`, `refresh` (republish only the affected layer revision, e.g. `link-check`), and `final` (the second read-only audit). At most one correction cycle; a residual fault is a `partial` result, never a halted run. | via its one correction cycle and Term-link round |
| `zt-digest ledger` | The run ledger: `next`, `hold`, `renew`, `done`, `fail`, `gate`, `status`, `resume`, `job`, `resource`. Every stage's units are claimed, held, completed or failed here; the ledger classifies faults and answers readiness. `ledger resume` is the one supported resume (invocation 2, only unresolved units reset). | no |
| `zt-calls.ps1`, `book-of.ps1`, `relatedness.ps1` | PowerShell libraries kept for PowerShell callers and their tests (`Invoke-Zt`, `Invoke-ZtCheck`; book attribution; TF-IDF relatedness). The tools use their Rust ports. | as called |

## DeepSeek instruction files (`scripts/book-digest/dsh-*.zh.md`)

Each opens the prompt of every job of its task, so DeepSeek caches it. Each has a 自查 (self-check) step.

| File | Task | Output |
| --- | --- | --- |
| `dsh-digest-base.zh.md` | Digest run: the Idea Cards of one group and its anchor. A variant adds supplements through `profile.instructions`; without `-Instructions`, `zt-digest prepare` uses `dsh-digest-base.zh.md,dsh-model-cards.zh.md`. Every Card goes in the book's Literature tree. | a JSON draft that `group-finish` applies |
| `dsh-link-instructions.zh.md` | Link run: only the semantic gap Cards, one reasoned Link sentence or a `no_link`/`none` reason each | One bound JSON draft, checked by `zt-digest link-finish -CheckOnly` |
| `dsh-card-instructions.zh.md` | Card cycle fix and review | JSON |
| `dsh-orphan-instructions.zh.md` | Links to orphan Cards | JSON |
| `dsh-terms-instructions.zh.md` | Book term table | JSON |
| `dsh-term-card-instructions.zh.md` | Term Cards: one exact term of the book, its complete template, the `SELF-CHECK` precheck and the finite fix | JSON |
| `dsh-skeleton-instructions.zh.md` | Literature index and structure Card | JSON |
| `dsh-hub-instructions.zh.md` | Field and topic hubs (`zt-idea-grow`'s hub jobs, not a digest's) | JSON |
| `dsh-bridge-instructions.zh.md` | Bridge judge: read frozen candidate bodies; judge mechanism, scope, and direction; never edit Cards | the assigned `br-NNN.json` only |

## Workflow files

| File | Use it to |
| --- | --- |
| `scripts/book-digest/workflow/zt-digest-base.blueprint.json` | The base: stages, slots, args, roles, modes, checkpoint. |
| `scripts/book-digest/workflow/modules/zt-digest-lib.js` | The shared library: settings from the args, `zdTool`, the digest and link managers, `drive`, the card cycle (`runCards`). |
| `scripts/book-digest/workflow/modules/` (`zt-dsh-source`, `zt-digest-prepare`, `zt-book-terms`, `zt-term-cards`, `zt-group-cycle`, `zt-cards-layer`, `zt-cards-book`, `zt-link-prepare`, `zt-link-pass`, `zt-link-check`, `zt-term-link`, `zt-term-link-lib`, `zt-dsh-cleanup`, `zt-organize`, `zt-figures`, `zt-bridge`, `zt-audit`, `zt-finalize`, `zt-digest-report`) | The base stage modules, one per stage; `zt-term-link-lib` is the pass shared by the `term-link` stage and the finite finalization. |
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
| `release-set.tests.ps1`; `cargo test --test release_manifest` | The candidate release set: executable/rule/schema/workflow versions, the installed frozen scripts against their locks, retained history, and the domain/skill/README guidance the release ships (`docs/term-cards-links-and-check-gate-release-manifest.json`). |
| `scripts/verify-release.ps1` (at the release boundary, not in `run-all`) | The installed and running half: installed binaries and their hashes, `assemble.mjs verify` for both variants, the installed skills, the running daemon's version and published rule/capability contract, the advisory gate stage, and the wiki check. |

