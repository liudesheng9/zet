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

## Remaining slices

#67–#89 remain implementation/isolated qualification work. #90 and #91 retain their live-workflow and Stiglitz acceptance gaps until the user performs those runs. The service/executable publication and deployment status must be checked against actual installed hashes and running process paths before the parent can be reported complete.
