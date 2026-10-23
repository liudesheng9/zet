# Digest Claude Token Use: Proposal

Status (2026-10-23, later): P1, P2, P4 (for the finish lines), P7 and P9 are built in zt-digest 0.12.0 and base 5.2.0 (econ 6.2.0, math 5.2.0); P3 is dropped by its own rule; P5 waits for its measurement; P6 and P8 are not built. The section "Implementation status" at the end says what each one became and which gates it has passed. The deterministic cleanup run (`cleanup-run` with the bridge tool `deepseek_settle`, zt-digest 0.11.7, commit 5ace03b) was built in another session while this analysis ran. It is shown below as done, with its measured effect.

This proposal looks at the Claude Code tokens (not the DeepSeek tokens) that one economics digest spent: *The Modern VAT* (`ebrill2001modern`, zt-econ-digest 6.1.1, 2026-10-23). It covers two parts:

- the workflow's own agents: trial 1 (`wf_c39c27e9-d1c`, 876 agents), its resume (`wf_a36a082e-c62`, 129 agents), and the later single-stage repair runs (modes `figures`, `bridge` and `audit`);
- the main session that started, watched and repaired the run.

It names the work an agent did that a script can do, and proposes a non-agent method for each part. No proposal may change the digest's quality or robustness: the section "Quality and robustness" states what may not change, the risk of each proposal, and the gates each must pass.

## Method

Every workflow agent has a transcript (`subagents/workflows/<run>/agent-<id>.jsonl`) and a meta file with its label, model and phase. Every API request in a transcript records its usage. The analysis sums the usage of each agent once per request id. It then puts each agent into one of four kinds:

| Kind | How it is recognized | What it does |
|---|---|---|
| relay | its task text starts with `Run this PowerShell command once` (`zdMake().runScript`) | runs one script command and returns its last output line |
| manager | every other Haiku agent | runs a compiler step and its DeepSeek jobs, or a block of units |
| architect / organizer | Sonnet / Opus | plans the models / makes the Organize decisions |
| cleanup agent | `cleanup chunk`, `cleanup workspaces`, `cleanup: settle the run` | deletes DeepSeek conversations and records the facts |

Raw token counts mix four prices. Cache reads are cheap and cache writes and output are expensive, so this document compares **weighted tokens**: input + 1.25 × cache write + 0.1 × cache read + 5 × output. A weighted token is in units of that model's own input price. Across models, Sonnet costs about 3× and Opus about 5× Haiku per token (the 4.5-generation list-price ratio; the exact current prices are not part of this analysis).

The main session's usage comes from its own transcript (`598e46fa-….jsonl`), split by time window: before the run, while trial 1 ran, during the resume and hand cleanup, and after.

The two analysis scripts sit in the session scratchpad (`usage.js`, `main.js`). They read only the transcripts and change nothing.

## Findings

### 1. The workflow agents

| Run | Agents | Weighted (M) | Relay | Haiku manager | Sonnet architect | Cleanup agents | Opus |
|---|---:|---:|---:|---:|---:|---:|---:|
| trial 1 | 876 | 68.7 | 613 agents, 32.4 (47%) | 226, 28.8 (42%) | 12, 4.8 (7%) | 24, 2.5 (4%) | 1, 0.2 |
| resume | 129 | 8.1 | 94, 5.1 (64%) | 30, 2.4 (29%) | — | 5, 0.6 (7%) | — |
| mode audit (6.1.2) | 7 | ≈0.6 | 4 | — | — | 3 | — |
| mode audit (6.1.3, `cleanup-run`) | 3 | ≈0.2 | 2 | — | — | 1 | — |

**A relay is almost all fixed cost.** A relay runs one command in about two requests. Its visible task is about 2,400 characters, but its first request carries about 61k tokens of context: about 40k of a prefix shared with other agents (read from cache) and about 20k written new for this agent alone. That context is Claude Code's subagent system prompt, the tool definitions and deferred-tool list of every connected MCP server (Slack, Chrome, computer use, Scholar Archive, zt, DeepSeek), the server instructions, and the project instructions. None of it is needed to run one command. The weighted cost is about 53k per relay, whatever the command does. Returning the command's line costs about 1k output tokens more (×5 weight).

**Which commands the relays ran** (weighted M, all runs together):

| Command | Relays | Weighted (M) | Why it is an agent today |
|---|---:|---:|---|
| `cleanup-finish` | 158 | 8.3 | one checked finish per cleanup unit — **replaced** by `cleanup-run` (0.11.7) |
| `ledger next` (claims) | 103 | 5.3 | each layer round claims through a relay before its managers start |
| `findings-finish` | 77 | 4.3 | one checked finish per unit, after the block's managers ended |
| `events-finish` | 73 | 4.0 | the same |
| `cards-finish` | 73 | 3.7 | the same |
| `ledger status` | 57 | 2.9 | a second read of a unit's state when a relay may have cut the finish line |
| `model-finish` | 44 | 2.5 | one per model |
| `model-deps` | 44 | 2.1 | one per planned model, before its fill |
| `ledger hold` | 24 | 1.6 | one per nine minutes per held block, to renew its leases |
| `ledger fail`, `gate`, `report`, `finalize`, `resource`, others | 60 | 3.2 | single facts |

The per-unit finishes and `model-deps` (311 relays, 16.5M weighted) are the largest group still in the workflow. Each runs a deterministic, idempotent command whose answer the ledger already holds.

**Managers.** The 226 Haiku managers cost 28.8M weighted (42%). They drive DeepSeek through the MCP bridge: start jobs, poll `deepseek_wait`, record ids and answers, delete conversations. Their context grows with every poll result, and their cost scales with the number of requests (1,203 requests in trial 1). The labels with the largest totals are the digest and Link lane blocks (38 agents, 35M raw) and the model chain's steps (`brief`, `fill`, `derive`, `review`, `split`, `close`, `report`; about 120 agents). About a dozen of the model-chain step agents (among them `close`, `report`, `review` and `fill` steps) started no DeepSeek run at all: they ran a script step only. The Sonnet planners start none by design.

**The Sonnet architect** (12 agents, 4.8M weighted, about 400k each) reads its context files with 16 requests per agent on average. This is model work, but the file reading is not.

**Cleanup** cost 158 relays plus 29 agents in trial 1 and its resume (about 11M weighted). With `cleanup-run` (0.11.7) a cleanup stage is one agent around one bridge call. The measured mode-audit run went from 7 agents to 3.

### 2. The main session (monitoring and repair)

| Window | Requests | Weighted Opus tokens (M) | Largest context |
|---|---:|---:|---:|
| before trial 1 | 15 | 0.25 | 113k |
| trial 1 running (4.5 h) | 177 | 3.96 | 279k |
| resume and hand cleanup | 66 | 2.39 | 370k |
| after (fixes, repair runs, this analysis) | 373 | 18.2 | 691k |

While trial 1 ran:

- 13 `Monitor` watches were started. Each watch expired after 30 minutes and had to be armed again, and each new watch replayed the journal from its start, so it reported the earlier phases again.
- The watches woke the session 82 times. **55 wake-ups (67%) needed no action.** Each wake-up re-reads the whole session context (about 200k tokens at that time) at Opus prices.
- The session also polled the ledger by hand with 75 Bash calls.

After the run, the main session did the repair and development work in the same conversation. Its context reached 691k tokens, and every request re-read most of it: 162M cache-read tokens, 16M of the 18M weighted.

At the 5:1 Opus-to-Haiku ratio, the main session's monitoring and repair (about 24M weighted Opus) cost more than all of trial 1's relays.

## Proposals

Each proposal names the work, the non-agent method, the evidence, and an estimate of the weighted tokens it saves on a book of this size (73 groups, 44 models). The estimates use the measured cost per agent and assume the work itself stays the same.

### P1. Finish a block's units with one command (largest relay group)

**Work:** `cards-finish`, `events-finish`, `findings-finish`, `model-finish`, `model-deps`: one relay per unit.

**Method:** a batch form of each finish, as `cleanup-finish -Ready` and `cleanup-run` are for cleanup: `zt-digest <layer>-finish -Out <out> -Units <id:token,…>` (or `-Ready -Owner <label>`). It runs each checked verifier in one process, records a fault with the claim's own hash for a unit that cannot finish, and prints one short line of counts and the open units. The layer driver calls it once per block. `model-deps` takes the batch's whole plan in one call.

**Evidence:** 311 relays, 16.5M weighted. Every finish is already idempotent and fenced by its claim token, so running several in one process changes no rule.

**Estimate:** 311 relays become about 25 (one per block), saving about 15M weighted (about 22% of trial 1).

### P2. Let the agent that already holds the claim run the commands

**Work:** `ledger next` relays before managers start (103), the defensive `ledger status` reads (57), `ledger fail` (17).

**Method:** for the event, finding, cards, model and explain layers, give the claim and the finish to the block manager's prompt, as the digest and Link block managers already do (`block`, `blockFinish`): the manager claims, works and finishes, and the driver reads one `ledger status -Held` line afterwards. The defensive state read exists because a relay could cut a long finish line. P4 makes every line short, so it can go.

**Estimate:** about 150 relays fewer, about 8M weighted.

### P3. Renew leases without an agent

**Work:** `ledger hold` relays (24 in trial 1), one each nine minutes per held block, during long model builds.

**Method:** a detached lease keeper started once per block (`zt-digest ledger hold -Units … -Detach`, ended by the block's finish), or lease renewal inside the zt-digest commands the managers run while they wait. zt-digest already renews a claim in-process (`start_heartbeat`) while a command runs; the gap is only the time between commands.

**Estimate:** 24 relays, about 1.6M weighted.

### P4. Short output lines everywhere

**Work:** relays re-type long JSON lines (about 1k output tokens each, ×5 weight), and a summarized line makes the driver read the ledger again (P2) or lose the result (the 138 KB `ledger report` line, fixed in 0.11.6 with `report_line`).

**Method:** every command a relay reads prints one bounded line: counts, the first eight open items, and the path of the full file. The full result goes to a file in the staging folder, as `run-report.json` does.

**Estimate:** about 3M weighted in output, plus the follow-up reads it removes. It is also a reliability fix: on grossman2002integration trials 5, 10, 12 and 17 a relay cut or rewrote a long line.

### P5. A minimal agent type for relays

**Work:** every relay loads about 61k tokens of context it does not use.

**Method:** run relays with `agent(..., {agentType: 'zt-relay'})`, a custom subagent type (`.claude/agents/zt-relay.md`) whose only tool is PowerShell (or Bash) and whose system prompt is a few lines. The Workflow runtime supports `agentType`. Two points need a measurement before this is adopted: whether a custom type still loads the MCP tool listing and server instructions, and whether it still receives the project instructions and memory index. Measure the first request's cache write of one relay of each kind; adopt the type if the per-agent write falls well below the present 20k.

**Estimate:** if the context falls from about 61k to about 10k, a relay costs about 10k weighted instead of 53k: about 25M weighted on trial 1's 613 relays, or about 4M on the roughly 90 relays left after P1 to P3. Do P1 to P3 first; P5 then makes the rest cheap.

### P6. One bridge call per DeepSeek job set (managers)

**Work:** a Haiku manager polls DeepSeek (`deepseek_run`, `deepseek_wait`, record commands, `deepseek_delete`), and its context grows with each poll; 226 managers, 28.8M weighted.

**Method:** the same pattern as `deepseek_settle`. A bridge tool `deepseek_run_jobs` takes a job file a script wrote (the jobs with their prompt files, model, effort, cwd and the record rules), starts and waits for every job itself, writes each answer and its usage to files, and returns counts. A script step then records ids, evidence and usage in the ledger from those files (`ledger resource -SweepEvidence` already does part of this). The manager becomes one agent with about three calls: run the step command, call the bridge tool, run the record command. Approval questions a job raises (`needs_response`) keep a manager path: the tool returns them and the manager answers only those.

**Evidence:** cleanup went from 158 relays and 29 agents to one agent with this pattern (0.11.7). The block managers' answers are already files (`<out>\answers\<session>.txt`), so no model judgment is lost: a manager only relays.

**Estimate:** about half the manager cost, 12–15M weighted. This needs a bridge release (dsh-mcp-bridge) and the largest change of all the proposals; do it after P1 to P5.

### P7. Merge the script-only steps of the model chain

**Work:** model-chain steps that start no DeepSeek run (`close` and `report` often, the skeleton step always) each take a step agent.

**Method:** one step command runs the consecutive script-only steps (for example `model -Step close` and `-Step report` together), and the chain asks for an agent only for steps that print jobs.

**Estimate:** about 20–30 agents per book, about 2–3M weighted.

### P8. Give the architect one prepared context file

**Work:** the Sonnet planner reads its context with about 16 requests per batch.

**Method:** the `model -Step brief` output already gathers the context; write it as one file of known size that the planner reads once, and ask for the plan in one structured answer.

**Estimate:** about half of the architect's 4.8M weighted (Sonnet prices, about 3× Haiku).

### P9. Monitor with a filter, not with the main session

**Work:** 82 wake-ups of a 200k-token Opus session, 55 of them with no action; 75 hand polls.

**Method:**

1. A watch script that remembers what it reported (a state file next to the journal), so a new watch never replays old phases, and that emits only actionable events: a stage failure, a refused finish or claim, a unit in `failed`, a stall with no journal event for N minutes, the run's end. Progress lines go to a file the user can read.
2. One watch per run that does not expire every 30 minutes (re-armed by the script itself, or a longer timeout).
3. One status command for hand checks (`zt-digest ledger status -Summary`, plus the watch's own progress file) instead of ad-hoc Bash and node reads.

**Estimate:** about two thirds of the 4M weighted Opus during the run, about 2.7M Opus (about 13M Haiku-equivalent).

### P10. Keep the watching session small

**Work:** the repair work after the run made the main session's context grow to 691k tokens; most of the 18M weighted Opus was re-reading that context.

**Method:** the main session only starts and watches the run, and every repair goes to its own session (a fork or a fresh session with a short brief), which ends with a commit. The monitoring session's context then stays near its starting size. Long investigations (transcript reads, test logs) go to a subagent that returns its conclusion, not its file dumps.

**Estimate:** not a fixed number; on this run the repair phase's context re-reads were about 16M weighted Opus.

## Summary of the estimates

On a book of this size (trial 1: 68.7M weighted, 876 agents):

| Proposal | Agents removed | Weighted tokens saved (M) | Change |
|---|---:|---:|---|
| done: `cleanup-run` (0.11.7) | about 180 | about 11 | zt-digest + bridge |
| P1 batch finishes | about 285 | about 15 | zt-digest + layer driver |
| P2 claim and finish in the manager | about 150 | about 8 | layer driver |
| P3 lease keeper | 24 | about 1.6 | zt-digest |
| P4 short lines | — | about 3 | zt-digest |
| P5 minimal relay type | — | about 4 after P1–P3 | agent definition |
| P6 bridge job runner | — | about 12–15 | bridge + driver |
| P7 merged script steps | about 25 | about 2–3 | model compiler |
| P8 one architect context file | — | about 2.4 (Sonnet) | model compiler |
| P9 filtered monitoring | — | about 2.7 Opus | watch script |
| P10 small watching session | — | up to about 16 Opus | practice |

P1 to P4 need only zt-digest and workflow module changes of the kind already made for cleanup, and together they remove about 460 of trial 1's 613 relays. P6 is the largest saving among the managers but needs a bridge release. The estimates overlap where proposals touch the same agents (P2 and P5, P1 and P5), so the totals are not additive.

## Quality and robustness

A token saving is accepted only when it leaves the digest's output and its fault handling exactly as they are. This section states what may not change, what each proposal could put at risk, and the gate it must pass before it ships. A proposal that fails its gate is dropped, not weakened.

### What may not change

1. **What DeepSeek is asked and how.** The same prompt files, model (DeepSeek V4.1 Flash, official source), reasoning effort, skills, working folder and wait limits per job. No proposal edits a prompt or a job list.
2. **Who decides a unit's outcome.** The ledger's checked verifiers (`group-finish`, `cards-finish`, `events-finish`, `findings-finish`, `link-finish`, `model-finish`, `explain-finish`, `cleanup-finish`, the publications) stay the only judges. No agent and no new tool may mark a unit done, and every check they run today still runs.
3. **The rules a manager follows.** Approval is an allow-list (the zt helper, the zt-check self-check, read-only reads under the run's folders, JSON writes under the job's folder; reject any other write). A question gets the fixed answer 「按任务说明自行决定，不需要询问。」. A failed job is continued once in the same conversation with the fixed resume prompt, then marked failed. These are fixed rules today; a tool that takes over must apply the same rules, word for word, and nothing else.
4. **Fault handling per unit.** A fault stays the fault of its own unit, with its class (retryable, content, operator) and the claim's hash. One unit's fault never stops the others in a batch, never stops a layer, and a retryable fault is claimed again within the frozen attempt budget.
5. **Fences and resume.** Every write keeps its claim token fence. A resume and every single-stage mode work exactly as before: each step stays idempotent, and an interrupted batch leaves each unit either finished or still claimed and retryable, never half-recorded.
6. **The owner's Lock and the Literature scope.** A `card_locked` refusal still stops the stage with its typed refusal; the Literature write scope still refuses Topic-tree and Hub writes.
7. **What a watcher can see.** Monitoring may report less often, but it may never miss a failure, a refusal, a failed unit or a stall.

### Risk per proposal

| Proposal | Quality risk | Robustness risk | Safeguard |
|---|---|---|---|
| P1 batch finishes | none: the same verifiers run on the same drafts | a long batch could pass the 10-minute command limit; one unit's error could stop the rest | per-unit transaction and fault, as `cleanup_ready` does; a block is at most 8 units; measure the slowest finish and bound the batch so it ends well inside the limit |
| P2 claim and finish in the manager | none | a manager may cut or rewrite a claim line (grossman2002integration trials 3, 4, 17) | the driver still decides from the ledger (`ledger status -Held`), as the digest and Link blocks do now; units a dead manager held are settled from the ledger |
| P3 lease keeper | none | a keeper that outlives its owner would keep a dead claim alive, and the unit would never be claimed again | **no detached keeper**: renewal only inside commands the manager runs, or the manager runs `ledger hold` itself; a keeper process, if ever used, needs a hard lifetime and ends with its owner. Low saving (1.6M); drop it if the safeguard is not simple |
| P4 short lines | none | a caller that reads a field the short line dropped | every field a module reads stays in the line; the full result is a file; harness checks per command |
| P5 minimal relay type | none | a relay without a tool it needs; a relay without the run-step guard the prompt gives | the relay prompt is unchanged; the type keeps PowerShell; one relay of each command kind runs in the harness and in a pilot before adoption |
| P6 bridge job runner | the jobs must run exactly as now (rule 1, rule 3) | approvals, questions, the one resume retry, a provider error, a timeout | the tool implements rule 3 from one written policy; every `needs_response` it cannot decide by that policy goes back to a manager; failure-injection tests for each case (error, no JSON answer, question, a write outside the allow-list, timeout) |
| P7 merged script steps | none | a merged step must stay resumable at each former step | each merged step keeps its own idempotent output file, so a resume restarts at the first unfinished one |
| P8 one architect context file | **possible**: a planner that reads one prepared file may miss context it now reads on demand | none | adopt only if an A/B run on the same model batch gives plans whose review finds no more faults and whose models are as complete (parts, links, self-sufficiency); otherwise drop it |
| P9 filtered monitoring | none | a filter can hide an abnormal behaviour nobody anticipated | report every failure, refusal, failed unit and stall; a short progress summary at a fixed interval (for example 30 minutes) so an unknown problem still shows |
| P10 small watching session | none | none for the workflow | repairs still follow the fix-test-reinstall rule in their own session |

P1, P2, P4, P5, P7 and P10 change how a command is reached, not what it does. P3, P6, P8 and P9 carry the real risk and have the strictest gates; P3 and P8 are dropped if their safeguards cannot be met.

### Acceptance gates

Every proposal passes all of these before its blueprint version is installed:

1. **Tests.** All `scripts/book-digest/tests/*.tests.ps1` files, both harnesses and the zt-digest unit tests pass. Each proposal adds failure-injection cases for the risks in its row above (the framework harness already injects garbled relay lines, failed claims, failing cleanup units and lock refusals).
2. **Same outcome on the same input.** A short source digested by the current and the proposed version, from fresh staging folders, must give: every unit in the same final state; the same number of Cards per type and per group (the DeepSeek text may differ run to run, as it does today); an audit with no more faults by rule; every model self-sufficient; the same refusals for injected faults; a run report that is complete in both.
3. **Fault behaviour.** On that pilot, inject one provider error, one content fault and one stale claim into a batch; each must be recorded against its own unit and the run must finish as it does today.
4. **Resume.** Stop the proposed version mid-stage, resume it, and require the same final state as an uninterrupted run.
5. **Measured saving.** Run the usage analysis on the pilot. A proposal that does not save what it claims is not adopted, whatever its other results.

## Order and checks

1. P4 (short lines), then P1 and P2, in one blueprint version: they change how the layer drivers call commands.
2. P3 with P1, because the batched finish ends the leases it keeps.
3. Measure P5 on one relay of each kind; adopt it if the context falls.
4. P9 and P10 for the next monitored run; they need no workflow change.
5. P6 and P7, then P8, each with its own bridge or compiler release.

Every step passes the acceptance gates of the section above before it ships, and the token check is the same one this document used: run the usage script on the pilot's transcripts and compare agents and weighted tokens per kind with the table above. A change that saves tokens but fails a quality or robustness gate is not adopted.

## Implementation status

Built on 2026-10-23 in zt-digest 0.12.0 with base 5.2.0, econ 6.2.0 and math 5.2.0 (`zt-digest-lib` v35, `zt-cards-layer` v2, `zt-event-layer` v4, `zt-finding-layer` v4, `zt-explain` v6, `zt-model-lib` v9, `zt-model-layer` v6). The developer wiki has the details: [zt-digest](wiki/zt-digest.md) ("The batch finish") and [Digest workflows](wiki/digest-workflows.md) ("Checked finishes in one command", "Watch a run").

| Proposal | State | What was built | Relays removed per block (estimate on trial 1) |
|---|---|---|---|
| P1 batch finishes | built | `zt-digest finish-batch -Units "<unit>:<token>,…"`: each unit's own verifier in turn in one process; a unit that cannot finish is decided by the ledger first, then recorded against that unit alone with the class `finishOrFault` gave it; at most 8 units; no unit starts after 420 s; full results in `<out>/finish/`. `model-deps -Units` does the same for a model batch. | one per unit finish and per model-deps (about 300 of 311) |
| P2 claim and finish in the manager | built, two forms | Cards: one `finish-batch -Layer cards -Take -Owner` command claims and finishes (no manager needed). Events, findings, explain: the block's chain ends with the batch finish as a step its own manager runs (`always`). The claim relay of these chain layers stays: the driver needs the units before it builds the chain, and a claim inside the manager would put the units list back into a relayed line. | about 19 Cards claims and one finish relay per chain block |
| P3 lease keeper | dropped | Its rule: no detached keeper, and drop it if the safe form is not simple. Renewal inside the commands alone cannot cover the gaps between them (a DeepSeek job may run its 1200 s cap twice with the one resume, and the architect's plan has no command), so `ledger hold` stays. | 0 |
| P4 short lines | built for the finish lines | The batch line is about 300 characters per unit (the single finishes were 2–18 KB, all relayed whole). The other long lines (`list-models`, `prepare-link`, `ledger publish`) are relayed once per run and were left as they are. | output tokens of every finish |
| P5 minimal relay type | waiting for its measurement | `.claude/agents/zt-relay.md` (PowerShell only, a short prompt) is written. A session loads agent types when it starts, so this session could not run it, and the CLI is not logged in. Measure in a fresh session (one relay of each kind, the first request's cache write against the present ~58k), then give the blueprint's relay calls their own role with `agentType: "zt-relay"` only if the context falls well below 20k. | — |
| P6 bridge job runner | not built | Needs a dsh-mcp-bridge release and the failure-injection tests its row names. | — |
| P7 merged script steps | built | `model -Step close -Report`: a close round that sends nothing back writes the batch report in the same process, so the report step runs only after a close round that still sent follow-ups. A compiler step's command agent no longer loads the DeepSeek tools. | one report agent per model batch, one tool-load request per compiler step |
| P8 one architect context file | not built | Its gate is an A/B run on the same model batch; without that run it is not adopted. | — |
| P9 filtered monitoring | built | `scripts/book-digest/watch-run.mjs`: only ABNORMAL, FAILED, EXPIRED, STALL, a SUMMARY every 30 minutes and END reach the session; progress goes to `<out>/watch/progress.log`; its place is kept in `<out>/watch/state.json`. Replayed over trial 1's journal (876 agents) it prints 19 ABNORMAL lines and END, where the watching session woke 82 times. | — (main-session tokens) |
| P10 small watching session | practice | Written into the wiki's "Watch a run": repairs go to their own session. | — (main-session tokens) |

What did not change (rule 1 to 7 above): every prompt, job list, model, effort, skill, working folder and wait limit; every verifier and the checks it runs; the manager rules; the fault classes; the claim tokens, leases and attempt budget; the Lock refusal, which still stops the stage (a batch starts no unit after a `card_locked` one); and what a watcher reports (every failure, refusal, failed unit, expired claim and stall, plus a periodic summary).

Gates passed so far:

1. Tests: the framework harness (fault injection added: content and transient batch faults, a Lock inside a batch, a cut and a rewritten batch line, a cut claim-and-finish line, a garbled model-deps line), the Bridge and grow harnesses, the zt-digest unit tests (376), `finish-batch.tests.ps1` against a real scratch archive, and the whole `run-all.ps1` suite.
2. Gates 2 to 5 (the same outcome on a pilot book, fault behaviour on the pilot, resume, and the measured saving) need a digest run with DeepSeek and are not yet run. The pilot is a short book (grossman2002integration) digested by 6.2.0 from a fresh staging folder, compared with its last clean run, with `scripts/book-digest/token-usage.mjs` (the usage script of this analysis, now in the repository) run on its transcripts.
