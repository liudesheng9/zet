# Digest speed, part 2: where the atkinson2015lectures run spent its time — proposal

Status: proposal (2026-11-02), written at the owner's request after the Term-card branch of *Lectures on Public Economics* finished. No code changes come with this document. It continues [Digest speed](digest-speed-proposal.md) (2026-11-01, S1–S7) with the whole run measured, the Term-card branch included. [Model compiler v2](model-compiler-v2-proposal.md) holds the model-compiler design work. Section 6 records the owner's follow-ups from the end of the 2026-11-02 run.

## 1. Summary

The book took 19 workflow runs over 31 hours (2026-11-01 02:51 to 2026-11-02 09:50 local time). The workflows ran for 22.4 hours, and the owner's pause took 5.8 hours. 1,908 agents ran for 75 agent-hours: 69 on Haiku, 6 on Sonnet. The DeepSeek work runs inside the Haiku managers' time.

Five causes account for most of the time:

| # | Cause | Where | Time it cost on this book |
|---|---|---|---|
| 1 | The digest, events and findings layers run **one block at a time** | `zt-econ-digest.js` `layerRun('digest' / 'events' / 'findings')`, no `lanes` | 3.6 h wall at an average of 1.0 agent per layer |
| 2 | **Rework** after faults: model rebuilds, a second whole-book card cycle, Term-card reruns | several (Section 2.4) | about 8 h of the 22.4 h |
| 3 | **Whole-book passes** that re-read every Card although few changed | Link check, whole-book card cycle, `term-link` | 68 + 77 min card cycles, 68 min Link check (11-01), 16 min Term-link |
| 4 | **Archive renames** cost about 0.1 s per moved Card, and a preview does the work twice | zt `apply_location_mapping_tx` + `link_index`, `batch.rs` preview | Term-card publication: 63 min before zt-digest 0.41.18 (stopped), 278 s for its one `reorder` after it |
| 5 | **Per-step overhead**: a drive job of about 40 s per script call, and ledger commands of 8 s | every drive step; `ledger.rs` `Prepared::read` | 1,021 drive agents, 20 agent-hours; about 19 % of every sequential block |

The proposals in Section 3 (P1–P8) address these causes. The largest single saving is P1, lanes for the digest layers: the digest phase would drop from about 3.5 hours to about 40 minutes. The largest saving overall comes from P2 and P3, because they prevent rework instead of speeding it up.

## 2. Evidence

Sources:

- the 1,908 agent transcripts under the session's `subagents/workflows/wf_*` folders (label, phase, model, first and last timestamp of every agent);
- the archive log `D:\zt\zt.log` (`edit lock held … held_ms`, logged for holds above the threshold);
- the ledger and the Term-card artifacts in `D:\zt-digest\atkinson2015lectures`;
- direct timings of `zt-digest ledger` commands on 2026-11-02.

Times are local (UTC−6).

### 2.1 The runs

| Run | Start | Wall | What it did |
|---|---|---|---|
| wf_7c348770-ef8 | 11-01 02:51 | 3.9 h | prepare; digest, events and findings side by side (212–217 min); halted at Cards on a manager remark |
| wf_ff03cb8d-5fd | 06:53 | 4.4 h | resume; models one block of 8 at a time (234 min, about 13 models/h) |
| wf_46a710e5-168 | 11:19 | 1.9 h | models in 8 lanes (69 min, about 50 models/h); Link one block at a time (45 min, stopped at 48/229) |
| wf_7ba0b0ff-ecb | 13:27 | 2.9 h | model rebuild (43 min), Link in 8 lanes (34 min), whole-book card cycle (68 min), Organize (16 min) |
| wf_9455e938-61c, wf_ac2fefce-070, wf_4a4cd296-c6b | 16:24–18:12 | 1.6 h | rebuild passes 2 and 3 (false refusals in `model-finish`, g098) |
| wf_aa5cc4f3-ea9 | 18:16 | 1.3 h | mode cards: a second whole-book card cycle (77 min) |
| wf_4245aeec-15b, wf_75fbac3d-a3c, wf_2400d021-dd5 | 19:43–21:50 | 1.0 h | rebuilds of 32\|a, 88\|a and 95\|b (borrowers of rebuilt models) |
| wf_03c487e2-4d6 … wf_4bcd69d2-f8e | 21:51–23:58 | 1.6 h | Term cards in waves (v9): lease refusal, first starts |
| wf_49c1e151-aec | 11-02 00:02 | 2.0 h | Term cards as one queue (v10, 98 min), Term-link (16 min) |
| wf_f2bc1325-045 | 07:50 | 1.7 h | Term cards: the 120 reopened jobs (34 min), then the finish publication (stopped after 63 min) |
| wf_a75c192f-154 | 09:39 | 0.2 h | Term cards on zt-digest 0.41.18: start 6.0 min, finish 4.7 min; 748/748 rows committed |

### 2.2 Time per stage, all runs together

Concurrency is the agent time divided by the wall time: how many agents worked side by side on average.

| Stage | Wall | Agent-hours | Concurrency | Runs |
|---|---|---|---|---|
| Models | 7.8 h | 20.4 | 2.6 | 9 |
| Term cards | 4.8 h | 23.2 | 4.8 | 7 |
| Digest / Events / Findings (side by side) | 3.6 h | 3.5 + 3.6 + 3.6 | **1.0 each** | 1 (+1 small) |
| Cards (whole-book cycle) | 2.9 h | 13.6 | 4.7 | 4 |
| Link | 1.4 h | 4.9 | 3.4 | 5 |
| Organize, Term-link, Cleanup, Prepare, Audit, Report | 1.8 h | 1.9 | 1.0 | — |

### 2.3 The slow steps and why they are slow

**(a) Digest, events and findings: one block at a time.** The digest layer claims 8 groups per block, and one manager writes them. The next block starts only when the previous block's `finish` ends. On the first run each block took start 0.6 min + write about 5 min + finish 0.7 min. The 229 groups made 29 blocks, so the phase took 212 minutes. The events and findings layers (4 groups per block, `wait: 600`) follow the digest groups in the same way. In the installed `zt-econ-digest.js`, only `link` (line 3699, `lanes: 8`) and the model stage use lanes. `digest` (line 3616), `events` (2902), `findings` (3566), `cards` (2177) and `explain` (3270) do not. `layerRun` already supports `o.lanes` (zt-digest-lib v49).

**(b) Model stage.** Before lanes it built about 13 models per hour, with one block of 8 at a time and a Sonnet plan of 5–9 minutes (median 5.4 over 54 plans) at the head of every block. With 8 lanes it built about 50 per hour. Most of its 7.8 hours went to the first sequential pass (3.9 h) and to six rebuild passes (2.6 h), not to the lanes.

**(c) Whole-book cycles.** The whole-book card cycle ran twice: 68 minutes (13:27 run, 253 jobs, 4 applied) and 77 minutes. The second ran because the rebuild of 31\|a broke the import of 32\|a after the first cycle had passed. The Link check of 11-01 took 68 minutes for 2,129 Cards. `term-link` took 16 minutes: in the archive log it held the edit lock 67 times, about 12.5 s per chunk, over every Card of the book.

**(d) Archive renames.** zt applies a move as a mapping. It re-keys each moved Card, updates the Link index, rewrites each linking body and refreshes the reverse-link sections (`src/main.rs` `apply_location_mapping_tx`, `src/link_index.rs` `Changes::renamed` / `finish`). On this archive (10,850 Cards, 74,214 Links) that costs roughly 0.1 s per moved Card:

- Before 0.41.18, Term-card publication filed each repaired row with its own `create_successor -Before` call, which moved every later Term head. The first calls held the lock for 5.6 s, call 119 for 70 s. 119 of 185 rows took 63 minutes; the rest would have taken more than another hour.
- In 0.41.18 the rows are appended (2 calls, 15 s and 12 s) and one `reorder` files the list (about 1,500 Term Cards). That reorder held the lock **125 s for its preview and 153 s for its commit**. A tree-operation batch commits only under the plan hash its preview returned, so the archive does the whole rename twice.
- The reverse-link query itself is indexed (0.3 ms per target, measured read-only on the live archive). The cost is elsewhere in the rename path, and nobody has profiled it yet.

**(e) Drive steps.** A deterministic step is a Haiku manager that starts a DeepSeek conversation, which runs one PowerShell command. The manager then records the conversation in the ledger and deletes it. One measured step (`digest block 2: finish`, 11-01) took 42 s: the DeepSeek conversation took 23 s (it starts, decides, runs the script and replies), and tool loading, ledger records and the deletion took the rest. The run had 1,021 such agents, averaging 1.2 min, 20 agent-hours in all. In a sequential block chain they are on the critical path: 1.3 of every 7 minutes of a digest block.

**(f) Ledger commands.** `zt-digest ledger status` now takes 7.7–8.4 s and `ledger gate` 5.1 s. On 11-01, at 4,122 Cards, status took 3.5–3.8 s. The prepared-run anchor check (`Prepared::read`) reads the root and all 230 anchors with text and grows with the archive (S1 of the first proposal, not built). Every drive step runs several such commands.

**(g) A new Term-card plan revision on every zt-digest build.** The Term-card plan records `versions.compiler` = the zt-digest crate version (`term_cards.rs`), and the check artifacts must match it. A rebuild for any reason (0.41.17 → 0.41.18 here) makes a new plan revision. The start then checks all 430 jobs again (6.0 min on revision 4) and gives every publication a new operation key. On revision 3 it also reopened every job and needed the settle step built for it.

### 2.4 Rework

About 8 of the 22.4 workflow hours redid work after a fault:

| Rework | Time | Cause | State |
|---|---|---|---|
| Six model rebuild passes | 2.6 h | false refusals in `model-finish`, `[[TN]]` imports, stale plans, reused batch folders | fixed in 0.41.3–0.41.7 |
| Second whole-book card cycle | 77 min | 32\|a drifted after the rebuild of 31\|a (a borrower) | found by hand; S6 not built |
| Three borrower rebuilds (32\|a, 88\|a, 95\|b) | 60 min | rebuilt models removed parts that others import | S6 not built |
| Term cards, waves and first starts | 1.6 h | lease above 7200 s, waves wait for the slowest job | fixed (v10) |
| Term cards, 120 reopened jobs | 1.7 h | 104 promoted terms had no review; managers ran ids instead of positions; DeepSeek outage | fixed (v11 chunk files, promoted-term review) |
| of which the Term-card publication | 63 min (stopped) | one `-Before` call per repaired row | fixed (0.41.18) |

## 3. Proposals

Each proposal names the script or step it changes, why it helps, and what it is expected to save on a book of this size.

### P1. Lanes for the digest, events and findings layers (largest single saving)

- **Change.** Give `layerRun('digest')`, `layerRun('events')` and `layerRun('findings')` the `lanes` option that `link` already uses. Modules: the digest, events and findings layer modules of `scripts/book-digest/workflow/` (installed as `zt-econ-digest.js` lines 2902, 3566, 3616), plus the math blueprint. The three layers run side by side and share the DSMP limit of 8 managers. So the lanes are a share of the 8 slots, not 8 each: for example digest 4, events 2, findings 2 while the digest runs, and the freed slots go to events and findings when the digest ends.
- **Why it helps.** Each layer had a concurrency of 1.0 while up to 64 DeepSeek conversations were allowed. The work in a block is independent of the other blocks; only the claim and the finish touch the ledger, and the Link lanes already showed that the ledger takes parallel claims (with the anchor-read retry of 0.41.4).
- **Expected effect.** Digest phase from about 212 minutes to 35–45 minutes. Events and findings end shortly after the digest, because they wait for its groups.
- **Risk and test.** The archive serializes writes, and digest writes are short keyed batches. Test: the framework harness with a lanes case for each layer, then a resume of a digest stage on a test-box copy.

### P2. The rebuild takes its borrowers with it (S6), and the whole-book cycle waits for a quiet model stage

- **Change.** (1) Before a model is rebuilt, the model step lists the models that import from it (`**引入**` lines, `get_model`). It rebuilds them in the same pass, or refreshes their imports after it. (2) The whole-book card cycle (mode cards, `cards-book`) starts only when the ledger has no failed, reopened or ready model unit. Today it can start while rebuilds are still pending.
- **Why it helps.** The second card cycle (77 min) and three borrower rebuilds (60 min) came from one cause: a rebuilt model removed a part that another model imported, and the drift was found only at `cards-book`.
- **Expected effect.** About 2 hours on this book. More on any book whose models share blocks.
- **Where.** `tools/zt-digest/src/model` (borrower list), the model layer module (`zt-model-layer`), the Cards stage gate in the base blueprint.

### P3. Scope the whole-book passes to what changed (S4, extended to Term-link)

- **Change.** The Link check, the whole-book card cycle and `term-link` plan only the Cards whose authored text changed since their last verified publication. The archive revision of that publication is known from the ledger and `applied_ops`. A pass with nothing changed is skipped. For `term-link` the input is also the set of Term rows published since its last run.
- **Why it helps.** The 11-01 card cycle sent 253 jobs over 2,129 Cards and applied 4. `term-link` held the archive for about 14 minutes over every Card of the book.
- **Expected effect.** A re-run of these passes after a small change takes minutes, not 68–77 minutes. The first full pass of a book is unchanged.
- **Where.** the card-cycle and Link-check plan steps in `tools/zt-digest` (`cards`, `link` plan), `term_link.rs` plan.

### P4. Make archive renames cheaper, and do the work once

- **Change.** (1) Profile the rename path on a box copy (a `reorder` of one Term list of about 1,500 Cards): `apply_location_mapping_tx`, `Changes::renamed`, `Changes::finish` / `refresh_reverse`, `spread::follow`, the search-index triggers on `cards.text`, and the per-item checks in `batch.rs` `simulate`. Fix the step that scales with the moved Cards times something larger than the moved Cards. (2) Let a keyed client commit a tree-operation batch without a separate preview, when it names the archive revision it planned against. For example `confirm: {revision: N}`: the archive refuses the batch if the revision moved, which is the same guarantee the plan hash gives. One pass instead of two.
- **Why it helps.** Every tree operation pays this cost: Term-card publication (278 s for one reorder), the model fold, settings and restructure passes, span moves, and any later reorganization of the box. About 0.1 s per moved Card makes a 2,000-Card reorganization take minutes inside one lock. Everything else waits for that lock.
- **Expected effect.** The Term-card reorder from 278 s to under 150 s with (2) alone, and further with (1). The size depends on what the profile finds.
- **Where.** zt: `src/main.rs`, `src/link_index.rs`, `src/batch.rs`; zt-digest `term_cards/publish.rs` `send_batch` (the client side of (2)).

### P5. A cheap ledger anchor check (S1, now twice as costly)

- **Change.** As proposed in S1: verify the prepared anchors once per archive revision (keep the last verified revision in the ledger `meta`), or read only `{address, workspace, kind}` instead of 230 text bodies. The record commands of every drive job take the ledger-only path.
- **Why it helps.** `ledger status` grew from 3.7 s to 8 s as the archive grew from 4,122 to 10,850 Cards, and every drive step runs several ledger commands.
- **Expected effect.** Ledger commands under 1 s. Drive steps 10–25 s shorter each, which on 1,021 drive steps is several hours of agent time, and minutes on every sequential block chain.
- **Where.** `tools/zt-digest/src/ledger.rs` `Prepared::read`.

### P6. Fewer drive jobs on the critical path (S2)

- **Change.** (1) Chain the deterministic steps that always follow each other into one step call: a block's `finish` and the next block's `start` in a lane, and the model batch's `derive` → `review` plan, `close` → `split`. (2) A step script's work stays deterministic and in one command; the DeepSeek conversation only runs it. The step prompt already says so, and a step of a few seconds should not need a second model turn.
- **Why it helps.** A drive job costs about 40 s whatever its script does. In a sequential chain of 29 digest blocks, start + finish cost 39 minutes of the 212. Lanes (P1) hide part of this cost, and chaining removes it.
- **Expected effect.** About half of the drive jobs of a block chain removed.
- **Note.** The owner's rule that DeepSeek does the labour stays. This proposal changes how many drive jobs there are, not who runs them.
- **Where.** the step scripts (`layer-steps.ps1`, `model-steps.ps1`) gain combined steps; the layer and model modules call them.

### P7. A Term-card plan revision only when the Term-card contract changes

- **Change.** Replace `versions.compiler` (the crate version) in the Term-card plan with a Term-card contract version: a constant in `term_cards.rs` that is raised only when the renderer, the check, the publication rules or the plan schema change. The checker, rule and instruction versions stay as they are.
- **Why it helps.** A zt-digest rebuild for any other reason (a model fix, a ledger fix) now opens a new plan revision. It checks all 430 jobs again (6 minutes), changes every publication key, and on revision 3 it reopened all jobs. The owner had to hold back zt-digest builds while the branch ran.
- **Expected effect.** No Term-card replanning after unrelated releases, and fixes can be deployed during a run.
- **Where.** `tools/zt-digest/src/term_cards.rs` (`versions()`), its tests.

### P8. Split the one-job writers (S5) and batch the cleanup (S7)

These two proposals from the first document are unchanged and still apply. The Organize skeleton is one DeepSeek job for the whole book (8–11 min). `cleanup: apply` and `settle the run` spent 2.5–3.8 minutes each in single ledger commands, which P5 also shortens.

## 4. Order

1. **P1** (lanes for the digest layers): module options and harness cases only; the largest wall-time saving on a new book.
2. **P2** and **P7**, with **F3** (Section 6) in the same release as P7: prevent the rework and the replanning that cost the most on this book.
3. **P5**: one change in `Prepared::read`; every step of every stage is faster.
4. **P4**: profile first, then fix. It also helps the model passes and every later reorganization of the box.
5. **P3**, then **P6** and **P8**.

Each is a patch release with tests (the framework harness, the step-script tests, the zt-digest and zt unit tests). Each is tested by resuming or by a single-stage mode on a test-box copy, never on a fresh full digest and never on the live archive.

## 5. What is already done

| Item | State |
|---|---|
| Model lanes (8 × 8) | done 2026-11-01 (zt-model-layer v10–v11) |
| Link lanes | done 2026-11-01 (zt-link-pass v10, zt-digest-lib v49) |
| Term cards as one queue, write and review in one DeepSeek job | done 2026-11-02 (zt-term-cards v10) |
| One chunk file per manager | done 2026-11-02 (zt-term-cards v11, zt-cited-cards v3) |
| Term-card publication: append, then one reorder | done 2026-11-02 (zt-digest 0.41.18, 7ab9a7d): 63+ min → 4.7 min |
| S1–S7 of [Digest speed](digest-speed-proposal.md) | not built (P5, P6, P3, P2, P8 here carry them forward) |

## 6. Follow-ups from the 2026-11-02 run (recorded, not started)

The owner recorded these conclusions at the end of the 2026-11-02 run. None of them has been started. F1 and F2 are correctness faults. F3 is a speed fault of the same kind as P7. F4 is a test fault that is not related to this run.

| # | Follow-up | Effect on the run | Relation to this proposal |
|---|---|---|---|
| F1 | The Model Card check must forgive moved Link targets, as the check for other Cards already does. | It still blocks the Card summary on `137\|a`. | none (correctness) |
| F2 | `19\|a` carries six images, and all six have the label "图 2". The figure labels of chapter 2 seem to collapse into one label. | Wrong figure labels on the Model Card. | none (correctness) |
| F3 | Every Cited-cards run normalizes the Cited list again, and this makes a new plan revision each time. | Every Cited-cards run replans, as the Term-card branch did before P7. | extends P7 to the Cited-card plan |
| F4 | The `model_explain_batch` test `a_chain_of_derivation_parts_needs_the_architects_explain_entry` fails (`plan-status` returns 0 faults). | none on the run; the zt-digest test suite is not green. | none (it fails without the 0.41.18 change) |

- **F1. Where to look.** The Model Card check in zt-check, against the forgiveness rule for moved Link targets in the check for other Cards. The fix makes both checks use one rule. A test gives a Model Card a Link to a Card that a tree operation moved, and expects the check to pass.
- **F2. Where to look.** The figures stage of zt-digest (the step that numbers and labels inserted figures) for the chapter-2 figures of `19|a`. A test gives a Model Card several figures from one chapter, and expects a different label for each figure.
- **F3. Where to look.** The Cited-cards plan in zt-digest: the normalization of the Cited list should be stable (the same input gives the same list), or the plan revision should depend only on the Cited-card contract and on the content of the list. A test runs the plan twice on the same box and expects one revision.
- **F4. Where to look.** `tools/zt-digest/tests/model_explain_batch.rs`. First find out whether the test or `plan-status` is wrong.
