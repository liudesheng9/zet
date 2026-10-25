---
block: idea-grow-workflow
title: Idea-grow workflow
paths:
  - "scripts/idea-grow/**"
  - ".claude/workflows/zt-idea-grow.js"
  - ".claude/workflow-framework/zt-idea-grow/**"
  - "!.claude/workflow-framework/*/history/**"
synced: 93e7b921af82
updated: 2026-10-25
---

# Idea-grow workflow

## Purpose

`zt-idea-grow` grows the agent **Topic trees** from the **Literature trees of the whole box**. By default it reads every Literature tree, in either Workspace. The more books the box holds, the more each idea can relate to, and one Topic gathers a concept from several sources. The user asked on 2026-10-20 that it not be locked on one Literature tree. The digests build a book's Literature tree, like Luhmann's bibliographic box. This workflow is the second act: it reads each section again against the whole box, keeps the ideas worth keeping, and writes them as standalone Topic-tree Cards in the reader's own words, each filed beside the Card it continues (the main box). The user asked for it on 2026-10-20. They asked that the Topic proposals (`docs/topic-tree-ideas-proposals.md`, P1–P7) live here and not in the digest workflows. Design: `docs/idea-grow-design.md`.

Since 3.0.0 this is the **only** workflow that writes Topic trees, Topic Cards, Hub Cards and the concept register: the owner moved the idea compose, the Topic-tree compose and the hub compose out of every digest (`docs/literature-only-digest-design.md`). The digests ([Digest workflows](digest-workflows.md)) build only Literature trees and refuse any Topic, hub or Topic-Link write in their Literature scope. So run this workflow after a digest to give the new book its Topic Cards, and its mode `hubs` to rebuild the hubs and the register without new Cards.

## At a glance

| | |
|---|---|
| Installed script | `.claude/workflows/zt-idea-grow.js` (project scope, frozen; registry `.claude/workflow-framework/zt-idea-grow/`) |
| Blueprint | `scripts/idea-grow/workflow/zt-idea-grow.blueprint.json` (4.0.1), dry run `zt-idea-grow.sim.json` |
| Modes | full (every stage: grow, report) and `hubs` (the concept review, the hubs round and the register alone) |
| Module | `scripts/idea-grow/workflow/modules/zt-grow-cycle.js` (v12), and the shared library `scripts/workflow-shared/modules/zt-workflow-lib.js` (v1: the Lock reader and the relay the digests use too) |
| Prompt | `scripts/idea-grow/dsh-grow-instructions.zh.md`, read from disk by `-Step plan` |
| Compiler | `zt-digest grow` (`tools/zt-digest/src/grow.rs`; the run as one command, `-Step next` and `-Step status`, in `grow_next.rs`; block [zt-digest](zt-digest.md)) |
| Writes | agent Topic trees, their Hub Cards and the register; never a Literature-tree Card or a human Card. Only the optional Topic maintenance round (`organize: true`) moves agent Topic Cards and deletes an emptied agent Topic root |

## Concepts

- **The box is the unit**: `-Books a,b`, `-Book a`, or nothing (every Literature tree of the box). Sections get global numbers across the books, and each records its `book`. Every check (G3, G4, the source line, zt-check `--book`, G11) uses the section's own book.
- **Section**: one anchor of a Literature tree (`Key/n`) with its whole subtree (Idea, Model, Event, Finding Cards). A tree with no anchor (a hand-written Literature Card with a page index) is one section. Each section is one DeepSeek job. Its file also lists the 15 notes of other books closest by title ("其他文献里用词接近的笔记"), so an idea can contrast, confirm or generalize what another source says.
- **Bridges**: the run's Topic Cards that cite two or more Literature trees. The report counts them.
- **Idea kinds**: `核心` (core claim or mechanism), `关联` (a real relation to a Card already in the box), `有用` (a method, criterion or condition usable elsewhere), `有趣` (surprising, intriguing, or a question worth asking), `直觉` (why a result holds, without formulas).
- **Two boxes** (P1): the Literature tree keeps the book's own apparatus. A claim stated without the book is a Topic Card. It cites its source note with `例示`/`对应`/`对照` and never rests on its own book (`依赖 [[Key/…]]`, I7, G4).
- **Run Topics** (P2): before any section, one DeepSeek job (gt-001) reads every section's anchor and the Topic catalogue. It picks the few concepts this run files under: an existing agent Topic (`existing`), or a new one. At most max(2, ⌈sections/2⌉) new Topics, and never more than 8. `-Step topics` makes them, and every section job lists them.
- **Link kinds**: `grow.rs` reads each Link's kind as zt does: a port of `RELATION_KINDS` and `read_links` in `src/model.rs`. The cue words come from the sentence start or the previous Link, and a Link with no cue shares the sentence's earlier kind. `承接` and `依据` are rests-on. `引出`, `见` and `由…可得` give a plain reference.
- **Placement** (P2, P3, P6): `same` (an existing Card states it: append a sentence, or record it for a human Card); `at` an agent Topic Card (Direct successor = the next step, Side successor = a branch), with a first sentence that links it with a kind word (G6); `at` an earlier idea (a chain); `at` a Topic root (the doorway `n/1` when the Topic is empty, else a new line beside it: `n/1|a`, `n/1|b` …, in which the section chains its ideas; one root placement per section and Topic, G13; a doorway with 7 lines sends the next one beside its last line); or a new Topic (`T1`) through the topic gate.
- **Link round**: after the sections are written, `-Step links` writes one job per Topic tree the run wrote in (`gl-NNN`). Each job lists that tree's new Cards, the run's new Cards in the other trees, and the 20 box Topic Cards closest by title. DeepSeek proposes Links to Cards in other trees: at most 2 per Card, each one sentence with its kind (G11). `apply` writes each sentence into its Card before the source line (`before_source`, an edit with `base`) and skips a Link the Card already has. A job offers only candidates that share wording with the tree's new Cards (at least 2 title bigrams). A tree with no candidate gets no job.
- **Topic share** (P5): the distinct Topic-tree Cards (either Workspace: the user's own Topic Cards count too) that link the book, divided by those Cards plus the book's Literature-tree Idea Cards (root and anchors left out). It is measured before and after the run.
- **Locked trees** (S15): the owner may lock one whole Topic tree. A Locked source book is read normally — a Lock never refuses a grow run. A Locked Topic is related context and a Link target only. The plan keeps each Card's Lock facts in `titles.json`, prints `locked` and marks the tree in `topics.md` (`已锁定：只读，可链接，不能写入`); the section file lists its Cards apart (`## 用词最接近的已锁定主题卡（只读：可作相关语境，可链接，不能写入）`). The precheck and `topics` report G12 when an answer names a Locked destination, so the answer names another one: the tool never substitutes a tree or a placement. If a Topic locks after its answers, the archive's own refusal stops `apply`: the job keeps its answers and receipts, the stage prints the typed `card_locked` JSON and exits 2, and `grow-report.md` names the stop; the same `out` resumes after the owner unlocks.

### The hubs round (2.5.0)

The grown Topics need ways in, like Luhmann's structure Cards and keyword register: a topic hub for each large Topic tree, a line hub for each long branch, field hubs over related Topics, and register terms that point at the Cards carrying them. After the Cards are written and checked, the module runs:

1. `grow -Step hubs-plan`: the hub compiler's survey and plan (`zt-digest hub`, run as a child process in `<out>\hubs`). The limits come from the survey by the digest's hub-manager rules (`hub_limits`), and each hub job is wrapped as a grow job `gh-NNN`: the hub instructions `dsh-hub-instructions.zh.md`, then the hub task with its own SELF-CHECK lines. It prints only counts.
2. DeepSeek writes each hub as JSON. Its SELF-CHECK, `grow -Step precheck -Job gh-NNN`, runs the hub compiler's precheck on every file the job writes and sums the faults; a missing file is a fault.
3. `grow -Step hubs-apply` (the hub compiler's apply), then up to `hubFixRounds` (default 1) of `grow -Step hubs-check`. The check reads `get_hub` on every hub, and each writer with faults gets a fix job `gh<round>-NNN` (the instructions, its original task, the faults) and another apply.
4. `grow -Step hubs-report -Register apply|plan|off`: the hub report. With `apply` (the default) it also rebuilds the register from the box's term tables (`zt-digest register`, in `<out>\register`), publishes it when its plan has no fault, and reads it back.

Since 3.0.0 the round takes the hub options the digests had: `hubTrees` (only these Topic trees, a pilot) and `hubRebuild` reach the hub plan as `-Trees` and `-Rebuild`; `hubLimits` (an object) replaces the survey's limits key by key (`grow -Step hubs-plan -Limits <json>`; `merge_limits` refuses an unknown or non-numeric key, and the hub compiler clamps them again); the gh jobs run at `hubEffort`. In mode `hubs` the module runs only this round (on a fresh work folder, which `Work::open` makes) and returns `{mode: "hubs", report: {hubs, faults, missing, register}}`.

The hub and register compilers are lock-aware: a typed `card_locked` refusal stops the step with exit 2. The round runs only on the live box (`ztConfig` empty), because the hub jobs' helper lines name no zt config; `hubs: false` skips it. A rerun with a work folder whose sections are all applied runs only the check, the hubs round and the report: that is how an existing box gets its hubs.

### The restatement review (3.0.0)

After the check/fix rounds and before the hubs, each new Topic Card is compared with its source book's anchors and Model Cards and with the earlier Topic Idea Cards (the restatement review that the digests ran before 3.0.0, `zt-digest restate`, block [zt-digest](zt-digest.md)):

1. `grow -Step restate-cards` writes one staging folder per source book, `<out>\restate\rNN`, with `manifest.json` (`citation_key`) and `cards.txt` (the run's new Topic Cards from that book, sorted, at most 200: `grown_by_book`), and prints `{"books": N, "reviews": [{id, book, staging, cards}]}`.
2. Per review (since 3.2.0 in three phases: every book's prepare, then all the books' `rNN-rj` judge jobs in one DeepSeek dispatch, side by side under the DSMP limits, then each book's judge, apply and report in the books' order): `zt-digest restate -Step prepare -Staging <rNN> -Work <rNN>\work -Cards <rNN>\cards.txt` freezes the evidence and prints one judge job. One DeepSeek job `rNN-rj` answers (expected: a JSON line); it is approved only for the judge self-check, reads under `out` and JSON writes under `<rNN>\work\out`. Then `judge -File <answer>`, `apply -OperationPrefix grow-<folder>/restate-rNN/1/apply` (it rewrites only the new Card) and `report`.
3. A review that fails for one book is recorded for that book (`status`; a failed prepare is `skipped` and sends no job) and the run goes on; `restate: false` skips the round. The result carries `restate: {books, reviews: [{id, book, cards, status, report_id, resolved_by_rewrite, pending, unresolved, written}]}`.

### The Topic maintenance round (3.1.0, optional)

The old digest Organize merged duplicate Topics and moved misplaced Topic Cards (an Opus decision carried out by `organize-apply`). Since 3.0.0 a digest writes no Topic, so this round does it for the run's Topics, after the restatement review and before the concept review and the hubs. It runs only with `organize: true`, in mode full, on the live box:

1. `grow -Step organize-plan [-Trees a,b]` surveys the Topics (`organizeTrees`, else the trees the run wrote in and the run's Topics) with `zt-digest organize-survey -Work <out>\organize -Topics <list>` (block [zt-digest](zt-digest.md)): duplicate candidates among all agent Topics (title bigrams ≥ 0.5, one title inside the other, or 4 Links between them), small Topics (≤ 3 Cards), move candidates (a Card that links one other Topic at least twice and more than its own), the Locked Topics, and every agent Topic-tree Card in `survey.json`. When the survey lists a candidate it writes the decision job `jobs\go-001.md` (the rules, the read-only helper, the answer format, then `survey.md`) and prints `{"organize_jobs": 0|1, "topics", "pairs", "small", "moves", "locked"}`.
2. One DeepSeek job go-001 writes `out\go-001.json`: `{"merges": [{drop, keep, under?, reason}], "moves": [{at, under, reason}]}`, at most 10 merges and 30 moves. Its SELF-CHECK, `grow -Step precheck -Job go-001`, checks it against the survey: O1 shape and reasons, O2 an unknown Card, O3 not a Topic root or not a surveyed Topic, O4 a Card that never moves (a root, a Hub, a Model Card or part), O5 a Locked Topic, O6 a destination outside the kept Topic or inside the moved Card, O7 Card notes (idea-grow hands out no notes), O8 a Topic dropped twice or both dropped and kept.
3. `grow -Step organize-apply` checks the answer again, drops each decision with a fault, writes the rest to `<out>\organize\decisions.json` and runs `zt-digest organize-apply -Work <out>\organize -Open` (the open scope: the digest's Literature scope refuses it). That tool plans every move with the archive first, so a Lock, a pin or a Link it would break blocks the decision instead of failing the run; a Lock that appears between plan and write stops the step with the typed `card_locked` JSON (exit 2), and the stage stops with it. The run's own addresses then follow the moves (`renamed`, `rename_state`: `state.json` Card keys and each section's created Cards), so the check, the hubs and the report read the Cards where they now are. A missing answer applies nothing.

The result carries `organize: {plan, applied}`; `applied` has `merges`, `moves`, `refused`, `blocked`, `renamed`, `complete`, `invalid` (decisions the check dropped) and `run_cards_renamed`.

### The concept review (3.1.0)

Before 3.0.0 each digest reviewed its book's term table against the Topics (`zt-digest concepts`) and its hub stage read that report as the register plan's `-Review`. Now each source book gets its own review here, right before the hubs round, in mode full and in mode hubs (`concepts`, default true; it needs the hubs round and a register, so `hubs: false` or `register: "off"` skips it):

1. `grow -Step concept-books [-Book k | -Books a,b]` picks the books: the named ones (mode hubs passes `book`/`books`), else the books the run grew Cards from (`grown_by_book`), else every agent book with a term table; at most 24. A book whose agent Literature tree has no term-table Card (title `术语表…`, the first by address: `term_tables`) is skipped with its reason. It removes `<out>\concepts` and writes one staging folder per book, `<out>\concepts\cNN`, with `manifest.json` (`citation_key`, `book_title`) and `terms.md` (the term-table Card's body), and records them in `state.json`.
2. Per book (since 3.2.0 every book's propose and prepare first, then all the `cNN-cj` judge jobs in one DeepSeek dispatch, then each book's judge and report): `zt-digest concepts -Step propose -Staging <cNN> -Work <cNN>\work` (no proposal: the review is `skipped`, no job), `prepare` (the frozen evidence and one judge job), one DeepSeek job `cNN-cj` (skills `asd-ste100-zh`, expected a JSON line; approved only for the `concepts -Step judge` self-check, reads under `out` and JSON writes under `<cNN>\work\out`), `judge -File <answer>` and `report`, which renders `<cNN>\concept-review.json` from the frozen files alone. A review that fails for one book is reported for that book and the run goes on.
3. When a report was rendered, the hubs round ends with `grow -Step hubs-report -Concepts`: the register plan gets every rendered report as `-Review a.json;b.json` (`zt-digest register` combines several reviews: `merge_reviews`), and the register line says how many it read (`reviews`).

The result carries `concepts: {books, skipped, reviews: [{id, book, status, report_id, findings, judged, unjudged}]}`.

## Architecture

Since 4.0.0 (`docs/workflow-simplicity-proposal.md` S3) the stage is one loop. `zt-digest grow -Step next` runs every deterministic step below in this order, stops at each DeepSeek job set, and prints one line: `{"next":"jobs",…}` (the module hands the jobs to DSMP `dsDispatch` and runs `next -After <token>`), `{"next":"done",…}` (the stage's report) or `{"next":"fault",…}` (the stage stops). The order of the rounds lives in `grow_next.rs` (`rounds()`); the module decides nothing.

```
args → grow (zt-grow-cycle: loop over grow -Step next) → report (value)
grow: zt-digest grow plan → DeepSeek gt-001 (the run's Topics) → grow topics → DeepSeek gw-001..N (SELF-CHECK = grow precheck) → apply → grow links → DeepSeek gl-001..M → apply → restate-cards → [restate prepare → DeepSeek rNN-rj → judge → apply → report] per book → check → DeepSeek fx1-001..M → apply (4.0.1: after the restatement review) → (organize: organize-plan → DeepSeek go-001 → organize-apply) → concept-books → [concepts propose → prepare → DeepSeek cNN-cj → judge → report] per book → hubs-plan → DeepSeek gh-001..K → hubs-apply → [hubs-check → DeepSeek gh<R>-NNN → hubs-apply] × hubFixRounds → hubs-report -Concepts (register) → report → DeepSeek cleanup
mode hubs, grow: concept-books → [concepts … report] per book → hubs-plan → DeepSeek gh-001..K → hubs-apply → [hubs-check → fix] × hubFixRounds → hubs-report -Concepts (register) → DeepSeek cleanup
```

A jobs line carries the round, a token, the job ids, one job template (`cwd`, `files` and `prompt` with `{id}`, `skills`, `model`, `effort`, `expect`, the approval rule) and `sum`, an FNV-1a checksum over the ids and the template; the module checks the sum before it dispatches, so a line a relay cut or rewrote is never dispatched (it is read again from `grow -Step status`, once). The relay (Haiku, the `zt-relay` agent type) runs each `zt-digest grow` command. DSMP managers (Haiku) run the DeepSeek jobs at most 8 per manager and 8 managers at once, and approve only what each template's rule names, for a section job:
- the read-only helper (`zt-digest call - -Allow <work>\read-only.allow.txt`, an empty allow-list);
- the precheck;
- reads under the work folder;
- JSON writes under `out\`.

## Interfaces

Args (`Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-idea-grow.js", args })`):

| Arg | Type | Default | Meaning |
|---|---|---|---|
| `books` | array | the whole box | Citation keys of the Literature trees to read (either Workspace) |
| `book` | string | — | one Citation key (instead of `books`) |
| `out` | string | required | work folder; also the DeepSeek workspace |
| `from`, `to` | number | all | anchor range, applied in every book; at most 120 sections per run, the rest `left` for a rerun with the same `out` |
| `ztConfig` | string | (the live box) | the `ZT_CONFIG_DIR` of the box to grow. A copy of the box runs its own zt service; the first `next` writes the box pin `zt-box.txt` into `out`, so every step and every DeepSeek job of the run reads and writes only that copy, and every round runs there (4.0.0) |
| `tool` | string | `zt-digest` | the zt-digest command, or the path of a zt-digest binary (a development build, so the installed one is never replaced under another run) |
| `effort` | high, max | high | DeepSeek effort of every job, the hub jobs included (the model is frozen: `consts.model`, the official source) |
| `hubs` | boolean | true | run the hubs round (mode full) |
| `register` | apply, off | apply | publish the rebuilt register, or leave it |
| `hubTrees` | string | all | only these Topic trees get hubs (comma-separated Topic numbers; a pilot) |
| `hubRebuild` | boolean | false | rewrite every hub |
| `hubLimits` | object | the survey's | hub limits that replace the survey's, key by key |
| `restate` | boolean | true | the restatement review of the run's new Topic Cards |
| `concepts` | boolean | true | the concept review of each source book's term table, read by the register plan as `-Review` |
| `organize` | boolean | false | the Topic maintenance round (duplicate-Topic merges, misplaced-Card moves) before the hubs |

4.0.0 froze `fixRounds` and `hubFixRounds` at 1, merged `hubEffort` into `effort`, froze `dshModel`, and dropped `register: plan` and `organizeTrees` (no run used them; proposal S9). A rerun with the same `out` and the same args resumes a stopped run; other args are refused (`options_changed`).

Mode `hubs` (`args.mode: "hubs"`) takes `out` (a fresh work folder), the hub args, and optionally `book` or `books` (the books whose concept review feeds the register; without them every agent book with a term table, at most 24); since 4.0.0 it also runs on a box copy (the pin).

Result (4.0.0): `{ book, books, out, mode, rounds, report, workspaces }`. `rounds` lists each dispatched job set (`round`, `jobs`, `ok`, `failed`). `report` is the done line of `-Step next`: in mode full `created`, `topics`, `linked`, `same`, `refused`, `duplicates`, `stopped`, `deeper`, `faults_left`, `share_before`, `share_after`, `books`, `bridges`, `sections`, `left` and `report` (the path of `grow-report.md`); and, for the rounds that ran, `restate` (`books`, `resolved`, `pending`, `unresolved`), `concepts` (`books`, `findings`, `judged`), `organize` (`merges`, `moves`, `refused`, `invalid`), `hubs`, `hubs_missing`, `register` and `register_entries`. The per-book review results are in `next.json` and in `grow-report.md` (`## 复核与整理`).

Module `zt-grow-cycle` v12 params: `book`, `books`, `work`, `instructions`, `tool` (default `zt-digest`), `ztConfig`, `from`, `to`, `model`, `effort` (high/max), `hubs`, `register`, `mode`, `hubTrees`, `hubRebuild`, `hubLimits`, `restate`, `concepts`, `organize`, `relay` (the role, default `relay`).

`zt-digest grow -Step next -Work <out> -Options <json> [-After <token>]` (4.0.0): the options are the module's (`mode`, `books`, `book`, `instructions`, `tool`, `zt_config`, `model`, `effort`, `restate`, `concepts`, `organize`, `hubs`, `register`, `hub_trees`, `hub_rebuild`, `hub_limits`, `from`, `to`), frozen in `next.json` at the first call. `-Step status -Work <out>` prints the last line `next` printed. Both exit 0 for every printed line. Fault codes: `card_locked` (with `tree`, `lock_id`), `step_failed` (a child step's exit and tail), `options_changed`, `bad_options`.

`zt-digest grow` steps added in 3.1.0: `concept-books [-Book|-Books]`, `organize-plan [-Trees]`, `organize-apply` (exit 2 on a Lock stop), `precheck -Job go-NNN`, and the `-Concepts` flag of `hubs-report`.

## Behaviour

0. **next** (4.0.0) runs the steps below as child processes of the same binary, in order, recording each round done in `next.json` with its line. A round that needs DeepSeek prints its job set and stops; the next call with `-After` confirms it. A call without `-After` (a rerun after a stop) prints the pending set again for the jobs whose answer file is missing, under a new token. The restatement and concept rounds record a failed book and go on; any other failed step prints a `step_failed` fault; a Lock prints `card_locked`. Before the report, the review rounds' results go into `state.json` (`rounds`), so `grow-report.md` renders them.
1. **plan** reads the Literature tree and the box, and writes:
   - the section files and the job files;
   - `snapshot.md` and `titles.json` (every title in the box);
   - `topics.md` (the Topic catalogue);
   - `instructions.zh.md` (the template with `{{TITLE}}`, `{{KEY}}`, `{{WORK}}`, `{{SNAPSHOT}}`, `{{TOPICS}}`, `{{ALLOW}}` and `{{MAX}}` filled in);
   - `baseline.json`.

   Each section file lists, per note, the Topic Cards already linking it (parsed from the stored text's Reverse links), and the 15 existing Topic Cards closest by title bigrams (at least 2 shared and 20% of the title). An applied section is not planned again, and the baseline is kept after the first apply.

   It also writes the Topic-planning job `jobs\gt-001.md`: each pending section's anchor with its `§` index lines, the Topic catalogue, and the bound on new Topics. It is written only once per folder: a rerun keeps the recorded run Topics.
1. **gt-001** (DeepSeek) answers `{"topics":[{id, existing | title+body, sections}]}`. Its precheck checks the ids, that `existing` is an agent Topic root, that no title is near an agent Topic or another new Topic (G8), that the sections exist (G2), and the bound (G1).
1. **topics** (`-Apply`) makes each valid new Topic through the topic gate (a `topic exists` refusal reuses that Topic), records `state.run_topics`, and writes the section jobs with a "本次运行的主题" block. G8 in the section answers compares the run Topics too.
2. **DeepSeek** reads the job file (the section is inline), greps the snapshot, and can read Cards through the read-only helper. It writes `out\gw-NNN.json` and runs its SELF-CHECK until `faults: 0`.
3. **apply** runs in job order:
   - It validates each answer again and refuses each idea with a fault.
   - It creates a new Topic only when an idea uses it. When the gate answers `topic exists: X/0`, the idea is filed beside X's doorway.
   - The first idea of a new Topic becomes its doorway (`n/1`).
   - A Topic root as `at`: with no doorway yet the idea becomes `n/1`. Otherwise it opens a line beside `n/1`. When `n/1` already has 7 lines (`MAX_LINES`), it goes beside the last line instead (`n/1|g|a`, `line_parent`), its receipt records `full`, and the report lists it under `## 入口卡已满` and counts `deeper`.
   - A Direct successor whose slot is taken becomes a Side successor.
   - It resolves `[[@I1]]` and `[[@T1]]`, and appends `来源：[[Key]]` when the body has no source line.
   - A title near one this run already wrote is a duplicate and is not written.
   - A fix edit carries the Card's stored text as `base`.
4. **check** runs zt-check `--cards` with `--book` (the Card layer rules I1–I7 and L1–L2 as well as the prose rules) on every Card the run wrote. Each hard fault becomes a fix job for its section; advisory lines are ignored.
5. **report** writes `grow-report.json` and `grow-report.md`, then prints the short line.

## Data and storage

The work folder (`out`) holds everything:

- the plan files: `plan.json`, `titles.json`, `snapshot.md`, `topics.md`, `instructions.zh.md`, `read-only.allow.txt`, `baseline.json`;
- the job and answer files: `sections\sNNN.md`, `jobs\gw-NNN.md` and `jobs\fxR-NNN.md`, `out\*.json`, `drafts-*.json`;
- the run's record: `state.json` (`sections`, with per section `created`, `topics`, `same`, `refused`, `duplicates` and `kinds`; `cards`, every Card the run wrote; `fixes`; `checked`; since 3.1.0 `organize` (the surveyed Topics, the survey line, the applied summary and the dropped decisions) and `concepts` (the staged reviews and the skipped books));
- the rounds' folders: `restate\rNN\`, `organize\` (`survey.md`, `survey.json`, `decisions.json`, `applied.json`, `moves.jsonl`), `concepts\cNN\` (`manifest.json`, `terms.md`, `work\`, `concept-review.json`, `concept-review.md`), `hubs\`, `register\`;
- the report: `grow-report.json` and `grow-report.md`.

- since 4.0.0, `next.json` (the frozen options, the rounds done with their lines, the pending job set, the last line) and, on a box copy, `zt-box.txt` (the box pin).

A rerun with the same folder and args resumes: the rounds done are skipped, and applied sections are skipped.

## Configuration

`consts.instructions` is the prompt template path (`E:\zet\scripts\idea-grow\dsh-grow-instructions.zh.md`). `-Step plan` also finds it beside `scripts\book-digest` when `-Instructions` is not given. The bounds are in `grow.rs`: 12 ideas per section, 120 jobs per run, title nearness 0.8 (the topic gate's Dice coefficient), Literature bodies cut to 1200 characters in a section file.

## Usage

On a copy of the box (the way the idea-grow work is developed while a digest runs on the live box):

```powershell
zt down; Copy-Item D:\zt\zt.sqlite3, D:\zt\artifacts D:\zt-grow-box\archive -Recurse; zt up
$env:ZT_CONFIG_DIR = "D:\zt-grow-box\config"; $env:ZT_GUI_PORT = "0"
zt config set archive_root D:\zt-grow-box\archive; zt up
```

```javascript
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-idea-grow.js",
  args: { out: "D:\\zt-grow\\box-pilot5", ztConfig: "D:\\zt-grow-box\\config", tool: "D:\\zt-grow-target\\debug\\zt-digest.exe" } })
```

```javascript
// A pilot on one finished Literature tree; back up the archive first (zt dp).
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-idea-grow.js",
  args: { book: "grossman2002integration", out: "D:\\zt-grow\\grossman2002integration-pilot" } })
```

Run it only when no digest is writing the same archive. It builds the hubs and the register itself; to rebuild them alone, run mode `hubs`:

```javascript
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-idea-grow.js",
  args: { mode: "hubs", out: "D:\\zt-grow\\hubs-2026-10-22", hubRebuild: true } })
```

With the Topic maintenance round (3.1.0; it merges and moves agent Topic Cards, so back up the archive first):

```javascript
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-idea-grow.js",
  args: { book: "grossman2002integration", out: "D:\\zt-grow\\grossman-organize", organize: true } })
```

## Testing

- 4.0.0: `cargo test --bin zt-digest grow_next` (5 tests: the rounds per options and their order, the job templates and answer files, the options' checks and canonical form, the pending job set confirmed or printed again, and the checksum, pinned to the value the JavaScript driver computes). `grow.tests.ps1` runs `grow -Step next` on a scratch archive to the end with canned answers: the first job set, the checksum computed a third way (PowerShell), the box pin, `status`, a rerun before the answers (the same set under a new token), a rerun with other options (`options_changed`), the done line with the restatement review, `next.json`'s rounds, a finished run that redoes nothing, and unknown options (`bad_options`). `grow-workflow-harness.mjs` (35 checks) drives the v12 loop: dispatch and `-After`, the filled templates, only relays and DSMP managers, the zt-relay role, the options, a cut line and a rewritten line read again from the status, a second unreadable line stopping the stage, a repeated job set dispatched once, the Lock fault with its tree and Lock id (never retried; the cleanup still runs), a step failure, the frozen args, and the build's modules and its one Lock check.

- `pwsh -NoProfile -File scripts\book-digest\tests\grow.tests.ps1` (also in `run-all.ps1`) drives plan, the topic job, topics, precheck, apply, check, a fix round, a rerun and the report on a scratch archive. It covers G2, G3, G4, G6, G7, G8 and G10, a reused and a new run Topic, the doorway, a chain, a later root placement opening a line beside the doorway, G13 for a second idea at one Topic root, a doorway with 7 lines sending the next root placement beside its last line (with `deeper` and the report line), a Side successor of an agent Card, `same`, and a share going from 1/4 to 5/8 (108 checks). Since 3.1.0 it also runs the concept review's input (`concept-books`: a book without a term table skipped with its reason, a named book's folder with `manifest.json` and `terms.md`; `concepts` propose, prepare and an unjudged report on it; `hubs-report -Concepts` reading that report and without the flag reading none) and the Topic maintenance round on fresh Topics (`organize-plan -Trees` finding a near-named pair and a Card that links another Topic more than its own; the go precheck naming O1 and O6, then passing; `organize-apply` merging the Topics, moving the Card, deleting the emptied root and renaming the run's Card in `state.json`; no answer applying nothing).
- Its Lock phase (S15, #138) takes real owner Locks through the page-token `/api/lock` and `/api/unlock` routes: a Locked source book still plans, a Locked Topic is marked and listed as read-only context only, `titles.json` carries each Lock, a Locked `at`/`same`/`existing` and a new Topic named like a Locked one are G12, a new Topic and its Cards are written while the source and another Topic stay Locked, a Topic locked after its answers stops the apply with exit 2 and the typed refusal (tree, Lock, step, address, no bypass hint), keeps the answers and the stopped job, writes nothing into the Locked tree and is named by the report, and unlocking the same folder writes the same three Cards and clears the stop.
- `cargo test --bin zt-digest grow` (26 unit tests: addresses, source citation, reverse links and share, Link kinds as zt reads them, the run Topics and their bound, the first sentence, a good answer, every fault code, a Locked destination, a Locked Topic in the topic answer, the typed refusal, the two closest blocks, the hub limits, the grown Cards by book; since 3.1.0 the term table of each book, the reviewed books, the surveyed Topics, a good and a faulty maintenance answer (O1–O8) and the run's addresses after the moves). Beside them `register::tests::several_reviews_combine_their_meanings_and_keep_each_report_id` and `organize::survey::tests::a_card_that_links_another_topic_more_than_its_own_is_a_move_candidate`.
- `node scripts/book-digest/tests/grow-workflow-harness.mjs` (also in `run-all.ps1`) builds `zt-idea-grow` from its blueprint and runs the built script against a stand-in runtime. It covers mode `hubs` (no Card grown, the hub options on the hub plan, the register, the hubs result), the restatement round (one book: prepare with the run's own Cards, after the fix rounds and before the hubs, reported per book; `restate: false` skips it), the hub jobs at `hubEffort`, the 3.1.0 rounds (with DSMP managers and the cleanup agent answered by label: restate → organize → concepts → hubs in that order, `hubs-report -Concepts`, one decision job go-001 and one judge job per book, the results, organize off by default, `concepts: false`, a term table with no concept starting no job, mode hubs reviewing the named books, a Lock refusal of `organize-apply` stopping the stage; 46 checks in all), the frozen build's `zt-grow-cycle` v10 and its Lock reader, a normal run (plan, no section job, check, report) that completes, and the seam itself: a `grow -Step topics -Apply` the owner's Lock refused stops the `grow` stage with the tool's sentence, `tree` and `lock_id`, runs that step once, plans no Topic Card write, keeps the plan line reported before the Lock, leaves the report stage unrun, reports no completed result, and stops even when the refusal carries no English sentence.
- `node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs simulate scripts/idea-grow/workflow/zt-idea-grow.blueprint.json --sim scripts/idea-grow/workflow/zt-idea-grow.sim.json` (the dry run the install repeats).

## Decisions and pitfalls

- **One command decides (4.0.0).** v11 had 28 branches in JavaScript; v12 has three (jobs, done, fault), and the order of the steps is typed and tested in Rust. The steps themselves did not change: `next` calls them in v11's order. Its memory is `next.json`, so a stop at any point resumes by rerunning the same folder.
- **A checksum on the job line (4.0.0).** A relay that cut or rewrote a long line was the cause of most digest incidents (trials 3, 5, 10, 12, 17). The job set is checked before any dispatch; a line that fails is read again from `status`, and a second failure stops the stage instead of guessing.
- **Every round on a box copy (4.0.0).** 3.2.0 skipped the live-box rounds on a copy because the hub jobs named no zt config. The box pin in the work folder points every child step and every DeepSeek job (they run in the work folder and name paths under it) at the copy, so a pilot on a copy tests every round.
- **Judge jobs side by side (3.2.0).** The restatement and concept reviews ran one book at a time, so a box of many books (up to 24 concept reviews) waited for one judge job after another. Every book is now prepared first and all judge jobs go out in one dispatch; the judge, apply and report steps stay in the books' order, so the result and the archive writes keep one order.
- **Mathematics textbooks (3.2.0).** A math Literature tree is sections of units (Model Cards with a `form`). The prompt keeps a unit's statement, hypotheses and proof in the Literature tree; a Topic Card states the idea behind the unit (its problem, its intuition, its use, its relations) and links the unit with `例示`/`对应`/`对照`/`特例`, never `依赖`. `grow.tests.ps1` plans a theorem unit as a section note.
- **Say what a box copy skips (3.2.0, superseded in 4.0.0).** On a copy of the box the restatement review, the Topic maintenance, the concept review and the hubs did not run (the hub jobs' helper lines named no zt config). 4.0.0 pins the work folder instead, and every round runs.

- **No forced Links.** In pilot 4 a public-credit Topic (from the Hamilton notes), offered only outsourcing Cards, wrote analogies such as `对照 [[394/1]]：沉没投资的供应商议价力下降，国家先持有资产则立起信用`. The candidates now must share wording, and the prompt asks an analogy to name the shared mechanism. An empty answer is often the right one.
- **The box, not one book.** 1.x read one Literature tree, so a section's ideas could only relate to Topic Cards. 2.0.0 reads every Literature tree by default. The Topic plan sees the sections of every book, each section sees the closest notes of other books, and the prompt asks how this note bears on what other books said. A Topic Card that brings two sources together is what Luhmann's main box was for.
- **Not in the digests.** The user said the Topic proposals belong to this workflow, not to the digests (2026-10-20), and then that the digests write no Topic, Topic Card, hub or register at all (2026-10-22, `docs/literature-only-digest-design.md`): every Topic-side compose is here.
- **Moved in 3.1.0: the concept review, one per source book.** `zt-digest concepts` reads a digest staging folder (`manifest.json`, `terms.md`); a grow run has none, so `concept-books` stages one from the book's term-table Card. The register reads every agent term table at once, so it reads every book's report at once: `-Review` takes several files, and `merge_reviews` unions the accepted entries and the distinct senses (one report is read exactly as before, so a one-review plan keeps its id). The book is reviewed only when it has a term table: proposing concepts from anything else would invent evidence (S23).
- **Moved in 3.1.0: Topic maintenance, opt-in.** It moves Cards and deletes emptied Topic roots, so it runs only with `organize: true`, and never in mode hubs. DeepSeek decides (the model roles: DeepSeek reviews, scripts build), not Opus as in the old Organize; the precheck and `organize-apply`'s own archive plan both check each decision before a write. The book survey of `organize-survey` finds no Topic since 3.0.0 (a Topic-tree Card belongs to no book), so the tool gained `-Topics`, a survey of named Topics. Card notes (the old Organize's third list) are not taken: they would need a card cycle, which writes Literature Cards in a digest.
- **Counts through relays, never lists (until 4.0.0).** The digest trials showed relays abridging long lines (trial 12: model part lists). The steps printed counts and the module named the jobs. Since 4.0.0 `next` prints the job ids itself (the link round's unapplied ids on a rerun were not a count: S0), guarded by the checksum.
- **Read Link kinds as zt does.** A hand-written list was too short (it missed `承接` and `依据`) and too long (it counted `引出`). The port of zt's table keeps G3, G4, G6 and G10 in step with the archive and with zt-check L1. When zt's table changes, change `RELATION_CUES` too.
- **A link round, not only section Links.** In pilot 3 (1.2.0) only 6 of 56 new Cards linked another Topic tree, all to 379/1, because a section job sees no Card that another job writes at the same time. The link round runs after every section is written, so it sees them all. A Link joins two trees, and a Card gets at most two in this round.
- **Plan the Topics before the sections.** In pilot 1 (1.0.0) eight parallel section jobs each opened a Topic, and four of them were on one concept. Each job also branched its "related" idea off the one earlier Topic Card it could see (379/1|a…e). The gt-001 job is the fix: Luhmann's Topics are few.
- **No blind main-line append.** 1.1.0 appended a root-direct idea to the end of the Topic's main line. In pilot 2, five parallel sections joined into one 20-step chain in 389, and a Card such as 389/5 followed 389/4 without relating to it. 1.2.0 opens a line beside the doorway for each later root placement; a section continues another section's Card only by naming it (`at`, with G6's first-sentence relation).
- **About seven lines per doorway.** On the live run of 2026-10-22, Topic 412 got 9 lines beside 412/1: section 1 put 3 ideas at the root and 7 more sections opened one line each. G13 lets a section open only one line per Topic. A doorway with 7 lines files the next root placement beside its last line. Advice alone (an eighth line plus a report line) was rejected, because a long-lived Topic only grows. The report names each Card filed one level down, because it may branch off another section's line (`docs/idea-grow-design.md`, "Lines beside the doorway").
- **`base` is the stored text** (`get_card` `text`), not the body; a body as base is refused as `changed since it was opened`.
- **Launch only after a matching user request.** Workflow subagents judge each command against the session's latest user message (see the digest trial 13 note in memory).

## Change history

- `2026-10-25` — S27 (#166): the grow prompt `dsh-grow-instructions.zh.md` ends with the same shared writer guidance block as the digest prompts — preserve a `[[词@地址]]` Term macro or write the visible term plainly, never manufacture an address or a synonym, run the exact SELF-CHECK (a run with no `faults:` line is no check), treat a `card_check` refusal as content for the current fix round and `card_locked` as a planning stop. The block is one text; `scripts/book-digest/tests/writer-guidance.tests.ps1` compares it verbatim there and in the 19 digest prompts. No module, blueprint or stage changed.
- `2026-10-24` — **4.0.1** (zt-digest 0.13.3): the restatement review runs before the check and fix round, so the fix round also sees the Cards the review's accepted rewrites changed. On the 4.0.0 pilot (the test box, both books) five rewrites with the remedy `distinct-implication` began `本卡依赖 [[<own book>/n]]`, so the run ended with five I7 faults after a check that had found none. Also from the pilot: zt-digest 0.13.2 (the register staged an empty `root_before` for an existing register, so its first plan with parts stopped half-published; a step failure's cause keeps the child's own error sentence). Pilot: 16 sections, 8 Topics, 113 Topic Cards, 74 Links across trees, 9 bridges, share 0 → 44%, 37 restatements resolved by rewrite, 688 concept findings, 6 topic and 3 field hubs, register published (152 terms); a deliberate stop in the section jobs (7 of 16 answered) and a rerun that dispatched only the other 9.
- `2026-10-23` — **4.0.0** (zt-grow-cycle v12, zt-workflow-lib v1, zt-digest 0.13.0; `docs/workflow-simplicity-proposal.md` S0–S4, S6, S7, S9): `zt-digest grow -Step next` and `-Step status` (`grow_next.rs`) run the whole cycle as one command with `next.json` as its memory; the module is one loop over its jobs, done and fault lines; every DeepSeek job set goes through `dsDispatch` with the command's job template and approval rule; the relay is the `zt-relay` agent type (the runner role leaves); a checksum guards each jobs line and `status` is the one recovery; resume is a rerun of the same folder (other args refused); the box pin lets every round run on a box copy; frozen args (`fixRounds`, `hubFixRounds`, `hubEffort`, `dshModel`, `register: plan`, `organizeTrees`). S0 repairs: the restatement round's `\w`/`\c`/`\o` paths (3.x lost the separators, so restate prepare found no Cards file and every book was skipped), the link round printing its unapplied job ids on a rerun, the Lock check on DeepSeek replies (it read `answer`, which DSMP results do not carry). The report renders the review rounds. Tests: 5 Rust tests, `grow.tests.ps1` (the next run, its stop and rerun), the harness rewritten (35 checks).

- `2026-10-22` — **3.2.0** (zt-grow-cycle v11, zt-digest 0.11.2): the restatement and concept reviews prepare every book, then send all the books' judge jobs in one DeepSeek dispatch, then judge, apply and report each book in order (a failed prepare skips only its book); a run on a box copy logs and returns the rounds it does not run (`skipped_on_copy`); the prompt adds the guidance for mathematics textbooks. Tests: grow harness (three books in one manager, prepare before judge, order, a failed prepare, the box copy), grow.tests.ps1 (a theorem unit as a section note).
- `2026-10-22` — **3.1.0** (zt-grow-cycle v10, zt-digest 0.11.0): the two Topic-side jobs the digests had left. The concept review of each source book's term table (`grow -Step concept-books`, then `zt-digest concepts` propose/prepare/judge/report with one DeepSeek judge job `cNN-cj` per book) runs before the hubs in mode full and mode hubs, and `hubs-report -Concepts` hands the rendered reports to the register plan (`register -Review a.json;b.json`, `merge_reviews`); arg `concepts` (default true). The optional Topic maintenance round (`organize: true`, `organizeTrees`) runs after the restatement review: `grow -Step organize-plan` (`organize-survey -Topics`), one DeepSeek decision job go-001 with the O1–O8 precheck, and `grow -Step organize-apply` (the valid decisions through `organize-apply -Open`, the run's addresses renamed). The result carries `organize` and `concepts`. Tests: 7 new grow unit tests and 2 beside them, `grow.tests.ps1` 108 checks, the harness 46 checks, the sim with both rounds.

- `2026-10-22` — **3.0.0** (zt-grow-cycle v9, zt-digest 0.10.0): the owner moved every Topic-side compose out of the digests (`docs/literature-only-digest-design.md`), so this workflow owns the Topic trees, Topic Cards, hubs and the register. Added mode `hubs` (the hubs round and the register alone), the hub options `hubTrees`, `hubRebuild`, `hubLimits`, `hubEffort`, `hubFixRounds` (`grow -Step hubs-plan -Limits/-Trees/-Rebuild`, `merge_limits`), and the restatement review of the run's new Topic Cards (`grow -Step restate-cards`, `grown_by_book`, one `zt-digest restate` chain per source book). Tests: 2 new grow unit tests, the grow harness (mode hubs, hub options, restatement round), the sim's restatement round.

- `2026-10-22` — 2.5.1 (zt-digest 0.9.3): two fixes from the first hubs round on the live box.
  - That round built 3 topic, 3 line and 2 field hubs, with a hub check of 0 faults. It published a register of 93 terms that point at specific Cards (42 before, all at Topic roots).
  - It reported the register as partial: the check prints its faults as a list (`fault_count` now counts a list).
  - It wrote a second field hub, leaving 411/1|a unanchored: a new hub work folder knew no field hub. `seed_fields` now gives the hub state the box's field hubs by title before the plan, so the field writer keeps and updates them.
  - The two remaining "missing" line hubs (412/1 and 413/1, the lines from a Topic's doorway) come from zt's hub policy, which counts every Idea Card as a branch. That is reported separately.
- `2026-10-22` — 2.5.0 (zt-grow-cycle v8, zt-digest 0.9.2): the hubs round (`hubs-plan`, `hubs-apply`, `hubs-check`, `hubs-report`; the gh precheck; `hub_limits`; the register rebuilt and published under `register: apply`) and the args `hubs` and `register`; the grow report has a "## 枢纽与索引" section. The user found the hubs "not well developed and well linked". The live box had one field hub, and a register whose 37 terms all pointed at Topic roots. The grown Topics 412–414 were in no hub: `list_hubs` reported topic hubs missing for 412 and 413, a line hub missing for 412/1, and 208 Cards unreached. Tests: `grow.tests.ps1` (the hubs round on a scratch archive), 17 grow unit tests (`the_hub_limits_come_from_the_survey`), both harnesses pinned to v8.
- `2026-10-22` — **2.4.0** (zt-digest 0.9.1): about seven lines per doorway. The live run of 2026-10-22 (`D:zt-growlive-2026-10-22`) gave Topic 412 nine lines beside 412/1. The precheck now reports G13 for a second idea of one answer at the same Topic root, and the prompt (rule 4 of 放在哪里, and the run-topics block of each section job) asks for one root placement per section and Topic. `apply` files a root placement beside the doorway's last line once the doorway has 7 lines, and the report counts and lists those Cards (`deeper`, `## 入口卡已满`). Module unchanged (v7). Installed and verified: `.claude/workflows/zt-idea-grow.js` 2.4.0 (fingerprint `61487262681e63a5`, `verify` ok, no library drift); 2.3.0 stays in `history/`. `grow.tests.ps1` 84 checks, `cargo test --bin zt-digest grow` 16 unit tests.
- `2026-10-22` — Tree Lock S17 (#140): **2.3.0**, `zt-grow-cycle` v7. The grow stage reads the tool's typed `card_locked` JSON (never an English sentence) out of a failed `grow` step or a DeepSeek job's answer and stops where it stands: the plan line and the Topic Cards already filed stay, the answer files stay, nothing is retried and no other write path is tried. The prompt's "已锁定的主题（只读）" section adds that rule for the DeepSeek writer, which reports `FLAG: card_locked <address>`. Installed and verified with the existing protocol: `.claude/workflows/zt-idea-grow.js` 2.3.0 (fingerprint `ce9190cdd04e7cea`, `verify` ok, no library drift); 2.2.1 stays in `workflow-framework/zt-idea-grow/history/`. New harness `scripts/book-digest/tests/grow-workflow-harness.mjs` (18 checks).
- `2026-10-22` — Tree Lock S15 (#138): `zt-digest grow` reads and never writes a Locked tree. The plan keeps each Card's `lock` in `titles.json`, prints `locked`, marks a Locked Topic in `topics.md`, and lists its Cards apart as related context; the precheck and `topics` report the new G12 for a Locked destination instead of moving an idea; the archive's `card_locked` refusal stops the apply with the typed JSON and exit 2 while the answers, receipts and stopped job stay, and the report names the stop. The prompt gains a "已锁定的主题（只读）" section. `grow.tests.ps1` grows to 78 checks with real owner Locks, and `cargo test --bin zt-digest grow` to 14 unit tests.
- `2026-10-20` — `grow.tests.ps1` without `-Digest` takes the newest zt-digest build among `D:zt-targetdebug`, `D:zt-grow-targetdebug` and the installed one, and prints which. A stale debug build in the digest session's folder had failed the instructions check.
- `2026-10-20` — 2.2.1 (zt-grow-cycle v6): the steps set no working folder (the plan step ran before the folder existed); the share counts Topic Cards of both Workspaces, and the report says so. Pilot 5 on 2.2.0, on the copy of the box (8 min, 15 conversations, all deleted): 9 sections of 2 books, 5 run Topics, 54 Topic Cards, 42 Links across trees, written before the source line, 1 `same`, 0 faults, share 6.3% → 49.2%, 0 bridges (the two books share no mechanism). The public-credit Topic wrote 2 Links, one of them to the user's human Card, instead of pilot 4's 4 forced analogies.
- `2026-10-20` — 2.2.0 (zt-grow-cycle v5): `ztConfig` and `tool` (`-ZtConfig`, `-Tool`, `retool` writes them into the job files and the helper line). The user asked that idea-grow be developed on a copy of the box while a separate session reruns the digest on the live box. The copy is `D:\zt-grow-box`, and its development build lives in `D:\zt-grow-target`.
- `2026-10-20` — 2.1.0: the link sentence goes before the source line; link candidates must share wording (a tree with none gets no job); the prompt asks an analogy to name its shared mechanism. Pilot 4 on 2.0.0, the whole box (grossman2002integration, sylla2018alexander, 10 min, 14 conversations, all deleted): 9 sections, 4 run Topics, 57 Topic Cards, 32 Links across trees, 0 faults, share 6.3% → 50.4%; the Hamilton section continued the user's own human Cards.
- `2026-10-20` — 2.0.0 (zt-grow-cycle v4; user: "not locked on one single literature"): by default every Literature tree of the box (`-Books`, `-Book` optional), global section numbers with each section's `book`, a tree with no anchor as one section, the closest notes of other books in each section, the topic job grouped by book, per-book checks (`book_of`, `faults_by_book`), the per-book share and the bridges count in the report, at most 120 sections per run with `left`. The prompt names the book per section (`Key`) and asks for ideas that bring several sources together.
- `2026-10-20` — 1.3.0 (zt-grow-cycle v3): the link round (`-Step links`, `gl-NNN`, G11 in `validate_links`, `append_card` in apply, `linked` in the report). Pilot 3 on 1.2.0 (5.5 min, 10 conversations, all deleted): 4 run Topics, 56 Topic Cards with one line per section beside each doorway (391/1|a…f), direct runs up to 4, 0 faults, Topic share 1.6% → 48.7%, but only 6 Links to other Topic trees.
- `2026-10-20` — 1.2.0: a later root placement opens a line beside the doorway (`n/1|a` …) instead of appending to the main line. Pilot 2 on 1.1.0 (grossman2002integration, 5 min, 9 DeepSeek conversations, all deleted): the Topic plan gave 4 Topics (3 new, 379/0 reused), 58 Topic Cards, 0 faults (G10 caught the plain Links before writing, so no fix round), Topic share 1.6% → 49.6%; the blind 20-step chain in 389 is the reason for 1.2.0.
- `2026-10-20` — 1.1.0 (zt-grow-cycle v2): the run's Topics are planned first (gt-001, `-Step topics`, bound max(2, ⌈sections/2⌉) ≤ 8); a root-direct idea goes to the end of the Topic's main line; Link kinds are read with zt's own table (`RELATION_CUES`, `link_kinds`), and G10 refuses a plain-reference Link outside the source line. Pilot 1 on 1.0.0 (grossman2002integration, 8 sections, 15 min, 8 agents, 0 errors): 53 Topic Cards, Topic share 1.6% → 47.4%, 12 faults fixed in one round, 0 left, 12 DeepSeek conversations deleted; but 8 new Topics (4 on one concept) and five related ideas fanned off 379/1.
- `2026-10-20` — Created: `zt-digest grow` (plan, precheck, apply, check, report), module `zt-grow-cycle` v1, blueprint 1.0.0 installed and verified, the prompt `dsh-grow-instructions.zh.md`, test `grow.tests.ps1`. Design `docs/idea-grow-design.md`.
