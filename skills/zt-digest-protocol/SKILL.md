---
name: zt-digest-protocol
description: The protocol of the ZT digest base (scripts/book-digest/workflow/zt-digest-base.blueprint.json), the shared part of every ZT digest workflow that turns a book into ZET Cards. It names the roles, the compiler cycle, the base stages and slots, the modes, the zt-digest tools the base runs, the profile contract a variant fills, the quality checks (zt-check and the SELF-CHECK lines), and the rules each run keeps. Use it to run or change the base, to debug a base stage, or to build a new variant on it. It does not describe a variant's own layers (models, events, findings, math): see that variant's blueprint and docs.
---

# ZT Digest Protocol (the base)

A digest turns one book (a Chinese or bilingual Markdown file from the Scholar Archive) into the book's **Literature tree** in the **agent Workspace** of the ZET slip-box, and nothing else (base 5.0.0, `docs/literature-only-digest-design.md`): it creates no Topic tree, writes no Topic-tree Card, no Hub Card and no register, and links no Topic-tree Card. Every zt-digest step of a digest runs in the **Literature scope**, which refuses such a write before it reaches the archive (`literature_only: …`). Topic trees, Topic Cards, hubs and the register are the work of the `zt-idea-grow` workflow, which runs after a digest. Every digest workflow is built by the Workflow Framework Protocol from one **base** and one **variant**:

- The base (`E:\zet\scripts\book-digest\workflow\zt-digest-base.blueprint.json`, `"kind": "base"`) holds every stage that all book types share, the args, the roles, the modes, and four empty **slots**.
- A variant blueprint names the base, sets the **profile** (section 4), and fills the slots with its own layers. The installed variants are frozen builds in `E:\zet\.claude\workflows\`. This skill covers the base only.

Docs in the repo (`E:\zet\docs`): `zt-digest-workflow.md` (stages and args), `zt-digest-framework.md` (base, variants, install), `hub-cards-design.md`, `digest-card-layers-proposal.md`. The developer wiki block is `docs/wiki/digest-workflows.md`. The tools, prompt files, and tests of the base are listed in [references/scripts.md](references/scripts.md).

## 1. Roles

Each kind of worker does one kind of work. Do not move work between them.

| Worker | Does | Never does |
| --- | --- | --- |
| **Tools** (`zt-digest <tool>`, `E:\zet\tools\zt-digest`) | Every structural step: skeletons, tree operations (insert, move, delete), job files, checks, apply, reports. Every zt write of a JSON answer. | Write prose. |
| **DeepSeek** (DeepSeek Harness, V4.1 Flash, effort high) | The labour: Card text, Links, reviews, terms, skeleton text, Bridge judgements. It writes Card bodies through the zt helper, or a JSON file that a tool applies. | Plan structure, or change Cards it does not own. |
| **Haiku** (role `manager`, effort low) | Runs one tool command per step; manages DeepSeek jobs: starts them, approves their tool calls, collects answers, deletes conversations. | Read the book, write Cards, decide content. |
| **Opus** (role `organizer`) | The Organize decisions only (`decisions.json`: the thesis and the notes on the book's own Cards; merges and moves are always empty). | Edit Cards: a tool and DeepSeek carry out its decisions. |

The base also declares the role `architect` (Sonnet) for variant layers that plan models; no base stage uses it. Modules start agents only through `ctx.agent`, so the role fixes the model and effort.

Card language: every Card follows `asd-ste100-zh` (one statement per sentence, no `；`, active voice, at most 40 characters per description sentence). zt-check enforces it (section 6).

## 2. The basic block: the compiler cycle

Every stage that hands work to DeepSeek is a **compiler cycle**: one tool with steps, DeepSeek in between.

```
survey ──▶ plan ──▶ DeepSeek writes ──▶ SELF-CHECK ──▶ apply ──▶ check ──▶ (one fix round) ──▶ report ──▶ cleanup
 tool      tool      JSON or bodies      the writer      tool      tool      same conversation    tool       Haiku
```

1. **plan** writes one job file per few items and prints `{"jobs":[{"id","files":[...],"prompt"}]}` as its last line. `files` opens with the shared instruction file (`dsh-*.zh.md`), so DeepSeek caches the prefix.
2. **DeepSeek writes** a JSON result file (preferred: the tool then makes every zt write), or Card bodies through the helper with `-Allow` (only its own Cards).
3. **SELF-CHECK**: every job file carries a `SELF-CHECK:` line, the exact read-only command that runs the checks of apply. The writer runs it, fixes each fault, and reruns until `faults: 0` (at most three runs). A fault the old Card already had is listed as kept, not counted.
4. **apply** validates again and writes. It refuses a changed Card, a lost Link, figure, or formula, and any new fault. It never writes unchecked text: no `faults: N` line from the checker means stop.
5. **check / fix**: refused items go back **once** to the same conversation (`"continue": "<job id>"`, with `fallback` files for a fresh one). A second refusal is final and reported.
6. **report** prints counts per item. **cleanup** deletes the DeepSeek conversations and removes the workspace (section 5).

The workflow drives one step with `drive()` (`zt-digest-lib.js`): one Haiku agent runs the command, Haiku managers run its jobs (8 per manager), and an optional cleanup agent deletes the conversations. The base's compilers: `card` (the card cycle), `terms`, `skeleton`, `bridge`; the Term stages are their own modules (`zt-term-cards`, `zt-term-link` with the shared `zt-term-link-lib`). A new stage is a new compiler with the same steps.

## 3. The base stages (mode `full`)

```
args ─▶ source ─▶ prepare ─▶ [after-prepare] ─▶ terms ─▶ term-cards ─▶ digest ─▶ cards
     ─▶ [after-digest] ─▶ cards-book ─▶ link-prepare ─▶ link ─▶ link-check ─▶ [after-link]
     ─▶ organize ─▶ figures ─▶ bridge ─▶ term-link ─▶ audit ─▶ finalize
     ─▶ [after-audit] ─▶ cleanup ─▶ report
```

`[slot]` marks where a variant puts its layers; the base leaves the slots empty. Each stage is one module in `scripts/book-digest/workflow/modules/`; every module builds its commands with `Z.tool('<script>')` (`zdTool`), which gives `zt-digest <tool>`.

The book term table precedes the first group writer and the **Term-card branch**, which run beside each other; the per-group Card checks follow the digest blocks and the whole-book card cycle follows the variant's after-digest layers; the gap-only Link pass follows the whole-book card cycle; then Organize, Figures and the **report-first Bridge** (Literature trees only); the deterministic **Term-link pass** follows the Bridge and the Term-card branch, and the finite finalization and its read-only audit stay last. The concept review, the restatement review of new Topic Cards, the hubs and the register plan left the base in 5.0.0: `zt-idea-grow` owns that work, and the run report names that follow-up.

| Stage (module) | Steps | Writes Cards? | On failure |
| --- | --- | --- | --- |
| args (`gate`) | Checks the required args per mode. Modes `resume`, `term-cards` and `term-link` accept only `out` (plus the Term settings) and read the book, its source and its Citation key from the prepared run; `termCards: false` contradicts `term-cards`, `termLink: false` contradicts `term-link`. | no | halt |
| source (`zt-dsh-source`) | The DeepSeek source is frozen at the official one (`deepseek-official/deepseek-flash`); no probe runs. | no | halt |
| prepare (`zt-digest-prepare`) | `zt-digest prepare -Apply` with the profile's `-Instructions` (and `-NoEventCandidates`): Chinese-only text, section groups (`groups\gNN.md`, each with its `SELF-CHECK:` line), the shared prefixes, and the Literature tree with one anchor Card per group. Mode `term-cards` prepares or reuses only the staged source slices (`-Sections -Brief`); mode `term-link` only validates the book and its staging (`-Validate`). Both stop on a Lock or a typed conflict before any provider work. | anchors | halt |
| terms (`zt-book-terms`) | `zt-digest terms` plan → one DeepSeek job → apply: the book term table (术语表) that rule R6 reads. | the 术语表 Card | continue |
| term-cards (`zt-term-cards` v2) | `zt-digest term-cards -Step plan` registers `termcards/tNNN@<plan revision>` and `termcards/book@<revision>` in the run ledger. Waves claim up to four ready job units, run their writers with the jobs, then `check`, `review-plan` and at most four independent reviewers alongside the retained writers, then `fix-plan` and the fix continuations in each writer's own conversation; a changed draft is checked and reviewed again. Each job unit completes only from `check/readiness.json` (schema, shared check, required reviews, fix disposition), and `apply -Token` publishes the ready ordered prefix; the book unit completes only from its publication report. A failed writer, a refused row or a partial publication leaves its unit incomplete and is reported. Every manager call takes one `dsmpSlot`. | Term Cards | continue |
| digest (`zt-group-cycle`) | Up to 8 lanes; each Haiku manager claims up to 8 ready `digest/gNNN@1` units, starts each group's snapshot and frozen job, runs their DeepSeek jobs side by side, and records each `group-finish`. No card cycle follows a block: the per-group offline checks are the `cards` stage and the whole-book cycle is `cards-book`. | yes | halt |
| cards (`zt-cards-layer`) | Claims and finishes the per-group offline Card checks (`cards/gNNN@1`), one `finish-batch` command per block. | no (checks) | continue |
| cards-book (`zt-cards-book`) | The whole-book card cycle with orphans, after the digest blocks and the variant's after-digest layers (mode `full`, and mode `cards` on its own). | rewrites | halt |
| link-prepare, link (`zt-link-prepare`, `zt-link-pass`) | `zt-digest prepare-link` computes each completed group's receipt-owned semantic gap set (`pending`, `no_gap`); then one chain per group claims `link/gNNN@1`, `zt-digest link-start` freezes the gap scope and writes the job, the group's DeepSeek run fills only those gaps (`dsh-link-instructions.zh.md`), and `zt-digest link-finish` applies and checks it. A group with no gap starts no provider run. | Links | halt |
| link-check (`zt-link-check` v2) | `zt-digest link-check -Step cycle` runs the Link check cycle on the book and publishes its own `link-check/book@N` revision from the cycle report; a mode-`full` run then lets the finalization refresh it only when its correction changed a Card. | rewrites, Links | halt |
| cleanup (`zt-dsh-cleanup`) | Delete leftover DeepSeek conversations, remove the workspace, delete scratch files, read the usage ledger. | no | halt |
| organize (`zt-organize`) | `zt-digest organize-survey` → Opus `decisions.json` (thesis and notes; no merge, no move) → `zt-digest organize-apply` (records the empty decisions; in the Literature scope it can move no Topic Card) → a notes card cycle (`-Notes`) on the book's own Cards → `zt-digest skeleton` plan → DeepSeek → apply (the Literature index and the structure Card; no Topic body). | yes (the book's own Cards) | continue |
| figures (`zt-figures`) | `zt-digest figures -Apply`: show the book's own figures (`artifact:`) in the Cards that name them; link the book's PDF and Markdown from the Literature Card. | yes | halt |
| bridge (`zt-bridge`) | `zt-digest bridge` with the profile's `bridgeProfile`: bounded plan → read-only judge jobs → immutable report (section 8). Its sources, targets and carriers are Literature-tree Cards; it runs after Figures so its endpoints are the final Card text. | no in discovery | continue; the stage shows `status: failed` |
| term-link (`zt-term-link` v2) | Delegates to the shared `zt-term-link-lib`: `zt-digest term-link -Step plan` freezes the plan and registers `termlink/book@1` (or `termlink/final@R` for a finalization round), the ledger claims it, `apply -Token -Apply` applies it, and `ledger done` completes it only from the apply report (complete, no refused Card, every chunk committed or replayed, or a proven no-op). A unit that is not ready is reported skipped with the ledger's reason, never linked. Mode `term-link` is the backfill pass of one already digested book. | yes, Term links only | continue |
| audit (`zt-audit` v4) | `zt-digest audit`, read-only, over both Workspaces: faults per rule and category, orphans, figures, broken Links, Bridge counts in and out per book, islands, and the `terms` section (coverage, T faults, uses, no-use observations, avoid words, top terms) with the run's Term generation and the check contract; it counts the box's Topic trees and hubs as information, never as a digest fault. `audit ran` is not `all terms passed`. | no | halt |
| finalize (`zt-finalize` v3) | The finite finalization: `-Step plan` reads the audit into one bounded correction cycle; `card -Step … -Cards <plan>` runs at most that one cycle; `-Step scope` records the changed Cards and touched trees; the round's Term-link pass relinks the changed Cards (or the whole book when the correction changed the Term mapping); `-Step refresh -Layer link-check` refreshes the published revision; `-Step final` runs the read-only audit again. A residual fault or incomplete Term-link round is a `partial` result, never a halted run. | via the one correction cycle and its Term-link round | continue (partial on a residual fault) |
| report (`zt-digest-report` v19) | No agent: the run's result for the mode, with each stage's status, `term_cards`/`term_link` (including the finalization round and the final audit's Term quality), and the variant's extra reports; the whole report names the `zt-idea-grow` follow-up. | no | halt |

A module that fails softly returns `{ status: 'failed', ... }` and its stage still ends; only a thrown error fails a stage. "halt" stops the run.

**Modes** (`args.mode`, default `full`) re-run base stages on an existing digest:

| Mode | Stages | Needs |
| --- | --- | --- |
| `resume` | args, prepare, terms, term-cards, digest, cards, the book type's layers, cards-book, link-prepare, link, link-check, the math tail, organize, figures, bridge, term-link, audit, finalize, the math check, cleanup, report | `out` only: the identity, source choices, Citation key, enabled layers, versions and settings come from the ledger; one supported resume per run |
| `link` | prepare, link-prepare, link, link-check, cleanup, organize (with `organize: true`) | `out`, `citationKey`, `groups` |
| `cards` | prepare, cards-book | `out`, `citationKey` |
| `figures` | figures | `out`, `citationKey` |
| `audit` | audit (one book with `citationKey`, else every book) | `out` |
| `bridge` | source, bridge (discovery); with `bridgeReport` and a selection, only apply | `out`, `citationKey` |
| `term-cards` | prepare (backfill branch), term-cards, term-link, audit, cleanup, report | `out` only plus the Term settings: the already digested book, its source, headings and limits come from the prepared run; `termCards: false` contradicts the mode |
| `term-link` | prepare (validate), term-link, audit, cleanup, report | `out` only plus the Term settings; no source stage, no Term-card branch, no provider probe; `termLink: false` contradicts the mode |

There is no mode `hubs`: `zt-idea-grow` mode `hubs` rebuilds the hubs and the register. A variant adds its own modes. **Checkpoint**: `args.until: "digest"` stops a full run after the digest, before the slot `after-digest` and the link pass.

**Args** are always an object; the kernel refuses an undeclared key. `out` is required; `source`, `bibtex`, `bookTitle` for `full`. The base's own args: sizing (`maxChars`, `maxUnits`, `dshPerBatch`, `dshManagers`, `applyShare`), the two run settings (`maxAttempts` 1–10 default 3, `dshRunCap` 60–3600 s default 1200; both are persisted in the prepared contract), DeepSeek effort (`dshEffort`: every value below `high` is raised to `high` and logged; the model and source are frozen at the official one), switches (`terms`, `cardChecks`, `organize`, `figures`, `bridge`, `audit`, `finalize`, `termCards`, `termLink`), the card cycle (`ideaSample`, `cardRules`, `cardFixRounds`, `cardMax`), the Term settings (`termCardsPerJob` 1–8 default 4, `termCardsMathPerJob` 1–8 default 1, `termLinkOccurrences` `first`|`all`), Bridge (`bridgeMax`, `bridgeCandidates`, `bridgeBooks`, `bridgeReport`, `bridgeApply`, `bridgeApplyEarlier`), and source slicing (`book`, `fileId`, `startHeading`, `endHeading`, `skipHeadings`, `groups`). `termCards`/`termLink` are frozen at preparation, so the prepared target excludes a disabled layer and no later pass waits for it. The base also declares the args its variants' model layers read (`models`, `level`, `perSonnet`, and the rest); they do nothing in the base.

## 4. What a variant fills

A variant changes no base stage. It gives:

1. **`consts.profile`**, which the base stages read:

   | Field | Read by | Example |
   | --- | --- | --- |
   | `instructions` | prepare: the digest prompt, built from `dsh-digest-base.zh.md` and the type's supplements | `"dsh-digest-base.zh.md,dsh-model-cards.zh.md"` |
   | `noEventCandidates` | prepare: no event candidates in the group files (when a layer writes the Event Cards) | `true` |
   | `digestPrefix`, `linkPrefix` | digest and link managers: the prefix files they send first | `shared-prefix.md`, `link-prefix.md` |
   | `bridgeProfile` | bridge: which source Cards to select | `book` |

   Other profile fields belong to the variant's own layers.
2. **`fill`**: the stages of each slot (`after-prepare`, `after-digest`, `after-link`, `after-audit`), with their modules, `when` conditions, params, and `after` edges.
3. Its own **modes**, args, and the `extra` reports that `report` adds to the result.

Steps to build one:

1. Write `scripts/book-digest/workflow/zt-<name>-digest.blueprint.json` with `"base": "./zt-digest-base.blueprint.json"`, the profile, and the slot fills; put new stage modules in `modules/zt-<name>-*.js` (one `defineModule` each; prefix top-level names; no clocks, randomness, or file access).
2. A new stage is a compiler (section 2): a `zt-digest` tool (`tools/zt-digest/src/<tool>.rs`) with plan, precheck, apply, check, report; a `dsh-<name>-*.zh.md` instruction file with a 自查 section; a `SELF-CHECK:` line in every job file; JSON answers applied by the tool. Add the script name to `ZD_TOOLS` in `zt-digest-lib.js` if a `.ps1` forwards to it.
3. New Card rules go into zt-check (`tools/zt-check/src/check.rs`) with a unit test.
4. Add the new read-only commands to the manager approval lines.
5. Write a sim file and checks in `scripts/book-digest/tests/framework-harness.mjs`, then install: `node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs install <blueprint> --sim <sim> --scope project --project E:\zet`, and `verify <name> --scope project --project E:\zet`.
6. Pilot on one chapter (`cardMax`), with a database backup first, before any whole-book run. A new layer writes only the book's Literature tree; a Topic-side layer belongs to `zt-idea-grow`.

A change to the base or to a shared module (`zt-digest-lib` and the base stage modules) changes every variant: raise the versions and reinstall them all.

## 5. Rules every run keeps

- **DeepSeek Subagent Managed Protocol** (global skill `deepseek-subagent-managed-protocol`): one Haiku manager per 8 DeepSeek jobs, at most 8 managers alive at once in the whole run (`dsmpSlot`), conversations deleted (never archived) after their answer is collected, a workspace removed when its work is done. Write cleanup steps as plain steps: a Haiku agent refuses deletes that cite "the user's protocol" as authority.
- **Approvals**: a manager approves only the zt helper (`zt-digest call`), `zt-check`, the read-only `-Step precheck` commands of the job's tool, read-only reads under `out` (or of `source`), and writes of the job's own JSON file. Everything else is rejected.
- **Skills are sent, not pasted**: managers pass a job's skills in the `skills` field of every `deepseek_run` and `deepseek_prompt`: `zettelkasten` and `asd-ste100-zh` for digest, link and Term-card writer runs, `asd-ste100-zh` for the card cycle, terms, skeleton, hubs and the Term-card reviewer. No prompt file carries skill text.
- **Files and state**: each compiler keeps `state.json` in its work folder (the DeepSeek cwd), so a rerun skips finished items. Every tool prints compact JSON as its last line. Job ids are stable; follow-ups continue the job's conversation.
- **The zt helper** is `zt-digest call`: a JSON array of calls on stdin (`@'…'@ | zt-digest call - …`), with `-Allow` (the Cards a run may change), `-Log` (Cards it made), `-SrcLog` (their source lines), and `-Moves` (address changes); `@@N` is the address of call N. It runs in the Literature scope: `create_topic`, a Topic-tree or Hub write, and a Link to a Topic-tree Card are refused (`literature_only`); only hand Topic work passes `-Open`, never a digest job. The old path `~\.codex\skills\zt\scripts\zt-mcp.ps1` forwards to it.
- **Source fidelity**: a step or explanation the book does not give carries `（补：书中略去）` or `（补：书中未写）`. The book's modality stays.

## 6. Quality checks

`zt-check` is the **one checker and the only source of truth** for Card quality (`E:\zet\tools\zt-check`, installed beside `zt.exe` by `scripts\install-zt-check.ps1`, no service restart). Every agent calls it directly; the `zt-digest` tools run it in-process.

```
zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,...]   # a writer's own Cards, classified as the audit does
zt-check --addresses <a,b,...>
zt-check --json <file>                                                      # drafts, offline
zt-check --draft <file> [--workspace human|agent] [--report <file>]         # exact proposed Cards with their type fields
```

Output: one line per fault `<address> <rule> <text>`, then `faults: N`; an advisory line carries `(advisory)` and `faults:` counts blocking diagnostics only. A run that prints no `faults:` line did not run, and nothing may be written on it. Exit codes: 0 complete with no blocking fault, 1 complete with a blocking fault, 2 invalid invocation, missing context, or a stale/incompatible check contract. A live check compares the running service's published `rule_version`, diagnostic schema, applicability version and Term capabilities with its own; a mismatch is `stale` (exit 2), never a successful acceptance prediction. A draft without an archive says its policy is the built-in default. Rules: R2 to R21 (Card language, size, exact corruption, and the standalone heuristics R20/R21), the Card layer rules I, E, F, L (Idea Cards, Event Cards, Finding Cards, Links; I7 covers a Topic Idea Card resting on its own source's Literature tree), and the Term rules T1–T10.

R20 and R21 are **advice in the pilot** (`(advisory)`): a local label that stands in for its content (`式 (12) 有该项`), and a deictic with no named Link in its sentence (`该卡`, `那里`). They name the fragment and the text; a semantic review decides. They are counted apart from blocking faults by `zt-check --cards` and by `zt-digest audit` (`style.advisory`). I7's unknown-source line is advice too. The writer guidance is the Card prompt's 改写规则 (R20, R21, I7), and the replacement of `依赖` by `对应` to silence a rule is never a repair. The pilot's outcome target is **under 30% of new Topic Idea Cards with an own-source `rests-on` dependency**; it is a threshold the audit measures for a run, never permission to misfile an individual Card.

I6 (**similar titles**, advice) now compares a checked Idea title with every title of its own Workspace, not only its tree: candidates come from a bigram index, the existing similarity decides, and only pairs touching the checked Cards are reported. It is advice like R20 and R21: a similar or copied title never merges, deletes, or rewrites either Card. The reviewer decides whether two Cards state one claim (then merge or continue the existing Card) or two claims (then the titles must show the difference).

**T1–T10 check a Term Card's authored template**, on the rendered draft and on the stored Card alike: the known ordered head lines and sections, the title/name/table-row identity, the mathematical content a math head needs, symbol-table coverage, numbered derivation steps with a basis and a result, no unexpanded skipped step, Term-link target and label agreement, one head per row, the measured prose size, and balanced formulas. A role mismatch (a Term part filed outside its head's chain, a duplicate head for one row, an unfiled Card with the list's name) is a placement fault; a label that differs from the head's name is advice, never rewritten. A Term use is no reasoning Link: it changes no L rule, no Model membership and no reasoning credit, and the same body with and without its Term macros produces the same prose diagnostics.

**The gate and its refusals.** Every write route makes one check inside its write transaction. `zt config policy` prints the stage, policy hash, revision, rule/diagnostic versions and every effective rule; `zt config set gate.<workspace>.<rule> block|advise|off|default` changes one rule for one workspace, and `zt config set gate.stage advisory|enforce|default` changes the stage. A fresh archive is `advisory`: every quality fault is returned as advice and the Card is stored with it. `enforce` lets a `block` rule refuse; a retained legacy fault stays advice while a new occurrence blocks. The mandatory envelope, Link, Access and Lock rules refuse in both stages. A refused write is typed `card_check` (content: fix it inside the finite fix cycle, never retry it unchanged) or `card_locked` (a planning stop: keep the drafts and receipts, never retry, unlock or re-point). An accepted write's warnings are not errors.

The SELF-CHECK of each base task:

| Task | SELF-CHECK |
| --- | --- |
| Digest run | `zt-check --cards <anchor> --log <owned> --book <key>` (in the group file) |
| Link run | `zt-digest link-finish -Out <out> -Group <n> -Token <n> -Draft <file> -CheckOnly` (binding, gap coverage, `L1,L2`) |
| Card cycle fix and orphan | `zt-digest card -Step precheck -Work <work> -Job <id>` |
| Skeleton | `zt-digest skeleton -Step apply -Staging <out> -Work <work>` without `-Apply` |
| Bridge judge | `zt-digest bridge -Step precheck -Staging <out> -CitationKey <key> -Work <work> -Job <id>` |
| Term-card writer | `zt-digest term-cards -Step check -Out <out> -Work <work> -Job <id>` (schema, T1–T10, the shared check) |
| Term-card reviewer | `zt-digest term-cards -Step review-plan -Out <out> -Work <work> -Job <id>` (a fresh independent review, bound to the exact draft and plan revision) |
| Term link | `zt-digest term-link -Step plan -Book <key> -Staging <out>` then `apply -Cards <plan>` (preservation and idempotence; a second plan has no edit) |

## 7. Operating rules

- **Back up before a run that changes Cards**: `node -e "const {DatabaseSync}=require('node:sqlite'); new DatabaseSync('D:/zt/zt.sqlite3',{readOnly:true}).exec(\"VACUUM INTO 'D:/zt/backups/<name>'\")"` (the archive is in WAL mode; a file copy misses changes). Check `pragma integrity_check`.
- **Run an installed workflow by `scriptPath`** (`E:\zet\.claude\workflows\<name>.js`); named workflows load when the session starts. Never edit an installed script: change the blueprint or module and reinstall.
- **Ask the user before the next stage of a long digest.**
- **Watch a run with one filter, not with the session**: start `node scripts\book-digest\watch-run.mjs <run journal.jsonl> <out>` once beside the Workflow. It prints only ABNORMAL, FAILED, EXPIRED, STALL, a SUMMARY every 30 minutes and END, writes every agent to `<out>\watch\progress.log`, and never reports a line twice when started again. Do repairs in their own session (a fork or a fresh one with a short brief), so the watching session stays small (`docs/digest-claude-token-proposal.md` P9/P10).
- **A block's unit finishes are one command** (base 5.2.0): `zt-digest finish-batch -Out <out> -Units "<unit>:<token>,…"` runs each unit's own verifier and records a unit's fault against that unit alone; by hand, use it the same way to finish claimed units after a repair.
- **Do not deploy zt during a run** (`zt down` kills the helper). `zt-check` and `zt-digest` install without a restart.
- **Build on D:** (E: is nearly full): zt with `CARGO_TARGET_DIR=D:/zt-target`, zt-check with `D:/zt-check-target`, zt-digest with `D:/zt-digest-target` (`scripts\install-zt-digest.ps1`, run from `E:\zet`: job files name paths of the repository the binary was built from).
- **Tests**: `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 4` runs every test file and harness in parallel; `cargo test --release` in `tools/zt-digest` and `tools/zt-check`.
- **Script traps**: JS `String.replace` with a string expands `$'`; `split(a).join(b)` needs a string `b`; a Bash heredoc loses backslashes (write files with the editor).

## 8. Cross-book Bridge contract

The base stage runs after Figures; the Term-link pass follows the Bridge and the Term-card branch, and the audit follows it. It links Literature trees only. Full mode discovers and reports; `bridge: false` returns skipped. Mode `bridge` needs only `out` and `citationKey`. `bridgeMax` defaults to 60 (integer 1–120), `bridgeCandidates` to 5 (integer 1–8, plus at most two literal term matches), and `bridgeBooks` optionally filters exact agent book keys. The variant's `bridgeProfile` picks which source Cards are selected. Retrieval is bounded and can miss relationships expressed deep in a body.

Report-only: use a fresh output folder, inspect `bridge/report.md` and immutable `bridge/proposals.json`. The workflow publishes the report only after every judge job answered. When the plan or a judge job fails, nothing is published: run mode `bridge` again with the same `out` to resume. The cleanup deletes every DeepSeek judge conversation and the DeepSeek workspace, never archives, and always runs, also after a throw. Proposal ids are `b001`, `b002`, and so on. Apply: use that same output folder with `mode: "bridge"`, `bridgeReport: "<actual report_id>"`, and `bridgeApply: ["<actual bNNN>"]` or `"all"`. `bridgeApplyEarlier` independently selects earlier carriers; earlier-only is valid. Empty arrays grant no edits; booleans, mixed arrays, duplicates, wrong sections, and unknown ids fail. Apply has no source probe, judge, planning, Organize, or Hubs.

A judge loads `zettelkasten` and `asd-ste100-zh`, reads only frozen candidates, writes its assigned answer, and runs its exact precheck up to three times. Its manager approves only the Bridge precheck, `zt-check`, the zt helper with the empty `read-only.allow.txt`, reads under `out`, and JSON writes under `bridge\out`. A judge never writes, splits, or moves Cards, and never grants acceptance. One source has at most two proposals, and one pair of Cards has at most one relation (the reverse of a proposed pair is refused). A Link names a whole Card. Human Cards are read-only. The tool checks BR1 to BR9 and the Card rules before a report and before each accepted edit.

Before the first live edit, get the user's actual selections and make a consistent archive backup (`zt dp`, or the SQLite `VACUUM INTO` backup with an integrity check). Keep filing quiet: no insert, move, delete, key rename, retype, or Organize alongside apply. The tool uses the raw `edit_card` base, per-report locking, and durable per-carrier intents. A stale report needs a fresh folder and a new review. Retries keep accepted text, verify persisted effects, and never restore an archive on their own. Keep old frozen workflow versions and reports.

