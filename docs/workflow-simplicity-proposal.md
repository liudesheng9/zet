# Workflow Simplicity: Proposal

Status (2026-10-23): partly built; see "Implementation status" at the end. It covers the digest workflows (zt-digest base 5.3.0, zt-econ-digest 6.3.0, zt-math-digest 5.3.0) and zt-idea-grow 3.2.0.

The proposal rests on two observations.

1. **Simple is robust.** The digest and idea-grow workflows can keep their output and their protections with about one third of their present decision points. Every control-flow decision moves into a zt-digest command that prints one short line, every job keeps exactly one code path, and a DeepSeek job is judged only by the command that applies its answer. The digests then have 1 recovery path for an unreadable line (14 today), 1 finish path (4), 1 claim path (4), 1 copy of the retry classifier (4) and 3 kinds of agent prompt (about 12). zt-idea-grow's cycle becomes one loop with 3 branches (28 today).
2. **Claude Code tokens are the scarce resource.** In the token pilot on grossman2002integration, the workflow's Claude Code agents spent 10.3M weighted tokens. Haiku managers took 59% of that, relays 27% and the Sonnet architect 12%. DeepSeek did the writing at a far lower price. The proposal moves every task that needs no Claude-specific judgement to DeepSeek: the stepping of each block, the Organize decisions of the digests, and the diagnosis and repair of a failed run. Claude Code keeps the transport and the management that the DeepSeek Subagent Managed Protocol requires, plus the model plans as long as the user keeps them on Sonnet.

Every DeepSeek task in this proposal, old or moved, goes through the **DeepSeek Subagent Managed Protocol (DSMP) v1.1**. The workflow carries the protocol block from the protocol's `protocol.js`, installed by the framework's `dsmpBlock()`, and calls its `dsDispatch`. Its eight rules apply unchanged. Haiku managers at low effort run at most 8 conversations each and at most 8 managers run at once (64 conversations). Each conversation is deleted after its answer is collected, and never archived. A finished workspace is removed. A failed job gets one retry. Managers collect promptly, polling every 90 s, and a conversation that runs past 240 min is cancelled.

The section "Findings" counts the present decision points and the present token split. The section "Proposals" names the simplifications S0 to S10 and the moves to DeepSeek D1 to D4. The section "What may not change" and the section "Acceptance" bind the proposal to the present quality.

## Principle

This proposal applies seven rules to both workflows.

1. **One way to do each job.** A job (claim a unit, finish a unit, classify a fault, run a command, dispatch DeepSeek work, clean up DeepSeek) has one code path. A second path for the same job exists only while the first one is being replaced.
2. **Commands decide; agents carry.** A gate or a branch is decided by a zt-digest command: its exit code or a field of its one-line JSON. An LLM agent never decides control flow and never passes or fails work. An agent that only runs a command (a relay) carries the line back and decides nothing.
3. **LLMs write content only.** DeepSeek writes Cards, plans, verdicts and decisions; the Sonnet architect plans models while the user keeps that role on Sonnet. A command checks each product before it is applied.
4. **Few gates.** A gate exists only where an incident or a rule of the user requires it. Each gate that answered an incident keeps its protection; the mechanism becomes the simplest one that gives the same protection.
5. **Few knobs.** A value that no run has changed is frozen in the blueprint and leaves the argument list. A mode that a general rule covers becomes that rule.
6. **DeepSeek does the work; Claude Code carries and manages.** A task goes to DeepSeek unless it needs a Claude tool (the Workflow runtime, the DeepSeek bridge) or a judgement the user has kept on a Claude model. Claude Code agents start processes, run DeepSeek managers, and carry lines.
7. **Every DeepSeek task goes through DSMP v1.1.** One dispatch function, one manager prompt, one pool, one cleanup, the protocol's rules unchanged.

A Workflow script cannot start a process. Every command therefore still goes through a relay agent. Rule 2 makes that relay a pure transport: one PowerShell call, its exit code and its last line.

## Findings

The counts come from the blueprints, the modules, the installed scripts and the zt-digest sources at the versions named above.

### Decision points

| Measure | Digest (base, econ, math) | zt-idea-grow |
|---|---|---|
| Stages | 17 base stages and 6 slots; econ 21, math 24 | 2 framework stages; 11 rounds and cleanup inside `grow` |
| Modes | base 6, econ 11, math 10 | 2, plus the box-copy path that skips 4 rounds |
| Arguments | 58 to 66; about 30 change control flow; 3 are unused and 2 are aliases | 21 |
| Agent prompt kinds | about 12 (relay, step agent, job manager, chain manager, block manager, legacy stage manager, source probe, prepare, link-prepare, list-models, cleanup, architect, organizer) | 3 Haiku kinds (runner, DeepSeek manager, cleanup) |
| DeepSeek dispatch | the DSMP block is installed and its pool (`dsmpSlot`) caps the managers; `dsDispatch` is never called, and four hand-written manager prompts (`manage`, `driveAgent`, `chainPrompt`, `blockPrompt`) run the jobs | DSMP v1.1 `dsDispatch` for all 9 job families |
| Retry loops | about 16 distinct loops | 5 (fix, hub fix, DeepSeek self-check, manager retry, manual rerun) |
| Recovery paths for an unreadable relay line | 14, each bespoke | none; every unreadable line halts the run |
| Places that read a `card_locked` refusal | 7 in JavaScript, 1 in the batch finish | 4 in JavaScript, 11 in `grow.rs` |
| Copies of the retryable-fault classifier | 4 (two regexes in `zt-digest-lib.js`, one in `finish_batch.rs`, `fault_class` in `ledger.rs`) | none (no ledger) |
| Paths that finish a unit | 4 (block manager, chain step, relay batch, per-unit `finishOrFault`) | — |
| Paths that claim a unit | 4 (`ledger next` in the block prompt, `claim()`, `finish-batch -Take`, the terms and cards-book claims) | — |
| Lines of workflow JavaScript | 2,810 in the modules; each installed script carries its own copy (4,613 and 4,839 lines) | 366 in the cycle module; 1,220 in the installed script |

### Claude Code tokens

The token pilot (zt-econ-digest 6.2.0, grossman2002integration, 8 section groups) measured the workflow agents with `token-usage.mjs`. Weighted tokens are input + 1.25 × cache write + 0.1 × cache read + 5 × output, in units of each model's own input price. Sonnet costs about 3 times and Opus about 5 times as much as Haiku per token.

| Agent kind | Agents | Weighted | Share | What the agents do |
|---|---:|---:|---:|---|
| Haiku managers | 45 | 6.05M | 59% | run DeepSeek jobs, and run the deterministic steps of chains and blocks between them |
| relays | 46 | 2.77M | 27% | run one command each |
| Sonnet architect | 1 | 1.28M | 12% | plans the structure of the models |
| Opus organizer | 1 | 0.13M | 1% | decides the Organize merges, moves and notes |
| cleanup agent | 1 | 0.12M | 1% | one bridge call around `deepseek_settle` |
| **total** | **94** | **10.3M** | | |

The relay share falls to about one fifth with the `zt-relay` agent type (base 5.3.0, measured at 9.2k first-request context against 59.2k), which leaves about 0.5M. The managers are then about 80% of the Claude Code cost. A manager's cost grows with the number of its turns. A chain manager takes turns for every deterministic step of its chain and for every poll of its DeepSeek jobs. A manager that only polls long DeepSeek conversations takes few turns.

### Where the complexity comes from

**Most recovery paths answer a relay that cut or rewrote a line.** The incident record (blueprint P6, the trial log of 2026-10-20) names trials 3, 5, 10, 12 and 17 of grossman2002integration and run 5 of Stiglitz. In each, a Haiku relay or manager summarized a JSON line. Each incident added one bespoke reader: a fallback to `ledger status -Held`, a re-read of the Model address, a ledger read before a fault, the report line in place of the plan line. The bounded output lines of zt-digest 0.11.6 and the `zt-relay` agent type remove the cause. The 14 readers remain.

**LLM agents decide control flow in about a dozen places.** Every one of the following decisions has a mechanical rule.

- A Haiku probe picks the DeepSeek source. The user's rule fixes the source to the official one.
- Haiku agents copy arrays back after `prepare`, `prepare-link` and the model listing. The model listing has dropped the `form` field before, which let Theory Cards leak into the model layer.
- The chain manager evaluates `repeat while pending_jobs > 0`, the `optional` and `always` attributes, and the placeholder substitution from prose rules.
- The four hand-written digest manager prompts each restate the approval rules and the resume rule in their own words.
- The model layer reads the architect's `plans[].status` and the count of derivations from the agent's answer to choose between fill-only and derive-and-review.
- The organize stage reads the count of notes from the Opus answer to decide whether the notes cycle runs.
- zt-idea-grow's cleanup is a Haiku agent; the digests settle cleanup with `cleanup-run` and the bridge tool `deepseek_settle`, with no model in the loop.

**The declared failure policy differs from the behaviour.** In the digests only a throw, a missing output or a failed `expect` marks a stage failed. Most modules return `{status: 'failed'}` or `{status: 'partial'}` and the kernel records the stage as done, so later stages run. The digest stage halts only on a lock, a failed `prepare` or `prepare-link`, or a failed model listing in mode `models`. Blueprint P8 states that digest halts when no ready unit could be verified; `zt-group-cycle` returns `done` when zero groups ran. zt-idea-grow halts on every failure, including the report its blueprint promises to run always.

**Resume is a set of special cases.** `settledOnResume` skips five stages by their ledger layers, `IDEA_SAMPLE` changes in mode resume, the resume group count comes from the gate, and the math stages have no ledger units and run again in full. zt-idea-grow resumes from flags in `state.json`; its review, concept, organize and hub rounds run again in full, and its links round numbers its jobs from a count (see S0).

**The two workflows share patterns without sharing code.** zt-idea-grow copies the lock reader of `zt-digest-lib.js`, runs its relays through a generic Haiku runner without the `zt-relay` type, and cleans up with an LLM agent. The digests carry the DSMP block without its dispatch. zt-idea-grow's restatement round and concept round are two copies of one three-phase pattern.

## Proposals

Each proposal states the change, the decision points or the tokens it removes, the protection it keeps, and its risk. S0 repairs defects found during the inventory. S1 to S7 simplify both workflows. S8 to S10 apply to one workflow each. D1 to D4 move Claude Code work to DeepSeek.

### S0. Repair four defects first

These repairs are small and independent of the rest. They follow the rule that a fault found in a working system is fixed before the system is reshaped.

1. **zt-idea-grow's restatement review cannot find its files.** The cycle module writes `${st}\work` and `${st}\cards.txt` in template literals (`zt-grow-cycle.js` lines 176, 179, 193 and 199). JavaScript reads `\w` and `\c` as plain `w` and `c`, so the paths lose their separator, and `restate -Step prepare` finds no Cards file. The `catch` records each book as `skipped` and the run continues. Versions 3.0.0 and 3.1.0 wrote double backslashes. The harness regex `/-Cards '[^']*cards\.txt'/` also matches the broken path. The repair writes the separators correctly and makes the harness assert the exact path.
2. **zt-idea-grow's links round loses work on a rerun.** When `state.links` exists, `grow -Step links` returns only the count of unapplied jobs, and the module numbers jobs `gl-001` onward from that count. These numbers need not be the unapplied jobs. The repair makes the step print the job ids.
3. **The failure policy in the blueprints differs from the code.** S7 states one policy. Until S7 is built, the blueprint text states the present behaviour.
4. **Four documents say that Haiku picks the hub thresholds.** `hub_limits` derives them from the survey's quantiles. The header of `hub.rs`, `docs/hub-cards-design.md`, `docs/wiki/hub-cards.md` and the memory note on hub-stage roles state the present rule after the repair.

### S1. One command runner: the `zt-relay` relay everywhere

Every command of both workflows runs through one relay of the `zt-relay` agent type. Its whole task is to run one PowerShell line and return the exit code and the last line. The digest's step agents, its prepare, link-prepare and list-models agents, the source probe, and zt-idea-grow's generic runner become this relay.

- **Removes:** about 6 agent prompt kinds and the decisions they take (the source probe, the copied arrays, the `zt up` retry inside link-prepare).
- **Keeps:** the relay's `RUN_STEP` line, which stops a relay from refusing a step (trials 17 to 19). The service check moves into the command: a command that needs the ZT service starts or waits for it itself and exits 4 on a timeout, as `zt-digest` already does for service waits.
- **Source:** the DeepSeek source is the frozen value `official` (the user's rule), and `dshSource` leaves the argument list.
- **Tokens:** about 2.3M weighted less on a book of the pilot's size (the relays fall from about 60k to about 10k weighted each).
- **Risk:** low.

### S2. One recovery for an unreadable line: read the state again

Every command that changes state records its result in the run's own state: the ledger in the digests, `state.json` in zt-idea-grow. Each layer has one read-only status command that prints that state as one bounded line (`ledger status -Held <owner>` or `-Units <list>`, and a new `grow -Step status`). When a relay returns a line that is not one JSON object, the driver runs that status command once and continues from its answer. When the status line is unreadable too, the stage halts with a typed fault.

- **Removes:** the 14 bespoke recovery paths of the digests (the claim fallback, the blind-round counter, the partial batch re-run, the Model-address re-read, the plan-to-report fallback, the resume layer fallback and the others in the inventory). zt-idea-grow gains recovery it lacks today.
- **Keeps:** the rule that the ledger decides before any fault is recorded (trials 5 and 10), in one place.
- **Requires:** every command is idempotent and fenced by its claim token. The finish, fail, renew and resource commands already are.
- **Risk:** medium. Each removed path needs a harness case that cuts the line at that point and shows that the one status read gives the same outcome.

### S3. A command decides what happens next

This is the central simplification. Each layer gets one `next` command, which runs every deterministic step it can and stops at the next DeepSeek job set or at the end:

```
zt-digest <layer> -Step next -Out <out> -Owner <lane>     # digest layers
zt-digest grow -Step next -Out <out>                      # idea-grow
```

Its line is one of three shapes.

| Line | Meaning | What the driver does |
|---|---|---|
| `{"next":"jobs","jobs":[…],"unit":…,"token":…}` | DeepSeek work is needed | hands the jobs to `dsDispatch`, then runs `next` again |
| `{"next":"done", …counts}` | the layer is complete | ends the layer |
| `{"next":"fault","code":…,"cause":…}` | a typed fault (a lock, an exhausted unit, a refused binding) | halts or records, as S7 says |

The command claims, plans, applies, checks, reports and finishes inside one process, with the ledger as its memory. Its order of steps lives in Rust, where it is typed and tested, and its version is compiled together with the blueprint version, as `patch_compatible` already requires. Each job in a `jobs` line is already a DSMP job (`id`, `run`, `expect`, `approve`), so the driver passes it through unchanged. The driver becomes one loop with three branches:

```js
for (;;) {
  const line = await relay(`zt-digest ${layer} -Step next ...`)
  if (line.next === 'jobs') { await dsDispatch(line.jobs, { label: layer, phase }); continue }
  if (line.next === 'done') return line
  throw fault(line)
}
```

- **Removes:** the chain manager and its prose loop rules; the block manager's substitution and its renewal duty; the four claim paths and four finish paths (claim and finish happen inside `next`); the 28 branches of `zt-grow-cycle.js`; the per-module mode checks of the digests.
- **Keeps:** every rule the steps hold today, because the steps themselves do not change. `next` only calls them in the present order. Leases stay renewed inside the process (`start_heartbeat`); `ledger hold` covers the time a DeepSeek job set runs.
- **Framework:** the Workflow Framework Protocol freezes structure at install. The stage list stays in the blueprint. The order inside a stage is frozen by the zt-digest version that the blueprint names. `next` prints its step table on request, and the install check compares it with the blueprint.
- **Tokens:** the managers stop running deterministic steps, so each manager takes only polling turns. D1 removes most of the remaining relays between job sets.
- **Risk:** high, because it moves the most code. zt-idea-grow goes first: it has no ledger, one stage and the smaller cycle. The digest follows one layer at a time, starting with the events layer, whose chain is the simplest (plan, jobs, apply, report, finish).

### S4. One DeepSeek path: DSMP v1.1 in both workflows

Every DeepSeek job of both workflows goes through `dsDispatch` and the protocol's one manager prompt. The four hand-written digest manager prompts leave the library. The protocol's rules apply as written, with these bindings.

- **Approval:** the command that plans a job writes its `approve` rule, naming the exact commands the job may run (its SELF-CHECK command, and under D1 its `next` command) and the folder it may write. The protocol's default rule covers every other job. One generated rule replaces the four prose copies.
- **Success:** the job's `expect` regex matches the status line the command asks DeepSeek to end with. The command that applies the answer decides whether the work passes; the regex only tells the manager whether a reply arrived in the right form, which is what the protocol's one retry needs. zt-idea-grow's `OK`/`FLAG` reply becomes a fixed status line of the same kind, and DeepSeek's SELF-CHECK stays a writing aid.
- **Retry:** the protocol's one retry covers a failed conversation. The ledger's attempt count covers a unit whose answer the command refused. These two retries answer different faults and are the only two.
- **Timeout:** the protocol's run-time budget of 240 min. A digest block is sized to finish well inside it (`dshRunCap` stays below the budget).
- **Cleanup:** collect-then-delete per conversation (rule 4) is the manager's own step. Workspace removal (rule 5) runs as the digests run it today: `cleanup-run plan`, one `deepseek_settle` bridge call, `cleanup-run apply`. zt-idea-grow takes the same cleanup, and its cleanup agent leaves the workflow.
- **Pool:** one pool for the whole script, at most 8 managers and 64 conversations (rule 3). The digests already enforce it through `dsmpSlot`.
- **Effort and source:** high effort, DeepSeek V4.1 Flash from the official source, for every job.
- **Risk:** low to medium. The harness checks that the generated approval rule of each job family equals its present prose rule, and that every DeepSeek job of both installed scripts passes through `dsDispatch`.

### S5. One fault classifier, in Rust

`fault_class` in `ledger.rs` is the only classifier. `ledger fail` and `finish-batch` record a cause and a code, and the ledger classifies. The two JavaScript regexes and the copy in `finish_batch.rs` are removed. The JavaScript side never reads a class; it reads the ledger state after the fault.

- **Removes:** 3 of 4 copies, and the drift between them (one copy lacks `cleanup_transient`).
- **Risk:** low. A table test runs every cause seen in the trial logs through `fault_class` and compares it with today's outcome.

### S6. One lock reader

The `card_locked` refusal is read in one shared function of a library module that both workflows load. Under S3, the `next` command prints the lock as a typed fault, so the script needs only the one check in its loop. The Rust side keeps one `stop_stage` helper per crate.

- **Removes:** 10 of the 11 JavaScript sites.
- **Keeps:** the Tree Lock rule (#138, #140): a lock is a destination decision, it is never retried, and the stage stops.

### S7. One failure policy and resume by rerun

A stage ends in one of two ways. Its last command reports the layer `done`, and the stage is done. Anything else throws, and the kernel applies the stage's declared `onFail`. A module never returns a `failed` or `partial` status as a finished stage. The run report lists the halted stage and its fault.

Resume is a rerun with the same output folder. Every layer reads its own state and skips its done work; this already holds for the ledger units. The math stages and the review, concept, organize and hub rounds of zt-idea-grow get recorded state, so they skip too.

- **Removes:** mode `resume` as a separate mode (it becomes the full run on an existing folder), `settledOnResume`, the resume branch of `IDEA_SAMPLE`, the resume group count, and the difference between the declared and the real policy.
- **Keeps:** `verify_binding`, which refuses a folder prepared by another contract; the rule that a tool cannot resume a run of another minor version; the trial principle that a broken trial is fixed and resumed at its break; the rule that a resume never rebuilds a finished model.
- **Risk:** medium. Stages that today continue after a soft failure (terms, organize, bridge, finalize, cleanup) keep `onFail: continue`; the others halt where they continue today, so a pilot must show that no run stops on a failure that was harmless before.

### S8. Digest: fewer modes and arguments

- **Modes:** `full` and one argument `only: [stage, …]`, applied by the kernel to the stage list. The single-stage modes (link, cards, figures, audit, bridge, models, explain, events, findings, math) become values of `only`. The modules no longer read the mode; the 8 mode-shaped returns of the report module become one.
- **Bridge apply** stays a separate run that the user starts. It is the user's decision gate and keeps its one check in the argument gate; the checks in the kernel and in `zt-bridge` go.
- **Arguments removed:** the unused `perManager` (as a control value), `linkPerManager` and `coreBudget`; the aliases `dshPerSonnet` and `redoModels`; `dshSource` and `dshModel` (S1).
- **Arguments frozen** in the blueprint, out of the argument list: `cardFixRounds` (1), `reviewRounds` (2), the close rounds (3), `maxAttempts` (3), the effort values (high). A new value is a blueprint release, which is how the trials changed them anyway.
- **Layer switches:** each switch is stored once, in the ledger's run options at prepare, and the stage `when` conditions read the ledger. Today the switches are frozen twice.
- **Dead code removed:** `STAGES`, `manage`, `runStage`, `drain`, `jobOf`, `resourceList`, `finishCleanup`, `renew`, `CARD_CHECKS`, `LINK_PER_MANAGER`, the `cleanup` part of `driveAgent`, `cleanupDsh` (a no-op that `zt-bridge` still calls), the option `remove: 'delete'` (three modules pass it and no code reads it), the identical branches in `zt-explain`, and `runMathScript` (it duplicates `runScript` without the lock check).
- **Variants:** the `models` and `explain` fills move into the base as optional layers, since both variants copy them unchanged.

### S9. zt-idea-grow: one review driver and one copy rule

- **One review driver.** The restatement round and the concept round run the same three phases (prepare every book, one DeepSeek dispatch, judge and report per book). Under S3 both are layers of `grow -Step next`. Before S3, they share one driver function.
- **One copy rule.** A run on a box copy skips the rounds that write hubs and the register. Today four JavaScript conditions on `ztConfig` decide this. The rule moves into `grow -Step next`, which reads the box pin and plans no such round; the run report states each skipped round with its reason.
- **Options:** `fixRounds` and `hubFixRounds` are frozen at 1. `hubEffort` merges into `effort`. `register: plan` and `organizeTrees` are kept only if a run has used them; the inventory found none.

### S10. Digest: commands read the agents' products

The model layer and the organize stage read two control values from an agent's answer. Under this proposal a command reads them from the files the agent wrote. `model -Step plan-status` reads `plan.json` and prints the plan status and the count of derivations; `organize -Step decisions-status` reads `decisions.json` and prints the count of notes. The content (the architect's plan, the organizer's decisions) stays with its author.

### D1. DeepSeek steps a whole block

Under S3 the driver calls a relay before and after each DeepSeek job set. D1 moves that stepping into the DeepSeek conversation. A block of at most 8 units of one layer becomes one DSMP job whose task is to run the layer's `next` command, write the content each `jobs` line asks for, run `next` again, and stop when `next` prints `done` or `fault`. The dsh goal mechanism of bridge 0.11 (the `goal` field of `deepseek_run`, passed through the job's `run` object) keeps the conversation working until the goal is met, as zt-issue-dev already does.

- **Commands still decide.** `next` claims, validates, applies and finishes. DeepSeek only carries the line from one call to the next and writes the content between them. A step DeepSeek skips or repeats does no harm, because every command is idempotent and fenced by its claim token.
- **Approval:** the job's `approve` rule allows exactly the layer's `next` command with that block's owner, its SELF-CHECK command, reads, and writes inside the job's folder. The manager rejects every other command.
- **Verification:** after the dispatch, one relay runs `ledger gate` for the layer. Open units go back to the ledger by lease expiry and are claimed by the next block, as today.
- **Removes:** about two relays per DeepSeek job set, and the managers' turns between job sets. A manager then takes turns only to start conversations, answer approvals and poll.
- **Keeps:** the protocol's 240-min budget per conversation; the block size of 8 units keeps a block inside it.
- **Risk:** medium. DeepSeek runs write commands it does not run today. The approval rule and the fencing bound what it can change; the pilot shows whether it follows the loop.

### D2. DeepSeek decides Organize in the digest

The digest's Organize decisions (merges, moves and notes for the book's Literature tree) are made by one Opus agent. zt-idea-grow makes the same kind of decision for its Topics with one DeepSeek job (`go-001`), checked by the O1 to O8 rules of `validate_organize` and planned against the archive by `organize-apply -Open`. D2 gives the digest the same mechanism: one DSMP job writes `decisions.json`, the O1 to O8 validation (`validate_organize` in `grow.rs`, extended to the Literature scope) checks it, and `organize-apply` carries out the valid decisions.

- **Removes:** the Opus organizer, the last Opus agent of the digest, and one decider kind (both workflows use DeepSeek for Organize).
- **Tokens:** small in count (0.13M weighted in the pilot) and about five times dearer per token than Haiku.
- **Keeps:** the read-only `-Allow` list of the Organize reads (base 5.2.1), and the checks applied before any move.
- **Risk:** low to medium. The pilot compares the decisions with the Opus decisions of the token pilot on the same book.

### D3. DeepSeek plans the models, as a trial for the user's decision

The Sonnet architect plans each model's structure: its level, its parts in build order, and where the Derivation parts go. The user decided on 2026-10-14 that this stays on Sonnet and that DeepSeek does all the writing. The architect is 12% of the pilot's Claude Code tokens at about three times the Haiku price per token.

D3 proposes one comparison run and leaves the decision to the user. A DeepSeek job, given the same architect prompt (`model-architect.md`, `model-architect-math.md`) and the same context file, writes `plan.json` for the models of grossman2002integration. `model -Step plan-status` and the existing plan checks validate it. The comparison sets the two plans side by side: the levels, the part lists, the Derivation positions, and the faults the later model review finds. The architect role moves only if the user chooses it after the comparison.

### D4. Outside the workflow: DeepSeek diagnoses and repairs

The main session that starts, watches and repairs a run spends Claude Code tokens of its own at Opus prices. On ebrill2001modern its monitoring and repair spent about 24M weighted Opus tokens, which costs more than all 613 relays of trial 1 (token proposal, finding 2). Two parts of that work move to DeepSeek through DSMP.

- **Diagnosis.** When a run halts, one DSMP job reads the run's ledger report, the halted stage's last lines and the relevant module, and writes a short diagnosis file: the failing command, its fault, the cause, and the repair it proposes. The main session reads that file and the watcher's lines (`watch-run.mjs`) and nothing longer.
- **Repair.** A repair of project code goes through zt-issue-dev: an issue states the fault, DeepSeek writes the test-first fix under a goal, and the main session reviews the diff and runs the tests. The rule that a workflow bug is fixed test-first, with its wiki update, and the run resumed at its break, stays.
- **Risk:** low. Both moves change who writes the first draft of the diagnosis and of the fix. The main session still decides what is committed and when a run resumes.

## Targets

The decision-point targets are counts of code paths, checked by reading the installed scripts after the release. The token targets are estimates on a book of the pilot's size; the pilot measures them with `token-usage.mjs`.

| Measure | Digest now | Digest target | zt-idea-grow now | zt-idea-grow target |
|---|---:|---:|---:|---:|
| Agent prompt kinds | about 12 | 3 (relay, the DSMP manager, the architect while it stays on Sonnet) | 3 | 2 (relay, the DSMP manager) |
| LLM control-flow decisions | about 12 | 0 | 6 | 0 |
| DeepSeek dispatch paths | 4 manager prompts | 1 (`dsDispatch`) | 1 | 1 |
| Recovery paths for an unreadable line | 14 | 1 | 0 | 1 |
| Claim paths / finish paths | 4 / 4 | 1 / 1 | — | — |
| Fault classifier copies | 4 | 1 | — | — |
| JavaScript lock-reading sites | 7 | 1 | 4 | 1 |
| Retry loops | about 16 | 4 (the DSMP retry, ledger attempts, the card fix round, the model review and close rounds) | 5 | 2 (the DSMP retry, the fix round) |
| Modes | 11 (econ) | 1 plus `only` | 2 | 1 plus `only: hubs` |
| JavaScript branches in a stage driver | dozens per module | 3 per layer | 28 | 3 |
| Claude Code weighted tokens, workflow agents | 10.3M (pilot) | about 4M to 6M (S1, S3, S4, D1, D2; the architect unchanged) | not measured | measured by its pilot |

The token estimate for the digest assumes that S1 brings the relays to about 0.5M, that S3 and D1 halve the manager tokens by removing their stepping turns, and that D2 removes the Opus agent. D3, if the user chooses it, removes about 1.3M more.

## What may not change

These properties hold before and after every step of this proposal.

1. The Cards are the same in kind and quality: the same layers, the same prompts to DeepSeek, the same zt-check rules and the same SELF-CHECK lines. A role that moves to DeepSeek (D2, and D3 if chosen) keeps its prompt and its checks.
2. The ledger stays the source of truth, and every write stays fenced by its claim token.
3. A `card_locked` refusal stops the stage and is never retried.
4. The user's decision points stay: the Work selection when `prepare` needs one, the `until` checkpoint, and the separate bridge-apply run.
5. Every DeepSeek task follows DSMP v1.1. DeepSeek is V4.1 Flash from the official source at high effort, and every conversation is deleted after its result is collected, never archived.
6. Digests write only Literature trees. zt-idea-grow is the only writer of Topic trees, hubs and the register.
7. A finished model is never rebuilt by a resume.

## Acceptance

The proposal applies its own rule to its acceptance: two gates.

1. **Tests.** The existing suites pass (`run-all.ps1`, both framework harnesses, `cargo test` for zt-digest and zt-check). Each removed recovery path gets one harness case that cuts the relay line at that point and shows the status read gives the same outcome. Each `next` command gets a Rust test per line shape. The harness checks that every DeepSeek job of both installed scripts goes through `dsDispatch`, and that its approval rule names only the commands its family needs.
2. **One pilot per workflow, with one deliberate stop.** grossman2002integration runs on the econ digest in a copy of the box, and zt-idea-grow runs on that copy afterwards. Each run is stopped once in the middle and rerun with the same folder. A pilot passes when its outcome equals the last clean run (for the digest, the token pilot's measures: the same models, no model fault, a complete run report) and the rerun finishes without redoing done work. This also covers the fault and resume behaviour that the token pilot did not exercise. Each pilot reports its Claude Code and DeepSeek tokens by agent kind; the report is a measurement and adds no gate.

D3 has its own comparison run and the user's decision, as its section states.

## Order

Each step is a release of its own, with its tests and wiki update.

1. S0, the four repairs (zt-idea-grow patch release; documentation).
2. S8's dead code and unused arguments, S5 and S6. These remove code without changing behaviour.
3. S1 and S4: one relay type and one DeepSeek path (DSMP v1.1 `dsDispatch`) in both workflows.
4. S2, S7 and S10: one recovery path, one failure policy, resume by rerun.
5. S3 and S9 in zt-idea-grow (major release 4.0.0), with its pilot.
6. S3 in the digest, one layer per release, starting with events; then S8's modes (major releases: base 6.0.0, econ 7.0.0, math 6.0.0), with the digest pilot.
7. D1 and D2 in the digest, measured on the next pilot. D4 for the main session, at any point after step 3.
8. D3's comparison run, then the user's decision.

Steps 1 to 4 keep the present structure and can stop there if step 5 shows that `next` costs more than it saves. Steps 5 to 8 change the structure or a role, and each starts only on the user's go-ahead.

## Implementation status

Built on 2026-10-23 and installed, uncommitted: zt-digest 0.13.0, zt-idea-grow 4.0.0, zt-digest base 5.4.0, zt-econ-digest 6.4.0, zt-math-digest 5.4.0, and the shared library `scripts/workflow-shared/modules/zt-workflow-lib.js` v1.

| Step | zt-idea-grow 4.0.0 | Digests (base 5.4.0) |
|---|---|---|
| S0 repairs | done: the restatement paths, the link round's job ids, the Lock check on DeepSeek replies (it read `answer`; DSMP gives `reply`) | done: P8 states the present policy; the hub-threshold documents |
| S1 one relay | done (the runner role leaves) | done: drive's step commands, prepare `-Brief`, prepare-link `-Brief`, the model list; the source probe leaves (official source frozen) |
| S2 one recovery | done (`grow -Step status`, a checksum on each jobs line) | not done (the 14 paths stay until S3) |
| S3 `next` | done (`grow -Step next`, `grow_next.rs`) | not done |
| S4 dsDispatch | done (the command writes the job template and its approval rule) | not done: see below |
| S5 one classifier | — | done (`cause_code`, `fault_class`; no `-Class`) |
| S6 one Lock reader | done (shared library) | done (shared library) |
| S7 policy, resume by rerun | done (`next.json`; other args refused) | text only (S0) |
| S8 modes and args | — | dead code and unused args removed; modes and frozen settings not done |
| S9 | done (frozen args; every round runs on a box copy through the pin) | — |
| S10 | — | done (`model -Step plan-status`; organize-apply's `notes`) |
| D1–D4 | — | not done |

S4 in the digests meets a dependency the proposal did not name: DSMP deletes each conversation as soon as its answer is collected, while the model compiler's split and close rounds continue the writers' own conversations (`continue` in `model/compiler.rs`), and every digest manager records each conversation, its answer evidence and its usage in the ledger (R322–R346), which DSMP managers do not. Moving the digests to `dsDispatch` therefore either drops the continuation (the fallback starts a new conversation without the writer's context, a quality risk in the model layer) or needs a DSMP extension that keeps named conversations until a later dispatch. That choice is the user's.

Tests at install: zt-digest 381 unit tests, zt-check 32, `run-all.ps1` 92/92, `framework-harness.mjs` 349, `grow-workflow-harness.mjs` 35, `bridge-workflow-harness.mjs`, and the dry runs of all three blueprints.

Pilot findings (2026-10-24, the digest pilot on the test box with one deliberate stop at Link):

1. A digest resume after a stop needs an operator step: `zt-digest ledger reclaim -Out <out> -Release` frees the stopped invocation's quiet claims; without it `ledger resume` refuses with `run_active` until the leases (the provider-run cap plus 300 s) end. The refusal names the remedy. Under S7 (resume by rerun) the resume itself should release quiet claims; staged for the next digest release.
2. The stopped block manager left its eight running Link conversations without answers, so their cleanup units failed (`cleanup_evidence_missing`) and the run report said complete false although every Card was done. Fixed in zt-digest 0.13.1 (the sweep records a fenced attempt's conversation); a run has one resume, so the pilot itself keeps the eight failed cleanup units.
3. zt-idea-grow 4.0.0 on the test box (both books, every round on, organize on): 16 sections, 8 Topics, 113 Topic Cards, 74 Links across trees, 9 bridges, Topic share 0 → 44%, 37 restatements resolved by rewrite, 688 concept findings, 6 topic and 3 field hubs, register published (152 terms). The deliberate stop in the section jobs (7 of 16 answered) was followed by a rerun that dispatched only the 9 unanswered jobs. The run then stopped at the register: zt-digest's register staged an empty `root_before` for an existing register, so its first plan with term parts stopped half-published (fixed in 0.13.2, with a test; the half-published root edit was completed by hand on the test box) and the stop's cause had lost the error sentence (fixed in 0.13.2). A rerun redid only `hubs-report`. Five I7 faults remained: the restatement review's rewrites ran after the only check round (fixed in zt-idea-grow 4.0.1, zt-digest 0.13.3: the review runs before the check; the recheck on the pilot's own folder fixed all five, faults left 0).
4. The digest pilot otherwise matched the token pilot's quality: 162 Cards, 6 models (5 formal, 1 concept) with no issue or build fault, 24 of 24 Derivation parts explained, 7 of 7 figures shown, 0 orphans, 0 broken Links, 10 soft R6 wording faults. Its resume skipped every finished layer and rebuilt no model. The live box stayed at revision 6697 throughout.
