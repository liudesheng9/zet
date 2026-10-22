---
block: idea-grow-workflow
title: Idea-grow workflow
paths:
  - "scripts/idea-grow/**"
  - ".claude/workflows/zt-idea-grow.js"
  - ".claude/workflow-framework/zt-idea-grow/**"
  - "!.claude/workflow-framework/*/history/**"
synced: 5907479
updated: 2026-10-22
---

# Idea-grow workflow

## Purpose

`zt-idea-grow` grows the agent **Topic trees** from the **Literature trees of the whole box**. By default it reads every Literature tree, in either Workspace. The more books the box holds, the more each idea can relate to, and one Topic gathers a concept from several sources. The user asked on 2026-10-20 that it not be locked on one Literature tree. The digests build a book's Literature tree, like Luhmann's bibliographic box. This workflow is the second act: it reads each section again against the whole box, keeps the ideas worth keeping, and writes them as standalone Topic-tree Cards in the reader's own words, each filed beside the Card it continues (the main box). The user asked for it on 2026-10-20. They asked that the Topic proposals (`docs/topic-tree-ideas-proposals.md`, P1–P7) live here and not in the digest workflows. Design: `docs/idea-grow-design.md`.

## At a glance

| | |
|---|---|
| Installed script | `.claude/workflows/zt-idea-grow.js` (project scope, frozen; registry `.claude/workflow-framework/zt-idea-grow/`) |
| Blueprint | `scripts/idea-grow/workflow/zt-idea-grow.blueprint.json` (2.2.1), dry run `zt-idea-grow.sim.json` |
| Module | `scripts/idea-grow/workflow/modules/zt-grow-cycle.js` (v6) |
| Prompt | `scripts/idea-grow/dsh-grow-instructions.zh.md`, read from disk by `-Step plan` |
| Compiler | `zt-digest grow` (`tools/zt-digest/src/grow.rs`, block [zt-digest](zt-digest.md)) |
| Writes | agent Topic trees only; never a Literature-tree Card, a human Card, a move or a delete |

## Concepts

- **The box is the unit**: `-Books a,b`, `-Book a`, or nothing (every Literature tree of the box). Sections get global numbers across the books, and each records its `book`. Every check (G3, G4, the source line, zt-check `--book`, G11) uses the section's own book.
- **Section**: one anchor of a Literature tree (`Key/n`) with its whole subtree (Idea, Model, Event, Finding Cards). A tree with no anchor (a hand-written Literature Card with a page index) is one section. Each section is one DeepSeek job. Its file also lists the 15 notes of other books closest by title ("其他文献里用词接近的笔记"), so an idea can contrast, confirm or generalize what another source says.
- **Bridges**: the run's Topic Cards that cite two or more Literature trees. The report counts them.
- **Idea kinds**: `核心` (core claim or mechanism), `关联` (a real relation to a Card already in the box), `有用` (a method, criterion or condition usable elsewhere), `有趣` (surprising, intriguing, or a question worth asking), `直觉` (why a result holds, without formulas).
- **Two boxes** (P1): the Literature tree keeps the book's own apparatus. A claim stated without the book is a Topic Card. It cites its source note with `例示`/`对应`/`对照` and never rests on its own book (`依赖 [[Key/…]]`, I7, G4).
- **Run Topics** (P2): before any section, one DeepSeek job (gt-001) reads every section's anchor and the Topic catalogue. It picks the few concepts this run files under: an existing agent Topic (`existing`), or a new one. At most max(2, ⌈sections/2⌉) new Topics, and never more than 8. `-Step topics` makes them, and every section job lists them.
- **Link kinds**: `grow.rs` reads each Link's kind as zt does: a port of `RELATION_KINDS` and `read_links` in `src/model.rs`. The cue words come from the sentence start or the previous Link, and a Link with no cue shares the sentence's earlier kind. `承接` and `依据` are rests-on. `引出`, `见` and `由…可得` give a plain reference.
- **Placement** (P2, P3, P6): `same` (an existing Card states it: append a sentence, or record it for a human Card); `at` an agent Topic Card (Direct successor = the next step, Side successor = a branch), with a first sentence that links it with a kind word (G6); `at` an earlier idea (a chain); `at` a Topic root (the doorway `n/1` when the Topic is empty, else a new line beside it: `n/1|a`, `n/1|b` …, in which the section chains its ideas); or a new Topic (`T1`) through the topic gate.
- **Link round**: after the sections are written, `-Step links` writes one job per Topic tree the run wrote in (`gl-NNN`). Each job lists that tree's new Cards, the run's new Cards in the other trees, and the 20 box Topic Cards closest by title. DeepSeek proposes Links to Cards in other trees: at most 2 per Card, each one sentence with its kind (G11). `apply` writes each sentence into its Card before the source line (`before_source`, an edit with `base`) and skips a Link the Card already has. A job offers only candidates that share wording with the tree's new Cards (at least 2 title bigrams). A tree with no candidate gets no job.
- **Topic share** (P5): the distinct Topic-tree Cards (either Workspace: the user's own Topic Cards count too) that link the book, divided by those Cards plus the book's Literature-tree Idea Cards (root and anchors left out). It is measured before and after the run.
- **Locked trees** (S15): the owner may lock one whole Topic tree. A Locked source book is read normally — a Lock never refuses a grow run. A Locked Topic is related context and a Link target only. The plan keeps each Card's Lock facts in `titles.json`, prints `locked` and marks the tree in `topics.md` (`已锁定：只读，可链接，不能写入`); the section file lists its Cards apart (`## 用词最接近的已锁定主题卡（只读：可作相关语境，可链接，不能写入）`). The precheck and `topics` report G12 when an answer names a Locked destination, so the answer names another one: the tool never substitutes a tree or a placement. If a Topic locks after its answers, the archive's own refusal stops `apply`: the job keeps its answers and receipts, the stage prints the typed `card_locked` JSON and exits 2, and `grow-report.md` names the stop; the same `out` resumes after the owner unlocks.

## Architecture

```
args → grow (zt-grow-cycle) → report (value)
grow: zt-digest grow plan → DeepSeek gt-001 (the run's Topics) → grow topics → DeepSeek gw-001..N (SELF-CHECK = grow precheck) → apply → grow links → DeepSeek gl-001..M → apply → [check → DeepSeek fxR-001..M → apply] × fixRounds → report → DeepSeek cleanup
```

The module builds the job ids from the counts the steps print (`{"topic_jobs":1,"pending":P}`, `{"jobs":N}`, `{"fix":M}`), so no id list passes through a relay agent. The runner (Haiku) relays each `zt-digest grow` command. DSMP managers (Haiku) run the DeepSeek jobs at most 8 per manager and 8 managers at once, and approve only:
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
| `fixRounds` | 0–2 | 1 | fix rounds after the first apply |
| `ztConfig` | string | (the live box) | the `ZT_CONFIG_DIR` of the box to grow. A copy of the box runs its own zt service; every step and the DeepSeek helper line set it, so the run reads and writes only that copy |
| `tool` | string | `zt-digest` | the zt-digest command, or the path of a zt-digest binary (a development build, so the installed one is never replaced under another run) |
| `dshModel` | string | `deepseek-official/deepseek-flash` | the official DeepSeek source (the user's rule) |
| `effort` | high, max | high | DeepSeek effort |

Result: `{ book, books, out, plan, topics, rounds, report, workspaces }`. `plan` names `books`, `sections`, `pending` and `left`. `report` adds `books` and `bridges`. `report` is the last line of `-Step report`: `created`, `topics`, `same`, `refused`, `duplicates`, `faults_left`, `share_before`, `share_after`, and `report` (the path of `grow-report.md`).

Result also carries `topics` (the topics step's line: `jobs`, `topics`, `created`, `refused`).

Module `zt-grow-cycle` v2 params: `book`, `work`, `instructions`, `tool` (default `zt-digest`), `from`, `to`, `model`, `effort` (high/max), `skills` (default `zettelkasten`, `asd-ste100-zh`), `fixRounds`, `runner`.

## Behaviour

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
   - A Topic root as `at`: with no doorway yet the idea becomes `n/1`. Otherwise it opens a line beside `n/1`.
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
- the run's record: `state.json` (`sections`, with per section `created`, `topics`, `same`, `refused`, `duplicates` and `kinds`; `cards`, every Card the run wrote; `fixes`; `checked`);
- the report: `grow-report.json` and `grow-report.md`.

A rerun with the same folder resumes: applied sections are skipped.

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

Run it only when no digest is writing the same archive. After it, run the digest's `hubs` mode to update hubs and the register.

## Testing

- `pwsh -NoProfile -File scripts\book-digest\tests\grow.tests.ps1` (also in `run-all.ps1`) drives plan, the topic job, topics, precheck, apply, check, a fix round, a rerun and the report on a scratch archive. It covers G2, G3, G4, G6, G7, G8 and G10, a reused and a new run Topic, the doorway, a chain, a later root placement opening a line beside the doorway, a Side successor of an agent Card, `same`, and a share going from 1/4 to 5/8 (78 checks).
- Its Lock phase (S15, #138) takes real owner Locks through the page-token `/api/lock` and `/api/unlock` routes: a Locked source book still plans, a Locked Topic is marked and listed as read-only context only, `titles.json` carries each Lock, a Locked `at`/`same`/`existing` and a new Topic named like a Locked one are G12, a new Topic and its Cards are written while the source and another Topic stay Locked, a Topic locked after its answers stops the apply with exit 2 and the typed refusal (tree, Lock, step, address, no bypass hint), keeps the answers and the stopped job, writes nothing into the Locked tree and is named by the report, and unlocking the same folder writes the same three Cards and clears the stop.
- `cargo test --bin zt-digest grow` (14 unit tests: addresses, source citation, reverse links and share, Link kinds as zt reads them, the run Topics and their bound, the first sentence, a good answer, every fault code, a Locked destination, a Locked Topic in the topic answer, the typed refusal, the two closest blocks).
- `node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs simulate scripts/idea-grow/workflow/zt-idea-grow.blueprint.json --sim scripts/idea-grow/workflow/zt-idea-grow.sim.json` (the dry run the install repeats).

## Decisions and pitfalls

- **No forced Links.** In pilot 4 a public-credit Topic (from the Hamilton notes), offered only outsourcing Cards, wrote analogies such as `对照 [[394/1]]：沉没投资的供应商议价力下降，国家先持有资产则立起信用`. The candidates now must share wording, and the prompt asks an analogy to name the shared mechanism. An empty answer is often the right one.
- **The box, not one book.** 1.x read one Literature tree, so a section's ideas could only relate to Topic Cards. 2.0.0 reads every Literature tree by default. The Topic plan sees the sections of every book, each section sees the closest notes of other books, and the prompt asks how this note bears on what other books said. A Topic Card that brings two sources together is what Luhmann's main box was for.
- **Not in the digests.** The user said the Topic proposals belong to this workflow, not to the digests. The digest prompts keep their filing ladder.
- **Counts through relays, never lists.** The digest trials showed relays abridging long lines (trial 12: model part lists). The steps print counts, and the module names the jobs.
- **Read Link kinds as zt does.** A hand-written list was too short (it missed `承接` and `依据`) and too long (it counted `引出`). The port of zt's table keeps G3, G4, G6 and G10 in step with the archive and with zt-check L1. When zt's table changes, change `RELATION_CUES` too.
- **A link round, not only section Links.** In pilot 3 (1.2.0) only 6 of 56 new Cards linked another Topic tree, all to 379/1, because a section job sees no Card that another job writes at the same time. The link round runs after every section is written, so it sees them all. A Link joins two trees, and a Card gets at most two in this round.
- **Plan the Topics before the sections.** In pilot 1 (1.0.0) eight parallel section jobs each opened a Topic, and four of them were on one concept. Each job also branched its "related" idea off the one earlier Topic Card it could see (379/1|a…e). The gt-001 job is the fix: Luhmann's Topics are few.
- **No blind main-line append.** 1.1.0 appended a root-direct idea to the end of the Topic's main line. In pilot 2, five parallel sections joined into one 20-step chain in 389, and a Card such as 389/5 followed 389/4 without relating to it. 1.2.0 opens a line beside the doorway for each later root placement; a section continues another section's Card only by naming it (`at`, with G6's first-sentence relation).
- **`base` is the stored text** (`get_card` `text`), not the body; a body as base is refused as `changed since it was opened`.
- **Launch only after a matching user request.** Workflow subagents judge each command against the session's latest user message (see the digest trial 13 note in memory).

## Change history

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
