# Workflow principles (digest and idea-grow)

What the 2026-10-26 migration to the Haiku-manager + DeepSeek module structure taught (issues #175–#182, commits 0727978 … 09dfc2a; final versions base 5.7.1, econ 6.7.1, math 5.7.1, idea-grow 4.0.6, zt 0.41.0, zt-check 0.14.1, zt-digest 0.29.16). The module rules themselves are in [workflow-module-structure](../wiki/workflow-module-structure.md); stages and modes are in [digest-workflows](../wiki/digest-workflows.md) and [idea-grow-workflow](../wiki/idea-grow-workflow.md). This file holds the working rules and the lessons behind them.

## Working rules (from the user)

- When the user has authorized you and said not to ask, do every follow-up yourself: no proposal cards, no `wrap_up` questions, no "want me to…". Report what was done. Why: a card is still a question; the user objected on 2026-10-26.
- Never do digest work by hand (cards, links, terms, models). Only the workflow does it; you fix the workflow and its tools. Never touch the live archive `D:\zt` in a trial; use a copied box.
- Claude tokens are precious: no Claude agent that only runs a command, no per-item agents for script work. Claude does design roles only (model architect, HTML visual composer, organize); DeepSeek does content; scripts do the rest.
- Keep it simple: one manager, many DeepSeek subagents, straight-line modules, one job that reviews and fixes (not a review job plus a fix job). Add a self-check tool for a job that fails often instead of orchestration around it. Do not overbuild.
- Test by resuming (`mode resume`) or a single-stage mode (e.g. `mode term-cards`) on the last staging folder. Never rerun a whole digest to verify; a full run costs about two hours. A run has one resume; when it is spent, a fresh run needs a new key and staging folder.
- Fix workflow bugs you meet during a run without asking: stop the run, fix test-first, update the wiki, reinstall, resume. Watch the journal and the ledger; a failed stage, repeated empty claims or a long-claimed unit is abnormal.
- A trial that works is not reworked: updates found in it wait for the next trial; a broken trial is fixed and resumed at its break.

## Module structure in one paragraph

Each feature block is one Haiku manager (≤8 DeepSeek conversations each, ≤8 managers alive at once) plus DeepSeek subagents. Content jobs write Cards, Links, Term drafts, reviews and fixes and run their own self-check. Deterministic chains are one script step (`term-steps`, `layer-steps`, `book-steps`, `model-steps`, `math-steps`, `grow-steps`) run by a DeepSeek drive job through `ztDrive`. No relay agents. A step answers with one compact JSON line of ≤400 characters. The DSMP block is inlined in every install, in project and global scope. Conversations are deleted, never archived.

## Failure classes found on live runs

| Class | What happened | Lesson |
|---|---|---|
| Retyped or unescaped answers | A Haiku manager retyped a long start line; another unescaped path backslashes in a line | Keep step lines ≤400 characters; the strict reader needs a top-level `ok`; `ztDriveParse` doubles lone backslashes; never rely on a manager to copy long text. |
| Leaked markup | `</answer>` ended up in a cleanup report | Cut wrapper markup in the shared lib (`zt-workflow-lib` v7) before parsing. |
| Line overflow read as success | A fill line over 400 characters came back as `{ok,detail}`; the module saw no jobs and the models stayed 待写 | Overflow is `ok:false` with `line_overflow`, never a quiet shape change. |
| Silent empty results | Term publication wrote 0 rows with no reason | Every refusal names itself: `publish_error` carries `{ok:false,code}`. |
| Lease expiry | A halted stage left claims leased 2 h so resume refused with `run_active`; cards-book and model-deps leases were too short | Resume runs `ledger reclaim -Release` first; long stages take a long lease (7200). |
| Stale plans | Term-cards ran beside digest writers; models changed notation and apply refused `plan_stale`; plan revision 2 conflicted with rows revision 1 had published | Run term-cards after cards-book; a Term head published by an earlier plan revision is kept (`kept_earlier`). |
| Closed sessions | After a Desktop restart, close-round follow-ups failed with "unknown or closed session" | A refused continuation starts a new conversation from the fallback (`zt-digest-lib` v43). If `deepseek_run` says "Internal error", restart the DeepSeek Desktop. |
| Contract versions embedded in zt-digest | A Term backfill was refused "incompatible workflow contract" because zt-digest predated the blueprint bumps | After every blueprint bump, rebuild and deploy zt-digest. `zt-digest version -Workflows` lists the embedded versions. |
| Box pin paths | A doubled-backslash `ZT_CONFIG_DIR` did not match the box pin | Compare paths with separators collapsed (`zt-check` 0.14.1). |
| Positional versus identity reads | Term publication and backfill read published heads by table position; one refused row blocked the rest | Read by Term identity and publish every ready row (#182, 0.29.11, 0.29.14). |
| Order past z | Term-list Side labels past z sorted differently from the archive | Order exactly as the archive does (0.29.15). |
| Stale test binaries | The `term_cards_apply` seam ran an old `zt` | Point `ZT_TEST_ZT` at a current build or rebuild `target/release/zt.exe` first. |
| Cwd that does not exist yet | Drive jobs failed because the stage folder was not created | Drive cwd is the run out folder. |
| `resume` lacks args | Resume has no `args.citationKey`, so the key was undefined downstream | Read the key from the run, not from args (#181). |
| Copies in the box | The register stayed `planned` with 156 `ambiguous` faults | Five copies of one paper in a pilot box cause it; not a bug. |

## Release discipline

- Test first. Harnesses must be green: `scripts/book-digest/tests/framework-harness.mjs` and the idea-grow harness; each step script has its own `.tests.ps1`.
- Update the block's wiki doc in the same commit (project-wiki skill; `node ~/.claude/skills/project-wiki/scripts/wiki.mjs check`).
- A patch bump of a blueprint keeps a prepared run drivable; a minor bump does not. Keep fixes patch-level, then rebuild and deploy zt-digest.
- Install every changed workflow in both scopes (project and global) and verify the install, including the DSMP block.
- A step script runs from the repo, so a step fix needs no reinstall. A lib or blueprint fix needs one.
- Rust changes need `cargo` on `D:` and deploy. On Windows a running `zt.exe` or `zt-digest.exe` is locked: rename it first, then copy the new one.
- Never rebuild zt-digest under a run you still mean to resume.
- Run commands that take over 2 minutes in the background.

## Known limitations still open

- The last Term rerun on staging8 (zt-digest 0.29.16, `kept_earlier`) was never rerun live; the fix has tests only.
- Model parts can still fail on content (closure, unanchored); expect 5–6 of 7 models on a small book.
- Term-card pass rate is content-bound: 38/38 jobs ready after review-and-fix v8, but publication was untested live.
- The pilot box `D:\zt-pilot-172` was removed except `grow5b` and `staging6`, held open by the DeepSeek Desktop; recycle them after a Desktop restart.
- The math digest has been built and harnessed but never run live (no math material).
