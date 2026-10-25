# Term Cards, Term Links and the Card check: S32 historical Term backfill (issue #174)

This is the runbook and status record for **S32 / #174**, the last owner-led gate of
[PRD #142](https://github.com/liudesheng9/zet/issues/142). It covers design section 16
(R0479, R0486) and acceptance criteria S32-AC01–S32-AC07.

Source binding: the [implementation design](term-cards-links-and-check-gate-implementation-design.md),
section 16 (SHA-256 `a27224263597460d0822d7859cd1a822d47e7376bb207686689843fc6d3309d4`).
The support was built by S25 (#168); this slice applies it to **one** owner-selected book.

## 1. What is delivered, and what stays with the owner

**Delivered and verified.** The one-book Term backfill support that S32 applies:
`zt-digest prepare -Out <staging> -Validate|-Sections` (`tools/zt-digest/src/backfill.rs`, the
read-only inspection/reconstruction branch) and the two dedicated Workflow modes `term-cards`
and `term-link` (`scripts/book-digest/workflow/`, the `termcards/*` and `termlink/*` ledger
units). This record verifies that support with its own tests and names the exact owner
procedure, the evidence paths and the two owner records.

**Not claimed.** No book was backfilled. No live Card, Term link, Lock or archive row was
written; no digest, workflow or provider run happened; no tree was unlocked or relocked. S32
is an owner gate, and it cannot start yet: the owner has not selected a book and authorized
the run, and no selected book currently has a compatible prepared run (see §3). The issue
stays **OPEN** until the owner records the selection, the run is qualified, and the owner
records the relock disposition.

## 2. The support under test (2026-10-25)

| Seam | Evidence | Result |
|---|---|---|
| `tests/term_backfill.rs` (11 tests) | real prepared runs: list/source/fingerprint inspection, byte-identical reuse and reconstruction, the Lock refusal, an occupied slot, duplicate rows, a missing or changed source, a staging without a contract, Term-unit registration | **11 passed, 0 failed** (6.00 s) |
| `cargo test --bin zt-digest backfill::` (5 tests) | the Workspace refusal, the list's Side children, an empty slot list, published heads in table order, an occupied slot | **5 passed, 0 failed** |
| The workflow modes | `framework-harness.mjs`'s S24/S25 mode and ledger checks, `term-cards-harness.mjs` (S29 qualification, §`R0545`/`R0549`) | the modes start only their own stages, never a digest/model/prose writer |
| Installed set (this tree) | `zt` 0.40.0 (SHA-256 `4171EE6C…F67B`), `zt-check` 0.14.0 (`355D321B…E24E`), `zt-digest` 0.29.0 (`2249AEAB…D32DE9`), frozen Workflow base 5.6.0 / econ 6.6.0 / math 5.6.0 | matches `docs/term-cards-links-and-check-gate-release-manifest.json` |
| Running service | `zt status`: `state: up`, pid 44236, version 0.40.0, archive `D:\zt`, revision 7921, 1952 Cards, 5 Locks `integrity ok`, 0 Sessions, GUI `http://127.0.0.1:4717/` | the running daemon, not a copied binary, is 0.40.0; S31 recorded the gate at `advisory` and applied no promotion |

The earlier slices' evidence for the pieces S32 composes: `tests/term_cards_apply.rs`,
`tests/term_link_apply.rs`, `tests/term_pilot.rs` and the S25/S26 workflow checks (see
[the pilot runbook](term-cards-links-and-check-gate-pilot.md) and
[the qualification record](term-cards-links-and-check-gate-qualification.md)).

## 3. Why the run cannot start yet

S32-AC01 needs the owner's selection; S32-AC02 needs an unlocked tree; and the backfill needs
a prepared run whose frozen contract and archive identity match the running set.

**No compatible prepared run exists.** `zt-digest` 0.29.0 embeds base 5.6.0 / variant 6.6.0,
and a prepared run is refused (`Prepared::read`: `incompatible workflow contract: base/variant
versions differ`) unless it froze those versions. Every retained live-book staging is older:

| Staging (`D:\zt-digest\…`) | Citation key | Base / variant |
|---|---|---|
| `acemoglu2007equilibrium` | `acemoglu2007equilibrium` | 5.4.0 / 6.4.0 |
| `acemoglu2010labor` | `acemoglu2010labor` | 5.4.0 / 6.4.0 |
| `acemoglu2010labor.trial1` | `acemoglu2010labor` | 5.4.0 / 6.4.0 |
| `acemoglu2025nobel` | `acemoglu2025nobel` | 5.1.0 / 6.1.0 |
| `ebrill2001modern` | `ebrill2001modern` | 5.1.1 / 6.1.1 |
| `grossman2002integration-test` | `grossman2002integration` | 4.10.0 / 5.10.0 |

The run ledger is bound to the archive identity it was prepared against (every guarded step
rechecks `archive_identity`), so the S30 copy-pilot's prepared run cannot drive the live
archive either. The issue explicitly does not authorize a new full digest as a workaround
(for a prepared run or for a conflict). A selected live book therefore needs a compatible
prepared run first — the owner's decision, through the S30 pilot's compatible run adoption or
another reviewed conversion.

**The copy-based pilot comes first.** The design's backfill decision (R0058) allows one selected
book **after** the copied-book pilot. S30 (#172) is deferred by the owner and its run needs an
owner-started full digest; #172 stays open. S31 (#173) qualified and deployed the live release;
its enforcement promotion is deferred, and the gate is still `advisory` (a Term backfill is
written under `advisory`, so this is not a blocker for the run itself, only for promotion).

**The tree is locked.** The live `grossman2002integration` tree is under Lock 3. Any selected
tree must be unlocked by the owner through the existing GUI page; the workflow and these
commands never unlock a tree (S32-AC02, R0481).

## 4. The selection record (S32-AC01)

The owner writes one record before any work and keeps it with the evidence, for example
`D:\zt-target\backfill-174\selection.json`. The agent does not invent it, and it authorizes
only the named book and scope:

```json
{
  "issue": 174,
  "slice": "S32",
  "owner": "liudesheng9",
  "recorded": "2026-10-25T15:49:46-05:00",
  "mode": "backfill",
  "book": "<the exact Citation key>",
  "workspace": "agent",
  "tree": "<the selected Literature tree address>",
  "list": "<the one Term list address>",
  "staging": "<the prepared run folder with its compatible manifest.json>",
  "scope": ["term-cards", "term-link"],
  "allowed_writes": [
    "the existing list's type/name metadata as required",
    "new Term heads and their continuation parts in the selected tree",
    "Term-link body edits in the selected book"
  ],
  "not_authorized": [
    "all-book rollout",
    "a new full digest",
    "model-manager reruns, notation/model-table or prose edits, avoid-word fixes",
    "a workflow unlock or relock"
  ],
  "live": { "archive_id": "<32 hex>", "archive_root": "D:\\zt", "revision": 7921 }
}
```

This keeps pilot approval from becoming blanket backfill authority: a different book, tree,
list, staging or revision is a different selection and needs its own record.

## 5. Procedure

All commands run from `E:\zet` against the running service and the **live** archive. The
owner or Claude Code starts the two workflow runs; the read-only checks below can run before
them.

### 5.1 Inspect before any work (S32-AC02)

1. Read `zt status` and record the archive id, revision, Card count and the `locks:` line;
   run `zt status --check-locks` and keep its `integrity:` line and the named Lock roots.
2. Run the inspection branch on the prepared run:

   ```powershell
   zt-digest prepare -Out "<staging>" -Validate -Brief
   ```

   It writes `<staging>\backfill.json` and `manifest.backfill` and **no Card**. It reports the
   Workspace, the one Term list (address, title, kind, rows, terms, authored title/body
   SHA-256), the tree's card/head/part counts and occupying addresses, the staged source
   slices and their fingerprints, the contract versions and the archive revision.
   - Exit 0: the book, list, occupancy, source and Lock are consistent; keep the record.
   - Exit 1, `backfill_conflict`: a typed conflict (`human_workspace`, `no_tree`, `no_list`,
     `ambiguous_list`, `duplicate_rows`, `invalid_rows`, `occupying_children`,
     `missing_source`, `source_changed`, `missing_sections`, `staged_source_mismatch`,
     `no_ledger`, `not_prepared`). Stop; show the conflicting children/rows/source passages
     and the incomplete generation. Never use a fresh digest to work around it.
   - Exit 2, `card_locked`: the owner's Lock. Stop and name the tree/Lock; only the owner
     unlocks through the GUI.
3. Record the inspection result with the selection.

### 5.2 Rebuild source sections only if needed (S32-AC03)

Run the same command with `-Sections` (the `term-cards` mode does this itself). It reuses the
staged `sections\sNN.md` and `book.zh.md` when they reproduce the source; otherwise it
reconstructs them from the frozen source under the contract's own options and staged
`figures.json`. A reconstruction is written only when it reproduces the recorded section and
book hashes byte for byte; a changed or unreconstructable source is a conflict, never a silent
staging rewrite.

### 5.3 Term Cards (S32-AC03, S32-AC05)

Start the project-scope frozen `zt-econ-digest` workflow (base 5.6.0, econ 6.6.0) in mode
`term-cards` on the same staging, with the Term settings:

```js
{ out: "<staging>", termCards: true, termLink: true,
  termCardsPerJob: 4, termCardsMathPerJob: 1, termLinkOccurrences: "first" }
```

The mode runs the backfill preparation, the Term-card branch (compiler plan, waves of writers,
checks, independent mathematical reviews, one finite fix, publication of the longest ready
ordered prefix) and the deterministic Term-link pass. It starts no digest, Card-cycle, model,
Link, Organize, Figure, Bridge or finalization stage. It writes only the allowed set: list
type/name metadata if the list is a legacy `术语表`, new Term heads as `side` successors and
continuation parts as `direct` successors, plus Term links. It records, per row: the writer
job, draft/render/check hashes, the check's status, the review, the fix, the publication
status, the head/part addresses and receipt keys, and the batches with their receipts
(`<staging>\term-cards\state.json`, `report.json`, `report.md`). A partial prefix is committed
work that leaves the book unit incomplete; it is **not** complete.

### 5.4 Term links (S32-AC03, S32-AC05)

The mode's `term-link` stage claims `termlink/book@1`, freezes the plan
(`<staging>\term-link\plan.json`, `preview.md`), applies it in identified chunks with receipts
(`apply.json`, `apply.md`) and completes the unit only from the apply report. Every changed
body must satisfy the preservation equation: strip the new Term macros and the bytes are the
prior body. A Lock stops with the typed `card_locked` (exit 2), a stale plan writes nothing,
and a Card already at its planned output replays its receipt.

Then run the pass a **second time** (mode `term-link` alone) and retain both plans: the second
plan must have `cards_changed: 0` and `cards_refused: 0`, the apply must be a proven no-op
with no new revision. That is the idempotent-rerun evidence (S32-AC05).

### 5.5 Read-only audit and final readback (S32-AC04, S32-AC05)

The mode's audit stage is read-only and always runs. It writes `audit.json`, `audit.md` and
`audit-faults.txt` and publishes the book's Term quality (rows/published/missing,
heads/parts/unfiled, math/concept counts, `T*` faults by rule, uses by Card type, heads and
substantive Cards with no use, avoid words, top terms, mapping fingerprints, run generation,
and a complete/degraded/refused/skipped status). Read it together with:

- every published head/part, read back with its type, Term name and role;
- the list's authored title and body SHA-256, unchanged from the inspection record (a legacy
  list may be retyped, never rewritten);
- the changed-body preservation equation for every Card the link pass edited;
- the revision before and after, and each chunk's receipt;
- the run report's own usage, timing and cleanup facts (never a price estimate).

Any unverified row, refused chunk or unread artifact is named; an incomplete prefix stays
incomplete, and the report never reads as complete coverage.

### 5.6 Interruption and resume (S32-AC06)

If a run is interrupted, read the frozen plan and the committed receipts first, then rerun the
same mode on the same staging: the ledger and the operation keys replay committed chunks, so
no second head is created. Verify the effects against the plan before retrying. Stop on a
stale plan, Lock, Access refusal or semantic conflict; never restore a backup or delete a Card
to make the retry convenient. Report committed and pending chunks explicitly.

### 5.7 The owner's review and relock (S32-AC04, S32-AC07)

The owner reviews the selected book in the GUI (the list and its published heads/parts, the
changed Cards and their Term links) and the audit. The owner then decides the Lock disposition:
relock the tree through the existing GUI page (or leave it unlocked, explicitly). Verify the
outcome with `zt status` (Lock integrity) and a fresh read-only Card check. Record the decision
with the evidence:

```json
{
  "issue": 174,
  "decision": "approved|rejected",
  "book": "<the Citation key>",
  "reviewed": { "terms": ["<address>"], "cards": ["<address>"] },
  "corrections": [{ "address": "<address>", "what": "<named correction>" }],
  "unresolved": ["<named work>"],
  "relock": "requested|not_requested",
  "lock_after": { "tree": "<address>", "locked": true },
  "applies": []
}
```

An approval names what was read; a rejection names the unresolved work. The decision applies
nothing itself and authorizes no further book: each later book needs its own selection.

## 6. Acceptance criteria map

| Criterion | State |
|---|---|
| **S32-AC01** exact selected book and scope recorded; no all-book rollout or new full digest | **Owner-pending**: the record format is §4; no selection exists. No code change authorizes an automatic migration |
| **S32-AC02** inspect source availability, list identity, duplicate rows, existing descendants and Locks; the owner alone unlocks/relocks | **Support verified** (§2 `term_backfill` tests cover every inspection fact and the Lock refusal); **live run not started** |
| **S32-AC03** rebuild sections only if needed; generate/check/review/publish ordered terms; deterministic Term-link edits in the selected tree | **Support verified** (S25/S26 tests and workflow modes); **not executed on the live archive** |
| **S32-AC04** writes limited to list metadata, new heads/parts and same-book Term-link edits; prior list text, model/notation tables, prose and unrelated Cards unchanged | **Support verified** by the publication and preservation tests; the live preservation result needs the run |
| **S32-AC05** every head/part/check/review/receipt, preservation result, idempotent rerun and read-only audit recorded; incomplete prefixes stay incomplete | **Support verified** by the term-card/term-link reports and the S30 qualification; the live record needs the run |
| **S32-AC06** interruption uses the same frozen plan/receipts, verifies effects, stops on stale/Lock/Access/conflict, never restores/deletes blindly | **Support verified** by the apply/replay and typed-stop tests; the live behavior needs the run |
| **S32-AC07** owner reviews the final result and records whether to relock; further books need separate selection | **Owner-pending**: §5.7 |

## 7. Limits

- The unit and integration tests prove the command and artifact shapes, the inspection
  conflicts, the preservation equation and the replay paths on scratch archives and real
  prepared runs. They do not prove the selected book's content, the owner's judgment, or a
  live provider run.
- No live-book prepared run is currently compatible with the installed set (§3). Until one is,
  the S32 run cannot start even with the owner's selection and unlock.
- `#172` (the copied-book pilot, deferred by the owner) and `#173` (the live release, with the
  enforcement promotion still owner-gated) remain open; this slice neither implements nor
  closes them.
