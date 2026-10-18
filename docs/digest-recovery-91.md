# Stiglitz backlog recovery (#91, DG27)

This is the recovery package for [#91](https://github.com/liudesheng9/zet/issues/91): reconcile the
current archive/run evidence and finish only the actual unresolved Stiglitz scope. It is the last
slice of [PRD #64](https://github.com/liudesheng9/zet/issues/64), after [#88](https://github.com/liudesheng9/zet/issues/88)
(legacy staging adoption) and [#90](https://github.com/liudesheng9/zet/issues/90) (deployment and
the bounded pilot package).

Status: **prepared and rehearsed; the authorized live window is pending.** This pass performed and
recorded everything that does not need provider spend or a live Card write: the consistent
backups, the exact recount on a copy of the settled archive, the adoption of the real legacy
staging evidence, the one-resume rehearsal, the final-report rehearsal, and two defects the recount
found (fixed test-first). Applying the adoption to `D:\zt-digest\stiglitz2014learning` and running
the one `mode: "resume"` with real provider work is the human's authorized step (section 6).

Nothing here is a completion claim. The dated numbers of the proposal (a fourth run in progress,
137 placeholders, 15 Cards/18 repeated paragraphs, 31 title bodies, 65 note suggestions, 13 named
figures and so on) are **historical observations**; section 2 recounts every category on the current
archive and replaces them.

## 1. Settled state, authorization and backups

- **No digest was active.** `zt-digest service-wait -Wait 5` answered `state: up`, `stage: null`,
  `last_exit_abnormal: false`; no `zt-digest` or `zt-check` process ran; the legacy folder holds no
  `ledger.sqlite3`, no `manifest.json` and no `zt.digest.run` registration. Nothing was stopped
  implicitly.
- **Consistent archive backup.** `D:\zt\backup\zt.sqlite3.20261018-174810.pre-issue91.bak`
  (`VACUUM INTO`, 33,366,016 bytes, `PRAGMA integrity_check` `ok`, 10,895 Cards, 56,994 Link
  occurrences, `archive_id` `806009944a966daad513b2aacabddc5f`, revision 0).
- **Consistent staging backup.** `D:\zt\backup\staging-stiglitz2014learning.20261018-174810.pre-issue91.zip`
  (4,275,893 bytes; `book.zh.md`, `sections/`, `groups/`, `terms.md`, `figures.json`,
  `bridge/report.md`, `bridge/proposals.json`, `bridge/plan.json`).
- **Installed binaries.** `zt.exe` `751727E77F27F68466018F7E0003773363FFBA49F3B0EBE4A9BAF2DE831624E9`
  (`zt 0.20.0`), `zt-boot.exe` `BCE8F896…`, `zt-check.exe` `36D0FD61…`, `zt-digest.exe`
  `2102296551C0CE64B941DE51F120A1DB417296D3A2C1493AFAE01E9E2DCBD90E` (`zt-digest 0.1.0`, the
  build of this slice's fix). `zt` is byte-identical to a fresh `--release --locked` build of the
  current HEAD; the previous `zt-digest.exe` is retained as
  `D:\zt-target\rollback\zt-digest-pre-issue91-20261018-174802.exe` (`CB840CCE…`). The service is
  up on `D:\zt` with revision 0, 10,895 Cards, no Sessions, `zt status --check-links` ok with
  56,994 occurrences and zero faults, the GUI root 200 and a tokenless `/api/ping` refused (403).
- **Read-only evidence copy.** Every command of this pass ran on a copy: the archive through one
  `VACUUM INTO`, the staging folder through a file copy. Section 5's rehearsal asserts that the
  live staging folder is byte-identical after it and that the live archive was never written.

## 2. The exact recount (R418)

Commands, both read-only, both on the consistent copy at revision 0:

```powershell
zt-digest ledger adopt -Out <staging copy> -Variant econ       # groups, models, retained jobs, Bridge
zt-digest audit -CitationKey stiglitz2014learning -Out <audit copy>   # card-level categories
```

| Backlog category (proposal) | Current recount (archive revision 0) | Where it is now |
| --- | --- | --- |
| 15 Cards/18 repeated Link paragraphs | **14 repeated prose paragraphs (R18)** on 14 Cards; the Link table check reports 56,994 occurrences with zero broken Links | `audit.json` `style.by_rule.R18`, `audit-faults.txt`; the exact paragraphs are priced by the Cards/whole-book cycle |
| 31 repeated title bodies; five `向向`/`价价` cases | **32 repeated Card titles (R19)**; no `向向` or `价价` text appears in any of the 1,448 book Cards any more, so those five named cases are absent rather than silently fixed | `audit.json` `style.by_rule.R19`; the five cases stay a reviewed disposition (absent) |
| Groups 92–95 and 168–170 | **11 incomplete groups: 10, 18, 92, 93, 94, 95, 127, 163, 168, 169, 170**; 160 verified groups. 92, 93, 95, 168, 170 keep a skeleton anchor with no substantive Card; 10, 18, 94, 127, 163, 169 hold no substantive Card for another reason | `ledger adopt` plan `groups[]`; the 11 are ready `digest/gNNN@1` units |
| Models reported unbuilt/placeholder-filled | **170 Model Cards: 54 verified built, 116 incomplete**; **42 still hold `待写`**; **96 carry 244 build faults** (closure, unanchored); 11 have no part | plan `models[]`, `ledger status -Verify` `placeholders.models` |
| `67\|a` derive and `59\|a` review | **`67\|a`: 12 parts, 3 unanchored faults, 6 Derivation parts and 6 explanation needs; `59\|a`: 1 part, no Derivation.** No retained model compiler state exists, so no historic derive or review is claimed; both are registered open model units and only their missing work is owed | plan `models[]`, `models/adopted-…@1` units |
| Two Finding jobs | **Not recoverable from this folder**: it retains no Event/Finding plan or job state (`evidence.retained_outputs.events` and `.findings` are `null`). No pending id is invented; the enabled Findings layer owns the enumeration of the re-run | plan `evidence.retained_outputs` |
| 65 model Link suggestions | **0 recovered**: the folder retains no model compiler state, so no retained plan holds a note that names a Link; the notes cycle gets no invented suggestion | plan `notes[]`, `evidence.retained_outputs` |
| Failed/background card blocks | **Not a schedule to reproduce**: 11 incomplete groups and the whole-book Card cycle own the remaining content; no obsolete block chain is replayed | plan proposal, `cards-book/book@1` |
| Remaining orphans/language/untyped-Link/build faults | **92 orphans** (90 model parts, 2 Ideas), **15 unkinded Links of 1,274 writing Cards**, 0 without a source, 2 hard layer faults (73 advisory), 0 broken Links. Style faults: R2 63, R19 32, R6 17, R18 14, R4 2, R12 1, R3 1 | `audit.json` `books[0]` |
| Unexplained Derivation parts and 13 named figures | **357 Derivation parts, 288 explained, 69 unexplained**; **44 figure labels shown, 3 missing known labels, 17 named-but-unknown labels** (20 named references without their image, not 13) | `audit.json` `models`/`figures` |
| DeepSeek leftovers | The folder records **no resource ids** (it predates the resource ledger), so this run owns none; `cleanup/book@1` is registered and reconciles only resources a new run registers | plan, `cleanup/book@1` |
| Unchecked Model drafts and source misprints | **0 of 170 models retain a source review**; the folder has no `source-faults.md`/`.json`, so the whole-book Card cycle owns the review and the source-fault report | plan `models[].review_retained`, derived artifacts |
| 57 open Bridge proposals | **57 proposals, 57 new, 0 refused**, report `9532997edc25a4dd32596f6459fb168409929d3608a3b93da888ad14f8836b9d`, immutable; **0 accepted, 0 applied** | `bridge/report.md`, plan `retained_outputs.bridge` |

Recount summary: 2,671 book Cards; 1,448 Cards carry a body read by the style pass; 130 style
faults across 7 rules; zero broken Links; `hubs_missing` 0. The machine-readable form is
[`digest-recovery-stiglitz-recount-2026-10-18.json`](digest-recovery-stiglitz-recount-2026-10-18.json).

## 3. What adoption makes of the recount

`zt-digest ledger adopt -Out <staging> -Variant econ -Apply` writes the prepared ledger from the
reviewed plan (the plan itself is read-only and separately hashed). On the current evidence:

- **214 adopted units, 127 unresolved obligations** (`digest` 171, `models` 170 by layer).
- The ledger binds the archive identity and the frozen source hash. `ledger status` after apply:
  **216 done, 127 ready, 8 registered**; seven aggregate publications registered and none current;
  `complete: false` because placeholders remain.
- The placeholder recount is verified against the archive: **232 placeholder Cards across 170
  models**, the 42 models above included.
- **Coverage faults name only the seven unbuilt publications** (`audit`, `bridge`, `cards-book`,
  `figures`, `link-check`, `organize`, `skeleton`), not the adoption's own records (section 4).
- The one supported resume is available and unspent (`resume_count: 0`).

## 4. Two defects the recount found, fixed test-first

The first apply of the real evidence produced **168 coverage faults before any resume**: 160
`producer_outputs_mismatch` on the adopted groups, 1 `preparation_outputs_mismatch`, and 7
legitimately unbuilt required publications. Two bookkeeping defects, both in the adoption path:

1. **An adopted group declared its anchor Card as an output but did not register it.**
   `adopted_group_result` enumerates `outputs` from the anchor-plus-substantive fingerprint map,
   while `apply_plan` registered rows only for the substantive set. Every adopted group therefore
   failed the ledger's own emission invariant (R413). Fix: the registration iterates the same
   fingerprint map the result declares (`src/ledger/adopt.rs`), so declaration and registration
   cannot diverge.
2. **The adopted preparation was a placeholder completion.** `initialize(prepared=true)` used
   `register(…, done=true)`, which writes `{"disposition":"verified"}`, while
   `preparation_outputs` demands a typed `prepare_completion`. Fix: an adopted preparation is now a
   typed `preparation_adoption` record declaring exactly the `preparation` output it registers
   (with the preparation hash, the group count and a read-only verification), and
   `preparation_outputs` verifies that declaration (`src/ledger.rs`), exactly as it already did for
   group/model/terms adoption. No historic prepare command or receipt is claimed.

Test-first evidence. `ledger-adopt.tests.ps1` gained the assertions **before** the fix: "every
adopted group registers exactly the outputs it declares" failed (RED) with
`producer_outputs_mismatch` on the fixture's group; after the fix the same suite passes **68
assertions** (60 before), including "the adopted preparation is a typed record, not a placeholder
completion", "the reconciliation names only the unbuilt publications as coverage faults", and the
registered anchor-Card output row. On the real evidence the fix drops coverage faults from **168 to
7** and run-report gaps from **535 to 374**.

## 5. The rehearsal and its evidence

`scripts/book-digest/tests/recovery-rehearsal.ps1` is deliberately **not** named `*.tests.ps1` (it
needs a copy of the real archive and the real legacy staging folder). It exits 1 on any failure and
prints one line per assertion:

```powershell
pwsh -NoProfile -File scripts/book-digest/tests/recovery-rehearsal.ps1 `
    -Archive D:\zt\zt.sqlite3 -Staging D:\zt-digest\stiglitz2014learning `
    -Work D:\zt-target\qualification\recovery-rehearsal
```

**52 assertions, 0 failed** on the installed binaries. It proves: the consistent copy and the
read-only live staging folder; the plan's exact classification (171/160/11, 170/54/116, 214/127,
the eleven group numbers, no invented note suggestion, the Bridge report by hash, 57 proposals and
0 applied, the four derived artifacts, no ledger before `-Apply`); the card-level recount (R18 14,
R19 32, 92 orphans, 15 unkinded Links, 0 broken Links, 288 of 357 explained, 170 models, the
figure counts); the apply (216/127/8, seven publications, `complete: false`, 232 placeholders in 170
models, coverage faults only the seven publications); the before manifest (216 done units, 160
groups); the claim of `digest/g010@1`, a refused fault class, the injected
`check_failed`/`content` failure, **one resume** (invocation 2, `resume_count` 1, the reset group,
135 downstream units, every adopted unit's result/attempts/completion time unchanged, the attempt
budget reset, a second resume refused as `resume_used`); the run report (374 gaps, 134 unresolved,
`complete: false`, the spent resume recorded); the Bridge report byte-identical after the resume;
the after recount identical in every card-level category; the full-parser Link check ok; and the
live staging folder unmutated. It writes `before-after.json` (1.16 MB locally) holding the full
before/after manifest; the compact repository copy is the recount JSON above.

## 6. The authorized live runbook

Only this section needs the user's maintenance window, provider spend and live Card writes. The
main agent does not run it (#90/#91 are refused as live-run work); Claude Code runs it with the
user, exactly as the frozen workflow provides.

1. **Window check.** No digest active (`zt-digest service-wait -Wait 5` → `stage: null`), no
   `zt-digest`/`zt-check` process, no `ledger.sqlite3` in the staging folder. If the window moves,
   take fresh backups first (section 1's two commands).
2. **Read-only plan on the live folder**:
   `zt-digest ledger adopt -Out D:\zt-digest\stiglitz2014learning -Variant econ`.
   Inspect: `status: ready`, `apply.allowed: true`, 171/160/11 groups, 170/54/116 models, 214
   adopted, 127 unresolved, the Bridge report hash `9532997e…`, no `missing` scope. Record the
   plan and proposal hashes.
3. **Apply**: the same command with `-Apply`. It writes `manifest.json`, `ledger.sqlite3` and
   `adoption/applied.json` from the reviewed plan; nothing else in the folder or the archive
   changes.
4. **Verify**: `zt-digest ledger status -Out <folder> -Verify` → 216 done / 127 ready / 8
   registered, `complete: false`, coverage faults only the seven publications, 232 placeholders in
   170 models.
5. **The one authorized resume**, with the frozen economics workflow:

   ```js
   Workflow({scriptPath: "E:\\zet\\.claude\\workflows\\zt-econ-digest.js",
             args: {mode: "resume", out: "D:\\zt-digest\\stiglitz2014learning"}})
   ```

   The tool form `zt-digest ledger resume -Out <folder>` opens the same invocation 2 when the
   workflow runner is not used. **There is one resume**: if the resumed run leaves hard failures,
   the remaining work is a manual worklist and a later repair is a separate preparation. Do not
   spend the resume on a rehearsal or a checkpoint stop.
6. **After the run**: `zt-digest ledger report -Out <folder>`,
   `zt-digest audit -CitationKey stiglitz2014learning -Out <folder>\audit-final`, and
   `zt status --check-links`. The final report must name every remaining gap, the incomplete groups,
   the models still holding `待写`, the unexplained Derivation parts, the unmatched figures, the
   source-review/fault outcome, the Bridge report id with its open proposals, and the owned-resource
   cleanup.
7. **Fill the evidence table** below and comment it on #91.

| Evidence | Value to record |
| --- | --- |
| Adoption plan/proposal hashes, `applied.json` hash | |
| `ledger status -Verify` before the resume (done/ready/registered, placeholders) | |
| Invocation and `resume_count`; the workflow run id | |
| Per-layer outcome after the run (digest, models, explain, cards-book, link-check, organize, skeleton, hubs, figures, bridge, audit) | |
| Completed groups before/after; any group still incomplete with its reason | |
| Models completed, models still `待写`, build faults remaining | |
| Derivation parts explained / still unexplained | |
| Figures shown / unmatched with reason / unresolved | |
| Event and Finding dispositions (candidates, jobs, pending count) | |
| Source review coverage and source-fault report (with original locations) | |
| Bridge report id, proposals open/applied (must stay 0 applied without an exact selection) | |
| Owned resource ids and deletion/workspace settlement | |
| Final audit revision, report hash, full-parser Link check | |
| Actual usage (requests, tokens, cost, missing coverage) and wall-clock time | |
| Remaining manual/source decisions, explicitly named | |

## 7. Named regression scenarios

Every scenario the issue names already has a public suite; this slice adds the real-evidence
rehearsal. The mapping is also recorded in [`digest-release-qualification.md`](digest-release-qualification.md)
section 7.1:

| Scenario | Suite |
| --- | --- |
| T08 owned-paragraph exact span | `owned-paragraph.tests.ps1` |
| T09 duplicate-title refusal and legacy metadata preservation | `content-corruption.tests.ps1`, `workspaces_mcp` integration test |
| T14 every prepared group registered; skeleton anchor is not success | `ledger-group-start.tests.ps1`, `ledger-adopt.tests.ps1` (68) |
| T17 partial model resume keeps the done neighbour's fingerprints | `model-resume.tests.ps1`, `model-finish.tests.ps1` |
| T18 missing evidence never reports done | `cards-finish.tests.ps1`, `ledger-event-finding.tests.ps1`, `explain-finish.tests.ps1`, `ledger-report.tests.ps1` |
| T19 repaired model publishes new downstream revisions only | `ledger-publication.tests.ps1`, `link-finish.tests.ps1`, `hub-compiler.tests.ps1` |
| T20 one resume drains the closure and preserves done builds | `ledger-resume.tests.ps1`, `ledger-claims.tests.ps1`, `ledger-owner-recovery.tests.ps1`, and this rehearsal |
| T23 source spans and note mapping | `book-cards.tests.ps1`, `source-notes.tests.ps1` |
| T24 explanation obligations in full and repair modes | `explain-finish.tests.ps1`, `model-compiler.tests.ps1` |
| T28 usage totals stay nonnegative, deduplicated and incomplete where appropriate | `ledger-report.tests.ps1` |
| T29 cleanup after success/failure and lost responses | `cleanup-resources.tests.ps1` |
| T30 final correction after the initial audit | `finalize.tests.ps1` |
| T32 isolated timings and parity | `qualify-release.ps1`, [`digest-release-latency-reverify-2026-10-18.json`](digest-release-latency-reverify-2026-10-18.json) |

## 8. Not claimed

- No live Card was written, no digest stage ran, no provider was called, and the live staging
  folder was not mutated. The adoption, the failure, the resume and both audits ran on copies.
- The book is not complete: 11 groups, 116 models, 69 unexplained Derivation parts, 20 unmatched
  figure references and the whole downstream publication chain remain open, and the run report says
  so (`complete: false`, 374 gaps).
- The two Finding jobs and the 65 note suggestions named by the proposal are not recoverable from
  this folder's evidence; they are recorded as missing provenance, not reconstructed.
- No Bridge proposal was applied; the 57 stay a review decision.
- The whole-run 3.5–4 hour estimate, the fewer-than-300-agent figure and the token target remain
  unmeasured until one comparable completed run exists (R354). This pass measured only the ledger
  and recount commands (plan/apply/status/report ≈ 3 s each on the 10,895-Card copy; audit ≈ 10 s).
