# Term Cards, Term Links and the Card check: S29 qualification (issue #171)

This is the reviewable qualification package for **S29 / #171**, the integrated-behaviour and
bounded-performance slice of [PRD #142](https://github.com/liudesheng9/zet/issues/142). It records,
requirement by requirement, which public seam proves the delivered behaviour, what the measured
numbers are, which defect this qualification found and fixed, and which claims this package
deliberately does not make.

Source binding: the [implementation design](term-cards-links-and-check-gate-implementation-design.md),
section 18 and §19 (SHA-256 `a27224263597460d0822d7859cd1a822d47e7376bb207686689843fc6d3309d4`),
whose §18.1–§18.6 checks this slice turns into passing tests and measurements. The design's own
sentence stands: §18 describes acceptance work, and no result is claimed by the design document
itself (R0503).

## 1. What is verified here, and what is not

**Delivered and verified here.** Every named case of design §18.2–§18.5 has a passing assertion at
its public seam: the GUI API, stdio MCP, the shell, the line Session, SQLite with the generation-1 →
generation-2 migration, dump/load, the `zt-digest` compiler commands, and the frozen workflows'
stand-in harness; the GUI half of §18.5 was inspected in a real browser on a scratch fixture of the
deployed build (§6). §18.6's bounds are measured on scratch archives with the release build: added gate
work per typical Card, a legal 64-item real batch, a 200-item preview sequence, and the relinker's
plan/apply on the 300-Card fixture.

**Not claimed here.**

- The digest **Workflow** itself is not run: the two variants are qualified through their stand-in
  harness only. The owner runs the real workflow with Claude Code after this slice.
- No provider (DeepSeek) call happened, so live provider cleanup is proven only against the
  stand-in's recorded resources, never against the provider.
- No real book, no owner Lock, no unlock, no historical Term backfill and no enforcement promotion
  ran. The live archive `D:\zt` was not touched by any test.
- The **S28 (#170) release set** is not published by this slice: this package measures the versions
  on `main` when it was produced, not a coordinated installed release set. Live installed-service
  qualification remains S31 (#173).
- Mathematical truth, explanatory depth and owner usability are not claimed: those need the S30
  (#172) owner review of a copied book and the S31 live release.

## 2. Release identity

Measured with the binaries built from the commit that carries this file on `main` (`cargo build
--release --locked`), Windows, `rustc 1.93.0`:

| Item | Value |
|---|---|
| `zt` | `0.39.0` |
| `zt-check` | `0.13.0` |
| `zt-digest` | `0.27.0` |
| Frozen workflows | base `5.5.6`, econ `6.5.6`, math `5.5.6` (the versions on `main`; S28 owns their coordinated re-publication) |
| Rule contract | `rule_version` 4, diagnostic schema 1 |
| Archive schema | Card-check policy in `metadata`, Tree Lock generation 2, Term field migration applied |

The GUI fixes from the S29 browser check are commit `04792c2` (the same 0.39.0 build); the excerpt fix
and the fixture corrections are `dacece9`.

## 3. Requirement matrix

Each row names the check and the behaviour it proves. "Stored state" means the test reads the
SQLite archive (rows, revision, receipts, pins, Lock baselines) or the route's structured reply, not
a human message.

### 3.1 Gate correctness (design §18.2)

| Requirement | Evidence | What it proves |
|---|---|---|
| R0510 new Event/Finding missing labels | `tests/gate_writes.rs:1203` `a_new_event_or_finding_without_its_labels_is_refused_without_residue` | An enforcing archive refuses a new Event (E2) and a new Finding (F1) and keeps no Card, receipt, revision or Topic allocation. |
| R0511 advisory fault in the human Workspace | `tests/check_routes.rs:800` `each_workspace_keeps_its_own_policy_and_each_client_its_own_access` | Under enforce, `gate.human.R3=advise` lets a human shell write store with its R3 advice and the new Card text, while the agent Workspace still refuses the same fault. |
| R0512 retained legacy fault | `src/gate.rs:1498` `a_legacy_fault_is_advice_and_a_second_copy_blocks`; `tools/zt-check/src/check.rs` occurrence identity | A stored legacy fault is returned as legacy advice and the same paragraph copied again is a new blocking occurrence. |
| R0513 second copy of one faulty paragraph | same as R0512; `tests/term_links.rs:383` | The ratchet pairs old and new diagnostics by semantic key, so a second occurrence blocks. |
| R0514 long messages sharing a clipped prefix | `tools/zt-check/src/core.rs` `faults_sharing_a_clipped_line_keep_distinct_semantic_keys` | Two faults clip to the same display prefix but keep distinct full keys; neither hides the other. |
| R0515 Idea retyped to Event | `src/gate.rs:1528` `a_retype_must_meet_the_new_types_format`; `tests/gate_writes.rs:324` | A retype is checked as the new type, so the newly applicable Event format refuses. |
| R0516 concept model promoted to formal | `tests/gate_writes.rs:1252` `promoting_a_concept_model_to_formal_runs_the_new_formal_checks` | A concept part edit stores; the metadata-only promotion stores `level=formal`; the same part write is then refused by R13 with unchanged text and revision. |
| R0517 metadata-only edit | `tests/gate_writes.rs:1252` (level promotion), `tests/gate_writes.rs:1326` (address change); `tools/zt-check/src/check.rs` metadata-only case | The proposed metadata is checked even when the body is identical. |
| R0518 move/rename and generated Reverse refresh | `tests/gate_writes.rs:1326` `a_move_keeps_a_retained_fault_and_refreshes_the_reverse_section`; `tests/gate_writes.rs:573` `a_citation_key_remap_keeps_its_structural_path` | A move/rename runs the structural path only: the retained prose fault stays, the Link target is unchanged, the target's generated Reverse section names the new address, and no prose rejection fires. |
| R0519 policy override | `tests/gate_policy.rs` `one_archive_policy_is_configured_validated_and_reset_while_another_stays_default`; `src/gate.rs:1552` `an_archive_policy_override_decides_an_enforced_write` | One row changes one rule in one Workspace; the other Workspace, the other archive, the invariants and Access stay intact. |
| R0520 missing context / stale version | `src/gate.rs:1581` `a_corrupt_policy_row_makes_the_check_incomplete`; `src/gate.rs:1603` `an_over_budget_target_set_makes_the_check_incomplete`; `tests/check_contract.rs:168`, `:203`, `:250` | An unavailable policy, an unavailable stage, or over-budget context is `check_incomplete`; a mismatched contract is a version gap, never a successful acceptance. |
| R0521 mixed batch rollback | `tests/gate_writes.rs:438`, `:780` | One blocking item rolls back the whole real chunk, receipts included, while an earlier committed chunk stays stored and replayable. |
| R0522 preview independence and dependence | `tests/gate_writes.rs:607`, `:924`, `:1065` | A preview collects independent faults, marks a dependent item blocked with its `blocked_by`, still checks later independent items, and persists nothing. |
| R0523 route × Workspace matrix | `tests/check_routes.rs:642`, `:717`, `:800` | One fault is attempted through the shell `n`, the shell `ins`, the line Session `n`, the GUI create and edit, the MCP single write, the real batch and the preview sequence; each route stores the advised fault with R3 or refuses it with `card_check`, and every refusal leaves the Card count, revision, receipts and Topic allocation unchanged. Both Workspaces are covered with each client's real Access (MCP agent-only; shell, GUI and Session owner). |

### 3.2 Term storage and Link lifecycle (design §18.3)

| Requirement | Evidence | What it proves |
|---|---|---|
| R0524 old archive with active Locks, pins, receipts and non-Idea types | `tests/tree_lock.rs:7118` `a_generation_one_archive_keeps_its_pins_non_idea_types_and_receipts`; `tests/tree_lock.rs:5345` | A downgraded generation-1 archive (active Lock, pinned target, committed receipt, Event Card) upgrades to generation 2 keeping every pin row, type, receipt, Lock event and seal. |
| R0525 row ids, text, fingerprints, seals, history | same test; `tests/tree_lock.rs:5345`; `tests/card_types.rs` authored-text case | Every `row_id`/address survives, the member baselines are byte-identical so the seal still verifies, the Lock history count is unchanged, and the new `term_name` field starts empty. |
| R0526 `term_name`-only change on a Locked head | `tests/tree_lock.rs:5483` `a_locked_term_name_cannot_change_and_a_pinned_target_keeps_its_editable_name` | A name-only change is refused through MCP and through the SQL backstop; a pinned target outside the Lock keeps its editable name. |
| R0527 dump/load of names, types and parts | `tests/term_portability.rs:595`, `:749` | A dump keeps every raw Term macro, name, type, head and part; a dumped archive loads back with the same Term structure, Links and Lock fingerprints. |
| R0528 UTF-8 ranges, strict errors, tolerant rendering, literal code/math | `tools/zt-check/src/text.rs` byte-range and strict/tolerant tests; `tests/term_links.rs:466` `term_like_examples_in_math_and_code_are_literal_at_the_write_seam` | Label/target byte ranges are exact; malformed markup is a strict typed fault; a Term lookalike in code or math stays literal. |
| R0529 rewrite only the target on moves, key rename, compaction | `tests/term_lifecycle.rs:339`, `:709`, `:991`; `tests/term_links.rs:518` | A move, a Citation-key rename and a side compaction rewrite only the target bytes of a Term link and keep its label. |
| R0530 indexed vs full lifecycle parity | `tests/term_lifecycle.rs:888` `indexed_and_full_lifecycle_broken_reports_agree` | The indexed report equals the full-parser report for ordinary Reverse links and Term uses. |
| R0531 a source with repeated uses or both forms | `src/link.rs` one source in each section; `tests/term_lifecycle.rs:485` | One source with several Term uses and an ordinary Link appears once in each generated section. |
| R0532 Term links pin but never reason | `src/tree_lock.rs` cross-tree Term use pin test; `tests/term_reasoning.rs:266`, `:310`, `:376`, `:428`, `:486` | A Term use outside a Locked tree pins its target; it gives no Model membership, relation or neighbour and no reasoning credit; the general readers still see it. |
| R0533 malformed/wrong-role target vs legacy retention | `tests/term_links.rs:220`, `:383` | A new malformed or wrong-role target is refused with no Card or revision change; a retained legacy target is reported with its label and multiplicity and is never grandfathered into a new macro. |

### 3.3 Compiler and matcher (design §18.4)

The relinker and compiler live in `tools/zt-digest`; every test below reads the frozen plan, the
apply report, the archive revision and the receipts.

| Requirement | Evidence | What it proves |
|---|---|---|
| R0534 plan, classification, promotion, 1/4 grouping, stable ids | `tools/zt-digest/src/term_cards.rs` (`t001[1-4]`, `t005[5,6]`, `t007`–`t009` grouping; classification; promotion in `state.json`); `tests/term_cards_plan.rs` | A source-bound plan classifies each row, groups at most four writer jobs and five review jobs, records promotion, and keeps stable ids across runs. |
| R0535 JSON fields, exact row spelling, auto-link persistence, deterministic rendering | `tools/zt-digest/src/term_cards.rs` (row spelling, `自动链接`, T10); `tests/term_link_plan.rs` deterministic bytes; `tests/term_cards_plan.rs` repeat plan | Every required plan field is written, the table row keeps its exact spelling, the auto-link opt-out persists, and a repeat plan is byte-identical. Limitation: no exhaustive schema golden enumerating each field name. |
| R0536 independent reviewer, changed draft, one fix | `tests/term_cards_check.rs:330`, `:468` | The reviewer is a fresh conversation; the draft, render and plan revision are bound into the request; a changed draft forces a new review; a second refusal is final. |
| R0537 whole-step packing, T rules on heads/parts, unsplittable overflow | `tests/term_cards_apply.rs:573`; `tools/zt-check/src/term.rs` packing/overflow cases | A head and its continuation part publish under the head with `推导（续 2/…`; overflow is refused, never truncated; whole-step packing and the per-part T rules are proven by the checker's own cases. |
| R0538 prefix publication after an earlier failed row | `tests/term_cards_apply.rs:701` | The ready prefix publishes; a later ready row stays staged; a repair continues in order. |
| R0539 lost reply, chunk interruption, receipt recovery, stale list/source, no duplicates | `tests/term_link_apply.rs` `a_lost_answer_leaves_its_chunk_unknown_and_never_claims_completion`, `a_body_already_at_its_planned_output_replays_instead_of_replanning`, `a_changed_list_invalidates_the_frozen_plan_before_any_write`; `scripts/book-digest/tests/identified-batch.tests.ps1` | A lost answer is unknown, not done; a committed Card replays; a changed list/source stops before any write; receipts are identified and never duplicated. |
| R0540 leftmost-longest, blockers, homonyms, Latin boundaries | `tools/zt-digest/src/term_link/matches.rs` four cases | Longest word wins, a blocker wins the exact tie, a qualified homonym blocks only the bare form, Latin boundaries and case are exact. |
| R0541 protected spans, CRLF, Unicode, escapes, reference links, HTML, incomplete markup | `tools/zt-digest/src/term_link/spans.rs` (code/charts/math/links/markdown/HTML/escapes/incomplete tails); `tools/zt-digest/src/term_link/card.rs` `a_crlf_body_links_its_prose_and_keeps_its_line_endings` | Protected spans never link; CRLF line endings are preserved while the prose term still links once; incomplete markup protects its uncertain tail. |
| R0542 stripping old/new Term macros gives byte-identical bodies | `tools/zt-digest/src/term_link/card.rs` `generated_bodies_keep_the_visible_text_and_are_idempotent` | Over 80 generated bodies × both occurrence policies, `strip_term_links(new) == strip_term_links(old)` and every edit is an exact byte replacement. |
| R0543 a second plan has zero edits and advances no revision | `tools/zt-digest/src/term_link/card.rs` `a_second_pass_changes_nothing_and_returns_the_same_body`; `tests/term_link_apply.rs` no-op case; `tests/term_cards_plan.rs` repeat plan | A second pass has no edit, sends no batch, reports `no_op`, and leaves the revision and receipts unchanged. |
| R0544 mapping/body guards, explicit revision, immediate Lock stop | `tests/term_link_apply.rs` changed-list, changed-body, locked-tree cases | A changed mapping invalidates the whole plan; a changed body is replanned into a new revision from fresh offsets; a Lock stops the apply immediately with exit 2 and no write. |

### 3.4 Workflows and display (design §18.5)

| Requirement | Evidence | What it proves |
|---|---|---|
| R0545 stage order, disabled layers, resume, dedicated modes, finalization round identity | `scripts/book-digest/tests/framework-harness.mjs` — the S24/S25 checks, plus `node … term-cards-harness.mjs` | The frozen variants run the declared stage order, a disabled Term layer freezes into the prepared target, a résumé of a settled Term branch claims nothing, the dedicated backfill modes run only their own stages, and the finalization round carries `termlink/final@R`. |
| R0546 successful and failed term branches; audit/report still run | `framework-harness.mjs` S24 failure cases | A failed writer fails the branch without halting the run, the book unit never completes, the Term-link dependency stays blocked with its reason, and the audit and run report still describe the book. |
| R0547 manager/conversation caps with overlapping jobs | `framework-harness.mjs` pool checks; `term-cards-harness.mjs` | A run's peak manager count stays within the shared pool and a Term branch's conversations stay within its wave bounds. |
| R0548 every recorded session collected and every workspace registration removed, including failures | `framework-harness.mjs` `settledResources` checks (S29): a full failure run records 18 conversations and 14 workspaces, all terminal+deleted and removed; a resume record is settled too; `scripts/book-digest/tests/cleanup-resources.tests.ps1` | The stand-in's own recorded facts show every conversation terminal and deleted and every recorded workspace removed, including after a failure and after a resume. The dedicated backfill modes start no recorded provider resource in this stand-in, so their check is vacuous and rests on the full/failure evidence. |
| R0549 term-only modes never rerun digest/model writers or provider probes | `framework-harness.mjs` S25 mode checks | Both backfill modes dispatch no digest, Card, model, Link, Organize, Figure, Bridge or finalization stage; the deterministic `term-link` mode takes no source stage and starts no DeepSeek manager. |
| R0550 GUI light/dark, hover/focus, broken/cross-Workspace links, completion, ordinary Link distinction, collapsed uses | see §6 below (real-browser inspection) and `src/gui.rs` tests; the fixture is `scripts/book-digest/tests/qualify-term-gui-fixture.ps1` | The browser pass records what the page shows for each of the eight fixture checks. |
| R0551 Session width/wrap/hit testing and raw editing with Han and combining characters | `tests/term_session.rs:268` `copying_a_wrapped_term_line_takes_the_visible_text`; `:239` `clicking_a_term_label_follows_its_head`; `:308` `raw_editing_keeps_the_complete_term_macro`; `tests/pty_session.rs:496`, `:545` | At 20 columns a wrapped Term line copies its visible text exactly, a combining mark stays on its base and Han characters keep their columns; a click follows the label's target; raw editing keeps the complete macro. |

### 3.5 Performance and preservation (design §18.6)

| Requirement | Evidence | What it proves |
|---|---|---|
| R0552 under 5 ms of added gate work per typical Card, separately from lock and I/O | `src/gate.rs:2056` `measure_the_gate_work_per_typical_card` (release measurement); `src/gate.rs:2040` bounded-context test | Measured medians are 154–304 µs per typical Card (table in §4), far under the 5 ms target; the measurement calls the gate decision directly, so it excludes lock waiting, file I/O and provider time. |
| R0553 legal 64-item real batch and a 200-item preview sequence, with context queries and archive size | `src/gate.rs:2040` (7 statements per decision, constant from a 6-Card to a 608-Card archive); `tests/gate_batches.rs:253`, `:291`, `:328` | A 200-item preview sequence is legal (bound 1024), checks all 200 and persists nothing; a 64-item real batch stores 64 Cards, one revision and 64 receipts, with one combined index refresh of 64 sources; both are timed in §4. |
| R0554 relinker plan and apply on a 300-Card fixture, with the actual batch count | `tools/zt-digest/tests/term_link_apply.rs:539` `measure_the_relinker_on_three_hundred_cards`; correctness at `the_term_link_apply_writes_three_hundred_cards_in_bounded_chunks` | 304 Cards scanned, 301 changed, 5 bounded batches; plan and apply timed in §4. |
| R0559 preservation for every changed body and zero writes on a second pass | `tools/zt-digest/tests/term_link_apply.rs` (every stored body equals the planned output and strips identically; the second apply writes 0 and keeps the receipts) | No changed body loses its visible prose, and a second pass writes nothing. |
| R0561 Term markup alone adds no prose fault | `tools/zt-check/src/core.rs:4043` `term_markup_adds_no_prose_fault`; `tests/term_links.rs:466` | The same body with and without its Term macros produces byte-identical diagnostics, including a real R3 fault, so linking adds no prose fault. |

## 4. Measured performance

Method: the release build (`cargo test --release`), a scratch archive on a temporary folder, a
warm-up followed by 200 measured gate decisions per Card class (in-process, one function call each),
and 5 runs of each batch path (plus one warm-up) through the real stdio MCP. "Added gate work" is
the time from `gate::check_proposed` entering to its decision, so lock waiting, file I/O and
provider time are outside the number by construction. The statement count comes from SQLite's own
statement trace on the measured connection.

Reproduce:

```powershell
cargo test --release --bin zt gate::perf -- --ignored --nocapture
cargo test --release --test gate_batches -- --ignored --nocapture
cd tools/zt-digest; cargo test --release --test term_link_apply -- --ignored --nocapture
```

### 4.1 Added gate work per typical Card (R0552)

Archive: 608 Cards, 94 208 bytes (one book with a term table and a Term head, one concept model with a
part, 300 unrelated Cards). Statements issued by one decision: **7**, the same on a 6-Card archive
and on the 608-Card one.

| Card class | Median | p95 | p99 | max | Accepted | Advice per run |
|---|---|---|---|---|---|---|
| Idea create in a book | 156 µs | 205 µs | 237 µs | 260 µs | 200/200 | 0 |
| Idea edit (ratchet path) | 220 µs | 249 µs | 382 µs | 478 µs | 200/200 | 1 |
| Term head edit (T rules) | 301 µs | 338 µs | 499 µs | 504 µs | 200/200 | 5 |
| Model part edit | 191 µs | 210 µs | 365 µs | 402 µs | 200/200 | 0 |

The target is **< 5 000 µs**: the worst measured median is 301 µs, about 6 % of the target. A second
run on the same build reproduced the medians within about 2 % (154–156, 220–221, 301–304, 191–197 µs);
the p99/max values move with machine noise (the max was 219–504 µs across the two runs).

### 4.2 Legal batch paths (R0553)

Archive: 396 Cards, 593 920 bytes. Five measured runs after one warm-up; a chain of direct
successors so every item has a distinct address.

| Path | Items | Median | p95 | max | Per item | Stored effect |
|---|---|---|---|---|---|---|
| Real batch | 64 | 39.3 ms | 40.2 ms | 40.2 ms | 613 µs | revision +1, 64 receipts, 64 indexed Cards, 64 refreshed sources, 0 changed Reverse sections |
| Preview sequence | 200 | 104.6 ms | 107.6 ms | 107.6 ms | 522 µs | 200 checked, 0 persisted, revision unchanged |

The 200-item **preview** sequence is the legal form of the proposal's "200-item batch" (the real
bound is 64, `src/batch.rs:525`); a 200-item real batch is not legal and is not benchmarked.

### 4.3 Relinker on the 300-Card fixture (R0554)

Archive: 304 Cards scanned (one book with a four-row term list and three heads, plus 300 prose
Cards), 675 840 bytes. Two runs, each on its own scratch archive; the plan and the apply run as the
real `zt-digest term-link` commands.

| Step | Median | Max | Result |
|---|---|---|---|
| Plan | 228.6 ms | 228.6 ms | 301 changed, 0 refused, complete |
| Apply | 582.4 ms | 582.4 ms | 5 bounded batches, 301 writes |

The plan time varied between runs (578 ms on a cold first run, 229 ms after the archive was warm); the
apply time was 582–639 ms. Both are reported as measured, not as a bound.

The 300-Card apply stays at **5 batches** (≤ 64 items each), which is the actual batch count the
requirement asks to record; the literal matcher is retained because no measurement suggests it is
inadequate.

## 5. Defects found and fixed

S29 found six defects and fixed all of them; a defect is only recorded here when a public seam
shows it and a regression test or a measured check now guards it.

1. **The shell insert route dropped its accepted warnings** (`src/main.rs`, `shell_insert`; fixed in
   `0f243ac`). It reached the gate and refused correctly, but discarded the `Accepted` value, so an
   advisory archive stored the insert **without publishing its warnings**. It now prints them
   through the same stream as `n`, `b` and `e`; `tests/check_routes.rs` fails if it stops.
2. **A map node excerpt leaked a Term use's hidden address** (`src/ops.rs`, `excerpt`; fixed in
   `dacece9`). A Term use previewed as `均衡@TermBook2026/1|a|a` because only the `[[`/`]]` marks
   were removed. The excerpt now runs the line through `zt_check::text::strip_term_links`, the same
   projection the checker and the projected reads use; `an_excerpt_projects_a_term_use_to_its_label`
   covers a whole-word use, a broken use and an ordinary Link (whose address is kept). The excerpt is a display summary, so it is now projected in both read modes while `body`/`text` keep the raw and label modes of S12; `tests/term_projection.rs` asserts that refined rule, and the MCP block doc records it.
3. **Completion ranked a title match before an exact Citation key** (`src/gui/draft.js`; fixed in
   `04792c2`). Typing `[[TermB` listed the human Card `1/1` first (its title contains
   `TermBook2026`), so Enter inserted `[[1/1]]`. One ranking scale now orders exact address or key,
   exact term name, address prefix, term-name prefix, held term name, then title; the inserted forms
   are unchanged (`[[label@address]]` and `[[address]]`).
4. **The completion menu covered the caret line** (`src/gui/app.js`; fixed in `04792c2`). It is now
   placed beside the measured caret line, below or above depending on the room, appended outside the
   desk's transform, with the active row scrolled into view.
5. **The list dialog was not a keyboard dialog** (`src/gui/app.js`, `listDialog`, used by Broken
   links, Workspaces, Spreads and Commands; fixed in `04792c2`). Escape closed nothing, focus stayed
   on the opener behind it, it had no accessible name, and a route change left it open. It is now a
   named modal that keeps focus inside (Tab wraps), closes on Escape or a hash change, and returns
   focus to the opener.
6. **The `TERM USES` summary control missed the text-contrast target** (`src/gui/app.css`; fixed in
   `04792c2`). It measured 2.66:1 in light and 4.0:1 in dark; it now measures 5.14:1 light and
   7.17:1 dark on the desk (5.70:1 and 5.76:1 on the stock), guarded by a test that computes the
   contrast from the theme tokens.

Two further findings were not product defects and needed no code change: a diagnostic list may name
a missing target's address (that is how a reader finds what to fix, while the body must not), and
cross-Workspace chips are inked by the source Card's Workspace in both directions. The fixture's
checklist was corrected for both (`dacece9`), and its `-Reset` now waits for the old scratch daemon
process to exit before it deletes the archive, because `zt down` clears the record before the process
ends.

## 6. GUI and Session display inspection (R0550, S29-AC08)

The Session checks are mechanical and cited in §3.4 (R0551). The GUI was inspected in a real browser
on the deployed build, twice: first against `0f243ac` (which found defects 2-6 above), then again
after the fixes (`04792c2`). The fixture is
`scripts/book-digest/tests/qualify-term-gui-fixture.ps1`; it builds a scratch archive (never
`D:\zt`), prints the GUI URL and token, and holds its own service. The inspection read the DOM and
computed contrast, at a 1024 px viewport in the first pass and 1280x560 in the second, in the light
and dark themes.

| Fixture check | Result |
|---|---|
| 1. A Term use is its bare label, quieter than an ordinary Link, with hover/focus and a click to the head | pass: two `均衡` labels, no `[[` or `@` in the body, prose ink with a dotted line; contrast 16.79:1 light, 12.35:1 dark; the ordinary Link is a chip (5.49:1 / 6.86:1) with an inset hover ring; clicking a label opened `TermBook2026/1\|a\|a` |
| 2. An ordinary reasoned Link is visibly a Link with its kind | pass: `依赖 [0/1 重复博弈的稳定策略]` chip |
| 3. A broken Term use is labelled and marked without hover | pass: `弹性` dashed in broken ink with a visible `missing` stamp (8.42:1 / 7.09:1) and no address in the body |
| 4. A broken ordinary Link is marked and listed | pass: `0/3` shows the struck target, and the Broken-links reader lists `0/3 → 0/1\|a` and `0/4 → TermBook2026/1\|a\|d · 弹性 · missing` |
| 5. A cross-Workspace Link carries the other Workspace's ink both ways | pass: `1/1` links `cross agent`; the reverse chip on `TermBook2026/1` is `cross human` |
| 5b. Map node excerpts and tooltips show the label, not the address | pass after defect 2: `0/2` shows `均衡`, `0/4` shows `弹性`, no `@TermBook` |
| 6. A Term head is one explanation Card with folded uses | pass: `释义` and `区别` headings, a `来源` line, and a closed `TERM USES · 1` that opens to `0/2` |
| 7. The writing desk completes Term names and ordinary citations | pass after defects 3-4: `[[TermB` lists `TermBook2026` first and Enter inserts `[[TermBook2026]]`; `[[均` offers the Term and inserts `[[均衡@TermBook2026/1\|a\|a]]`; the menu never covers the caret line |
| 8. Light/dark and focus | pass after defects 5-6: Tab order is stamp → `均衡` → `均衡` → `0/1` → `TermBook2026` → chips → buttons, the focus ring is visible, broken marks stay at least 5.16:1, and the `TERM USES` summary reaches 5.14:1 light and 7.17:1 dark |

The GUI fixes were built and tested by the GUI session of this repository (the main agent), whose
`src/gui.rs` tests now number 78 and include four S29 tests: completion ranking, menu placement and
row scrolling, the list dialog's modality/Escape/route/focus, and the Term-uses contrast computed
from the tokens. The Session's wide-character and hit-testing checks are `tests/term_session.rs`
(sharing `tests/support/pty.rs`), cited in §3.4.

## 7. Verdict per acceptance criterion

| Criterion | Verdict |
|---|---|
| S29-AC01 route/policy/ratchet/preview/version/context cases across both Workspaces with real Access | met: §3.1 rows R0510–R0523, including the parameterized route matrix and its stored-state checks |
| S29-AC02 old active-Lock archive upgrade, identities/seals/history, term-name freezing, receipt projections, dump round trips | met: §3.2 rows R0524–R0527 |
| S29-AC03 indexed/full lifecycle, syntax/projection/Model/graph, preservation/idempotence properties, stale/lost-response recovery | met: §3.2 rows R0528–R0533 and §3.5 row R0561 |
| S29-AC04 compiler grouping/promotion/schema/review invalidation/one-fix/prefix/chunk/occupancy/source-conflict and every T-rule role | met: §3.3 rows R0534–R0538, R0540–R0544 |
| S29-AC05 full/dedicated/resume/finalize/disabled/failure workflow behaviour and recorded session/workspace cleanup | met in the stand-in only: §3.4 rows R0545–R0549; the dedicated modes record no provider resource in this stand-in (stated above) |
| S29-AC06 added gate latency under 5 ms, median/tail, context queries, index refreshes and archive size separately from lock/provider time | met: §4.1 (worst median 301 µs, 7 statements, constant across archive size) |
| S29-AC07 legal 64-item real batches, a 200-item preview or four real chunks, and a 300-Card relink fixture | met: §4.2 and §4.3; no invalid 200-item real batch is benchmarked |
| S29-AC08 light/dark/focus, broken and cross-Workspace GUI, completion and ordinary citations, use disclosures, Session wide-character wrapping/hit/copy | met: Session by §3.4 row R0551; GUI by the real-browser inspection in §6, whose two defects of type §5-2 to §5-6 were found there and fixed in `04792c2` |
| S29-AC09 explicit evidence/limitations; stand-in results claim no mathematical truth, live provider cleanup, owner usability or installed-service qualification | met: §1 and §3.4 row R0548 state each limit |
