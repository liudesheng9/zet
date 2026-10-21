# Tree Lock PRD verification (#123)

Status: verified 2026-10-21 at `bc4574e`. This is the review record for
[PRD #123](https://github.com/liudesheng9/zet/issues/123). The implementation lives in
[#124–#141](https://github.com/liudesheng9/zet/issues/141) and is not part of this document.

Sources: [implementation design](tree-lock-implementation-design.md) (SHA-256
`fe641bcab7f271118d77838c639643e87d25e87cdacd3ee892c75ac4d61fd14a`, verified byte-for-byte),
[proposal](tree-lock-proposal.md), [issue map](tree-lock-issue-map.md).

## Baseline

- `bc4574e` on `main`, equal to `origin/main`, 0 ahead and 0 behind after `git fetch origin`.
- `Cargo.toml` is 0.21.0. The PRD targets 0.22.0 when its slices ship; this review ships no feature
  code and no version bump.
- `src/` is unmodified. The working tree also holds the unsettled idea-grow changes that the design
  names: `tools/zt-digest/src/grow.rs`, its `main.rs` module line, `scripts/idea-grow/`, the
  `zt-idea-grow` Workflow and frozen install, its wiki docs, and `grow.tests.ps1`. Those stay
  uncommitted (see Risks).
- The tree-lock documents are pinned to LF in `.gitattributes`, like the frozen Workflow scripts, so
  the design's SHA-256 is the same in every checkout.

## Tracker

- All 19 issues (`#123`, `#124`–`#141`) are open, each with exactly `enhancement` and
  `ready-for-agent`.
- Every live body reproduces the body SHA-256 published in the issue map: 19 of 19, byte-exact.
  The parent body is unchanged since publication (its `updatedAt` is the publication time).
- Every child names `#123` as its parent and cites the design SHA-256. The design file in this
  repository hashes to exactly that value.
- Child titles, blocker sets, user-story sets and acceptance-criteria counts match the map's table.
  The 144 acceptance criteria, 46 user stories and 36 named scenarios (T01–T24, D01–D12) are each
  owned exactly once.
- The 415 source clauses R001–R415 are each embedded in exactly one child, and each map row still
  points at the stated design line. 13 clauses are code, SQL, JSON or diagram blocks restated as
  behavioural contracts, as the map records; the other 402 keep their source wording.
- Every blocker is a lower-numbered open child, so the published dependency order is acyclic and
  topological.

## Architecture claims

Each claim in the design's §1 and §3 integration table was checked against the current source.

| Design claim | Check |
| --- | --- |
| `open_db` opens SQLite directly; bootstrap opens its own connection and runs ordered upgrades (§3) | Holds: `open_db` (`src/main.rs:1955`), `initialize_database` opens its own connection (`src/main.rs:1251`) |
| The three Card rebuilds copy `row_id`, then drop and rename the table (§3) | Holds: `ensure_open_model_parts` (`1466`), `ensure_hub_cards` (`1543`), `ensure_finding_cards` (`1592`) copy `row_id`; `DROP TABLE cards` and rename at `1492`, `1570`, `1619` |
| `card::authored_fingerprint` hashes parsed title/body, BibTeX and type fields, not generated Reverse links (§1, §3) | Holds: `src/card.rs:94` |
| Agent Access refuses human writes and human Link rewrites; Owner bypasses only that rewrite restriction (§3) | Holds: `src/workspace.rs:66-115` |
| Shell and Session can bypass `ops`; insert uses direct SQL; moves use `__moving__<old>` (§3) | Holds: `src/place.rs:138`; `__moving__` in `src/main.rs` |
| GUI and MCP share transaction-level writers and destructive plans in `ops` (§3) | Holds: `edit_tx`, `create_literature_tx` and the plans in `src/ops.rs` |
| Model, hub and math metadata setters UPDATE Cards directly (§3) | Holds: `src/model.rs:208`, `:363`; `src/hub.rs:85`; `src/math.rs:143` |
| Batch identified writes keep caller-owned transactions, receipts, a rollback preview and fail-fast order (§3, §8.3) | Holds: receipt insert `src/batch.rs:628`, `tx.rollback()` `:686`, early `?` returns in the apply loop |
| `link_index` owns authored Link occurrences, generated Reverse links and revision helpers (§1, §3) | Holds: `revision` `src/link_index.rs:18`, `bump_revision` `:32` |
| MCP has scoped and lean text readers as well as full Snapshots (§3) | Holds: `read::cards(..., text)` `src/read.rs:436`; `get_cards` view `full|text` `src/mcp.rs:361` |
| `Snapshots` caches on SQLite `data_version`; the displayed fingerprint hashes Card data (§3, §9.3) | Holds: `src/gui.rs:619`, `:687`; `Snapshot::fingerprint` `src/ops.rs:596` |
| Reply writes a human Card through `create_successor_tx` and refreshes the agent target's Reverse links (§3) | Holds: `src/reply.rs:51`, `:64`, `:126` |
| A delete re-points Links, deletes Cards, compacts Side successors and follows Spreads in one transaction (§3) | Holds: `apply_delete_plan` `src/main.rs:3421-3453`, `spread::follow` `:3446` |
| `cmd_clear` deletes Cards, rebuilds Links and resets `next_topic_id`, keeping other metadata and files (§3, §12.4) | Holds: `src/main.rs:1835-1867` |
| `dump` writes named payload entries and checks compression (§3, §14.3) | Holds: manifest `src/dump.rs:19`, artifacts `:586`, `CompressionFormat` `:176` |
| Session registration has its own `zt.sessions.lock`; the edit lock is `zt.edit.lock` (§1, §3, §12.4) | Holds: `src/main.rs:391`, `:395` |
| No Lock table, trigger, module or route exists yet (§1, §5, §10.1) | Holds: no `src/tree_lock.rs`, no `CREATE TRIGGER` in `src/`, no `/api/lock` route |
| The SQL wrappers need the `functions` feature; SHA-256 is already a dependency (§5.3) | Holds: `rusqlite` enables only `bundled`; `sha2 = "0.10"` |
| The new GUI routes fit the existing token-guarded route set (§10.1) | Holds: `/api/locks`, `/api/lock`, `/api/unlock`, `/api/lock/note` are unused; the token header is `X-ZT-Token` |
| Keep-root can start again at `3/1` or `<citation-key>/1` with the existing successor rule (§12.3) | Holds: `direct_successor` `src/main.rs:2214` |
| Every document named in the release checklist exists (§19) | Holds: 13 of 13 paths present |

Two name/location nuances do not contradict the design, which names behaviour:

- `Snapshot::fingerprint` is defined in `src/ops.rs` on the `Snapshot` type, not in `src/gui.rs`.
- `update_literature_metadata` lives in `src/main.rs`; the archive-revision helpers live in
  `src/link_index.rs`.

## Clarification recorded for S01

`zt_authored_fp(text, bibtex, card_type, model_facet, model_emphasis, model_level, hub_level,
math_form)` must produce exactly the digest of `card::authored_fingerprint`. That function hashes
**one** level slot: `Card::level_id()` returns the Model level for a Model Card and the Hub level
for a Hub Card. The SQL wrapper therefore consumes both columns but hashes their non-null value
once (`COALESCE(model_level, hub_level)`), never two separate fields. Hashing both would diverge
from the Rust digest, and the six triggers would then refuse the generated Reverse-link refresh of
a locked Hub Card (R113, S01-AC07, S01-AC08). The design's "preserve its output" and S01's "share
existing fingerprint framing exactly" already require this; the parenthesised argument list only
says which columns the wrapper receives.

## Baseline tests and tooling

Run from `E:\zet` on 2026-10-21.

| Command | Result |
| --- | --- |
| `cargo test --all-targets` | 22 test binaries, 527 passed, 0 failed |
| `cargo test --manifest-path tools/zt-check/Cargo.toml` | 23 passed, 0 failed |
| `cargo test --manifest-path tools/zt-digest/Cargo.toml` | 328 passed, 0 failed |
| `pwsh -NoProfile -File scripts/book-digest/tests/run-all.ps1` | 86 tests, 0 failed (see the deployment finding below) |
| `node scripts/book-digest/tests/framework-harness.mjs` | 291 `ok` checks, `failed: 0` |
| `node scripts/book-digest/tests/bridge-workflow-harness.mjs` | 2 `ok` checks, exit 0 |
| `node %USERPROFILE%/.claude/skills/workflow-framework-protocol/tests/run-tests.mjs` | 28 passed, 0 failed |
| `node scripts/issue-dev/tests/issue-relay.tests.mjs` | 23 passed, 0 failed |
| `node %USERPROFILE%/.claude/skills/project-wiki/scripts/wiki.mjs check` | "the wiki is current": 17 blocks, 3 PENDING for the unsettled idea-grow code and docs, every other block FRESH |

The wiki PENDING entries (`digest-scripts`, `idea-grow-workflow`, `zt-digest`) come from the
uncommitted idea-grow changes touching code and docs together; the check itself passes. No
tree-lock code exists yet, so no block is stale for this PRD.

## Deployment finding

The first full digest-harness run had two failures, `pilot.tests.ps1` and
`ledger-preparation.tests.ps1`, not caused by the sources: the installed `zt-digest.exe` (built
2026-10-21 01:00) still embedded the previous `zt-econ-digest` blueprint (5.10.0) through
`include_str!`, while the tree at `bc4574e` carries 5.10.2 (blueprint written 02:57). The pilot
refusal stated it exactly: "the installed `zt-econ-digest` is 5.10.2, but the prepared contract
froze 5.10.0". `zt-digest 0.5.3` and `zt-check 0.1.0` were rebuilt from `bc4574e` and installed, and
both tests then passed; the table above is the clean re-run against those binaries. This was a
stale installed executable, not a source defect: it is exactly the deployment check the PRD asks
for, and it is fixed by the redeploy recorded here.

## Deployed binaries and service

- `cargo build --release` at `bc4574e` built `zt.exe` (8,854,016 bytes) and `zt-boot.exe`; both
  were installed into `~/.cargo/bin` (the previous files are kept as `*.old-<timestamp>`).
- `zt down` then `zt up` restarted the live service on the new binary: pid 56372, `zt 0.21.0`,
  GUI `http://127.0.0.1:4717/`, archive `D:\zt`, revision 4537, 161 Cards.
- `zt status --check-links` returns
  `{"ok":true,"faults":[],"cards":161,"links":680,"archive_revision":4537}`.
- MCP smoke test on the same binary: `initialize` reports `zt 0.21.0`, `tools/list` returns 29
  tools.
- `zt-check` 0.1.0 and `zt-digest` 0.5.3 were rebuilt from `bc4574e` and installed beside `zt`
  (`zt-digest.exe`, 25,961,984 bytes, now embeds the 5.10.2 econ and 4.10.2 math blueprints).
- `node scripts/issue-dev/repo-state.mjs --repo liudesheng9/zet --cwd E:/zet --issue 123` reports
  `pushed: true`, `zt.match: true`, `service: up`; `clean` is false only for the uncommitted
  idea-grow work (and this review's own files before the commit).

## Risks owned outside this PRD

- The idea-grow changes are uncommitted and untracked, and no GitHub issue owns them. S15 and S17
  depend on that code, and every later issue's repo-state check reports `clean: false` until the
  owning work is committed.
- Release 0.22.0 is not cut. The slices must bump the version and run the design's release
  checklist (§19).

## Result

The published plan and the 18-child map are correct and current; every architecture claim still
holds; the baseline tests and Workflow tooling pass; the installed binaries and the service run the
build from `bc4574e`. No design or tracker body was changed. Implementation continues in
`#124`–`#141`.
