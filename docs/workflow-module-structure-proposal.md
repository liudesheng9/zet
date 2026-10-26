# Workflow module structure

Status: accepted by the user on 2026-10-26. It is the basic structure of every module of the digest
workflows (`zt-econ-digest`, `zt-math-digest`, their base) and of `zt-idea-grow`. A new module follows
it; an existing module moves to it when it is next changed. The developer wiki keeps the working copy
of these rules: [Workflow module structure](wiki/workflow-module-structure.md).

## 1. The principles

1. **Claude Code tokens are precious.** A Claude agent costs far more than a DeepSeek agent, and a
   small Claude agent re-reads its whole prompt on every turn. Every Claude agent in a module must earn
   its cost. Count the Claude agents a stage starts, not the commands it runs.
2. **The simplest structure is the most robust.** A module is a straight line of steps. Each decision
   branch is a place where a run can go wrong and a test can miss, so a module has as few as possible.
3. **DeepSeek does the labour.** Card composition, Links, Term work, reviews, fixes and every
   deterministic command run inside DeepSeek subagents, managed by the DeepSeek Subagent Managed
   Protocol (DSMP, the skill `deepseek-subagent-managed-protocol`).

## 2. The shape of a module

A module is a few feature blocks in a fixed order. Each block is **one Haiku manager and several
DeepSeek subagents**:

```
feature block = 1 Haiku manager (DSMP) ──► N DeepSeek subagents (≤ 8 per manager, ≤ 8 managers)
module        = block 1 → block 2 → … → block k        (a straight line; no loop over items in JS)
```

There are two kinds of DeepSeek job:

| Job | What the DeepSeek agent does | Example |
|---|---|---|
| **Content job** | Writes Cards, Links, Term drafts, reviews or fixes from its job files, runs its own self-check tool, and fixes its own faults until the check passes. It writes large output to a file and answers with one short status line. | a Term-card writer job; a group digest writer |
| **Drive job** | Runs one deterministic script step exactly as given and answers with the step's last output line (one JSON line). | `term-steps.ps1 -Step finish` |

A drive job replaces every Claude agent that only ran a command. A script step does a whole chain of
commands in one process (plan + claim + list, or complete-each-unit + publish), so one drive job
replaces many commands.

### 2.1 The rules

1. **No single-command relay agent.** No Claude agent runs one command and returns its line. A
   deterministic command chain is one script step, run by a DeepSeek drive job.
2. **Card composition and Link work are DeepSeek content jobs**, under DSMP: Haiku managers, at most 8
   conversations per manager and 8 managers at once, collect then delete (never archive), one retry,
   prompt collection (`wait_seconds: 0`, poll, 90 s block), a 240-minute budget, and the workspace
   removed at the end.
3. **Give DeepSeek tools.** Each content job carries the self-check it can run (`zt-check`, the
   compiler's `-Step check`, a precheck) and the rule to fix its own draft until it passes, so a fault
   is fixed inside the cheap agent instead of a new round of Claude agents.
4. **Few decision branches.** A module's code is a straight line: start → content → check → finish.
   Allowed branches: "no work" (skip the block) and "a step failed" (stop the module with its reason).
   Per-unit outcomes (done, failed, not ready) are data in the run ledger and the step's JSON line, not
   branches in the module.
5. **The ledger is the truth.** A script step records every unit's outcome (`ledger done` / `fail`)
   and prints one compact JSON line. The module reads that line; it never re-derives state.
6. **One shot, then report.** A module runs its blocks once and reports what is incomplete. It never
   loops to "try again" in JavaScript; a retry is the DSMP retry of one DeepSeek job, or a later run.
7. **Release what you hold.** A step that starts a branch releases the claims a stopped run left
   (`ledger reclaim -Release`), and a step never keeps a unit it did not ask for.
8. **Install with the protocol.** Every install of a digest or idea-grow workflow carries the DSMP
   block (`assemble.mjs install` inlines it; check `dsmpSlot` and `dsCleanupWorkspaces` are in the
   installed script), in the project and the global scope.

### 2.2 Models

| Role | Model | Used for |
|---|---|---|
| Manager | Haiku, effort low | every DSMP manager: starts, waits for, collects and deletes DeepSeek jobs |
| Labour | DeepSeek V4.1 Flash, official source, effort high | every content job and every drive job |
| Designer | Sonnet | only the overall designer of a complicated task (the model architect's structure plan) and the HTML visual composer |
| Decision | Opus | only the judgement roles the user has approved (the Organize decision); no new Opus role without the user |

No other role uses Sonnet. No module starts a Claude agent of any model to run a command.

## 3. The reference implementation

The Term stages of base 5.6.2 (2026-10-26, #172) follow this structure and are the first two slices' reference:

- `scripts/book-digest/term-steps.ps1`: the deterministic steps `start`, `review`, `fix`, `finish`,
  `link`. Each prints one JSON line and keeps its files under `<staging>\term-cards\drive`.
- `zt-term-cards` v4: `drive start` → writers (all jobs, 8 per manager, managers in parallel) →
  `drive review` → reviewers → `drive fix` → fixers → `drive review` → reviewers → `drive finish`.
- `zt-term-link` v3: one `drive link` job (plan, apply, complete, second plan).

Slice 1 (#175) is the layer driver: `scripts/book-digest/layer-steps.ps1` (`start`, `apply`, `finish`),
`zt-workflow-lib` v2's `ztDrive`, and `layerRun`/`driveStep`/`runJobs` in `zt-digest-lib` v38.

Slice 3 (#177) is the model layer: `scripts/book-digest/model-steps.ps1` (a block's `start` and `finish`, and each batch's brief, fill, derive, review, split, close and report), `zmlMake`'s `start`, `elaborate` and `finish` in `zt-model-lib` v11, and `zt-model-layer` v7 as a straight line of those steps. The Sonnet architect stays.

Slice 2 (#176) is the book-level driver: `scripts/book-digest/book-steps.ps1` (one step per block: the
preparation and its resume/backfill branches, the Link preparation, a stage publication, the card cycle's
plan/apply/check/report, the whole-book claim and checked finish, Organize's survey/apply, the skeleton,
Figures, the Bridge, the audit, the finalization's steps, the Term-link round and the settlement's
plan/apply/report), `bookStep` and `cardCycle` in `zt-digest-lib` v39, and one Haiku manager that makes the
one call no script can (`deepseek_settle`).

On the copied grossman book it ran with 15 Claude agents, all Haiku managers, and published 29 Term
Cards with 593 Term links. The relay version used about 22 Claude agents for one wave of four jobs.

A module skeleton:

```js
run: async (ctx, p) => {
  const drive = (step) => manager(`drive ${step}`, [driveJob(step)]).then(lastJsonLine)  // one DeepSeek job
  const content = (label, file, n) => Promise.all(chunk(n, 8).map((r) => manager(label, jobsIn(file, r))))
  const started = await drive('start')                 // plan + claim + list jobs -> jobs.json
  if (started.jobs) await content('write', started.file, started.jobs)
  const done = await drive('finish')                   // complete or fail each unit, publish
  return { status: done.complete ? 'done' : 'failed', ...done }
}
```

## 4. Moving the other modules

The modules below still start Claude relay agents (counted by `ztRelay`/`runScript` calls in their
source on 2026-10-26). Each moves to the structure when it is next changed: its command chains become
script steps, run by drive jobs.

| Module | Relay calls | Notes |
|---|---|---|
| `zt-digest-lib` | 10 | the layer driver (#175), the book-level driver (#176) and the model layer (#177) moved to script steps; the relay helpers left serve the math layers |
| `zt-model-lib` | 0 | moved by #177: every chain is one `model-steps.ps1` step run by a DeepSeek drive job; the Sonnet architect stays (designer role) |
| `zt-math-lib` | 4 | the mathematics compiler steps |
| `zt-grow-cycle` | — | zt-idea-grow's chain driver (slice 5) |

Moved by #175 (workflow structure slice 1): `zt-group-cycle` v7, `zt-cards-layer` v3, `zt-event-layer` v5,
`zt-finding-layer` v5, `zt-explain` v8, `zt-link-pass` v8 and `zt-book-terms` v5 call `ztRelay`, `runScript`
and `wfpScript` zero times: their blocks are `layer-steps.ps1` steps run by DeepSeek drive jobs, and `ztDrive`
(zt-workflow-lib v2) is the one drive helper the Term stages use too. Base 5.6.3, econ 6.6.3, math 5.6.3.

Moved by #177 (workflow structure slice 3): `zt-model-lib` v11 and `zt-model-layer` v7 call `ztRelay`, `runScript` and `wfpScript` zero times: the model layer's blocks are `model-steps.ps1` steps (a block's `start` and `finish`, and each batch's brief, fill, derive, review, split, close and report) run by DeepSeek drive jobs, its content jobs run in Haiku managers, and the Sonnet architect stays the one designer. No digest run starts a `zt-relay` agent. Base 5.6.5, econ 6.6.5, math 5.6.5.

Moved by #176 (workflow structure slice 2): `zt-digest-prepare` v16, `zt-link-prepare` v7, `zt-link-check` v3,
`zt-cards-book` v5, `zt-figures` v6, `zt-audit` v5, `zt-organize` v9, `zt-bridge` v10, `zt-finalize` v4,
`zt-term-link-lib` v3 and `zt-dsh-cleanup` v12 call `ztRelay`, `runScript` and `wfpScript` zero times: their
blocks are `book-steps.ps1` steps run by DeepSeek drive jobs, and one Haiku manager makes the one
`deepseek_settle` call the settlement needs. Base 5.6.4, econ 6.6.4, math 5.6.4.

## 5. Review checklist for a module change

- [ ] Every Claude agent the module starts is a Haiku DSMP manager (or an approved designer/decision role).
- [ ] No Claude agent runs a single command.
- [ ] Card, Link and Term content is written by DeepSeek content jobs with a self-check tool.
- [ ] Deterministic chains are script steps that print one JSON line and record outcomes in the ledger.
- [ ] The module is a straight line; its only branches are "no work" and "a step failed".
- [ ] The harness checks the drive steps' order and that no relay agent runs.
- [ ] The install carries the DSMP block in both scopes and `verify` is ok.

