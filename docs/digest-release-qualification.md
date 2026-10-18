# Digest release qualification (issue #89)

This is the reviewable release package for the reliable and fast digest runs ([PRD #64](https://github.com/liudesheng9/zet/issues/64)): the regression, pre-flight, simulation, installation-verification and isolated-latency evidence for **both** frozen variants, plus the installation/rollback and short-source failure/resume pilot package. Real provider runs and the Stiglitz book stay separate milestones ([#90](https://github.com/liudesheng9/zet/issues/90), [#91](https://github.com/liudesheng9/zet/issues/91)).

The machine-readable evidence is [`digest-release-latency-evidence.json`](digest-release-latency-evidence.json), produced by [`scripts/book-digest/tests/qualify-release.ps1`](../scripts/book-digest/tests/qualify-release.ps1) on a consistent copy of the representative archive. The pilot source is [`digest-pilot-short-source.zh.md`](digest-pilot-short-source.zh.md).

## 1. What this release is, and is not

**Delivered and verified here.** Both frozen variants (economics `zt-econ-digest` 4.4.0, mathematics `zt-math-digest` 3.4.0, base `zt-digest-base` 3.4.0) validate, simulate through every stage, match their installed and frozen scripts, and pass their full regression. Every enabled ledger layer completes only through a verified adapter. The named tool targets are measured on an isolated consistent archive copy with a warm-up and ten measured runs, with call/address/byte/update/queue/lock evidence and full-parser parity. Fixed local work is proven against 256 unrelated Cards.

**Not claimed here.** A real provider run, a real book completion, the whole-run 3.5–4 hour estimate, the fewer-than-300-agent figure and the token target are **not** measured by this package (R354 explicitly says estimates alone do not pass that gate). Those need one comparable completed run and belong to #90/#91 and to the user's Claude Code run.

## 2. Release identity

Measured binaries (release, `--release --locked`), each byte-identical to a fresh build of the release commit:

| Binary | SHA-256 | Version |
|---|---|---|
| `zt.exe` | `751727E77F27F68466018F7E0003773363FFBA49F3B0EBE4A9BAF2DE831624E9` | `zt 0.20.0` |
| `zt-boot.exe` | `BCE8F8966EA41FB5900441A14F45A0A0F1C82144437CCC9869774A941E42EE89` | — |
| `zt-check.exe` | `36D0FD6195C0867746B87F97A2C06971E5BFDFDF112EACAFC08AC13E44F30A2F` | — |
| `zt-digest.exe` | `EF3FDE47DA58D89082AB6D73C141DB0F23D353097BEBC790A9937B8D53A767C7` | `zt-digest 0.1.0` |

Frozen workflows installed at project scope and verified by the assembler:

| Workflow | Version | Frozen script | Library drift | Problems |
|---|---|---|---|---|
| `zt-econ-digest` | 4.4.0 | `.claude/workflows/zt-econ-digest.js` | none | none |
| `zt-math-digest` | 3.4.0 | `.claude/workflows/zt-math-digest.js` | none | none |
| `zt-issue-dev` | 1.1.0 | `.claude/workflows/zt-issue-dev.js` | none | none |

Module matrix of the release (frozen into both variants): `zt-digest-lib` 12, `zt-model-lib` 3, `zt-math-lib` 2 (libraries); `zt-digest-prepare` 7, `zt-group-cycle` 2, `zt-book-terms` 3, `zt-cards-layer` 1, `zt-cards-book` 3, `zt-model-layer` 2, `zt-explain` 4, `zt-event-layer` 3, `zt-finding-layer` 3, `zt-link-prepare` 3, `zt-link-pass` 2, `zt-link-check` 1, `zt-organize` 2, `zt-figures` 2, `zt-hubs` 2, `zt-bridge` 5, `zt-audit` 2, `zt-finalize` 1, `zt-dsh-source` 1, `zt-dsh-cleanup` 3, `zt-digest-report` 9, `zt-math-skeleton` 1, `zt-math-boundary` 1, `zt-math-proof` 1, `zt-math-theory` 1, `zt-math-check` 1.

## 3. Pre-flight P1–P9 and installation verification

Each blueprint carries its own P1–P9 record in `preflight`; `assemble.mjs validate` reports **0 errors, 0 warnings** for the base and both variants (and for `zt-issue-dev`). The record below is the release re-check of those decisions, not a new design.

| Step | Economics 4.4.0 | Mathematics 3.4.0 |
|---|---|---|
| P1 goal/result | the book digest with the economics layers; the base result plus `models`, `events`, `findings`, the Bridge report and the settlement | the mathematics digest with unit/proof/Theory layers; the base result plus the mathematics reports and M1–M18 |
| P2 inputs | base args plus `events`, `findings`, `args.book` (Work id or title) | base args plus the mathematics stage args |
| P3 units | the base groups plus the book's Model Cards, dated sentences (Event) and data sentences (Finding, 3 groups per job) | the base groups plus the planned mathematical units |
| P4 roles | manager Haiku/low (relays one command), architect Sonnet (plan only), organizer Opus (decisions only) | same three roles |
| P5 stage graph | `args → source → prepare → terms → events ∥ findings → digest → cards → models → explain → hubs-first → cards-book → link-prepare → link → link-check → organize → figures ∥ hubs → bridge → audit → finalize → cleanup → report` | `args → source → prepare → math-skeleton → terms → digest → cards → models → math-boundary → math-proof → explain → hubs-first → cards-book → link-prepare → link → link-check → math-theory → organize → figures ∥ hubs → bridge → audit → finalize → math-check → cleanup → report` |
| P6 modules | the economics module set above, one module per stage/step | the mathematics module set above |
| P7 checks | `expect` checks per stage, one SELF-CHECK per DeepSeek job, `zt-check` rules, Bridge BR1–BR9 and exact report acceptance | as economics plus the boundary/proof/Theory checks |
| P8 failure policy | `args`, `source`, `prepare`, `digest`, `link-prepare` halt; `terms`, `organize`, `bridge`, `finalize`, `cleanup` continue; content faults settle against the unit, transient faults retry within the unit; modes `link`, `cards`, `hubs`, `figures`, `audit`, `bridge`, `models`, `explain`, `events`, `findings`, `resume`; checkpoint `digest` | as economics plus the mathematics modes |
| P9 dry run | `zt-econ-digest.sim.json` reaches every stage; the harness checks the readiness graph, joins, runner restrictions and the four finalization branches | `zt-math-digest.sim.json` reaches every stage including the mathematics tail |

Release checks executed on 2026-10-18:

- `assemble.mjs validate` — 0 errors, 0 warnings for base, economics, mathematics, issue-dev.
- `assemble.mjs simulate … --sim …` — economics: `ok: true`, `halted: null`, stage order through `finalize → cleanup → report`, 35 agent calls, peak 4, 34 distinct labels. Mathematics: `ok: true`, `halted: null`, stage order through `math-check → cleanup → report`, 46 agent calls, peak 2, 45 labels. The stand-in runtime executes no shell command and starts no model.
- `assemble.mjs verify zt-econ-digest|zt-math-digest|zt-issue-dev --scope project` — `ok: true`, `problems: []`, `notes: []`, `libraryDrift: []` for all three.
- No workflow module, blueprint or simulation changed in this slice, so the installed frozen versions are the source versions; the assembler refuses a same-version rebuild, and `verify` is the correct release action.
- Agent skills re-installed from `skills/` by `scripts/install-agent-integrations.ps1`: `zt` (3 files), `zettelkasten` (3), `zt-digest-protocol` (2) are now byte-identical in `~/.claude/skills`, `~/.codex/skills` and the `~/.dsh` links. The installer re-registered the `zt` MCP server (27 tools) and answered a handshake.

## 4. Regression evidence (all required suites retained)

Every required suite was run from PowerShell on 2026-10-18, with the separate D: target directories the protocol names.

| Check | Command | Result | Log |
|---|---|---|---|
| Root, all targets, locked | `CARGO_TARGET_DIR=D:\zt-target cargo test --all-targets --locked -- --test-threads=4` | **376 passed, 0 failed, 1 intentionally ignored** (the manual real-clipboard check) | `D:\zt-target\qualification\issue89\root-tests.log` |
| Checker, release, locked | `CARGO_TARGET_DIR=D:\zt-check-target cargo test --release --locked` | **10 passed, 0 failed** | `issue89\check-tests.log` |
| Digest, release, locked | `CARGO_TARGET_DIR=D:\zt-digest-target cargo test --release --locked` | **132 passed, 0 failed** | `issue89\digest-tests.log` |
| Aggregate public runner, throttle 4 | `pwsh -NoProfile -File scripts\book-digest\tests\run-all.ps1 -Throttle 4` | **62 suites, 0 failed, 225 s** | `issue89\public2-summary.log` + `issue89\public2\` |
| Frozen framework harness | inside the runner (`framework-harness.mjs`) | pass | `issue89\public\framework-harness.mjs.log` |
| Bridge workflow harness | inside the runner (`bridge-workflow-harness.mjs`) | pass | `issue89\public\bridge-workflow-harness.mjs.log` |
| Workflow tools (no workflow run) | `node scripts/issue-dev/tests/issue-relay.tests.mjs` | **16 scenarios passed** | — |
| Framework protocol own tests | `node …\workflow-framework-protocol\tests\run-tests.mjs` | **28 passed, 0 failed** | — |
| Developer wiki | `node ~/.agents/skills/project-wiki/scripts/wiki.mjs check` | **the wiki is current** | — |

The MCP/CLI/model/math/hub/checker suites are among the 62 public suites (`workspaces_mcp`, `cli_service`, `model-compiler`, `math-compiler`, `hub-compiler`, `zt-check`, `owned-paragraph`, …). The aggregate runner is the retained throttle-four entry point; it discovers every `*.tests.ps1` and `*harness.mjs` in `scripts/book-digest/tests/`, so **no suite is registered anywhere else**.

## 5. Architecture: the four modules, one ledger, no new runtime

| Design item | Where | Verified by |
|---|---|---|
| R038 Link index | `src/link_index.rs` (328 lines) | migration/reindex/ordinary-write suites, live full-parser parity |
| R039 Targeted reads | `src/read.rs` (666 lines) | projection/model/hub parity suites and the live whole-archive oracle |
| R040 Batches and receipts | `src/batch.rs` (680 lines) | `workspaces_mcp` batch/receipt/rollback tests, `identified-batch` |
| R041 Digest ledger | `tools/zt-digest/src/ledger.rs` (14.7k lines, one module) | `ledger-*.tests.ps1` (17 suites) |
| R042 Narrow runner | the base blueprint's `manager` role (Haiku, low, relays one command) with the mechanical instructions in `zt-digest-lib` | `framework-harness.mjs` (runner restrictions, one command per step, the 600000 ms timeout) |

No new persistent read cache, writer service, broker, general workflow engine or GUI was added. The dependency sets are unchanged from the design baseline: the root crate keeps `rusqlite`/`serde_json`/`tempfile` among its ordinary dependencies, `zt-digest` only adds `rusqlite`/`serde_json`/`regex`/`tempfile`/`getrandom` and `zt-check` as a library, and `zt-check` needs only `anyhow`/`regex`/`serde_json`. Provider jobs still run in the workflow's DeepSeek stage; the compilers still use the one `zt mcp` client and the existing Access rules.

## 6. Checked adapter coverage: no manager text establishes completion

Every unit finishes through one command that re-reads the archive (or the frozen job files) and records a verified completion; a manager's statement is never evidence. The layer list is `ledger.rs`'s `COMMON` + `MATH` sets; each row names the adapter and the public suite that drives it end to end.

| Layer | Verified adapter (finish path) | Public suite(s) exercising it |
|---|---|---|
| `prepare` | `ledger done` → `prepare_finish` | `ledger-preparation`, `ledger-prepare-recovery` |
| `terms` | `terms -Step apply -OperationPrefix` (accepted plan + receipts) | `ledger-terms`, `book-terms`, `identified-batch` |
| `digest` (one group) | `group-start` → `group-finish` | `ledger-group-start`, `ledger-group-finish`, `ledger-group-finish-boundaries`, `ledger-claims` |
| `cards` (per group) | `cards-finish` (offline SELF-CHECK over the frozen owned set) | `cards-finish`, `book-cards` |
| `cards-book` (whole book) | `book-cards-finish` | `book-cards`, `cards-finish` |
| `models` | `model-finish` (+ `model-deps` for readiness) | `model-finish`, `model-dependencies`, `model-resume`, `model-compiler` |
| `explain` | `explain-finish` | `explain-finish` |
| `events` | `events-finish` | `ledger-event-finding`, `event-compiler` |
| `findings` | `findings-finish` | `ledger-event-finding`, `finding-compiler` |
| `link` | `link-finish` | `link-finish`, `owned-paragraph` |
| `link-check` | `ledger done` → `publication_finish` | `orphan-scope`, `digest-audit` |
| `notes` | `ledger done` → `note_disposition` | `source-notes`, `organize` |
| `organize` | `organize-apply` then publication | `organize` |
| `skeleton` | `skeleton -Step apply` then publication | `organize`, `framework-harness` |
| `hubs` | publication (`hubs`) | `hub-compiler` |
| `figures` | publication (`figures`) | `attach-figures` |
| `bridge` | `bridge -Step precheck/apply` then publication | `bridge-planning`, `bridge-compiler`, `bridge-validation`, `bridge-human`, `bridge-math`, `bridge-attribution-retry` |
| `audit` | publication (`audit`) | `digest-audit`, `orphan-scope` |
| `finalize` | `finalize -Step plan\|scope\|refresh\|final` + publication | `finalize` |
| `cleanup` | `cleanup-finish` | `cleanup-resources` |
| `math-index` / `math-docs` / `math-skeleton` / `notation` / `math-check` | `ledger done` → `math_*_finish` | `ledger-math`, `math-compiler`, `framework-harness` |
| `boundary` / `proof` | `ledger done` → `boundary_finish` / `proof_finish` | `ledger-math`, `math-compiler` |
| `theory` | publication (`theory`) | `ledger-math`, `math-compiler` |
| claims, leases, recovery | `ledger claim\|renew\|done\|fail\|reclaim\|next\|gate` | `ledger-core`, `ledger-schema`, `ledger-claims`, `ledger-owner-recovery`, `ledger-options`, `ledger-settings` |
| run report | `ledger report`, `ledger status -Verify` | `ledger-report`, `ledger-publication`, `ledger-resume`, `ledger-adopt` |

Every adapter command exists in the release binary (checked directly: all 27 tools answer with their own parameter requirements, and `ledger` lists `init, status, gate, next, renew, done, fail, rerun-list, resume, publish, notes, relocate, math, reclaim, job, resource, report, adopt`). The schema of the one ledger is validated table-by-table by `ledger-schema`, and its `events` table is append-only by trigger.

### 7.1 Named regression scenarios (T01–T32)

Every scenario in the issue's testing decisions is exercised through the public command/MCP/compiler boundary (a private helper-only check is never counted). The assertion counts are from this release run; the suite files are the ones the aggregate runner executes.

| Scenario | Where it is exercised |
|---|---|
| T01 migration parity and idempotence | `tests/workspaces_mcp.rs` `migrating_an_existing_markdown_archive_keeps_authored_cards_and_is_idempotent` |
| T02 failed migration and reindex recovery | `legacy_link_migration_names_a_bad_card_and_can_be_retried_after_repair`, `an_explicit_reindex_repairs_corrupt_derived_rows_without_changing_card_text` |
| T03 ordinary and structural Link maintenance | `ordinary_link_edits_only_index_the_changed_card_and_refresh_its_neighbours`, `structural_changes_keep_the_link_index_in_step_and_protect_human_authorship`, `creating_a_previously_missing_target_keeps_the_broken_links_that_named_it` |
| T04 move/insert/rename/delete/compact across Workspaces | `tests/cli_service.rs` (GUI `/api/create`/`/api/move`/`/api/delete`, Spread compaction, Session `mv`/`del`) + the same `workspaces_mcp` structural tests |
| T05 projection parity against the full Snapshot | `tests/workspaces_mcp.rs` model/math/hub parity + the live whole-archive oracle in [`digest-indexed-read-live-oracle.json`](digest-indexed-read-live-oracle.json) (645 checks, 0 failures) |
| T06 bulk read order, missing addresses, limits, revision consistency | `prepare.tests.ps1` (29), `scoped-bulk.tests.ps1` (31), `digest-audit.tests.ps1` (17) |
| T07 batch rollback, replay, conflicts and local references | `tests/workspaces_mcp.rs` batch suite + `identified-batch.tests.ps1` (69 calls in 64+5 chunks) |
| T08 owned-paragraph exact-span correction | `owned-paragraph.tests.ps1` (22 assertions) |
| T09 duplicate-title refusal and legacy metadata preservation | `content-corruption.tests.ps1` + `workspaces_mcp` `ordinary_and_identified_writes_refuse_a_duplicate_leading_title_but_preserve_legacy_metadata_edits` |
| T10 competing writers, live lock holder, Windows os error 32 | `service-health.tests.ps1` (53) + `workspaces_mcp` lock tests + `cli_service` |
| T11 two claimants, one owner, stale token refusal | `ledger-claims.tests.ps1` (43) |
| T12 killed manager/compiler before and after commit | `mcp-lifecycle.tests.ps1` (27), `identified-batch.tests.ps1`, `ledger-owner-recovery.tests.ps1` |
| T13 lease expiry while a unit guard is held | `ledger-owner-recovery.tests.ps1`, `cleanup-resources.tests.ps1` (45) |
| T14 every prepared group registered; skeleton anchor is not success | `ledger-group-start.tests.ps1`, `ledger-adopt.tests.ps1` (60) |
| T15 producer completion and child registration commit together | `ledger-preparation.tests.ps1`, `ledger-publication.tests.ps1` (62), `ledger-group-finish.tests.ps1` |
| T16 failed prerequisite, dependency cycle, missing part, late prerequisite | `ledger-dependencies.tests.ps1` (7), `model-dependencies.tests.ps1` (24) |
| T17 partial model resume keeps the done neighbour's fingerprints | `model-resume.tests.ps1` (20), `model-finish.tests.ps1` |
| T18 missing evidence never reports done | `cards-finish.tests.ps1`, `ledger-event-finding.tests.ps1`, `explain-finish.tests.ps1`, `ledger-report.tests.ps1` (54) |
| T19 repaired model publishes new downstream revisions only | `ledger-publication.tests.ps1`, `link-finish.tests.ps1`, `hub-compiler.tests.ps1` |
| T20 one resume drains the closure and preserves done builds | `ledger-resume.tests.ps1` (43), `ledger-claims.tests.ps1`, `ledger-owner-recovery.tests.ps1` |
| T21 hash/identity/contract mismatch refused before writing | `ledger-resume.tests.ps1`, `ledger-preparation.tests.ps1`, `ledger-settings.tests.ps1` |
| T22 audit and link check share one orphan set at one revision | `orphan-scope.tests.ps1` (21) |
| T23 source spans and note mapping | `book-cards.tests.ps1` (47), `source-notes.tests.ps1` (55) |
| T24 explanation obligations in full and repair modes | `explain-finish.tests.ps1` (64), `model-compiler.tests.ps1` |
| T25 frozen simulations of both variants | `framework-harness.mjs` (172 checks, including the join graph and all four finalization branches) + `assemble.mjs simulate` for both variants |
| T26 runner tool restrictions and step prompts | `framework-harness.mjs` (172 checks) |
| T27 provider hang, cancellation, service loss, MCP never returns | `mcp-lifecycle.tests.ps1` (27), `service-health.tests.ps1` (53) |
| T28 usage totals stay nonnegative, deduplicated and labelled incomplete | `ledger-report.tests.ps1` (54) |
| T29 cleanup after success/failure and lost responses | `cleanup-resources.tests.ps1` (45) |
| T30 final correction after the initial audit | `finalize.tests.ps1` (57) |
| T31 book-title ambiguity, multiple Markdown files, Work id, explicit source | `book-preparation.tests.ps1` (50), `prepare.tests.ps1` (29) |
| T32 isolated timings, parity and fixed local work | **this package, section 7.2–7.4**: both variants, one warm-up and ten measured runs per command, full-parser parity before and after every phase |

### 7.2 Named tool targets, both variants

Method (R347–R348, R355, T32): one consistent copy of the representative archive per variant (`zt.sqlite3`, 10,895 Cards, 56,994 Link occurrences, revision 0), an isolated service per copy (`ZT_CONFIG_DIR`, `ZT_GUI_PORT=0`), one real `prepare -Variant … -Apply` per variant, one warm-up per command, **ten measured invocations** (untraced, so the seam does not add latency), nearest-rank median and p95, and a traced warm-up for the call/address/process evidence. No digest workflow, provider call or live Card write ran; `D:\zt` was only read to make the copies.

| Variant | Command | Median ms | p95 ms | Min ms | Max ms | Target ms | MCP calls | MCP processes | Addresses read | Output bytes |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| econ | `snapshot-topics` | 684.3 | 739.1 | 654.0 | 739.1 | 1,000 | 1 | 1 | 0 | 1,193,347 |
| econ | Card `plan` | 1,325.5 | 1,392.2 | 1,305.2 | 1,392.2 | 10,000 | 5 | 1 | 4 | 478 |
| econ | Card `check` | 1,323.1 | 1,371.0 | 1,302.9 | 1,371.0 | 10,000 | 5 | 1 | 4 | 59 |
| econ | Card `report` | 1,327.4 | 1,381.8 | 1,298.5 | 1,381.8 | 10,000 | 5 | 1 | 4 | 1,606 |
| econ | `prepare-link` | 4,622.3 | 5,191.5 | 4,481.3 | 5,191.5 | 20,000 | 56 | 1 | 10,899 | 241 |
| math | `snapshot-topics` | 667.5 | 713.1 | 655.7 | 713.1 | 1,000 | 1 | 1 | 0 | 1,193,347 |
| math | Card `plan` | 1,366.2 | 1,422.8 | 1,308.5 | 1,422.8 | 10,000 | 5 | 1 | 4 | 478 |
| math | Card `check` | 1,361.9 | 1,410.1 | 1,315.9 | 1,410.1 | 10,000 | 5 | 1 | 4 | 59 |
| math | Card `report` | 1,334.2 | 1,380.0 | 1,305.6 | 1,380.0 | 10,000 | 5 | 1 | 4 | 1,606 |
| math | `prepare-link` | 4,555.4 | 4,960.0 | 4,479.8 | 4,960.0 | 20,000 | 56 | 1 | 10,899 | 241 |

All ten command/variant combinations exit 0 on every measured run and meet their named target with both median **and** p95. `snapshot-topics` is one `export_titles` call and no tree or Card scan. Every Card step is one MCP process with five calls and four addressed Cards: its cost is the bounded quality read of the prepared group plus its own audit, not a whole-book scan. `prepare-link` uses one title export plus bounded text batches and reads 10,899 addresses in 56 calls, all in one process. The measured read-only phases advance no archive revision (`grep`-able in the evidence: `after_tools.archive_revision` equals `after_prepare.archive_revision` = 1).

### 7.3 Model apply, ordinary read/write, queue and lock

`model apply` (R352) is a real 66-part model (69 calls: one Topic, one formal Model Card, 66 parts and the Model Card's structure edit) applied through `zt-digest call -OperationPrefix` on the isolated copy. Ten applies, each on a fresh Topic and prefix:

| Variant | Median ms | p95 ms | Max ms | Chunks per apply | Accepted addresses | Revisions per apply | Replay revisions | Part map |
|---|---:|---:|---:|---|---:|---:|---:|---|
| econ | 446.3 | 656.2 | 656.2 | 64 + 5, all ten runs | 69 | 2 | 0 | 66 parts |
| math | 436.8 | 471.4 | 471.4 | 64 + 5, all ten runs | 69 | 2 | 0 | 66 parts |

Well inside the 30,000 ms target. The batch's own row counts are `indexed_cards: 5`, `refreshed_cards: 67`, `changed_reverse_sections: 67` — the last chunk plus the Model Card's structure refresh, not a whole-archive rebuild. A complete replay of the accepted plan publishes no revision and duplicates no Card. No `edit wait`/`edit lock held` line was written during the ten applies (each lock hold is far below the 5-second diagnostic threshold).

Ordinary Card read/write (R353), one warm `zt mcp` process:

| Variant | `get_card` median / p95 | `edit_card` median / p95 | Reply bytes | `last_write` after a write |
|---|---|---:|---|
| econ | 3.0 / 3.8 ms | 11.7 / 328.3 ms | 1,422 | `indexed_cards: 1`, `refreshed_cards: 1`, `changed_reverse_sections: 0` |
| math | 2.7 / 2.9 ms | 11.7 / 13.4 ms | 1,422 | `indexed_cards: 1`, `refreshed_cards: 1`, `changed_reverse_sections: 0` |

The econ write p95 of 328.3 ms is one outlier sample on a shared host (its median and every other sample are 11–17 ms); it is retained rather than dropped. One ordinary write parses and updates exactly the changed Card; no unrelated Card is parsed.

Queue and lock (R348's separate wait/hold evidence). A real lock holder keeps `zt.edit.lock` for 6.5 s while an ordinary write runs:

| Variant | Waiter time | `queue_wait_ms` | `held_ms` | Daemon line |
|---|---:|---:|---:|---|
| econ | 6,644.0 ms | 6,630 | 10 | `edit wait done: op=edit_card … outcome=acquired queue_wait_ms=6630 held_ms=10` |
| math | 6,644.9 ms | 6,632 | 9 | `edit wait done: op=edit_card … outcome=acquired queue_wait_ms=6632 held_ms=9` |

So the wait and the hold are attributed separately: 6,630 ms of waiting, 10 ms of holding. The bounded concurrent workload (four simultaneous identified applies) commits **4 of 4** with zero lock-failure log lines and exactly two revisions each (`revisions: 8`); no `edit wait` line appears, so no apply waited 5 s or more.

### 7.4 Fixed local work against 256 unrelated Cards (R355)

Two copies of the same consistent archive, the identical fixture (one Topic, one fixed Card, two neighbours), the identical fixed edit and the identical scoped read:

| Copy | Cards after setup | Fixed edit median / p95 | `indexed_cards` | `refreshed_cards` | `changed_reverse_sections` | Scoped read | Cards returned |
|---|---:|---|---:|---:|---:|---|---:|
| control | 10,899 | 12.7 / 13.9 ms | 1 | 3 | 0 | 3.5 ms, 4,782 bytes | 3 |
| + 256 unrelated Cards | 11,159 | 11.6 / 12.1 ms | 1 | 3 | 0 | 3.4 ms, 4,776 bytes | 3 |

The parsed-row and updated-row counts are **identical** with 260 more addresses in the archive (4 new Topics and 256 unrelated Cards), the returned Card payload hash is identical (`3ac5b904…`), and the latency does not grow. The 254-byte reply difference is only the `archive_revision` envelope of the larger copy. This is the fixed-local-work claim, measured rather than inferred.

### 7.5 A defect found and fixed by this gate

The bounded concurrent workload failed on its first qualification run: one of four simultaneous applies exited 1 with `incomplete identified output verification: archive revision changed`. The identified-output verifier re-read the whole archive's revision after checking its own outputs, so a neighbouring unit's commit refused a correct unit — the exact concurrency the declared four-writer workload is meant to allow.

The fix is test-first: `identified-batch.tests.ps1` now injects a real second-writer commit through the `ZT_RACE_AFTER_BULK_REPLY` seam between the helper's own output read and its completion (RED: `archive revision changed`), and `calls.rs` verifies the accepted outputs by completeness and fingerprint instead of a frozen global revision, with a unit test covering a changed output, a deleted output, a repeated address and a human Card (GREEN). The Link-draft preview drops the same over-strict comparison. After the fix: 132 digest unit tests, all 62 public suites and all 141 qualification checks pass, and 4 of 4 concurrent applies commit in every run.

## 8. Installation and rollback

**Preconditions.** No digest work is active. The service is stopped before replacing bytes; the archive has a consistent backup.

**Install (release).**

```powershell
# 1. Record the installed identity and keep rollback copies.
$bin = "$HOME\.cargo\bin"
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
New-Item -ItemType Directory -Force D:\zt-target\rollback | Out-Null
Copy-Item "$bin\zt.exe" "D:\zt-target\rollback\zt-$stamp.exe"
Copy-Item "$bin\zt-boot.exe" "D:\zt-target\rollback\zt-boot-$stamp.exe"
Copy-Item "$bin\zt-check.exe" "D:\zt-target\rollback\zt-check-$stamp.exe"
Copy-Item "$bin\zt-digest.exe" "D:\zt-target\rollback\zt-digest-$stamp.exe"

# 2. Build the four release artifacts from the release commit and check them in.
cd E:\zet
$env:CARGO_TARGET_DIR = 'D:\zt-target';   cargo build --release --locked
$env:CARGO_TARGET_DIR = 'D:\zt-check-target'; Push-Location tools\zt-check;  cargo build --release --locked; Pop-Location
$env:CARGO_TARGET_DIR = 'D:\zt-digest-target'; Push-Location tools\zt-digest; cargo build --release --locked; Pop-Location

# 3. Backup the archive, stop the service, replace the four files, start it.
zt status            # must report no active digest and the expected Cards
Copy-Item D:\zt\zt.sqlite3 "D:\zt\backup\zt.sqlite3.$stamp.bak"
zt down
Copy-Item D:\zt-target\release\zt.exe        "$bin\zt.exe"        -Force
Copy-Item D:\zt-target\release\zt-boot.exe   "$bin\zt-boot.exe"   -Force
Copy-Item D:\zt-check-target\release\zt-check.exe   "$bin\zt-check.exe"   -Force
Copy-Item D:\zt-digest-target\release\zt-digest.exe "$bin\zt-digest.exe" -Force
zt up
```

**Verify the installation (all four checks must pass).**

```powershell
zt version                     # zt 0.20.0
zt-digest version              # zt-digest 0.1.0
zt status                      # state: up, the daemon path is $bin\zt.exe, 10,895 Cards, 0 Sessions
zt status --check-links        # ok: true, 10,895 Cards, 56,994 occurrences, 0 faults
Invoke-WebRequest http://127.0.0.1:4717/ -UseBasicParsing            # 200
# a tokenless /api/ping must be refused with 403
```

On a scratch archive, also run one read and one identified batch smoke test (`zt mcp` `get_cards` on an empty address list, then `zt-digest call <file> -OperationPrefix <run/unit/1/build>`), so a broken MCP boundary is found before a real run.

**Rollback.** Restore the four files from `D:\zt-target\rollback` and, if the archive was migrated, restore the pre-install `zt.sqlite3` copy as well. An older binary does not maintain the derived Link index or the atomic receipts: **do not run mixed old/new writers on an upgraded archive** (R082). After a rollback, `zt status --check-links` must report `ok` before trusting the archive.

**Frozen workflows.** Reinstalling the variants is separate from the binaries:

```powershell
$asm = "$HOME\.claude\skills\workflow-framework-protocol\tools\assemble.mjs"
node $asm verify zt-econ-digest --scope project
node $asm verify zt-math-digest --scope project
# after any blueprint/module change: bump the version, then
node $asm install scripts\book-digest\workflow\zt-econ-digest.blueprint.json --sim scripts\book-digest\workflow\zt-econ-digest.sim.json --scope project
node $asm install scripts\book-digest\workflow\zt-math-digest.blueprint.json --sim scripts\book-digest\workflow\zt-math-digest.sim.json --scope project
```

## 9. Short-source failure/resume pilot package

The pilot procedure now lives in the **#90 package**, [`digest-pilot-90.md`](digest-pilot-90.md) §7: the deployment and installed identity (§3), the qualified bounded source and its persisted identity (§6), the exact `Workflow(...)` invocation, the one injected failure, the single `mode: "resume"`, and the evidence table the run must fill. This section states the frame and points there, because three details of the first draft changed when the procedure was rehearsed (`scripts/book-digest/tests/pilot-rehearsal.ps1`, 20 assertions) and must not be run from the old text:

- **The source is calibrated for the workflow's own prepare command.** The frozen prepare stage passes `-MaxChars` and `-MaxUnits` but never `-MinUnitChars`, whose default is 1000 characters per unit. The source is now 3,575 characters in three units of 1,155 / 1,181 / 1,149 (SHA-256 `d0a27cd8…`), so `prepare -Variant econ -Apply` with the workflow's own arguments yields three groups: `g01` 导论/Intro.1 (the 1937 Coase Event), `g02` 第一章/1.1/1.2 (the shared premise 「有限描述假设」 and Hart 1995), `g03` 第二章/2.1/2.2 (the formal model with its setting and result, and the 12% / 1980–1990 data sentence). The earlier draft had prepared the source by hand with `-MinUnitChars 200`, which the workflow cannot pass; that produced one group and would have left the pilot without a completed neighbour group.
- **The injected failure is a fault document, not a message.** `ledger fail` takes `-Fault <json|file>` (with matching `unit` and `input_hash`) and a class that the fault code belongs to. The deterministic injection is `check_failed` with `-Class content`, which leaves the unit `failed` until the resume; `provider_transient`/`retryable` would be retried inside the attempt budget and never reach the resume. The `-Class transient -Message …` form of the earlier draft does not exist.
- **Do not use `until: "digest"` for the failure/resume pilot.** A checkpoint stop is a legitimate cheap inspection pass, but continuing it with `mode resume` spends the run's single resume budget (`ledger resume` reports `resumed` whenever unresolved units remain), which the injected failure needs. A resume takes `{ out }` only; any identity, source, group count or Bridge selection is refused by the args gate.

The pilot still runs **only** under the user's authorization, against a fresh verified backup, with a fresh staging folder (`mode full` prepares it once; the only continuation is `mode resume`), and it never runs against `D:\zt` while a digest is active (R408). Installing binaries never interrupts a run: the pilot's binaries were installed and verified before the window, and the live archive stayed at revision 0 through that work. A full comparable book run is still the final runtime/performance qualification, not the first test.
## 10. Limits and open items

- The whole-run time, agent count and token targets of R354 are **unmeasured** and are not claimed. The measured tool targets are the five commands in section 7 plus the ordinary read/write and model-apply results; a single completed comparable run is still required for R354.
- Unix process-group behaviour is present but not exercised here; all evidence is from Windows, as before.
- `zt status --check-links` is the full-parser parity gate used throughout. It compares the derived index and every stored Reverse section with the body parser on the live copy; the qualification ran it before and after every phase.
- The host is shared and Defender scans run on this machine; the timings are isolated-copy measurements on a quiet host, not a guaranteed production latency. Every optimization is accepted by correctness parity plus recorded timings, never by a larger timeout.
- The one intentionally ignored test remains the manual real-clipboard check.



