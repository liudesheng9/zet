# Digest Run Issues (stiglitz2014learning): Proposal

Status (2026-10-17): proposal, second version. Nothing in it is built yet, except the validator fix of I14 (commit `bb7e905`). The workflow `zt-econ-digest` is a frozen install (v3.3.0). Its changes go through the workflow-framework-protocol pre-flight as a new version of the digest base, so `zt-math-digest` gets them too.

## Goals

The user set three goals for this proposal:

- **G1. Nothing is lost in one run.** Every group, model, event, link, and hub that a run sets out to make is made, or it is named with its reason.
- **G2. At most one rerun.** When a run ends, one rerun covers everything the run left. The rerun never touches work that is done.
- **G3. The slow tools get fast.** The zt-digest commands that took minutes take seconds.

The means for G1 and G2 is a **run ledger** inside zt-digest (Part 1). Each layer registers its work in the ledger and hands its outputs to the next layer through it. The means for G3 is a set of changes in zt and zt-digest (Part 2), based on measurements of the cause.

## Sources

The session "Learning Society growth approach" (`local_be5c6028-325f-4954-a2d5-2cd60a298934`) ran `zt-econ-digest` in mode `full` on *Creating a Learning Society* (Stiglitz and Greenwald 2014, Scholar Archive file 107, citation key `stiglitz2014learning`), then two mode `models` repairs. The evidence comes from:

- the run outputs `C:\Users\chenx\AppData\Local\Temp\claude\E--zet\6844536f-716e-4b04-bf38-48dbcb1fcd9c\tasks\{win4uo8lq,wyryval0h,wrh1hcmcz}.output` (logs, results, and per-agent `workflowProgress` with start times and durations);
- the subagent transcripts under `C:\Users\chenx\.claude\projects\E--zet\6844536f-716e-4b04-bf38-48dbcb1fcd9c\subagents\workflows\`;
- the staging folder `D:\zt-digest\stiglitz2014learning` (`audit.md`, `bridge\report.md`);
- `D:\zt\zt.log`, and read-only queries and timings on the live slip-box (`D:\zt\zt.sqlite3`: 10,891 Cards, 218 trees, 10.7 MB of Card text);
- the code: `src/mcp.rs`, `src/ops.rs`, `src/main.rs`, `tools/zt-digest/src/{audit,card,snapshot,prepare}.rs`, `tools/zt-check/src/mcp.rs`.

Durations are wall-clock minutes from the run start (03:55 UTC).

## The three runs

| Run | Mode | Wall time | Agents | Claude tokens | Result |
| --- | --- | --- | --- | --- | --- |
| `wf_e0953c3d-96e` | full | 424 min (7 h 4 min) | 616 | 46.1 M | 1,404 Idea Cards from 164 of 171 groups; 104 of 170 models built; 16 Events; 11 Findings; 270 hubs; 23 of 35 card-check blocks failed |
| `wf_96cd75b3-88a` | models, `only` = 66 failed | 42 min | 26 | 2.0 M | 6 of 66 attempted, 5 built (60 skipped as "already have their parts") |
| `wf_cb793e9d-1ef` | models, `rebuild`, `only` = 61 | 54 min | 141 | 11.4 M | 20 built, 41 failed (19 lock, 15 validator, 7 other) |

After three runs, 129 of 170 models are built and 7 groups have no Cards. The first session is still at work on the last 41 models when this document is written. The book needed three runs and still has gaps: G1 and G2 failed.

### Timeline of the full run

| Minutes | Stage | Agents running | Note |
| --- | --- | --- | --- |
| 0–6 | Prepare, terms | 1 | |
| 6–99 | Digest | 8 managers | 35 managers, about 19 min each |
| 19–148 | Per-block card cycles | up to 16 | most ran at 90–148, queued behind the 16-agent cap |
| 148–229 | Models, Events, Findings | up to 16 | Models waited 49 min (99–148) for the card-cycle backlog |
| 229–309 | Link | 8, then 1 | the zt daemon died and restarted at minute 252; one DeepSeek run alone held minutes 280–309 |
| 309–333 | Link check | 1 | |
| 333–382 | Organize | 1–4 | |
| 382–393 | Figures, Bridge | 1 | |
| 393–422 | Hubs | 1–3 | |
| 422–424 | Audit | 1 | |

## Why the tools are slow: measurements

**M1. Every zt read call rebuilds the whole slip-box in memory.** The `zt mcp` server answers each `get_card`, `get_tree`, `get_model`, and `list_trees` with `Snapshot::read` (`src/mcp.rs:245-337`). `Snapshot::load` (`src/ops.rs:463`) loads all 10,891 Cards, parses all of them, builds the Link lifecycle, and indexes every inbound Link. It keeps nothing between calls. (The GUI keeps its Snapshot and reloads it only when SQLite's `data_version` changes, `src/gui.rs:285`. The MCP server does not.) Measured on the idle live service:

| Call | Time | Answer size |
| --- | --- | --- |
| `list_trees` | 452 ms | 122 KB |
| `get_card` (any Card) | 233–234 ms | 11–28 KB |
| `get_model` | 237 ms | 21 KB |
| `get_tree` (Topic tree 216) | 236 ms | 45 KB |
| `get_tree stiglitz2014learning` | 348 ms | 2.3 MB |

So one call costs about 0.24 s of CPU, whatever it asks for. The cost grows with the whole slip-box, not with the book: `mascolell1995microeconomic` alone is 5,092 of the 10,891 Cards. Every new book makes every later digest slower.

**M2. The digest tools make thousands of such calls.**

| Command | zt calls | Idle cost (calls × 0.24 s) | Measured in the run |
| --- | --- | --- | --- |
| `snapshot-topics` (per group) | 1 `list_trees` + 218 `get_tree` | about 53 s | p50 61 s, p90 101 s, max 222 s; 171 times |
| card audit (`card plan`, `check`, `report`) | 218 `get_tree` + about 1,500 `get_card` | about 7 min on one worker, about 70 s on 6 | `card plan` p50 128 s, p90 242 s; most `check` and `report` calls ran past 600 s |
| `prepare-link` | 218 `get_tree` + `get_card` per Card of the book | about 7 min | 576 s |

The card audit reads with 6 worker processes (`read_many`, `tools/zt-digest/src/audit.rs:152`). Each worker is its own `zt mcp` process that rebuilds the Snapshot on every call. With 16 agents at work, up to about 100 such processes rebuilt 10,891 Cards at the same time. The machine was CPU-bound, so each call took far longer than 0.24 s.

**M3. Every write rewrites every Card, under the edit lock.** `insert_card`, `update_card_text`, and the other writers call `regenerate_reverse_links_tx` (`src/main.rs:2314`). It loads all Cards, parses them, builds the Link lifecycle, and then runs one `UPDATE` per Card for all 10,891 Cards, to refresh each Card's "Reverse links" section. Measured on a copy of the database: the `UPDATE` of all rows alone takes 185–219 ms, and the load and parse take about 230 ms more (M1). So one write holds the edit lock for at least about 0.45 s and writes about 10.7 MB. All writers of the run share that one lock, so the whole run can make at most about two writes per second. A model `apply` makes hundreds of writes (p50 188 s in the run). A writer waits at most 20 s (`EDIT_PATIENCE`, `src/ops.rs:19`), so when about 40 writes are queued in front of it, it fails. This is the cause of the 66 failed models (I8).

**M4. The zt daemon died during the run.** `D:\zt\zt.log` records at minute 252: "service record of daemon 48612 cleared: the daemon ended without `zt down`", then a restart. Every zt call checks that the service is up (`require_service_up`), so calls made in that window failed. The one DeepSeek link run that took 56 minutes (I5) was in progress at that time.

## Issues

### A. Running time

**I1. Every digest group reads the whole slip-box** (M2, `snapshot-topics`). 23 of 167 calls ran past the agent's 120 s default command timeout.

**I2. Every card-cycle step audits the whole book through 1,700 zt calls** (M2, `card.rs:264`, `audit.rs:271`), three times per block.

**I3. The per-block card cycle costs much and finds little.** 204 agents and 13.9 M tokens (30 % of the run). In 33 of 35 blocks, plan found 0 hard faults and reviewed 4–5 sample Cards. Only block 91–95 had faults (3 Cards, 8 faults). The later whole-book link check found 53 faults and 179 orphans and fixed them to 1.

**I4. The card-cycle backlog delays the model layer** by 49 min (minutes 99–148).

**I5. One slow DeepSeek run holds a whole stage.** A link run in groups 65–72 ran 3,376 s (56 min), with 3 `job_kill`. No step caps the time of one DeepSeek run.

**I6. The tail is serial.** Minutes 309–424 ran 1–4 agents. Figures and Bridge wait for Hubs without need.

**I7. Agent count drives token cost.** 616 agents used 46.1 M tokens, 75 k per agent. A trivial agent (7 tool calls) used 66.6 k tokens. Most Claude tokens are the start-up cost of each agent. The Workflow runtime has no shell step, so every command needs a full agent.

### B. Concurrent writes and the edit lock

**I8. Model writes fail on the edit lock** (M3). 66 of 170 models failed in the full run, almost all with `zt: edit in progress`. In the third run, with only the model stage, 19 of 61 still failed.

**I9. A lock-file error is not retried.** Three models failed with `zt: failed to remove D:\zt\zt.edit.lock: ... (os error 32)` from `remove_stale_edit_lock` (`src/main.rs:1998`). `acquire_edit_lock_within` (`src/main.rs:2005`) retries only the text `edit in progress`.

**I10. A failed write can repeat a paragraph.** The slip-box now has 18 repeated Link paragraphs in 15 Cards. `stiglitz2014learning/22|b` has the same three-Link paragraph 5 times. Others: `stiglitz2014learning/9`, `stiglitz2014learning/16|a`, `103/1` and five Cards under it, `211/1`, `232/1|a`, `232/1|b`. zt-check and the audit do not report it.

**I11. A failed apply leaves placeholder parts.** After run 2, 304 parts in 46 models still said `待写`, and 15 models had no parts.

### C. Lost work and reruns

**I12. Failed groups are lost, and no mode re-digests a group.** Groups 92–95 and 168–170 have no Cards. Manager 19 started the DeepSeek run for group 91 before its snapshot, reported an order fault, and stopped groups 92–95 "awaiting the parent's go-ahead", a path that does not exist. Manager 34: the snapshot for group 168 hung twice, and the manager gave up groups 168–170. The digest stage still reported `done`, and the run `halted: null`. Link, Organize, and Hubs then ran on the incomplete book.

**I13. A finished background command still counts as a failure.** When a card step ran past the agent's timeout, the agent returned "no output" and the block was marked failed, but the command went on. Of 184 background task files, many hold a valid final JSON line (for example `bm92wl0p5`: groups 131–135 report, exit 0); 41 are empty, because their commands were killed when the agent ended.

**I14. The plan validator refused models built from linked parts.** 15 models. Fixed in `bb7e905`.

**I15. A rerun skips placeholder models.** Mode `models` skips a model that "already has its parts", even when its parts say `待写`. Run 2 asked for 66 models and attempted 6.

**I16. One failed model makes later models in its batch fail.** Seven models failed with `reference X#A: model X has no part A yet`.

**I17. The prompts forbid retries of transient faults.** Agents wrote "Not retried, per the task" for lock errors and timeouts.

**I18. The truth about a unit of work is an agent's free text.** Whether a group, block, or model is done comes from what a Haiku agent writes in its result. Nothing checks it against the slip-box, and nothing keeps it after the run. A rerun therefore has to be planned by hand (the first session read SQLite to find unbuilt models).

### D. Agent behaviour

**I19. Haiku agents use tools that do not belong in an automated run**: `mcp__terminal__run_in_terminal` (it opened a terminal tab in the user's app), `Monitor` loops, blocked `sleep` commands, a hand-off to a new session.

**I20. Commands run without the long timeout.** Snapshots ran under the 120 s default; card steps ran into the 600 s cap.

**I21. Managers break the step order.** Managers 18, 19, and 20 started DeepSeek before the snapshot. The order lives only in the prompt.

### E. Reporting

**I22. The DeepSeek cost is wrong.** The run subtracted two Harness ledger totals (128.91 − 145.47) and reported `"cost": -16.5626`. The ledger total drops old entries, so the difference is not the run's cost. The first session then reported the ledger total (about 129) as the run's cost.

**I23. The report undercounts placeholder parts** ("14 parts still 待写"; the slip-box had at least 304).

**I24. No tool reports a book's build state.** `zt-digest list-models -CitationKey ...` failed on its parameters.

**I25. A stage with failed items reports `done`.** Failures sit deep in a 1.2 MB result (`result.result.digest[].status`).

**I26. The audit and the link check disagree on orphans** (0 left after the link check; 157 in the audit 90 minutes later).

### F. Left in the book

From `audit.md`: 157 orphans; 77 Card language faults; 15 Cards with no kinded Link; 107 of 170 models not self-sufficient; 98 of 233 Derivation parts with 思路 and 解读 (`explain` is skipped in mode `full`); 13 figures named but not shown; 57 Bridge proposals open (by design).

### G. Operator friction

**I27. The workflow does not take a book title.** The session found the archive file, chose `startHeading` 前言, `endHeading` 注释, and `skipHeadings`, and wrote the BibTeX entry by hand.

## Part 1. The run ledger (G1, G2)

### The idea

Today each stage gets its work list from the workflow script, gives it to agents, and believes what they answer. A lost answer is lost work.

With the ledger, zt-digest owns the work list. Every unit of work of every layer is a row in one database in the staging folder. A unit leaves the list only when zt-digest has **checked the slip-box** and found the unit's outputs there. When a layer finishes a unit, zt-digest registers what it made, and those outputs become the units of the next layer. The workflow script only asks the ledger "what is ready" and "is this layer complete". The agents never mark a unit done; the zt-digest command that finishes the unit does.

### L1. The ledger store

`<out>\ledger.sqlite3`, one per book (WAL mode, busy timeout, so many processes can use it at once). Tables:

- `units`: `layer`, `unit` (for example `digest/g092`, `models/stiglitz2014learning/7|a`, `hubs/216`), `state`, `attempts`, `owner`, `lease_until`, `fault_class`, `fault`, `inputs` (JSON), `created_by` (the unit that registered it).
- `deps`: unit → unit it waits for.
- `outputs`: unit → address or file it made, with its kind (`idea`, `model`, `event`, `link`, `file`).
- `events`: an append-only history of every state change, with time and owner.
- `ops`: the idempotency keys of writes already applied (L6).

### L2. Unit states

| State | Meaning | Next |
| --- | --- | --- |
| `registered` | known, waits for dependencies | `ready` when all `deps` are `done` |
| `ready` | can start | `claimed` |
| `claimed` | an owner works on it; a lease runs | `done`, `retry`, `failed`; back to `ready` when the lease ends without renewal |
| `retry` | failed with a transient fault | `ready` after backoff, at most `maxAttempts` (default 3) |
| `failed` | failed with a content fault, or used all attempts | the rerun list |
| `blocked` | a dependency is `failed` | the rerun list, with the dependency named |
| `done` | outputs verified in the slip-box | final; a rerun never touches it |

Transient faults: the edit lock, a timeout, a DeepSeek run over its cap, the zt service down, an MCP error, a lost agent (lease ended). Content faults: validator faults, zt-check hard faults left after the fix rounds, a DeepSeek answer that fails its SELF-CHECK twice.

The lease fixes I13: a command that goes on in the background after its agent ended renews the lease itself and writes its own result into the ledger. A command that was killed lets the lease end, and the unit goes back to `ready`.

### L3. Commands

| Command | What it does |
| --- | --- |
| `zt-digest ledger init -Out <out>` | creates the ledger; `prepare` calls it and registers the digest units |
| `zt-digest ledger next -Layer <l> -Take <n> -Owner <label>` | claims up to n ready units; prints them as JSON |
| `zt-digest ledger done -Unit <u> -Outputs <file>` | verifies the outputs in the slip-box, records them, registers the next layer's units; refuses if the check fails |
| `zt-digest ledger fail -Unit <u> -Class transient\|content -Fault <text>` | records a fault |
| `zt-digest ledger gate -Layer <l>` | prints `complete`, or the units not done with their states; exit 0 only when no unit is `ready`, `claimed`, or `retry` |
| `zt-digest ledger status [-Layer <l>]` | coverage per layer: done, failed, blocked, open (replaces the SQLite reads of I24) |
| `zt-digest ledger rerun-list` | the `failed` and `blocked` units, with faults; the input of mode `resume` |

The existing step commands call the ledger themselves: `model apply` marks its model units `done` or `failed`; `card report` marks its unit; the new `group-start` and `group-finish` (L4) wrap a digest group. So most agents never call `ledger done` by hand.

### L4. Layer contracts

Each layer states its unit, what "done" means in the slip-box, and what it hands on.

| Layer | Unit | Registered by | Done when (checked by zt-digest) | Hands on |
| --- | --- | --- | --- | --- |
| digest | group `gNN` | `prepare` | the anchor Card `<book>/NN` exists; `owned-gNN.txt` lists the group's Cards, and each exists; the DeepSeek SELF-CHECK line is present | `cards/gNN` (offline check), one `models/<address>` per Model Card made, `events/gNN` if the group has dated sentences, `findings/gNN` if it has result sentences, `link/gNN` |
| cards (offline) | group | digest | zt-check on the group's Cards has no hard fault, or its faults are registered for the book cycle | `cards-book` |
| cards-book | book | first digest unit | the card cycle report shows its faults fixed or refused with reasons | `organize` |
| models | Model Card | digest | apply finished; no part says `待写`; `validate` faults 0 or listed as tolerated | `explain/<address>` for a formal model with Derivation parts; `organize` |
| events, findings | group | digest | the JSON is applied, or the group has none | `organize` |
| link | group | digest | `linked-gNN.txt` exists; every owned Card has a kinded Link or a recorded reason | `link-check` |
| link-check | book | first link unit | report faults 0, or each left fault named | `organize` |
| organize | book | link-check, models, events, findings | structure Card, Topic bodies, notes cycle done | `hubs/<tree>` for each Topic tree touched; `figures`, `bridge` |
| hubs | Topic tree | organize | hub check has 0 faults for the tree | `audit` |
| figures, bridge, audit | book | organize, hubs | their reports exist | none |

`prepare` registers all 171 digest units at the start. From then on, every unit of the run descends from them, so the ledger knows the full target of the run before any agent starts.

### L5. How the workflow uses the ledger

Every stage becomes the same loop in the workflow script:

1. A manager agent takes units with `ledger next` and works them. Its commands (`group-start`, DeepSeek, `group-finish`; or `model apply`; and so on) record the result.
2. When the managers end, one short agent runs `ledger gate -Layer <l>`.
3. If the gate reports units in `retry` or `ready` (lost agents, transient faults), the script starts managers for them once more. At most `maxAttempts` rounds; each round has only the open units.
4. If the gate reports only `done`, `failed`, and `blocked` units, the stage ends. It ends as `complete` when all are `done`, else as `partial`. A `partial` stage never stops the run: the next layer works on what is done. The `failed` units go to the rerun list.

Rules for agents that come with it:

- A manager never stops its other units for a fault of one unit (I12). It records the fault and goes on.
- An agent never decides a retry. The ledger and the script decide (I17).
- A layer starts its units as soon as they are `ready`, not when the whole previous layer ends, where the layer allows it (Part 3, W5).

### L6. Idempotent writes

Every write that a unit makes carries a key: `<unit>#<step>#<n>`. The zt write (Part 2, A4) skips a write whose key it has already applied. So a retry, a rerun, or a lost answer never writes a paragraph twice (I10). Writes that append a Links paragraph also skip a paragraph that the body already has, so old writes without a key are safe too.

### L7. The rerun: mode `resume`

`mode: "resume"` with `out` only. It reads the ledger, puts every `failed` and `blocked` unit back to `ready` with `attempts` 0, and runs the normal stages on them, in layer order. It never touches a `done` unit, so a finished model stays as built. A unit that fails again in the rerun is reported with its fault for manual work.

The rule "no model re-runs without asking" still holds: the run asks before `resume`, and `resume` re-runs only units that are not done.

### L8. Why one run, or one rerun, covers everything

1. The target is complete from the start: `prepare` registers every digest unit, and each later unit is registered by the unit that makes its input.
2. No unit can disappear: a unit leaves `ready`/`claimed` only to `done` (checked in the slip-box), `failed`, or `blocked`. A lost agent or a killed command lets the lease end, and the unit comes back.
3. Transient faults are fixed inside the run: up to 3 attempts per unit, with backoff. In this run, the 7 lost groups, the 23 card blocks, and most of the 66 model failures (lock errors, and the models that waited on a failed model) were transient.
4. Content faults are named: the rerun list says which unit failed and why. One rerun works exactly that list.
5. With the speed-ups of Part 2, the transient faults themselves become rare, so most runs need no rerun.

### L9. Reports from the ledger

- The run's report is `ledger status`, written to `<out>\run-report.md`: per layer, done / failed / blocked, stage wall times, agents and tokens per stage, and the exact `resume` command when the rerun list is not empty (I23, I25).
- Placeholder parts are counted from the slip-box when a model unit is checked (I23).
- DeepSeek cost: the sum of the usage in each DeepSeek run's own result, recorded per unit. Never the difference of two ledger totals (I22).

## Part 2. Faster tools (G3)

The order is by effect. A1–A4 are in zt; A5–A7 in zt-digest.

### A1. Keep the Snapshot in the MCP server (M1)

The `zt mcp` server keeps its Snapshot between calls and reloads it only when SQLite's `data_version` changes, the same way the GUI does (`src/gui.rs:285`). A run of reads with no write between them then costs one load, not one per call.

Expected: a read between writes goes from about 240 ms to a few milliseconds. `snapshot-topics` and a card audit read mostly between writes, so they get most of this.

### A2. Bulk reads (M2)

New read tools, so one call replaces hundreds:

- `get_cards` with a list of addresses: the `get_card` answers in one reply.
- `get_tree` with `bodies: true`: every Card of the tree with its title and body.
- `export_titles` with a Workspace filter: address, type, facet, and title of every Card, the content of a topic snapshot, in one call.

Then `snapshot-topics` is 1 call, the card audit about 3 calls, and `prepare-link` about 2 calls.

### A3. Change only the reverse links that change (M3)

`regenerate_reverse_links_tx` rewrites all 10,891 Cards on every write. Replace it, for the single-Card writers (`insert_card`, `update_card_text`, the metadata writers), with an update of only the Cards whose reverse sections change: the targets in the Card's old and new Links, and the Card itself. The full regeneration stays for moves, renames, deletes, and conversions, which change many addresses.

Expected: a write goes from at least 0.45 s and 10.7 MB to milliseconds and a few kilobytes, and it holds the lock only that long. With A3, the edit lock stops being a bottleneck (I8).

### A4. A batch write tool (M3, I8, I10)

`apply_batch`: a list of creates and edits, applied in one transaction under one lock, with one reverse-link update at the end. Each item carries its idempotency key (L6). `model apply`, `card apply`, `hub apply`, the event and finding apply, and the DeepSeek link writes use it.

Expected: a model apply with hundreds of writes takes one lock for well under a second, in place of hundreds of locks over 3 minutes.

### A5. A robust edit lock (I8, I9)

Even with A3 and A4, the lock needs these fixes:

1. Treat a sharing error on the lock file (os error 32) as busy, and retry it like `edit in progress`.
2. A longer patience for batch clients: `ZT_EDIT_PATIENCE` (default 20 s; zt-digest sets 180 s).
3. Log a wait longer than 5 s, with the holder's pid, in `zt.log`.

Later, one writer: the zt service applies all writes from a queue, and the lock file goes away.

### A6. Find why the daemon died (M4)

Add the exit cause to `zt.log` (panic message, or the signal), and a heartbeat line from the daemon every minute during a run. A digest run checks `zt status` before each stage and waits for a restart in place of failing its calls.

### A7. zt-digest reads less and once

1. **One snapshot per run, updated by the ledger.** `prepare` writes the topic snapshot once (A2 `export_titles`). When a digest unit is done, `ledger done` appends its new Cards to the snapshot. A group reads a snapshot that is current to the last finished group, with no zt call at all.
2. **Scoped audits.** The card steps audit only their unit's Cards and the Cards they link to, not the whole book and not every tree.
3. **One read per step chain.** The card plan, check, and report of one unit share one read when `data_version` did not change.
4. **Fewer processes.** `read_many` keeps its 6 workers only when the reads are many; with A2 bulk reads, one process is enough.

### Expected command times

Estimates from M1–M3, for a slip-box of this size, to be measured after each change:

| Command | Now | After A1–A4, A7 |
| --- | --- | --- |
| `snapshot-topics` | 61 s (per group) | 0 s (the ledger keeps it); 1–2 s at prepare |
| `card plan` / `check` / `report` | 128 s to over 600 s | under 10 s |
| `prepare-link` | 576 s | under 20 s |
| `model apply` | 188–363 s, and fails under load | under 30 s, no lock failures |
| one zt write | at least 0.45 s, under a shared lock | a few ms |
| one zt read | 0.24 s | a few ms |

The other slow steps of the run (`skeleton plan` 213 s, `skeleton apply` 226 s, `finding plan` 210 s, `hub apply` 217–327 s, `audit` 152 s, `organize-survey` 144 s, `list-models` 85 s) read and write through the same calls, so A1–A4 speed them up too.

## Part 3. Workflow changes

**W1. One whole-book card cycle in place of 35 block cycles** (I3, I4, I7). In mode `full`, digest units run only the offline zt-check (unit `cards/gNN`), and one card cycle runs on the whole book after the digest (unit `cards-book`). The link check stays. About 200 fewer agents and about 13 M fewer tokens.

**W2. One agent per step chain** (I7, I13, I20). One agent runs plan → DeepSeek jobs → apply → check → report for its units, in place of one agent per step. Every zt-digest call gets the 600 000 ms timeout; Part 2 makes the calls short enough to fit it.

**W3. A narrow agent type** (I19, I21). `zt-digest-runner`, with only PowerShell, Read, Write, and the deepseek tools: no terminal tabs, no Monitor, no session tools. A digest group is two commands around the DeepSeek run: `zt-digest group-start -Group N` (claims the unit, writes the group prompt; the snapshot is already current, A7.1) and `zt-digest group-finish -Group N` (checks the outputs, calls `ledger done`). The order then lives in the tool, not in the prompt.

**W4. A time cap on one DeepSeek run** (I5). After `dshRunCap` (default 20 minutes), the manager cancels the run, keeps what it wrote, and records a transient fault; the ledger retries the unit. The zt helper (`skills/zt/scripts/zt-mcp.ps1`) fails a call fast when the lock stays busy, so DeepSeek sees an error and not a hang.

**W5. Start a layer when its units are ready** (I4, I6). Events and Findings start per group when the group's digest unit is done. Models start when the digest layer's managers end, without waiting for card checks. Figures and Bridge run beside Hubs. Audit runs last.

**W6. Model compiler repairs** (I11, I15, I16). With the ledger, a model is done only when its unit is `done` (L4), so placeholder models are never skipped. A model whose reference target has no part yet waits on that model's unit (a `deps` row) in place of failing. `apply` resumes from its first unwritten part.

**W7. A repeat check** (I10). A new zt-check rule: a body has the same paragraph twice. The audit counts it. Clean up the 15 Cards of I10 with the card fix apply.

**W8. A book argument** (I27). `args.book` takes a title or a Scholar Archive work id. Prepare finds the bilingual Markdown, the file id, and the BibTeX entry, lists the top headings, and suggests `startHeading`, `endHeading`, and `skipHeadings`.

**W9. One orphan definition** (I26) for the audit and the link check.

**W10. Quality work after a run** (section F). The `explain` layer for formal models (135 Derivation parts without 思路 and 解读), and a final fix cycle on the audit's faults.

## Coverage: every issue and its fix

| Issue | Fixed by |
| --- | --- |
| I1 snapshot per group | A1, A2, A7.1 |
| I2 whole-book audits | A1, A2, A7.2–3 |
| I3 block card cycles | W1 |
| I4 card backlog delays models | W1, W5 |
| I5 slow DeepSeek run | W4 |
| I6 serial tail | W5 |
| I7 agent count | W1, W2 |
| I8 lock failures | A3, A4, A5, L2 retries |
| I9 os error 32 | A5.1 |
| I10 repeated paragraphs | L6, A4, W7 |
| I11 placeholder parts | A4, W6 |
| I12 lost groups | L2, L5, L8 |
| I13 background commands | L2 leases, W2 |
| I14 validator | done (`bb7e905`) |
| I15 rerun skips placeholders | L4, W6, L7 |
| I16 cascade in a batch | W6 (`deps`) |
| I17 no retries | L2, L5 |
| I18 agent text as truth | L1–L4 |
| I19 wrong tools | W3 |
| I20 timeouts | W2, Part 2 |
| I21 step order | W3 |
| I22 cost | L9 |
| I23 undercount | L9 |
| I24 no status tool | L3 `ledger status` |
| I25 `done` with failures | L5 (`partial`), L9 |
| I26 orphan definitions | W9 |
| I27 book title | W8 |
| daemon died (M4) | A6 |

## Expected effect

Estimates, not measurements:

- **Lost work:** none silently. Transient faults are retried inside the run; content faults are on the rerun list. One `resume` works that list.
- **Run time:** the digest drops the 171 snapshots (about 3 h of command time); the card cycle and model writes stop failing and waiting. Target: one full run of a book this size in about 3.5–4 h, with no rerun in most cases.
- **Agents and tokens:** fewer than 300 agents and about half the Claude tokens (W1, W2).

## Order of work

1. **Clean up this book** (now). The 15 Cards with repeated paragraphs (W7), the 7 missing groups, the models still unbuilt, and the failed card blocks. The first session is still working on the 41 models.
2. **Fast zt** (A1, A3, A5.1–2). Three small changes in zt, each with a test and a timing before and after. A1 and A3 give most of G3.
3. **The ledger** (L1–L4, L6) in zt-digest, with tests on a small book; then A2, A4, A7.
4. **The workflow** (L5, L7, W1–W6) as a new version of the digest base, through the pre-flight. Pilot it on a short paper, then on the 7 missing groups of this book as a `resume`.
5. **The rest** (A6, W7–W10, one writer in the service).

## Decisions for the user

| No. | Question | Proposed answer |
| --- | --- | --- |
| Q1 | Drop the per-block card cycle in mode `full`, and keep one whole-book cycle plus the link check? | Yes (W1). |
| Q2 | May a run retry a unit with a transient fault, up to 3 times, without asking? | Yes. It never touches a `done` unit, so "no model re-runs" still holds. |
| Q3 | Does a `partial` stage stop the run, or does the run go on with what is done? | Go on, and put the rest on the rerun list (L5). |
| Q4 | Where does the ledger live? | `<out>\ledger.sqlite3`, beside the staging files of the book. |
| Q5 | Keep the lock file, or move all writes into the service? | Lock file with A3–A5 now; one writer later. |
