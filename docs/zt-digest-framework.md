# ZT digests on the Workflow Framework Protocol

> **2026-10-22: the digests build only Literature trees** (base 5.0.0, econ 6.0.0, math 5.0.0; `docs/literature-only-digest-design.md`). Topic trees, Topic Cards, Hub Cards and the register are written only by zt-idea-grow 3.0.0 (`docs/idea-grow-design.md`). Where this document describes a digest stage that writes them (concepts, review, hubs, mode hubs, Topic placement, Organize merges and Topic bodies), that stage is gone from the digests.

The ZT digests are built by the Workflow Framework Protocol (global skill `workflow-framework-protocol`): one **base** holds every stage that all book types share, and each book type is a **variant** that installs its own **module set** (its layers and its prompts) into the base's slots. The assembler composes a variant with the base, builds one self-contained Workflow script, and installs it frozen: while it runs, its stages, their order, the roles and their models, the retries, checks, and modes cannot change; only data moves.

| Workflow | Blueprint | Installed script | Book type |
| --- | --- | --- | --- |
| `zt-econ-digest` | `scripts/book-digest/workflow/zt-econ-digest.blueprint.json` | `.claude/workflows/zt-econ-digest.js` | economics |
| `zt-math-digest` | `scripts/book-digest/workflow/zt-math-digest.blueprint.json` | `.claude/workflows/zt-math-digest.js` | mathematics |

Both share `scripts/book-digest/workflow/zt-digest-base.blueprint.json`. The hand-written `zt-book-digest.js`, the legacy `.claude/workflows-legacy/zt-math-digest.js`, its generator `make-math-workflow.mjs`, and their harnesses were removed on 2026-10-16: the base and the two variants replace them.

## The base: what every book type shares

```
args → source → prepare → [after-prepare] → terms → cards-book → digest → [after-digest]
     → hubs-first → link-prepare → link → link-check → [after-link] → cleanup → organize → figures → bridge → hubs → audit
     → [after-audit] → report
```

| Stage | Module | Runs | On failure |
| --- | --- | --- | --- |
| args | `gate` | always: citationKey (not in modes hubs, audit); in mode full either `book` (a Scholar Archive Work id or exact title, base 2.4.0) or source, bibtex, bookTitle | halt |
| source | `zt-dsh-source` | ordinary model modes and Bridge discovery; skipped for figures, audit and accepted Bridge apply | halt |
| prepare | `zt-digest-prepare` | mode full: prepare-book-digest.ps1 with the profile's instruction files; else args.groups | halt |
| terms | `zt-book-terms` | mode full, `terms` not false | continue |
| cards-book | `zt-cards-book` | mode cards | — |
| digest | `zt-digest-blocks` | mode full: the digest blocks (Idea Cards and anchors, plus what the type's prompt supplements ask for); each block's card cycle as soon as it finishes | halt when no group is done |
| hubs-first | `zt-hubs` | mode hubs | — |
| link-prepare, link | `zt-link-prepare`, `zt-link-pass` | modes full, link | halt (prepare) |
| link-check | `zt-link-check` | modes full, link; `cardChecks` not false | — |
| cleanup | `zt-dsh-cleanup` | modes full, link | — |
| organize | `zt-organize` | mode full, or `organize` true or "legacy" | continue |
| figures | `zt-figures` | mode figures, or full with `figures` not false | — |
| bridge | `zt-bridge` | full unless `bridge: false`; mode bridge discovers or applies an identified selection | continue; expects a status other than `failed`, and the result keeps `{status: failed}` |
| hubs | `zt-hubs` | full with `hubs` not false, or link with `hubs` true | — |
| audit | `zt-audit` | mode audit, or full with `audit` not false | — |
| report | `zt-digest-report` | always: the result per mode (the shape the removed zt-book-digest.js returned) | — |

The base writes no Model Card, no Event Card, and runs no model compiler: those are layers of a book type. Its digest prompt is `dsh-digest-base.zh.md` (`dsh-instructions.zh.md` without the section on Event and Model Cards). Roles: `manager` (Haiku, effort low), `architect` (Sonnet), `organizer` (Opus). Shared machinery: `zt-digest-lib` (managers, `drive`, the card cycle). Base modes: `link`, `cards`, `hubs`, `figures`, `audit`, `bridge`. Checkpoint: `until: "digest"`.

The book profile (`consts.profile`) is the base's one open const: `instructions` (the digest prompt: `dsh-digest-base.zh.md` plus the type's supplements, inserted before its reply section), `noEventCandidates`, `digestPrefix`, `linkPrefix`, `compilerDocs`, `architectSupplement`, `skipForms`. Each variant also sets `bridgeProfile`.

The installed versions are base 2.1.3, `zt-econ-digest` 3.1.3, and `zt-math-digest` 2.2.3, with the modules `zt-bridge` v4, `zt-digest-lib` v4, and `zt-digest-report` v5. Build, install, and verify through the assembler; edit only the source modules and blueprints. Earlier frozen versions stay in the registry history.

## Bridge discovery and apply

Both workflows (`zt-econ-digest`, `zt-math-digest`) use the same `bridge-compiler.ps1`, judge prompt (`dsh-bridge-instructions.zh.md`), attribution helper (`book-of.ps1`), and checker. Each variant sets `profile.bridgeProfile`: `book` for economics, `math` for mathematics. `zt-book-digest` uses `book`. With the math profile, the stage carries out L6 of `docs/math-cards-luhmann-comparison.md`. Full mode runs Bridge after Figures and before Hubs. It publishes proposals and changes no Card. `bridge: false` returns `skipped`. Other maintenance modes do not run Bridge. `mode: "bridge"` needs only `out` and `citationKey`; no old groups, source, or BibTeX.

```javascript
// Discovery: use a fresh folder; replace the book key and paths with actual values.
Workflow({scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: {
  mode: "bridge", out: "E:\\zet-staging\\book-bridge-review", citationKey: "<actual key>",
  bridgeMax: 12, bridgeCandidates: 5, bridgeBooks: "<optional exact earlier key>"
}})
// Only after the user reviews the actual report and selects its exact ids:
Workflow({scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: {
  mode: "bridge", out: "E:\\zet-staging\\book-bridge-review", citationKey: "<same key>",
  bridgeReport: "<actual report_id>", bridgeApply: ["<actual bNNN>"]
}})
```

`bridgeMax` takes a whole number from 1 to 120 (default 60). `bridgeCandidates` takes a whole number from 1 to 8 (default 5). The gate refuses any other value. Each source also gets at most two term matches. Up to twelve sources share a judge job. Retrieval uses the existing 80-term TF-IDF over the title (twice) and the excerpt. Then it adds term matches per source: the book's term-table terms that occur in the source's title or excerpt, and a Model Card's or unit's name (its title up to the first colon). It matches these terms against the title and excerpt of every Card. The search is not exhaustive. Book attribution counts distinct source Links: 10 for a root or an anchor, 1 for another Card of the Literature tree. `book-of.ps1` breaks a tie as the old audit did (sorted, case-insensitive key order). A missing tree read stops the plan.

**Publish only when complete.** The workflow publishes a report only after every judge job answered. When the plan fails or a judge job fails, it publishes nothing. The result then says to run mode `bridge` again with the same `out` to resume. The cleanup deletes every DeepSeek judge conversation and removes the DeepSeek workspace. It never archives, and it always runs, also after a throw. The Bridge stage expects `status != failed` and continues on a failure. So a failure shows in the stage status, and the run result keeps `{status: 'failed'}` in mode `full`.

**Judges are read-only.** A judge manager approves only one list: `bridge-compiler.ps1 -Step precheck`, `zt-check`, the zt helper with the empty `read-only.allow.txt`, reads under `out`, and JSON writes under `bridge\out`.

**The checks.** BR5 accepts the kind word with or without a space before the Link. A Model Card carrier takes no evidence word (`印证`, `案例`). One relation per pair of Cards: the reverse of a proposed pair is refused. R6 uses the model's setting when it holds a 术语表, else the book term table.

Read `out/bridge/report.md` and `proposals.json`. Their immutable plan and report ids bind the exact endpoint snapshots and sentences. Proposal ids are `b001`, `b002`, and so on. `report.md` names the book title. Its seven tables show new carriers, separately accepted earlier carriers, the target books ("按目标书", with a column per Relation kind), no relation or no candidate, omitted sources, refused or failed work, and apply outcomes. Source titles are shown with their addresses. A late judge file does not replace a report. Keep `plan.json`, `proposals.json`, the answers, and the separate `state.json` journal.

Selections are `"all"` or unique arrays of ids. `bridgeApplyEarlier` selects earlier carriers apart from `bridgeApply`, also for an earlier-only apply. An empty array allows no edit. Booleans, mixed arrays, wrong sections, unknown ids, and a different report fail. Apply skips the source probe, planning, judges, model compilers, Organize, and Hubs. It calls the compiler's script path and returns the stored outcomes with the same report id, also after a partial failure. Each accepted carrier gets the paragraph `跨书关联：<sentence>` through `edit_card` with `base` (the carrier's raw text).

Apply checks the changed carrier for L1 and L2 as an Idea draft, also when the carrier is a Model Card. It compares the model shape with the state just before the edit. A saved intent that was not written and whose ids differ from a new selection is dropped. A blocked carrier gives each of its accepted ids the outcome `blocked`. The summary line is compact: `outcome_counts` always, and the per-proposal `outcomes` only when there are at most 30. Else `report.md` and `state.json` hold them.

Before a live apply, keep a consistent archive backup (`zt dp`, or the SQLite `VACUUM INTO` procedure with an integrity check), and keep filing quiet: no insertion, move, deletion, key rename, retype, or concurrent Organize. The raw `base` protects each carrier. It does not protect both endpoints in one transaction. A lock per report and a saved write intent let a later run verify an interrupted write. An existing Link without a matching verified journal entry stays `already_linked`. A write whose check failed after it was stored stays `written_unverified`. No automatic rollback removes accepted text. A stale report needs a fresh folder and a new review. Hubs after an apply are a separate request; a restore of the whole archive is the user's decision.

The read-only audit counts actual distinct semantic pairs per book, out and in, by book, with `human` and `unattributed` buckets. It does not count proposals. Both endpoints must be substantive, and the Link must have a kind other than `refers`. `island` is true when a book has no semantic Bridge in or out. An incomplete tree read gives `complete: false, island: null`, so zero counts then do not prove isolation. `audit.md` lists the islands in its Agent Workspace table. Source tests, installed-copy checks, the real report-only pilot, and an accepted live apply are separate delivery gates.

## The economics module set (`zt-econ-digest`)

| Layer | What it is |
| --- | --- |
| Prompts | `dsh-digest-base.zh.md` + `dsh-model-cards.zh.md` (the digest writes Idea Cards and Model Cards, no Event Card; no event candidates in the group files); `shared-prefix.md`, `link-prefix.md` |
| Model layer | `models` (`zt-model-layer`, `zt-model-lib`): after the digest, the model compiler on every Model Card of the book with the economic architect `model-architect.md` (DeepSeek brief → Sonnet plan → DeepSeek fill, derive, review, split). `explain` (`zt-explain`): mode explain |
| Event layer | `events` (`zt-event-layer`, `event-compiler.ps1`, `dsh-event-instructions.zh.md`): see below |
| Empirical results | `findings` (`zt-finding-layer`, `finding-compiler.ps1`, `dsh-finding-instructions.zh.md`): see below. Needs zt 0.20.0 deployed (the Card type `finding`, docs/finding-cards-design.md) |

Modes: the base modes plus `models`, `explain`, `events`, `findings` (each layer alone on an existing digest). Args: the base args plus `events` and `findings` (false skips that layer).

**Before a real run:** the live zt must be 0.20.0 or later and zt-check must have the F rules; until then the findings stage fails to create its Cards (run with `findings: false`). Deploying zt is a separate step (it restarts the service; never during a digest run).

### The event layer

`event-compiler.ps1` is a compiler cycle (zt-digest-protocol section 2):

1. **plan**: groups the event candidates (`events.json`: the sentences of a group that name a year, from prepare-book-digest.ps1) by group, lists each group's own Idea Cards (`owned-gNN.txt`) with titles, and writes one job per 3 groups with its `SELF-CHECK:` line.
2. **DeepSeek** writes `events\out\<job>.json`: per real event its title (with the year), 时间, 地点, 参与者, 经过, the section, the Idea Cards it supports (only its group's), the evidence role (`本事件例示 [[…]] …`), and `existing` (an Event Card of the same event, found with `search_cards`).
3. **precheck** (the SELF-CHECK, read-only): the fields, the supported Cards, the role Link, the existing Event Card, and zt-check (E1, E2, the Card language) on the new Event Cards as drafts.
4. **apply**: refuses an event with a fault; for `existing`, adds this book's 来源 line and role to that Card; else creates the Event Card (type event) as a Side successor of its first supported Idea Card; then every supported Idea Card links it: `案例 [[event]]：<title>。`. A rerun skips applied jobs (`state.json`).
5. **report**; the last step deletes the DeepSeek conversations. In mode full the link check card cycle then checks every Event Card (E rules).

Test: `pwsh -NoProfile -File scripts\book-digest\tests\event-compiler.tests.ps1` (a scratch archive).

The self-check gives each draft `category` and `book`: zt-check applies the Card layer rules (E1, E2; F1 to F3) to a `--json` draft only when it carries a category.

### The empirical-results layer

`finding-compiler.ps1` follows the same cycle, for Finding Cards (one empirical result each, docs/finding-cards-design.md):

1. **plan**: finds each group's result candidates in its section files: lines that name a number and a cue of empirical work (数据, 样本, 调查, 访谈, 回归, 实验, 统计, 显著, 系数, %, …), at most 16 per group; lists the group's own Idea Cards and its anchor; one job per 3 groups with its `SELF-CHECK:` line.
2. **DeepSeek** writes `findings\out\<job>.json`: per result its title (the result as a claim with its scope), 测量, 数据, 方法, 结果 (one result, with its size or direction), 局限, the section, the Idea Cards it is evidence for, and the role (`这一结果印证 [[…]] …`).
3. **precheck**: the fields, the supported Cards, the role Link, and zt-check on the drafts (F1 labels, F2 the source line links the group's anchor, F3 one result, the Card language).
4. **apply**: refuses a finding with a fault; creates the Finding Card (type `finding`) as a Side successor of its group's anchor (body: the six labels, then the role); every supported Idea Card links it: `印证 [[finding]]：<title>。` (F4).
5. **report**; the last step deletes the DeepSeek conversations.

Test: `pwsh -NoProfile -File scripts\book-digest\tests\finding-compiler.tests.ps1` (a scratch archive; by default it uses the zt and zt-check debug builds in `D:\zt-target` and `D:\zt-check-target`, which must be 0.20.0 or later).

## The mathematics module set (`zt-math-digest`)

| Layer | What it is |
| --- | --- |
| Prompts | `dsh-digest-base.zh.md` + `dsh-model-cards.zh.md`, then `math-prefix.md` and `math-link-prefix.md` (math-compiler.ps1 -Step docs appends the math supplement); no Event Card |
| Model layer | `models` with the math supplements (`-BriefDoc`, `-FillDoc`, `-ReviewDoc`, `model-architect-math.md`), Theory Cards left out; `explain` |
| math-skeleton (after-prepare) | index, docs, skeleton (halt on a failure), notation table (unless `notation` is false) |
| math-boundary → math-proof (after-digest, after the models) | hypothesis roles, counterexamples, contrast pairs; the proof review |
| math-theory (after-link) | the Theory Cards, once the cited premises are complete |
| math-check (after-audit) | rules M1 to M18 |

Modes: the base modes plus `models`, `explain`, `math`.

## Changing a digest

1. Change a module (`scripts/book-digest/workflow/modules/*.js`; raise its version when its behaviour changes), a compiler script, or a blueprint (raise its version; update the `preflight` lines it touches).
2. Run the fixture: `node scripts/book-digest/tests/framework-harness.mjs` (the stage, command, and manager checks on the built scripts; the layers of each book type; frozen structure; installed = built), and the compiler tests you touched (`tests/event-compiler.tests.ps1`, `tests/finding-compiler.tests.ps1`). `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1` runs all of them in parallel.
3. Install (project scope) and verify, in PowerShell:

```powershell
$A = "$HOME\.claude\skills\workflow-framework-protocol\tools\assemble.mjs"
node $A install scripts\book-digest\workflow\zt-econ-digest.blueprint.json --sim scripts\book-digest\workflow\zt-econ-digest.sim.json --scope project --project E:\zet
node $A verify zt-econ-digest --scope project --project E:\zet
```

4. Run by path: `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: {...} })`, a pilot first, after a database backup.

A new book type is a new variant: its profile, its prompt supplements, and its layers in the slots.
