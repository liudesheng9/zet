# Model compiler v2: lanes, self-contained builds, one import pass, one job per model — proposal

Status: proposal (2026-11-01), written during the atkinson2015lectures live run at the owner's request. Part D1 (model lanes) is implemented and released (base 5.11.3, econ 6.11.3, math 5.11.3; zt-model-layer v10, zt-model-lib v17), because the owner asked for it during the run. Parts D2–D5 are design only: no code, prompt or blueprint changes yet. The owner will ask for the implementation after this run.

Related: [Shared building blocks between models](model-shared-blocks-proposal.md) (the `**引入**` imports, implemented in #205), [One DeepSeek conversation per model](model-one-conversation-proposal.md) (#194, implemented), [Workflow principles](agents/workflow-principles.md) ("one conversation does a whole job"; "add a self-check tool instead of orchestration").

## 1. Evidence: the atkinson2015lectures run

*Lectures on Public Economics: Updated Edition* (Atkinson & Stiglitz, 563 pages) is the first full textbook the digest has met. Prepare made 229 groups, and the digest wrote **188 Model Cards**. The books before it held 1 to 40.

### 1.1 Time

| Phase | Model stage throughput |
|---|---|
| Resume run (`TAKE = 8`, one block at a time) | one block of 7–8 models in about 45 minutes, about **13 models an hour**; 188 models would have taken 18–20 hours |
| Mode `models` with eight lanes (D1) | 64 models claimed at once, about **50 models an hour** |

Inside a block, the batches already ran in parallel (`elaborate` uses `parallel`). The limit was that a block held one batch. Inside a batch, the time goes to round trips rather than to writing. After the fill, one formal batch passes through up to 12 deterministic steps: derive, two rounds of review and split, up to three close rounds, explain, up to three explain-apply rounds, and report. Each step is one Haiku manager plus one DeepSeek drive job (1–3 minutes), followed by a content round of Haiku managers. Every round waits for the slowest model of the batch before any model moves on.

### 1.2 Failures

At the time of writing, 21 of the 188 models had failed. The causes:

| Models | Cause | Class |
|---|---|---|
| 8 | Batch 95\|c: model 98\|a took a part (`97|a#D4`) that 97\|a in the same batch had not built yet, so its apply failed and left an empty part map. The split round's imports refresh then rebuilt 98\|a's `**引入**` through `resolve()`, which returns a bare plan id it cannot map, and wrote `[[TN]]`. The archive refused the edit, the split step failed, and all 8 models of the batch failed with it. | shared blocks; one model fails its batch |
| 4 | The Sonnet architect wrote `"take": "<book>/48|a"`, a whole Model Card instead of one of its parts. The id mapped to no part, and `model-finish` refused the model ("mapped but belongs to no model") after the whole build. | shared blocks |
| 1 | g146: the 结构 list does not name a taken part. | shared blocks |
| 6 | Batch 1\|b: the DeepSeek drive job copied the fill step's line without its `file` field, and the managers were given the path `undefined`. | drive answer fidelity |
| 1 | g060: the 结构 list does not name one part. | content |
| 1 | g083: unanchored build faults. | content |

Later in the same run, batch 169|a failed the same way (7 models): four models referred to parts of other models in the batch, malformed (`173#P`, without `|a`), their apply failed, and the imports refresh wrote `[[T1]]`. Thirteen of the first 21 come from sharing building blocks between models *while the models are being built*. All 8 of the batch-wide failures come from one model's step fault stopping its whole batch.

### 1.3 Faults found and fixed during the run

| Fault | Fix | Release |
|---|---|---|
| A drive manager added its own remark ("Note for the parent: …") after the step's JSON line; the Cited list failed and Cards block 19 halted the run | `ztDriveLast` leaves prose lines after the line out | zt-workflow-lib v10, base 5.11.2 |
| A zt-check refusal "changing read view" was recorded as a content failure | `cause_code` reads it as `provider_transient` | zt-digest 0.41.2 |
| A drive job dropped `file` from the fill line | a batch step that reports jobs but no file reads the batch's fixed job file name | zt-model-lib v16, base 5.11.3 |
| A `take` of a whole Model Card | the plan check refuses it (`whole_model_take`); the architect doc says a take names a part | zt-digest 0.41.3; model-architect.md |
| One block at a time | model lanes (D1) | zt-model-layer v10, zt-model-lib v17, base 5.11.3 |
| `[[TN]]`: a failed model's take written as a bare plan id | `import_section` drops takes that resolved to no address; `refresh_shared` skips failed models | written, deploys after this run |
| A failed unit cannot be reopened once the run's one resume is spent | `ledger reopen -Units <ids> -Reason <text>` | written, deploys after this run |

## 2. Problems

- **P1. One block at a time.** The model layer claimed eight units per block and ran one block after another. A book with many models spends most of its run in the model stage. *Solved by D1.*
- **P2. Orchestration round trips.** Review, split, close and explain are DeepSeek work, and each model already keeps one conversation (#194). The workflow still drives them round by round: up to 12 step hops per batch, each round waiting for the slowest model.
- **P3. Sharing during the build.** A `take` makes one model depend on another model's part while both are still being built. That creates ordering between models (in-batch references, deferrals, `model-deps` waits), address drift after a split (the `refresh_shared` pass), and new failure classes: whole-model takes, takes of parts that do not exist yet, bare-id links. Lanes make the ordering worse, because a producer may sit in another lane.
- **P4. One model fails its batch.** A step that one model breaks (a refused archive edit) returns `failed` for the whole batch, and every model in it fails.
- **P5. Drive answer fidelity.** DeepSeek drive jobs and Haiku managers retype the step's line, wrap it, add remarks, or drop fields (§1.3). Each case so far has been patched in the reader.

## 3. Design

### D1. Model lanes (implemented)

The model layer runs up to eight lanes side by side. Each lane claims eight units per block, so up to 64 models are built at once, within the DSMP pool (at most 8 Haiku managers in the run).

- `model-steps.ps1 -Lane <name>` keeps a lane's block files apart (`units.<lane>.json`, `models.<lane>.json`, `batches.<lane>.json`, `models-all.<lane>.json`); a batch's own files already carry its batch folder.
- Only the stage's first `start` releases a stopped run's claims; every other `start` passes `-KeepClaims`.
- A lane ends when it finds no ready unit. A last single lane then takes what the others released late, such as a model that waited for a producer in another lane.
- A `read_view_changed` on a model completion, caused by lanes committing beside each other, is retried, never content.
- Tests: framework-harness (eight lanes; only the first start releases; every step names its lane), model-steps.tests.ps1 (`-Lane`, `-KeepClaims`).

### D2. Self-contained builds: no sharing while models are built

During the model stage, the architect plans **no `take`**. Each model is built from its own parts:

- A premise that another model of the book also states is written in this model as its own setting part. The part carries a marker naming the part it duplicates (`待引入 [[<address>]]`). This uses a part's existing fields; it is no new Card type.
- A premise this model only mentions stays a Link in prose (`参照 [[…]]`).
- The plan check (`validate`, `plan-status`) refuses a `take` in the build. A `<model>#<id>` reference between models of one batch is refused too.

Effect: no model waits on another one. Lanes, batches and models become independent, and `model-deps`, `defer`, in-batch references and the imports refresh after a split all leave the build path. Every class in P3 disappears from the build. The rule "never restate a part of a model" becomes "never restate one in the finished book", and D3 enforces it.

### D3. One import pass at the end of the model stage

When every lane has finished, one import pass turns the marked restatements into imports. It applies the rules of the `zt-model-import` skill, which the owner now runs by hand after the model passes.

1. **Candidates (script):** every part with a `待引入` marker, plus the parts whose statement fingerprint equals a part of another model (the archive's `restatement` role). A candidate names the borrowing model, its part and the target part.
2. **Decision (one DeepSeek job per borrowing model, at most eight per manager):** the job reads both parts and the borrowing model's chain, and answers *import* (same statement, same symbols or a stated symbol mapping) or *keep* (the restatement differs: a variant, another assumption). It runs its own SELF-CHECK.
3. **Apply (script):** for *import*, it writes the `**引入**` line (`单卡`, or `整块` when the job says so), removes the restated part with `delete_card into`, so Links re-point to the target, and rebuilds the 结构 list. A *keep* removes the marker.
4. **Check:** every `**引入**` target is a faceted part (never a Model Card), every Link still resolves, and `model-finish` passes for every changed model. A changed model's explanations are refreshed, as the import step 8 of the model passes already does.

Every import target exists when the pass runs, so the address and ordering faults of P3 cannot occur. A failed import leaves the model self-contained and complete. It is a lost deduplication, never a lost model.

### D4. One job per model after the plan (the model loop)

After brief, plan (Sonnet) and fill, the workflow starts **one DeepSeek job per model**, continuing the model's conversation (#194). The job runs the remaining work as its own loop, against a per-model check tool:

```
derive → check → (fix | split | close fault | explain) → check → … until the check is clean or the round cap is reached
```

- **Check tool:** `zt-digest model -Step check -Work <batch> -Model <address>` runs the review checks, the split need, the close faults and the explain gaps for one model. It prints one short line with what is still open and the files holding the details. The job runs it as its SELF-CHECK, as the card-cycle and Term-card jobs already do.
- **Apply tool:** `zt-digest model -Step apply-one -Work <batch> -Model <address>` applies that model's drafts, including a split. It writes only that model's Cards.
- **Per-model state:** the batch's `state.json` is shared today. D4 needs per-model state files, or one file lock around each compiler write. The per-model file is simpler and needs no lock.
- **Allow list:** the manager approves only the check and apply commands for the job's own model, plus reads under its folder.
- **Round cap:** at most six check rounds inside the job, then it answers with the last check line. The workflow then runs `model-finish` (`finish-batch`) as today. The deterministic gate stays, so an unfinished loop cannot pass a bad model.
- **Review independence:** econ already reviews in the model's own conversation. The math variant's independent reviewer (`-IndependentReview`, #194) stays as one extra job after the loop.

The workflow's batch path becomes: start → brief → plan → fill → **model loop** → finish. That is four content rounds and no step rounds between them; each model moves at its own pace.

### D5. Containment: one model never fails its batch

A compiler step that one model breaks records that model as failed in the batch state and goes on with the others. This covers a refused archive edit, a part map that cannot resolve, and an apply error. Only a fault that concerns the whole batch (the archive unreachable, the owner's Lock) stops the step. D4 makes most of this structural, because each model has its own apply; D5 is the rule for the steps that stay batch-wide (brief, fill).

### D6. Drive answers read from the file the step wrote (P5)

Every step script already writes its line to `drive-lines/<name>.json` (`line`) before it prints it. The drive reader should take the step's line from that file, with the DeepSeek answer only as the pointer (`detail`), instead of trusting a retyped copy. Two changes are needed. The detail file name must be unique per call; today lanes and batches overwrite `fill.json`, so a run-unique suffix is required. And the Haiku manager returns the file's `line` verbatim (it can read files), or the module reads it through one small read step. This ends the patch series of §1.3 (fences, curly quotes, remarks, dropped fields).

### D7. Where a rebuild spends its time, and three quick cuts

Measured on the rebuild run of 2026-11-01 (wf_7ba0b0ff-ecb, 2 h 51 min, 46 models): Models 43 min, Link 34 min, **link check 68 min**, Organize 16 min, Cleanup 8 min. In the model stage, the Sonnet plan is the slowest step (9.3 min on average per batch of 7–8 models, up to 17 min); each content round takes 1.4–3.2 min, and each of about 12 drive steps per batch about 0.5 min.

1. **Scope the link check to what changed.** It re-checked the whole book (2,129 Cards, 229 jobs in about 4 waves of 8 managers) although only the rebuilt models and their groups had changed. The check should plan only the Cards changed since its last publication; a run that changes nothing skips it.
2. **Delete conversations in parallel.** One manager deleted the 229 link-check conversations one after another (7 min); the deletion splits into up to eight managers, as the content rounds do.
3. **Smaller architect batches when models wait.** `perSonnet: 4` halves a plan call and lets more batches run side by side; the default stays 8 for small books, where the architect needs the models of one batch together.

## 4. What changes where

| Area | D2 | D3 | D4 | D5 | D6 |
|---|---|---|---|---|---|
| `tools/zt-digest` model compiler | refuse takes in build; `待引入` marker field | candidate finder, apply, check | `-Step check`, `-Step apply-one`, per-model state | per-model failure in batch steps | unique detail file names |
| `scripts/book-digest/model-steps.ps1` | — | `import-plan`, `import-apply` steps | `check`, `apply-one` steps | — | — |
| `zt-model-lib` / `zt-model-layer` | — | import pass after the lanes | the model loop job replaces derive/review/split/close/explain rounds | — | read lines from the detail file |
| Prompts | `model-architect.md`: no take; restate with `待引入` | new `dsh-model-import-instructions.zh.md` | new `dsh-model-loop-instructions.zh.md` | — | drive manager prompt |
| Tests | plan-check unit tests | harness: import pass; unit tests | harness: loop job and round cap; model-steps tests | unit test: one model's refused edit leaves the batch | harness: a retyped line is read from the file |
| Wiki | models-and-math, digest-prompts | digest-workflows, models-and-math | digest-workflows, zt-digest | zt-digest | workflow-module-structure |

Every part is a blueprint bump. D2+D3 change the stage's shape, so they take a minor bump (5.12.0 / 6.12.0 / 5.12.0) and a zt-digest minor release.

## 5. Rollout

1. **Finish atkinson2015lectures on the current design.** Deploy the `[[TN]]` fix and `ledger reopen`, reopen all failed models, rebuild them with `only` + `rebuild`, then run the remaining modes and add the small `finalize` mode.
2. **D5 and D6 first.** They are small, they help every later run, and they need no new prompts.
3. **D2 + D3 together.** D2 without D3 would leave duplicates in the finished book. Pilot on a test-box copy of a book with shared settings (hart1995firms or acemoglu2025nobel), never on the live box, and compare its imports with the hand-made model passes on the live box.
4. **D4 last.** It is the largest change. Pilot it on the same test box. Measure batch time and failures against the D1 baseline (§1.1, §1.2).
5. Test by resuming or single-stage modes on the pilot staging, never a fresh full digest, per the workflow principles.

## 6. Acceptance

- D1: a book with more than 64 models runs eight lanes; no lane releases another lane's claim; `finish-batch` never holds more than eight units. *(Met on this run.)*
- D2: a build plan with a `take` is refused; no model of the pilot defers or waits.
- D3: every `待引入` marker is resolved to an import or kept with a reason; every `**引入**` target is a faceted part; `model-finish` passes for every changed model; a second import plan changes nothing.
- D4: no batch runs a review, split or close step round; each model's loop ends clean or at its cap; time per formal batch is at most half the D1 baseline.
- D5: a model whose edit the archive refuses fails alone; the other models of its batch finish.
- D6: a drive answer that drops or retypes a field still gives the step's exact line.

## 7. Decisions for the owner

1. **Restate-then-import (D2+D3) or take-only-finished (a smaller step).** The smaller alternative keeps takes, but allows them only of parts of models already done in an earlier block, and refuses takes in the same batch. It removes the in-batch class but not the ordering between lanes. Recommendation: D2+D3.
2. **The marker.** `待引入 [[<address>]]` in the part body, or a structured field in the plan only. Recommendation: the plan field, so no Card text carries a workflow marker after the pass.
3. **The loop's round cap (D4).** Six check rounds, or the current totals (2 review + 3 close + 3 explain). Recommendation: six.
