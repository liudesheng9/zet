# Luhmann comparison: published PRD and issue coverage

Status: published and read back from GitHub. Planning date: 2026-10-18.

Parent: [#93 — [PRD] Make the ZT box easier to enter, relate and answer across books](https://github.com/liudesheng9/zet/issues/93).

Source: [implementation design](luhmann-comparison-implementation-design.md), SHA-256 `dfb171ccb91f21ebbb6a6505ec3f467f6e6e093acda38f3c7fdbdb2bc18223fd`. The source remains unchanged. The user approved 29 focused issues: 24 AFK implementation slices and five HITL review/qualification gates. This record describes tracker publication, not feature implementation or authorization to apply historical archive edits.

## Publication verification

- All 30 newly created issue bodies and titles were fetched from GitHub and matched the exact published drafts.
- All remain open; parent and 24 AFK slices have `ready-for-agent`; five HITL gates have `ready-for-human`; all have `enhancement`.
- All 29 children reference the actual parent; every blocker uses its real GitHub issue URL, including the external #92 safety dependency.
- All 384 source clauses, 43 user stories and 231 named child acceptance criteria were checked in their actual live bodies.
- The parent was created once and was not edited after child publication. Existing issue #92's body was fetched again and remained unchanged.
- P6 and P10 remain dropped. Optional S29 is a separately selected historical batch, not an implicit release requirement.

## Feature coverage

| Kept proposal | Owning implementation issues |
| --- | --- |
| P1 | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S06 / #103](https://github.com/liudesheng9/zet/issues/103), [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| P2 | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S08 / #107](https://github.com/liudesheng9/zet/issues/107), [S14 / #111](https://github.com/liudesheng9/zet/issues/111), [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| P3 | [S05 / #98](https://github.com/liudesheng9/zet/issues/98), [S06 / #103](https://github.com/liudesheng9/zet/issues/103), [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| P4 | [S06 / #103](https://github.com/liudesheng9/zet/issues/103), [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| P5 | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S07 / #100](https://github.com/liudesheng9/zet/issues/100), [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| P7 | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| P8 | [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| P9 | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S25 / #102](https://github.com/liudesheng9/zet/issues/102) |
| P11 | [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| P12 | [S15 / #114](https://github.com/liudesheng9/zet/issues/114), [S16 / #116](https://github.com/liudesheng9/zet/issues/116), [S17 / #118](https://github.com/liudesheng9/zet/issues/118) |
| P13 | [S13 / #106](https://github.com/liudesheng9/zet/issues/106), [S14 / #111](https://github.com/liudesheng9/zet/issues/111) |
| P14 | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| P15 | [S19 / #108](https://github.com/liudesheng9/zet/issues/108), [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |

Audit/resource/access/compatibility contracts and delivery gates are traced by the full source-clause table below, rather than reduced to this feature-only summary.

## Issue sequence and dependencies

The publication order is topological; stable S identifiers retain the design breakdown. AFK describes implementation readiness, not bypass of listed blockers. HITL requires the specific human evidence named in that issue.

| Slice / issue | Type | Deliverable | Blocked by | Stories | Criteria |
| --- | --- | --- | --- | --- | --- |
| [S01 / #94](https://github.com/liudesheng9/zet/issues/94) | AFK | Report a reproducible Luhmann baseline with explicit source evidence | None | US001, US002 | 7 |
| [S02 / #95](https://github.com/liudesheng9/zet/issues/95) | AFK | Check standalone Card text and own-source Topic dependencies | None | US003, US004 | 9 |
| [S03 / #96](https://github.com/liudesheng9/zet/issues/96) | AFK | Distinguish Workspace titles and refuse hubs that copy anchor titles | None | US005 | 7 |
| [S04 / #97](https://github.com/liudesheng9/zet/issues/97) | AFK | Produce immutable concept, restatement and filing review reports | None | US006, US007 | 9 |
| [S05 / #98](https://github.com/liudesheng9/zet/issues/98) | AFK | Freeze bounded earlier-Card context before each digest group | None | US008, US009 | 8 |
| [S18 / #99](https://github.com/liudesheng9/zet/issues/99) | AFK | Create human Reply Cards from the agent reader | None | US031, US032 | 9 |
| [S07 / #100](https://github.com/liudesheng9/zet/issues/100) | AFK | Gate Topic creation by concept meaning with locked receipt replay | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S05 / #98](https://github.com/liudesheng9/zet/issues/98) | US012, US013 | 8 |
| [S10 / #101](https://github.com/liudesheng9/zet/issues/101) | AFK | Build and read a small concept register with substantive entries | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) | US018, US019 | 9 |
| [S25 / #102](https://github.com/liudesheng9/zet/issues/102) | HITL | Calibrate standalone and concept advice against reviewed examples | [S01 / #94](https://github.com/liudesheng9/zet/issues/94), [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S04 / #97](https://github.com/liudesheng9/zet/issues/97) | US039 | 5 |
| [S06 / #103](https://github.com/liudesheng9/zet/issues/103) | AFK | Write standalone Cards beside earlier thought with inline Links | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S05 / #98](https://github.com/liudesheng9/zet/issues/98), [S07 / #100](https://github.com/liudesheng9/zet/issues/100) | US010, US011 | 8 |
| [S11 / #104](https://github.com/liudesheng9/zet/issues/104) | AFK | Publish stable register parts with reviewed pinyin grouping and replay | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) | US020, US021 | 9 |
| [S12 / #105](https://github.com/liudesheng9/zet/issues/105) | AFK | Make hubs cross books and suppress redundant mirror hubs | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S10 / #101](https://github.com/liudesheng9/zet/issues/101) | US022 | 8 |
| [S13 / #106](https://github.com/liudesheng9/zet/issues/106) | AFK | Show entry-first results consistently in GUI and MCP search | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S10 / #101](https://github.com/liudesheng9/zet/issues/101) | US023, US024 | 9 |
| [S08 / #107](https://github.com/liudesheng9/zet/issues/107) | AFK | Resolve new restatements without rewriting finished models | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S06 / #103](https://github.com/liudesheng9/zet/issues/103) | US014, US015 | 9 |
| [S19 / #108](https://github.com/liudesheng9/zet/issues/108) | AFK | Offer one explained distant Card in reader and Table | [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S13 / #106](https://github.com/liudesheng9/zet/issues/106), [S18 / #99](https://github.com/liudesheng9/zet/issues/99) | US033 | 7 |
| [S22 / #109](https://github.com/liudesheng9/zet/issues/109) | AFK | Plan and apply an approved Topic join with fresh mappings | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) | US036 | 8 |
| [S09 / #110](https://github.com/liudesheng9/zet/issues/110) | AFK | Run the Link pass only for receipt-owned semantic gaps | [S06 / #103](https://github.com/liudesheng9/zet/issues/103), [S08 / #107](https://github.com/liudesheng9/zet/issues/107), [#92](https://github.com/liudesheng9/zet/issues/92) | US016, US017 | 8 |
| [S14 / #111](https://github.com/liudesheng9/zet/issues/111) | AFK | Fold only demonstrated restatements in entry search | [S08 / #107](https://github.com/liudesheng9/zet/issues/107), [S13 / #106](https://github.com/liudesheng9/zet/issues/106) | US025 | 7 |
| [S20 / #112](https://github.com/liudesheng9/zet/issues/112) | AFK | Use a selected Bridge report as an optional distant candidate source | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) | US034 | 8 |
| [S21 / #113](https://github.com/liudesheng9/zet/issues/113) | AFK | Resolve an approved Topic-home transfer while preserving old addresses | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S08 / #107](https://github.com/liudesheng9/zet/issues/107) | US035 | 8 |
| [S15 / #114](https://github.com/liudesheng9/zet/issues/114) | AFK | Gather an explained bounded Spread proposal from a question | [S13 / #106](https://github.com/liudesheng9/zet/issues/106), [S14 / #111](https://github.com/liudesheng9/zet/issues/111) | US026, US027 | 10 |
| [S23 / #115](https://github.com/liudesheng9/zet/issues/115) | AFK | Integrate the new digest paths into frozen variants and finite finalization | [S06 / #103](https://github.com/liudesheng9/zet/issues/103), [S08 / #107](https://github.com/liudesheng9/zet/issues/107), [S09 / #110](https://github.com/liudesheng9/zet/issues/110), [S11 / #104](https://github.com/liudesheng9/zet/issues/104), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) | US037 | 9 |
| [S16 / #116](https://github.com/liudesheng9/zet/issues/116) | AFK | Preview and keep Ask Spreads without overwriting the Table | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) | US028, US029 | 7 |
| [S26 / #117](https://github.com/liudesheng9/zet/issues/117) | HITL | Qualify one authorized book under the new frozen digest practices | [S23 / #115](https://github.com/liudesheng9/zet/issues/115), [S25 / #102](https://github.com/liudesheng9/zet/issues/102) | US040 | 8 |
| [S17 / #118](https://github.com/liudesheng9/zet/issues/118) | AFK | File a kept Ask Spread as an explicitly placed agent hub draft | [S12 / #105](https://github.com/liudesheng9/zet/issues/105), [S16 / #116](https://github.com/liudesheng9/zet/issues/116) | US030 | 6 |
| [S27 / #119](https://github.com/liudesheng9/zet/issues/119) | HITL | Review and publish the exact existing register and crossing-hub plan | [S11 / #104](https://github.com/liudesheng9/zet/issues/104), [S12 / #105](https://github.com/liudesheng9/zet/issues/105), [S26 / #117](https://github.com/liudesheng9/zet/issues/117) | US041 | 7 |
| [S24 / #120](https://github.com/liudesheng9/zet/issues/120) | AFK | Qualify the integrated APIs, GUI and scratch archive release | [S01 / #94](https://github.com/liudesheng9/zet/issues/94), [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S05 / #98](https://github.com/liudesheng9/zet/issues/98), [S07 / #100](https://github.com/liudesheng9/zet/issues/100), [S06 / #103](https://github.com/liudesheng9/zet/issues/103), [S08 / #107](https://github.com/liudesheng9/zet/issues/107), [S09 / #110](https://github.com/liudesheng9/zet/issues/110), [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S11 / #104](https://github.com/liudesheng9/zet/issues/104), [S12 / #105](https://github.com/liudesheng9/zet/issues/105), [S13 / #106](https://github.com/liudesheng9/zet/issues/106), [S14 / #111](https://github.com/liudesheng9/zet/issues/111), [S15 / #114](https://github.com/liudesheng9/zet/issues/114), [S16 / #116](https://github.com/liudesheng9/zet/issues/116), [S17 / #118](https://github.com/liudesheng9/zet/issues/118), [S18 / #99](https://github.com/liudesheng9/zet/issues/99), [S19 / #108](https://github.com/liudesheng9/zet/issues/108), [S20 / #112](https://github.com/liudesheng9/zet/issues/112), [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109), [S23 / #115](https://github.com/liudesheng9/zet/issues/115) | US038 | 9 |
| [S28 / #121](https://github.com/liudesheng9/zet/issues/121) | HITL | Qualify the installed reader release, usability and rollback | [S24 / #120](https://github.com/liudesheng9/zet/issues/120), [S26 / #117](https://github.com/liudesheng9/zet/issues/117), [S27 / #119](https://github.com/liudesheng9/zet/issues/119) | US042 | 8 |
| [S29 / #122](https://github.com/liudesheng9/zet/issues/122) | HITL | Apply a separately selected historical resolution batch | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109), [S28 / #121](https://github.com/liudesheng9/zet/issues/121) | US043 | 7 |

## Full design-clause ownership

D identifiers are exact publication clauses, not implementation results. Prose and list/table decisions were retained in the owning live bodies; illustrative code structures were expressed as prose to follow the issue-writing skills' rule against stale file paths/code snippets. Line links resolve the unchanged local source. Shared clauses have more than one owning ticket; each acceptance list delimits that ticket's end-to-end slice.

| Clause | Source section / line | Owning issue bodies |
| --- | --- | --- |
| D001 | [§0, line 3](luhmann-comparison-implementation-design.md#L3) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D002 | [§0, line 5](luhmann-comparison-implementation-design.md#L5) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D003 | [§0, line 7](luhmann-comparison-implementation-design.md#L7) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D004 | [§1, line 11](luhmann-comparison-implementation-design.md#L11) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D005 | [§1, line 13](luhmann-comparison-implementation-design.md#L13) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D006 | [§1, line 15](luhmann-comparison-implementation-design.md#L15) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D007 | [§1, line 16](luhmann-comparison-implementation-design.md#L16) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D008 | [§1, line 18](luhmann-comparison-implementation-design.md#L18) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D009 | [§1, line 20](luhmann-comparison-implementation-design.md#L20) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D010 | [§1, line 37](luhmann-comparison-implementation-design.md#L37) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D011 | [§1.1, line 43](luhmann-comparison-implementation-design.md#L43) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D012 | [§1.1, line 44](luhmann-comparison-implementation-design.md#L44) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D013 | [§1.1, line 45](luhmann-comparison-implementation-design.md#L45) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D014 | [§1.1, line 46](luhmann-comparison-implementation-design.md#L46) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D015 | [§1.1, line 47](luhmann-comparison-implementation-design.md#L47) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D016 | [§1.1, line 48](luhmann-comparison-implementation-design.md#L48) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D017 | [§1.1, line 49](luhmann-comparison-implementation-design.md#L49) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D018 | [§1.1, line 50](luhmann-comparison-implementation-design.md#L50) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D019 | [§1.1, line 51](luhmann-comparison-implementation-design.md#L51) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D020 | [§1.1, line 53](luhmann-comparison-implementation-design.md#L53) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D021 | [§2, line 59](luhmann-comparison-implementation-design.md#L59) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D022 | [§2, line 60](luhmann-comparison-implementation-design.md#L60) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D023 | [§2, line 61](luhmann-comparison-implementation-design.md#L61) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D024 | [§2, line 62](luhmann-comparison-implementation-design.md#L62) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D025 | [§2, line 63](luhmann-comparison-implementation-design.md#L63) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D026 | [§2, line 64](luhmann-comparison-implementation-design.md#L64) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D027 | [§2, line 65](luhmann-comparison-implementation-design.md#L65) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D028 | [§2, line 66](luhmann-comparison-implementation-design.md#L66) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D029 | [§2, line 67](luhmann-comparison-implementation-design.md#L67) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D030 | [§2, line 68](luhmann-comparison-implementation-design.md#L68) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D031 | [§2, line 69](luhmann-comparison-implementation-design.md#L69) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D032 | [§2, line 70](luhmann-comparison-implementation-design.md#L70) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D033 | [§2, line 71](luhmann-comparison-implementation-design.md#L71) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D034 | [§2, line 73](luhmann-comparison-implementation-design.md#L73) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D035 | [§2, line 75](luhmann-comparison-implementation-design.md#L75) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D036 | [§2.1, line 79](luhmann-comparison-implementation-design.md#L79) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D037 | [§2.1, line 80](luhmann-comparison-implementation-design.md#L80) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D038 | [§2.1, line 81](luhmann-comparison-implementation-design.md#L81) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D039 | [§2.1, line 82](luhmann-comparison-implementation-design.md#L82) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D040 | [§2.1, line 83](luhmann-comparison-implementation-design.md#L83) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D041 | [§3, line 87](luhmann-comparison-implementation-design.md#L87) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D042 | [§3, line 91](luhmann-comparison-implementation-design.md#L91) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D043 | [§3, line 92](luhmann-comparison-implementation-design.md#L92) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D044 | [§3, line 93](luhmann-comparison-implementation-design.md#L93) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D045 | [§3, line 94](luhmann-comparison-implementation-design.md#L94) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D046 | [§3, line 95](luhmann-comparison-implementation-design.md#L95) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D047 | [§3, line 96](luhmann-comparison-implementation-design.md#L96) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D048 | [§3, line 97](luhmann-comparison-implementation-design.md#L97) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D049 | [§3, line 98](luhmann-comparison-implementation-design.md#L98) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D050 | [§3, line 99](luhmann-comparison-implementation-design.md#L99) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D051 | [§3, line 100](luhmann-comparison-implementation-design.md#L100) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D052 | [§3, line 101](luhmann-comparison-implementation-design.md#L101) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D053 | [§3, line 102](luhmann-comparison-implementation-design.md#L102) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D054 | [§3, line 103](luhmann-comparison-implementation-design.md#L103) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D055 | [§3, line 104](luhmann-comparison-implementation-design.md#L104) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D056 | [§3, line 105](luhmann-comparison-implementation-design.md#L105) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D057 | [§3, line 107](luhmann-comparison-implementation-design.md#L107) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D058 | [§4.1, line 113](luhmann-comparison-implementation-design.md#L113) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D059 | [§4.1, line 115](luhmann-comparison-implementation-design.md#L115) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D060 | [§4.1, line 117](luhmann-comparison-implementation-design.md#L117) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D061 | [§4.1, line 133](luhmann-comparison-implementation-design.md#L133) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D062 | [§4.1, line 135](luhmann-comparison-implementation-design.md#L135) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D063 | [§4.2, line 139](luhmann-comparison-implementation-design.md#L139) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D064 | [§4.2, line 141](luhmann-comparison-implementation-design.md#L141) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D065 | [§4.2, line 143](luhmann-comparison-implementation-design.md#L143) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D066 | [§4.2, line 145](luhmann-comparison-implementation-design.md#L145) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D067 | [§4.3, line 149](luhmann-comparison-implementation-design.md#L149) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D068 | [§4.3, line 151](luhmann-comparison-implementation-design.md#L151) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D069 | [§4.3, line 153](luhmann-comparison-implementation-design.md#L153) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D070 | [§4.3, line 154](luhmann-comparison-implementation-design.md#L154) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D071 | [§4.3, line 155](luhmann-comparison-implementation-design.md#L155) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D072 | [§4.3, line 156](luhmann-comparison-implementation-design.md#L156) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D073 | [§4.3, line 157](luhmann-comparison-implementation-design.md#L157) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D074 | [§4.3, line 159](luhmann-comparison-implementation-design.md#L159) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D075 | [§4.3, line 161](luhmann-comparison-implementation-design.md#L161) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D076 | [§4.4, line 165](luhmann-comparison-implementation-design.md#L165) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D077 | [§4.4, line 169](luhmann-comparison-implementation-design.md#L169) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D078 | [§4.4, line 170](luhmann-comparison-implementation-design.md#L170) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D079 | [§4.4, line 171](luhmann-comparison-implementation-design.md#L171) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D080 | [§4.4, line 173](luhmann-comparison-implementation-design.md#L173) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D081 | [§4.4, line 175](luhmann-comparison-implementation-design.md#L175) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D082 | [§4.4, line 177](luhmann-comparison-implementation-design.md#L177) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D083 | [§4.5, line 181](luhmann-comparison-implementation-design.md#L181) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D084 | [§4.5, line 183](luhmann-comparison-implementation-design.md#L183) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D085 | [§4.5, line 193](luhmann-comparison-implementation-design.md#L193) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D086 | [§4.5, line 195](luhmann-comparison-implementation-design.md#L195) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D087 | [§4.5, line 197](luhmann-comparison-implementation-design.md#L197) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D088 | [§5.1, line 203](luhmann-comparison-implementation-design.md#L203) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D089 | [§5.1, line 205](luhmann-comparison-implementation-design.md#L205) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D090 | [§5.1, line 206](luhmann-comparison-implementation-design.md#L206) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D091 | [§5.1, line 207](luhmann-comparison-implementation-design.md#L207) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D092 | [§5.1, line 208](luhmann-comparison-implementation-design.md#L208) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D093 | [§5.1, line 209](luhmann-comparison-implementation-design.md#L209) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D094 | [§5.1, line 211](luhmann-comparison-implementation-design.md#L211) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D095 | [§5.1, line 213](luhmann-comparison-implementation-design.md#L213) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D096 | [§5.1, line 215](luhmann-comparison-implementation-design.md#L215) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D097 | [§5.2, line 219](luhmann-comparison-implementation-design.md#L219) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D098 | [§5.2, line 223](luhmann-comparison-implementation-design.md#L223) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D099 | [§5.2, line 224](luhmann-comparison-implementation-design.md#L224) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D100 | [§5.2, line 225](luhmann-comparison-implementation-design.md#L225) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D101 | [§5.2, line 226](luhmann-comparison-implementation-design.md#L226) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D102 | [§5.2, line 227](luhmann-comparison-implementation-design.md#L227) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D103 | [§5.2, line 229](luhmann-comparison-implementation-design.md#L229) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D104 | [§5.2, line 231](luhmann-comparison-implementation-design.md#L231) | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D105 | [§5.3, line 235](luhmann-comparison-implementation-design.md#L235) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D106 | [§5.3, line 237](luhmann-comparison-implementation-design.md#L237) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D107 | [§5.3, line 239](luhmann-comparison-implementation-design.md#L239) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D108 | [§5.3, line 241](luhmann-comparison-implementation-design.md#L241) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D109 | [§5.4, line 245](luhmann-comparison-implementation-design.md#L245) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D110 | [§5.4, line 247](luhmann-comparison-implementation-design.md#L247) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D111 | [§5.4, line 249](luhmann-comparison-implementation-design.md#L249) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D112 | [§5.4, line 250](luhmann-comparison-implementation-design.md#L250) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D113 | [§5.4, line 251](luhmann-comparison-implementation-design.md#L251) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D114 | [§5.4, line 253](luhmann-comparison-implementation-design.md#L253) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D115 | [§5.4, line 255](luhmann-comparison-implementation-design.md#L255) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D116 | [§5.4, line 257](luhmann-comparison-implementation-design.md#L257) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| D117 | [§5.5, line 261](luhmann-comparison-implementation-design.md#L261) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D118 | [§5.5, line 263](luhmann-comparison-implementation-design.md#L263) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D119 | [§5.5, line 265](luhmann-comparison-implementation-design.md#L265) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D120 | [§5.5, line 266](luhmann-comparison-implementation-design.md#L266) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D121 | [§5.5, line 267](luhmann-comparison-implementation-design.md#L267) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D122 | [§5.5, line 269](luhmann-comparison-implementation-design.md#L269) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D123 | [§5.5, line 271](luhmann-comparison-implementation-design.md#L271) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D124 | [§5.5, line 273](luhmann-comparison-implementation-design.md#L273) | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| D125 | [§6, line 277](luhmann-comparison-implementation-design.md#L277) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D126 | [§6.1, line 283](luhmann-comparison-implementation-design.md#L283) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D127 | [§6.1, line 284](luhmann-comparison-implementation-design.md#L284) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D128 | [§6.1, line 285](luhmann-comparison-implementation-design.md#L285) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D129 | [§6.1, line 286](luhmann-comparison-implementation-design.md#L286) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D130 | [§6.1, line 287](luhmann-comparison-implementation-design.md#L287) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D131 | [§6.1, line 289](luhmann-comparison-implementation-design.md#L289) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D132 | [§6.2, line 293](luhmann-comparison-implementation-design.md#L293) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D133 | [§6.2, line 295](luhmann-comparison-implementation-design.md#L295) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D134 | [§6.2, line 297](luhmann-comparison-implementation-design.md#L297) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| D135 | [§6.3, line 301](luhmann-comparison-implementation-design.md#L301) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D136 | [§6.3, line 303](luhmann-comparison-implementation-design.md#L303) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D137 | [§6.3, line 305](luhmann-comparison-implementation-design.md#L305) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D138 | [§6.3, line 307](luhmann-comparison-implementation-design.md#L307) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| D139 | [§7.1, line 313](luhmann-comparison-implementation-design.md#L313) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D140 | [§7.1, line 315](luhmann-comparison-implementation-design.md#L315) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D141 | [§7.1, line 317](luhmann-comparison-implementation-design.md#L317) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D142 | [§7.1, line 319](luhmann-comparison-implementation-design.md#L319) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D143 | [§7.1, line 331](luhmann-comparison-implementation-design.md#L331) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D144 | [§7.1, line 333](luhmann-comparison-implementation-design.md#L333) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D145 | [§7.1, line 335](luhmann-comparison-implementation-design.md#L335) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D146 | [§7.2, line 339](luhmann-comparison-implementation-design.md#L339) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D147 | [§7.2, line 341](luhmann-comparison-implementation-design.md#L341) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D148 | [§7.2, line 342](luhmann-comparison-implementation-design.md#L342) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D149 | [§7.2, line 343](luhmann-comparison-implementation-design.md#L343) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D150 | [§7.2, line 344](luhmann-comparison-implementation-design.md#L344) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D151 | [§7.2, line 345](luhmann-comparison-implementation-design.md#L345) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D152 | [§7.2, line 346](luhmann-comparison-implementation-design.md#L346) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D153 | [§7.2, line 348](luhmann-comparison-implementation-design.md#L348) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D154 | [§7.2, line 350](luhmann-comparison-implementation-design.md#L350) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D155 | [§7.3, line 354](luhmann-comparison-implementation-design.md#L354) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D156 | [§7.3, line 356](luhmann-comparison-implementation-design.md#L356) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D157 | [§7.3, line 357](luhmann-comparison-implementation-design.md#L357) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D158 | [§7.3, line 358](luhmann-comparison-implementation-design.md#L358) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D159 | [§7.3, line 360](luhmann-comparison-implementation-design.md#L360) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D160 | [§8.1, line 366](luhmann-comparison-implementation-design.md#L366) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D161 | [§8.1, line 368](luhmann-comparison-implementation-design.md#L368) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D162 | [§8.1, line 369](luhmann-comparison-implementation-design.md#L369) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D163 | [§8.1, line 370](luhmann-comparison-implementation-design.md#L370) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D164 | [§8.1, line 372](luhmann-comparison-implementation-design.md#L372) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D165 | [§8.1, line 374](luhmann-comparison-implementation-design.md#L374) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D166 | [§8.2, line 378](luhmann-comparison-implementation-design.md#L378) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D167 | [§8.2, line 380](luhmann-comparison-implementation-design.md#L380) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D168 | [§8.2, line 382](luhmann-comparison-implementation-design.md#L382) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D169 | [§8.2, line 384](luhmann-comparison-implementation-design.md#L384) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| D170 | [§8.3, line 388](luhmann-comparison-implementation-design.md#L388) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D171 | [§8.3, line 390](luhmann-comparison-implementation-design.md#L390) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D172 | [§8.3, line 391](luhmann-comparison-implementation-design.md#L391) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D173 | [§8.3, line 392](luhmann-comparison-implementation-design.md#L392) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D174 | [§8.3, line 394](luhmann-comparison-implementation-design.md#L394) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D175 | [§8.3, line 396](luhmann-comparison-implementation-design.md#L396) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D176 | [§8.4, line 400](luhmann-comparison-implementation-design.md#L400) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D177 | [§8.4, line 402](luhmann-comparison-implementation-design.md#L402) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D178 | [§8.4, line 409](luhmann-comparison-implementation-design.md#L409) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D179 | [§8.4, line 411](luhmann-comparison-implementation-design.md#L411) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D180 | [§8.4, line 413](luhmann-comparison-implementation-design.md#L413) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D181 | [§8.4, line 422](luhmann-comparison-implementation-design.md#L422) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D182 | [§8.4, line 424](luhmann-comparison-implementation-design.md#L424) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D183 | [§8.4, line 426](luhmann-comparison-implementation-design.md#L426) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D184 | [§8.4, line 428](luhmann-comparison-implementation-design.md#L428) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D185 | [§8.4, line 430](luhmann-comparison-implementation-design.md#L430) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D186 | [§8.5, line 434](luhmann-comparison-implementation-design.md#L434) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D187 | [§8.5, line 436](luhmann-comparison-implementation-design.md#L436) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D188 | [§8.5, line 438](luhmann-comparison-implementation-design.md#L438) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D189 | [§8.5, line 440](luhmann-comparison-implementation-design.md#L440) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D190 | [§8.5, line 442](luhmann-comparison-implementation-design.md#L442) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D191 | [§8.5, line 444](luhmann-comparison-implementation-design.md#L444) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D192 | [§9.1, line 450](luhmann-comparison-implementation-design.md#L450) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D193 | [§9.1, line 452](luhmann-comparison-implementation-design.md#L452) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D194 | [§9.1, line 459](luhmann-comparison-implementation-design.md#L459) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D195 | [§9.1, line 461](luhmann-comparison-implementation-design.md#L461) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D196 | [§9.1, line 463](luhmann-comparison-implementation-design.md#L463) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D197 | [§9.1, line 465](luhmann-comparison-implementation-design.md#L465) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D198 | [§9.2, line 469](luhmann-comparison-implementation-design.md#L469) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D199 | [§9.2, line 471](luhmann-comparison-implementation-design.md#L471) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D200 | [§9.2, line 473](luhmann-comparison-implementation-design.md#L473) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D201 | [§9.2, line 474](luhmann-comparison-implementation-design.md#L474) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D202 | [§9.2, line 475](luhmann-comparison-implementation-design.md#L475) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D203 | [§9.2, line 476](luhmann-comparison-implementation-design.md#L476) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D204 | [§9.2, line 477](luhmann-comparison-implementation-design.md#L477) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D205 | [§9.2, line 478](luhmann-comparison-implementation-design.md#L478) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D206 | [§9.2, line 480](luhmann-comparison-implementation-design.md#L480) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D207 | [§9.2, line 482](luhmann-comparison-implementation-design.md#L482) | [S14 / #111](https://github.com/liudesheng9/zet/issues/111) |
| D208 | [§9.2, line 484](luhmann-comparison-implementation-design.md#L484) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D209 | [§9.2, line 486](luhmann-comparison-implementation-design.md#L486) | [S14 / #111](https://github.com/liudesheng9/zet/issues/111) |
| D210 | [§9.2, line 488](luhmann-comparison-implementation-design.md#L488) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D211 | [§9.3, line 492](luhmann-comparison-implementation-design.md#L492) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D212 | [§9.3, line 494](luhmann-comparison-implementation-design.md#L494) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D213 | [§9.3, line 496](luhmann-comparison-implementation-design.md#L496) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D214 | [§9.3, line 498](luhmann-comparison-implementation-design.md#L498) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D215 | [§9.4, line 502](luhmann-comparison-implementation-design.md#L502) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D216 | [§9.4, line 504](luhmann-comparison-implementation-design.md#L504) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D217 | [§9.4, line 506](luhmann-comparison-implementation-design.md#L506) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D218 | [§9.4, line 508](luhmann-comparison-implementation-design.md#L508) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D219 | [§9.4, line 510](luhmann-comparison-implementation-design.md#L510) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D220 | [§9.4, line 511](luhmann-comparison-implementation-design.md#L511) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D221 | [§9.4, line 512](luhmann-comparison-implementation-design.md#L512) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D222 | [§9.4, line 513](luhmann-comparison-implementation-design.md#L513) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D223 | [§9.4, line 514](luhmann-comparison-implementation-design.md#L514) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D224 | [§9.4, line 516](luhmann-comparison-implementation-design.md#L516) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D225 | [§9.4, line 518](luhmann-comparison-implementation-design.md#L518) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D226 | [§9.5, line 522](luhmann-comparison-implementation-design.md#L522) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D227 | [§9.5, line 524](luhmann-comparison-implementation-design.md#L524) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D228 | [§9.5, line 529](luhmann-comparison-implementation-design.md#L529) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D229 | [§9.5, line 531](luhmann-comparison-implementation-design.md#L531) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D230 | [§9.5, line 533](luhmann-comparison-implementation-design.md#L533) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D231 | [§9.5, line 551](luhmann-comparison-implementation-design.md#L551) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D232 | [§9.5, line 553](luhmann-comparison-implementation-design.md#L553) | [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| D233 | [§9.5, line 555](luhmann-comparison-implementation-design.md#L555) | [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| D234 | [§9.5, line 557](luhmann-comparison-implementation-design.md#L557) | [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| D235 | [§9.6, line 561](luhmann-comparison-implementation-design.md#L561) | [S17 / #118](https://github.com/liudesheng9/zet/issues/118) |
| D236 | [§9.6, line 563](luhmann-comparison-implementation-design.md#L563) | [S17 / #118](https://github.com/liudesheng9/zet/issues/118) |
| D237 | [§9.6, line 565](luhmann-comparison-implementation-design.md#L565) | [S17 / #118](https://github.com/liudesheng9/zet/issues/118) |
| D238 | [§10.1, line 571](luhmann-comparison-implementation-design.md#L571) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D239 | [§10.1, line 573](luhmann-comparison-implementation-design.md#L573) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D240 | [§10.1, line 575](luhmann-comparison-implementation-design.md#L575) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D241 | [§10.1, line 576](luhmann-comparison-implementation-design.md#L576) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D242 | [§10.1, line 577](luhmann-comparison-implementation-design.md#L577) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D243 | [§10.1, line 578](luhmann-comparison-implementation-design.md#L578) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D244 | [§10.1, line 580](luhmann-comparison-implementation-design.md#L580) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D245 | [§10.1, line 582](luhmann-comparison-implementation-design.md#L582) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D246 | [§10.1, line 584](luhmann-comparison-implementation-design.md#L584) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D247 | [§10.1, line 586](luhmann-comparison-implementation-design.md#L586) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D248 | [§10.1, line 588](luhmann-comparison-implementation-design.md#L588) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D249 | [§10.2, line 592](luhmann-comparison-implementation-design.md#L592) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D250 | [§10.2, line 594](luhmann-comparison-implementation-design.md#L594) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D251 | [§10.2, line 596](luhmann-comparison-implementation-design.md#L596) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D252 | [§10.2, line 597](luhmann-comparison-implementation-design.md#L597) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D253 | [§10.2, line 598](luhmann-comparison-implementation-design.md#L598) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D254 | [§10.2, line 600](luhmann-comparison-implementation-design.md#L600) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D255 | [§10.2, line 602](luhmann-comparison-implementation-design.md#L602) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D256 | [§10.2, line 604](luhmann-comparison-implementation-design.md#L604) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D257 | [§10.3, line 608](luhmann-comparison-implementation-design.md#L608) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D258 | [§10.3, line 610](luhmann-comparison-implementation-design.md#L610) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D259 | [§10.3, line 612](luhmann-comparison-implementation-design.md#L612) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D260 | [§10.3, line 614](luhmann-comparison-implementation-design.md#L614) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D261 | [§10.3, line 616](luhmann-comparison-implementation-design.md#L616) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D262 | [§11.1, line 622](luhmann-comparison-implementation-design.md#L622) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D263 | [§11.1, line 624](luhmann-comparison-implementation-design.md#L624) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D264 | [§11.1, line 625](luhmann-comparison-implementation-design.md#L625) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D265 | [§11.1, line 626](luhmann-comparison-implementation-design.md#L626) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D266 | [§11.1, line 628](luhmann-comparison-implementation-design.md#L628) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D267 | [§11.1, line 630](luhmann-comparison-implementation-design.md#L630) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D268 | [§11.1, line 632](luhmann-comparison-implementation-design.md#L632) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| D269 | [§11.2, line 636](luhmann-comparison-implementation-design.md#L636) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D270 | [§11.2, line 638](luhmann-comparison-implementation-design.md#L638) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D271 | [§11.2, line 640](luhmann-comparison-implementation-design.md#L640) | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D272 | [§11.3, line 644](luhmann-comparison-implementation-design.md#L644) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D273 | [§11.3, line 646](luhmann-comparison-implementation-design.md#L646) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D274 | [§11.3, line 648](luhmann-comparison-implementation-design.md#L648) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D275 | [§12.1, line 656](luhmann-comparison-implementation-design.md#L656) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D276 | [§12.1, line 657](luhmann-comparison-implementation-design.md#L657) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D277 | [§12.1, line 658](luhmann-comparison-implementation-design.md#L658) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D278 | [§12.1, line 659](luhmann-comparison-implementation-design.md#L659) | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D279 | [§12.1, line 660](luhmann-comparison-implementation-design.md#L660) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D280 | [§12.1, line 661](luhmann-comparison-implementation-design.md#L661) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D281 | [§12.1, line 662](luhmann-comparison-implementation-design.md#L662) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D282 | [§12.1, line 663](luhmann-comparison-implementation-design.md#L663) | [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| D283 | [§12.1, line 664](luhmann-comparison-implementation-design.md#L664) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D284 | [§12.1, line 665](luhmann-comparison-implementation-design.md#L665) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D285 | [§12.1, line 667](luhmann-comparison-implementation-design.md#L667) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D286 | [§12.2, line 673](luhmann-comparison-implementation-design.md#L673) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| D287 | [§12.2, line 674](luhmann-comparison-implementation-design.md#L674) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D288 | [§12.2, line 675](luhmann-comparison-implementation-design.md#L675) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| D289 | [§12.2, line 676](luhmann-comparison-implementation-design.md#L676) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| D290 | [§12.2, line 677](luhmann-comparison-implementation-design.md#L677) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D291 | [§12.2, line 678](luhmann-comparison-implementation-design.md#L678) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| D292 | [§12.2, line 679](luhmann-comparison-implementation-design.md#L679) | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| D293 | [§12.2, line 680](luhmann-comparison-implementation-design.md#L680) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D294 | [§12.2, line 682](luhmann-comparison-implementation-design.md#L682) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D295 | [§12.2, line 684](luhmann-comparison-implementation-design.md#L684) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D296 | [§12.3, line 690](luhmann-comparison-implementation-design.md#L690) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D297 | [§12.3, line 691](luhmann-comparison-implementation-design.md#L691) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D298 | [§12.3, line 692](luhmann-comparison-implementation-design.md#L692) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D299 | [§12.3, line 693](luhmann-comparison-implementation-design.md#L693) | [S10 / #101](https://github.com/liudesheng9/zet/issues/101), [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| D300 | [§12.3, line 694](luhmann-comparison-implementation-design.md#L694) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D301 | [§12.3, line 695](luhmann-comparison-implementation-design.md#L695) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D302 | [§12.3, line 696](luhmann-comparison-implementation-design.md#L696) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| D303 | [§12.3, line 697](luhmann-comparison-implementation-design.md#L697) | [S04 / #97](https://github.com/liudesheng9/zet/issues/97), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D304 | [§12.3, line 698](luhmann-comparison-implementation-design.md#L698) | [S05 / #98](https://github.com/liudesheng9/zet/issues/98), [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| D305 | [§12.3, line 699](luhmann-comparison-implementation-design.md#L699) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| D306 | [§12.3, line 700](luhmann-comparison-implementation-design.md#L700) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| D307 | [§12.3, line 701](luhmann-comparison-implementation-design.md#L701) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114), [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| D308 | [§12.3, line 702](luhmann-comparison-implementation-design.md#L702) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| D309 | [§12.3, line 703](luhmann-comparison-implementation-design.md#L703) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| D310 | [§13.1, line 709](luhmann-comparison-implementation-design.md#L709) | [S23 / #115](https://github.com/liudesheng9/zet/issues/115) |
| D311 | [§13.1, line 711](luhmann-comparison-implementation-design.md#L711) | [S23 / #115](https://github.com/liudesheng9/zet/issues/115) |
| D312 | [§13.1, line 723](luhmann-comparison-implementation-design.md#L723) | [S23 / #115](https://github.com/liudesheng9/zet/issues/115) |
| D313 | [§13.1, line 725](luhmann-comparison-implementation-design.md#L725) | [S23 / #115](https://github.com/liudesheng9/zet/issues/115) |
| D314 | [§13.1, line 727](luhmann-comparison-implementation-design.md#L727) | [S23 / #115](https://github.com/liudesheng9/zet/issues/115) |
| D315 | [§13.2, line 733](luhmann-comparison-implementation-design.md#L733) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D316 | [§13.2, line 734](luhmann-comparison-implementation-design.md#L734) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D317 | [§13.2, line 735](luhmann-comparison-implementation-design.md#L735) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D318 | [§13.2, line 736](luhmann-comparison-implementation-design.md#L736) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D319 | [§13.2, line 737](luhmann-comparison-implementation-design.md#L737) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D320 | [§13.2, line 738](luhmann-comparison-implementation-design.md#L738) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D321 | [§13.2, line 739](luhmann-comparison-implementation-design.md#L739) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D322 | [§13.2, line 740](luhmann-comparison-implementation-design.md#L740) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D323 | [§13.2, line 741](luhmann-comparison-implementation-design.md#L741) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D324 | [§13.2, line 742](luhmann-comparison-implementation-design.md#L742) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D325 | [§13.2, line 743](luhmann-comparison-implementation-design.md#L743) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D326 | [§13.2, line 744](luhmann-comparison-implementation-design.md#L744) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D327 | [§13.2, line 745](luhmann-comparison-implementation-design.md#L745) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D328 | [§13.2, line 746](luhmann-comparison-implementation-design.md#L746) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D329 | [§13.2, line 747](luhmann-comparison-implementation-design.md#L747) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D330 | [§13.2, line 748](luhmann-comparison-implementation-design.md#L748) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D331 | [§13.2, line 750](luhmann-comparison-implementation-design.md#L750) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D332 | [§13.2, line 752](luhmann-comparison-implementation-design.md#L752) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D333 | [§14.1, line 760](luhmann-comparison-implementation-design.md#L760) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D334 | [§14.1, line 761](luhmann-comparison-implementation-design.md#L761) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D335 | [§14.1, line 762](luhmann-comparison-implementation-design.md#L762) | [S02 / #95](https://github.com/liudesheng9/zet/issues/95), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D336 | [§14.1, line 763](luhmann-comparison-implementation-design.md#L763) | [S03 / #96](https://github.com/liudesheng9/zet/issues/96), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D337 | [§14.1, line 764](luhmann-comparison-implementation-design.md#L764) | [S07 / #100](https://github.com/liudesheng9/zet/issues/100), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D338 | [§14.1, line 765](luhmann-comparison-implementation-design.md#L765) | [S08 / #107](https://github.com/liudesheng9/zet/issues/107), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D339 | [§14.1, line 766](luhmann-comparison-implementation-design.md#L766) | [S11 / #104](https://github.com/liudesheng9/zet/issues/104), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D340 | [§14.1, line 767](luhmann-comparison-implementation-design.md#L767) | [S12 / #105](https://github.com/liudesheng9/zet/issues/105), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D341 | [§14.1, line 768](luhmann-comparison-implementation-design.md#L768) | [S14 / #111](https://github.com/liudesheng9/zet/issues/111), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D342 | [§14.1, line 769](luhmann-comparison-implementation-design.md#L769) | [S15 / #114](https://github.com/liudesheng9/zet/issues/114), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D343 | [§14.1, line 770](luhmann-comparison-implementation-design.md#L770) | [S16 / #116](https://github.com/liudesheng9/zet/issues/116), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D344 | [§14.1, line 771](luhmann-comparison-implementation-design.md#L771) | [S18 / #99](https://github.com/liudesheng9/zet/issues/99), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D345 | [§14.1, line 772](luhmann-comparison-implementation-design.md#L772) | [S20 / #112](https://github.com/liudesheng9/zet/issues/112), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D346 | [§14.1, line 773](luhmann-comparison-implementation-design.md#L773) | [S21 / #113](https://github.com/liudesheng9/zet/issues/113), [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D347 | [§14.2, line 777](luhmann-comparison-implementation-design.md#L777) | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D348 | [§14.2, line 779](luhmann-comparison-implementation-design.md#L779) | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D349 | [§14.2, line 781](luhmann-comparison-implementation-design.md#L781) | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D350 | [§14.2, line 790](luhmann-comparison-implementation-design.md#L790) | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D351 | [§14.2, line 792](luhmann-comparison-implementation-design.md#L792) | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D352 | [§14.2, line 794](luhmann-comparison-implementation-design.md#L794) | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| D353 | [§14.3, line 798](luhmann-comparison-implementation-design.md#L798) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D354 | [§14.3, line 802](luhmann-comparison-implementation-design.md#L802) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D355 | [§14.3, line 803](luhmann-comparison-implementation-design.md#L803) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D356 | [§14.3, line 804](luhmann-comparison-implementation-design.md#L804) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D357 | [§14.3, line 805](luhmann-comparison-implementation-design.md#L805) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D358 | [§14.3, line 806](luhmann-comparison-implementation-design.md#L806) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D359 | [§14.3, line 807](luhmann-comparison-implementation-design.md#L807) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D360 | [§14.3, line 808](luhmann-comparison-implementation-design.md#L808) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D361 | [§14.3, line 809](luhmann-comparison-implementation-design.md#L809) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D362 | [§14.3, line 810](luhmann-comparison-implementation-design.md#L810) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D363 | [§14.3, line 811](luhmann-comparison-implementation-design.md#L811) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D364 | [§14.3, line 812](luhmann-comparison-implementation-design.md#L812) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D365 | [§14.3, line 814](luhmann-comparison-implementation-design.md#L814) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D366 | [§14.3, line 816](luhmann-comparison-implementation-design.md#L816) | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| D367 | [§15-phase-1, line 822](luhmann-comparison-implementation-design.md#L822) | [S25 / #102](https://github.com/liudesheng9/zet/issues/102) |
| D368 | [§15-phase-1, line 824](luhmann-comparison-implementation-design.md#L824) | [S25 / #102](https://github.com/liudesheng9/zet/issues/102) |
| D369 | [§15-phase-2, line 828](luhmann-comparison-implementation-design.md#L828) | [S26 / #117](https://github.com/liudesheng9/zet/issues/117) |
| D370 | [§15-phase-2, line 830](luhmann-comparison-implementation-design.md#L830) | [S26 / #117](https://github.com/liudesheng9/zet/issues/117) |
| D371 | [§15-phase-3, line 834](luhmann-comparison-implementation-design.md#L834) | [S27 / #119](https://github.com/liudesheng9/zet/issues/119) |
| D372 | [§15-phase-3, line 836](luhmann-comparison-implementation-design.md#L836) | [S27 / #119](https://github.com/liudesheng9/zet/issues/119) |
| D373 | [§15-phase-4, line 840](luhmann-comparison-implementation-design.md#L840) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D374 | [§15-phase-4, line 842](luhmann-comparison-implementation-design.md#L842) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D375 | [§15-phase-5, line 846](luhmann-comparison-implementation-design.md#L846) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D376 | [§15-phase-5, line 848](luhmann-comparison-implementation-design.md#L848) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D377 | [§15-phase-6, line 852](luhmann-comparison-implementation-design.md#L852) | [S29 / #122](https://github.com/liudesheng9/zet/issues/122) |
| D378 | [§15-phase-6, line 854](luhmann-comparison-implementation-design.md#L854) | [S29 / #122](https://github.com/liudesheng9/zet/issues/122) |
| D379 | [§15-release, line 858](luhmann-comparison-implementation-design.md#L858) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D380 | [§15-release, line 860](luhmann-comparison-implementation-design.md#L860) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D381 | [§15-release, line 862](luhmann-comparison-implementation-design.md#L862) | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| D382 | [§16, line 866](luhmann-comparison-implementation-design.md#L866) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D383 | [§16, line 868](luhmann-comparison-implementation-design.md#L868) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |
| D384 | [§16, line 870](luhmann-comparison-implementation-design.md#L870) | [PRD / #93](https://github.com/liudesheng9/zet/issues/93) |

## User-story ownership

| Story | Owning issue |
| --- | --- |
| US001 | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| US002 | [S01 / #94](https://github.com/liudesheng9/zet/issues/94) |
| US003 | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| US004 | [S02 / #95](https://github.com/liudesheng9/zet/issues/95) |
| US005 | [S03 / #96](https://github.com/liudesheng9/zet/issues/96) |
| US006 | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| US007 | [S04 / #97](https://github.com/liudesheng9/zet/issues/97) |
| US008 | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| US009 | [S05 / #98](https://github.com/liudesheng9/zet/issues/98) |
| US010 | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| US011 | [S06 / #103](https://github.com/liudesheng9/zet/issues/103) |
| US012 | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| US013 | [S07 / #100](https://github.com/liudesheng9/zet/issues/100) |
| US014 | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| US015 | [S08 / #107](https://github.com/liudesheng9/zet/issues/107) |
| US016 | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| US017 | [S09 / #110](https://github.com/liudesheng9/zet/issues/110) |
| US018 | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| US019 | [S10 / #101](https://github.com/liudesheng9/zet/issues/101) |
| US020 | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| US021 | [S11 / #104](https://github.com/liudesheng9/zet/issues/104) |
| US022 | [S12 / #105](https://github.com/liudesheng9/zet/issues/105) |
| US023 | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| US024 | [S13 / #106](https://github.com/liudesheng9/zet/issues/106) |
| US025 | [S14 / #111](https://github.com/liudesheng9/zet/issues/111) |
| US026 | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| US027 | [S15 / #114](https://github.com/liudesheng9/zet/issues/114) |
| US028 | [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| US029 | [S16 / #116](https://github.com/liudesheng9/zet/issues/116) |
| US030 | [S17 / #118](https://github.com/liudesheng9/zet/issues/118) |
| US031 | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| US032 | [S18 / #99](https://github.com/liudesheng9/zet/issues/99) |
| US033 | [S19 / #108](https://github.com/liudesheng9/zet/issues/108) |
| US034 | [S20 / #112](https://github.com/liudesheng9/zet/issues/112) |
| US035 | [S21 / #113](https://github.com/liudesheng9/zet/issues/113) |
| US036 | [S22 / #109](https://github.com/liudesheng9/zet/issues/109) |
| US037 | [S23 / #115](https://github.com/liudesheng9/zet/issues/115) |
| US038 | [S24 / #120](https://github.com/liudesheng9/zet/issues/120) |
| US039 | [S25 / #102](https://github.com/liudesheng9/zet/issues/102) |
| US040 | [S26 / #117](https://github.com/liudesheng9/zet/issues/117) |
| US041 | [S27 / #119](https://github.com/liudesheng9/zet/issues/119) |
| US042 | [S28 / #121](https://github.com/liudesheng9/zet/issues/121) |
| US043 | [S29 / #122](https://github.com/liudesheng9/zet/issues/122) |

## Live body fingerprints

These SHA-256 values cover fetched Markdown with CRLF normalized to LF. They identify the issue text verified for this publication; later legitimate tracker edits will change them.

| Issue | Body SHA-256 |
| --- | --- |
| [PRD / #93](https://github.com/liudesheng9/zet/issues/93) | `864aef69f4d3e499e35c5767ac1fe0a86af650e9a176d0425ad2793974dd6105` |
| [S01 / #94](https://github.com/liudesheng9/zet/issues/94) | `0bd96bc22abfdc5e340e90a2debd9df6bf17bdca2290662af3718e7204494218` |
| [S02 / #95](https://github.com/liudesheng9/zet/issues/95) | `c1cf84aef6b1c0662abbd49d07119682f7da8d43349634ce3ffa39ef4461dd09` |
| [S03 / #96](https://github.com/liudesheng9/zet/issues/96) | `8e13b8d64a39948bdd2112a4b83d32e170564a8fde9ee6b2f2e162950e4297e2` |
| [S04 / #97](https://github.com/liudesheng9/zet/issues/97) | `1010bbd6dab66eeee054a95cb2fbd1964d6cb1ba9525a3ec35997828ca1e61c1` |
| [S05 / #98](https://github.com/liudesheng9/zet/issues/98) | `164a83d2903682754e5920528de36b3a6f09cba0beac72b73510d50b03be555a` |
| [S18 / #99](https://github.com/liudesheng9/zet/issues/99) | `30def4c5817b34e7ee8e829bf69bf26aee0277fa70f0fbbf5515a15b5f865b1b` |
| [S07 / #100](https://github.com/liudesheng9/zet/issues/100) | `ac72119e004fed6678f0b3a61b2f797f1631d9994411653db2feae5c79f28729` |
| [S10 / #101](https://github.com/liudesheng9/zet/issues/101) | `e22a8613f4fb769e0c539b255fb49fca7b8b9000a417da777af666284ed83b29` |
| [S25 / #102](https://github.com/liudesheng9/zet/issues/102) | `e68747420b64139081328d178b48fcff3571c88c6f2c346d9a386e863b0ded7c` |
| [S06 / #103](https://github.com/liudesheng9/zet/issues/103) | `c807329257e81b4b7956efd1683a72faa269b17e3cf9caebcf5e4c74191dc5b9` |
| [S11 / #104](https://github.com/liudesheng9/zet/issues/104) | `acd54cece431734a8a221557f0f8bef0120aff89c1498f36ca29fd6648760a0a` |
| [S12 / #105](https://github.com/liudesheng9/zet/issues/105) | `1b937d1a0c6559a16a33e7a7a058f0ab91130a97c548410a0f07345d42314fb7` |
| [S13 / #106](https://github.com/liudesheng9/zet/issues/106) | `1f5793e4fe9fd1e343639936d0ca63caa5b25c50477224a333f572bdd9ad0445` |
| [S08 / #107](https://github.com/liudesheng9/zet/issues/107) | `4366f12dde0d32553bfd95c621f1f263421325f3eabdacbe601056f68dd4c650` |
| [S19 / #108](https://github.com/liudesheng9/zet/issues/108) | `34b2df20e3305f294a8cbd9da2a3abba45bcc98998da23e6ecf262867fbc9c12` |
| [S22 / #109](https://github.com/liudesheng9/zet/issues/109) | `27f385ad57248793217eb1deab4bfe4bcd062ddab2dbc2873beea9b200b2243d` |
| [S09 / #110](https://github.com/liudesheng9/zet/issues/110) | `e7be6154bcb23ff519c06ddf7d0dd61dabd6e8497d9d8e0762bceb1454728d0a` |
| [S14 / #111](https://github.com/liudesheng9/zet/issues/111) | `64375ab9f01914189c9b693c83db92bb5e130e9b879fe5f1eac34e82a017ebd7` |
| [S20 / #112](https://github.com/liudesheng9/zet/issues/112) | `7ce8e45bc579607e1d02e03b089acde781e7a1d9e3db8408797ae26a6eeb501f` |
| [S21 / #113](https://github.com/liudesheng9/zet/issues/113) | `0eeed9cec4531715616b9fe1db73aafd5baae6ffc91c855d09816ed08c991905` |
| [S15 / #114](https://github.com/liudesheng9/zet/issues/114) | `eb1ce557701f9e70cc27f665342e3c18869979c85e462beb8dd6eeef662a77ae` |
| [S23 / #115](https://github.com/liudesheng9/zet/issues/115) | `0a72f86894a6cf9f29d731a2213ce3896e1278e4506609c0e608ce3c7747e5e0` |
| [S16 / #116](https://github.com/liudesheng9/zet/issues/116) | `f581c79932ae26537d79026ccc03729031f390dabd54f176bfd20faadc072f81` |
| [S26 / #117](https://github.com/liudesheng9/zet/issues/117) | `aa6e3508cf0fecd6acc6ddfde610df3c4ac3990168aaaf066eb4c09d07d5fe38` |
| [S17 / #118](https://github.com/liudesheng9/zet/issues/118) | `76ec3a0ef7cefa46e967771966860fb926816e744cfb333d65be344aad7575ee` |
| [S27 / #119](https://github.com/liudesheng9/zet/issues/119) | `32f8c7251473ccd3733b8d59b60daa1d4d14a78dde157d7afbd875a84368a2dc` |
| [S24 / #120](https://github.com/liudesheng9/zet/issues/120) | `a348def94e847465b0f9371a0e45abdfcb27ba19da43ba696828fe1e62b9ced1` |
| [S28 / #121](https://github.com/liudesheng9/zet/issues/121) | `4843e5169a81712d4f27934a3fe04f148e8f291d91913eab5009626b78a1700c` |
| [S29 / #122](https://github.com/liudesheng9/zet/issues/122) | `0c3ed973596a0bb2fe2916cdaad3a2e65cb059bcfb703dc02d7ca9026f61856c` |
