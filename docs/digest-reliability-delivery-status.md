# Digest reliability implementation and verification

This records implementation evidence for [PRD #64](https://github.com/liudesheng9/zet/issues/64) and the [27-slice issue plan](digest-run-stiglitz2014-issue-plan.md). The publication coverage JSON describes requirements coverage, not implementation completion.

## Scope

The authorized work is implementation and verification of the tools, compilers and workflow integration. Actual book-digest workflow execution and Stiglitz recovery remain for the user's later Claude Code run. Do not infer those results from isolated tests. The production archive and running installation remain unchanged until a separately verified deployment step.

## #65: derived Link index

Implemented in `src/link_index.rs` and the existing write paths. Authored bodies remain authoritative. Ordinary edits validate referenced targets, replace one source's occurrence rows and refresh its old/new targets. Structural changes retain a full rebuild within their existing transaction. The index preserves repeated occurrences, relation kinds, Broken links and existing ordering. Metadata/text edits commit atomically with one revision; unchanged saves do not advance it.

Migration runs even for existing Markdown archives, makes an integrity-checked SQLite backup and installs its marker/index transactionally. A malformed Card names its address and rolls back. `zt status --check-links` compares the index and generated text with the full parser; `zt reindex` repairs derived data explicitly. Lock sharing violations are classified as busy and retried without removing a live holder's lock.

Verification on Windows, 2026-10-17:

- Public MCP/CLI tests cover migration rollback/retry, no repeated migration on restart, scoped updates, duplicate occurrences, title changes, metadata revisions, no-op saves, move/insert/rename/delete/repoint, human authorship protection, explicit corruption recovery and Windows sharing-lock retry.
- `cargo test --all-targets --locked -- --test-threads=4`: 325 passed, zero failed; one deliberately ignored manual real-clipboard test. After the unchanged-save and diagnostic-detail additions, both affected integration suites passed again (67 CLI + 16 Workspace/MCP tests). The final atomic Topic-allocation refactor passed all 171 root unit tests and all 16 Workspace/MCP tests.
- Checker baseline: ten library tests passed. This slice does not change the checker.
- Full root log: `D:/zt-target/qualification/issue65-root-tests.log`; final affected-suite log: `D:/zt-target/qualification/issue65-final-tests.log`; transaction regression log: `D:/zt-target/qualification/issue65-transaction-tests.log`.
- [Isolated archive-copy measurement](digest-link-index-benchmark.json): 10,895 starting Cards, persistent MCP, ten edits after warm-up. Installed release baseline median 1,311 ms / p95 1,442 ms; new debug build median 20.4 ms / p95 22.0 ms. One source indexed, two Cards refreshed, one generated section changed; full integrity passed. These are local edit measurements, not a workflow throughput claim.

Deployment must first verify no active digest work, inventory the current installation, retain a consistent pre-upgrade backup and compare authored content/metadata after migration. Old writers do not maintain the index: downgrade requires the compatible pre-upgrade archive and binary, or a separately qualified conversion. Do not mix old/new writers on an upgraded archive.

## #66: indexed and bulk read projections

`src/read.rs` selects a consistent per-request view through indexed queries and the existing `Snapshot`, model/math and hub calculations. It adds `get_cards` full/text views, `get_tree` bodies and `export_titles`; preserves missing-address and duplicate ordering; enforces 200 addresses / 8 MiB of encoded tool-result data with explicit `read_limit` errors. Tree selection preserves case, treats `_` literally and returns ZET order. Global hub policy streams metadata/title lines, Hub bodies and the Link index; ordinary bodies are not read for that policy. There is no persistent read cache.

Verification on Windows, 2026-10-17:

- Full root regression before the final model-scope refinement: 335 passed, zero failed; one intentionally ignored manual clipboard check. After refining the scope, 45 projection/model/math/hub tests passed (26 Workspace/MCP tests plus 19 domain integration tests). The final bulk-limit and mixed-model union checks are included in the affected-suite log.
- The public GUI's unchanged full Snapshot supplies the oracle for every Card field and complete model structure in a shared, cross-tree math fixture. This includes derivation chains, filed/also owners, external/cited premises, reach-in faults, neighbours, human discussion, hub membership, load, theories and study order. A neighbour's unrelated model cannot force a read into another model's body.
- Public tests also cover exact tree selection, numeric ordering, repeated Broken occurrences, Workspace filters, default/full/text bulk projections, explicit missing reads, all three reply-size limits, and body/revision consistency while another MCP process performs 20 edits.
- `zt-digest` baseline: 80 unit tests passed. Checker baseline: ten tests passed. These baselines do not claim the remaining compiler changes are implemented.
- Logs: `D:/zt-target/qualification/issue66-root-tests.log`, `issue66-projections-final.log`, `issue66-bulk-final.log`, and `baseline-digest-tests.log`.

The [release-to-release isolated measurement](digest-indexed-read-final-benchmark.json) uses consistent copies of the same 10,895-Card archive and one persistent MCP process per measurement. Each existing tool has one warm-up and ten measured calls. Normalized projection hashes match the installed full-Snapshot baseline for all six tools; only the new tree completeness/revision fields and intended tree ordering are normalized.

| Tool | Before median / p95, ms | After median / p95, ms |
|---|---:|---:|
| `get_card` | 223.0 / 229.9 | 1.58 / 1.68 |
| `get_tree` | 309.0 / 347.3 | 2.03 / 2.33 |
| `list_trees` | 691.6 / 721.5 | 43.4 / 48.9 |
| `list_links` | 882.3 / 937.4 | 736.6 / 778.7 |
| `list_workspaces` | 355.6 / 378.3 | 69.3 / 81.1 |
| `get_model` | 330.5 / 349.5 | 28.8 / 32.3 |

The global Link result is still 7.7 MB of structured JSON, so its cost remains substantial. Complete title export of all 10,895 Cards was observed at 249 ms, and a two-Card text read at 2.45 ms; these are single observations, not percentile estimates. [Hub parity](digest-indexed-hub-parity.json) additionally matches the full baseline hashes for `get_hub` and `list_hubs`; their single measured observations after warm-up were 302 → 161 ms and 20.2 → 3.01 seconds. No live Card writes or actual digest workflows were run.

## #67: bulk digest clients (in progress)

The first implementation step switches `snapshot-topics` to one fresh title export per group and `prepare-link` to one title export plus bounded text batches through one MCP process. Existing rendering, ownership, compact excerpts and existing-Link exclusion are preserved. The ranking corpus still requires one bulk text pass over the candidate catalogue. The lean MCP text view now provides the existing excerpt/edge/Link/kind/parent facts; it does not calculate models or hubs.

The shared checker client chunks at 200 addresses, reduces chunks after a structured reply-limit error, and rejects missing, reordered or changing-view inputs. Public preparation tests observed each old failure before its fix (per-tree snapshot reads, per-Card link reads, loss of linked-target exclusion, and lack of chunk adaptation). The latest preparation suite also verifies missing input fails explicitly. All 27 Workspace/MCP, 80 digest unit and ten checker unit tests pass for this step. Logs are under `D:/zt-target/qualification/issue67-*.log`.

This slice remains open for retained-archive command timing qualification. The fixture checks below do not establish the final shared orphan-definition gate assigned to #76. It does not yet satisfy all #67 acceptance criteria.

The next implementation step adds owned-manifest group checks for the normal `-NoOrphans` group cycle, using required parents and the book term table and labelling global checks `not_run`. Public tests pass for identical requested R4/R6/L1/L2 fault lines against the full-book path, one MCP process, no unrelated tree reads, and missing Card/manifest refusal. Every standalone live checker mode now uses lean batches or its complete model context, preserves its fault rules and checks revision consistency. Filed members share one primary model read; unfiled shared members are resolved independently.

After each observed pre-fix failure, the public group/checker tests pass. The final relevant regression run passes all six suites: preparation (27 assertions), scoped inputs (17, then 18 after adding nonzero Link-fault parity), Card compiler (57), model compiler (59), and both language/layer fixture suites. Ten checker and 80 digest unit tests also pass. Logs: `D:/zt-target/qualification/issue67-group-checker-suites/` and `issue67-check-*-red/green.log`. These verify the delivered partial changes; #67 stays open for its remaining whole-book scope and timing requirements.

The global audit now reads its required complete graph through a fresh title export and bounded lean batches, reuses those bodies, and reads model contexts only for the selected report scope. It computes inbound facts from the indexed links in those complete rows, retains both Workspaces' Bridge endpoints, and uses one MCP client. The worker/null-fallback path is removed; legacy worker flags remain accepted for wrapper compatibility. Successful reports record completeness and their checked revision. Missing graph/model input or an actual edit between graph and relationship reads fails explicitly before publishing a completed report.

Frozen public report and fault fixtures were captured from `fda5cf1` before this audit migration. The new output matches every previous report field and exact fault line after removing only generation/time and the new completeness/revision fields. Tests also verify requested model scope and failure behavior. All 80 digest unit tests and five affected public suites pass: audit (17 assertions), Card compiler (57), scoped inputs (18), Organize/skeleton (19), and Bridge planning (17). Logs: `D:/zt-target/qualification/issue67-audit-*.log` and `issue67-audit-suites/`.

The whole-book Card cycle now uses the same scoped quality reader as local groups. It bulk-reads the book, declared owned Topic Cards and already-attributed Topic members. Indexed relationship addresses provide incoming references and membership; necessary external Idea/Finding carriers provide kinded inbound facts. Navigation anchor bodies and parent bodies are not required. Fresh catalogue titles retain parent context in job files. Context rows cannot become writable work or orphan candidates. The public global audit is no longer invoked for Card planning/check/report.

Declared owned Cards remain checked even when their source attribution is missing, so missing-source/term faults are visible. Human ownership, another Literature tree, unreadable declared staging, a missing manifest/body/carrier or a changing revision stops the cycle explicitly. Public RED/GREEN tests demonstrate the removed global body read, unnecessary navigation/parent body reads, foreign ownership and silent staging omission. Fixture language/Link faults and orphan/Event quality faults agree with the separate global audit, with the latter comparison checking equal revisions.

All 80 digest unit tests pass. The final caller validation passes Card compiler (57 assertions), Organize/skeleton (19), audit (17) and scoped inputs (31). The first parallel run had a PowerShell `OutOfMemoryException` and one MCP child closure before the new book assertions; Windows event 2004 recorded virtual-memory exhaustion at 16:50/16:53. Those failures are retained in `issue67-book-suites/`. After host memory recovered, the two failed suites passed sequentially without a production-code change (`issue67-book-retry-sequential/`). Additional RED/GREEN logs are `issue67-book-*-red/green.log` and `issue67-parent-context-*.log` under `D:/zt-target/qualification/`.

Preparation of the retained-data timing comparison found one remaining full-view assumption: structure/index ownership inference still tested `outbound.length`, which the lean view does not expose. A public regression reproduced the resulting false group ownership. The lean view now supplies the same distinct-target count, including Broken targets but excluding repeated/fenced occurrences, from its existing index query; preparation requires that count. A public MCP test verifies parity after a confirmed target deletion, and the real preparation test retains the eight-target exclusion. All 27 affected Workspace/MCP tests, 28 preparation assertions and 80 digest unit tests pass. Logs: `D:/zt-target/qualification/issue67-outbound-count-*.log`, `issue67-structure-owner-*.log` and `issue67-structure-owner-unit.log`. This is a source/behavior correction required before timing qualification, not a claim that the timing target has passed.

### Retained-data command qualification

The comparison exposed an ordering regression in the title adapter: the title API's numeric Topic order changed the former digest rendering order. The public preparation test first failed for multi-digit Topic ids, then passed after the adapter restored binary tree/address input order before the existing rendering/ranking sorts (29 preparation assertions). The API's numeric ZET order is unchanged.

The completed [command evidence](digest-bulk-command-benchmark.json) records the matching installed baseline client/server and candidate builds, hashes, request counts, payload-row counts and preserved artifacts under `D:/zt-target/qualification/issue67-command-bulk/`. Each release pair reads a separate consistent copy of the same 10,895 Cards, including the 1,565-Card `stiglitz2014learning` Literature tree. The 171 group ownership manifests are synthetic inputs over those retained Cards; no source extraction, provider call, apply or actual workflow runs. Both isolated services have been stopped, and both Card row hashes, revisions and the original fixture file hash remain unchanged.

| Command | Baseline observation, seconds | Candidate median, seconds | Calls before → after | MCP processes before → after |
|---|---:|---:|---:|---:|
| snapshot-topics | 50.317 | 0.592 | 219 → 1 | 1 → 1 |
| Card plan | 106.175 | 2.624 | 1,669 → 18 | 11 → 1 |
| Card check | 101.753 | 2.423 | 1,669 → 18 | 11 → 1 |
| Card report | 105.316 | 2.470 | 1,669 → 18 | 11 → 1 |
| prepare-link | 555.258 | 7.963 | 10,898 → 56 | 12 → 1 |

Each candidate command has one warm-up and three measured samples. The baseline has one observation per command. Some source/build/test work overlapped the long baseline preparation run, so these are observed developer-host timings, without a percentile or exact causal speedup claim. All candidate samples meet the named local command latency targets. They do not establish whole-workflow throughput.

The snapshot matches byte for byte. All Card steps match the selected 1,445-Card state, complete jobs and 83 fault lines; the complete report JSON also matches. All 343 link-preparation files (3,991,649 bytes) match byte for byte, including candidate ranking and ownership. The initial attempt to run the old client against the new server failed on a 9.9 MB tree reply exceeding the new 8 MiB limit; that failure and the earlier ordering mismatch remain retained rather than being counted as successful measurements.

T32 is qualified for these commands. #67 remains open for its T22 gate: the shared orphan predicate and exact revision-scoped sets assigned to #76 are not established by this timing comparison.

### Executable testing and host incidents

Windows Defender quarantined the temporary test copy and subsequently blocked the debug build itself as `Trojan:Win32/Bearfoos.A!ml`. The [investigation](windows-defender-build-investigation.md) records the events, source/build review, artifact hashes and limits of the diagnosis. The leading explanation is a development-build false positive; the classifier's exact cause is not established.

The unnecessary hidden PowerShell/`ExecutionPolicy Bypass` daemon launcher has been replaced with direct Windows process creation, explicit path arguments and no inherited handles. The public no-shell/path regression failed before the change and passes afterward. The full root regression passes: 338 tests, zero failures, one deliberately ignored real-clipboard test. Both final debug and release binaries pass Defender custom scans; real-time protection and behavior monitoring remain enabled. No exclusions or security settings were changed.

The interrupted scoped integration test was rerun through the executable boundary, reached its intended pre-fix `--cards` assertion, and then passed after the checker migration described above. Logs: `D:/zt-target/qualification/defender-native-*.log` and `issue67-check-cards-red/green.log`.

Defender later recorded another detection at 16:47 on a temporary copy whose SHA-256 matches the rebuilt debug executable exactly. That copy and the current build were readable at 16:55. The launcher change and clean scans are not a proven permanent antivirus fix; the decisive classifier/context signal remains unknown. Protection remains enabled. The separate host memory incident and successful sequential retries are described above and in the investigation document.

The installed service was last checked up at PID 77700, archive `D:/zt`, 10,895 Cards, zero Sessions. It has not yet been upgraded; final deployment remains part of the unfinished delivery.

## #68: atomic batch archive boundary (in progress)

The MCP `apply_batch` path supports Topic/Literature/successor creation and content/type/model/math metadata edits through the same transaction-level validators/writers as ordinary writes. One edit lock/transaction covers all unseen items, the combined Link/Reverse-link update, one Card revision and authoritative `applied_ops` receipts. Limits are 64 items and 1 MiB of authored inputs, including base text. Destructive operations retain their separate confirmed path.

Each receipt keeps the required stable key, canonical request hash, UTC commit time and exact result with before/after authored fingerprints. Canonical creation defaults use the same MCP argument parsers, including empty/direct successor kinds and absent/null metadata; JSON object order and CRLF/LF transport changes do not create new operations. Hashes cover local references and their resolved bindings. A matching retry supplies original addresses to later items; a rebound reference or changed payload conflicts. Receipts do not expire, and replays do not recreate outputs that someone subsequently changed or deleted. A compiler must reread and verify those outputs before completion.

Ten public MCP regression scenarios demonstrate rollback of earlier creates/Topic allocation, new-client replay, changed-key payload refusal, local-reference binding integrity and replay-prefix extension, canonical defaults, Literature creation, guarded text/type edits and fingerprints, model level/math form, limits/identity/Access/destructive refusal, and all 64 sources in a combined refresh with one revision. Each new behavior failed before its fix. The full root regression passes 348 tests, zero failures, with one intentionally ignored manual clipboard test (`D:/zt-target/qualification/issue68-root-regression.log`). After the final metadata-default refinement, all 37 Workspace/MCP tests pass again (`issue68-mcp-final.log`); focused RED/GREEN evidence is `issue68-*.log`.

The next step adds `call -OperationPrefix run/unit/plan-revision/step` and the shared `invoke_identified` client. One MCP process applies deterministic chunks capped by both 64 items and 1 MiB, recovering cross-chunk local references from authoritative receipts. Unsupported writes/forward references/oversized individual items are refused before writing. The mode explicitly refuses incompatible legacy guard/resume/log options. It rereads the final output per address through bounded text batches, verifies authored fingerprints and revision consistency, and reports success only when those outputs still match. An independent edit remains intact and makes completion fail.

The public process-failure test builds a 66-part model with 69 operations. A real MCP child exits after the first 64 operations commit but before the helper receives the reply. A new helper process replays that prefix and completes the five remaining operations, preserving every original address and publishing exactly two revisions. A complete later replay publishes none. This proves two archive chunks and checked outputs, not whole-model transactional atomicity or workflow completion.

The `terms` compiler now offers the same explicit prefix mode. It retains an immutable accepted create/guarded-edit plan before the archive call, so a lost-create reply cannot make retry change the operation into an edit. A changed input needs a new accepted plan revision. Receipts remain the sole commit authority; the retained file describes accepted input. The compiler rereads outputs before reporting success and returns the receipt keys. The shared pure canonical/hash module was moved from `bridge/canon.rs` to `canon.rs`; Bridge behavior is preserved. Atomic no-clobber accepted-plan publication uses the same `tempfile` 3.27 dependency already used by the root crate.

All 80 digest unit tests and four affected public suites pass: term compiler (11 assertions), Bridge compiler (39), identified writes/compiler recovery (17), and helper calls (exit 0). Logs: `D:/zt-target/qualification/issue68-identified-suites/` and `issue68-identified-unit-final.log`. Public RED/GREEN logs for the initial helper, historical-receipt verification and actual compiler adoption are retained under `issue68-*.log`.

#68 remains open for model-compiler adoption and its manager/compiler lifecycle gate, which also depends on the later unit/lease work. No live installation or production archive migration has been performed.

## Remaining slices

#67–#89 remain implementation/isolated qualification work. #90 and #91 retain their live-workflow and Stiglitz acceptance gaps until the user performs those runs. The service/executable publication and deployment status must be checked against actual installed hashes and running process paths before the parent can be reported complete.
