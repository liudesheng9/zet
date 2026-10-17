# Digest reliability: published PRD and implementation issues

PRD: [#64 — Reliable and fast digest runs with verified one-resume recovery](https://github.com/liudesheng9/zet/issues/64)

The user approved 27 focused slices and the existing temporary-archive MCP/CLI, Snapshot oracle, compiler/provider/process-failure and frozen-workflow test seams. This publishes requirements; implementation and live-book completion are not claimed.

Source: [digest-run-stiglitz2014-implementation-design.md](digest-run-stiglitz2014-implementation-design.md); SHA-256 `a3c52a99609571e1817385c16e6c45d9a917e6206729441a743df1a8a8a56559`.

## Issue map

| Slice | GitHub issue | Type | Blocked by | User stories |
| --- | --- | --- | --- | --- |
| DG01 | [#65 — Maintain indexed Links through every Card operation and archive migration](https://github.com/liudesheng9/zet/issues/65) | AFK | None | US11, US12, US13, US14 |
| DG02 | [#66 — Serve complete Card and model views through targeted and bulk MCP reads](https://github.com/liudesheng9/zet/issues/66) | AFK | [#65](https://github.com/liudesheng9/zet/issues/65) | US06, US07, US08, US14 |
| DG03 | [#67 — Make digest snapshots, Card checks and link preparation use bounded bulk reads](https://github.com/liudesheng9/zet/issues/67) | AFK | [#66](https://github.com/liudesheng9/zet/issues/66) | US09, US10, US56 |
| DG04 | [#68 — Apply bounded Card batches with archive-atomic replay receipts](https://github.com/liudesheng9/zet/issues/68) | AFK | [#65](https://github.com/liudesheng9/zet/issues/65) | US15, US16, US17, US60 |
| DG05 | [#69 — Correct a link pass paragraph through its exact committed receipt](https://github.com/liudesheng9/zet/issues/69) | AFK | [#68](https://github.com/liudesheng9/zet/issues/68) | US18, US44 |
| DG06 | [#70 — Track one prepared digest group from ledger claim through verified completion](https://github.com/liudesheng9/zet/issues/70) | AFK | [#68](https://github.com/liudesheng9/zet/issues/68), [#67](https://github.com/liudesheng9/zet/issues/67) | US01, US02, US20, US21, US25, US26, US27, US28 |
| DG07 | [#71 — Recover lost owners and background commands without a second writer](https://github.com/liudesheng9/zet/issues/71) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70) | US21, US22, US23, US24, US38 |
| DG08 | [#72 — Repair only unfinished model parts and wait for actual prerequisite outputs](https://github.com/liudesheng9/zet/issues/72) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#71](https://github.com/liudesheng9/zet/issues/71) | US04, US29, US30, US32 |
| DG09 | [#73 — Settle every Event and Finding candidate and pending job](https://github.com/liudesheng9/zet/issues/73) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#71](https://github.com/liudesheng9/zet/issues/71) | US25, US33 |
| DG10 | [#74 — Complete formal derivation explanations in full and repair modes](https://github.com/liudesheng9/zet/issues/74) | AFK | [#72](https://github.com/liudesheng9/zet/issues/72) | US31 |
| DG11 | [#75 — Detect and prevent repeated paragraphs and duplicated Card titles](https://github.com/liudesheng9/zet/issues/75) | AFK | None | US19 |
| DG12 | [#76 — Use one revision-scoped orphan definition in audits and link checks](https://github.com/liudesheng9/zet/issues/76) | AFK | [#67](https://github.com/liudesheng9/zet/issues/67) | US10, US34 |
| DG13 | [#81 — Run one book Card cycle with source-review coverage and explicit quality outcomes](https://github.com/liudesheng9/zet/issues/81) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#72](https://github.com/liudesheng9/zet/issues/72), [#73](https://github.com/liudesheng9/zet/issues/73), [#75](https://github.com/liudesheng9/zet/issues/75), [#76](https://github.com/liudesheng9/zet/issues/76), [#80](https://github.com/liudesheng9/zet/issues/80) | US32, US35 |
| DG14 | [#77 — Publish repaired model downstream work as new checked revisions](https://github.com/liudesheng9/zet/issues/77) | AFK | [#72](https://github.com/liudesheng9/zet/issues/72), [#73](https://github.com/liudesheng9/zet/issues/73), [#69](https://github.com/liudesheng9/zet/issues/69), [#70](https://github.com/liudesheng9/zet/issues/70) | US25, US41, US42, US44 |
| DG15 | [#82 — Drive restricted compiler chains with the frozen ready-work schedule](https://github.com/liudesheng9/zet/issues/82) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#71](https://github.com/liudesheng9/zet/issues/71), [#72](https://github.com/liudesheng9/zet/issues/72), [#73](https://github.com/liudesheng9/zet/issues/73), [#81](https://github.com/liudesheng9/zet/issues/81), [#77](https://github.com/liudesheng9/zet/issues/77) | US24, US26, US27, US35, US36, US37, US38, US39, US40 |
| DG16 | [#78 — Track mathematics units, proof reviews and Theory publication in the same ledger](https://github.com/liudesheng9/zet/issues/78) | AFK | [#72](https://github.com/liudesheng9/zet/issues/72), [#74](https://github.com/liudesheng9/zet/issues/74), [#77](https://github.com/liudesheng9/zet/issues/77), [#70](https://github.com/liudesheng9/zet/issues/70) | US43 |
| DG17 | [#87 — Resume the entire unresolved dependency closure once without rebuilding done work](https://github.com/liudesheng9/zet/issues/87) | AFK | [#74](https://github.com/liudesheng9/zet/issues/74), [#77](https://github.com/liudesheng9/zet/issues/77), [#82](https://github.com/liudesheng9/zet/issues/82), [#78](https://github.com/liudesheng9/zet/issues/78), [#83](https://github.com/liudesheng9/zet/issues/83), [#85](https://github.com/liudesheng9/zet/issues/85), [#86](https://github.com/liudesheng9/zet/issues/86) | US03, US04, US05, US41, US42, US44, US59 |
| DG18 | [#79 — Resolve a book title or Work id into an explicit preparation manifest](https://github.com/liudesheng9/zet/issues/79) | AFK | None | US45, US46 |
| DG19 | [#80 — Supply chapter endnotes and preserve original-location source-fault reports](https://github.com/liudesheng9/zet/issues/80) | AFK | [#79](https://github.com/liudesheng9/zet/issues/79) | US47, US48 |
| DG20 | [#83 — Settle owned DeepSeek resources and scratch cleanup from ledger evidence](https://github.com/liudesheng9/zet/issues/83) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#71](https://github.com/liudesheng9/zet/issues/71) | US22, US37, US50, US51 |
| DG21 | [#84 — Expose bounded lock waits and daemon restart diagnostics](https://github.com/liudesheng9/zet/issues/84) | AFK | [#65](https://github.com/liudesheng9/zet/issues/65), [#71](https://github.com/liudesheng9/zet/issues/71) | US38, US52 |
| DG22 | [#85 — Report verified coverage, placeholders and exact per-run usage](https://github.com/liudesheng9/zet/issues/85) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#71](https://github.com/liudesheng9/zet/issues/71), [#77](https://github.com/liudesheng9/zet/issues/77), [#83](https://github.com/liudesheng9/zet/issues/83) | US01, US02, US05, US53, US54, US59 |
| DG23 | [#86 — Finish with one bounded audit-fix-refresh-final-audit path](https://github.com/liudesheng9/zet/issues/86) | AFK | [#81](https://github.com/liudesheng9/zet/issues/81), [#77](https://github.com/liudesheng9/zet/issues/77), [#74](https://github.com/liudesheng9/zet/issues/74), [#78](https://github.com/liudesheng9/zet/issues/78) | US49 |
| DG24 | [#88 — Adopt legacy digest staging through a verified reconciliation plan](https://github.com/liudesheng9/zet/issues/88) | AFK | [#70](https://github.com/liudesheng9/zet/issues/70), [#71](https://github.com/liudesheng9/zet/issues/71), [#72](https://github.com/liudesheng9/zet/issues/72), [#77](https://github.com/liudesheng9/zet/issues/77), [#78](https://github.com/liudesheng9/zet/issues/78), [#83](https://github.com/liudesheng9/zet/issues/83) | US04, US57 |
| DG25 | [#89 — Qualify both frozen variants and the isolated performance/correctness release](https://github.com/liudesheng9/zet/issues/89) | AFK | All preceding implementation slices (listed individually in the issue) | US55, US56, US59, US60 |
| DG26 | [#90 — Deploy matching binaries and qualify a real bounded digest/resume pilot](https://github.com/liudesheng9/zet/issues/90) | HITL | [#89](https://github.com/liudesheng9/zet/issues/89) | US03, US04, US44, US55, US56, US59 |
| DG27 | [#91 — Reconcile and finish the Stiglitz backlog with one authorized resume](https://github.com/liudesheng9/zet/issues/91) | HITL | [#90](https://github.com/liudesheng9/zet/issues/90), [#88](https://github.com/liudesheng9/zet/issues/88) | US04, US05, US44, US48, US58, US59 |

## Architecture represented in the issue text

The issues reproduce the complete relevant design clauses, replacing source-file locators with stable module responsibilities and code examples with their decision-rich prose. The architecture retains two binaries, four focused module owners, one narrow runner definition, one five-table run ledger, a derived Link index and archive-atomic write receipts. There is no new service, read cache or general workflow engine. Each issue includes actual contracts, acceptance criteria and complete named test scenarios, rather than relying on a shared design link.

## Publication verification

Live GitHub read-back: **PASS**. Verified one PRD, 27 children, exact published bodies, state/labels, real parent/blocker references and dependency order; covered 60 user stories, 506 extracted design clauses, all L/A/W/I/G/M4 identifiers, delivery gates D0–D7 and 32 named acceptance scenarios.

[Machine-readable coverage audit](digest-run-stiglitz2014-github-coverage.json) records clause-to-live-issue evidence, exact body hashes, user-story/test mappings and every check result. Cross-reference-only source rows are identified separately from direct operative clause-text checks.

The 25 AFK slices are ready-for-agent. The final two live qualification/recovery slices are ready-for-human; actual maintenance windows, provider spend and live Card changes retain their explicit authorization/checkpoint boundaries. Existing Bridge review/application issues remain separate and were not closed or edited.
