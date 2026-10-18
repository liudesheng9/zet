---
name: zt-digest-protocol
description: The protocol of the ZT digest base (scripts/book-digest/workflow/zt-digest-base.blueprint.json), the shared part of every ZT digest workflow that turns a book into ZET Cards. It names the roles, the compiler cycle, the base stages and slots, the modes, the zt-digest tools the base runs, the profile contract a variant fills, the quality checks (zt-check and the SELF-CHECK lines), and the rules each run keeps. Use it to run or change the base, to debug a base stage, or to build a new variant on it. It does not describe a variant's own layers (models, events, findings, math): see that variant's blueprint and docs.
---

# ZT Digest Protocol (the base)

A digest turns one book (a Chinese or bilingual Markdown file from the Scholar Archive) into Cards in the **agent Workspace** of the ZET slip-box. Every digest workflow is built by the Workflow Framework Protocol from one **base** and one **variant**:

- The base (`E:\zet\scripts\book-digest\workflow\zt-digest-base.blueprint.json`, `"kind": "base"`) holds every stage that all book types share, the args, the roles, the modes, and four empty **slots**.
- A variant blueprint names the base, sets the **profile** (section 4), and fills the slots with its own layers. The installed variants are frozen builds in `E:\zet\.claude\workflows\`. This skill covers the base only.

Docs in the repo (`E:\zet\docs`): `zt-digest-workflow.md` (stages and args), `zt-digest-framework.md` (base, variants, install), `hub-cards-design.md`, `digest-card-layers-proposal.md`. The developer wiki block is `docs/wiki/digest-workflows.md`. The tools, prompt files, and tests of the base are listed in [references/scripts.md](references/scripts.md).

## 1. Roles

Each kind of worker does one kind of work. Do not move work between them.

| Worker | Does | Never does |
| --- | --- | --- |
| **Tools** (`zt-digest <tool>`, `E:\zet\tools\zt-digest`) | Every structural step: skeletons, tree operations (insert, move, delete), job files, checks, apply, reports. Every zt write of a JSON answer. | Write prose. |
| **DeepSeek** (DeepSeek Harness, V4.1 Flash, effort high) | The labour: Card text, Links, reviews, terms, hubs, skeleton text, Bridge judgements. It writes Card bodies through the zt helper, or a JSON file that a tool applies. | Plan structure, or change Cards it does not own. |
| **Haiku** (role `manager`, effort low) | Runs one tool command per step; manages DeepSeek jobs: starts them, approves their tool calls, collects answers, deletes conversations. Chooses the hub limits from a survey. | Read the book, write Cards, decide content. |
| **Opus** (role `organizer`) | The Organize decisions only (`decisions.json`: thesis, Topic merges, moves, notes). | Edit Cards: a tool and DeepSeek carry out its decisions. |

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

The workflow drives one step with `drive()` (`zt-digest-lib.js`): one Haiku agent runs the command, Haiku managers run its jobs (8 per manager), and an optional cleanup agent deletes the conversations. The base's compilers: `card` (the card cycle), `terms`, `hub`, `skeleton`, `bridge`. A new stage is a new compiler with the same steps.

## 3. The base stages (mode `full`)

```
args ─▶ source ─▶ prepare ─▶ [after-prepare] ─▶ terms ─▶ digest (blocks in parallel, each with its card cycle) ─▶ [after-digest]
     ─▶ link-prepare ─▶ link ─▶ link-check ─▶ [after-link] ─▶ cleanup ─▶ organize ─▶ figures ─▶ bridge ─▶ hubs ─▶ audit ─▶ [after-audit] ─▶ report
```

`[slot]` marks where a variant puts its layers; the base leaves the slots empty. Each stage is one module in `scripts/book-digest/workflow/modules/`; every module builds its commands with `Z.tool('<script>')` (`zdTool`), which gives `zt-digest <tool>`.

| Stage (module) | Steps | Writes Cards? | On failure |
| --- | --- | --- | --- |
| args (`gate`) | Checks `citationKey` (unless mode `hubs` or `audit`) and `source`, `bibtex`, `bookTitle` (mode `full`). | no | halt |
| source (`zt-dsh-source`) | With `dshSource: auto`, a Haiku probe asks the official DeepSeek source, then OpenCode Go, for one short answer; the run stops when neither answers. | no | halt |
| prepare (`zt-digest-prepare`) | `zt-digest prepare -Apply` with the profile's `-Instructions` (and `-NoEventCandidates`): Chinese-only text, section groups (`groups\gNN.md`, each with its `SELF-CHECK:` line), the shared prefixes, and the Literature tree with one anchor Card per group. | anchors | halt |
| terms (`zt-book-terms`) | `zt-digest terms` plan → one DeepSeek job → apply: the book term table (术语表) that rule R6 reads. | the 术语表 Card | continue |
| cards-book (`zt-cards-book`) | Mode `cards` only: the card cycle on the whole book, with orphans. | rewrites | halt |
| digest (`zt-digest-blocks`) | One Haiku manager per block of `perManager` groups (at most 8). Each group: `zt-digest snapshot-topics`, then one DeepSeek run on the digest prefix (built from the profile's instruction files) and the group file; it writes Cards with the helper (`-Log`, `-SrcLog`, `-TopicGate`), then runs its SELF-CHECK. When a block's manager returns, its card cycle starts (`zt-digest card -Groups … -Sample … -NoOrphans`). | yes | halt when no group is done |
| link-prepare, link (`zt-link-prepare`, `zt-link-pass`) | `zt-digest prepare-link`, then link managers (up to `linkPerManager` runs each) whose DeepSeek runs append reasoned Links (`dsh-link-instructions.zh.md`, `-Allow allow-gNN.txt`), each with its SELF-CHECK. | Links | halt |
| link-check (`zt-link-check`) | A card cycle after the link pass (`-Work <out>\cards-link`, with orphan jobs): every hard rule in mode `full`, otherwise `cardRules` or L1 and L2. | rewrites, Links | halt |
| cleanup (`zt-dsh-cleanup`) | Delete leftover DeepSeek conversations, remove the workspace, delete scratch files, read the usage ledger. | no | halt |
| organize (`zt-organize`) | `zt-digest organize-survey` → Opus `decisions.json` → `zt-digest organize-apply` → a notes card cycle (`-Notes`) → `zt-digest skeleton` plan → DeepSeek → apply (Literature index, structure Card, Topic bodies). | yes | continue |
| figures (`zt-figures`) | `zt-digest figures -Apply`: show the book's own figures (`artifact:`) in the Cards that name them; link the book's PDF and Markdown from the Literature Card. | yes | halt |
| bridge (`zt-bridge`) | `zt-digest bridge` with the profile's `bridgeProfile`: bounded plan → read-only judge jobs → immutable report (section 8). | no in discovery | continue; the stage shows `status: failed` |
| hubs (`zt-hubs`) | `zt-digest hub` on the whole agent Workspace: survey → Haiku limits → DeepSeek field and topic hubs (JSON) → apply → check → report. | Hub Cards | halt |
| audit (`zt-audit`) | `zt-digest audit`, read-only, over both Workspaces: faults per rule and category, orphans, figures, hubs, broken Links, Bridge counts in and out per book, islands. | no | halt |
| report (`zt-digest-report`) | No agent: the run's result for the mode, with each stage's status and the variant's extra reports. | no | halt |

A module that fails softly returns `{ status: 'failed', ... }` and its stage still ends; only a thrown error fails a stage. "halt" stops the run.

**Modes** (`args.mode`, default `full`) re-run base stages on an existing digest:

| Mode | Stages | Needs |
| --- | --- | --- |
| `link` | prepare, link-prepare, link, link-check, cleanup, organize (with `organize: true`), hubs (with `hubs: true`) | `out`, `citationKey`, `groups` |
| `cards` | prepare, cards-book | `out`, `citationKey` |
| `hubs` | hubs-first (the hub stage on the whole agent Workspace), organize (with `organize: true`) | `out` |
| `figures` | figures | `out`, `citationKey` |
| `audit` | audit (one book with `citationKey`, else every book) | `out` |
| `bridge` | source, bridge (discovery); with `bridgeReport` and a selection, only apply | `out`, `citationKey` |

A variant adds its own modes. **Checkpoint**: `args.until: "digest"` stops a full run after the digest, before the slot `after-digest` and the link pass.

**Args** are always an object; the kernel refuses an undeclared key. `out` is required; `source`, `bibtex`, `bookTitle` for `full`. The base's own args: sizing (`perManager`, `maxChars`, `maxUnits`, `linkPerManager`, `dshPerBatch`, `dshManagers`, `applyShare`), DeepSeek (`dshSource`, `dshModel`, `dshEffort`), switches (`terms`, `cardChecks`, `organize`, `figures`, `bridge`, `hubs`, `audit`), the card cycle (`ideaSample`, `cardRules`, `cardFixRounds`, `cardMax`), hubs (`hubWork`, `hubLimits`, `hubTrees`, `hubRebuild`, `hubEffort`, `hubFixRounds`), Bridge (`bridgeMax`, `bridgeCandidates`, `bridgeBooks`, `bridgeReport`, `bridgeApply`, `bridgeApplyEarlier`), and source slicing (`fileId`, `startHeading`, `endHeading`, `skipHeadings`, `groups`). Every DeepSeek effort below `high` is raised to `high` and logged. The base also declares the args its variants' model layers read (`models`, `level`, `perSonnet`, and the rest); they do nothing in the base.

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
6. Pilot on one chapter (`cardMax`, `hubTrees`), with a database backup first, before any whole-book run.

A change to the base or to a shared module (`zt-digest-lib` and the base stage modules) changes every variant: raise the versions and reinstall them all.

## 5. Rules every run keeps

- **DeepSeek Subagent Managed Protocol** (global skill `deepseek-subagent-managed-protocol`): one Haiku manager per 8 DeepSeek jobs, at most 8 managers alive at once in the whole run (`dsmpSlot`), conversations deleted (never archived) after their answer is collected, a workspace removed when its work is done. Write cleanup steps as plain steps: a Haiku agent refuses deletes that cite "the user's protocol" as authority.
- **Approvals**: a manager approves only the zt helper (`zt-digest call`), `zt-check`, the read-only `-Step precheck` commands of the job's tool, read-only reads under `out` (or of `source`), and writes of the job's own JSON file. Everything else is rejected.
- **Skills are sent, not pasted**: managers pass a job's skills in the `skills` field of every `deepseek_run` and `deepseek_prompt`: `zettelkasten` and `asd-ste100-zh` for digest and link runs, `asd-ste100-zh` for the card cycle, terms, skeleton, and hubs. No prompt file carries skill text.
- **Files and state**: each compiler keeps `state.json` in its work folder (the DeepSeek cwd), so a rerun skips finished items. Every tool prints compact JSON as its last line. Job ids are stable; follow-ups continue the job's conversation.
- **The zt helper** is `zt-digest call`: a JSON array of calls on stdin (`@'…'@ | zt-digest call - …`), with `-Allow` (the Cards a run may change), `-Log` (Cards it made), `-SrcLog` (their source lines), `-TopicGate` (no duplicate Topics), and `-Moves` (address changes); `@@N` is the address of call N. The old path `~\.codex\skills\zt\scripts\zt-mcp.ps1` forwards to it.
- **Source fidelity**: a step or explanation the book does not give carries `（补：书中略去）` or `（补：书中未写）`. The book's modality stays.

## 6. Quality checks

`zt-check` is the **one checker and the only source of truth** for Card quality (`E:\zet\tools\zt-check`, installed beside `zt.exe` by `scripts\install-zt-check.ps1`, no service restart). Every agent calls it directly; the `zt-digest` tools run it in-process.

```
zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,...]   # a writer's own Cards, classified as the audit does
zt-check --addresses <a,b,...>
zt-check --json <file>                                                      # drafts, offline
```

Output: one line per fault `<address> <rule> <text>`, then `faults: N`. A run that prints no `faults:` line did not run, and nothing may be written on it. Rules: R2 to R19 (Card language, size and exact corruption) and the Card layer rules I, E, F, L (Idea Cards, Event Cards, Finding Cards, Links).

The SELF-CHECK of each base task:

| Task | SELF-CHECK |
| --- | --- |
| Digest run | `zt-check --cards <anchor> --log <owned> --book <key>` (in the group file) |
| Link run | `zt-check --log <allow> --cards "" --book <key> --rules L1,L2,R2,R3,R4,R5,R7,R9` |
| Card cycle fix and orphan | `zt-digest card -Step precheck -Work <work> -Job <id>` |
| Hubs | `zt-digest hub -Step precheck -Work <work> -File <hub json>` |
| Skeleton | `zt-digest skeleton -Step apply -Staging <out> -Work <work>` without `-Apply` |
| Bridge judge | `zt-digest bridge -Step precheck -Staging <out> -CitationKey <key> -Work <work> -Job <id>` |

## 7. Operating rules

- **Back up before a run that changes Cards**: `node -e "const {DatabaseSync}=require('node:sqlite'); new DatabaseSync('D:/zt/zt.sqlite3',{readOnly:true}).exec(\"VACUUM INTO 'D:/zt/backups/<name>'\")"` (the archive is in WAL mode; a file copy misses changes). Check `pragma integrity_check`.
- **Run an installed workflow by `scriptPath`** (`E:\zet\.claude\workflows\<name>.js`); named workflows load when the session starts. Never edit an installed script: change the blueprint or module and reinstall.
- **Ask the user before the next stage of a long digest.**
- **Do not deploy zt during a run** (`zt down` kills the helper). `zt-check` and `zt-digest` install without a restart.
- **Build on D:** (E: is nearly full): zt with `CARGO_TARGET_DIR=D:/zt-target`, zt-check with `D:/zt-check-target`, zt-digest with `D:/zt-digest-target` (`scripts\install-zt-digest.ps1`, run from `E:\zet`: job files name paths of the repository the binary was built from).
- **Tests**: `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 4` runs every test file and harness in parallel; `cargo test --release` in `tools/zt-digest` and `tools/zt-check`.
- **Script traps**: JS `String.replace` with a string expands `$'`; `split(a).join(b)` needs a string `b`; a Bash heredoc loses backslashes (write files with the editor).

## 8. Cross-book Bridge contract

The base stage runs after Figures and before Hubs. Full mode discovers and reports; `bridge: false` returns skipped. Mode `bridge` needs only `out` and `citationKey`. `bridgeMax` defaults to 60 (integer 1–120), `bridgeCandidates` to 5 (integer 1–8, plus at most two literal term matches), and `bridgeBooks` optionally filters exact agent book keys. The variant's `bridgeProfile` picks which source Cards are selected. Retrieval is bounded and can miss relationships expressed deep in a body.

Report-only: use a fresh output folder, inspect `bridge/report.md` and immutable `bridge/proposals.json`. The workflow publishes the report only after every judge job answered. When the plan or a judge job fails, nothing is published: run mode `bridge` again with the same `out` to resume. The cleanup deletes every DeepSeek judge conversation and the DeepSeek workspace, never archives, and always runs, also after a throw. Proposal ids are `b001`, `b002`, and so on. Apply: use that same output folder with `mode: "bridge"`, `bridgeReport: "<actual report_id>"`, and `bridgeApply: ["<actual bNNN>"]` or `"all"`. `bridgeApplyEarlier` independently selects earlier carriers; earlier-only is valid. Empty arrays grant no edits; booleans, mixed arrays, duplicates, wrong sections, and unknown ids fail. Apply has no source probe, judge, planning, Organize, or Hubs.

A judge loads `zettelkasten` and `asd-ste100-zh`, reads only frozen candidates, writes its assigned answer, and runs its exact precheck up to three times. Its manager approves only the Bridge precheck, `zt-check`, the zt helper with the empty `read-only.allow.txt`, reads under `out`, and JSON writes under `bridge\out`. A judge never writes, splits, or moves Cards, and never grants acceptance. One source has at most two proposals, and one pair of Cards has at most one relation (the reverse of a proposed pair is refused). A Link names a whole Card. Human Cards are read-only. The tool checks BR1 to BR9 and the Card rules before a report and before each accepted edit.

Before the first live edit, get the user's actual selections and make a consistent archive backup (`zt dp`, or the SQLite `VACUUM INTO` backup with an integrity check). Keep filing quiet: no insert, move, delete, key rename, retype, or Organize alongside apply. The tool uses the raw `edit_card` base, per-report locking, and durable per-carrier intents. A stale report needs a fresh folder and a new review. Retries keep accepted text, verify persisted effects, and never restore an archive on their own. Keep old frozen workflow versions and reports.
