# One DeepSeek conversation per model: proposal and design

Status: proposal (2026-10-27). Not implemented. No code, prompt, blueprint or installed workflow changes with this document.

## 1. Problem

### 1.1 The model compiler opens many conversations per model

The model layer (`zt-model-layer`, `zt-model-lib` v13, `zt-digest model`) builds each batch of models in a chain of compiler steps. Each step lists DeepSeek content jobs, and nearly every job starts a new conversation:

| Step | Jobs the compiler lists | New conversations |
|---|---|---|
| brief | one per model (`<model>:brief`) | 1 per model |
| fill | one per 3 planned parts (`<model>:f<n>`) | about parts ÷ 3 per model |
| derive | one per 3 parts of each derivation chain (`<model>:d<n>`) | about derivation parts ÷ 3 per model |
| review | one per chain group (`<model>:r<n>`), in each of up to `reviewRounds` (2) rounds | 1 or more per model and round |
| split | continues the writer's conversation | 0, and a new one when the writer is gone |
| close | continues the writer's conversation with 修正 notes, up to 3 rounds | 0, and a new one for an unowned part |

The staging folder of acemoglu2010labor gives one measured case. Its batch `m-acemoglu2010labor_2_a` held 8 formal models, and the compiler state records 44 writer jobs for them: 26 fill jobs and 18 derive jobs (2 to 7 for each model). The 8 brief conversations and the review conversations come on top of these. One batch of 8 models therefore opened about 60 DeepSeek conversations.

### 1.2 Each step also costs a drive job and Haiku managers

Since #177, a DeepSeek drive job runs each compiler step under its own Haiku manager. The content jobs that the step lists then run in further Haiku managers, at most 8 jobs per manager. A formal batch passes through about 10 to 14 steps: brief, fill, derive, two rounds each of review and split, up to three close rounds, and report. Each step costs at least two Haiku agents and one more DeepSeek drive conversation. All of this is Claude work, and its only purpose is to move files between conversations that each know one part of one model.

### 1.3 Split conversations lose the model

- **Repeated reading.** Every new conversation reads the instruction file, `setting.md` and its task file again. The cached prefix of a conversation is lost when the next job starts a new one.
- **Writers know only their part.** A fill job writes 3 parts and a derive job writes 3 steps of one chain, but neither has seen the rest of the model. The review jobs read the Cards cold, and the close step must route each build fault back to the job that wrote the Card. The routing uses the job table, `continue` keys, `fallback` files and an `unowned` list for faults in Cards that no job of the batch wrote.
- **Fragile machinery.** Much of this machinery exists only to route work across conversations: the `+<seq>` follow-up ids, the session table, renumbering after a split ("name every target by its model and id first"), and the fallback that rebuilds a conversation that was deleted. Live runs have broken here (base 4.7.0, 4.9.1, 4.10.0: writers that answered a 修正 note and left the fault in place).

### 1.4 The principle already works elsewhere

The first principle added on 2026-10-27 to `docs/agents/workflow-principles.md` is that one conversation does a whole job: review, fix, fill and its own check. The card cycle adopted it in base 5.8.2 (#191). There, a group job carries the group's review, fix, fill and orphan Links, the fix rounds continue the same conversation, and one delete manager removes the kept conversations at the end. The Literature-note backfill of all five live books ran on that design. The model layer is the largest remaining layer that does not follow the principle.

## 2. Goal

One model, one DeepSeek conversation, one job: the **model job**. The model job receives the model's tasks in sequence (brief, fill, derive, review, split, fix, close check, explanations) and keeps its context across all of them. The aims are:

1. **Fewer DeepSeek conversations:** one per model instead of about 7 or more.
2. **Fewer Claude agents:** about 4 drive steps per batch and 2 content managers per 8 models, instead of 20 or more Haiku agents per batch.
3. **Fewer DeepSeek tokens:** the instruction files, the setting and the context pack are read once per model and then sit in the cached prefix.
4. **A simpler compiler:** no routing of follow-ups across conversations, and no `unowned` faults inside a batch.
5. **Unchanged quality gates:** the same build checks, `zt-check` rows, model-finish verifier and ledger units.

### Non-goals

- **The Sonnet architect stays.** It is still the one designer of each batch, and it still plans the parts that models share (`<model>#<id>`). It also gets one new design task: the explanation design of each derivation chain (§3.7). This keeps the rule in `model-compiler-roles`: Sonnet only plans, and DeepSeek writes, derives, reviews and splits.
- **No change** to the Card format, the Model parts, the facets, the build report, `model-deps`, the ledger units or `model-finish`.
- **No rerun of finished models.** The live books keep their models as built. The new design applies to new books and to an explicit `mode models` rebuild on a copy.
- **The mathematics stages are left out:** skeleton, Boundary, Proof, Theory and the M1–M18 check. They can follow this design in a later proposal.
- **Nothing is implemented** with this document.

## 3. Design

### 3.1 Principle

The script plans and checks, and DeepSeek writes. The difference from today is that one DeepSeek conversation does all of the writing for one model. The compiler no longer lists a job for each step. It writes one **task file** for each model, and the model job works through the file in order. Each task ends with a check command whose output the job must read before it starts the next task. The rules of the card cycle apply here too: SELF-CHECK lines, bounded fix rounds and one status line in the answer.

### 3.2 Batch flow

| # | Step | Who | Change from today |
|---|---|---|---|
| 1 | `start`: claim, listing, batches | drive job | unchanged |
| 2 | `brief`: context packs, then **one model job per model**, first prompt | drive job, then model jobs | the brief job becomes the model job's first task; the conversation is kept, not deleted |
| 3 | architect plan (`plan.json` for each model) | Sonnet | unchanged |
| 4 | `fill-prep`: plan-status, `model-deps -Units`, defer, then a task file for each model | drive job | the old `fill` step without its job list; it writes `task.md` for each planned model |
| 5 | **model job, second prompt** (continues the conversation from step 2) | model jobs | replaces the fill, derive, review, split and close jobs and their steps, and the explain jobs of these models |
| 6 | `report` | drive job | unchanged; it reads the per-model states |
| 7 | delete the batch's conversations | one delete manager | as the card cycle does since #191; the run cleanup is the backstop |

Model jobs that the defer step releases (they wait for a producer in another batch) never receive the second prompt. Their conversations are deleted with the batch's, and the ledger readies them again as it does today.

### 3.3 The model job

**First prompt (brief).** This is today's brief task, unchanged: read the context pack and write `brief.md`. The answer is one line, `{"model":"<m>","brief":"ok"}`.

**Second prompt (build).** `Read <dir>/task.md and do its tasks in order.` The task file holds:

| Task | What the job does | Check before the next task |
|---|---|---|
| T1 Fill | Write every planned part, in order, in chunks of up to 3 parts, which are today's job blocks. | `CHECK` for those addresses (the existing fill check) |
| T2 Derive | Only when the plan has Derivation parts: write each chain in order. | `CHECK` per chain |
| T3 Review | Re-read the stored derivation Cards **from the archive**, not from memory, against `review.zh.md`. Write the verdict file `review-<round>.json`. | none: the verdicts are data for T4 |
| T4 Split and fix | `model -Step split -Model <m>` applies the job's own verdicts and split needs, and the job then fills the new parts. T3 and T4 repeat up to `reviewRounds` times and stop at a round with no split and no fix. | `CHECK` for the changed addresses |
| T5 Close | `model -Step close-check -Model <m>` (new, read-only) lists the build faults of this model: closure, forward, reach-in, unanchored and missing planned Links. The job fixes them, at most 3 rounds. | the same command; it must print `faults: 0` |
| T6 Explain | Only when the model has Derivation parts: write 思路, 说明 and 解读 for every Derivation part, following the architect's explanation design for its chain (§3.7). Then run `model -Step explain-apply -Model <m>`, which refuses a changed 起点, step or 结果, as today. | `model -Step explain-check -Model <m>` must print `faults: 0`, at most 3 rounds |
| End | One line: `{"model":"<m>","parts":N,"split":S,"fixed":F,"explained":E,"faults":0}` | |

The job reads the instruction files (`fill.zh.md`, `review.zh.md`, and in the math profile the `BriefDoc`, `FillDoc` and `ReviewDoc` under `math\`) at the task where they are first needed. The prompt files of the first prompt hold the shared ones, so that the cached prefix is the same for every model job of a batch.

A split needs no extra trip through the module. Today the module reads `{"status":"split","need":N}` from an answer and starts a split step. In the new design the job runs the split command itself (`-Step split -Model <m> -Need <addr>=<n>`) and fills the parts that the split inserted.

### 3.4 Changes to `zt-digest model`

- **Per-model state.** Model jobs of one batch now run compiler commands side by side. The batch `state.json` becomes one state file for each model (`<dir>/<model>/state.json`), with a small batch index for the order and the plans. Steps that touch the whole batch (fill-prep, report) read every model's file. This removes the write race. A file lock is the alternative, but it is a second mechanism to keep, so the proposal does not use it.
- **`-Model` scope.** `split` and the new `close-check` take `-Model <m>` and touch only that model. A command that names a model outside the job's own model is refused.
- **Task file.** `fill-prep` writes `task.md` from the plan, with the same part blocks that `new_job` writes today and the CHECK and ALLOW lines for each task.
- **Removed:** the per-step job lists of `apply`, `derive`, `review` and `close`, `follow_jobs` with its `+<seq>` and `fallback` ids, and the `unowned` count inside a batch. `close-check` lists faults; it routes nothing.
- **Kept:** `brief`, `plan-status`, `defer`, `model-deps`, `report`, `resume` and the explain steps.

### 3.5 Managers and approvals

- **Managers.** One Haiku manager runs up to 8 model jobs. A formal batch holds up to `perSonnet` (8) models, so it needs one manager for each prompt. A concept batch holds up to `perConcept` (10) models and needs two. The DSMP caps are unchanged: 8 conversations per manager and 8 managers in the run.
- **Long jobs.** A model job runs longer than one fill job. The 240-minute DSMP budget counts the conversation's own run time. The run times of the live models were not recorded per model, so the pilot measures them (§3.8).
- **Approvals.** The manager allows the model job's reads, the zt helper, `zt-check`, the fill CHECK, and exactly two write commands: `model -Step split -Model <own model>` and the read-only `model -Step close-check -Model <own model>`. Since #177, DeepSeek drive jobs already run `model-steps.ps1` commands, so this adds no new class of command. It narrows the commands to one model.
- **Context.** A large model can fill a conversation. The manager reads `context.used_pct` in each report. Above 80%, or when a conversation is gone, the manager starts a fresh conversation with the fallback files (`task.md` and the model's state file). The tasks can be resumed because the state records each written part and each finished task, so the new conversation starts at the first unfinished task. This replaces today's per-job fallback.

### 3.6 Review inside the writer's conversation

Today the review is a separate conversation that reads the derivation cold. In the new design the writer reviews its own work, and the review loses some independence. The proposal accepts this for three reasons:

1. **Same practice as the card cycle.** The card cycle (#191) already reviews, fixes and fills in one conversation, and the Literature-note backfill passed with it.
2. **The review reads the archive.** T3 re-reads the stored Cards through the helper, not the text the job remembers writing, and it uses the unchanged `review.zh.md` checklist. Its verdicts go to a file that the script applies.
3. **The deterministic gates do not change.** The build faults in T5, the `zt-check` rows and `model-finish` stay as they are. They are what refuse a model, not the review.

The pilot measures whether the change lowers quality (§3.8, criterion 3). If it does, the fallback for the economics profile is one fresh reviewer conversation per model, which is still far fewer conversations than today. The design does not include that branch for economics until the pilot shows it is needed.

**Exception: the mathematics profile keeps an independent reviewer (owner decision, 2026-10-27).** In `zt-math-digest` the derivations are proofs, so T3 is not done in the model job. After T2 the module starts **one fresh reviewer conversation per model**. It reads the stored derivation Cards from the archive against the math `ReviewDoc` and writes `review-<round>.json`. The model job then continues with T4 and reads that verdict file in place of its own. The reviewer conversation is kept open across the review rounds and is deleted with the batch. A math model therefore uses two conversations: the writer and the reviewer. This is a per-profile setting (`profile.independentReview: true` in the math variant, absent in economics), not a branch inside the job. The owner may remove it after the first math book.

### 3.7 Explanations (思路/说明/解读) in the model job

**Owner decision (2026-10-27): the model job writes the explanations, and the Sonnet architect completes their design.** This replaces decision D6 of `docs/derivation-explanations-proposal.md` ("the plan does not change"). The writing rules of that proposal (the sections, the 说明 cases, the explanation budget of 16 sentences and 600 characters, and the chain position line written by the script) do not change.

**Design (Sonnet, in `plan.json`).** For each derivation chain, the architect adds an `explain` entry next to the chain's parts:

| Field | What the architect decides |
|---|---|
| `route` | The route of the whole chain: the key move and why it works, and why the obvious route fails when the book says so. The first part's 思路 states it. |
| `parts[].shows` | What each part shows, in one phrase, for that part's 思路. |
| `parts[].notes` | The steps that must get a 说明 line, with the kind of choice each one makes (case split, approximation, change of variable, limit, or a step a reader may not see at once). |
| `parts[].meaning` | What the result means for the model, the check that 解读 gives (a special case, a sign or a limit), and what the next part does with the result. |

The architect reads the briefs and the planned parts, as now, so the design needs no new input. `model-architect.md` gets one section for the entry and an example. `plan-status` refuses a plan in which a chain with Derivation parts has no `explain` entry, so a missing design is a plan fault, not a gap that the writer fills alone.

**Writing (DeepSeek, task T6).** The model job writes the text of each part from the design entry and from the derivation that it wrote itself. The design sets what each section says. The writer sets the wording, and it may add a 说明 line for a step that the design missed. The explain check is the existing one.

**Ledger order.** Today `model-finish` registers each `explain/<model>@1` unit, and the explain stage claims it later. In the new flow the explanations already exist when the batch finishes. `finish-batch` therefore runs `explain-finish` right after `model-finish` for each model whose job answered `explained > 0`. `explain-finish` re-reads the parts and refuses a part that still lacks text, as it does now. A refused unit stays ready, and the explain stage takes it as today. In mode `full` the explain stage then finds no ready unit for these models and ends at once.

**Mode `explain`** (books already built) keeps its own stage, since there is no model job to continue. It moves to the same principle: one conversation per model in place of one per chain. A Sonnet design step for those books is not part of this proposal, because their plans were made before the `explain` entry existed. That stage keeps its current plan, in which the script writes the chain position.

### 3.8 Testing and acceptance

- **Unit tests in `zt-digest`:**
  - per-model state isolation, with two models of one batch splitting at the same time;
  - the `-Model` refusal for another model;
  - `close-check` with the fault fixtures of trials 6, 10 and 17–19;
  - `task.md` built from a fixture plan;
  - resuming a task file from its state.
- **Harness (`framework-harness.mjs`):**
  - the model layer lists exactly one content job per model in each prompt;
  - no step after `fill-prep` lists content jobs;
  - the two prompts use one session id;
  - the delete manager runs once per batch.
- **Pilot.** The pilot runs on a copied box and an adopted copy of the acemoglu2010labor staging folder (`adopt-staging-copy`), never on the live box. It runs `mode models` with `rebuild` and `only` set to the 8 models of batch `2_a`. It is a single-stage test (`test-by-resuming`), and it starts only on the user's go-ahead. Acceptance:
  1. One DeepSeek conversation per model, counted from the bridge listing and the ledger resources. The only exception is a fallback conversation that the manager started for a recorded reason.
  2. Every model passes `model-finish`, and `close-check` prints `faults: 0`.
  3. The review keeps its strength: the split and fix verdicts are at least 70% of the live run's count for the same models. A read-through of 3 derivation chains against the source finds no step that a cold reviewer would have refused.
  4. The parts and facets are comparable to the live models: no model loses a core part of its plan.
  5. Every derivation chain has an `explain` entry in its plan. Every Derivation part passes the explain check, and its `explain/<model>@1` unit is done after the batch finish. A read-through of the same 3 chains finds that each 思路, 说明 and 解读 follows its design entry.
  6. DeepSeek tokens (`deepseek_usage`) are at or below the live tokens of the batch's model and explain work together, and the Claude agents for the batch, the architect included, are at most 8.
- **First real use.** The next new book is the first full run with the new layer. The math profile's independent reviewer (§3.6) is first used on the next math book, because the pilot book is an economics book.

### 3.9 Versions and release

| Part | Change | Version |
|---|---|---|
| `zt-digest` | per-model state, `-Model` scope, `close-check`, `explain-apply`/`explain-check` per model, `task.md`, the `explain` entry check in `plan-status`, `explain-finish` inside `finish-batch`, and removal of the step job lists and follow-up routing | 0.34.0 |
| `zt-model-lib` | the `compileBatch` of §3.2: two content prompts per batch, the math reviewer conversation when the profile asks for it, one delete manager | v14 |
| `zt-model-layer`, `zt-explain` | the new batch result; mode `explain` with one conversation per model | next versions |
| prompts | new `dsh-model-job.zh.md` (the task sequence and its SELF-CHECK lines); `model-architect.md` gains the `explain` entry; `fill`, `review`, the explanation instructions and the math docs stay as references that the job reads | — |
| `zt-digest-base` | minor: module behaviour; `reviewRounds` now bounds the job's own review rounds; no stage or data-flow change | 5.9.0 |
| `zt-econ-digest`, `zt-math-digest` | rebuilt on the base; the math profile sets `independentReview: true` | 6.9.0, 5.9.0 |

The release follows the release discipline in `workflow-principles.md`:

- Bump the three blueprints together, rebuild `zt-digest` (it embeds the blueprint versions), install in both scopes and dry-run mode `models`.
- A minor bump makes runs prepared under 5.8 non-drivable, so an old staging folder is adopted on a copy.
- The wiki docs change in the same commits: `digest-workflows.md`, `zt-digest.md`, `digest-prompts.md` and `models-and-math.md`.

## 4. Risks

| Risk | Answer |
|---|---|
| The writer reviews its own work less strictly. | The review re-reads the archive against the unchanged checklist, and the deterministic gates do not change (§3.6). Pilot criterion 3 measures this. The one fresh reviewer per model is the measured fallback. |
| A large model fills the conversation context. | The manager watches `used_pct` and continues in a fresh conversation from the model's state (§3.5). Tasks can be resumed by part. |
| Parallel model jobs corrupt the batch state. | Each model gets its own state file, and `-Model` refuses another model's commands (§3.4). |
| One failed conversation now loses a whole model, not 3 parts. | The DSMP retry and the state-based resume continue from the first unfinished task, not from the start. |
| A model job runs longer than the DSMP budget. | The budget counts the job's own run time. The pilot measures the largest model, and the fallback conversation splits a job that runs too long. |
| A writer works around a Lock. | Nothing changes here: `card_locked` stops the stage (base 4.11.0), and the model job never retries a write that a Lock refused. |
| Haiku managers poll for longer. | There are two managers per 8 models instead of 20 or more agents per batch. Each poll round stays one message (DSMP v1.1). |

## 5. Slices

| Slice | Delivers | Blocked by | Type |
|---|---|---|---|
| M1 | `zt-digest model`: per-model state, `-Model` scope, `close-check`, per-model explain apply and check, with unit tests | none | AFK |
| M2 | The architect's `explain` entry: `model-architect.md` section and example, the `plan-status` check | none | AFK |
| M3 | `fill-prep` writes `task.md` (T1–T6); `dsh-model-job.zh.md`; resume from state | M1, M2 | AFK |
| M4 | `zt-model-lib` v14: the two-prompt batch, the math reviewer conversation, approvals, `used_pct` fallback, one delete manager; `explain-finish` in `finish-batch`; `zt-explain` with one conversation per model; harness checks | M3 | AFK |
| M5 | Release 5.9.0 / 6.9.0 / 5.9.0: rebuild, install both scopes, simulate, wiki | M4 | AFK |
| M6 | Pilot `mode models` on an adopted acemoglu2010labor copy in a copied box, against §3.8 | M5 and the user's go-ahead | owner gate |

## 6. Owner decisions (2026-10-27)

1. **No dsh goal.** The model job uses a plain prompt with SELF-CHECK lines, as the card cycle does. The pause between the brief and the build prompt needs two prompts in any case.
2. **Explanations go into the model job, and Sonnet completes their design** (§3.7). The architect plans an `explain` entry for each derivation chain, and DeepSeek writes the text in task T6.
3. **The math profile keeps one independent reviewer per model for now** (§3.6). Economics reviews inside the model job.
