# ZT digests on the Workflow Framework Protocol

The ZT digests are built by the Workflow Framework Protocol (global skill `workflow-framework-protocol`): one **base** holds every stage that all book types share, and each book type is a **variant** that installs its own **module set** (its layers and its prompts) into the base's slots. The assembler composes a variant with the base, builds one self-contained Workflow script, and installs it frozen: while it runs, its stages, their order, the roles and their models, the retries, checks, and modes cannot change; only data moves.

| Workflow | Blueprint | Installed script | Book type |
| --- | --- | --- | --- |
| `zt-econ-digest` | `scripts/book-digest/workflow/zt-econ-digest.blueprint.json` | `.claude/workflows/zt-econ-digest.js` | economics |
| `zt-math-digest` | `scripts/book-digest/workflow/zt-math-digest.blueprint.json` | `.claude/workflows/zt-math-digest.js` | mathematics |

Both share `scripts/book-digest/workflow/zt-digest-base.blueprint.json`. The hand-written workflows stay: `.claude/workflows/zt-book-digest.js`, and `.claude/workflows-legacy/zt-math-digest.js` (moved there unchanged when the framework build took the name `zt-math-digest`; `make-math-workflow.mjs` and `math-workflow-harness.mjs` follow it there).

## The base: what every book type shares

```
args → source → prepare → [after-prepare] → terms → cards-book → digest → [after-digest]
     → hubs-first → link-prepare → link → link-check → [after-link] → cleanup → organize → figures → hubs → audit
     → [after-audit] → report
```

| Stage | Module | Runs | On failure |
| --- | --- | --- | --- |
| args | `gate` | always: citationKey (not in modes hubs, audit); source, bibtex, bookTitle (mode full) | halt |
| source | `zt-dsh-source` | not in modes figures, audit: the DeepSeek source probe | halt |
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
| hubs | `zt-hubs` | full with `hubs` not false, or link with `hubs` true | — |
| audit | `zt-audit` | mode audit, or full with `audit` not false | — |
| report | `zt-digest-report` | always: the result in the shape of zt-book-digest.js | — |

The base writes no Model Card, no Event Card, and runs no model compiler: those are layers of a book type. Its digest prompt is `dsh-digest-base.zh.md` (`dsh-instructions.zh.md` without the section on Event and Model Cards). Roles: `manager` (Haiku, effort low), `architect` (Sonnet), `organizer` (Opus). Shared machinery: `zt-digest-lib` (managers, `drive`, the card cycle). Base modes: `link`, `cards`, `hubs`, `figures`, `audit`. Checkpoint: `until: "digest"`.

The book profile (`consts.profile`) is the base's one open const: `instructions` (the digest prompt: `dsh-digest-base.zh.md` plus the type's supplements, inserted before its reply section), `noEventCandidates`, `digestPrefix`, `linkPrefix`, `compilerDocs`, `architectSupplement`, `skipForms`.

## The economics module set (`zt-econ-digest`)

| Layer | What it is |
| --- | --- |
| Prompts | `dsh-digest-base.zh.md` + `dsh-model-cards.zh.md` (the digest writes Idea Cards and Model Cards, no Event Card; no event candidates in the group files); `shared-prefix.md`, `link-prefix.md` |
| Model layer | `models` (`zt-model-layer`, `zt-model-lib`): after the digest, the model compiler on every Model Card of the book with the economic architect `model-architect.md` (DeepSeek brief → Sonnet plan → DeepSeek fill, derive, review, split). `explain` (`zt-explain`): mode explain |
| Event layer | `events` (`zt-event-layer`, `event-compiler.ps1`, `dsh-event-instructions.zh.md`): see below |
| Empirical results | to come: Finding Cards (the `finding` Card type, zt 0.20.0, on branch `claude/trusting-jepsen-4e77dd`; the layer is built when it is merged and deployed) |

Modes: the base modes plus `models`, `explain`, `events` (the event layer alone on an existing digest). Args: the base args plus `events` (false skips the event layer).

### The event layer

`event-compiler.ps1` is a compiler cycle (zt-digest-protocol section 2):

1. **plan**: groups the event candidates (`events.json`: the sentences of a group that name a year, from prepare-book-digest.ps1) by group, lists each group's own Idea Cards (`owned-gNN.txt`) with titles, and writes one job per 3 groups with its `SELF-CHECK:` line.
2. **DeepSeek** writes `events\out\<job>.json`: per real event its title (with the year), 时间, 地点, 参与者, 经过, the section, the Idea Cards it supports (only its group's), the evidence role (`本事件例示 [[…]] …`), and `existing` (an Event Card of the same event, found with `search_cards`).
3. **precheck** (the SELF-CHECK, read-only): the fields, the supported Cards, the role Link, the existing Event Card, and zt-check (E1, E2, the Card language) on the new Event Cards as drafts.
4. **apply**: refuses an event with a fault; for `existing`, adds this book's 来源 line and role to that Card; else creates the Event Card (type event) as a Side successor of its first supported Idea Card; then every supported Idea Card links it: `案例 [[event]]：<title>。`. A rerun skips applied jobs (`state.json`).
5. **report**; the last step deletes the DeepSeek conversations. In mode full the link check card cycle then checks every Event Card (E rules).

Test: `pwsh -NoProfile -File scripts\book-digest\tests\event-compiler.tests.ps1` (a scratch archive).

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
2. Run the fixture: `node scripts/book-digest/tests/framework-harness.mjs` (every check of workflow-harness.mjs and math-workflow-harness.mjs on the built scripts; the layers of each book type; frozen structure; installed = built), and the compiler tests you touched (e.g. `tests/event-compiler.tests.ps1`).
3. Install (project scope) and verify, in PowerShell:

```powershell
$A = "$HOME\.claude\skills\workflow-framework-protocol\tools\assemble.mjs"
node $A install scripts\book-digest\workflow\zt-econ-digest.blueprint.json --sim scripts\book-digest\workflow\zt-econ-digest.sim.json --scope project --project E:\zet
node $A verify zt-econ-digest --scope project --project E:\zet
```

4. Run by path: `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js", args: {...} })`, a pilot first, after a database backup.

A new book type is a new variant: its profile, its prompt supplements, and its layers in the slots.
