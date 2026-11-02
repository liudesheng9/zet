# Digest speed: where the scripts lose time, and the card-box operations that can replace it — proposal

Status: proposal (2026-11-01), written during the atkinson2015lectures run at the owner's request. No code changes with this document. The owner decides the order; the design work on the model compiler itself is in [Model compiler v2](model-compiler-v2-proposal.md).

## 1. Evidence

*Lectures on Public Economics* (229 groups, 188 Model Cards, 4,122 Cards) is the first textbook-sized digest. Its runs were timed from the agent transcripts and from direct measurements of the step tools.

### 1.1 Per command: the prepared-run check costs 3.3 s

| Command (measured on the live box, 2026-11-01) | Time |
|---|---|
| `zt-digest version` (no archive) | 0.03 s |
| `zt status` (service check) | 0.06 s |
| `zt-digest ledger reclaim` (ledger only: `read_mode` without Cards) | 0.36 s |
| `zt-digest ledger status`, `ledger gate`, `list-models` | 3.5–3.8 s |
| `pwsh -NoProfile` startup | 0.4 s |

Every ledger command that reads the prepared run with Cards (`Prepared::read`) re-reads the Literature root and every group anchor through the archive (`zt.cards` of 230 addresses, text view, up to five times while the revision moves) to prove that no anchor moved. That check is about 3.3 s of every such command, and it grows with the group count. A model or layer step runs several such commands (status, reclaim, list, next, gate, finish), and every drive job adds three `ledger resource` record commands (register, evidence, settle).

### 1.2 Per step: a drive job costs 0.5–1 minute

Each deterministic step is one Haiku manager that loads its tools, starts one DeepSeek drive conversation, waits for it (the poll interval is 90 s), records it three times and deletes it. The script inside often runs for a few seconds. Measured on the rebuild run (wf_7ba0b0ff-ecb): `find models` 0.7 min, `brief`/`fill`/`derive`/`review`/`close`/`split` steps 0.4–0.6 min each, Link `start` 0.7 min and `finish` 1.3 min. One formal model batch passes about 12 such steps.

### 1.3 Per stage: whole-book cycles and one-job writers

| Stage (rebuild run) | Wall time | Cause |
|---|---|---|
| Link check | 68 min | the whole book (2,129 Cards, 229 jobs) although few Cards changed; then one manager deleted 229 conversations in turn (7 min) |
| Whole-book card cycle (mode cards) | 78 min | the same shape: 253 jobs over 2,129 Cards; 4 applied |
| Organize skeleton | 11 min | one DeepSeek job writes the index of all 229 groups |
| Cleanup | 8 min | `cleanup: apply` 3.8 min and `settle the run` 3.7 min, both made of many ledger commands |

### 1.4 Writes: two paths, one of them untraced

The digest and Link applies write through keyed `apply_batch` chunks: 4,249 operations, about 70 ms apart inside one command (90th percentile 230 ms), recoverable and traced in `applied_ops`. The model compiler, the card cycle and the imports refresh write Card by Card through `calls::run` without operation keys. Those writes are not in `applied_ops`, so the drift of `32|a` (its Card changed after its completion; `cards-book/book@2` refused the book) left no trace of which writer changed it; it took a re-run of `model-finish` to find that the rebuild of `31|a` had removed a part `32|a` imports.

## 2. The card-box operations available now

Since zt 0.50–0.52 (PRD #228) the archive offers operations the scripts do not use yet:

- **`apply_batch`** with tree operations: atomic, keyed, previewable batches of create/edit/move/insert/delete, recovered by repeating the same input.
- **Span tools** `move_span` and `delete_span`: a whole subtree moved or deleted in one operation.
- **`get_cards`** (up to 200 addresses, `full` or `text`) and **`export_titles`** (titles only, one pass).
- **`get_model`**, **`list_links`**, **`search_cards` `mode: entry`**.

## 3. Proposals

### S1. A cheap anchor check (largest saving per command)

The anchor check needs to know that each prepared address still holds an agent Card of the right kind; it does not need the bodies. Two ways, in order of preference:

1. **Read the check view once per archive revision.** The ledger keeps the last verified archive revision in `meta`; a command whose observed revision equals it skips the 230-Card read. A moved revision is re-checked, so correctness does not change.
2. **A lighter read.** Ask the archive for addresses, workspace and kind only (`export_titles` already returns one line per Card; or a small `locate` tool returning `{address, workspace, kind}` for a list), instead of 230 text bodies.

Expected effect: a ledger command from 3.7 s to 0.4–0.8 s. The three record commands of every drive job need no Cards at all and should take the ledger-only path (`read_mode` without Cards), as `reclaim` already does.

### S2. Fewer drive jobs per batch

- **Chain the deterministic steps that always follow each other** into one step call: a model batch's `derive` → `review` plan, a layer's `finish` → next `start`, the explain plan → apply. One drive job instead of two or three.
- **Shorter waits for drive jobs**: a drive job runs a script of seconds; its manager should wait in short polls (10–20 s), not the 90 s content-job poll.
- The per-model loop job of [Model compiler v2](model-compiler-v2-proposal.md) D4 removes most model steps altogether.

### S3. One keyed write path

Move the model compiler, the card cycle, the imports refresh and the cleanup of an old build onto keyed `apply_batch` chunks:

- every Card change is in `applied_ops` with its writer's key, so a drift like `32|a` names its writer;
- a batch is atomic and recoverable by repeating it;
- the old build of a rebuilt model is removed with one `delete_span` (with `into` for its Links) instead of leaving parts that the next finish refuses ("holds a part its plan does not map");
- a move of a block uses `move_span` instead of a sequence of single moves.

### S4. Scope the whole-book cycles to what changed

The link check and the whole-book card cycle plan only the Cards changed since their last publication (the archive revision of that publication is known), and the deletion of their conversations runs in up to eight managers. A run that changed nothing skips them. (Same as [Model compiler v2](model-compiler-v2-proposal.md) D7.)

### S5. Split the one-job writers

The Organize skeleton index is written per part of the book (one job per lecture or per 30 groups), then joined by a script. The same rule applies to any single DeepSeek job whose input grows with the book.

### S6. A rebuild takes its borrowers with it

Before a model is rebuilt, the step lists the models that import from it (`get_model` / the `**引入**` lines) and either rebuilds them in the same batch or refreshes their imports after it, so no borrower is left pointing at a part the rebuild removed (`32|a` → `31|a|a|9`). With [Model compiler v2](model-compiler-v2-proposal.md) D2 + D3 (no sharing during the build, one import pass at the end) the case disappears.

### S7. Batch the cleanup

`cleanup: apply` and `settle the run` spend most of their 7.5 minutes in single ledger commands. One `ledger resource -SettleBatch <file>` command settles every recorded conversation from one file, and with S1 each remaining command is under a second.

## 4. Order

1. **S1** (one change in `Prepared::read`, one in the record commands): every step of every stage gets faster; small and testable.
2. **S4** and **S7**: the two largest single stages of a rebuild.
3. **S3** with **S6**: correctness and traceability first, speed second.
4. **S2** and **S5**, then Model compiler v2 D4.

Each is a patch release with tests (the harness, the step-script tests, the zt-digest unit tests), tested by resuming or a single-stage mode on a test-box copy, never on a fresh full digest.
