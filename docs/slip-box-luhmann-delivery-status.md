# Slip-box Bridge delivery receipt

Status: source and installed delivery, 2026-10-16. The user explicitly deferred real tests to Claude Code. Real installed Workflow qualification and accepted live apply remain open; no further real pilot or live Bridge apply is part of this delivery.

Scope: PRD [#50](https://github.com/liudesheng9/zet/issues/50), source slices #51–#61 and the installation portions of #62. [The implementation specification](slip-box-luhmann-implementation-design.md) remains the contract; [the coverage index](slip-box-luhmann-issue-coverage.md) maps its sections to issues. Source tests and installation do not complete #62, #63, the full PRD or math L6 runtime qualification.

## Implemented structure

The production addition is four files:

| File | Responsibility |
| --- | --- |
| `scripts/book-digest/book-of.ps1` | Pure shared classification and heuristic book attribution. |
| `scripts/book-digest/bridge-compiler.ps1` | Bounded frozen planning, one validator, immutable reports, guarded accepted edits and retry journal. |
| `scripts/book-digest/dsh-bridge-instructions.zh.md` | One judge contract for book and math profiles. |
| `scripts/book-digest/workflow/modules/zt-bridge.js` | Connect the compiler to the existing Workflow drivers and manager pool. |

Focused changes connect these files to the existing audit, handwritten book adapter, frozen economics/math blueprints, result module, protocol and authoring guidance. There is no Rust change, schema migration, new Card type, ownership column, stored graph, embedding/approval service, GUI or MCP tool. Links remain ordinary reasoned Links; reports and retry state remain local files.

`plan.json` freezes inputs. `proposals.json` freezes review content and its report id. `state.json` records acceptance inputs, intent and verified progress. An open file handle serializes apply per report; `edit_card` uses the exact current raw `base`. Apply never substitutes `append_card`, relocates by title, broadens accepted ids or rebuilds a model to make a proposal valid.

Final recovery refinements were established with failing fixtures before fixes: resume recreates missing job inputs after an interrupted plan render from the same frozen snapshot; fully verified attempts retain their resulting inferred book alongside the intended body fingerprint, because an accepted Link can change the heuristic vote. Repeat apply recognizes its own verified change without accepting unrelated prose. Publication refuses both conflicting sentences for one directed pair; the first cannot silently survive the second. Missing work stays failed/refused rather than becoming no relation.

## Source acceptance by issue

These are source-level acceptance results. The GitHub completion comments record the exact publication commit and closure. Real runtime gates remain separate below.

| Issue | Verified contract and principal evidence |
| --- | --- |
| [#51](https://github.com/liudesheng9/zet/issues/51) | Insertion, Placement, whole-Card Links and writable-only splitting agree with existing `insert_card`; core operation tests pass. Installed authoring copies match. |
| [#52](https://github.com/liudesheng9/zet/issues/52) | Weighted distinct votes, ordinal ties/identities, reordered inputs, human exclusion and categories including Finding. Pure helper and audit fixtures. |
| [#53](https://github.com/liudesheng9/zet/issues/53) | Standalone existing-book planning, complete reads, priorities, term-name retrieval, snapshots, caps, job bounds and interrupted resume. Compiler/planning fixtures. |
| [#54](https://github.com/liudesheng9/zet/issues/54) | BR1–BR9, carrier/direction matrix, R6 setting/book priority, terminology availability, immutable reports, reconciliation and failed jobs. Compiler/offline validation fixtures. |
| [#55](https://github.com/liudesheng9/zet/issues/55) | Report-bound selection, preview, exact accepted paragraphs, raw-base CAS, target preservation, Reverse links, stale endpoints, lock contention and post-write verification. Compiler/human/math fixtures. |
| [#56](https://github.com/liudesheng9/zet/issues/56) | Expected/intended/neither-state recovery, partial progress, incremental ids, external existing Links, `written_unverified`, idempotence and journal-proven attribution. Compiler/planning/attribution-retry fixtures. |
| [#57](https://github.com/liudesheng9/zet/issues/57) | Unit selection, no-unit success, Definition specialization, exact earlier-to-unit direction, separate acceptance, unit load and preserved Model parts. Math/validation fixtures; L6 live gate remains open. |
| [#58](https://github.com/liudesheng9/zet/issues/58) | Actual distinct semantic pairs, incoming/outgoing book/human/unattributed buckets, part inclusion, navigation exclusions, proposal-only isolation and unknown incomplete reads. Audit/human/planning fixtures. |
| [#59](https://github.com/liudesheng9/zet/issues/59) | All three maintained paths: Figures → report-only Bridge → Hubs, Bridge mode, skip conditions, bounded roles, cleanup/retention and visible failures. Workflow/framework/Bridge harnesses. |
| [#60](https://github.com/liudesheng9/zet/issues/60) | Apply dispatches only the compiler before source probing; exact separate selections and earlier-only apply; no judge, Organize, Hubs or model work. Harnesses and compiler fixtures. |
| [#61](https://github.com/liudesheng9/zet/issues/61) | Four-file footprint, versions/preflight, assembler installation/verification, prior histories, named tests, same-commit owning wiki and operator documentation. |

Baseline: `51ba12d2460cb4d4bc1a8828f6f6798174ad7cf6`. The approved implementation specification remains unchanged, SHA-256 `e5ed0b6249e7bf4a41dead07ed02ce68177ac162df77ce16f2837ed9c2b15f8f`.

## Automated verification

Every mutating fixture uses a new temporary `ZT_CONFIG_DIR` and `ZT_GUI_PORT=0`. Judge answers are hand-authored. The Human fixture seeds only its stopped, empty scratch database; tested behavior uses public compiler/MCP operations. Fixtures make no Card in the active archive and require no paid model call.

Commands run from the repository root and exit 0. Logs are local, ignored files under `target/luhmann-delivery/`; historical `*-red*` logs are intentional failing TDD evidence, not release results.

| Command | Release evidence |
| --- | --- |
| `cargo test --all-targets --locked` | `cargo-test.log`: 318 passed, zero failed across 13 suites; the pre-existing manual real-Windows-clipboard test remains ignored. |
| `cargo build --release --locked` | `cargo-release.log`; resulting executable installed as the service binary. |
| `pwsh -NoProfile -File scripts/book-digest/tests/book-of.tests.ps1` | `book-of-final.log` |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-compiler.tests.ps1 -Zt E:/zet/target/release/zt.exe` | `bridge-release.log`; exact release binary copied into scratch fixture. |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-planning.tests.ps1` | `planning-render-green.log` |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-validation.tests.ps1` | `validation-release.log`; no service connection. |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-attribution-retry.tests.ps1` | `attribution-retry-green.log` |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-math.tests.ps1` | `math-members-final.log` |
| `pwsh -NoProfile -File scripts/book-digest/tests/bridge-human.tests.ps1 -Python C:/Users/chenx/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/python.exe` | `human-expanded.log` |
| `pwsh -NoProfile -File scripts/book-digest/tests/digest-audit.tests.ps1` | `audit-final.log` |
| `pwsh -NoProfile -File scripts/book-digest/tests/requires-version.tests.ps1` | `requires-version-release.log`; new compiler detected automatically. |
| `pwsh -NoProfile -File scripts/book-digest/tests/zt-calls.tests.ps1` | `zt-calls-final.log`; UTF-8 under GBK. |
| `node scripts/book-digest/tests/workflow-harness.mjs` | `book-release.log`, zero failures. |
| `node scripts/book-digest/tests/framework-harness.mjs` | `framework-release.log`, zero failures; installed equals freshly built. |
| `node scripts/book-digest/tests/bridge-workflow-harness.mjs` | `bridge-adapters-release.log`; all three paths. |
| `node scripts/book-digest/make-math-workflow.mjs --check` | Up to date; targets `workflows-legacy`, never the framework installation. |
| `node scripts/book-digest/tests/math-workflow-harness.mjs` | `legacy-math-release.log`, zero failures. |
| `node C:/Users/chenx/.claude/skills/project-wiki/scripts/wiki.mjs check` | `wiki-release.log`; code and owning docs commit together, followed by post-commit freshness check. |

Negative fixtures cover invalid selections/ranges/types, each ineligible carrier class, direction/dependency restrictions, terminology/language faults, duplicate/conflicting pairs, failed jobs/reads, stale/moved endpoints, CAS races, interrupted/partial writes and two-process lock contention. Failed verification after a confirmed write names the changed carrier as `written_unverified`; later verification appends nothing. These checks establish deterministic compiler/audit/adapter behavior; they do not establish model judgment or a real Claude Code Workflow run.

## Frozen installation

Project scope: `E:\zet`. Changed module versions: `zt-bridge` 3, shared library 3 and report 4. Patch versions advanced during qualification rather than reusing installed identities.

| Artifact | Version | Fingerprint | SHA-256 |
| --- | --- | --- | --- |
| Shared base source | 2.1.2 | Composed into each variant | `221f24b0e9c993550af7a02d8f6f9486424730affdcad15f348f3a2d3beef414` |
| Economics installed script | 3.1.2 | `4ae602b1c446c9dc` | `742068612b01e7b00984c34def7f59bbab77a93e38b0b43e68cd02a38df53769` |
| Mathematics installed script | 2.2.2 | `171fa49f295e9850` | `5ad919c302ef02a1a256f322f8265d507f13bd1b3f60feaf93bae61c33e09107` |

```powershell
$assembler = 'C:/Users/chenx/.claude/skills/workflow-framework-protocol/tools/assemble.mjs'
node $assembler install scripts/book-digest/workflow/zt-econ-digest.blueprint.json --sim scripts/book-digest/workflow/zt-econ-digest.sim.json --scope project --project E:/zet
node $assembler install scripts/book-digest/workflow/zt-math-digest.blueprint.json --sim scripts/book-digest/workflow/zt-math-digest.sim.json --scope project --project E:/zet
node $assembler verify zt-econ-digest --scope project --project E:/zet
node $assembler verify zt-math-digest --scope project --project E:/zet
```

All commands exit 0; both verifies return `ok: true`, no problems and no library drift. Logs: `install-econ-final.log`, `install-math-final.log`, `verify-econ-final.log`, `verify-math-final.log`. Earlier economics 1.0.0/2.0.0/3.0.0/3.1.0/3.1.1 and math 2.0.0/2.1.0/2.2.0/2.2.1 histories remain. Locks/history/scripts are assembler output. The handwritten book adapter is separately tested; the legacy generator writes only its legacy destination.

## Installed guidance

`pwsh -NoProfile -File scripts/install-agent-integrations.ps1` exits 0. Twelve hash comparisons pass across Claude Code, Codex and DeepSeek Harness copies/junction targets (`installed-skill-hashes.json`):

| Source file | SHA-256 |
| --- | --- |
| `zettelkasten/SKILL.md` | `0e1dd9e7363d03cf833fdeeeee43511a48b729f9093d232754549bdc2b110b6d` |
| `zettelkasten/references/luhmann-and-practice.md` | `f8260f63a9ea381af7e5ef1c90435e3dfebc5f44d03ec909573cee8a603200d7` |
| `zt-digest-protocol/SKILL.md` | `26cb7e648f9651271c04ebf482dbfad843fda34146dec7d9d805a1f8be9c2c76` |
| `zt-digest-protocol/references/scripts.md` | `c867938a104420a9f6cd9c55df04c2e89821783e771910076f7c1235e6512171` |

A fresh DeepSeek client loaded installed guidance and restated insertion renumbering, whole-Card Links and human-split reporting. The fresh Claude Code CLI attempt stopped before an API call because it was not logged in. The user then explicitly deferred real tests; no additional login/runtime test is required for this source delivery.

## Service delivery

This script/guidance feature alone needs no `zt down`, `cargo install`, migration, schema backup or service restart. Separately, the user requested the latest tested development service. With zero live Sessions and no active digest, the exact release executable was installed and the daemon refreshed:

- `zt 0.20.0`, PID 48612, process `C:\Users\chenx\.cargo\bin\zt.exe`, archive `D:\zt`.
- Installed/release SHA-256 both `fd3538cdb14f2d908937e0bc532bef92a534292be63d075901ede580f48950f0`.
- Before/after: 8,052 Cards, zero Sessions, autostart on. GUI HTTP 200 at `http://127.0.0.1:4717/`; fresh MCP handshake reports 0.20.0 and 24 tools.
- Previous executable: `C:\Users\chenx\.cargo\bin\zt.exe.pre-luhmann-20261016-160622`.
- Consistent SQLite snapshot: `D:\zt\zt-release-20261016T210535Z.sqlite3`; integrity `ok`, 8,052 Cards, SHA-256 `92c2816c47106d9a7653e487bf1fe764525eb4e4e671edaf041a7ff22283f35d`.

Receipts: `service-refresh-receipt.json`, `service-mcp-handshake.jsonl`, `service-backup-receipt.json`. The supported `zt dp` attempt failed here with Windows error 206 (command line too long), so it is not counted as a backup. The retained SQLite snapshot uses verified `VACUUM INTO`; artifacts remain in place. It is a SQLite snapshot, not a successful full dump. No live Bridge paragraph or restore was performed.

## Deferred real qualification

[#62](https://github.com/liudesheng9/zet/issues/62) remains open for the real installed Workflow run and fresh Claude Code check. [#63](https://github.com/liudesheng9/zet/issues/63) remains open for report-specific user acceptance and actual live apply. [#50](https://github.com/liudesheng9/zet/issues/50) remains open until those gates have evidence. Math L6 is source/installed, not runtime-complete.

Before deferral, an exploratory direct-compiler report used a real read-only judge: 12 selected sources, 13 valid proposals, zero refusals and no live apply. All 8,052 logical Card texts/metadata matched before/after. Conversations/workspace were removed while report/retry inputs remain. Its local receipt holds the actual ids and paths. This predates final read-coverage fields and was not an installed Claude Code Workflow run; it does not close #62. Private Card bodies/proposals are not published to GitHub.

For the later Claude Code test, follow [the operator contract](zt-digest-framework.md#bridge-discovery-and-apply):

1. Start a fresh client, confirm installed guidance, choose a real already digested key and a fresh output folder, then discover with `mode: "bridge"`, `bridgeMax: 12`, `bridgeCandidates: 5` and an optional exact earlier-book filter.
2. Record installed identity, plan/report ids, read coverage, counts, exact directions/sentences, prechecks and read-only evidence. A report id grants no acceptance.
3. Obtain actual new/earlier section choices separately. Before the first live edit, retain a supported verified backup and keep filing quiet; no concurrent Organize.
4. Apply from the same reviewed folder with exact `bridgeReport` and `bridgeApply` / `bridgeApplyEarlier`. Earlier-only is valid. No source probe, replan, judge rerun or acceptance transfer by title.
5. Record persisted paragraphs, Relation kinds, Reverse links, old/new faults, Model membership, relevant unit load, idempotent repeat and the actual-Link audit. Retain partial/unverified results.

A stale report needs a fresh folder and review. Optional later Hubs and any archive restore are separate user decisions. Apply has no automatic rollback, model repair or maintenance.

## Review and fixes (2026-10-16, Claude Code)

A review of commit `3e066aa` found faults in the Bridge stage, the audit, and the docs. [The review and its fixes](slip-box-luhmann-review-and-fixes.md) lists each finding and its fix. The installed versions after the fixes are `zt-econ-digest` 3.1.3, `zt-math-digest` 2.2.3, and base 2.1.3. The gates above stay open.

`tests/bridge-human.tests.ps1` now seeds its scratch database with `node:sqlite`, so it needs no Python. It accepts `-Python` and ignores it. The command in the verification table above is the one that ran before the fix. Run it now without `-Python`.
