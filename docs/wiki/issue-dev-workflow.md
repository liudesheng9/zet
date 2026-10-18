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

`zt-issue-dev` is a Claude Code Workflow that develops GitHub issues of `liudesheng9/zet` with DeepSeek doing the work. It takes a list of issues and develops them one at a time. For each issue, a Haiku manager starts one DeepSeek Harness conversation (DeepSeek V4.1 Flash, official source, effort `max` by default) with the `/goal /tdd` task. The DeepSeek agent has its own shell. It develops test-first, deploys, commits and pushes. When it needs something it cannot do, it asks. The **main agent** (an agent on the session model) does what was asked, and the manager sends the result back. The issue ends when the agent reports `STATUS: DONE` or `STATUS: NEEDS-HUMAN`. The block exists so that this relay loop, its limits, and its roles are fixed and testable. The workflow is built and frozen by the Workflow Framework Protocol.

**Owns:** the blueprint and sim file, the `zt-issue-relay` module (the relay loop and the manager, main-agent, and delete prompts), the issue-list and repo-state scripts, the installed script and its registry, and the harness.

**Does not own:** the book-digest workflows and the framework assembler they share: see [Digest workflows](digest-workflows.md). The code that the issues change: the other blocks of the [wiki](README.md).

## At a glance

| | |
|---|---|
| Entry points | `Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: ["65-91"] } })`; `node scripts/issue-dev/tests/issue-relay.tests.mjs` |
| Key files | `scripts/issue-dev/workflow/zt-issue-dev.blueprint.json` — the blueprint: args, roles, consts (the task prompt, the relay protocol), stages |
| | `scripts/issue-dev/workflow/modules/zt-issue-relay.js` — the `develop` stage: the per-issue relay loop |
| | `scripts/issue-dev/workflow/zt-issue-dev.sim.json` — the dry run used by `install` |
| | `scripts/issue-dev/issue-list.mjs` — expands numbers and `a-b` ranges and reads each issue with `gh` |
| | `scripts/issue-dev/repo-state.mjs` — read-only state after an issue: clean, pushed, issue state, deployed zt version, service up |
| | `scripts/issue-dev/tests/issue-relay.tests.mjs` — the harness |
| | `.claude/workflows/zt-issue-dev.js` — the installed, read-only build; `.claude/workflow-framework/zt-issue-dev/` — its registry (blueprint, sim, lock, sources) |
| Depends on | the Workflow Framework (`~/.claude/skills/workflow-framework-protocol`), the DeepSeek Subagent Managed Protocol block, the `deepseek` MCP bridge, `gh`, `git`, `zt` |
| Used by | the user, who runs it from a Claude Code session |
| Tests | `scripts/issue-dev/tests/issue-relay.tests.mjs` |

## Concepts

- `Manager` — a Haiku agent (role `manager`) that relays one DeepSeek conversation: it starts it, waits, responds, prompts, or deletes it. It never answers or decides anything itself. Each issue has exactly one DeepSeek conversation. Each relay step is a new manager agent on the same `session_id`.
- `Main agent` — role `executor` (no model set, so the session model) that does what the DeepSeek agent asked and returns `{outcome done|refused|failed, result, answers, decision}` (`ZIR_EXEC`).
- `Ask` — a question or approval (bridge status `needs_response`, from `ask_user_question` or a tool approval), or a finished turn that ends `STATUS: ASK — ...`.
- `STATUS line` — the last `STATUS: DONE|NEEDS-HUMAN|ASK` line of an answer (`zirStatus`).
- `Round` — one relay step that is not a plain wait: an ask answered, a nudge, a resume. Capped by `maxRounds`.
- `Outcome` — per issue: `done`, `needs-human`, `max-rounds`, `timeout`, `failed`.

## Architecture

```mermaid
flowchart TD
    P["prepare: script issue-list.mjs"] --> D["develop: zt-issue-relay"]
    D --> C["check: gate"]
    C --> R["report: value"]
    subgraph per issue, in order
      S["manager start: deepseek_run + wait"] --> L{state}
      L -- running --> W["manager wait"] --> L
      L -- needs_response --> X1["main agent per pending item"] --> RS["manager respond"] --> L
      L -- "finished, STATUS: ASK" --> X2["main agent"] --> PR["manager prompt result"] --> L
      L -- "finished, no STATUS" --> N["manager prompt nudge"] --> L
      L -- "failed / error" --> RE["manager prompt resume (once)"] --> L
      L -- "DONE / NEEDS-HUMAN / limit" --> DEL["manager delete"] --> RSs["runner: repo-state.mjs"]
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
| `maxRounds` | number 1–200 | 40 | relay rounds per issue |
| `until` | `prepare` | — | stop after the issue list is resolved |

### Result

`{total, done, needsHuman, notDone: ["#n outcome"], repoNotOk: ["#n"], check, checkError, workspace, issues: [{number, title, stateBefore, outcome, summary, rounds, asks, session, deleted, lastAnswer, trail, repo}]}`; the report also has `deepseek` (the DSMP log: `deleted`, `notDeleted`, `workspaces`).

### Scripts

- `node scripts/issue-dev/issue-list.mjs --repo R --issues '<json array>'` prints `{ok, repo, count, issues, missing}`. It exits 1 when an issue is missing or the list is malformed. It exports `expandIssues`.
- `node scripts/issue-dev/repo-state.mjs --repo R --cwd DIR --issue N` prints `{ok, clean, branch, head, upstream, pushed, dirty, issue: {number, state}, zt: {source, installed, match, service}}`. It runs `git fetch` and changes nothing.

### The DeepSeek task

The first message is `consts.taskPrompt` (the user's `/goal /tdd` prompt, `{N}` = the issue number), followed by `consts.protocol` (the relay rules: how to ask, what needs the user, the STATUS lines). The run arguments are `model: deepseek-official/deepseek-flash`, `reasoning_effort` and `skills: ["tdd"]`.

## Behaviour

### One issue

1. `zirIssue` builds the prompt and the manager starts the conversation (`op: start`). The manager calls `deepseek_wait` with 900 s waits, at most 12 per manager agent.
2. `running` starts another wait manager, at most `maxWaitCalls` (8) per issue; after that the outcome is `timeout`.
3. `finished` with `DONE` or `NEEDS-HUMAN` ends the issue. This check comes before the round limit.
4. `needs_response`: the main agent is called once per pending item. Questions go back as `answers [{id, custom}]` and approvals as `decision`. Then a `respond` manager continues.
5. `finished` with `ASK`: the main agent gets the whole answer, and a `prompt` manager sends `RESULT FROM THE MAIN AGENT (...)` plus `continueText`. Without a STATUS line, `nudgeText` is sent.
6. `failed`, `cancelled` or `error`: one `resumeText` prompt in the same conversation; a second failure gives the outcome `failed`.
7. Then the conversation is deleted, cancelled first if it is still running, and never archived. The runner records `repo-state.mjs`.

### Rules

- Issues run strictly in order: they share one checkout, one service and one installed `zt`. This is why `develop` is one stage and not a kernel `foreach`, which runs items in parallel.
- A failed issue does not stop the run: the next issue starts. `develop` itself has no retries, so a retry cannot redo finished issues.
- The `check` gate requires every issue `done`, every repo state `ok`, and every conversation deleted. It only reports (`onFail: continue`).
- The main agent refuses work that needs the user: live digest runs or resumes on `D:\zt` (#90, #91), deletes of backups, Cards or data, force-pushes, and settings changes. It never runs a Workflow itself.

## Data and storage

None: the workflow keeps no files of its own. The registry `.claude/workflow-framework/zt-issue-dev/` (blueprint, sim, lock, sources, history) is written by the framework's `install`. The DeepSeek conversations are deleted after each issue.

## Configuration

Fixed at install (a change is a new version): the DeepSeek model and skills (`consts.dsModel`, `consts.dsSkills`), the prompts (`consts.*`), the roles, and the module defaults `waitSeconds` 900, `waitsPerManager` 12, `maxWaitCalls` 8 (`ZIR_DEFAULTS`). The kernel passes only the params a blueprint gives, so the module merges its defaults itself.

## Usage

```js
// look at the resolved list first
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: ["65-91"], until: "prepare" } })
// pilot: one closed issue (reverify only)
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: [65] } })
// the full list
Workflow({ scriptPath: "E:\\zet\\.claude\\workflows\\zt-issue-dev.js", args: { issues: ["65-91"] } })
```

To change the workflow: edit the blueprint or the module, raise `version`, then run

```bash
node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs install scripts/issue-dev/workflow/zt-issue-dev.blueprint.json --sim scripts/issue-dev/workflow/zt-issue-dev.sim.json --scope project --project E:/zet
node ~/.claude/skills/workflow-framework-protocol/tools/assemble.mjs verify zt-issue-dev --scope project --project E:/zet
```

## Testing

`node scripts/issue-dev/tests/issue-relay.tests.mjs` builds the blueprint and runs it in the framework's stand-in runtime, one scenario per path:
- `expandIssues`;
- question → respond, ASK → prompt, DONE;
- the start prompt (model, effort, `tdd`, no unreplaced placeholders);
- `dsEffort: high`;
- NEEDS-HUMAN followed by the next issue, in order;
- nudge and wait managers;
- one resume, then failed;
- `maxRounds`;
- an approval relayed as a decision;
- a repo state that is not ok;
- a missing issue halting at `prepare`;
- `until: prepare`.

Gap: no test drives the real bridge. The first real run is the pilot `{issues: [65]}`.

## Decisions and pitfalls

- The DeepSeek agent runs with full shell access (verified 2026-10-18: `pwsh` ran with no approval, even outside its workspace). So "asks" are mostly `ask_user_question` calls, and the approval path is a fallback.
- Haiku managers copy text verbatim: answers are cut to 6000 characters (the tail, where the STATUS line is), and main-agent results to 3800 characters. Long logs go to files.
- `E:/zet` is usually also the user's own DeepSeek workspace. The cleanup removes it only when it is empty (`ownWorkspaces: false`).
- Commit `scripts/issue-dev/` before a run: the DeepSeek agent commits and pushes on `main`, and it would otherwise include stray working-tree files.

## Change history

- `2026-10-18` — Wiki doc created with zt-issue-dev 1.0.0: issue list, sequential Haiku-relayed DeepSeek conversations, main-agent asks, repo-state check, harness.
