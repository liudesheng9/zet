---
block: issue-dev-workflow
title: Issue development workflow
paths:
  - "scripts/issue-dev/**"
  - ".claude/workflows/zt-issue-dev.js"
  - ".claude/workflow-framework/zt-issue-dev/**"
  - "!.claude/workflow-framework/*/history/**"
synced: bfde6673e03d
updated: 2026-10-18
---

# Issue development workflow

## Purpose

`zt-issue-dev` is a Claude Code Workflow that develops GitHub issues of `liudesheng9/zet` with DeepSeek doing the work. It takes a list of issues and develops them one at a time. For each issue, a Haiku manager starts one DeepSeek Harness conversation: DeepSeek V4.1 Flash from the official source, effort `max` by default. The conversation starts with a **goal** that the manager sets through the deepseek bridge: the user's `/goal /tdd ...` text becomes the objective of a dsh goal. The agent then keeps working in goal rounds until it marks the goal complete or blocked. It has its own shell: it develops test-first, deploys, commits and pushes. When it needs something it cannot do, it asks with `ask_user_question`. The **main agent** (an agent on the session model) does what was asked, and the manager sends the answer back. The block exists so that this relay loop, its limits, and its roles are fixed and testable. The workflow is built and frozen by the Workflow Framework Protocol.

**Owns:** the blueprint and sim file, the `zt-issue-relay` module (the relay loop and the manager, main-agent, and delete prompts), the issue-list and repo-state scripts, the installed script and its registry, and the harness.

**Does not own:** the book-digest workflows and the framework assembler they share: see [Digest workflows](digest-workflows.md). Goals in DeepSeek Harness and the bridge tools (`deepseek_run` `goal`, `deepseek_goal`): the `dsh-mcp-bridge` repository (`E:\dsh-mcp-bridge`, README "Goals"). The code that the issues change: the other blocks of the [wiki](README.md).

## At a glance

| | |
|---|---|
| Entry points | `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: ["65-91"] } })`; `node scripts/issue-dev/tests/issue-relay.tests.mjs` |
| Key files | `scripts/issue-dev/workflow/zt-issue-dev.blueprint.json` — the blueprint: args, roles, consts (the first message, the goal objective, the relay rules), stages |
| | `scripts/issue-dev/workflow/modules/zt-issue-relay.js` — the `develop` stage: the per-issue relay loop (module v2) |
| | `scripts/issue-dev/workflow/zt-issue-dev.sim.json` — the dry run used by `install` |
| | `scripts/issue-dev/issue-list.mjs` — expands numbers and `a-b` ranges and reads each issue with `gh` |
| | `scripts/issue-dev/repo-state.mjs` — read-only state after an issue: clean, pushed, issue state, deployed zt version, service up |
| | `scripts/issue-dev/tests/issue-relay.tests.mjs` — the harness |
| | `.claude/workflows/zt-issue-dev.js` — the installed, read-only build; `.claude/workflow-framework/zt-issue-dev/` — its registry (blueprint, sim, lock, sources) |
| Depends on | the Workflow Framework (`~/.claude/skills/workflow-framework-protocol`), the DeepSeek Subagent Managed Protocol block, the `deepseek` MCP bridge **0.11 or later** (goals) with the DeepSeek Harness Desktop running, `gh`, `git`, `zt` |
| Used by | the user, who runs it from a Claude Code session |
| Tests | `scripts/issue-dev/tests/issue-relay.tests.mjs`; live goal path: `node E:/dsh-mcp-bridge/scripts/goal-check.mjs <empty scratch folder>` |

## Concepts

- `Goal` — dsh's long-running completion objective, one per conversation. The Desktop's goal-round driver queues a goal round each time the agent goes idle, until the model marks the goal complete (`update_goal`) or blocked, or the round cap (`max_goal_rounds`) is reached. The bridge reports the conversation as `running` (or `needs_response`) until then, and every report carries `goal: {phase, rounds, activation, blocked_reason}`.
- `Manager` — a Haiku agent (role `manager`) that relays one DeepSeek conversation: it starts it with the goal, waits, responds, resumes the goal, or deletes the conversation. It never answers or decides anything itself. Each issue has exactly one DeepSeek conversation. Each relay step is a new manager agent on the same `session_id`.
- `Main agent` — role `executor` (no model set, so the session model) that does what the DeepSeek agent asked and returns `{outcome done|refused|failed, result, answers, decision}` (`ZIR_EXEC`).
- `Ask` — a question (`ask_user_question`) or a tool approval: bridge status `needs_response`. Ending a round does not reach the main agent: the driver just starts the next round.
- `STATUS line` — the last `STATUS: DONE|NEEDS-HUMAN` line of an answer (`zirStatus`), used as the summary.
- `Outcome` — per issue: `done` (goal complete), `needs-human` (goal blocked), `max-rounds` (blocked `round-limit`), `max-asks`, `timeout`, `failed`.

## Architecture

```mermaid
flowchart TD
    P["prepare: script issue-list.mjs"] --> D["develop: zt-issue-relay"]
    D --> C["check: gate"]
    C --> R["report: value"]
    subgraph per issue, in order
      S["manager start: deepseek_run with prompt + goal, wait"] --> L{state}
      L -- running --> W["manager wait"] --> L
      L -- needs_response --> X1["main agent per pending item"] --> RS["manager respond"] --> L
      L -- "finished, goal stopped short / error" --> RE["manager resume: deepseek_goal resume (once)"] --> L
      L -- "goal complete / blocked / a limit" --> DEL["manager delete"] --> RSs["runner: repo-state.mjs"]
    end
```

The blueprint holds no code. `zt-issue-relay.js` builds the manager prompts (`zirManagerPrompt`), runs them through `dsmpSlot` (the script-wide DSMP pool), calls the main agent (`zirExecute`), and deletes each conversation (`zirDelete`). It runs the repo check through the framework's `script` module (`wfpScript`). After the last issue, `dsCleanupWorkspaces` removes the `cwd` workspace from the DeepSeek Desktop, but only when no conversations are left in it.

## Interfaces

### Workflow args

| Arg | Type | Default | Meaning |
|---|---|---|---|
| `issues` | array | required | issue numbers or `"a-b"` ranges, in order |
| `repo` | string | `liudesheng9/zet` | GitHub repository |
| `cwd` | string | `E:/zet` | the checkout the DeepSeek agent works in |
| `dsEffort` | `high` \| `max` | `max` | DeepSeek reasoning effort |
| `maxGoalRounds` | number 1–1000 | 60 | the goal round cap per issue (`max_goal_rounds`) |
| `maxAsks` | number 1–500 | 40 | main-agent asks per issue |
| `until` | `prepare` | — | stop after the issue list is resolved |

### Result

`{total, done, needsHuman, notDone: ["#n outcome"], repoNotOk: ["#n"], check, checkError, workspace, issues: [{number, title, stateBefore, outcome, summary, goalRounds, asks, session, deleted, lastAnswer, trail, repo}]}`. The report also has `deepseek`, the DSMP log: `deleted`, `notDeleted`, `workspaces`.

### Scripts

- `node scripts/issue-dev/issue-list.mjs --repo R --issues '<json array>'` prints `{ok, repo, count, issues, missing}`. It exits 1 when an issue is missing or the list is malformed. It exports `expandIssues`.
- `node scripts/issue-dev/repo-state.mjs --repo R --cwd DIR --issue N` prints `{ok, clean, branch, head, upstream, pushed, dirty, issue: {number, state}, zt: {source, installed, match, service}}`. It runs `git fetch` and changes nothing.

### The DeepSeek start

The manager calls `deepseek_run` with:
- `prompt`: `consts.taskPrompt` (the user's task text) followed by `consts.protocol` (the relay rules: how to ask, what needs the user, when to mark the goal complete or blocked, the STATUS lines).
- `goal`: `consts.goalObjective`, the user's text after `/goal` (`/tdd`, `issue #N`, the instructions), plus one line that points to the rules.
- `max_goal_rounds`, `model: deepseek-official/deepseek-flash`, `reasoning_effort` and `skills: ["tdd"]`.

`{N}`, `{REPO}` and `{CWD}` are replaced per issue.

## Behaviour

### One issue

1. `zirIssue` builds the start arguments, and the manager starts the conversation (`op: start`). The bridge sends the message, sets the goal once the agent is working on it, and keeps the turn running through the goal rounds. The manager calls `deepseek_wait` with 900 s waits, at most 12 per manager agent.
2. `running` starts another wait manager, at most `maxWaitCalls` (8) per issue. After that the outcome is `timeout`.
3. `needs_response`: the main agent is called once per pending item. Questions go back as `answers [{id, custom}]` and approvals as `decision`. Then a `respond` manager continues. After `maxAsks` asks the outcome is `max-asks`.
4. `finished` ends the issue by the goal's phase: `complete` gives `done`; `blocked` gives `needs-human`, or `max-rounds` for a `round-limit` block. The summary is the STATUS text, else the tail of the answer.
5. A goal that stopped short (paused, disarmed, none), or a failed turn, gets one `resume` manager (`deepseek_goal` `resume`). A second stop gives the outcome `failed`.
6. Then the conversation is deleted and never archived. If it is still running, it is cancelled first, which pauses its goal. The runner records `repo-state.mjs`.

### Rules

- Issues run strictly in order: they share one checkout, one service and one installed `zt`. This is why `develop` is one stage and not a kernel `foreach`, which runs items in parallel.
- A failed issue does not stop the run: the next issue starts. `develop` itself has no retries, so a retry cannot redo finished issues.
- The `check` gate requires every issue `done`, every repo state `ok`, and every conversation deleted. It only reports (`onFail: continue`).
- The main agent refuses work that needs the user: live digest runs or resumes on `D:\zt` (#90, #91), deletes of backups, Cards or data, force-pushes, and settings changes. It never runs a Workflow itself. The DeepSeek agent then reports `STATUS: NEEDS-HUMAN` and blocks its goal.

## Data and storage

None: the workflow keeps no files of its own. The registry `.claude/workflow-framework/zt-issue-dev/` (blueprint, sim, lock, sources, history) is written by the framework's `install`. The DeepSeek conversations, and their goals, are deleted after each issue.

## Configuration

These are fixed at install; a change is a new version:
- the DeepSeek model and skills (`consts.dsModel`, `consts.dsSkills`);
- the texts (`consts.taskPrompt`, `consts.protocol`, `consts.goalObjective`);
- the roles;
- the module defaults (`ZIR_DEFAULTS`): `waitSeconds` 900, `waitsPerManager` 12, `maxWaitCalls` 8.

The kernel passes only the params a blueprint gives, so the module merges its defaults itself.

## Usage

```js
// look at the resolved list first
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: ["65-91"], until: "prepare" } })
// pilot: one closed issue (reverify only)
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: [65] } })
// the full list
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: ["65-91"] } })
```

Before a run, the session's `deepseek` MCP server must be bridge 0.11 or later, with 16 tools including `deepseek_goal`. An older server process silently drops the `goal` argument. After the bridge is rebuilt, reconnect the server (`/mcp`), and restart the Desktop so it loads the plugin.

To change the workflow: edit the blueprint or the module, raise `version`, then run

```bash
node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs install scripts/issue-dev/workflow/zt-issue-dev.blueprint.json --sim scripts/issue-dev/workflow/zt-issue-dev.sim.json --scope project --project E:/zet
node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs verify zt-issue-dev --scope project --project E:/zet
```

## Testing

`node scripts/issue-dev/tests/issue-relay.tests.mjs` builds the blueprint and runs it in the framework's stand-in runtime, one scenario per path (16 tests):
- `expandIssues`;
- the start arguments: `goal` objective, `max_goal_rounds`, model, effort, `tdd`, no placeholders left;
- the manager prompt passing `goal` and loading `deepseek_goal`;
- `dsEffort` and `maxGoalRounds` passed through;
- a question in a goal round, then goal complete;
- complete without a STATUS line;
- blocked → `needs-human`, followed by the next issue, in order;
- `round-limit` → `max-rounds`;
- wait managers and `timeout`;
- one resume, then failed;
- `maxAsks`;
- an approval relayed as a decision;
- a repo state that is not ok;
- a missing issue halting at `prepare`;
- `until: prepare`;
- the removed `maxRounds` arg refused.

The goal path itself is checked live in the bridge repository with `scripts/goal-check.mjs`; it passed against the Desktop on 2026-10-18. The first real run of the workflow is the pilot `{issues: [65]}`.

## Decisions and pitfalls

- 1.0.0 put `/goal` in the prompt text. Text sent over ACP is not a Desktop command, so no goal was created: the agent did one turn and stopped. 1.1.0 sets the goal through the bridge's `goal` argument, which calls the Desktop's goal service as `/goal` does.
- In goal mode, ending a round never reaches the main agent; only `ask_user_question` does. The relay rules say so, and the `STATUS: ASK` path and the nudges of 1.0.0 were removed.
- The DeepSeek agent runs with full shell access (verified 2026-10-18: `pwsh` ran with no approval, even outside its workspace). So "asks" are mostly `ask_user_question` calls, and the approval path is a fallback.
- Haiku managers copy text verbatim. Answers are cut to the last 6000 characters (the bridge reports the last 12,000 characters of a goal turn), and main-agent results to 3800 characters. Long logs go to files.
- `E:/zet` is usually also the user's own DeepSeek workspace. The cleanup removes it only when it is empty (`ownWorkspaces: false`).
- Commit `scripts/issue-dev/` before a run: the DeepSeek agent commits and pushes on `main`, and it would otherwise include stray working-tree files.

## Change history

- `2026-10-18` — zt-issue-dev 1.1.0 (relay module v2): each conversation starts with a real dsh goal set through the bridge (`deepseek_run` `goal`, bridge 0.11). Issues end on the goal's phase. Questions from every goal round reach the main agent. New args `maxGoalRounds` and `maxAsks` replace `maxRounds`. The `STATUS: ASK`, nudge and prompt-resume paths are removed.
- `2026-10-18` — Wiki doc created with zt-issue-dev 1.0.0: issue list, sequential Haiku-relayed DeepSeek conversations, main-agent asks, repo-state check, harness.
