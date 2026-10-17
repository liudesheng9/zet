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

## Remaining slices

#66–#89 remain implementation/isolated qualification work. #90 and #91 retain their live-workflow and Stiglitz acceptance gaps until the user performs those runs. The service/executable publication and deployment status must be checked against actual installed hashes and running process paths before the parent can be reported complete.
