# Workflow principles (digest and idea-grow)

What the 2026-10-26 migration to the Haiku-manager + DeepSeek module structure taught (issues #175–#182, commits 0727978 … 09dfc2a; final versions base 5.7.1, econ 6.7.1, math 5.7.1, idea-grow 4.0.6, zt 0.41.0, zt-check 0.14.1, zt-digest 0.29.16). The module rules themselves are in [workflow-module-structure](../wiki/workflow-module-structure.md); stages and modes are in [digest-workflows](../wiki/digest-workflows.md) and [idea-grow-workflow](../wiki/idea-grow-workflow.md). This file holds the working rules and the lessons behind them.

## Working rules (from the user)

- When the user has authorized you and said not to ask, do every follow-up yourself: no proposal cards, no `wrap_up` questions, no "want me to…". Report what was done. Why: a card is still a question; the user objected on 2026-10-26.
- Never do digest work by hand (cards, links, terms, models). Only the workflow does it; you fix the workflow and its tools. Never touch the live archive `D:\zt` in a trial; use a copied box.
- Claude tokens are precious: no Claude agent that only runs a command, no per-item agents for script work. Claude does design roles only (model architect, HTML visual composer, organize); DeepSeek does content; scripts do the rest.
- Keep it simple: one manager, many DeepSeek subagents, straight-line modules, one job that reviews and fixes (not a review job plus a fix job). Add a self-check tool for a job that fails often instead of orchestration around it. Do not overbuild.
- Test by resuming (`mode resume`) or a single-stage mode (e.g. `mode term-cards`) on the last staging folder. Never rerun a whole digest to verify; a full run costs about two hours. A run has one resume; when it is spent, a fresh run needs a new key and staging folder.
- Fix workflow bugs you meet during a run without asking, test-first, with the wiki, then reinstall. Stop a run only when it crashes or a stage cannot go on (2026-10-27: "unless … conditions that crash the workflow, there's no need to stop"); then fix and resume at the break. A failed unit or a bug found in passing waits until the run finishes. Watch the journal and the ledger; a failed stage, repeated empty claims or a long-claimed unit is abnormal.
- A trial that works is not reworked: updates found in it wait for the next trial; a broken trial is fixed and resumed at its break.

## Principles added 2026-10-27 (Literature notes, #183–#192)

From the Literature-note pilot (#189), the one-conversation card cycle (#191) and the live backfill and relink (#190; final versions base 5.8.5, econ 6.8.5, math 5.8.5, zt-digest 0.33.6).

- One DeepSeek conversation does a whole job: review, fix, fill and its own check (the output check tool). A later round continues the same conversation with `deepseek_prompt`, and the cycle deletes every kept conversation after its last round. Why: it saves tokens and time and keeps quality, because the job's SELF-CHECK gates it (user, 2026-10-27).
- Run on the live archive only when the user names it ("only on the current main running service"), one book at a time, and back it up first with `VACUUM INTO` and an integrity check. Every other trial runs on a copied box.
- A gap that a deterministic pass can close is closed by that pass alone (for example `zt-digest term-link -Step plan|apply`). Do not rerun the workflow and do not start an agent. Plan first, read-only, and apply only after the counts make sense.
- Every mode that writes a Card runs the Term-link pass after its writers. `framework-harness.mjs` guards it for both variants. A new writer stage or mode must join that guard.
- A pass is done when a second plan changes nothing. Check it after every apply. When the second plan still changes a Card, that is a bug in the pass, not noise.
- A typed refusal such as `card_locked` is the owner's decision. Stop, write nothing, report it, and give the exact command to run after the owner acts. Never unlock a tree or work around a Lock.
- After a job, remove its test boxes, copies, scratch files and test services. Use the Recycle Bin. Before deleting a folder, check whether it holds a book's hash-pinned source: `D:\zt-digest\grossman2002integration-test` is Grossman's source, not test data. Delete backups only when the user asks.
- A finished book is never rerun. A legacy book is adopted on a copy of its staging folder (`ledger adopt`), and only the needed mode runs.

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
| A mode without the pass after it | Mode `cards` rewrote 421 Literature notes and ran no Term-link pass, so a term's first use carried no link. Seven more modes had the same gap. | A stage that changes Cards must be followed by every pass that derives from Card text. The harness checks mode membership, the stage's `when` and the upstream order (base 5.8.4–5.8.5). |
| A done unit that skips later changes | `term-steps.ps1 -Step link` answered `skipped` when `termlink/book@1` was already done, even though Cards changed after it | An idempotent pass reruns without a claim (`relinked`); the done unit stays done. |
| Address characters read as syntax | The table check counted every `\|` on a line, and Term addresses hold `\|` (`[[免税@K/1\|a\|al]]`), so each line with one Term link was a "table" and no other term on it was linked (1,140 links missing) | Parse `[[…]]` before any line-level heuristic (zt-digest 0.33.5). Look for whole skip classes with unexpected counts (3,936 `table` skips on prose). |
| Legacy shapes in old runs | Adoption and the card plan read `owned-gNN.txt` logs that named Topic Cards now owned by zt-idea-grow, and refused a correct book | Old artifacts only count inside the book's own Literature tree (`owned_lines`, `lost_in_book`, 0.33.2–0.33.3). |
| Checks that re-derive instead of read | `book-cards-finish` re-parsed links from bodies and kept rules the cycle never records, so it refused a clean cycle | A finish check reads the recorded view and keeps only the rules the cycle owns (0.32.2–0.32.3). |
| Writers beyond their scope | The card cycle rewrote Event and Finding Cards as prose, and a Finding lost its labelled fields | A cycle declares the Card types it may write (`WRITABLE = idea, anchor`). Check the per-type diff of the tree after a pilot. |
| A prompt's first line overrides its rules | The managers' lead line said "record each conversation and delete it", so kept conversations were deleted at collection | The first line of a manager prompt must agree with every later rule. Test the rendered prompt, not only the rule (lib v46). |
| Faithful to what? | Rewritten legacy notes kept cross-card claims (`依赖 [[…]]：…`) that their own source lines did not support | Every sentence rests on the note's own source lines. A Link without support stays as a neutral `另见 [[…]]`. |
| Contract between majors | A 5.7 staging folder cannot be driven by 5.8 | Copy it, strip `run_contract` and the ledger, repoint the manifest paths, then adopt it again. |
| Only resumes tested | The first fresh full run after the drive migration stopped at its first step: `deepseek_run` refused a staging folder that only that step creates (2026-10-27) | A harness check covers the fresh path as well as the resume. A job that runs before its folder exists names a `workspace` that does (base 5.8.6). |
| A count read as a line count | `faults: N` counts blocking lines only; the Link SELF-CHECK compared it with every line and refused 7 of 8 groups for advisory lines | Read a checker's count against the lines it counts (zt-digest 0.33.7). |
| A kept Card checked against a new draft | On a resume, plan revision 2 re-rendered 18 Term rows; publication kept the revision-1 Cards, and the book completion compared their parts with the new draft | A kept row is checked against what was kept (its head), never against a later plan (zt-digest 0.33.7). |
| Lines that carry unread fields | `finalize-final` overflowed at 429 characters with fields no module reads; a line also drops every false, so a `false` flag reads as its default | A step line carries only what its module reads; a flag whose meaning is `false` is sent under its positive name (zt-finalize v6). |
| Two digests on one box | On 2026-10-29 a second digest wrote to the live box while hansen2021demand ran; the archive revision is global, so the link-check plan, the Organize survey and the Term-link plan refused with a changing read view | Pure-read steps (`audit`, `organize-survey`, `card`/`term-link -Step plan`) read their whole set again after 5–60 s waits (zt-digest 0.35.2). A step that claims or writes is never rerun. |
| A resume reuses a stale audit | The resume republished Term cards and redid the link check, then its Audit stage took the run-1 audit as settled; Finalize planned from it and refused on three Term-card parts that no longer existed | Open: on resume, the read-only audit must run again (drop `settledOnResume` in `zt-audit`). |
| Retyped drive lines | A drive job wrapped its line in a json code fence (Term link, run 1) or retyped it with curly quotes (`“ok”`, Term-card finish on the resume), so a successful step read as a failed stage | Open: `ztDriveAnswer` should skip fence lines and read curly quotes as straight when the line has no straight quote (zt-workflow-lib). |
| A resume probe answered as a fault | On resume, `finalize changes` answered `ok:false` because run 1 recorded no correction cycle; the stage then planned normally | Open: a missing retained cycle is a no-op line, not `ok:false`. |
| Parallel batches share one job file | hart1995firms run 1 (2026-10-29): two model batches wrote `models\drive\jobs.json` side by side; one batch's brief overwrote the other's before its writers read it, so five formal models got no brief and failed | A batch step names its files after its batch folder (`jobs.<batch>.json`, model-steps.ps1). Any step that runs beside a copy of itself writes only its own files. |
| A layer that writes beside a running writer | hart1995firms run 1: the Findings plan covered groups whose digest writers were still running; the writers rewrote the anchors and dropped the `印证` Links, so four units failed | `event`/`finding -Step plan` puts a group in a job only once its digest unit is done; `-Step apply` writes back a dropped Link for applied jobs (zt-digest 0.35.3). |
| A binary swapped under a run | hart1995firms run 1: another session reinstalled `zt.exe` at 03:33; the Events claim at that second answered "program not found" and the layer stopped with 20 units ready | Do not deploy `zt`/`zt-digest` while another session's digest runs. The resume claims the ready units; nothing is lost. |
| A later stage edits a finished Model Card | hart1995firms resume: bridge/book@2 appended a `跨书关联` line to Model Card `1|b`; cards-book@3, which finished after it, refused the Card as a rewrite of the reviewed draft | `book-cards-finish` ignores the Bridge's own `跨书关联：` paragraphs (zt-digest 0.35.4). A stage that writes onto a finished Card must be one the drift checks know. |
| An explain pass over a model whose chains moved | hart1995firms gaps: model 16's explain pass numbered parts 1-3 as one chain; the build put a shared setting between parts 2 and 3, so `explain-finish` refused the chain headers. Three of its supplied parts also lost their 补 markers in that pass | Fix the headers and markers on the Cards and rerun the finish; the Cards are content, the model is not rebuilt. |
| The Term-link pass moves links on finished Cards | hart1995firms gaps: the resume's Term-link pass moved term links to their first occurrence on Model Cards `3|a`, `5|a`, `7|a` and on checked Cards; `book-cards-finish` refused them as rewrites | The drift checks compare the text with Term links turned back into labels (zt-digest 0.35.4); a Model completion records that fingerprint too. The archive's `applied_ops` log shows which op changed a Card. |
| A legal option the finish refuses | hart1995firms gaps: mode cards with `ideaSample: 0` left an empty sample record and `book-cards-finish` refused it | The cycle records `not_sampled` (0.35.4). An argument value the workflow accepts must give a record its finish accepts. |
| A binary rebuilt during a Term-card branch | hart1995firms gaps: a 0.35.4 build made every Term-card check stale (`versions`), and `apply` refused the plan as `plan_stale` | Finish the Term-card branch with the binary its plan froze (keep the old exe beside the new one); a new plan revision rewrites every job. |
| Resume right after a stop | The first resume after a TaskStop refused: two Link claims had recorded activity in the last 120 s | After a stop, check that no DeepSeek job runs, wait out the 120-second quiet period (`ledger reclaim -Release`), then resume; a refused resume does not spend the run's one resume. |

## Release discipline

- Test first. Harnesses must be green: `scripts/book-digest/tests/framework-harness.mjs` and the idea-grow harness; each step script has its own `.tests.ps1`.
- Update the block's wiki doc in the same commit (project-wiki skill; `node ~/.claude/skills/project-wiki/scripts/wiki.mjs check`).
- A patch bump of a blueprint keeps a prepared run drivable; a minor bump does not. Keep fixes patch-level, then rebuild and deploy zt-digest.
- Install every changed workflow in both scopes (project and global) and verify the install, including the DSMP block.
- A step script runs from the repo, so a step fix needs no reinstall. A lib or blueprint fix needs one.
- Rust changes need `cargo` on `D:` and deploy. On Windows a running `zt.exe` or `zt-digest.exe` is locked: rename it first, then copy the new one.
- Never rebuild zt-digest under a run you still mean to resume.
- Run commands that take over 2 minutes in the background.
- A change to a mode or a stage's `when` is a blueprint bump: raise the patch version of base, econ and math together, rebuild zt-digest (it embeds them), and install both workflows in both scopes. Dry-run the changed modes with `assemble.mjs simulate --mode <m>` and read the stage order.
- A bug fix comes with a test that fails before the fix and reproduces the live case. When a small fixture passes, rebuild the real input (the book's vocabulary or the Card's old body from the backup) until the test fails.

## Known limitations still open

- A `kept_earlier` Term row's receipts name rows by number, and the numbering shifts between plan revisions (becker2025division row 45 kept `1|a|as` with receipts for `1|a|ar`). The book completion no longer reads them; the receipts themselves are still wrong.

- The last Term rerun on staging8 (zt-digest 0.29.16, `kept_earlier`) was never rerun live; the fix has tests only.
- Model parts can still fail on content (closure, unanchored); expect 5–6 of 7 models on a small book.
- Term-card pass rate is content-bound: 38/38 jobs ready after review-and-fix v8, but publication was untested live.
- `D:\zt-digest\grossman2002integration-terms` (a Term-backfill copy) is held open by another program; recycle it once that program closes. The other pilot boxes and backups were recycled on 2026-10-27.
- The 10-note faithfulness read of the Literature notes was not repeated on the live books after the 另见 prompt fix.
- `tests/term_pilot.rs` failed because a stale `zt-check 0.1.0` sat beside the test binary in `D:\zt-target\debug` (and `release`), and the seam's `find_program` prefers that sibling over PATH, before the installed `zt-check 0.15.0`; `--report` has been in the checker since 0.5.0. Rebuild `zt-check` with `CARGO_TARGET_DIR=D:\zt-target` (debug and release) before running the suite.
- The math digest has been built and harnessed but never run live (no math material).
