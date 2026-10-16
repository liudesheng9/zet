---
name: zt-digest-protocol
description: The base protocol of the ZT digest workflows (zt-book-digest, zt-math-digest) that turn a book into ZET Cards. It names the building blocks, every script and what to use it for, the order of the stages, the job and file formats, the quality checks (zt-check and the SELF-CHECK lines), and the rules each run must keep. Use it to run or change a digest workflow, to debug a stage, or to build a new digest variant on the same blocks.
---

# ZT Digest Protocol

A digest turns one book (a Chinese or bilingual Markdown file from the Scholar Archive) into Cards in the **agent Workspace** of the ZET slip-box. The reference implementation is `E:\zet\.claude\workflows\zt-book-digest.js`. `zt-math-digest.js` is the first variant: `scripts/book-digest/make-math-workflow.mjs` generates it from the book digest. Build every new variant on the blocks below, so all digests keep one standard.

Detailed docs in the repo (`E:\zet\doc`): `zt-book-digest-workflow.md` (the stages and arguments), `model-compiler-design.md`, `hub-cards-design.md`, `digest-card-layers-proposal.md`, `derivation-explanations-proposal.md`, `zt-math-digest-workflow.md`, `math-cards-design.md`. The full list of scripts and instruction files is in [references/scripts.md](references/scripts.md).

## 1. Roles

Each kind of worker does one kind of work. Do not move work between them.

| Worker | Does | Never does |
| --- | --- | --- |
| **Scripts** (`scripts/book-digest/*.ps1`) | Every structural step: skeletons, tree operations (insert, move, delete), job files, checks, apply, reports. Every zt write of a JSON answer. | Write prose. |
| **DeepSeek** (DeepSeek Harness, V4.1 Flash, effort high) | The labour: Card text, briefs, Derivation parts, explanations, reviews, hubs, terms, charts. It writes Card bodies through the zt helper, or a JSON file that a script applies. | Plan structure, or change Cards it does not own. |
| **Haiku** | Managers: start DeepSeek jobs, approve their tool calls, collect answers, delete conversations. Runs one script command per step. Chooses limits from a survey (hubs). | Read the book, write Cards, decide content. |
| **Sonnet** | The model architect: one `plan.json` per model (level, parts, build order, Derivation parts). | Write Cards, start DeepSeek runs. |
| **Opus** | Organize decisions only (`decisions.json`: thesis, Topic merges, moves, notes). | Edits: a script and DeepSeek carry out its decisions. |

Card language: every Card follows `asd-ste100-zh` (one statement per sentence, no `；`, active voice, at most 40 characters per description sentence). The checker enforces it (rules R2 to R17).

## 2. The basic block: the compiler cycle

Every stage that hands work to DeepSeek is a **compiler cycle**: one script with steps, DeepSeek in between.

```
survey ──▶ plan ──▶ DeepSeek writes ──▶ SELF-CHECK ──▶ apply ──▶ check ──▶ (one fix round) ──▶ report ──▶ cleanup
 script    script    JSON or bodies      the writer      script    script    same conversation    script     Haiku
```

1. **plan** writes one job file per few items and prints `{"jobs":[{"id","files":[...],"prompt"}]}` as its last line. `files` opens with the shared instruction file (`dsh-*.zh.md`), so DeepSeek caches the prefix.
2. **DeepSeek writes** a JSON result file (preferred: the script then makes every zt write), or Card bodies through the helper with `-Allow` (only its own Cards).
3. **SELF-CHECK**: every job file carries a `SELF-CHECK:` line, the exact read-only command that runs the checks of apply. The writer runs it, fixes each fault, and reruns until `faults: 0` (at most three runs). A fault the old Card already had is listed as kept, not counted. This removes guessing and most fix rounds.
4. **apply** validates again and writes. It refuses a changed Card, a lost Link, figure, or formula, and any new fault. It never writes unchecked text: no `faults: N` line from the checker means stop.
5. **check / fix**: refused items go back **once** to the same conversation (`"continue": "<job id>"`, with `fallback` files for a fresh one). A second refusal is final and reported.
6. **report** prints counts per item. **cleanup** deletes the DeepSeek conversations and removes the workspace (section 4).

Compilers that follow it: `model-compiler.ps1` (brief, plan, apply, derive, review, split, explain), `card-compiler.ps1` (card cycle), `hub-compiler.ps1`, `book-terms.ps1`, `book-skeleton.ps1`, `math-compiler.ps1`. A new stage is a new compiler with the same steps.

## 3. Stage logistics (zt-book-digest, mode `full`)

```
Prepare ─▶ Digest blocks (in parallel) ─┬─▶ Models for the block (model compiler)  ─┐
                                        └─▶ Card cycle for the block (card compiler) ┴─▶ model sweep ─▶ Link ─▶ Link check ─▶ Cleanup ─▶ Organize ─▶ Figures ─▶ Hubs ─▶ Audit
```

| Stage | Steps | Writes Cards? |
| --- | --- | --- |
| Prepare | Haiku probes the DeepSeek source (`dshSource`). `prepare-book-digest.ps1 -Apply`: Chinese text, section groups, the Literature tree with one anchor per group, the shared prefixes. `book-terms.ps1`: the term table (术语表). | anchors, 术语表 |
| Digest | One Haiku manager per block of `perManager` groups (at most 8). Each group: `snapshot-topics.ps1`, then one DeepSeek run (`dsh-instructions.zh.md` + the group file) that writes Idea, Event, and Model Cards with the helper (`-Log`, `-SrcLog`, `-TopicGate`), then its SELF-CHECK. | yes |
| Models | `list-models.ps1`, then per batch the model compiler: brief (DeepSeek) → plan (Sonnet) → fill → derive → review → split → report. | Model parts |
| Cards | The card compiler per block: plan → fix/review/orphan jobs (JSON) → apply → check → report. | rewrites |
| Link | `prepare-link-pass.ps1`, then link managers (up to 8 runs each) that append reasoned Links (`dsh-link-instructions.zh.md`), each with its SELF-CHECK. Then a card cycle on L1, L2, and orphans. | Links |
| Cleanup | Delete leftover DeepSeek conversations, remove the workspace, delete scratch files, read the usage ledger. | no |
| Organize | `organize-survey.ps1` → Opus `decisions.json` → `organize-apply.ps1` → a notes card cycle → `book-skeleton.ps1` (Literature index, structure Card, Topic bodies). | yes |
| Figures | `attach-figures.ps1`: show the book's own figures in the Cards that name them. | yes |
| Hubs | `hub-compiler.ps1` on the whole agent Workspace: survey → Haiku limits → DeepSeek field and topic hubs (JSON) → apply → check → report. | Hub Cards |
| Audit | `digest-audit.ps1`, read-only: faults per rule, orphans, models, explanations, figures, hubs. | no |

Modes run single stages on an existing digest: `link`, `models`, `cards`, `explain`, `hubs`, `figures`, `audit` (see `docs/zt-book-digest-workflow.md`). The args are always an object: `out`, `citationKey`, and for `full` also `source`, `bibtex`, `bookTitle`.

## 4. Protocols every variant inherits

- **DeepSeek Subagent Managed Protocol** (global skill `deepseek-subagent-managed-protocol`): one Haiku manager per 8 DeepSeek jobs, at most 8 managers alive at once in the whole run (`dsmpSlot`), conversations deleted (never archived) after their answer is collected, a workspace removed when its work is done. Write cleanup steps as plain steps: a Haiku agent refuses deletes that cite "the user's protocol" as authority.
- **Quality checks** (section 5): every DeepSeek task checks its own work before it replies.
- **Approvals**: a manager approves only zt-mcp.ps1, `zt-check`, the read-only precheck steps, read-only reads under `out`, and writes of the job's own JSON file. Everything else is rejected.
- **Files and state**: each compiler keeps `state.json` in its work folder (the DeepSeek cwd), so a rerun skips finished items. Every script prints compact JSON as its last line. Job ids are stable; follow-ups continue the job's conversation.
- **The zt helper** (`~\.codex\skills\zt\scripts\zt-mcp.ps1`): a JSON array of calls on stdin; `-Allow` (Cards a run may change), `-Log` (created Cards), `-SrcLog` (source lines), `-TopicGate` (no duplicate Topics), `-Moves` (address changes). Scripts call it through `zt-calls.ps1` (UTF-8 under a GBK console).
- **Source fidelity**: a step or explanation the book does not give carries `（补：书中略去）` or `（补：书中未写）`. The book's modality stays.

## 5. Quality checks

`zt-check` is the **one checker and the only source of truth** for Card quality. It is a crate of its own (`E:\zet\tools\zt-check`): building it never builds zt, and `scripts\install-zt-check.ps1` installs `zt-check.exe` beside `zt.exe` without a service restart. Every agent calls it directly: Claude sessions, workflow agents, Haiku managers, and DeepSeek runs. Scripts call it through `Invoke-ZtCheck` in `zt-calls.ps1`, which takes the old parameter names and throws when zt-check does not run. There is no PowerShell checker.

```
zt-check --cards <a,b,...> [--log <file>] [--book <key>] [--rules R2,...]   # a writer's own Cards, classified as the audit does
zt-check --addresses <a,b,...> [--setting <a>] [--level formal] [--require-explain]
zt-check --model <address> [--skip-shared]
zt-check --json <file> [--setting <a>] [--level concept|formal]               # drafts, offline
limits: --max-sentences 12 --max-chars 400 --max-steps 6 --max-explain-sentences 16 --max-explain-chars 600
```

Output: one line per fault `<address> <rule> <text>`, then `faults: N`; a run that prints no `faults:` line did not run, and nothing may be written on it. Rules: R2 to R17 (Card language, Card size, Derivation steps and explanation) and I, E, F, L (Idea Cards, Event Cards, Finding Cards, Links; see `docs/finding-cards-design.md` for F1 to F4). A new rule goes into `tools/zt-check/src/check.rs` with a unit test; its tests are `cargo test` in the crate and `scripts/book-digest/tests/zt-check.tests.ps1`, `card-layer.tests.ps1`.

The SELF-CHECK commands per task:

| Task | SELF-CHECK |
| --- | --- |
| Digest run | `zt-check --cards <anchor> --log <owned> --book <key>` (in the group file) |
| Link run | `zt-check --log <allow> --cards "" --book <key> --rules L1,L2,R2,R3,R4,R5,R7,R9` |
| Model fill and derive | `zt-check --addresses <own Cards> <LIMITS>` (`CHECK` and `LIMITS` in the job file; LIMITS holds `--require-explain`) |
| Explain | `model-compiler.ps1 -Step explain-check -Job <id>` |
| Card cycle fix and orphan | `card-compiler.ps1 -Step precheck -Job <id>` |
| Hubs | `hub-compiler.ps1 -Step precheck -File <hub json>` |
| Skeleton | `book-skeleton.ps1 -Step apply` without `-Apply` |

## 6. Building a new digest variant

1. **Generate, do not copy.** Write `make-<name>-workflow.mjs` like `make-math-workflow.mjs`: read `zt-book-digest.js`, apply exact-match edits (each must match once), write `.claude/workflows/zt-<name>-digest.js`. Shared stages stay the book digest's own text. Rerun the generator after every change to the base.
2. **A new stage is a compiler** (section 2): a `.ps1` with plan, precheck, apply, check, report; a `dsh-<name>-*.zh.md` instruction file with a 自查 section; a `SELF-CHECK:` line in every job file; JSON answers applied by the script.
3. **New rules go into zt-check** (`tools/zt-check/src/check.rs`) with a unit test, so every agent gets them.
4. **Tests**: a compiler test on a scratch archive (`ZT_CONFIG_DIR`, its own `zt up`; see `tests/model-compiler.tests.ps1`), and a workflow harness (`tests/workflow-harness.mjs`, `tests/math-workflow-harness.mjs`) with canned agent answers that checks stage order, commands, manager counts, and that no prompt archives.
5. **Approvals and docs**: add the new read-only commands to the manager approval lines; document the stage in `docs/`.
6. **Pilot** on one model or one chapter (`only`, `explainMax`, `cardMax`, `hubTrees`), with a database backup first, before any whole-book run.

## 7. Operating rules

- **Back up before a run that changes Cards**: `node -e "const {DatabaseSync}=require('node:sqlite'); new DatabaseSync('D:/zt/zt.sqlite3',{readOnly:true}).exec(\"VACUUM INTO 'D:/zt/backups/<name>'\")"` (the archive is in WAL mode; a file copy misses changes). Check `pragma integrity_check`.
- **Run an edited workflow by `scriptPath`**: named workflows load when the session starts.
- **Never rerun finished models**, and ask the user before the next stage of a long digest.
- **Do not deploy zt during a run** (`zt down` kills the helper). `zt-check` installs without a restart.
- **Build zt with `CARGO_TARGET_DIR=D:/zt-target`**, zt-check with `D:/zt-check-target` (E: is nearly full).
- **Script traps**: PowerShell names are case-insensitive (a local `$check` overwrites `$Check`); JS `String.replace` with a string expands `$'`; `split(a).join(b)` needs a string `b`, not a function; a Bash heredoc loses backslashes (write files with the editor).
