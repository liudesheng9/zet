# Proposal: a robust digest workflow

Status: proposed 2026-11-14, for after the IMEN1 queue. Nothing here changes a running workflow.

## Problem

The digest workflows (zt-econ-digest, zt-math-digest, zt-history-digest and their libraries) have had many releases, but live runs still stop on new faults. The owner's question (2026-11-14): why do so many updates still not give a robust workflow?

Example from the IMEN1 A→B gap (2026-11-14): zt-digest 0.54.0 and the v19 libraries fixed the compiler fault that left a model part `待写`. The first mode `models` runs on rotunno2026industrial and riboni2026presidential still did nothing. The real `start` step called each reopened model "built" (parts above 0, no issue) and never read the listing's `todo`. The v19 tests had used a stand-in for that step. Each book lost about 40 minutes (fixed in 9b1dd122). The Riboni resume after that hit a second fault: the earlier run had already settled `cleanup/book@1`, so the ledger refused to register the resume's new DeepSeek conversations (`stale_claim`).

## Causes

1. **Tests use stand-ins, not the real seams.** The harnesses replace the step scripts, the ledger and DeepSeek with fakes. Most live faults sit where two real parts meet, which is where the fakes are.
2. **The repair paths are the least tested code.** Reopen → mode run → resume → cleanup by hand runs only after a failure. Each live fault adds one more special path (resume a reviewed model, settle a stray, repair a retyped line), so the number of states grows and each path runs perhaps once.
3. **Several state holders can disagree.** The ledger, the model compiler's `state.json`, the box, the DeepSeek Desktop and the workflow journal each hold their own truth. "Built" is decided by part count in one place and by `待写` in another. A settled cleanup in the ledger blocks conversations that a later run starts.
4. **LLMs are in the control path.** Drive jobs and Haiku managers carry step commands and JSON lines. Each new way they misbehave is a new failure class: a promise instead of a run, a retyped line (`{"ok"":`), a manager that runs the step itself, a new conversation instead of a continued one. These cannot be closed one at a time.
5. **The environment is shared.** Two digests and a post-digest session write one box (`read_view_changed`); another person uses the DeepSeek Desktop (stray conversation 3b1d187b).
6. **Fixes land during the queue.** Time pressure favours the narrowest patch that unblocks the current book.

## Proposal

| # | Change | Removes | Size |
|---|---|---|---|
| P1 | **Take LLMs out of the control path.** The workflow script runs each step command itself (one deterministic step agent or a direct process call); DeepSeek only writes content; no drive job and no manager relays a step line. | cause 4: drive-line retyping, promises, self-run steps, foreign claims | large; biggest gain |
| P2 | **One owner of state: the ledger.** "Built", "needs work", "settled" and "republish" are decided by one ledger query that every step reads. The `start` step, the compiler and the finish ask the same question. A later run reopens the run-level cleanup instead of meeting `stale_claim`. | cause 3 | medium |
| P3 | **One `repair` command per unit.** `zt-digest repair -Out <out> -Unit <unit>` reopens the unit, runs only its layer, then republishes exactly the book publications that depend on it (cards-book, skeleton, figures, bridge, audit) and settles cleanup. It replaces reopen + mode run + resume + by-hand cleanup. | cause 2 | medium |
| P4 | **Seam tests on a copied box.** Every release runs each repair path and one small book end to end with the real step scripts and the real ledger on a copied box (own `ZT_CONFIG_DIR`, own port), including a reopened model with a `待写` part and a resume after a settled cleanup. Harness fakes stay for speed only. | cause 1 | medium |
| P5 | **Release discipline.** No release during a queue except a blocker fix; a blocker fix is first run against the live failure's own state (for example `model-steps.ps1 -Step start` on the failing book) before the gap opens. | cause 6 | small |
| P6 | **Isolation.** One DeepSeek workspace per run (no shared parent `D:/zt-digest`); pure reads in a digest retry by design, and the post-digest never runs on a book with an open unit. | cause 5 | small |

Order: P5 now (rule only); P4 first after the queue, because it shows which seams fail; then P2 and P3 together; then P1, which replaces the drive-job layer (zt-workflow-lib) and needs its own trial book.

## Done when

- A trial book and every repair path pass on a copied box with the real scripts.
- A reopened unit reaches `complete:true` with one `repair` command and no by-hand step.
- No step line passes through an LLM.
- Two queue books in a row run with no stop and no by-hand fix.
