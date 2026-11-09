# ★ JOB MAP — the restart point of the digest workflows ★

> **READ THIS FIRST.** This file is the **starting point of every restart** of work on the digest and idea-grow workflows. Before you change, resize, run or resume a DeepSeek job, find it in the map below. The map says what one DeepSeek subagent (one conversation) does, how many messages it receives, and whether the job is in its current shape or has a proposed change waiting.
>
> **Keep the map true.** When a release changes a job's size, its messages or its shape, change its row here in the same commit. A stale map is worse than none: the next agent restarts from it.

| | |
|---|---|
| Map written | 2026-11-08 |
| Code it was read from | `main` at 3a1dcf33 (zt-digest 0.48.2, `zt-digest-lib` v59, `zt-cited-cards` v6); Cited Cards and Term Cards rows updated for zt-digest 0.49.1 (base 5.11.41, `zt-term-cards` v16) |
| Last proposal | [docs/digest-job-messages-proposal.md](../digest-job-messages-proposal.md) — Cited Cards and Term Cards (concept) as conversations of 4 messages of 12 items. **Implemented 2026-11-08** (zt-digest 0.49.0; 12 items a message since 0.49.1). No proposal is open. |
| Read with | [workflow-principles.md](workflow-principles.md) (the owner's rules), [docs/digest-cost-experience.md](../digest-cost-experience.md) (the cost evidence), [docs/wiki/workflow-module-structure.md](../wiki/workflow-module-structure.md) (the module shape) |

## How every module runs

Every module has one shape: **one start step → one queue of Haiku managers → one finish step**.

- A start or finish step is a DeepSeek *drive job*: it runs one PowerShell step script, which plans the work, writes a job file and prints one JSON line.
- A Haiku manager takes up to **8 jobs**. At most **8 managers** run at once (64 conversations).
- A *job* is one DeepSeek conversation (V4.1 Flash, official source, effort high). It writes its draft, runs its SELF-CHECK, and answers with one line. The manager collects the answer and deletes the conversation.

## The three conversation patterns

| Pattern | What happens | Why |
|---|---|---|
| **Messages** (`then`) | The planner writes the job as one conversation of several user messages. The manager sends message 1, waits for `finished`, then sends each item of `then` with `deepseek_prompt`, one at a time. | A DeepSeek turn writes about 35K–60K output tokens whatever it holds, and it thinks only about what its user message gave it. 40 Cards in one message found 1 of 12 natural faults; the same Cards in messages of 12 found 9 (docs/digest-cost-experience.md, "Card review size test"). A tool result does not reset the thinking; a user message does. |
| **Continued** (`continue`) | A later pipeline step sends its job into the conversation an earlier step opened. The order comes from the steps, not from a planned list. | The conversation keeps its context as a cached prefix, so a follow-up is cheap (#191, #194). |
| **One message** | All the job's material is in one message; the job thinks two passes, writes once, checks once. | When the material is small, one careful turn is enough, and forbidding browsing is the main saving. |

## The map

Status: **current** = the code on `main` does this. **PROPOSED** = the owner ordered a change; the proposal is written and the code is not changed yet (a proposed row shows the current shape first and the target after the arrow). No row is PROPOSED now.

| Workflow / module | Job | One subagent covers | Messages per conversation | Pattern | Status | Where the size is set |
|---|---|---|---|---|---|---|
| Digest groups, **history** | Write a group's reading notes | One group = one chapter | One per source unit | Messages | current | `message_parts`, `ledger.rs`; blueprint `groupBy: "chapter"` (history only) |
| Digest groups, **econ / math** | Write a group's reading notes | One group (soft limit 25,000 characters, up to 3 units) | 1 | One message | current | prepare options `max_chars`, `max_units` |
| Card review (whole-book card cycle, link check) | Faithful check and fix of Literature Cards | Whole groups up to 40 Cards | One per 12 Cards (3–4) | Messages, then Continued | current | `CARDS_PER_JOB` 40, `CARDS_PER_PART` 12, `card.rs` |
| Card review, fix rounds | Fix what the check found | The same job's Cards | +1 per fix round | Continued | current | `cardFixRounds`, `zt-digest-lib` `cardCycle` |
| Cited list read (the successor of "Cited normalize") | Copy one entry per work from numbered reference lines | 4 parts of up to 60 lines (12,000 characters) = up to 240 lines | 4 (one per part) | Messages | current (owner kept it, 2026-11-08) | `PART_LINES` 60, `PART_CHARS` 12,000, `PARTS_PER_JOB` 4, `cited.rs` |
| Cited Cards | Write a Cited Card per cited work | 48 works (4 jobs of 12) | 4 (one job of 12 works each) | Messages | current (2026-11-08) | `DEFAULT_PER_JOB` 12, `MESSAGES_PER_CONVERSATION` 4, `cited_cards.rs`; blueprint `citedCardsPerJob` 12 |
| Links | Fill link gaps | Up to 3 groups or 30 gaps | 1 | One message | current | `Join-LinkJobs`, `layer-steps.ps1` |
| Term table | Find Term candidates (explorers), then merge | One explorer part of about `-ExploreChars` characters; one merge job | 1 each | One message | current | `terms.rs` |
| Term Cards, concept | Write Term Cards | 48 terms (4 jobs of 12) | 4 (one job of 12 terms each) | Messages | current (2026-11-08) | `DEFAULT_PER_JOB` 12, `MESSAGES_PER_CONVERSATION` 4, `term_cards.rs`; blueprint `termCardsPerJob` 12 |
| Term Cards, mathematical | Write → self-check → review → fix in one job | 2 terms | 1 | One message | current | `DEFAULT_MATH_PER_JOB` 2; blueprint `termCardsMathPerJob` |
| Term link | Link Terms in Card text | No content job (a drive step only) | — | — | current | `term-steps.ps1 -Step link` |
| Events / Findings | Write Event or Finding Cards | 3 groups | 1 | One message | current | `-GroupsPerJob` 3, `event.rs`, `finding.rs` |
| Model layer | brief → fill → derive → review → split → close → explain | One model | One per step (about 6–8 or more) | Continued | current | `zt-model-lib` `compileBatch` (#194, #195) |
| Explain apply | Fix a refused derivation explanation | One model part | +1 per fix, up to 3 rounds | Continued | current | `model-steps.ps1 -Step explain-apply` |
| Math stages (boundary, proof, theory) | Write and review mathematical parts | 4 units | 1 | One message | current | `math-steps.ps1 -UnitsPerJob` 4 |
| Figures (digest stage) | Attach figures to Cards | Up to 80 Cards | 1 | One message (no browsing) | current | `CARDS_PER_JOB` 80, `figures.rs` |
| zt-figure-explain | Explain each image | 8 images | 1 | One message | current | `IMAGES_PER_JOB` 8, `figure_explain.rs` (the `deepseek_run` `images` limit) |
| Skeleton, Bridge, Finalize fix | Structure Cards, Bridge judging, final fixes | Bridge in chunks of 40; the others small fixed jobs | 1 | One message | current | `bridge.rs` `CHUNK` 40 |
| Organize | Decide the tree's order | Opus judgement role, no DeepSeek job | — | — | current | `zt-organize` |
| zt-idea-grow | Grow Topic Cards from a section | One section entry; one manager per 8 entries | 1 | One message | current | `grow.rs`, `zt-grow-cycle` |
| zt-term-sync | Match a book's Terms to the standard list | About 300 open rows (a tail under 50 joins the others) | 1 | One message | current | `ROWS_PER_JOB` 300, `term_sync.rs` |
| Drive jobs (every module) | Run one step script | One command | 1 | One message | current | `ztDrive`, `zt-workflow-lib` |

## Facts a restarting agent needs

- **"Cited normalize" no longer exists.** zt-digest 0.48.0 (2026-11-07) deleted the reference-list parser and its normalize jobs (60 rows a job). The job that reads the reference list now is the **Cited list read** (`zt-digest cited -Step plan`), already a conversation of 4 messages. A 2026-11-08 draft cut it to 30 lines a message (120 a job); the owner withdrew that the same day, so it keeps 240 lines a job.
- **Which managers send `then`:** the content manager of `zt-digest-lib` (card cycle, digest groups), the manager of `zt-cited-cards` (Cited list read and Cited Cards writers), and since v16 the manager of `zt-term-cards`. A job is one message: its own rows, output, SELF-CHECK and ledger unit; the compiler's `jobs` step groups four jobs into one conversation and writes each later job as a follow-up file (`jobs\<id>.next.md`).
- **A job size lives in two places:** the compiler's default and range, and the blueprint arg (`termCardsPerJob`, `citedCardsPerJob`). Change both together, or the blueprint's old default overrides the binary (this happened once: the 12-term release never ran).
- **Measured cost of the 2026-11-08 change:** on 24 items, the 4-message conversation wrote as much or more per item but cost more (Term +69%, Cited +23%), all of it cached re-reads (proposal section 5). A second test at 4 messages × 12 items (48 a subagent, no code change: `-PerJob 12`) cut the extra cost to Term +18% and Cited +4%, with depth about that of the old jobs (proposal section 6); the owner chose 4 × 12 (zt-digest 0.49.1). A real Haiku manager then ran one 4 × 12 Term conversation end to end: every message sent in order, every draft passed, 0.0201 a term against 0.0230 for the old jobs (proposal section 7). Read that before resizing these jobs again.
- **Not yet run on a real book:** the card-review messages (v58), the history chapter grouping (0.47.5) and the owner's 2026-11-06 job sizes. Score the first book against the baselines in docs/digest-cost-experience.md.
- **Never measured with a session report:** Term Cards, Cited Cards, Links and the card cycle.
